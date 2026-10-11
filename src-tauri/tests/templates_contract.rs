use screenshot_renamer_lib::{
    naming_engine::{name, NamingRules},
    naming_templates::{validate_template, NamingPreset},
    screenshot_metadata::{ScreenshotMetadata, WindowContext},
    settings::{ApplicationSettings, SettingsStore},
    ScreenshotFile,
};
fn metadata() -> ScreenshotMetadata {
    let file = ScreenshotFile::new("capture.png".into(), Some(std::time::UNIX_EPOCH)).unwrap();
    let mut m = ScreenshotMetadata::from_file(&file);
    m.window_context = Some(WindowContext {
        app_name: "Code".into(),
        window_title: None,
    });
    m.set_ocr_text(
        "main.rs TypeError import E0432".into(),
        "main.rs TypeError import E0432".into(),
    );
    m
}
#[test]
fn five_presets_render_the_same_local_metadata() {
    for (preset, expected) in [
        (
            NamingPreset::Descriptive,
            "code-main.rs-typeerror-import-e0432",
        ),
        (NamingPreset::Minimal, "main.rs-typeerror-import"),
        (NamingPreset::Coding, "code-main.rs-typeerror-import"),
        (
            NamingPreset::WorkResearch,
            "code-main.rs-typeerror-import-0",
        ),
        (
            NamingPreset::ReceiptsOrders,
            "main.rs-typeerror-import-e0432-0",
        ),
    ] {
        assert_eq!(
            name(
                &metadata(),
                &NamingRules {
                    preset,
                    ..Default::default()
                }
            )
            .unwrap(),
            expected
        );
    }
}
#[test]
fn templates_handle_missing_fields_escaped_braces_unknown_and_unclosed_tokens() {
    for template in ["{unknown}", "{app", "app}", "", "{APP}"] {
        assert!(validate_template(template).is_err(), "{template}");
    }
    let mut m = metadata();
    m.window_context = None;
    let rules = NamingRules {
        template: Some("{{shot}}-{app}-{topic}-{detail}".into()),
        ..Default::default()
    };
    assert_eq!(
        name(&m, &rules).unwrap(),
        "{shot}-main.rs-typeerror-import-e0432"
    );
    m.cleaned_ocr_text = None;
    assert_eq!(
        name(
            &m,
            &NamingRules {
                template: Some("{app}-{topic}-{detail}".into()),
                ..Default::default()
            }
        )
        .unwrap(),
        "screenshot-0"
    );
    assert!(name(
        &metadata(),
        &NamingRules {
            template: Some("{unknown}".into()),
            ..Default::default()
        }
    )
    .is_err());
    let stem = name(
        &metadata(),
        &NamingRules {
            template: Some("日".repeat(300)),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(stem.encode_utf16().count() <= 120);
}
#[test]
fn legacy_settings_gain_defaults_and_invalid_templates_never_replace_saved_choice() {
    let old: ApplicationSettings =
        serde_json::from_str(r#"{"version":1,"watchedFolder":null,"autoRename":false}"#).unwrap();
    assert_eq!(old.naming, NamingRules::default());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let store = SettingsStore::new(path.clone());
    let good = ApplicationSettings {
        naming: NamingRules {
            preset: NamingPreset::Minimal,
            template: Some("{topic}-{timestamp}".into()),
            ..Default::default()
        },
        ..old
    };
    store.save(&good).unwrap();
    let bad = ApplicationSettings {
        naming: NamingRules {
            template: Some("{secret}".into()),
            ..Default::default()
        },
        ..good.clone()
    };
    assert!(store.save(&bad).is_err());
    assert_eq!(SettingsStore::new(path).load().settings, good);
}
