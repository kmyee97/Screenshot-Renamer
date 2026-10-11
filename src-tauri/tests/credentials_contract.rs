#[cfg(windows)]
#[test]
fn isolated_windows_credential_survives_reload_and_can_be_removed() {
    use screenshot_renamer_lib::credentials::{CredentialStore, OsCredentials, Secret};
    let target = format!("ScreenshotRenamer/test/{}", uuid::Uuid::new_v4());
    let store = OsCredentials::new(&target);
    assert!(!store.exists().unwrap());
    store
        .save(Secret::new("test-only-fixture".into()).unwrap())
        .unwrap();
    assert_eq!(
        OsCredentials::new(&target).load().unwrap().expose(),
        "test-only-fixture"
    );
    store.remove().unwrap();
    assert!(!store.exists().unwrap());
    assert!(store.load().is_err());
}
