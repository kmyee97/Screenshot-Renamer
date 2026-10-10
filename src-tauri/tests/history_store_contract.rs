use std::{
    fs,
    path::PathBuf,
    time::{Duration, UNIX_EPOCH},
};

use screenshot_renamer_lib::{
    rename_screenshot_and_persist, HistoryStore, PersistedRenameError, RenameHistoryEntry,
    RenameOutcome, ScreenshotFile, UndoStatus,
};

fn successful_entry(name: &str, at_ms: u64) -> RenameHistoryEntry {
    let mut entry = RenameHistoryEntry::new(
        PathBuf::from(format!("{name}.png")),
        UNIX_EPOCH + Duration::from_millis(at_ms),
    )
    .unwrap();
    entry
        .mark_rename_succeeded(PathBuf::from(format!("renamed-{name}.png")))
        .unwrap();
    entry
}

#[test]
fn entries_and_undo_outcomes_survive_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("history.sqlite");
    let mut entry = successful_entry("first", 1234);
    {
        let store = HistoryStore::open(&path).unwrap();
        store.append(&entry).unwrap();
        entry
            .mark_undo_succeeded(UNIX_EPOCH + Duration::from_secs(3))
            .unwrap();
        store.update_undo_outcome(&entry).unwrap();
    }
    let reopened = HistoryStore::open(&path).unwrap();
    let loaded = reopened.get(entry.id()).unwrap().unwrap();
    assert_eq!(loaded, entry);
    assert_eq!(loaded.undo_status(), &UndoStatus::Succeeded { at_ms: 3000 });
}

#[test]
fn recent_entries_are_newest_first_and_bounded() {
    let directory = tempfile::tempdir().unwrap();
    let store = HistoryStore::open(&directory.path().join("history.sqlite")).unwrap();
    for (name, time) in [("old", 100), ("new", 300), ("middle", 200)] {
        store.append(&successful_entry(name, time)).unwrap();
    }
    let recent = store.list_recent(2).unwrap();
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0].original_path(), PathBuf::from("new.png"));
    assert_eq!(recent[1].original_path(), PathBuf::from("middle.png"));
}

#[test]
fn opening_an_old_empty_database_migrates_without_losing_it() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("history.sqlite");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.pragma_update(None, "user_version", 0).unwrap();
    drop(connection);
    let store = HistoryStore::open(&path).unwrap();
    store.append(&successful_entry("migrated", 1)).unwrap();
    assert_eq!(store.list_recent(10).unwrap().len(), 1);
    assert!(fs::metadata(path).unwrap().len() > 0);
}

#[test]
fn malformed_rows_do_not_block_recent_history() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("history.sqlite");
    let store = HistoryStore::open(&path).unwrap();
    store.append(&successful_entry("valid", 1)).unwrap();
    drop(store);
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "INSERT INTO entries (id, attempted_at_ms, payload) VALUES ('broken', 2, 'not json')",
            [],
        )
        .unwrap();
    drop(connection);
    let reopened = HistoryStore::open(&path).unwrap();
    let recent = reopened.list_recent(10).unwrap();
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].outcome(), &RenameOutcome::Succeeded);
}

#[test]
fn successful_move_is_stored_before_it_is_returned() {
    let directory = tempfile::tempdir().unwrap();
    let original = directory.path().join("Screenshot.png");
    fs::write(&original, b"image").unwrap();
    let store = HistoryStore::open(&directory.path().join("history.sqlite")).unwrap();
    let mut screenshot = ScreenshotFile::new(original, None).unwrap();
    let view = rename_screenshot_and_persist(&mut screenshot, "project", &store).unwrap();
    let saved = store.get(&view.id).unwrap().unwrap();
    assert_eq!(
        saved.new_path(),
        Some(directory.path().join("project.png").as_path())
    );
}

#[test]
fn storage_write_failure_reports_real_location_without_losing_file() {
    let directory = tempfile::tempdir().unwrap();
    let original = directory.path().join("Screenshot.png");
    let destination = directory.path().join("project.png");
    fs::write(&original, b"image bytes").unwrap();
    let db_path = directory.path().join("history.sqlite");
    let store = HistoryStore::open(&db_path).unwrap();
    let connection = rusqlite::Connection::open(&db_path).unwrap();
    connection.execute_batch("CREATE TRIGGER reject_insert BEFORE INSERT ON entries BEGIN SELECT RAISE(FAIL, 'write blocked'); END;").unwrap();
    drop(connection);
    let mut screenshot = ScreenshotFile::new(original.clone(), None).unwrap();
    let error = rename_screenshot_and_persist(&mut screenshot, "project", &store).unwrap_err();
    assert!(matches!(error, PersistedRenameError::Storage { .. }));
    assert_eq!(error.actual_path(), destination);
    assert!(!original.exists());
    assert_eq!(fs::read(destination).unwrap(), b"image bytes");
}
