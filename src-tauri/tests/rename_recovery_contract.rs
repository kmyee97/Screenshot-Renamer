use std::fs;

use screenshot_renamer_lib::{
    HistoryStore, RenameFailureCategory, RenameOutcome, RenameRecovery, ScreenshotFile,
};

fn screenshot(path: std::path::PathBuf) -> ScreenshotFile {
    ScreenshotFile::new(path, None).unwrap()
}

#[test]
fn invalid_name_failure_is_actionable_and_can_retry_with_a_corrected_name() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("Screenshot.png");
    fs::write(&source, b"image").unwrap();
    let store = HistoryStore::open(&directory.path().join("history.sqlite")).unwrap();
    let mut recovery = RenameRecovery::default();
    let failure = recovery
        .rename(screenshot(source.clone()), "CON", Ok(&store))
        .unwrap_err();
    assert_eq!(failure.category, RenameFailureCategory::InvalidName);
    assert_eq!(failure.source_path, source.to_string_lossy());
    assert_eq!(failure.stage, "rename");
    assert!(failure.retryable);
    assert!(!failure.message.is_empty());
    assert_eq!(fs::read(&source).unwrap(), b"image");
    assert!(matches!(
        store.list_recent(10).unwrap()[0].outcome(),
        RenameOutcome::Failed { .. }
    ));

    fs::write(directory.path().join("project.png"), b"existing").unwrap();
    let view = recovery
        .retry(&failure.id, Some("project"), Ok(&store))
        .unwrap();
    assert_eq!(view.new_name.as_deref(), Some("project (2).png"));
    assert_eq!(
        fs::read(directory.path().join("project.png")).unwrap(),
        b"existing"
    );
    assert_eq!(
        fs::read(directory.path().join("project (2).png")).unwrap(),
        b"image"
    );
    assert!(recovery.failures().is_empty());
    assert!(recovery.retry(&failure.id, None, Ok(&store)).is_err());
    assert_eq!(store.list_recent(10).unwrap().len(), 2);
}

#[test]
fn missing_source_failure_does_not_block_later_files() {
    let directory = tempfile::tempdir().unwrap();
    let store = HistoryStore::open(&directory.path().join("history.sqlite")).unwrap();
    let mut recovery = RenameRecovery::default();
    let source = directory.path().join("missing.png");
    let failure = recovery
        .rename(screenshot(source.clone()), "project", Ok(&store))
        .unwrap_err();
    assert_eq!(failure.category, RenameFailureCategory::SourceMissing);
    assert_eq!(
        recovery
            .retry(&failure.id, None, Ok(&store))
            .unwrap_err()
            .category,
        RenameFailureCategory::SourceMissing
    );
    let next = directory.path().join("next.png");
    fs::write(&next, b"next image").unwrap();
    recovery
        .rename(screenshot(next), "next-project", Ok(&store))
        .unwrap();
    fs::write(&source, b"restored image").unwrap();
    recovery.retry(&failure.id, None, Ok(&store)).unwrap();
    assert_eq!(
        fs::read(directory.path().join("project.png")).unwrap(),
        b"restored image"
    );
}

