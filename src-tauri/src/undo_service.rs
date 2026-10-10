use std::{
    error::Error,
    fmt, fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

use crate::{
    rename_service::rename_without_replacement, HistoryRepository, HistoryStoreError,
    RenameHistoryEntry, RenameHistoryView, RenameOutcome, SuppressedPaths, UndoStatus,
};

#[derive(Debug)]
pub enum UndoError {
    UnknownHistoryEntry,
    NotUndoable,
    AlreadyUndone,
    OriginalOccupied {
        path: PathBuf,
    },
    RenamedSourceMissing {
        path: PathBuf,
    },
    Io {
        source_path: PathBuf,
        destination_path: PathBuf,
        source: io::Error,
    },
    HistoryRead(HistoryStoreError),
    HistoryWrite {
        actual_path: PathBuf,
        source: HistoryStoreError,
    },
}

impl fmt::Display for UndoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownHistoryEntry => f.write_str("rename history entry was not found"),
            Self::NotUndoable => f.write_str("rename was not successful and cannot be undone"),
            Self::AlreadyUndone => f.write_str("rename was already undone"),
            Self::OriginalOccupied { path } => {
                write!(f, "original path is occupied: {}", path.display())
            }
            Self::RenamedSourceMissing { path } => {
                write!(f, "renamed screenshot is missing: {}", path.display())
            }
            Self::Io {
                source_path,
                destination_path,
                source,
            } => write!(
                f,
                "could not restore {} to {}: {source}",
                source_path.display(),
                destination_path.display()
            ),
            Self::HistoryRead(source) => write!(f, "could not read rename history: {source}"),
            Self::HistoryWrite {
                actual_path,
                source,
            } => write!(
                f,
                "undo outcome could not be saved; screenshot is at {}: {source}",
                actual_path.display()
            ),
        }
    }
}
impl Error for UndoError {}

pub fn undo_rename(entry: &mut RenameHistoryEntry) -> Result<PathBuf, UndoError> {
    undo_rename_with(entry, rename_without_replacement)
}

