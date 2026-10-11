use screenshot_renamer_lib::{settings::SettingsSnapshot, tray::TrayPresentation};

#[test]
fn tray_only_offers_resumable_actions_and_reflects_failures() {
    let mut snapshot = SettingsSnapshot::new(Default::default(), None);
    let empty = TrayPresentation::from_snapshot(&snapshot);
    assert!(!empty.toggle_enabled);
    assert!(empty.status.contains("Choose"));
    snapshot.settings.watched_folder = Some("C:/Screenshots".into());
    let paused = TrayPresentation::from_snapshot(&snapshot);
    assert_eq!(paused.toggle_label, "Resume");
    assert!(paused.toggle_enabled);
    snapshot.settings.auto_rename = true;
    snapshot.watcher_status = "Watching".into();
    assert_eq!(
        TrayPresentation::from_snapshot(&snapshot).toggle_label,
        "Pause"
    );
    snapshot.watcher_status = "Error".into();
    snapshot.watcher_error = Some("Folder disappeared".into());
    let failed = TrayPresentation::from_snapshot(&snapshot);
    assert!(failed.status.contains("Error"));
    assert_eq!(failed.toggle_label, "Pause");
}
