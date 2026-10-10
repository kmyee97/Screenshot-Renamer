use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

use crate::{
    rename_screenshot_recorded, HistoryRepository, HistoryStoreError, RenameError,
    RenameHistoryView, ScreenshotFile,
};

#[derive(Debug)]
pub enum PersistedRenameError {
    Rename {
        actual_path: PathBuf,
        error: RenameError,
    },
    Storage {
        actual_path: PathBuf,
        error: HistoryStoreError,
    },
}

impl PersistedRenameError {
    pub fn actual_path(&self) -> &Path {
        match self {
            Self::Rename { actual_path, .. } | Self::Storage { actual_path, .. } => actual_path,
        }
    }
}

impl fmt::Display for PersistedRenameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rename { actual_path, error } => {
                write!(f, "rename failed at {}: {error}", actual_path.display())
            }
            Self::Storage { actual_path, error } => write!(
                f,
                "history storage failed; screenshot is at {}: {error}",
                actual_path.display()
            ),
        }
    }
}
impl Error for PersistedRenameError {}

/// Keeps the filesystem result independent of history storage. A failed write
/// returns the real path and never removes or rolls back the screenshot.
pub fn rename_screenshot_and_persist<R: HistoryRepository>(
    screenshot: &mut ScreenshotFile,
    candidate_stem: &str,
    repository: &R,
) -> Result<RenameHistoryView, PersistedRenameError> {
    let recorded = rename_screenshot_recorded(screenshot, candidate_stem);
    let actual_path = recorded
        .result
        .as_ref()
        .cloned()
        .unwrap_or_else(|_| screenshot.original_path().to_path_buf());
    repository
        .append(&recorded.history)
        .map_err(|error| PersistedRenameError::Storage {
            actual_path: actual_path.clone(),
            error,
        })?;
    recorded
        .result
        .map_err(|error| PersistedRenameError::Rename { actual_path, error })?;
    Ok(recorded.history.view())
}
