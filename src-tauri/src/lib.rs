use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use tauri::{AppHandle, Emitter, Manager, State};

pub mod filename_sanitizer;
pub mod folder_watcher;
pub mod history_pipeline;
pub mod history_store;
pub mod rename_history;
pub mod rename_service;
pub mod screenshot_file;
pub mod undo_service;

pub use filename_sanitizer::{sanitize_stem, SanitizeError};
pub use folder_watcher::{
    wait_for_file_readiness, FolderWatcher, FolderWatcherError, FolderWatcherProcessor,
    ReadinessError, SuppressedPaths,
};
pub use history_pipeline::{rename_screenshot_and_persist, PersistedRenameError};
pub use history_store::{HistoryRepository, HistoryStore, HistoryStoreError};
pub use rename_history::{
    HistoryError, RenameHistoryEntry, RenameHistoryView, RenameOutcome, UndoStatus,
};
pub use rename_service::{
    rename_screenshot, rename_screenshot_recorded, RecordedRename, RenameError,
};
pub use screenshot_file::{
    is_supported_image_file, ProcessingStage, ProcessingState, ScreenshotFile, ScreenshotFileError,
};
pub use undo_service::{undo_and_persist, undo_rename, UndoError};

#[derive(Default)]
pub struct WatcherState(Mutex<Option<FolderWatcher>>);

pub struct SuppressionState(Arc<SuppressedPaths>);

impl Default for SuppressionState {
    fn default() -> Self {
        Self(Arc::new(SuppressedPaths::default()))
    }
}

pub struct HistoryState {
    database_path: Result<PathBuf, String>,
    /// Snapshot loaded during app startup for future UI integration.
    startup_recent: Vec<RenameHistoryView>,
    undo_gate: Arc<Mutex<()>>,
}

impl HistoryState {
    fn from_app(app: &AppHandle) -> Self {
        let database_path = app
            .path()
            .app_data_dir()
            .map(|directory| directory.join("rename-history.sqlite"))
            .map_err(|error| error.to_string());
        let startup_recent = database_path
            .as_ref()
            .ok()
            .and_then(|path| HistoryStore::open(path).ok())
            .and_then(|store| store.list_recent(50).ok())
            .unwrap_or_default()
            .into_iter()
            .map(|entry| entry.view())
            .collect();
        Self {
            database_path,
            startup_recent,
            undo_gate: Arc::new(Mutex::new(())),
        }
    }

    fn path(&self) -> Result<PathBuf, String> {
        self.database_path.clone()
    }
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn start_watching(
    app: AppHandle,
    state: State<'_, WatcherState>,
    suppression: State<'_, SuppressionState>,
    directory: String,
) -> Result<(), String> {
    let watcher = FolderWatcher::start_with_suppression(
        PathBuf::from(directory),
        move |file| {
            let path = file.original_path().to_string_lossy().into_owned();
            let _ = app.emit("screenshot-detected", path);
        },
        Arc::clone(&suppression.0),
    )
    .map_err(|error| error.to_string())?;

    let mut active_watcher = state
        .0
        .lock()
        .map_err(|_| "watcher state is unavailable".to_owned())?;
    if let Some(previous_watcher) = active_watcher.as_mut() {
        previous_watcher.stop().map_err(|error| error.to_string())?;
    }
    *active_watcher = Some(watcher);

    Ok(())
}

#[tauri::command]
fn stop_watching(state: State<'_, WatcherState>) -> Result<(), String> {
    let mut active_watcher = state
        .0
        .lock()
        .map_err(|_| "watcher state is unavailable".to_owned())?;
    if let Some(mut watcher) = active_watcher.take() {
        watcher.stop().map_err(|error| error.to_string())?;
    }

    Ok(())
}

#[tauri::command]
async fn list_recent_history(
    state: State<'_, HistoryState>,
    limit: Option<usize>,
) -> Result<Vec<RenameHistoryView>, String> {
    let path = state.path()?;
    tauri::async_runtime::spawn_blocking(move || {
        let store = HistoryStore::open(&path).map_err(|error| error.to_string())?;
        store
            .list_recent(limit.unwrap_or(50).min(100))
            .map(|entries| entries.into_iter().map(|entry| entry.view()).collect())
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
fn startup_history(state: State<'_, HistoryState>) -> Vec<RenameHistoryView> {
    state.startup_recent.clone()
}

#[tauri::command]
async fn rename_and_record(
    state: State<'_, HistoryState>,
    original_path: String,
    candidate_stem: String,
) -> Result<RenameHistoryView, String> {
    let path = state.path()?;
    tauri::async_runtime::spawn_blocking(move || {
        let store = HistoryStore::open(&path).map_err(|error| error.to_string())?;
        let mut screenshot = ScreenshotFile::new(PathBuf::from(original_path), None)
            .map_err(|error| format!("invalid screenshot path: {error:?}"))?;
        rename_screenshot_and_persist(&mut screenshot, &candidate_stem, &store)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn undo_history(
    state: State<'_, HistoryState>,
    suppression: State<'_, SuppressionState>,
    id: String,
) -> Result<RenameHistoryView, String> {
    let path = state.path()?;
    let gate = Arc::clone(&state.undo_gate);
    let suppressed = Arc::clone(&suppression.0);
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = gate
            .lock()
            .map_err(|_| "undo lock is unavailable".to_owned())?;
        let store = HistoryStore::open(&path).map_err(|error| error.to_string())?;
        let entry = store
            .get(&id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "rename history entry was not found".to_owned())?;
        let original = entry.original_path().to_path_buf();
        suppressed.suppress(&original);
        let result = undo_and_persist(&id, &store);
        let restored = match &result {
            Ok(_) => true,
            Err(UndoError::HistoryWrite { actual_path, .. }) => actual_path == &original,
            Err(_) => false,
        };
        if !restored {
            suppressed.clear(&original);
        }
        result.map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::greet;

    #[test]
    fn greet_formats_the_supplied_name() {
        assert_eq!(greet("Ari"), "Hello, Ari! You've been greeted from Rust!");
    }

    #[test]
    fn temporary_filesystem_fixture_is_written_and_read_in_isolation() {
        let temp_dir = tempdir().expect("create temporary fixture directory");
        let fixture_path = temp_dir.path().join("fixture.txt");

        fs::write(&fixture_path, "temporary fixture content").expect("write fixture");

        assert_eq!(
            fs::read_to_string(fixture_path).expect("read fixture"),
            "temporary fixture content"
        );
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(WatcherState::default())
        .manage(SuppressionState::default())
        .setup(|app| {
            let state = HistoryState::from_app(&app.handle());
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            start_watching,
            stop_watching,
            startup_history,
            list_recent_history,
            rename_and_record,
            undo_history
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
