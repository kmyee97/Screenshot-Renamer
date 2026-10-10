use std::{fs, path::PathBuf, time::UNIX_EPOCH};

use screenshot_renamer_lib::{
    undo_and_persist, undo_rename, HistoryStore, RenameHistoryEntry, UndoError, UndoStatus,
};

fn renamed_entry(original: PathBuf, renamed: PathBuf) -> RenameHistoryEntry {
    let mut entry = RenameHistoryEntry::new(original, UNIX_EPOCH).unwrap();
    entry.mark_rename_succeeded(renamed).unwrap();
    entry
}

#[test]
fn undo_restores_original_name_and_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let original = directory.path().join("Screenshot.png");
    let renamed = directory.path().join("project.png");
    fs::write(&renamed, b"original bytes").unwrap();
    let mut entry = renamed_entry(original.clone(), renamed.clone());
    assert_eq!(undo_rename(&mut entry).unwrap(), original);
    assert_eq!(fs::read(&original).unwrap(), b"original bytes");
    assert!(!renamed.exists());
    assert!(matches!(entry.undo_status(), UndoStatus::Succeeded { .. }));
}

#[test]
fn occupied_original_does_not_overwrite_either_file() {
    let directory = tempfile::tempdir().unwrap();
    let original = directory.path().join("Screenshot.png");
    let renamed = directory.path().join("project.png");
    fs::write(&original, b"other file").unwrap();
    fs::write(&renamed, b"screenshot").unwrap();
    let mut entry = renamed_entry(original.clone(), renamed.clone());
    assert!(matches!(
        undo_rename(&mut entry),
        Err(UndoError::OriginalOccupied { .. })
    ));
    assert_eq!(fs::read(original).unwrap(), b"other file");
    assert_eq!(fs::read(renamed).unwrap(), b"screenshot");
    assert!(matches!(entry.undo_status(), UndoStatus::Failed { .. }));
}

#[test]
fn missing_source_and_repeated_undo_are_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let original = directory.path().join("Screenshot.png");
    let renamed = directory.path().join("project.png");
    let mut entry = renamed_entry(original.clone(), renamed.clone());
    assert!(matches!(
        undo_rename(&mut entry),
        Err(UndoError::RenamedSourceMissing { .. })
    ));
    fs::write(&renamed, b"image").unwrap();
    undo_rename(&mut entry).unwrap();
    assert!(matches!(
        undo_rename(&mut entry),
        Err(UndoError::AlreadyUndone)
    ));
    assert_eq!(fs::read(original).unwrap(), b"image");
}

#[test]
fn no_op_rename_can_be_undone_without_moving_the_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("Screenshot.png");
    fs::write(&path, b"image").unwrap();
    let mut entry = renamed_entry(path.clone(), path.clone());
    assert_eq!(undo_rename(&mut entry).unwrap(), path);
    assert_eq!(fs::read(path).unwrap(), b"image");
    assert!(matches!(entry.undo_status(), UndoStatus::Succeeded { .. }));
}

#[test]
fn undo_updates_the_original_stored_entry() {
    let directory = tempfile::tempdir().unwrap();
    let original = directory.path().join("Screenshot.png");
    let renamed = directory.path().join("project.png");
    fs::write(&renamed, b"image").unwrap();
    let store = HistoryStore::open(&directory.path().join("history.sqlite")).unwrap();
    let entry = renamed_entry(original.clone(), renamed);
    let id = entry.id().to_owned();
    store.append(&entry).unwrap();
    let view = undo_and_persist(&id, &store).unwrap();
    assert_eq!(view.id, id);
    assert!(matches!(
        store.get(&id).unwrap().unwrap().undo_status(),
        UndoStatus::Succeeded { .. }
    ));
}

#[test]
fn history_write_failure_reports_where_the_restored_file_is() {
    let directory = tempfile::tempdir().unwrap();
    let original = directory.path().join("Screenshot.png");
    let renamed = directory.path().join("project.png");
    fs::write(&renamed, b"image").unwrap();
    let db_path = directory.path().join("history.sqlite");
    let store = HistoryStore::open(&db_path).unwrap();
    let entry = renamed_entry(original.clone(), renamed.clone());
    let id = entry.id().to_owned();
    store.append(&entry).unwrap();
    let connection = rusqlite::Connection::open(&db_path).unwrap();
    connection.execute_batch("CREATE TRIGGER reject_update BEFORE UPDATE ON entries BEGIN SELECT RAISE(FAIL, 'write blocked'); END;").unwrap();
    drop(connection);

    let error = undo_and_persist(&id, &store).unwrap_err();
    assert!(matches!(error, UndoError::HistoryWrite { .. }));
    assert_eq!(fs::read(&original).unwrap(), b"image");
    assert!(!renamed.exists());
    assert!(matches!(
        store.get(&id).unwrap().unwrap().undo_status(),
        UndoStatus::NotAttempted
    ));
}
