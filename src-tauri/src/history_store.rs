use std::{error::Error, fmt, fs, path::Path, sync::Mutex};

use rusqlite::{params, Connection, OptionalExtension};

use crate::{RenameHistoryEntry, RenameOutcome};

#[derive(Debug)]
pub struct HistoryStoreError(pub String);

impl fmt::Display for HistoryStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for HistoryStoreError {}

pub trait HistoryRepository {
    fn append(&self, entry: &RenameHistoryEntry) -> Result<(), HistoryStoreError>;
    fn update_undo_outcome(&self, entry: &RenameHistoryEntry) -> Result<(), HistoryStoreError>;
    fn get(&self, id: &str) -> Result<Option<RenameHistoryEntry>, HistoryStoreError>;
    fn list_recent(&self, limit: usize) -> Result<Vec<RenameHistoryEntry>, HistoryStoreError>;
}

pub struct HistoryStore {
    connection: Mutex<Connection>,
}

impl HistoryStore {
    pub fn open(path: &Path) -> Result<Self, HistoryStoreError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| HistoryStoreError(format!("create history directory: {error}")))?;
        }
        let mut connection = Connection::open(path).map_err(sql_error)?;
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(sql_error)?;
        if version > 1 {
            return Err(HistoryStoreError(format!(
                "unsupported history schema version {version}"
            )));
        }
        if version == 0 {
            let transaction = connection.transaction().map_err(sql_error)?;
            transaction
                .execute_batch(
                    "CREATE TABLE IF NOT EXISTS entries (
                id TEXT PRIMARY KEY NOT NULL,
                attempted_at_ms INTEGER NOT NULL,
                payload TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS entries_recent ON entries (attempted_at_ms DESC, id DESC);
            PRAGMA user_version = 1;",
                )
                .map_err(sql_error)?;
            transaction.commit().map_err(sql_error)?;
        }
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>, HistoryStoreError> {
        self.connection
            .lock()
            .map_err(|_| HistoryStoreError("history connection lock was poisoned".into()))
    }

    pub fn append(&self, entry: &RenameHistoryEntry) -> Result<(), HistoryStoreError> {
        <Self as HistoryRepository>::append(self, entry)
    }
    pub fn update_undo_outcome(&self, entry: &RenameHistoryEntry) -> Result<(), HistoryStoreError> {
        <Self as HistoryRepository>::update_undo_outcome(self, entry)
    }
    pub fn get(&self, id: &str) -> Result<Option<RenameHistoryEntry>, HistoryStoreError> {
        <Self as HistoryRepository>::get(self, id)
    }
    pub fn list_recent(&self, limit: usize) -> Result<Vec<RenameHistoryEntry>, HistoryStoreError> {
        <Self as HistoryRepository>::list_recent(self, limit)
    }
}

impl HistoryRepository for HistoryStore {
    fn append(&self, entry: &RenameHistoryEntry) -> Result<(), HistoryStoreError> {
        if entry.outcome() == &RenameOutcome::Pending {
            return Err(HistoryStoreError(
                "cannot persist an unfinished rename".into(),
            ));
        }
        let payload = serde_json::to_string(entry).map_err(json_error)?;
        self.connection()?
            .execute(
                "INSERT INTO entries (id, attempted_at_ms, payload) VALUES (?1, ?2, ?3)",
                params![entry.id(), entry.attempted_at_ms(), payload],
            )
            .map_err(sql_error)?;
        Ok(())
    }

    fn update_undo_outcome(&self, entry: &RenameHistoryEntry) -> Result<(), HistoryStoreError> {
        if entry.outcome() != &RenameOutcome::Succeeded {
            return Err(HistoryStoreError(
                "cannot update undo for an unsuccessful rename".into(),
            ));
        }
        let payload = serde_json::to_string(entry).map_err(json_error)?;
        let updated = self
            .connection()?
            .execute(
                "UPDATE entries SET payload = ?1 WHERE id = ?2",
                params![payload, entry.id()],
            )
            .map_err(sql_error)?;
        if updated == 0 {
            return Err(HistoryStoreError(format!(
                "history entry {} was not found",
                entry.id()
            )));
        }
        Ok(())
    }

    fn get(&self, id: &str) -> Result<Option<RenameHistoryEntry>, HistoryStoreError> {
        let connection = self.connection()?;
        let payload: Option<String> = connection
            .query_row("SELECT payload FROM entries WHERE id = ?1", [id], |row| {
                row.get(0)
            })
            .optional()
            .map_err(sql_error)?;
        payload
            .map(|text| serde_json::from_str(&text).map_err(json_error))
            .transpose()
    }

    fn list_recent(&self, limit: usize) -> Result<Vec<RenameHistoryEntry>, HistoryStoreError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let connection = self.connection()?;
        let mut statement = connection
            .prepare("SELECT payload FROM entries ORDER BY attempted_at_ms DESC, id DESC")
            .map_err(sql_error)?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(sql_error)?;
        let mut entries = Vec::new();
        for row in rows {
            let payload = row.map_err(sql_error)?;
            if let Ok(entry) = serde_json::from_str::<RenameHistoryEntry>(&payload) {
                entries.push(entry);
                if entries.len() >= limit.min(100) {
                    break;
                }
            }
        }
        Ok(entries)
    }
}

fn sql_error(error: rusqlite::Error) -> HistoryStoreError {
    HistoryStoreError(format!("history database: {error}"))
}
fn json_error(error: serde_json::Error) -> HistoryStoreError {
    HistoryStoreError(format!("history record: {error}"))
}
