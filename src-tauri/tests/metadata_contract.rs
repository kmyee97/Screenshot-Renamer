use screenshot_renamer_lib::{
    screenshot_metadata::{OcrStatus, ScreenshotMetadata, VisualContext, WindowContext},
    ScreenshotFile,
};
use std::{
    path::PathBuf,
    time::{Duration, UNIX_EPOCH},
};

#[test]
fn partial_metadata_preserves_identity_and_distinct_ocr_outcomes() {
    let time = UNIX_EPOCH + Duration::from_millis(12345);
    let file = ScreenshotFile::new(PathBuf::from("capture.png"), Some(time)).unwrap();
    let mut metadata = ScreenshotMetadata::from_file(&file);
    assert_eq!(metadata.captured_at_ms, 12345);
    assert_eq!(metadata.ocr_status, OcrStatus::NotAttempted);
    metadata.set_ocr_text(String::new(), String::new());
    assert_eq!(metadata.ocr_status, OcrStatus::NoText);
    metadata.set_ocr_text("main.rs TypeError".into(), "main.rs TypeError".into());
    metadata.window_context = Some(WindowContext {
        app_name: "Editor".into(),
        window_title: Some("main.rs".into()),
    });
    metadata.visual_context = Some(VisualContext {
        topic: Some("code".into()),
        detail: None,
    });
    let restored: ScreenshotMetadata =
        serde_json::from_str(&serde_json::to_string(&metadata).unwrap()).unwrap();
    assert_eq!(metadata, restored);
    metadata.set_ocr_failure("engine unavailable".into());
    assert!(matches!(metadata.ocr_status, OcrStatus::Failed(_)));
    assert_eq!(metadata.source_path, file.original_path());
    assert_eq!(metadata.captured_at_ms, 12345);
    assert!(metadata.raw_ocr_text.is_none());
}

#[test]
fn caller_supplies_observed_time_when_creation_time_is_unknown() {
    let file = ScreenshotFile::new(PathBuf::from("capture.jpg"), None).unwrap();
    let metadata = ScreenshotMetadata::from_file_at(&file, UNIX_EPOCH + Duration::from_secs(20));
    assert_eq!(metadata.captured_at_ms, 20000);
    assert!(metadata.window_context.is_none());
    assert!(metadata.cleaned_ocr_text.is_none());
}
