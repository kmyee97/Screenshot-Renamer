use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use tauri::{AppHandle, Emitter, Manager, State};

pub mod ai_settings;
pub mod credentials;
pub mod desktop;
pub mod filename_sanitizer;
pub mod folder_watcher;
pub mod history_pipeline;
pub mod history_store;
pub mod history_ui;
pub mod naming_engine;
pub mod naming_templates;
pub mod notifications;
pub mod ocr_service;
pub mod ocr_text_cleaner;
pub mod processing_mode;
pub mod rename_history;
pub mod rename_recovery;
pub mod rename_service;
pub mod screenshot_file;
pub mod screenshot_metadata;
pub mod settings;
pub mod settings_controller;
pub mod startup;
pub mod tray;
pub mod undo_service;
pub mod vision_provider;
pub mod vision_providers;

#[tauri::command]
async fn has_ai_credential() -> Result<bool, String> {
    use credentials::CredentialStore;
    tauri::async_runtime::spawn_blocking(|| {
        credentials::OsCredentials::default()
            .exists()
            .map_err(|_| "Credential storage is unavailable.".to_string())
    })
    .await
    .map_err(|_| "Credential storage is unavailable.".to_string())?
}
#[tauri::command]
async fn set_ai_credential(credential: String) -> Result<(), String> {
    use credentials::CredentialStore;
    tauri::async_runtime::spawn_blocking(move || {
        credentials::Secret::new(credential)
            .and_then(|secret| credentials::OsCredentials::default().save(secret))
            .map_err(|_| "Could not save the API key in Windows Credential Manager.".into())
    })
    .await
    .map_err(|_| "Credential storage is unavailable.".to_string())?
}
#[tauri::command]
async fn delete_ai_credential() -> Result<(), String> {
    use credentials::CredentialStore;
    tauri::async_runtime::spawn_blocking(|| {
        credentials::OsCredentials::default()
            .remove()
            .map_err(|_| "Could not remove the API key from Windows Credential Manager.".into())
    })
    .await
    .map_err(|_| "Credential storage is unavailable.".to_string())?
}
pub mod window_context;

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
pub use rename_recovery::{RenameFailure, RenameFailureCategory, RenameRecovery};
pub use rename_service::{
    rename_screenshot, rename_screenshot_recorded, RecordedRename, RenameError,
};
pub use screenshot_file::{
    is_supported_image_file, ProcessingStage, ProcessingState, ScreenshotFile, ScreenshotFileError,
};
pub use undo_service::{undo_and_persist, undo_and_persist_suppressing, undo_rename, UndoError};

pub struct SettingsState {
    controller: Arc<Mutex<settings_controller::SettingsController<FolderWatcher>>>,
}
#[derive(Default)]
struct WatcherPublication {
    gate: Mutex<()>,
    revision: Arc<std::sync::atomic::AtomicU64>,
}
impl SettingsState {
    fn from_app(app: &AppHandle) -> Self {
        let store = app
            .path()
            .app_config_dir()
            .map(|directory| settings::SettingsStore::new(directory.join("settings.json")))
            .map_err(|error| error.to_string());
        Self {
            controller: Arc::new(Mutex::new(settings_controller::SettingsController::new(
                store,
            ))),
        }
    }
}
#[tauri::command]
async fn get_settings(
    app: AppHandle,
    state: State<'_, SettingsState>,
) -> Result<settings::SettingsSnapshot, String> {
    let controller = state.controller.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let publication = app.state::<WatcherPublication>();
        let _publication_guard = publication
            .gate
            .lock()
            .map_err(|_| "Settings are unavailable.")?;
        controller
            .lock()
            .map(|controller| {
                let mut snapshot = controller.snapshot();
                snapshot.revision = publication
                    .revision
                    .load(std::sync::atomic::Ordering::SeqCst);
                snapshot
            })
            .map_err(|_| "Settings are unavailable.".into())
    })
    .await
    .map_err(|error| error.to_string())?
}
#[tauri::command]
async fn update_settings(
    app: AppHandle,
    state: State<'_, SettingsState>,
    suppression: State<'_, SuppressionState>,
    settings: settings::ApplicationSettings,
) -> Result<settings::SettingsSnapshot, String> {
    let shared_controller = state.controller.clone();
    let suppression = suppression.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut controller = shared_controller
            .lock()
            .map_err(|_| "Settings are unavailable.")?;
        let rules = controller.processing_settings();
        let result = controller.apply(settings, |folder, enabled, error| {
            prepare_watcher(&app, folder, enabled, error, suppression, rules)
        });
        drop(controller);
        let snapshot = publish_watcher_state(&app)?;
        result.map(|_| snapshot)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn set_auto_rename(
    app: AppHandle,
    state: State<'_, SettingsState>,
    suppression: State<'_, SuppressionState>,
    enabled: bool,
) -> Result<settings::SettingsSnapshot, String> {
    let controller = state.controller.clone();
    let suppression = suppression.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        apply_auto_rename(&app, controller, suppression, Some(enabled))
    })
    .await
    .map_err(|error| error.to_string())?
}

