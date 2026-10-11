use std::{
    error::Error,
    ffi::{OsStr, OsString},
    fmt, fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

use crate::{
    filename_sanitizer::sanitize_stem, ProcessingStage, ProcessingState, RenameHistoryEntry,
    ScreenshotFile,
};

pub struct RecordedRename {
    pub history: RenameHistoryEntry,
    pub result: Result<PathBuf, RenameError>,
}

/// Runs the filesystem rename and returns its history record with the result.
/// A caller can persist the record before presenting a success as undoable.
pub fn rename_screenshot_recorded(
    screenshot: &mut ScreenshotFile,
    candidate_stem: &str,
) -> RecordedRename {
    let mut history =
        RenameHistoryEntry::new(screenshot.original_path().to_path_buf(), SystemTime::now())
            .expect("screenshot path and current time are valid for history");
    let result = rename_screenshot(screenshot, candidate_stem);
    match &result {
        Ok(destination) => history
            .mark_rename_succeeded(destination.clone())
            .expect("valid destination"),
        Err(error) => {
            let destination = match error {
                RenameError::Io {
                    destination_path, ..
                }
                | RenameError::CollisionLimit { destination_path } => {
                    Some(destination_path.clone())
                }
                _ => None,
            };
            history
                .mark_rename_failed(destination, error.to_string())
                .expect("valid failure destination");
        }
    }
    RecordedRename { history, result }
}

/// Renames a screenshot to a sanitized filename stem in the same directory.
///
/// The source extension is retained. If the requested name already exists,
/// numbered suffixes such as `name (2).png` are tried until an unused path is
/// found. A case-only destination that resolves to the source is treated as
/// already renamed. Failures are returned to the caller and recorded on the
/// screenshot so a watcher can report the problem and continue processing.
pub fn rename_screenshot(
    screenshot: &mut ScreenshotFile,
    candidate_stem: &str,
) -> Result<PathBuf, RenameError> {
    rename_screenshot_with(screenshot, candidate_stem, rename_without_replacement)
}

fn rename_screenshot_with<F>(
    screenshot: &mut ScreenshotFile,
    candidate_stem: &str,
    mut move_file: F,
) -> Result<PathBuf, RenameError>
where
    F: FnMut(&Path, &Path) -> io::Result<()>,
{
    screenshot.transition_to(ProcessingState::Renaming);

    let result = rename_file(
        screenshot.original_path(),
        screenshot.extension(),
        candidate_stem,
        &mut move_file,
    );
    match &result {
        Ok(_) => screenshot.transition_to(ProcessingState::Renamed),
        Err(error) => screenshot.transition_to(ProcessingState::Failed {
            stage: ProcessingStage::Rename,
            message: error.to_string(),
        }),
    }

    result
}

#[derive(Debug)]
pub enum RenameError {
    InvalidCandidateName {
        candidate: String,
        reason: &'static str,
    },
    SourceNotFile {
        path: PathBuf,
        reason: String,
    },
    Io {
        source_path: PathBuf,
        destination_path: PathBuf,
        source: io::Error,
    },
    CollisionLimit {
        destination_path: PathBuf,
    },
}

impl RenameError {
    pub fn destination_path(&self) -> Option<&Path> {
        match self {
            Self::Io {
                destination_path, ..
            }
            | Self::CollisionLimit { destination_path } => Some(destination_path),
            _ => None,
        }
    }

    pub fn category(&self) -> crate::rename_recovery::RenameFailureCategory {
        use crate::rename_recovery::RenameFailureCategory;
        match self {
            Self::InvalidCandidateName { .. } => RenameFailureCategory::InvalidName,
            Self::CollisionLimit { .. } => RenameFailureCategory::CollisionExhausted,
            Self::SourceNotFile { path, .. } if matches!(fs::symlink_metadata(path), Err(error) if error.kind() == io::ErrorKind::NotFound) => {
                RenameFailureCategory::SourceMissing
            }
            Self::Io { source, .. } if source.kind() == io::ErrorKind::NotFound => {
                RenameFailureCategory::SourceMissing
            }
            _ => RenameFailureCategory::PermissionIo,
        }
    }
}

impl fmt::Display for RenameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCandidateName { candidate, reason } => write!(
                formatter,
                "cannot rename screenshot to {candidate:?}: {reason}"
            ),
            Self::SourceNotFile { path, reason } => write!(
                formatter,
                "cannot rename screenshot at {}: source file is unavailable ({reason}); check that it exists and is accessible",
                path.display()
            ),
            Self::Io {
                source_path,
                destination_path,
                source,
            } => write!(
                formatter,
                "could not rename {} to {}: {source}; check that the file is available and the folder is writable",
                source_path.display(),
                destination_path.display()
            ),
            Self::CollisionLimit { destination_path } => write!(
                formatter,
                "could not find an available filename near {}; move or remove existing files and try again",
                destination_path.display()
            ),
        }
    }
}

