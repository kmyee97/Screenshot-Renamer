use crate::{
    HistoryStore, HistoryStoreError, RenameHistoryEntry, RenameHistoryView, RenameOutcome,
    UndoError, UndoStatus,
};
use serde::Serialize;
use std::{fs, io};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    #[serde(flatten)]
    pub history: RenameHistoryView,
    pub can_undo: bool,
    pub undo_reason: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentHistory {
    pub entries: Vec<HistoryItem>,
    pub undo_last: Option<RenameHistoryView>,
}

/// Availability is advisory: undo itself rechecks and uses a no-replacement move.
pub fn undo_unavailable_reason(entry: &RenameHistoryEntry) -> Option<String> {
    if entry.outcome() != &RenameOutcome::Succeeded {
        return Some("Rename did not succeed.".into());
    }
    if matches!(entry.undo_status(), UndoStatus::Succeeded { .. }) {
        return Some("Rename was already undone.".into());
    }
    let Some(renamed) = entry.new_path() else {
        return Some("Renamed path is unavailable.".into());
    };
    match fs::symlink_metadata(renamed) {
        Ok(metadata) if metadata.file_type().is_file() => {}
        Ok(_) => return Some("Renamed screenshot is missing or is no longer a file.".into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Some("Renamed screenshot is missing.".into())
        }
        Err(error) => return Some(format!("Cannot access renamed screenshot: {error}")),
    }
    if renamed != entry.original_path() {
        match fs::symlink_metadata(entry.original_path()) {
            Ok(_) => return Some("Original path is occupied.".into()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Some(format!("Cannot access original path: {error}")),
        }
    }
    None
}

pub fn recent_history(
    store: &HistoryStore,
    limit: usize,
) -> Result<RecentHistory, HistoryStoreError> {
    let entries = store
        .list_recent_successful(limit)?
        .into_iter()
        .map(|entry| {
            let undo_reason = undo_unavailable_reason(&entry);
            HistoryItem {
                history: entry.view(),
                can_undo: undo_reason.is_none(),
                undo_reason,
            }
        })
        .collect();
    let undo_last = store
        .find_latest_successful(|entry| undo_unavailable_reason(entry).is_none())?
        .map(|entry| entry.view());
    Ok(RecentHistory { entries, undo_last })
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoFailure {
    pub category: &'static str,
    pub message: String,
    pub actual_path: Option<String>,
}
impl UndoFailure {
    pub fn unavailable(message: String) -> Self {
        Self {
            category: "unavailable",
            message,
            actual_path: None,
        }
    }
}
impl From<UndoError> for UndoFailure {
    fn from(error: UndoError) -> Self {
        let category = match &error {
            UndoError::UnknownHistoryEntry => "unknownId",
            UndoError::NotUndoable => "notUndoable",
            UndoError::AlreadyUndone => "alreadyUndone",
            UndoError::OriginalOccupied { .. } => "originalOccupied",
            UndoError::RenamedSourceMissing { .. } => "sourceMissing",
            UndoError::Io { .. } => "io",
            UndoError::HistoryRead(_) => "historyRead",
            UndoError::HistoryWrite { .. } => "historyWrite",
        };
        let actual_path = match &error {
            UndoError::HistoryWrite { actual_path, .. } => {
                Some(actual_path.to_string_lossy().into_owned())
            }
            _ => None,
        };
        Self {
            category,
            message: error.to_string(),
            actual_path,
        }
    }
}
