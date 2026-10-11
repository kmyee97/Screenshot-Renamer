use screenshot_renamer_lib::{
    ai_settings::*,
    settings::{ApplicationSettings, SettingsStore},
};

#[test]
fn defaults_and_legacy_settings_are_local_without_consent() {
    let old: ApplicationSettings =
        serde_json::from_str(r#"{"version":1,"watchedFolder":null,"autoRename":false}"#).unwrap();
    assert_eq!(old.ai.mode, ProcessingMode::Local);
    assert!(!old.ai.cloud_consent);
    assert!(old.ai.authorize_cloud().is_err());
}
#[test]
fn cloud_requires_enablement_and_explicit_consent_and_survives_restart() {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(dir.path().join("settings.json"));
    let mut settings = ApplicationSettings::default();
    settings.ai.mode = ProcessingMode::Hybrid;
    assert!(store.save(&settings).is_err());
    settings.ai.enabled = true;
    assert!(store.save(&settings).is_err());
    settings.ai.cloud_consent = true;
    store.save(&settings).unwrap();
    assert_eq!(store.load().settings.ai, settings.ai);
    assert!(settings.ai.authorize_cloud().is_ok());
    let serialized = serde_json::to_string(&settings).unwrap();
    assert!(!serialized.contains("apiKey"));
    assert!(!serialized.contains("credential"));
    settings.ai.mode = ProcessingMode::Local;
    assert!(settings.ai.authorize_cloud().is_err());
    store.save(&settings).unwrap();
}
#[test]
fn malformed_cloud_settings_recover_to_local() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    std::fs::write(&path, r#"{"version":1,"watchedFolder":null,"autoRename":false,"ai":{"mode":"ai","enabled":true,"cloudConsent":false,"provider":"openAi","model":"gpt-4.1-mini"}}"#).unwrap();
    assert_eq!(
        SettingsStore::new(path).load().settings.ai.mode,
        ProcessingMode::Local
    );
}
