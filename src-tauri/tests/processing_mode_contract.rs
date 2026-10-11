use screenshot_renamer_lib::{
    ai_settings::{AiSettings, ProcessingMode},
    naming_engine::NamingRules,
    processing_mode::*,
    vision_provider::*,
    ScreenshotFile,
};
use std::{cell::Cell, fs};
fn settings(mode: ProcessingMode) -> AiSettings {
    AiSettings {
        mode,
        enabled: true,
        cloud_consent: true,
        ..Default::default()
    }
}
fn fixture() -> (tempfile::TempDir, ScreenshotFile) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.png");
    fs::write(&path, include_bytes!("fixtures/ocr-text.png")).unwrap();
    let file = ScreenshotFile::new(path, Some(std::time::UNIX_EPOCH)).unwrap();
    (dir, file)
}
#[test]
fn local_uses_ocr_and_never_acquires_a_cloud_request() {
    let (_dir, file) = fixture();
    let result = route(
        &file,
        &settings(ProcessingMode::Local),
        &NamingRules::default(),
        |m| {
            m.set_ocr_text("Order receipt".into(), "Order receipt".into());
            None
        },
        |_| panic!("Local must not request cloud data"),
    )
    .unwrap();
    assert_eq!(result.source, AnalysisSource::Local);
    assert!(result.candidate.contains("order-receipt"));
}
#[test]
fn ai_calls_vision_once_and_skips_ocr() {
    let (_dir, file) = fixture();
    let calls = Cell::new(0);
    let result = route(
        &file,
        &settings(ProcessingMode::Ai),
        &NamingRules::default(),
        |_| panic!("AI should use vision"),
        |_| {
            calls.set(calls.get() + 1);
            Ok(VisionResult {
                topic: Some("Invoice".into()),
                ..Default::default()
            })
        },
    )
    .unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(result.source, AnalysisSource::Ai);
    assert!(result.candidate.contains("invoice"));
}
#[test]
fn hybrid_escalates_only_without_useful_topic_and_falls_back_on_failure() {
    let (_dir, file) = fixture();
    let calls = Cell::new(0);
    let result = route(
        &file,
        &settings(ProcessingMode::Hybrid),
        &NamingRules::default(),
        |m| {
            m.set_ocr_text("Order receipt".into(), "Order receipt".into());
            None
        },
        |_| panic!("useful local text must not upload"),
    )
    .unwrap();
    assert_eq!(result.source, AnalysisSource::Local);
    let result = route(
        &file,
        &settings(ProcessingMode::Hybrid),
        &NamingRules::default(),
        |_| None,
        |_| {
            calls.set(calls.get() + 1);
            Err(VisionError::Network)
        },
    )
    .unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(result.source, AnalysisSource::Local);
    assert_eq!(result.candidate, "screenshot-0");
    assert!(result.warning.is_some());
    assert!(result.completion_message().contains("could not be reached"));
    assert!(file.original_path().exists());
}
#[test]
fn ai_failure_preserves_source_and_missing_consent_never_reads_for_cloud() {
    let (_dir, file) = fixture();
    let before = fs::read(file.original_path()).unwrap();
    assert!(route(
        &file,
        &settings(ProcessingMode::Ai),
        &NamingRules::default(),
        |_| None,
        |_| Err(VisionError::Timeout)
    )
    .is_err());
    assert_eq!(fs::read(file.original_path()).unwrap(), before);
    let mut config = settings(ProcessingMode::Ai);
    config.cloud_consent = false;
    assert!(route(
        &file,
        &config,
        &NamingRules::default(),
        |_| None,
        |_| panic!("missing consent must not read image bytes")
    )
    .is_err());
}
#[test]
fn failed_analysis_retry_is_reserved_once_and_rejects_a_replaced_source() {
    let (_dir, file) = fixture();
    let mut recovery = AnalysisRecovery::default();
    let failure = recovery.record(file.clone(), VisionError::Network, None, None);
    let pending = recovery.reserve(&failure.id).unwrap();
    assert!(recovery.reserve(&failure.id).is_err());
    assert!(pending.verify().is_ok());
    fs::write(file.original_path(), b"replaced").unwrap();
    assert!(pending.verify().is_err());
    let failure = recovery.record(file, VisionError::Network, Some(pending), None);
    assert!(!failure.retryable);
}

