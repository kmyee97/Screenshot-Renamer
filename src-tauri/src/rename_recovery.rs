use std::{collections::HashMap, fs, io, path::Path};

use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    rename_screenshot_and_persist, HistoryRepository, HistoryStoreError, PersistedRenameError,
    RenameHistoryEntry, RenameHistoryView, RenameOutcome, ScreenshotFile,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RenameFailureCategory {
    InvalidName,
    CollisionExhausted,
    SourceMissing,
    PermissionIo,
    HistoryWrite,
    FileChanged,
    RetryUnavailable,
}

/// Serializable command error and event payload. The ID addresses a retained
/// attempt, never a path supplied by the caller during retry.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameFailure {
    pub id: String,
    pub category: RenameFailureCategory,
    pub source_path: String,
    pub attempted_destination: Option<String>,
    pub actual_path: String,
    pub stage: String,
    pub message: String,
    pub retryable: bool,
    pub filesystem_succeeded: bool,
}

struct Attempt {
    screenshot: ScreenshotFile,
    candidate: String,
    fingerprint: Option<Vec<u8>>,
    pending_history: Option<RenameHistoryEntry>,
    failure: RenameFailure,
}

/// Session recovery queue. History remains durable in SQLite; failed attempts
/// stay available until retried or the app closes. Hold the app's filesystem
/// gate while calling this manager so rename, retry and undo cannot interleave.
#[derive(Default)]
pub struct RenameRecovery {
    attempts: HashMap<String, Attempt>,
}

impl RenameRecovery {
    pub fn failures(&self) -> Vec<RenameFailure> {
        let mut failures: Vec<_> = self
            .attempts
            .values()
            .map(|attempt| attempt.failure.clone())
            .collect();
        failures.sort_by(|a, b| a.id.cmp(&b.id));
        failures
    }

    pub fn rename<R: HistoryRepository>(
        &mut self,
        screenshot: ScreenshotFile,
        candidate: &str,
        repository: Result<&R, HistoryStoreError>,
    ) -> Result<RenameHistoryView, RenameFailure> {
        let id = Uuid::new_v4().to_string();
        let source = screenshot.original_path().to_string_lossy().into_owned();
        let mut attempt = Attempt {
            screenshot,
            candidate: candidate.to_owned(),
            fingerprint: None,
            pending_history: None,
            failure: RenameFailure {
                id: id.clone(),
                category: RenameFailureCategory::RetryUnavailable,
                source_path: source.clone(),
                attempted_destination: None,
                actual_path: source,
                stage: "rename".into(),
                message: String::new(),
                retryable: true,
                filesystem_succeeded: false,
            },
        };
        let result = Self::process(&mut attempt, repository);
        if result.is_err() {
            self.attempts.insert(id, attempt);
        }
        result
    }

    pub fn retry<R: HistoryRepository>(
        &mut self,
        id: &str,
        candidate: Option<&str>,
        repository: Result<&R, HistoryStoreError>,
    ) -> Result<RenameHistoryView, RenameFailure> {
        let Some(attempt) = self.attempts.get_mut(id) else {
            return Err(RenameFailure {
                id: id.into(), category: RenameFailureCategory::RetryUnavailable,
                source_path: String::new(), attempted_destination: None, actual_path: String::new(),
                stage: "retry".into(), message: "This retry is no longer available; refresh the failure list and rename history.".into(),
                retryable: false, filesystem_succeeded: false,
            });
        };
        // A completed move can only retry its original history write.
        if !attempt.failure.filesystem_succeeded {
            if let Some(candidate) = candidate {
                attempt.candidate = candidate.into();
            }
        }
        let result = Self::process(attempt, repository);
        if result.is_ok() {
            self.attempts.remove(id);
        }
        result
    }

