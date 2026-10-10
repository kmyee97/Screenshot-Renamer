use std::{path::PathBuf, sync::Mutex};

use tauri::{AppHandle, Emitter, State};

pub mod folder_watcher;
pub mod rename_service;
pub mod screenshot_file;

pub use folder_watcher::{
    wait_for_file_readiness, FolderWatcher, FolderWatcherError, FolderWatcherProcessor,
    ReadinessError,
};
pub use rename_service::{rename_screenshot, RenameError};
pub use screenshot_file::{
    is_supported_image_file, ProcessingStage, ProcessingState, ScreenshotFile, ScreenshotFileError,
};

#[derive(Default)]
pub struct WatcherState(Mutex<Option<FolderWatcher>>);

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn start_watching(
    app: AppHandle,
    state: State<'_, WatcherState>,
    directory: String,
) -> Result<(), String> {
    let watcher = FolderWatcher::start(PathBuf::from(directory), move |file| {
        let path = file.original_path().to_string_lossy().into_owned();
        let _ = app.emit("screenshot-detected", path);
    })
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
        .invoke_handler(tauri::generate_handler![
            greet,
            start_watching,
            stop_watching
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
