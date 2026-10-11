use crate::settings::SettingsSnapshot;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

pub struct TrayPresentation {
    pub status: String,
    pub toggle_label: &'static str,
    pub toggle_enabled: bool,
}
impl TrayPresentation {
    pub fn from_snapshot(snapshot: &SettingsSnapshot) -> Self {
        Self {
            status: if snapshot.settings.watched_folder.is_none() {
                "Choose a screenshot folder".into()
            } else {
                format!("Screenshot Renamer — {}", snapshot.watcher_status)
            },
            toggle_label: if snapshot.settings.auto_rename {
                "Pause"
            } else {
                "Resume"
            },
            toggle_enabled: snapshot.settings.watched_folder.is_some()
                || snapshot.settings.auto_rename,
        }
    }
}
pub struct TrayState {
    icon: TrayIcon,
    status: MenuItem<tauri::Wry>,
    toggle: MenuItem<tauri::Wry>,
    pub quitting: AtomicBool,
}
pub fn show_main(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or("Main window unavailable.")?;
    window
        .unminimize()
        .and_then(|_| window.show())
        .and_then(|_| window.set_focus())
        .map_err(|_| "Could not restore the main window.".into())
}
pub fn report_error(app: &AppHandle, message: &str) {
    let _ = show_main(app);
    let _ = app.emit("desktop-error", message);
}
pub fn refresh(
    app: &AppHandle,
    snapshot: &SettingsSnapshot,
    latest: std::sync::Arc<std::sync::atomic::AtomicU64>,
) {
    let view = TrayPresentation::from_snapshot(snapshot);
    let revision = snapshot.revision;
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if latest.load(Ordering::SeqCst) != revision {
            return;
        }
        if let Some(state) = handle.try_state::<TrayState>() {
            let result = state
                .status
                .set_text(&view.status)
                .and_then(|_| state.toggle.set_text(view.toggle_label))
                .and_then(|_| state.toggle.set_enabled(view.toggle_enabled))
                .and_then(|_| state.icon.set_tooltip(Some(&view.status)));
            if result.is_err() {
                report_error(
                    &handle,
                    "Could not update tray controls. Use the main window.",
                );
            }
        }
    });
}
pub fn initialize(app: &AppHandle, snapshot: &SettingsSnapshot) -> tauri::Result<()> {
    let view = TrayPresentation::from_snapshot(snapshot);
    let status = MenuItem::with_id(app, "tray-status", &view.status, false, None::<&str>)?;
    let open = MenuItem::with_id(app, "tray-open", "Open", true, None::<&str>)?;
    let toggle = MenuItem::with_id(
        app,
        "tray-toggle",
        view.toggle_label,
        view.toggle_enabled,
        None::<&str>,
    )?;
    let settings = MenuItem::with_id(app, "tray-settings", "Settings", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "tray-quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&status, &open, &toggle, &settings, &quit])?;
    let icon = app
        .default_window_icon()
        .ok_or_else(|| tauri::Error::AssetNotFound("Application icon".into()))?
        .clone();
    let icon = TrayIconBuilder::with_id("screenshot-renamer-tray")
        .icon(icon)
        .tooltip(&view.status)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|icon, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                if let Err(message) = show_main(icon.app_handle()) {
                    report_error(icon.app_handle(), &message);
                }
            }
        })
        .on_menu_event(|app, event| match event.id.as_ref() {
            "tray-open" => {
                if let Err(message) = show_main(app) {
                    report_error(app, &message);
                }
            }
            "tray-settings" => {
                if show_main(app).is_ok() {
                    let _ = app.emit("open-settings", ());
                }
            }
            "tray-toggle" => {
                let app = app.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    let controller = app.state::<crate::SettingsState>().controller.clone();
                    let suppression = app.state::<crate::SuppressionState>().0.clone();
                    let result = crate::apply_auto_rename(&app, controller, suppression, None);
                    if let Err(message) = result {
                        report_error(&app, &message);
                    }
                });
            }
            "tray-quit" => {
                if let Some(state) = app.try_state::<TrayState>() {
                    state.quitting.store(true, Ordering::SeqCst);
                }
                let app = app.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    if let Ok(mut controller) =
                        app.state::<crate::SettingsState>().controller.lock()
                    {
                        controller.shutdown();
                    }
                    app.exit(0);
                });
            }
            _ => {}
        })
        .build(app)?;
    app.manage(TrayState {
        icon,
        status,
        toggle,
        quitting: AtomicBool::new(false),
    });
    Ok(())
}
