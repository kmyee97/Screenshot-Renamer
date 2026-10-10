use std::{
    path::PathBuf,
    time::{Duration, UNIX_EPOCH},
};

use screenshot_renamer_lib::{
    rename_screenshot_recorded, RenameHistoryEntry, RenameOutcome, ScreenshotFile, UndoStatus,
};

#[test]
fn successful_rename_preserves_paths_names_and_ui_timestamps() {
    let original = PathBuf::from(r"C:\Screenshots\Screenshot.png");
    let destination = PathBuf::from(r"C:\Screenshots\project.png");
    let mut entry = RenameHistoryEntry::new(
        original,
        UNIX_EPOCH + Duration::from_millis(1_700_000_000_123),
    )
    .unwrap();
    entry.mark_rename_succeeded(destination).unwrap();

    assert!(!entry.id().is_empty());
    assert_eq!(entry.outcome(), &RenameOutcome::Succeeded);
    let view = serde_json::to_value(entry.view()).unwrap();
    assert_eq!(view["originalPath"], r"C:\Screenshots\Screenshot.png");
    assert_eq!(view["originalName"], "Screenshot.png");
    assert_eq!(view["newPath"], r"C:\Screenshots\project.png");
    assert_eq!(view["newName"], "project.png");
    assert_eq!(view["attemptedAtMs"], 1_700_000_000_123_i64);
}

#[test]
fn failure_without_destination_does_not_fabricate_new_path() {
    let mut entry = RenameHistoryEntry::new(PathBuf::from("Screenshot.png"), UNIX_EPOCH).unwrap();
    entry
        .mark_rename_failed(None, "invalid name".into())
        .unwrap();
    assert!(matches!(entry.outcome(), RenameOutcome::Failed { .. }));
    let view = serde_json::to_value(entry.view()).unwrap();
    assert!(view["newPath"].is_null());
    assert!(view["newName"].is_null());
}

#[test]
fn failure_with_proposed_destination_keeps_that_path() {
    let mut entry = RenameHistoryEntry::new(PathBuf::from("Screenshot.png"), UNIX_EPOCH).unwrap();
    entry
        .mark_rename_failed(
            Some(PathBuf::from("project.png")),
            "permission denied".into(),
        )
        .unwrap();
    assert_eq!(entry.view().new_name.as_deref(), Some("project.png"));
}

#[test]
fn undo_transitions_require_success_and_reject_repeated_success() {
    let mut entry = RenameHistoryEntry::new(PathBuf::from("Screenshot.png"), UNIX_EPOCH).unwrap();
    assert!(entry.mark_undo_succeeded(UNIX_EPOCH).is_err());
    entry
        .mark_rename_succeeded(PathBuf::from("project.png"))
        .unwrap();
    entry
        .mark_undo_failed(UNIX_EPOCH + Duration::from_secs(1), "occupied".into())
        .unwrap();
    assert!(matches!(entry.undo_status(), UndoStatus::Failed { .. }));
    entry
        .mark_undo_succeeded(UNIX_EPOCH + Duration::from_secs(2))
        .unwrap();
    assert_eq!(entry.undo_status(), &UndoStatus::Succeeded { at_ms: 2_000 });
    assert!(entry
        .mark_undo_succeeded(UNIX_EPOCH + Duration::from_secs(3))
        .is_err());
    assert!(entry.mark_rename_failed(None, "too late".into()).is_err());
}

#[test]
fn rename_service_returns_a_complete_record_for_success_and_failure() {
    let directory = tempfile::tempdir().unwrap();
    let original = directory.path().join("Screenshot.png");
    std::fs::write(&original, b"image").unwrap();
    let mut screenshot = ScreenshotFile::new(original.clone(), None).unwrap();
    let success = rename_screenshot_recorded(&mut screenshot, "project");
    assert_eq!(success.history.original_path(), original);
    assert_eq!(
        success.history.new_path(),
        Some(directory.path().join("project.png").as_path())
    );
    assert!(success.result.is_ok());

    let mut missing = ScreenshotFile::new(directory.path().join("missing.png"), None).unwrap();
    let failure = rename_screenshot_recorded(&mut missing, "project");
    assert!(failure.result.is_err());
    assert!(matches!(
        failure.history.outcome(),
        RenameOutcome::Failed { .. }
    ));
    assert_eq!(failure.history.new_path(), None);
}
