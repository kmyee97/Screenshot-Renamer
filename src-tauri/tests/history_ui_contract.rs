use screenshot_renamer_lib::history_ui::{recent_history, undo_unavailable_reason, UndoFailure};
use screenshot_renamer_lib::{undo_and_persist, HistoryStore, RenameHistoryEntry, UndoError};
use std::{
    fs,
    time::{Duration, UNIX_EPOCH},
};

fn entry(directory: &std::path::Path, name: &str, time: u64) -> RenameHistoryEntry {
    let mut entry = RenameHistoryEntry::new(
        directory.join(format!("{name}-original.png")),
        UNIX_EPOCH + Duration::from_millis(time),
    )
    .unwrap();
    let destination = directory.join(format!("{name}.png"));
    fs::write(&destination, b"screenshot bytes").unwrap();
    entry.mark_rename_succeeded(destination).unwrap();
    entry
}

#[test]
fn latest_eligible_can_be_older_than_visible_history_and_survives_restart() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("history.sqlite");
    let store = HistoryStore::open(&path).unwrap();
    let oldest = entry(directory.path(), "eligible", 1);
    store.append(&oldest).unwrap();
    for time in 2..=105 {
        let item = entry(directory.path(), &format!("missing-{time}"), time);
        fs::remove_file(item.new_path().unwrap()).unwrap();
        store.append(&item).unwrap();
    }
    drop(store);
    let store = HistoryStore::open(&path).unwrap();
    let snapshot = recent_history(&store, 50).unwrap();
    assert_eq!(snapshot.entries.len(), 50);
    assert!(snapshot.entries.iter().all(|row| !row.can_undo));
    assert_eq!(snapshot.undo_last.unwrap().id, oldest.id());
    undo_and_persist(oldest.id(), &store).unwrap();
    assert!(recent_history(&store, 50).unwrap().undo_last.is_none());
    assert_eq!(
        fs::read(oldest.original_path()).unwrap(),
        b"screenshot bytes"
    );
}

#[test]
fn eligibility_reports_occupied_missing_and_already_undone_without_mutating_files() {
    let directory = tempfile::tempdir().unwrap();
    let mut item = entry(directory.path(), "example", 1);
    assert!(undo_unavailable_reason(&item).is_none());
    fs::write(item.original_path(), b"unrelated bytes").unwrap();
    assert!(undo_unavailable_reason(&item).unwrap().contains("occupied"));
    assert_eq!(fs::read(item.original_path()).unwrap(), b"unrelated bytes");
    fs::remove_file(item.original_path()).unwrap();
    fs::remove_file(item.new_path().unwrap()).unwrap();
    assert!(undo_unavailable_reason(&item).unwrap().contains("missing"));
    item.mark_undo_succeeded(UNIX_EPOCH).unwrap();
    assert!(undo_unavailable_reason(&item)
        .unwrap()
        .contains("already undone"));
}

#[test]
fn command_failure_payload_preserves_category_and_actual_restored_location() {
    let directory = tempfile::tempdir().unwrap();
    let original = directory.path().join("restored.png");
    let failure = UndoFailure::from(UndoError::HistoryWrite {
        actual_path: original.clone(),
        source: screenshot_renamer_lib::HistoryStoreError("disk full".into()),
    });
    let value = serde_json::to_value(failure).unwrap();
    assert_eq!(value["category"], "historyWrite");
    assert_eq!(value["actualPath"], original.to_string_lossy().as_ref());
    assert!(value["message"].as_str().unwrap().contains("disk full"));
    let invalid = UndoFailure::from(UndoError::UnknownHistoryEntry);
    assert_eq!(invalid.category, "unknownId");
}