#[test]
fn modes_and_failure_fallback_share_collision_safe_history_pipeline() {
    use screenshot_renamer_lib::{HistoryStore, RenameRecovery};
    for (mode, useful_local, provider_fails, expected_source) in [
        (ProcessingMode::Local, true, false, AnalysisSource::Local),
        (ProcessingMode::Ai, false, false, AnalysisSource::Ai),
        (ProcessingMode::Hybrid, false, false, AnalysisSource::Ai),
        (ProcessingMode::Hybrid, false, true, AnalysisSource::Local),
    ] {
        let (dir, file) = fixture();
        let original = fs::read(file.original_path()).unwrap();
        let result = route(
            &file,
            &settings(mode),
            &NamingRules::default(),
            |m| {
                if useful_local {
                    m.set_ocr_text("Order receipt".into(), "Order receipt".into());
                }
                None
            },
            |_| {
                if provider_fails {
                    Err(VisionError::Timeout)
                } else {
                    Ok(VisionResult {
                        topic: Some("Order receipt".into()),
                        ..Default::default()
                    })
                }
            },
        )
        .unwrap();
        assert_eq!(result.source, expected_source);
        let collision = dir.path().join(format!("{}.png", result.candidate));
        fs::write(&collision, b"existing").unwrap();
        let store = HistoryStore::open(&dir.path().join("history.sqlite")).unwrap();
        let mut recovery = RenameRecovery::default();
        let entry = recovery
            .rename(file.clone(), &result.candidate, Ok(&store))
            .unwrap();
        assert_eq!(fs::read(entry.new_path.unwrap()).unwrap(), original);
        assert_eq!(fs::read(collision).unwrap(), b"existing");
        assert_eq!(store.list_recent_successful(10).unwrap().len(), 1);
        assert!(recovery
            .rename(file, &result.candidate, Ok(&store))
            .is_err());
        assert_eq!(store.list_recent_successful(10).unwrap().len(), 1);
    }
}

#[test]
fn hybrid_escalates_when_a_template_would_produce_generic_fallback() {
    let (_dir, file) = fixture();
    let rules = NamingRules {
        template: Some("{detail}".into()),
        separator: '_',
        ..Default::default()
    };
    let result = route(
        &file,
        &settings(ProcessingMode::Hybrid),
        &rules,
        |m| {
            m.set_ocr_text("Receipt".into(), "Receipt".into());
            None
        },
        |_| {
            Ok(VisionResult {
                detail: Some("Paid".into()),
                ..Default::default()
            })
        },
    )
    .unwrap();
    assert_eq!(result.source, AnalysisSource::Ai);
    assert_eq!(result.candidate, "paid");
}

#[test]
fn saved_mode_changes_preserve_starting_snapshot_and_revoke_cloud_escalation() {
    use screenshot_renamer_lib::{
        settings::{ApplicationSettings, SettingsStore},
        settings_controller::{SettingsController, WatchSession},
    };
    struct Watch;
    impl WatchSession for Watch {
        fn stop(&mut self) -> Result<(), String> {
            Ok(())
        }
    }
    let (dir, file) = fixture();
    let mut controller =
        SettingsController::<Watch>::new(Ok(SettingsStore::new(dir.path().join("settings.json"))));
    let initial = ApplicationSettings {
        ai: settings(ProcessingMode::Hybrid),
        ..Default::default()
    };
    controller.apply(initial, |_, _, _| Ok(Watch)).unwrap();
    let shared = controller.processing_settings();
    let snapshot = shared.lock().unwrap().clone();
    controller
        .apply(ApplicationSettings::default(), |_, _, _| Ok(Watch))
        .unwrap();
    assert_eq!(snapshot.ai.mode, ProcessingMode::Hybrid);
    assert_eq!(shared.lock().unwrap().ai.mode, ProcessingMode::Local);
    let result = route(
        &file,
        &snapshot.ai,
        &snapshot.naming,
        |_| None,
        |_| {
            shared.lock().unwrap().ai.authorize_cloud()?;
            panic!("revoked mode must never upload")
        },
    )
    .unwrap();
    assert_eq!(result.source, AnalysisSource::Local);
    assert!(result.warning.is_some());
    let queued = shared.lock().unwrap().clone();
    assert_eq!(
        route(
            &file,
            &queued.ai,
            &queued.naming,
            |_| None,
            |_| panic!("queued Local must not upload")
        )
        .unwrap()
        .source,
        AnalysisSource::Local
    );
}