#[test]
fn moved_file_history_retry_preserves_recreated_original_and_is_idempotent() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("Screenshot.png");
    let destination = directory.path().join("project.png");
    fs::write(&source, b"image").unwrap();
    let database = directory.path().join("history.sqlite");
    let store = HistoryStore::open(&database).unwrap();
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection.execute_batch("CREATE TRIGGER reject_insert BEFORE INSERT ON entries BEGIN SELECT RAISE(FAIL, 'blocked'); END;").unwrap();
    let mut recovery = RenameRecovery::default();
    let failure = recovery
        .rename(screenshot(source.clone()), "project", Ok(&store))
        .unwrap_err();
    assert_eq!(failure.category, RenameFailureCategory::HistoryWrite);
    assert_eq!(failure.stage, "history");
    assert!(failure.filesystem_succeeded);
    assert_eq!(failure.actual_path, destination.to_string_lossy());
    assert!(store.list_recent(10).unwrap().is_empty());
    assert!(!source.exists());
    assert_eq!(fs::read(&destination).unwrap(), b"image");
    fs::write(&source, b"new screenshot").unwrap();
    assert!(recovery.retry(&failure.id, None, Ok(&store)).is_err());
    connection
        .execute_batch("DROP TRIGGER reject_insert;")
        .unwrap();
    let saved = recovery.retry(&failure.id, None, Ok(&store)).unwrap();
    assert_eq!(saved.outcome, RenameOutcome::Succeeded);
    assert_eq!(store.list_recent(10).unwrap().len(), 1);
    assert!(recovery.retry(&failure.id, None, Ok(&store)).is_err());
    assert_eq!(fs::read(&source).unwrap(), b"new screenshot");
    assert_eq!(fs::read(&destination).unwrap(), b"image");
    assert!(!directory.path().join("project (2).png").exists());
}

#[test]
fn history_retry_rejects_a_missing_or_replaced_destination() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("Screenshot.png");
    let destination = directory.path().join("project.png");
    fs::write(&source, b"image").unwrap();
    let database = directory.path().join("history.sqlite");
    let store = HistoryStore::open(&database).unwrap();
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection.execute_batch("CREATE TRIGGER reject_insert BEFORE INSERT ON entries BEGIN SELECT RAISE(FAIL, 'blocked'); END;").unwrap();
    let mut recovery = RenameRecovery::default();
    let failure = recovery
        .rename(screenshot(source), "project", Ok(&store))
        .unwrap_err();
    connection
        .execute_batch("DROP TRIGGER reject_insert;")
        .unwrap();
    fs::remove_file(&destination).unwrap();
    assert_eq!(
        recovery
            .retry(&failure.id, None, Ok(&store))
            .unwrap_err()
            .category,
        RenameFailureCategory::SourceMissing
    );
    fs::write(&destination, b"other").unwrap();
    assert_eq!(
        recovery
            .retry(&failure.id, None, Ok(&store))
            .unwrap_err()
            .category,
        RenameFailureCategory::FileChanged
    );
    assert!(store.list_recent(10).unwrap().is_empty());
    assert_eq!(fs::read(destination).unwrap(), b"other");
}

#[test]
fn history_open_failure_keeps_source_and_can_retry() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("Screenshot.png");
    fs::write(&source, b"image").unwrap();
    let store = HistoryStore::open(&directory.path().join("history.sqlite")).unwrap();
    let mut recovery = RenameRecovery::default();
    let failure = recovery
        .rename(
            screenshot(source.clone()),
            "project",
            Err(screenshot_renamer_lib::HistoryStoreError(
                "unavailable".into(),
            )) as Result<&HistoryStore, _>,
        )
        .unwrap_err();
    assert_eq!(failure.category, RenameFailureCategory::HistoryWrite);
    assert!(!failure.filesystem_succeeded);
    assert_eq!(fs::read(source).unwrap(), b"image");
    recovery.retry(&failure.id, None, Ok(&store)).unwrap();
    assert_eq!(
        fs::read(directory.path().join("project.png")).unwrap(),
        b"image"
    );
}