fn publish_watcher_state(app: &AppHandle) -> Result<settings::SettingsSnapshot, String> {
    let publication = app.state::<WatcherPublication>();
    let _guard = publication
        .gate
        .lock()
        .map_err(|_| "Settings are unavailable.")?;
    let mut snapshot = app
        .state::<SettingsState>()
        .controller
        .lock()
        .map_err(|_| "Settings are unavailable.")?
        .snapshot();
    snapshot.revision = publication
        .revision
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        + 1;
    let _ = app.emit("watcher-state-changed", &snapshot);
    tray::refresh(app, &snapshot, publication.revision.clone());
    Ok(snapshot)
}
fn apply_auto_rename(
    app: &AppHandle,
    shared_controller: Arc<Mutex<settings_controller::SettingsController<FolderWatcher>>>,
    suppression: Arc<SuppressedPaths>,
    enabled: Option<bool>,
) -> Result<settings::SettingsSnapshot, String> {
    let mut controller = shared_controller
        .lock()
        .map_err(|_| "Settings are unavailable.")?;
    let mut settings = controller.snapshot().settings;
    settings.auto_rename = enabled.unwrap_or(!settings.auto_rename);
    let rules = controller.processing_settings();
    let result = controller.apply(settings, |folder, enabled, error| {
        prepare_watcher(app, folder, enabled, error, suppression, rules)
    });
    drop(controller);
    let snapshot = publish_watcher_state(app)?;
    result.map(|_| snapshot)
}

fn prepare_watcher(
    app: &AppHandle,
    folder: &str,
    enabled: Arc<std::sync::atomic::AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    suppression: Arc<SuppressedPaths>,
    rules: Arc<Mutex<settings::ApplicationSettings>>,
) -> Result<FolderWatcher, String> {
    let file_app = app.clone();
    let error_app = app.clone();
    let processing_enabled = enabled.clone();
    let error_enabled = enabled.clone();
    let retained_suppression = suppression.clone();
    FolderWatcher::start_controlled(
        settings::validate_folder(folder)?,
        move |file| {
            if processing_enabled.load(std::sync::atomic::Ordering::SeqCst) {
                let _ = file_app.emit(
                    "screenshot-detected",
                    file.original_path().to_string_lossy().into_owned(),
                );
                let _ = process_automatic_screenshot(
                    &file_app,
                    file,
                    &processing_enabled,
                    &retained_suppression,
                    &rules,
                    None,
                );
            }
        },
        suppression,
        enabled,
        move |message| {
            error_enabled.store(false, std::sync::atomic::Ordering::SeqCst);
            if let Ok(mut failure) = error.lock() {
                *failure = Some(message.clone());
            }
            let app = error_app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let _ = publish_watcher_state(&app);
            });
            notifications::publish(&error_app, notifications::Outcome::WatcherFailed);
        },
    )
    .map_err(|error| error.to_string())
}