impl Error for RenameError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

fn rename_file<F>(
    source: &Path,
    extension: Option<&OsStr>,
    candidate_stem: &str,
    move_file: &mut F,
) -> Result<PathBuf, RenameError>
where
    F: FnMut(&Path, &Path) -> io::Result<()>,
{
    let stem =
        sanitize_stem(candidate_stem).map_err(|error| RenameError::InvalidCandidateName {
            candidate: candidate_stem.to_owned(),
            reason: error.reason(),
        })?;
    let metadata = fs::symlink_metadata(source).map_err(|error| RenameError::SourceNotFile {
        path: source.to_path_buf(),
        reason: error.to_string(),
    })?;
    if !metadata.file_type().is_file() {
        return Err(RenameError::SourceNotFile {
            path: source.to_path_buf(),
            reason: "the source is not a regular file".to_owned(),
        });
    }

    let parent = source.parent().unwrap_or_else(|| Path::new(""));
    let mut suffix = None;

    loop {
        let name = destination_name(&stem, suffix, extension);
        let destination = parent.join(name);
        if destination == source {
            return Ok(source.to_path_buf());
        }
        if fs::symlink_metadata(&destination).is_ok() {
            if paths_resolve_to_same_file(source, &destination) {
                return Ok(source.to_path_buf());
            }
            suffix = Some(next_collision_suffix(suffix, &destination)?);
            continue;
        }

        match move_file(source, &destination) {
            Ok(()) => return Ok(destination),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                suffix = Some(next_collision_suffix(suffix, &destination)?);
            }
            Err(source_error) => {
                return Err(RenameError::Io {
                    source_path: source.to_path_buf(),
                    destination_path: destination,
                    source: source_error,
                })
            }
        }
    }
}

fn paths_resolve_to_same_file(source: &Path, destination: &Path) -> bool {
    match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.file_type().is_file() => {}
        _ => return false,
    }

    match (fs::canonicalize(source), fs::canonicalize(destination)) {
        (Ok(source), Ok(destination)) => source == destination,
        _ => false,
    }
}

fn next_collision_suffix(current: Option<u32>, destination: &Path) -> Result<u32, RenameError> {
    if current.is_some_and(|suffix| suffix >= 10_000) {
        return Err(RenameError::CollisionLimit {
            destination_path: destination.to_path_buf(),
        });
    }
    match current {
        Some(current) => current
            .checked_add(1)
            .ok_or_else(|| RenameError::CollisionLimit {
                destination_path: destination.to_path_buf(),
            }),
        None => Ok(2),
    }
}

fn destination_name(stem: &str, suffix: Option<u32>, extension: Option<&OsStr>) -> OsString {
    let mut name = OsString::from(stem);
    if let Some(suffix) = suffix {
        name.push(format!(" ({suffix})"));
    }
    if let Some(extension) = extension {
        name.push(".");
        name.push(extension);
    }
    name
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn MoveFileW(existing_file_name: *const u16, new_file_name: *const u16) -> i32;
}

#[cfg(windows)]
pub(crate) fn rename_without_replacement(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    let source_wide = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination_wide = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    // MoveFileW refuses an existing destination; same-folder moves stay on one volume.
    let succeeded = unsafe { MoveFileW(source_wide.as_ptr(), destination_wide.as_ptr()) };

    if succeeded != 0 {
        Ok(())
    } else {
        let error = io::Error::last_os_error();
        let destination_is_missing = matches!(
            fs::symlink_metadata(destination),
            Err(ref metadata_error) if metadata_error.kind() == io::ErrorKind::NotFound
        );

        if error.kind() == io::ErrorKind::PermissionDenied && destination_is_missing {
            return copy_without_replacement(source, destination).map_err(|fallback_error| {
                io::Error::new(
                    fallback_error.kind(),
                    format!(
                        "atomic rename failed ({error}); safe copy fallback failed ({fallback_error})"
                    ),
                )
            });
        }

        Err(error)
    }
}