#[cfg(windows)]
#[test]
fn locked_source_reports_permission_io_and_can_retry_after_unlock() {
    use std::os::windows::fs::OpenOptionsExt;
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("Screenshot.png");
    fs::write(&source, b"image").unwrap();
    let store = HistoryStore::open(&directory.path().join("history.sqlite")).unwrap();
    let locked = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&source)
        .unwrap();
    let mut recovery = RenameRecovery::default();
    let failure = recovery
        .rename(screenshot(source.clone()), "project", Ok(&store))
        .unwrap_err();
    assert_eq!(failure.category, RenameFailureCategory::PermissionIo);
    assert!(store.list_recent(10).unwrap().is_empty());
    assert!(source.exists());
    let payload = serde_json::to_value(&failure).unwrap();
    assert_eq!(payload["category"], "permissionIo");
    assert_eq!(payload["sourcePath"], source.to_string_lossy().as_ref());
    assert_eq!(payload["filesystemSucceeded"], false);
    assert_eq!(payload["retryable"], true);
    drop(locked);
    recovery.retry(&failure.id, None, Ok(&store)).unwrap();
    assert_eq!(
        fs::read(directory.path().join("project.png")).unwrap(),
        b"image"
    );
}

#[test]
fn a_failed_move_with_failed_history_write_retries_without_false_success() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("Screenshot.png");
    fs::write(&source, b"image").unwrap();
    let database = directory.path().join("history.sqlite");
    let store = HistoryStore::open(&database).unwrap();
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection.execute_batch("CREATE TRIGGER reject_insert BEFORE INSERT ON entries BEGIN SELECT RAISE(FAIL, 'blocked'); END;").unwrap();
    let mut recovery = RenameRecovery::default();
    let failure = recovery
        .rename(screenshot(source.clone()), "CON", Ok(&store))
        .unwrap_err();
    assert!(!failure.filesystem_succeeded);
    assert!(source.exists());
    connection
        .execute_batch("DROP TRIGGER reject_insert;")
        .unwrap();
    recovery
        .retry(&failure.id, Some("project"), Ok(&store))
        .unwrap();
    let entries = store.list_recent(10).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries
            .iter()
            .filter(|entry| entry.outcome() == &RenameOutcome::Succeeded)
            .count(),
        1
    );
    assert_eq!(
        entries
            .iter()
            .filter(|entry| matches!(entry.outcome(), RenameOutcome::Failed { .. }))
            .count(),
        1
    );
    assert_eq!(
        fs::read(directory.path().join("project.png")).unwrap(),
        b"image"
    );
}

#[test]
fn a_changed_source_is_not_renamed_by_an_old_retry() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("Screenshot.png");
    fs::write(&source, b"image").unwrap();
    let store = HistoryStore::open(&directory.path().join("history.sqlite")).unwrap();
    let mut recovery = RenameRecovery::default();
    let failure = recovery
        .rename(screenshot(source.clone()), "CON", Ok(&store))
        .unwrap_err();
    fs::write(&source, b"other").unwrap();
    assert_eq!(
        recovery
            .retry(&failure.id, Some("project"), Ok(&store))
            .unwrap_err()
            .category,
        RenameFailureCategory::FileChanged
    );
    assert_eq!(fs::read(source).unwrap(), b"other");
    assert!(!directory.path().join("project.png").exists());
    assert!(store
        .list_recent(10)
        .unwrap()
        .iter()
        .all(|entry| matches!(entry.outcome(), RenameOutcome::Failed { .. })));
}

#[test]
fn history_outage_does_not_let_retry_adopt_a_replacement_source() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("Screenshot.png");
    fs::write(&source, b"first").unwrap();
    let store = HistoryStore::open(&directory.path().join("history.sqlite")).unwrap();
    let mut recovery = RenameRecovery::default();
    let unavailable: Result<&HistoryStore, _> = Err(screenshot_renamer_lib::HistoryStoreError(
        "unavailable".into(),
    ));
    let failure = recovery
        .rename(screenshot(source.clone()), "project", unavailable)
        .unwrap_err();
    fs::write(&source, b"other").unwrap();
    assert_eq!(
        recovery
            .retry(&failure.id, None, Ok(&store))
            .unwrap_err()
            .category,
        RenameFailureCategory::FileChanged
    );
    assert_eq!(fs::read(source).unwrap(), b"other");
    assert!(!directory.path().join("project.png").exists());
    assert!(store.list_recent(10).unwrap().is_empty());
}