fn process_automatic_screenshot(
    app: &AppHandle,
    file: ScreenshotFile,
    enabled: &Arc<std::sync::atomic::AtomicBool>,
    suppression: &SuppressedPaths,
    preferences: &Arc<Mutex<settings::ApplicationSettings>>,
    retry: Option<processing_mode::PendingAnalysis>,
) -> Result<RenameHistoryView, String> {
    let settings = preferences
        .lock()
        .map_err(|_| "Settings unavailable.")?
        .clone();
    if let Some(pending) = &retry {
        pending.verify()?;
    }
    let live_settings = preferences.clone();
    let starting_ai = settings.ai.clone();
    let live_enabled = enabled.clone();
    let live = move || {
        live_enabled.load(std::sync::atomic::Ordering::SeqCst)
            && live_settings
                .lock()
                .map(|settings| settings.ai == starting_ai && settings.ai.authorize_cloud().is_ok())
                .unwrap_or(false)
    };
    let authorized: Arc<dyn Fn() -> bool + Send + Sync> = Arc::new(live);
    let mut uploaded_digest = None;
    let analysis = processing_mode::route(
        &file,
        &settings.ai,
        &settings.naming,
        |metadata| {
            let result = ocr_service::analyze(file.original_path());
            let warning = result
                .as_ref()
                .err()
                .map(|error| format!("{error} Using local fallback naming."));
            ocr_service::enrich_metadata(metadata, result);
            warning
        },
        |metadata| {
            use credentials::CredentialStore;
            if !enabled.load(std::sync::atomic::Ordering::SeqCst) || !authorized() {
                return Err(vision_provider::VisionError::Configuration);
            }
            let credentials = Arc::new(credentials::OsCredentials::default());
            drop(credentials.load()?);
            let bytes = processing_mode::read_image(&file)?;
            uploaded_digest = Some(processing_mode::fingerprint(&bytes));
            let request = vision_provider::VisionRequest::new(
                bytes,
                metadata.cleaned_ocr_text.as_deref().unwrap_or(""),
            )?;
            let registry = vision_providers::configured_registry(
                settings.ai.clone(),
                credentials,
                authorized,
            )?;
            if !enabled.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(vision_provider::VisionError::Configuration);
            }
            registry.analyze(settings.ai.provider, &request)
        },
    );
    let analysis = match analysis {
        Ok(analysis) => analysis,
        Err(error) => {
            notifications::publish(app, notifications::Outcome::AnalysisFailed);
            if let processing_mode::ProcessingError::Vision(kind) = &error {
                let state = app.state::<AnalysisState>();
                let mut recovery = state
                    .0
                    .lock()
                    .map_err(|_| "Analysis recovery unavailable.")?;
                recovery.record(file, *kind, retry, uploaded_digest);
                let _ = app.emit("analysis-failures-changed", recovery.failures());
            } else {
                let _ = app.emit("screenshot-naming-failed", error.to_string());
            }
            return Err(error.to_string());
        }
    };
    if let Some(warning) = &analysis.warning {
        let _ = app.emit("screenshot-analysis-warning", warning);
    }
    let history = app.state::<HistoryState>();
    let _gate = history
        .undo_gate
        .lock()
        .map_err(|_| "History unavailable.")?;
    if !enabled.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("Processing was paused.".into());
    }
    if let Some(pending) = &retry {
        pending.verify()?;
    }
    if let Some(digest) = uploaded_digest {
        if processing_mode::fingerprint(
            &processing_mode::read_image(&file).map_err(|e| e.to_string())?,
        ) != digest
        {
            return Err("Screenshot changed during analysis; it has not been renamed.".into());
        }
    }
    let store = history
        .path()
        .map_err(HistoryStoreError)
        .and_then(|path| HistoryStore::open(&path));
    let mut recovery = history
        .recovery
        .lock()
        .map_err(|_| "Rename recovery unavailable.")?;
    let result = recovery.rename(
        file,
        &analysis.candidate,
        store
            .as_ref()
            .map_err(|error| HistoryStoreError(error.to_string())),
    );
    if let Ok(entry) = &result {
        if let Some(path) = &entry.new_path {
            suppression.suppress(std::path::Path::new(path));
        }
    }
    emit_rename_result(app, &result);
    if result.is_ok() {
        let _ = app.emit(
            "screenshot-analysis-completed",
            analysis.completion_message(),
        );
    }
    result.map_err(|failure| failure.message)
}

