use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::Manager;

#[derive(Default)]
pub struct WindowIntents(AtomicU64);
impl WindowIntents {
    pub fn capture(&self) -> u64 {
        self.0.fetch_add(1, Ordering::SeqCst) + 1
    }
    pub fn cancel(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
    pub fn is_current(&self, revision: u64) -> bool {
        self.0.load(Ordering::SeqCst) == revision
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopPreferences {
    pub minimize_to_tray: bool,
    #[serde(default)]
    pub start_with_windows: bool,
}
impl Default for DesktopPreferences {
    fn default() -> Self {
        Self {
            minimize_to_tray: true,
            start_with_windows: false,
        }
    }
}
#[derive(Clone, Copy)]
pub enum WindowIntent {
    Close,
    Minimize,
    Restore,
}
#[derive(Debug, PartialEq, Eq)]
pub enum WindowAction {
    Hide,
    Exit,
    Keep,
    Show,
}
pub fn window_action(
    intent: WindowIntent,
    tray_ready: bool,
    watching: bool,
    quitting: bool,
    minimize_to_tray: bool,
) -> WindowAction {
    match intent {
        WindowIntent::Close if tray_ready && watching && !quitting => WindowAction::Hide,
        WindowIntent::Close => WindowAction::Exit,
        WindowIntent::Minimize if tray_ready && minimize_to_tray && !quitting => WindowAction::Hide,
        WindowIntent::Minimize => WindowAction::Keep,
        WindowIntent::Restore => WindowAction::Show,
    }
}
pub fn handle_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if window.label() != "main" {
        return;
    }
    let app = window.app_handle();
    let Some(tray) = app.try_state::<crate::tray::TrayState>() else {
        return;
    };
    let quitting = tray.quitting.load(std::sync::atomic::Ordering::SeqCst);
    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
    }
    if quitting {
        return;
    }
    let intent = match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            WindowIntent::Close
        }
        tauri::WindowEvent::Resized(_) if window.is_minimized().unwrap_or(false) => {
            WindowIntent::Minimize
        }
        _ => return,
    };
    let intents = tray.window_intents.clone();
    let revision = intents.capture();
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let settings = app.state::<crate::SettingsState>();
        let Ok(controller) = settings.controller.lock() else {
            crate::tray::report_error(&app, "Window controls are unavailable.");
            return;
        };
        let snapshot = controller.snapshot();
        drop(controller);
        let quitting = app
            .state::<crate::tray::TrayState>()
            .quitting
            .load(std::sync::atomic::Ordering::SeqCst);
        let action = window_action(
            intent,
            true,
            snapshot.watcher_status == "Watching",
            quitting,
            snapshot.settings.desktop.minimize_to_tray,
        );
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            if !intents.is_current(revision)
                || handle
                    .state::<crate::tray::TrayState>()
                    .quitting
                    .load(Ordering::SeqCst)
            {
                return;
            }
            match action {
                WindowAction::Hide => {
                    if let Some(window) = handle.get_webview_window("main") {
                        if window.hide().is_err() {
                            crate::tray::report_error(
                                &handle,
                                "Could not hide the window. Use Quit to exit.",
                            );
                        }
                    }
                }
                WindowAction::Exit => crate::tray::quit(&handle),
                _ => {}
            }
        });
    });
}
