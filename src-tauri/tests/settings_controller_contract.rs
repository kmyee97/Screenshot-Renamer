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
        .apply(initial.clone(), |_, _, _| Ok(Watch(stopped.clone())))
        .unwrap();
    let replacement = ApplicationSettings {
        watched_folder: Some(new.to_string_lossy().into()),
        ..initial.clone()
    };
    assert!(controller
        .apply(replacement, |_, _, _| Err("Cannot watch new folder".into()))
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
        .apply(settings, |_, _, _| Ok(Watch(stopped.clone())))
        .is_err());
    assert_eq!(
        controller.snapshot().settings,
        ApplicationSettings::default()
    );
    assert!(stopped.load(Ordering::SeqCst));
    assert!(!controller.is_watching());
}

#[test]
fn pause_resume_and_restart_restore_true_watcher_state() {
    let dir = tempdir().unwrap();
    let folder = dir.path().join("screenshots");
    fs::create_dir(&folder).unwrap();
    let path = dir.path().join("settings.json");
    let mut controller = SettingsController::<Watch>::new(Ok(SettingsStore::new(path.clone())));
    let on = ApplicationSettings {
        watched_folder: Some(folder.to_string_lossy().into()),
        auto_rename: true,
        ..Default::default()
    };
    controller
        .apply(on.clone(), |_, _, _| {
            Ok(Watch(Arc::new(AtomicBool::new(false))))
        })
        .unwrap();
    assert_eq!(controller.snapshot().watcher_status, "Watching");
    let off = ApplicationSettings {
        auto_rename: false,
        ..on.clone()
    };
    controller
        .apply(off, |_, _, _| Ok(Watch(Arc::new(AtomicBool::new(false)))))
        .unwrap();
    assert_eq!(controller.snapshot().watcher_status, "Paused");
    let mut restarted = SettingsController::<Watch>::new(Ok(SettingsStore::new(path)));
    restarted.restore(|_, _, _| panic!("Paused preferences must not start a watcher"));
    assert_eq!(restarted.snapshot().watcher_status, "Paused");
    controller
        .apply(on, |_, _, _| Ok(Watch(Arc::new(AtomicBool::new(false)))))
        .unwrap();
    assert_eq!(controller.snapshot().watcher_status, "Watching");
}

#[test]
fn watcher_restore_failure_is_error_not_watching() {
    let dir = tempdir().unwrap();
    let folder = dir.path().join("screenshots");
    fs::create_dir(&folder).unwrap();
    let path = dir.path().join("settings.json");
    SettingsStore::new(path.clone())
        .save(&ApplicationSettings {
            watched_folder: Some(folder.to_string_lossy().into()),
            auto_rename: true,
            ..Default::default()
        })
        .unwrap();
    let mut controller = SettingsController::<Watch>::new(Ok(SettingsStore::new(path)));
    controller.restore(|_, _, _| Err("Watch service unavailable".into()));
    assert_eq!(controller.snapshot().watcher_status, "Error");
    assert!(controller
        .snapshot()
        .watcher_error
        .unwrap()
        .contains("Watch service unavailable"));
}

#[test]
fn controlled_watcher_discards_paused_creates_and_reports_removed_folder() {
    use screenshot_renamer_lib::{FolderWatcher, SuppressedPaths};
    use std::{sync::Mutex, thread, time::Duration};
    let dir = tempdir().unwrap();
    let folder = dir.path().join("screenshots");
    fs::create_dir(&folder).unwrap();
    let active = Arc::new(AtomicBool::new(false));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_files = seen.clone();
    let errors = Arc::new(Mutex::new(Vec::new()));
    let emitted_errors = errors.clone();
    let mut watcher = FolderWatcher::start_controlled(
        folder.clone(),
        move |file| {
            seen_files
                .lock()
                .unwrap()
                .push(file.original_path().to_path_buf())
        },
        Arc::new(SuppressedPaths::default()),
        active.clone(),
        move |error| emitted_errors.lock().unwrap().push(error),
    )
    .unwrap();
    fs::write(folder.join("paused.png"), "image").unwrap();
    thread::sleep(Duration::from_millis(150));
    active.store(true, Ordering::SeqCst);
    fs::write(folder.join("active.png"), "image").unwrap();
    for _ in 0..40 {
        if !seen.lock().unwrap().is_empty() {
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    assert_eq!(*seen.lock().unwrap(), vec![folder.join("active.png")]);
    fs::remove_dir_all(&folder).unwrap();
    for _ in 0..40 {
        if !errors.lock().unwrap().is_empty() {
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    assert!(!errors.lock().unwrap().is_empty());
    watcher.stop().unwrap();
}
