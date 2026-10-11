use screenshot_renamer_lib::{
    naming_engine::{name, NamingRules},
    rename_screenshot, sanitize_stem,
    screenshot_metadata::{ScreenshotMetadata, WindowContext},
    ScreenshotFile,
};
fn metadata() -> ScreenshotMetadata {
    let file = ScreenshotFile::new("capture.png".into(), Some(std::time::UNIX_EPOCH)).unwrap();
    let mut metadata = ScreenshotMetadata::from_file(&file);
    metadata.window_context = Some(WindowContext {
        app_name: "Code".into(),
        window_title: Some("main.rs".into()),
    });
    metadata.set_ocr_text(
        "TypeError in main.rs unresolved import E0432".into(),
        "TypeError in main.rs unresolved import E0432".into(),
    );
    metadata
}
#[test]
fn naming_is_deterministic_with_stable_signal_priority() {
    let m = metadata();
    let rules = NamingRules::default();
    assert_eq!(
        name(&m, &rules).unwrap(),
        "code-typeerror-main.rs-unresolved-import-e0432"
    );
    assert_eq!(name(&m, &rules), name(&m, &rules));
    let mut m = m;
    m.window_context = None;
    m.cleaned_ocr_text = None;
    assert_eq!(name(&m, &rules).unwrap(), "screenshot-0");
}
#[test]
fn unicode_reserved_names_and_long_signals_produce_safe_bounded_stems() {
    for text in [
        "CON",
        "NUL.txt",
        "résumé 日本語",
        "TypeError main.rs v1.2.3",
        &"日😀".repeat(300),
    ] {
        let mut m = metadata();
        m.window_context = None;
        m.set_ocr_text(text.into(), text.into());
        let stem = name(&m, &NamingRules::default()).unwrap();
        assert_eq!(sanitize_stem(&stem).unwrap(), stem);
        assert!(stem.encode_utf16().count() <= 120);
    }
    assert!(name(
        &metadata(),
        &NamingRules {
            max_stem_length: 0,
            ..Default::default()
        }
    )
    .is_err());
}
#[test]
fn candidates_integrate_with_safe_rename_and_keep_collision_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Screenshot.PNG");
    std::fs::write(&path, b"original").unwrap();
    let mut file = ScreenshotFile::new(path, Some(std::time::UNIX_EPOCH)).unwrap();
    let m = ScreenshotMetadata::from_file(&file);
    let stem = name(&m, &NamingRules::default()).unwrap();
    std::fs::write(dir.path().join(format!("{stem}.PNG")), b"existing").unwrap();
    let destination = rename_screenshot(&mut file, &stem).unwrap();
    assert_eq!(std::fs::read(destination).unwrap(), b"original");
    assert_eq!(
        std::fs::read(dir.path().join(format!("{stem}.PNG"))).unwrap(),
        b"existing"
    );
}