fn undo_rename_with<F>(entry: &mut RenameHistoryEntry, move_file: F) -> Result<PathBuf, UndoError>
where
    F: FnOnce(&Path, &Path) -> io::Result<()>,
{
    if entry.outcome() != &RenameOutcome::Succeeded {
        return Err(UndoError::NotUndoable);
    }
    if matches!(entry.undo_status(), UndoStatus::Succeeded { .. }) {
        return Err(UndoError::AlreadyUndone);
    }
    let original = entry.original_path().to_path_buf();
    let renamed = entry
        .new_path()
        .ok_or(UndoError::NotUndoable)?
        .to_path_buf();
    if original == renamed {
        return match fs::symlink_metadata(&renamed) {
            Ok(metadata) if metadata.file_type().is_file() => {
                entry
                    .mark_undo_succeeded(SystemTime::now())
                    .expect("successful rename is undoable");
                Ok(original)
            }
            Ok(_) => {
                let error = UndoError::RenamedSourceMissing { path: renamed };
                entry
                    .mark_undo_failed(SystemTime::now(), error.to_string())
                    .expect("successful rename is undoable");
                Err(error)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let error = UndoError::RenamedSourceMissing { path: renamed };
                entry
                    .mark_undo_failed(SystemTime::now(), error.to_string())
                    .expect("successful rename is undoable");
                Err(error)
            }
            Err(error) => {
                let error = UndoError::Io {
                    source_path: renamed.clone(),
                    destination_path: original,
                    source: error,
                };
                entry
                    .mark_undo_failed(SystemTime::now(), error.to_string())
                    .expect("successful rename is undoable");
                Err(error)
            }
        };
    }

    let result = match fs::symlink_metadata(&renamed) {
        Ok(metadata) if metadata.file_type().is_file() => match fs::symlink_metadata(&original) {
            Ok(_) => Err(UndoError::OriginalOccupied {
                path: original.clone(),
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => move_file(&renamed, &original)
                .map_err(|error| match error.kind() {
                    io::ErrorKind::AlreadyExists => UndoError::OriginalOccupied {
                        path: original.clone(),
                    },
                    io::ErrorKind::NotFound => UndoError::RenamedSourceMissing {
                        path: renamed.clone(),
                    },
                    _ => UndoError::Io {
                        source_path: renamed.clone(),
                        destination_path: original.clone(),
                        source: error,
                    },
                }),
            Err(error) => Err(UndoError::Io {
                source_path: renamed.clone(),
                destination_path: original.clone(),
                source: error,
            }),
        },
        Ok(_) => Err(UndoError::RenamedSourceMissing {
            path: renamed.clone(),
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Err(UndoError::RenamedSourceMissing {
                path: renamed.clone(),
            })
        }
        Err(error) => Err(UndoError::Io {
            source_path: renamed.clone(),
            destination_path: original.clone(),
            source: error,
        }),
    };
    match result {
        Ok(()) => {
            entry
                .mark_undo_succeeded(SystemTime::now())
                .expect("successful rename is undoable");
            Ok(original)
        }
        Err(error) => {
            entry
                .mark_undo_failed(SystemTime::now(), error.to_string())
                .expect("successful rename is undoable");
            Err(error)
        }
    }
}

pub fn undo_and_persist<R: HistoryRepository>(
    id: &str,
    repository: &R,
) -> Result<RenameHistoryView, UndoError> {
    let mut entry = repository
        .get(id)
        .map_err(UndoError::HistoryRead)?
        .ok_or(UndoError::UnknownHistoryEntry)?;
    let previous = entry.undo_status().clone();
    let result = undo_rename(&mut entry);
    if entry.undo_status() != &previous {
        let actual_path = match &result {
            Ok(path) => path.clone(),
            Err(_) => entry
                .new_path()
                .unwrap_or(entry.original_path())
                .to_path_buf(),
        };
        repository
            .update_undo_outcome(&entry)
            .map_err(|source| UndoError::HistoryWrite {
                actual_path,
                source,
            })?;
    }
    result?;
    Ok(entry.view())
}

/// Suppress the watcher only when undo moves a file to the original path.
pub fn undo_and_persist_suppressing<R: HistoryRepository>(
    id: &str,
    repository: &R,
    suppressed: &SuppressedPaths,
) -> Result<RenameHistoryView, UndoError> {
    let entry = repository
        .get(id)
        .map_err(UndoError::HistoryRead)?
        .ok_or(UndoError::UnknownHistoryEntry)?;
    let original = entry.original_path().to_path_buf();
    let moving = entry.new_path().is_some_and(|path| path != original);
    if moving {
        suppressed.suppress(&original);
    }
    let result = undo_and_persist(id, repository);
    if moving {
        let restored = match &result {
            Ok(_) => true,
            Err(UndoError::HistoryWrite { actual_path, .. }) => actual_path == &original,
            Err(_) => false,
        };
        if !restored {
            suppressed.clear(&original);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use std::{fs, time::UNIX_EPOCH};

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn destination_created_during_move_is_never_replaced() {
        let directory = tempdir().unwrap();
        let original = directory.path().join("Screenshot.png");
        let renamed = directory.path().join("project.png");
        fs::write(&renamed, b"screenshot").unwrap();
        let mut entry = RenameHistoryEntry::new(original.clone(), UNIX_EPOCH).unwrap();
        entry.mark_rename_succeeded(renamed.clone()).unwrap();
        let error = undo_rename_with(&mut entry, |from, to| {
            fs::write(to, b"racing file")?;
            rename_without_replacement(from, to)
        })
        .unwrap_err();
        assert!(matches!(error, UndoError::OriginalOccupied { .. }));
        assert_eq!(fs::read(original).unwrap(), b"racing file");
        assert_eq!(fs::read(renamed).unwrap(), b"screenshot");
        assert!(matches!(entry.undo_status(), UndoStatus::Failed { .. }));
    }
}
