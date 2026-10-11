use screenshot_renamer_lib::desktop::{window_action, WindowAction, WindowIntent};

#[test]
fn hiding_requires_a_tray_and_quit_always_exits() {
    assert_eq!(
        window_action(WindowIntent::Close, true, true, false, true),
        WindowAction::Hide
    );
    assert_eq!(
        window_action(WindowIntent::Close, true, false, false, true),
        WindowAction::Exit
    );
    assert_eq!(
        window_action(WindowIntent::Close, false, true, false, true),
        WindowAction::Exit
    );
    assert_eq!(
        window_action(WindowIntent::Minimize, true, false, false, true),
        WindowAction::Hide
    );
    assert_eq!(
        window_action(WindowIntent::Minimize, true, true, false, false),
        WindowAction::Keep
    );
    assert_eq!(
        window_action(WindowIntent::Close, true, true, true, true),
        WindowAction::Exit
    );
    assert_eq!(
        window_action(WindowIntent::Restore, true, true, false, true),
        WindowAction::Show
    );
}

#[test]
fn minimize_preference_persists_and_legacy_settings_get_a_default() {
    let settings: screenshot_renamer_lib::settings::ApplicationSettings =
        serde_json::from_str(r#"{"version":1,"watchedFolder":null,"autoRename":false}"#).unwrap();
    assert!(settings.desktop.minimize_to_tray);
    let dir = tempfile::tempdir().unwrap();
    let store =
        screenshot_renamer_lib::settings::SettingsStore::new(dir.path().join("settings.json"));
    let mut settings = settings;
    settings.desktop.minimize_to_tray = false;
    store.save(&settings).unwrap();
    assert!(!store.load().settings.desktop.minimize_to_tray);
}

#[test]
fn restoring_invalidates_a_deferred_hide_without_canceling_future_intents() {
    let intents = screenshot_renamer_lib::desktop::WindowIntents::default();
    let hide = intents.capture();
    assert!(intents.is_current(hide));
    intents.cancel();
    assert!(!intents.is_current(hide));
    let later = intents.capture();
    assert!(intents.is_current(later));
    assert!(!intents.is_current(hide));
}
