use screenshot_renamer_lib::{
    ocr_service::{analyze, enrich_metadata, OcrError, OcrResult},
    screenshot_metadata::{OcrStatus, ScreenshotMetadata},
    ScreenshotFile,
};
use std::path::Path;

#[cfg(windows)]
#[test]
fn native_windows_ocr_reads_dark_google_search_snippet() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/google-search-snippet.png");
    // Watched folders are canonicalized, producing a verbatim Windows path.
    let path = path.canonicalize().unwrap();
    let result = analyze(&path).unwrap();
    let OcrResult::Text(text) = result else {
        panic!("expected search query, got {result:?}")
    };
    let text = text.to_lowercase();
    assert!(text.contains("dead hang"), "OCR missed the query: {text}");
}

#[test]
fn file_errors_are_typed_and_do_not_modify_files() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        analyze(&dir.path().join("missing.png")),
        Err(OcrError::UnreadableFile(_))
    ));
    let path = dir.path().join("capture.gif");
    std::fs::write(&path, b"not an image").unwrap();
    assert_eq!(analyze(&path), Err(OcrError::UnsupportedFormat));
    let path = dir.path().join("broken.png");
    std::fs::write(&path, b"not an image").unwrap();
    assert!(matches!(analyze(&path), Err(OcrError::ProcessingFailed(_))));
    assert_eq!(std::fs::read(&path).unwrap(), b"not an image");
}

#[test]
fn enrichment_keeps_source_identity_for_text_empty_and_errors() {
    let file = ScreenshotFile::new("capture.png".into(), None).unwrap();
    let mut metadata = ScreenshotMetadata::from_file(&file);
    enrich_metadata(
        &mut metadata,
        Ok(OcrResult::Text("File\nmain.rs TypeError".into())),
    );
    assert_eq!(
        metadata.cleaned_ocr_text.as_deref(),
        Some("main.rs TypeError")
    );
    assert_eq!(
        metadata.raw_ocr_text.as_deref(),
        Some("File\nmain.rs TypeError")
    );
    enrich_metadata(&mut metadata, Ok(OcrResult::NoText));
    assert_eq!(metadata.ocr_status, OcrStatus::NoText);
    enrich_metadata(&mut metadata, Err(OcrError::EngineUnavailable));
    assert!(matches!(metadata.ocr_status, OcrStatus::Failed(_)));
    assert_eq!(metadata.source_path, file.original_path());
}

#[cfg(windows)]
#[test]
fn native_windows_ocr_reads_png_jpeg_and_blank_fixtures() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for name in ["ocr-text.png", "ocr-text.jpg"] {
        let path = root.join(name);
        let before = std::fs::read(&path).unwrap();
        let OcrResult::Text(text) =
            analyze(&path).expect("Windows OCR with user language installed")
        else {
            panic!("expected text in {name}")
        };
        assert!(text.to_uppercase().contains("SCREENSHOT"), "{name}: {text}");
        assert_eq!(std::fs::read(path).unwrap(), before);
    }
    assert_eq!(
        analyze(&root.join("ocr-blank.png")).unwrap(),
        OcrResult::NoText
    );
}
