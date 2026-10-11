use screenshot_renamer_lib::startup::{set_startup, StartupRegistration};
use std::cell::Cell;
struct Registration {
    enabled: Cell<bool>,
    fail: bool,
}
impl StartupRegistration for Registration {
    fn is_enabled(&self) -> Result<bool, String> {
        Ok(self.enabled.get())
    }
    fn set_enabled(&self, enabled: bool) -> Result<(), String> {
        if self.fail {
            Err("Registration unavailable".into())
        } else {
            self.enabled.set(enabled);
            Ok(())
        }
    }
}
#[test]
fn enables_disables_and_saves_confirmed_registration() {
    let os = Registration {
        enabled: Cell::new(false),
        fail: false,
    };
    let saved = Cell::new(false);
    assert!(set_startup(&os, true, |enabled| {
        saved.set(enabled);
        Ok(())
    })
    .unwrap());
    assert!(saved.get());
    assert!(!set_startup(&os, false, |enabled| {
        saved.set(enabled);
        Ok(())
    })
    .unwrap());
    assert!(!saved.get());
}
#[test]
fn registration_failure_does_not_save_and_save_failure_rolls_back_os() {
    let os = Registration {
        enabled: Cell::new(false),
        fail: true,
    };
    assert!(set_startup(&os, true, |_| panic!("must not save failed registration")).is_err());
    assert!(!os.enabled.get());
    let os = Registration {
        enabled: Cell::new(false),
        fail: false,
    };
    assert!(set_startup(&os, true, |_| Err("Disk full".into())).is_err());
    assert!(!os.enabled.get());
}

#[test]
fn partial_registration_failure_restores_previous_state() {
    struct Partial(Cell<bool>);
    impl StartupRegistration for Partial {
        fn is_enabled(&self) -> Result<bool, String> {
            Ok(self.0.get())
        }
        fn set_enabled(&self, enabled: bool) -> Result<(), String> {
            self.0.set(enabled);
            if enabled {
                Err("Partial failure".into())
            } else {
                Ok(())
            }
        }
    }
    let os = Partial(Cell::new(false));
    assert!(set_startup(&os, true, |_| panic!("must not persist partial failure")).is_err());
    assert!(!os.0.get());
}

#[test]
fn login_errors_keep_window_visible_and_executable_paths_are_quoted() {
    assert!(!screenshot_renamer_lib::startup::start_hidden(
        true, true, false, true
    ));
    assert!(screenshot_renamer_lib::startup::start_hidden(
        true, true, false, false
    ));
    assert!(!screenshot_renamer_lib::startup::start_hidden(
        false, true, false, false
    ));
    assert!(!screenshot_renamer_lib::startup::start_hidden(
        true, true, true, false
    ));
    assert!(!screenshot_renamer_lib::startup::start_hidden(
        true, false, false, false
    ));
    assert_eq!(
        screenshot_renamer_lib::startup::startup_command(
            "C:\\Program Files\\Screenshot Renamer\\app.exe"
        )
        .unwrap(),
        "\"C:\\Program Files\\Screenshot Renamer\\app.exe\" --autostart"
    );
}

#[test]
fn startup_can_be_disabled_after_watched_folder_disappears() {
    use screenshot_renamer_lib::{
        settings::{ApplicationSettings, SettingsStore},
        settings_controller::{SettingsController, WatchSession},
    };
    struct Watch;
    impl WatchSession for Watch {
        fn stop(&mut self) -> Result<(), String> {
            Ok(())
        }
    }
    let config = tempfile::tempdir().unwrap();
    let watched = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(config.path().join("settings.json"));
    let mut settings = ApplicationSettings {
        watched_folder: Some(watched.path().to_string_lossy().into_owned()),
        auto_rename: true,
        ..Default::default()
    };
    settings.desktop.start_with_windows = true;
    store.save(&settings).unwrap();
    let mut controller = SettingsController::<Watch>::new(Ok(store));
    controller.restore(|_, _, _| Err("Folder unavailable".into()));
    watched.close().unwrap();
    let os = Registration {
        enabled: Cell::new(true),
        fail: false,
    };
    assert!(!set_startup(&os, false, |enabled| {
        let mut desktop = controller.snapshot().settings.desktop;
        desktop.start_with_windows = enabled;
        controller.save_desktop_preferences(desktop).map(|_| ())
    })
    .unwrap());
    assert_eq!(controller.snapshot().watcher_status, "Error");
    assert!(controller.snapshot().settings.auto_rename);
    assert!(!controller.snapshot().settings.desktop.start_with_windows);
}
