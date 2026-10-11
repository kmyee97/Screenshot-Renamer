use screenshot_renamer_lib::window_context::{capture_with_timeout, normalize_context};
use std::time::Duration;

#[test]
fn context_is_optional_normalized_and_ignores_renamer() {
    let context = normalize_context("Code.exe", " main.rs\n - Workspace ").unwrap();
    assert_eq!(context.app_name, "Code");
    assert_eq!(context.window_title.as_deref(), Some("main.rs - Workspace"));
    assert!(normalize_context("screenshot-renamer.exe", "Settings").is_none());
    assert!(normalize_context("", "Title").is_none());
    assert_eq!(
        normalize_context("notepad.exe", "").unwrap().window_title,
        None
    );
    assert!(
        normalize_context("Browser.exe", &"日".repeat(1000))
            .unwrap()
            .window_title
            .unwrap()
            .chars()
            .count()
            <= 80
    );
}

#[test]
fn metadata_receives_event_time_context_after_readiness() {
    let file = screenshot_renamer_lib::ScreenshotFile::new("capture.png".into(), None)
        .unwrap()
        .with_window_context(normalize_context("notepad.exe", "notes.txt"));
    let metadata =
        screenshot_renamer_lib::screenshot_metadata::ScreenshotMetadata::from_file(&file);
    assert_eq!(
        metadata.window_context.as_ref().unwrap().app_name,
        "notepad"
    );
    assert_eq!(metadata.source_path, file.original_path());
}

#[test]
fn success_permission_failure_vanishing_window_and_timeout_are_nonfatal() {
    assert!(capture_with_timeout(
        || normalize_context("Editor", "main.rs"),
        Duration::from_secs(1)
    )
    .is_some());
    assert!(capture_with_timeout(|| None, Duration::from_secs(1)).is_none());
    assert!(capture_with_timeout(
        || {
            std::thread::sleep(Duration::from_millis(80));
            normalize_context("slow", "title")
        },
        Duration::from_millis(2)
    )
    .is_none());
}

#[cfg(windows)]
#[test]
fn native_lookup_is_bounded_and_context_is_safe_if_available() {
    let start = std::time::Instant::now();
    if let Some(context) = screenshot_renamer_lib::window_context::capture() {
        assert!(!context.app_name.is_empty());
        assert!(context
            .window_title
            .as_ref()
            .is_none_or(|title| title.chars().count() <= 80));
    }
    assert!(start.elapsed() < Duration::from_secs(1));
}