#[derive(Default)]
pub struct AnalysisState(Mutex<processing_mode::AnalysisRecovery>);
#[tauri::command]
fn list_analysis_failures(
    state: State<'_, AnalysisState>,
) -> Result<Vec<processing_mode::AnalysisFailure>, String> {
    state
        .0
        .lock()
        .map(|recovery| recovery.failures())
        .map_err(|_| "Analysis recovery unavailable.".into())
}
#[tauri::command]
async fn retry_analysis(
    app: AppHandle,
    state: State<'_, SettingsState>,
    id: String,
) -> Result<RenameHistoryView, String> {
    let preferences = state
        .controller
        .lock()
        .map_err(|_| "Settings unavailable.")?
        .processing_settings();
    tauri::async_runtime::spawn_blocking(move || {
        let pending = {
            let state = app.state::<AnalysisState>();
            let mut recovery = state
                .0
                .lock()
                .map_err(|_| "Analysis recovery unavailable.")?;
            let result = recovery.reserve(&id);
            let _ = app.emit("analysis-failures-changed", recovery.failures());
            result?
        };
        let file = pending.file.clone();
        process_automatic_screenshot(
            &app,
            file,
            &Arc::new(std::sync::atomic::AtomicBool::new(true)),
            &app.state::<SuppressionState>().0,
            &preferences,
            Some(pending),
        )
    })
    .await
    .map_err(|_| "Analysis retry failed.".to_string())?
}
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
    recovery: Arc<Mutex<RenameRecovery>>,
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
            .and_then(|store| store.list_recent_successful(50).ok())
            .unwrap_or_default()
            .into_iter()
            .map(|entry| entry.view())
            .collect();
        Self {
            database_path,
            startup_recent,
            undo_gate: Arc::new(Mutex::new(())),
            recovery: Arc::new(Mutex::new(RenameRecovery::default())),
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
async fn start_watching(
    app: AppHandle,
    state: State<'_, SettingsState>,
    suppression: State<'_, SuppressionState>,
    directory: String,
) -> Result<(), String> {
    let settings = settings::ApplicationSettings {
        watched_folder: Some(directory),
        auto_rename: true,
        ..get_settings(app.clone(), state.clone()).await?.settings
    };
    update_settings(app, state, suppression, settings)
        .await
        .map(|_| ())
}
#[tauri::command]
async fn stop_watching(
    app: AppHandle,
    state: State<'_, SettingsState>,
    suppression: State<'_, SuppressionState>,
) -> Result<(), String> {
    set_auto_rename(app, state, suppression, false)
        .await
        .map(|_| ())
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
            .list_recent_successful(limit.unwrap_or(50).min(100))
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
async fn get_recent_history(
    state: State<'_, HistoryState>,
    limit: Option<usize>,
) -> Result<history_ui::RecentHistory, String> {
    let path = state.path()?;
    let gate = Arc::clone(&state.undo_gate);
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = gate
            .lock()
            .map_err(|_| "History is unavailable.".to_owned())?;
        let store = HistoryStore::open(&path).map_err(|error| error.to_string())?;
        history_ui::recent_history(&store, limit.unwrap_or(50).min(100))
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn rename_and_record(
    app: AppHandle,
    state: State<'_, HistoryState>,
    original_path: String,
    candidate_stem: String,
) -> Result<RenameHistoryView, RenameFailure> {
    let path = state.path();
    let gate = Arc::clone(&state.undo_gate);
    let recovery = Arc::clone(&state.recovery);
    let error_path = original_path.clone();
    let event_app = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _guard = gate
            .lock()
            .map_err(|_| command_failure(&original_path, "rename lock is unavailable"))?;
        let mut recovery = recovery
            .lock()
            .map_err(|_| command_failure(&original_path, "rename recovery is unavailable"))?;
        let result = (|| {
            let original = std::path::absolute(&original_path)
                .map_err(|error| command_failure(&original_path, &error.to_string()))?;
            let screenshot = ScreenshotFile::new(original, None).map_err(|_| {
                command_failure(&original_path, "Choose a screenshot file with a filename.")
            })?;
            let store = path
                .map_err(HistoryStoreError)
                .and_then(|path| HistoryStore::open(&path));
            recovery.rename(
                screenshot,
                &candidate_stem,
                store
                    .as_ref()
                    .map_err(|error| HistoryStoreError(error.to_string())),
            )
        })();
        emit_rename_result(&event_app, &result);
        result
    })
    .await
    .map_err(|error| command_failure(&error_path, &error.to_string()))?;
    result
}

#[tauri::command]
async fn list_rename_failures(
    state: State<'_, HistoryState>,
) -> Result<Vec<RenameFailure>, String> {
    let recovery = Arc::clone(&state.recovery);
    tauri::async_runtime::spawn_blocking(move || {
        recovery
            .lock()
            .map(|recovery| recovery.failures())
            .map_err(|_| "rename recovery is unavailable".into())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RenameRetryResult {
    id: String,
    history: Option<RenameHistoryView>,
    failure: Option<RenameFailure>,
}

#[tauri::command]
fn preview_naming(rules: naming_engine::NamingRules) -> Result<String, String> {
    naming_templates::preview(&rules).map_err(|error| error.to_string())
}

#[tauri::command]
async fn retry_rename(
    app: AppHandle,
    state: State<'_, HistoryState>,
    id: String,
    candidate_stem: Option<String>,
) -> Result<RenameHistoryView, RenameFailure> {
    let path = state.path();
    let gate = Arc::clone(&state.undo_gate);
    let recovery = Arc::clone(&state.recovery);
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _guard = gate
            .lock()
            .map_err(|_| command_failure("", "rename lock is unavailable"))?;
        let mut recovery = recovery
            .lock()
            .map_err(|_| command_failure("", "rename recovery is unavailable"))?;
        let store = path
            .map_err(HistoryStoreError)
            .and_then(|path| HistoryStore::open(&path));
        let result = recovery.retry(
            &id,
            candidate_stem.as_deref(),
            store
                .as_ref()
                .map_err(|error| HistoryStoreError(error.to_string())),
        );
        emit_rename_result(&app, &result);
        let event = RenameRetryResult {
            id,
            history: result.as_ref().ok().cloned(),
            failure: result.as_ref().err().cloned(),
        };
        let _ = app.emit("screenshot-rename-retry-result", event);
        result
    })
    .await
    .map_err(|error| command_failure("", &error.to_string()))?;
    result
}

fn command_failure(path: &str, message: &str) -> RenameFailure {
    RenameFailure {
        id: String::new(),
        category: RenameFailureCategory::RetryUnavailable,
        source_path: path.into(),
        attempted_destination: None,
        actual_path: path.into(),
        stage: "prepare".into(),
        message: message.into(),
        retryable: false,
        filesystem_succeeded: false,
    }
}

fn emit_rename_result(app: &AppHandle, result: &Result<RenameHistoryView, RenameFailure>) {
    match result {
        Ok(history) => {
            let _ = app.emit("screenshot-renamed", history);
            notifications::publish(
                app,
                notifications::Outcome::Renamed {
                    id: &history.id,
                    name: history.new_name.as_deref().unwrap_or(""),
                },
            );
        }
        Err(failure) => {
            let _ = app.emit("screenshot-rename-failed", failure);
            notifications::publish(app, notifications::Outcome::RenameFailed);
        }
    }
}

#[tauri::command]
async fn undo_history(
    app: AppHandle,
    state: State<'_, HistoryState>,
    suppression: State<'_, SuppressionState>,
    id: String,
) -> Result<RenameHistoryView, history_ui::UndoFailure> {
    let path = state.path().map_err(history_ui::UndoFailure::unavailable)?;
    let gate = Arc::clone(&state.undo_gate);
    let suppressed = Arc::clone(&suppression.0);
    let event_id = id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _guard = gate.lock().map_err(|_| {
            history_ui::UndoFailure::unavailable("Undo lock is unavailable.".into())
        })?;
        let store = HistoryStore::open(&path)
            .map_err(|error| history_ui::UndoFailure::from(UndoError::HistoryRead(error)))?;
        undo_and_persist_suppressing(&id, &store, &suppressed)
            .map_err(history_ui::UndoFailure::from)
    })
    .await
    .map_err(|error| history_ui::UndoFailure::unavailable(error.to_string()))?;
    let _ = app.emit("screenshot-undo-result", event_id);
    if result.is_err() {
        notifications::publish(&app, notifications::Outcome::UndoFailed);
    }
    result
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
        .manage(startup::LaunchState::default())
        .manage(startup::StartupGate::default())
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            if !args.iter().any(|arg| arg == "--autostart") {
                app.state::<startup::LaunchState>()
                    .0
                    .store(true, std::sync::atomic::Ordering::SeqCst);
                let _ = tray::show_main(app);
            }
        }))
        .on_window_event(desktop::handle_window_event)
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(SuppressionState::default())
        .manage(AnalysisState::default())
        .manage(WatcherPublication::default())
        .setup(|app| {
            app.manage(HistoryState::from_app(&app.handle()));
            let settings = SettingsState::from_app(&app.handle());
            app.manage(settings);
            {
                let settings = app.state::<SettingsState>();
                let mut controller = settings
                    .controller
                    .lock()
                    .map_err(|_| "Settings unavailable")?;
                let rules = controller.processing_settings();
                app.manage(notifications::NotificationState::new(rules.clone()));
                controller.restore(|folder, enabled, error| {
                    prepare_watcher(
                        &app.handle(),
                        folder,
                        enabled,
                        error,
                        Arc::clone(&app.state::<SuppressionState>().0),
                        rules,
                    )
                });
            }
            let snapshot = app
                .state::<SettingsState>()
                .controller
                .lock()
                .map_err(|_| "Settings unavailable")?
                .snapshot();
            if tray::initialize(app.handle(), &snapshot).is_err() {
                app.state::<SettingsState>()
                    .controller
                    .lock()
                    .map_err(|_| "Settings unavailable")?
                    .set_warning("System tray unavailable. Keep the main window open.".into());
            }
            let hidden = startup::start_hidden(
                std::env::args().any(|arg| arg == "--autostart"),
                app.try_state::<tray::TrayState>().is_some(),
                app.state::<startup::LaunchState>()
                    .0
                    .load(std::sync::atomic::Ordering::SeqCst),
                snapshot.watcher_status == "Error" || snapshot.warning.is_some(),
            );
            if !hidden {
                let _ = tray::show_main(app.handle());
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            startup::get_startup_enabled,
            startup::set_startup_enabled,
            has_ai_credential,
            set_ai_credential,
            delete_ai_credential,
            get_settings,
            preview_naming,
            update_settings,
            set_auto_rename,
            start_watching,
            stop_watching,
            startup_history,
            list_recent_history,
            get_recent_history,
            rename_and_record,
            list_rename_failures,
            retry_rename,
            list_analysis_failures,
            retry_analysis,
            undo_history
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
