use std::{
    fmt,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", content = "details", rename_all = "camelCase")]
pub enum RenameOutcome {
    Pending,
    Succeeded,
    Failed { message: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", content = "details", rename_all = "camelCase")]
pub enum UndoStatus {
    NotAttempted,
    Succeeded { at_ms: i64 },
    Failed { at_ms: i64, reason: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RenameHistoryEntry {
    id: String,
    original_path: PathBuf,
    new_path: Option<PathBuf>,
    attempted_at_ms: i64,
    outcome: RenameOutcome,
    undo_status: UndoStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameHistoryView {
    pub id: String,
    pub original_path: String,
    pub original_name: String,
    pub new_path: Option<String>,
    pub new_name: Option<String>,
    pub attempted_at_ms: i64,
    pub outcome: RenameOutcome,
    pub undo_status: UndoStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HistoryError {
    MissingFilename,
    PathResolution(String),
    TimeBeforeEpoch,
    InvalidTransition(&'static str),
}

impl fmt::Display for HistoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingFilename => write!(f, "history path has no filename"),
            Self::PathResolution(reason) => {
                write!(f, "history path cannot be made absolute: {reason}")
            }
            Self::TimeBeforeEpoch => write!(f, "history timestamp is before the Unix epoch"),
            Self::InvalidTransition(reason) => write!(f, "invalid history transition: {reason}"),
        }
    }
}

impl std::error::Error for HistoryError {}

impl RenameHistoryEntry {
    pub fn new(original_path: PathBuf, attempted_at: SystemTime) -> Result<Self, HistoryError> {
        filename(&original_path)?;
        let original_path = absolute_path(original_path)?;
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            original_path,
            new_path: None,
            attempted_at_ms: epoch_ms(attempted_at)?,
            outcome: RenameOutcome::Pending,
            undo_status: UndoStatus::NotAttempted,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn original_path(&self) -> &Path {
        &self.original_path
    }
    pub fn new_path(&self) -> Option<&Path> {
        self.new_path.as_deref()
    }
    pub fn attempted_at_ms(&self) -> i64 {
        self.attempted_at_ms
    }
    pub fn outcome(&self) -> &RenameOutcome {
        &self.outcome
    }
    pub fn undo_status(&self) -> &UndoStatus {
        &self.undo_status
    }

    pub fn mark_rename_succeeded(&mut self, destination: PathBuf) -> Result<(), HistoryError> {
        self.require_pending()?;
        filename(&destination)?;
        self.new_path = Some(absolute_path(destination)?);
        self.outcome = RenameOutcome::Succeeded;
        Ok(())
    }

    pub fn mark_rename_failed(
        &mut self,
        destination: Option<PathBuf>,
        message: String,
    ) -> Result<(), HistoryError> {
        self.require_pending()?;
        self.new_path = destination
            .map(|path| {
                filename(&path)?;
                absolute_path(path)
            })
            .transpose()?;
        self.outcome = RenameOutcome::Failed { message };
        Ok(())
    }

    pub fn mark_undo_succeeded(&mut self, at: SystemTime) -> Result<(), HistoryError> {
        self.require_undoable()?;
        self.undo_status = UndoStatus::Succeeded {
            at_ms: epoch_ms(at)?,
        };
        Ok(())
    }

    pub fn mark_undo_failed(&mut self, at: SystemTime, reason: String) -> Result<(), HistoryError> {
        self.require_undoable()?;
        self.undo_status = UndoStatus::Failed {
            at_ms: epoch_ms(at)?,
            reason,
        };
        Ok(())
    }

    pub fn view(&self) -> RenameHistoryView {
        RenameHistoryView {
            id: self.id.clone(),
            original_path: self.original_path.to_string_lossy().into_owned(),
            original_name: filename(&self.original_path)
                .expect("validated original path")
                .to_string_lossy()
                .into_owned(),
            new_path: self
                .new_path
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            new_name: self.new_path.as_ref().map(|path| {
                filename(path)
                    .expect("validated destination")
                    .to_string_lossy()
                    .into_owned()
            }),
            attempted_at_ms: self.attempted_at_ms,
            outcome: self.outcome.clone(),
            undo_status: self.undo_status.clone(),
        }
    }

    fn require_pending(&self) -> Result<(), HistoryError> {
        if self.outcome == RenameOutcome::Pending {
            Ok(())
        } else {
            Err(HistoryError::InvalidTransition(
                "rename outcome already recorded",
            ))
        }
    }

    fn require_undoable(&self) -> Result<(), HistoryError> {
        if self.outcome != RenameOutcome::Succeeded {
            return Err(HistoryError::InvalidTransition("rename did not succeed"));
        }
        if matches!(self.undo_status, UndoStatus::Succeeded { .. }) {
            return Err(HistoryError::InvalidTransition("rename already undone"));
        }
        Ok(())
    }
}

fn filename(path: &Path) -> Result<&std::ffi::OsStr, HistoryError> {
    path.file_name().ok_or(HistoryError::MissingFilename)
}

fn absolute_path(path: PathBuf) -> Result<PathBuf, HistoryError> {
    std::path::absolute(path).map_err(|error| HistoryError::PathResolution(error.to_string()))
}

fn epoch_ms(at: SystemTime) -> Result<i64, HistoryError> {
    let millis = at
        .duration_since(UNIX_EPOCH)
        .map_err(|_| HistoryError::TimeBeforeEpoch)?
        .as_millis();
    i64::try_from(millis).map_err(|_| HistoryError::InvalidTransition("timestamp is out of range"))
}
