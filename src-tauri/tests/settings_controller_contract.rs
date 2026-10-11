use screenshot_renamer_lib::{
    settings::{ApplicationSettings, SettingsStore},
    settings_controller::{SettingsController, WatchSession},
};
use std::{
    fs,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tempfile::tempdir;
struct Watch(Arc<AtomicBool>);
impl WatchSession for Watch {
    fn stop(&mut self) -> Result<(), String> {
        self.0.store(true, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn failed_replacement_keeps_the_working_folder_and_saved_preference() {
    let dir = tempdir().unwrap();
    let old = dir.path().join("old");
    let new = dir.path().join("new");
    fs::create_dir(&old).unwrap();
    fs::create_dir(&new).unwrap();
    let path = dir.path().join("settings.json");
    let mut controller = SettingsController::<Watch>::new(Ok(SettingsStore::new(path.clone())));
    let stopped = Arc::new(AtomicBool::new(false));
    let initial = ApplicationSettings {
        watched_folder: Some(old.to_string_lossy().into()),
        auto_rename: true,
        ..Default::default()
    };
    controller
        .apply(initial.clone(), |_, _| Ok(Watch(stopped.clone())))
        .unwrap();
    let replacement = ApplicationSettings {
        watched_folder: Some(new.to_string_lossy().into()),
        ..initial.clone()
    };
    assert!(controller
        .apply(replacement, |_, _| Err("Cannot watch new folder".into()))
        .is_err());
    assert_eq!(controller.snapshot().settings, initial);
    assert_eq!(SettingsStore::new(path).load().settings, initial);
    assert!(!stopped.load(Ordering::SeqCst));
    assert!(controller.is_watching());
}

#[test]
fn failed_save_disposes_the_prepared_watcher_and_retains_previous_values() {
    let dir = tempdir().unwrap();
    let folder = dir.path().join("screenshots");
    fs::create_dir(&folder).unwrap();
    let blocker = dir.path().join("blocker");
    fs::write(&blocker, "not a directory").unwrap();
    let mut controller =
        SettingsController::<Watch>::new(Ok(SettingsStore::new(blocker.join("settings.json"))));
    let stopped = Arc::new(AtomicBool::new(false));
    let settings = ApplicationSettings {
        watched_folder: Some(folder.to_string_lossy().into()),
        auto_rename: true,
        ..Default::default()
    };
    assert!(controller
        .apply(settings, |_, _| Ok(Watch(stopped.clone())))
        .is_err());
    assert_eq!(
        controller.snapshot().settings,
        ApplicationSettings::default()
    );
    assert!(stopped.load(Ordering::SeqCst));
    assert!(!controller.is_watching());
}