fn copy_without_replacement(source: &Path, destination: &Path) -> io::Result<()> {
    use std::io::Write;

    let mut destination_file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    let copy_result = (|| {
        let mut source_file = fs::File::open(source)?;
        io::copy(&mut source_file, &mut destination_file)?;
        destination_file.flush()?;
        fs::remove_file(source)
    })();

    if let Err(error) = copy_result {
        drop(destination_file);
        if let Err(cleanup_error) = fs::remove_file(destination) {
            if cleanup_error.kind() != io::ErrorKind::NotFound {
                return Err(io::Error::new(
                    error.kind(),
                    format!(
                        "copy or source removal failed ({error}); partial destination cleanup failed ({cleanup_error})"
                    ),
                ));
            }
        }
        return Err(error);
    }

    Ok(())
}

#[cfg(not(windows))]
pub(crate) fn rename_without_replacement(source: &Path, destination: &Path) -> io::Result<()> {
    rename_through_hard_link_or_copy(source, destination, fs::hard_link)
}

#[cfg(any(not(windows), test))]
fn rename_through_hard_link_or_copy<F>(
    source: &Path,
    destination: &Path,
    try_hard_link: F,
) -> io::Result<()>
where
    F: FnOnce(&Path, &Path) -> io::Result<()>,
{
    match try_hard_link(source, destination) {
        Ok(()) => {
            if let Err(error) = fs::remove_file(source) {
                if let Err(cleanup_error) = fs::remove_file(destination) {
                    return Err(io::Error::new(
                        error.kind(),
                        format!(
                            "could not remove the original source ({error}) or roll back the destination link ({cleanup_error})"
                        ),
                    ));
                }
                return Err(error);
            }
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Err(error),
        Err(link_error) => copy_without_replacement(source, destination).map_err(|copy_error| {
            io::Error::new(
                copy_error.kind(),
                format!(
                    "hard-link rename failed ({link_error}); safe copy fallback failed ({copy_error})"
                ),
            )
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, io, path::PathBuf};

    use tempfile::tempdir;

    use crate::{ProcessingStage, ProcessingState, ScreenshotFile};

    use super::{
        rename_screenshot, rename_screenshot_with, rename_through_hard_link_or_copy,
        rename_without_replacement, RenameError,
    };

    #[test]
    fn collision_exhaustion_is_bounded_and_keeps_the_screenshot() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("Screenshot.png");
        fs::write(&source, b"image").unwrap();
        let mut screenshot = screenshot(source.clone());
        let mut attempts = 0;
        let error = rename_screenshot_with(&mut screenshot, "project", |_, _| {
            attempts += 1;
            Err(io::Error::new(io::ErrorKind::AlreadyExists, "occupied"))
        })
        .unwrap_err();
        assert_eq!(attempts, 10_000);
        assert_eq!(
            error.category(),
            crate::RenameFailureCategory::CollisionExhausted
        );
        assert_eq!(
            error.destination_path(),
            Some(directory.path().join("project (10000).png").as_path())
        );
        assert_eq!(fs::read(source).unwrap(), b"image");
        assert!(matches!(screenshot.state(), ProcessingState::Failed { .. }));
    }

    fn screenshot(path: PathBuf) -> ScreenshotFile {
        ScreenshotFile::new(path, None).expect("create screenshot model")
    }

    #[test]
    fn sanitizes_the_candidate_and_preserves_the_original_extension() {
        let directory = tempdir().expect("create temporary directory");
        let source = directory.path().join("Screenshot.PNG");
        fs::write(&source, b"screenshot bytes").expect("create screenshot fixture");
        let mut screenshot = screenshot(source.clone());

        let final_path = rename_screenshot(&mut screenshot, "project:settings").unwrap();

        assert_eq!(final_path, directory.path().join("project-settings.PNG"));
        assert!(!source.exists());
        assert_eq!(fs::read(final_path).unwrap(), b"screenshot bytes");
        assert_eq!(screenshot.state(), &ProcessingState::Renamed);
    }

    #[test]
    fn preserves_jpg_extension_when_candidate_contains_dots() {
        let directory = tempdir().expect("create temporary directory");
        let source = directory.path().join("Screenshot.jpg");
        fs::write(&source, b"screenshot bytes").expect("create screenshot fixture");
        let mut screenshot = screenshot(source.clone());

        let final_path = rename_screenshot(&mut screenshot, "report.final").unwrap();

        assert_eq!(final_path, directory.path().join("report.final.jpg"));
        assert!(!source.exists());
    }

    #[test]
    fn invalid_only_candidate_keeps_source_and_records_failure() {
        let directory = tempdir().expect("create temporary directory");
        let source = directory.path().join("Screenshot.png");
        fs::write(&source, b"screenshot bytes").expect("create screenshot fixture");
        let mut screenshot = screenshot(source.clone());

        let error = rename_screenshot(&mut screenshot, "<>:?").unwrap_err();

        assert!(matches!(error, RenameError::InvalidCandidateName { .. }));
        assert_eq!(fs::read(source).unwrap(), b"screenshot bytes");
        assert!(matches!(
            screenshot.state(),
            ProcessingState::Failed {
                stage: ProcessingStage::Rename,
                ..
            }
        ));
    }

    #[test]
    fn adds_a_numbered_suffix_for_collisions_without_overwriting() {
        let directory = tempdir().expect("create temporary directory");
        let source = directory.path().join("Screenshot.png");
        let existing = directory.path().join("report.png");
        fs::write(&source, b"new screenshot").expect("create screenshot fixture");
        fs::write(&existing, b"existing file").expect("create existing destination");
        let mut screenshot = screenshot(source.clone());

        let final_path = rename_screenshot(&mut screenshot, "report").unwrap();

        assert_eq!(final_path, directory.path().join("report (2).png"));
        assert_eq!(fs::read(existing).unwrap(), b"existing file");
        assert_eq!(fs::read(final_path).unwrap(), b"new screenshot");
        assert!(!source.exists());
    }

    #[test]
    fn uses_unsuffixed_name_when_available_and_marks_renamed() {
        let directory = tempdir().expect("create temporary directory");
        let source = directory.path().join("Screenshot.jpg");
        fs::write(&source, b"new screenshot").unwrap();
        let mut screenshot = screenshot(source.clone());

        let final_path = rename_screenshot(&mut screenshot, "report").unwrap();

        assert_eq!(final_path, directory.path().join("report.jpg"));
        assert_eq!(fs::read(&final_path).unwrap(), b"new screenshot");
        assert!(!source.exists());
        assert_eq!(screenshot.state(), &ProcessingState::Renamed);
    }

    #[test]
    fn retries_after_destination_appears_between_selection_and_move() {
        let directory = tempdir().expect("create temporary directory");
        let source = directory.path().join("Screenshot.png");
        fs::write(&source, b"new screenshot").unwrap();
        let mut screenshot = screenshot(source.clone());
        let mut first_attempt = true;

        let final_path = rename_screenshot_with(&mut screenshot, "report", |from, to| {
            if first_attempt {
                first_attempt = false;
                fs::write(to, b"other process")?;
            }
            rename_without_replacement(from, to)
        })
        .unwrap();

        assert_eq!(final_path, directory.path().join("report (2).png"));
        assert_eq!(
            fs::read(directory.path().join("report.png")).unwrap(),
            b"other process"
        );
        assert_eq!(fs::read(&final_path).unwrap(), b"new screenshot");
        assert!(!source.exists());
        assert_eq!(screenshot.state(), &ProcessingState::Renamed);
    }

    #[test]
    fn unrelated_move_error_does_not_advance_to_another_suffix() {
        let directory = tempdir().expect("create temporary directory");
        let source = directory.path().join("Screenshot.png");
        fs::write(&source, b"new screenshot").unwrap();
        let mut screenshot = screenshot(source.clone());
        let mut attempts = 0;

        let error = rename_screenshot_with(&mut screenshot, "report", |_, _| {
            attempts += 1;
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "access denied",
            ))
        })
        .unwrap_err();

        assert!(matches!(error, RenameError::Io { .. }));
        assert_eq!(attempts, 1);
        assert_eq!(fs::read(source).unwrap(), b"new screenshot");
        assert!(!directory.path().join("report (2).png").exists());
        assert!(matches!(
            screenshot.state(),
            ProcessingState::Failed {
                stage: ProcessingStage::Rename,
                ..
            }
        ));
    }

    #[cfg(windows)]
    #[test]
    fn accepts_a_parenthesized_candidate_when_the_destination_is_unused() {
        let directory = tempdir().expect("create temporary directory");
        let source = directory.path().join("Screenshot.png");
        fs::write(&source, b"screenshot").expect("create screenshot fixture");
        let mut screenshot = screenshot(source);

        let final_path = rename_screenshot(&mut screenshot, "report (2)").unwrap();

        assert_eq!(final_path, directory.path().join("report (2).png"));
    }

    #[test]
    fn skips_multiple_existing_collision_names() {
        let directory = tempdir().expect("create temporary directory");
        let source = directory.path().join("Screenshot.png");
        fs::write(&source, b"screenshot").expect("create screenshot fixture");
        fs::write(directory.path().join("report.png"), b"first").unwrap();
        fs::write(directory.path().join("report (2).png"), b"second").unwrap();
        let mut screenshot = screenshot(source);

        let final_path = rename_screenshot(&mut screenshot, "report").unwrap();

        assert_eq!(final_path, directory.path().join("report (3).png"));
    }

    #[test]
    fn rejects_reserved_names_with_an_actionable_error_and_keeps_the_source() {
        let directory = tempdir().expect("create temporary directory");
        let source = directory.path().join("Screenshot.png");
        fs::write(&source, b"screenshot").expect("create screenshot fixture");
        let mut screenshot = screenshot(source.clone());

        let error = rename_screenshot(&mut screenshot, "CON").unwrap_err();

        assert!(matches!(error, RenameError::InvalidCandidateName { .. }));
        assert!(error.to_string().contains("reserved Windows device name"));
        assert!(source.exists());
        assert!(matches!(
            screenshot.state(),
            ProcessingState::Failed {
                stage: ProcessingStage::Rename,
                ..
            }
        ));
    }

    #[test]
    fn rejects_reserved_com_and_lpt_superscript_variants() {
        for candidate in ["COM¹", "LPT³"] {
            let directory = tempdir().expect("create temporary directory");
            let source = directory.path().join("Screenshot.png");
            fs::write(&source, b"screenshot").expect("create screenshot fixture");
            let mut screenshot = screenshot(source.clone());

            let error = rename_screenshot(&mut screenshot, candidate).unwrap_err();

            assert!(matches!(error, RenameError::InvalidCandidateName { .. }));
            assert!(source.exists());
        }
    }

    #[test]
    fn rejects_console_stream_device_names() {
        for candidate in ["CONIN$", "CONOUT$"] {
            let directory = tempdir().expect("create temporary directory");
            let source = directory.path().join("Screenshot.png");
            fs::write(&source, b"screenshot").expect("create screenshot fixture");
            let mut screenshot = screenshot(source.clone());

            let error = rename_screenshot(&mut screenshot, candidate).unwrap_err();

            assert!(matches!(error, RenameError::InvalidCandidateName { .. }));
            assert!(source.exists());
        }
    }

    #[test]
    fn copies_without_replacing_when_hard_links_are_unavailable() {
        let directory = tempdir().expect("create temporary directory");
        let source = directory.path().join("Screenshot.png");
        let destination = directory.path().join("report.png");
        fs::write(&source, b"screenshot bytes").expect("create screenshot fixture");

        rename_through_hard_link_or_copy(&source, &destination, |_, _| {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "hard links are unavailable",
            ))
        })
        .unwrap();

        assert!(!source.exists());
        assert_eq!(fs::read(destination).unwrap(), b"screenshot bytes");
    }

    #[cfg(windows)]
    #[test]
    fn case_only_rename_does_not_allocate_a_collision_suffix() {
        let directory = tempdir().expect("create temporary directory");
        let source = directory.path().join("Report.png");
        fs::write(&source, b"screenshot").expect("create screenshot fixture");
        let mut screenshot = screenshot(source.clone());

        let final_path = rename_screenshot(&mut screenshot, "report").unwrap();

        assert_eq!(final_path, source);
        assert!(!directory.path().join("report (2).png").exists());
    }

    #[test]
    fn reports_a_missing_source_without_panicking() {
        let directory = tempdir().expect("create temporary directory");
        let source = directory.path().join("missing.png");
        let mut screenshot = screenshot(source);

        let error = rename_screenshot(&mut screenshot, "report").unwrap_err();

        assert!(matches!(error, RenameError::SourceNotFile { .. }));
        assert!(error.to_string().contains("source file is unavailable"));
        assert!(matches!(
            screenshot.state(),
            ProcessingState::Failed {
                stage: ProcessingStage::Rename,
                ..
            }
        ));
    }
}
