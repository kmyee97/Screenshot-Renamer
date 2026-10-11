use screenshot_renamer_lib::settings::{ApplicationSettings, SettingsStore};
use std::fs;
use tempfile::tempdir;

#[test]
fn settings_survive_restart_and_replace_an_existing_file() {
    let dir = tempdir().unwrap();
    let folder = dir.path().join("screenshots");
    fs::create_dir(&folder).unwrap();
    let path = dir.path().join("config/settings.json");
    let store = SettingsStore::new(path.clone());
    let settings = ApplicationSettings {
        watched_folder: Some(folder.to_string_lossy().into()),
        auto_rename: true,
        ..Default::default()
    };
    store.save(&settings).unwrap();
    assert_eq!(SettingsStore::new(path.clone()).load().settings, settings);
    let paused = ApplicationSettings {
        auto_rename: false,
        ..settings
    };
    store.save(&paused).unwrap();
    assert_eq!(SettingsStore::new(path).load().settings, paused);
}

#[test]
fn missing_corrupt_unknown_version_and_invalid_fields_fall_back_safely() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let store = SettingsStore::new(path.clone());
    assert_eq!(store.load().settings, ApplicationSettings::default());
    assert!(store.load().warning.is_some());
    for json in [
        "broken",
        r#"{"version":99,"watchedFolder":null,"autoRename":true}"#,
        r#"{"version":1,"watchedFolder":null,"autoRename":"yes"}"#,
        r#"{"version":1,"watchedFolder":"Z:/missing-screen-folder","autoRename":true}"#,
    ] {
        fs::write(&path, json).unwrap();
        let loaded = store.load();
        assert_eq!(loaded.settings, ApplicationSettings::default());
        assert!(loaded.warning.is_some());
    }
}

#[test]
fn invalid_updates_and_abandoned_temporary_writes_preserve_saved_settings() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let store = SettingsStore::new(path.clone());
    store.save(&ApplicationSettings::default()).unwrap();
    fs::write(dir.path().join(".settings-interrupted.tmp"), "half written").unwrap();
    let invalid = ApplicationSettings {
        watched_folder: Some(dir.path().join("missing").to_string_lossy().into()),
        auto_rename: true,
        ..Default::default()
    };
    assert!(store.save(&invalid).is_err());
    assert_eq!(store.load().settings, ApplicationSettings::default());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
}
