use std::{
    error::Error,
    ffi::{OsStr, OsString},
    fmt, fs, io,
    path::{Path, PathBuf},
};

use crate::{ProcessingStage, ProcessingState, ScreenshotFile};

/// Renames a screenshot to a sanitized filename stem in the same directory.
///
/// The source extension is retained. If the requested name already exists,
/// numbered suffixes such as `name (2).png` are tried until an unused path is
/// found. Failures are returned to the caller and recorded on the screenshot
/// so a watcher can report the problem and continue processing other files.
pub fn rename_screenshot(
    screenshot: &mut ScreenshotFile,
    candidate_stem: &str,
) -> Result<PathBuf, RenameError> {
    screenshot.transition_to(ProcessingState::Renaming);

    let result = rename_file(screenshot.original_path(), candidate_stem);
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

fn rename_file(source: &Path, candidate_stem: &str) -> Result<PathBuf, RenameError> {
    let stem = sanitize_candidate_stem(candidate_stem)?;
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
    let extension = source.extension();
    let mut suffix = None;

    loop {
        let name = destination_name(&stem, suffix, extension);
        let destination = parent.join(name);
        if destination == source {
            return Ok(source.to_path_buf());
        }
        if fs::symlink_metadata(&destination).is_ok() {
            suffix = Some(next_collision_suffix(suffix, &destination)?);
            continue;
        }

        match rename_without_replacement(source, &destination) {
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

fn next_collision_suffix(current: Option<u32>, destination: &Path) -> Result<u32, RenameError> {
    match current {
        Some(current) => current
            .checked_add(1)
            .ok_or_else(|| RenameError::CollisionLimit {
                destination_path: destination.to_path_buf(),
            }),
        None => Ok(2),
    }
}

fn sanitize_candidate_stem(candidate: &str) -> Result<String, RenameError> {
    let mut sanitized = candidate
        .trim()
        .chars()
        .map(|character| {
            if character.is_control() || "<>:\"/\\|?*".contains(character) {
                '-'
            } else {
                character
            }
        })
        .collect::<String>();
    sanitized = sanitized
        .trim_end_matches(|character| character == ' ' || character == '.')
        .to_owned();

    if sanitized.is_empty() {
        return Err(RenameError::InvalidCandidateName {
            candidate: candidate.to_owned(),
            reason: "the name is empty after removing unsupported characters",
        });
    }

    if is_reserved_windows_name(&sanitized) {
        return Err(RenameError::InvalidCandidateName {
            candidate: candidate.to_owned(),
            reason: "the name is a reserved Windows device name",
        });
    }

    Ok(sanitized)
}

fn is_reserved_windows_name(stem: &str) -> bool {
    let first_part = stem
        .split('.')
        .next()
        .unwrap_or(stem)
        .trim_end_matches(|character| character == ' ' || character == '.');
    let uppercase = first_part.to_ascii_uppercase();

    matches!(uppercase.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            uppercase.strip_prefix(prefix).is_some_and(|digit| {
                matches!(digit, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
        })
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
fn rename_without_replacement(source: &Path, destination: &Path) -> io::Result<()> {
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
    // MoveFileW is a same-volume atomic move and fails if the destination exists.
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

#[cfg(not(windows))]
fn rename_without_replacement(source: &Path, destination: &Path) -> io::Result<()> {
    rename_through_hard_link(source, destination)
}

#[cfg(windows)]
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
fn rename_through_hard_link(source: &Path, destination: &Path) -> io::Result<()> {
    // A hard link claims the destination atomically without replacing an existing file.
    // The source and destination share a directory, so they are on the same filesystem.
    fs::hard_link(source, destination)?;
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

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use tempfile::tempdir;

    use crate::{ProcessingStage, ProcessingState, ScreenshotFile};

    use super::{rename_screenshot, RenameError};

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
