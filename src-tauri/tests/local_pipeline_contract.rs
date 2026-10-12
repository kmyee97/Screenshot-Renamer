use screenshot_renamer_lib::{
    naming_engine::{name, NamingRules},
    naming_templates::NamingPreset,
    ocr_service::{analyze, enrich_metadata},
    rename_screenshot,
    screenshot_metadata::ScreenshotMetadata,
    FolderWatcher, SuppressedPaths,
};
use std::{
    sync::{mpsc, Arc},
    time::Duration,
};

#[cfg(windows)]
#[test]
fn canonical_watched_folder_names_google_snippet_from_its_content() {
    use screenshot_renamer_lib::{screenshot_metadata::WindowContext, ScreenshotFile};
    let dir = tempfile::tempdir().unwrap();
    let suppressed = Arc::new(SuppressedPaths::default());
    let callback_suppression = suppressed.clone();
    let (tx, rx) = mpsc::channel();
    let mut watcher = FolderWatcher::start_with_suppression(
        screenshot_renamer_lib::settings::validate_folder(dir.path().to_str().unwrap()).unwrap(),
        move |file| {
            // Reproduce saving while Snipping Tool is in the foreground.
            let mut file: ScreenshotFile = file.with_window_context(Some(WindowContext {
                app_name: "SnippingTool".into(),
                window_title: Some("Snipping Tool".into()),
            }));
            let mut metadata = ScreenshotMetadata::from_file(&file);
            enrich_metadata(&mut metadata, analyze(file.original_path()));
            let stem = name(&metadata, &NamingRules::default()).unwrap();
            let result = rename_screenshot(&mut file, &stem).unwrap();
            callback_suppression.suppress(&result);
            tx.send(result).unwrap();
        },
        suppressed,
    )
    .unwrap();
    let bytes = include_bytes!("fixtures/google-search-snippet.png");
    std::fs::write(dir.path().join("Screenshot.png"), bytes).unwrap();
    let destination = rx.recv_timeout(Duration::from_secs(10)).unwrap();
    watcher.stop().unwrap();
    assert_eq!(
        destination.file_name().unwrap(),
        "google-if-i-dead-hang-every.png"
    );
    assert_eq!(std::fs::read(destination).unwrap(), bytes);
}

#[cfg(windows)]
#[test]
fn ready_png_and_jpeg_are_locally_analyzed_named_and_renamed_without_byte_changes() {
    let dir = tempfile::tempdir().unwrap();
    let suppressed = Arc::new(SuppressedPaths::default());
    let callback_suppression = suppressed.clone();
    let rules = NamingRules {
        preset: NamingPreset::Minimal,
        ..Default::default()
    };
    let (tx, rx) = mpsc::channel();
    let mut watcher = FolderWatcher::start_with_suppression(
        dir.path().into(),
        move |mut file| {
            let mut metadata = ScreenshotMetadata::from_file(&file);
            enrich_metadata(&mut metadata, analyze(file.original_path()));
            let stem = name(&metadata, &rules).unwrap();
            let result = rename_screenshot(&mut file, &stem).unwrap();
            callback_suppression.suppress(&result);
            tx.send(result).unwrap();
        },
        suppressed,
    )
    .unwrap();
    for extension in ["png", "jpg"] {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("tests/fixtures/ocr-text.{extension}"));
        let bytes = std::fs::read(fixture).unwrap();
        std::fs::write(dir.path().join(format!("Screenshot.{extension}")), &bytes).unwrap();
        let destination = rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(destination.extension().unwrap(), extension);
        assert!(destination
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .contains("screenshot-order-12345"));
        assert_eq!(std::fs::read(destination).unwrap(), bytes);
    }
    watcher.stop().unwrap();
}

#[test]
fn analysis_failure_still_names_and_invalid_rules_preserve_source() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("corrupt.png");
    std::fs::write(&path, b"corrupt").unwrap();
    let mut file =
        screenshot_renamer_lib::ScreenshotFile::new(path.clone(), Some(std::time::UNIX_EPOCH))
            .unwrap();
    let mut metadata = ScreenshotMetadata::from_file(&file);
    enrich_metadata(&mut metadata, analyze(&path));
    assert!(name(
        &metadata,
        &NamingRules {
            template: Some("{unknown}".into()),
            ..Default::default()
        }
    )
    .is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"corrupt");
    let stem = name(&metadata, &NamingRules::default()).unwrap();
    let destination = rename_screenshot(&mut file, &stem).unwrap();
    assert_eq!(destination.file_name().unwrap(), "screenshot-0.png");
    assert_eq!(std::fs::read(destination).unwrap(), b"corrupt");
}