    fn process<R: HistoryRepository>(
        attempt: &mut Attempt,
        repository: Result<&R, HistoryStoreError>,
    ) -> Result<RenameHistoryView, RenameFailure> {
        let repository = match repository {
            Ok(repository) => repository,
            Err(error) => {
                // Even a pre-move storage outage must retain the original file
                // identity; a later retry cannot adopt a replacement screenshot.
                if attempt.pending_history.is_none() && attempt.fingerprint.is_none() {
                    attempt.fingerprint = fingerprint(attempt.screenshot.original_path()).ok();
                }
                return Err(Self::fail(
                    attempt, RenameFailureCategory::HistoryWrite, "history",
                    format!("History is unavailable: {error}. Check the app data folder and retry; screenshot is at {}.", attempt.failure.actual_path),
                ));
            }
        };
        if let Some(entry) = attempt.pending_history.clone() {
            if entry.outcome() == &RenameOutcome::Succeeded {
                let destination = entry.new_path().expect("confirmed move has a destination");
                Self::verify_file(attempt, destination)?;
            }
            // An ambiguous append error may follow a committed insert. Look it
            // up by its stable ID before retrying; never create another success.
            let existing = repository.get(entry.id()).map_err(|error| {
                Self::fail(
                    attempt,
                    RenameFailureCategory::HistoryWrite,
                    "history",
                    error.to_string(),
                )
            })?;
            match existing {
                Some(existing) if existing != entry => return Err(Self::fail(attempt, RenameFailureCategory::HistoryWrite, "history", "History has a conflicting record; check the history database before retrying.".into())),
                Some(_) => {},
                None => repository.append(&entry).map_err(|error| Self::fail(attempt, RenameFailureCategory::HistoryWrite, "history", format!("Could not save rename history: {error}. Screenshot is at {}; retry to save this outcome.", attempt.failure.actual_path)))?,
            }
            attempt.pending_history = None;
            if entry.outcome() == &RenameOutcome::Succeeded {
                return Ok(entry.view());
            }
        }

        let source = attempt.screenshot.original_path().to_path_buf();
        if attempt.fingerprint.is_some() {
            Self::verify_file(attempt, &source)?;
        } else {
            match fingerprint(&source) {
                Ok(hash) => attempt.fingerprint = Some(hash),
                // Let the rename service create the missing-source failure record.
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(Self::fail(
                        attempt,
                        RenameFailureCategory::PermissionIo,
                        "rename",
                        format!(
                        "Cannot read screenshot at {}: {error}. Check file permissions and retry.",
                        source.display()
                    ),
                    ))
                }
            }
        }
        match rename_screenshot_and_persist(&mut attempt.screenshot, &attempt.candidate, repository)
        {
            Ok(view) => Ok(view),
            Err(PersistedRenameError::Rename { error, .. }) => {
                attempt.failure.attempted_destination = error.destination_path().map(display_path);
                let category = error.category();
                Err(Self::fail(attempt, category, "rename", error.to_string()))
            }
            Err(PersistedRenameError::Storage {
                actual_path,
                pending_history,
                error,
            }) => {
                attempt.failure.filesystem_succeeded =
                    pending_history.outcome() == &RenameOutcome::Succeeded;
                attempt.failure.actual_path = display_path(&actual_path);
                attempt.failure.attempted_destination =
                    pending_history.new_path().map(display_path);
                let filesystem_message = match pending_history.outcome() {
                    RenameOutcome::Failed { message } => {
                        format!("The screenshot was not renamed: {message}.")
                    }
                    _ => "The screenshot was renamed.".into(),
                };
                attempt.pending_history = Some(pending_history);
                Err(Self::fail(attempt, RenameFailureCategory::HistoryWrite, "history", format!("{filesystem_message} History could not be saved: {error}. Screenshot is at {}; retry to save the recorded outcome.", attempt.failure.actual_path)))
            }
        }
    }

    fn verify_file(attempt: &mut Attempt, path: &Path) -> Result<(), RenameFailure> {
        match fingerprint(path) {
            Ok(hash) if attempt.fingerprint.as_ref() == Some(&hash) => Ok(()),
            Ok(_) => Err(Self::fail(attempt, RenameFailureCategory::FileChanged, "reconcile", format!("The file at {} has changed. Restore the original screenshot before retrying; no files were moved.", path.display()))),
            Err(error) => Err(Self::fail(attempt, if error.kind() == io::ErrorKind::NotFound { RenameFailureCategory::SourceMissing } else { RenameFailureCategory::PermissionIo }, "reconcile", format!("Cannot verify screenshot at {}: {error}. Restore access to the confirmed file and retry; no files were moved.", path.display()))),
        }
    }

    fn fail(
        attempt: &mut Attempt,
        category: RenameFailureCategory,
        stage: &str,
        message: String,
    ) -> RenameFailure {
        attempt.failure.category = category;
        attempt.failure.stage = stage.into();
        attempt.failure.message = message;
        attempt.failure.clone()
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn fingerprint(path: &Path) -> io::Result<Vec<u8>> {
    if !fs::symlink_metadata(path)?.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "screenshot is not a regular file",
        ));
    }
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    io::copy(&mut file, &mut hash)?;
    Ok(hash.finalize().to_vec())
}
