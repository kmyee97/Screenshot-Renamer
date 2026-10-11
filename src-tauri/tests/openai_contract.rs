use screenshot_renamer_lib::{
    ai_settings::{AiSettings, ProcessingMode},
    credentials::{CredentialStore, Secret},
    vision_provider::*,
    vision_providers::*,
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
struct Keys;
#[test]
fn revoked_consent_stops_a_pending_retry() {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    struct Revoking {
        allowed: Arc<AtomicBool>,
        count: Arc<AtomicUsize>,
    }
    impl HttpTransport for Revoking {
        fn post(
            &self,
            _: &serde_json::Value,
            _: &Secret,
            _: std::time::Duration,
        ) -> Result<HttpResponse, VisionError> {
            self.count.fetch_add(1, Ordering::SeqCst);
            self.allowed.store(false, Ordering::SeqCst);
            response(503, "unavailable")
        }
    }
    let allowed = Arc::new(AtomicBool::new(true));
    let check = allowed.clone();
    let count = Arc::new(AtomicUsize::new(0));
    let provider = OpenAiProvider::new(
        config(),
        Arc::new(Keys),
        Arc::new(Revoking {
            allowed,
            count: count.clone(),
        }),
        Arc::new(move || check.load(Ordering::SeqCst)),
    )
    .unwrap();
    let request = VisionRequest::new(include_bytes!("fixtures/ocr-text.png").to_vec(), "").unwrap();
    assert_eq!(
        provider.analyze(&request).unwrap_err(),
        VisionError::Configuration
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);
}
impl CredentialStore for Keys {
    fn load(&self) -> Result<Secret, VisionError> {
        Secret::new("test-credential".into())
    }
    fn save(&self, _: Secret) -> Result<(), VisionError> {
        unreachable!()
    }
    fn remove(&self) -> Result<(), VisionError> {
        unreachable!()
    }
}
struct MockHttp {
    replies: Mutex<VecDeque<Result<HttpResponse, VisionError>>>,
    bodies: Mutex<Vec<serde_json::Value>>,
}
impl HttpTransport for MockHttp {
    fn post(
        &self,
        body: &serde_json::Value,
        _: &Secret,
        _: std::time::Duration,
    ) -> Result<HttpResponse, VisionError> {
        self.bodies.lock().unwrap().push(body.clone());
        self.replies.lock().unwrap().pop_front().unwrap()
    }
}
fn config() -> AiSettings {
    AiSettings {
        mode: ProcessingMode::Ai,
        enabled: true,
        cloud_consent: true,
        ..Default::default()
    }
}
fn response(status: u16, body: &str) -> Result<HttpResponse, VisionError> {
    Ok(HttpResponse {
        status,
        body: body.as_bytes().to_vec(),
    })
}
fn valid() -> String {
    serde_json::json!({"status":"completed", "output":[{"type":"message", "content":[{"type":"output_text", "text":r#"{"app":"Browser","topic":"Order receipt","detail":null,"confidence":0.8}"#}]}]}).to_string()
}
fn run(
    replies: Vec<Result<HttpResponse, VisionError>>,
) -> (Result<VisionResult, VisionError>, Arc<MockHttp>) {
    let http = Arc::new(MockHttp {
        replies: Mutex::new(replies.into()),
        bodies: Mutex::new(Vec::new()),
    });
    let provider =
        OpenAiProvider::new(config(), Arc::new(Keys), http.clone(), Arc::new(|| true)).unwrap();
    let request =
        VisionRequest::new(include_bytes!("fixtures/ocr-text.png").to_vec(), "Browser").unwrap();
    (provider.analyze(&request), http)
}
#[test]
fn constrained_response_feeds_naming_without_transport_types() {
    let (result, http) = run(vec![response(200, &valid())]);
    let result = result.unwrap();
    assert_eq!(result.topic.as_deref(), Some("Order receipt"));
    let fixture = tempfile::tempdir().unwrap();
    let file = screenshot_renamer_lib::ScreenshotFile::new(fixture.path().join("source.png"), None)
        .unwrap();
    let mut metadata =
        screenshot_renamer_lib::screenshot_metadata::ScreenshotMetadata::from_file(&file);
    result.apply(&mut metadata);
    let rules = screenshot_renamer_lib::naming_engine::NamingRules {
        template: Some("{app}-{topic}-{detail}".into()),
        ..Default::default()
    };
    assert_eq!(
        screenshot_renamer_lib::naming_engine::name(&metadata, &rules).unwrap(),
        "browser-order-receipt"
    );
    let bodies = http.bodies.lock().unwrap();
    assert_eq!(bodies.len(), 1);
    assert_eq!(bodies[0]["store"], false);
    assert_eq!(bodies[0]["text"]["format"]["strict"], true);
    assert!(bodies[0]["input"][0]["content"][1]["image_url"]
        .as_str()
        .unwrap()
        .starts_with("data:image/png;base64,"));
    assert!(!bodies[0].to_string().contains("test-credential"));
}
#[test]
fn authentication_and_malformed_responses_do_not_retry_or_expose_body() {
    for (status, body, expected) in [
        (401, "secret service detail", VisionError::Authentication),
        (403, "secret", VisionError::Authentication),
        (200, "not json secret", VisionError::InvalidResponse),
        (
            200,
            r#"{"status":"incomplete","output":[]}"#,
            VisionError::InvalidResponse,
        ),
    ] {
        let (result, http) = run(vec![response(status, body)]);
        assert_eq!(result.unwrap_err(), expected);
        assert_eq!(http.bodies.lock().unwrap().len(), 1);
        assert!(!expected.to_string().contains("secret"));
    }
}
#[test]
fn transient_failures_retry_once_and_then_return_typed_errors() {
    for first in [
        response(429, "limit"),
        response(503, "unavailable"),
        Err(VisionError::Timeout),
        Err(VisionError::Network),
    ] {
        let (result, http) = run(vec![first, response(200, &valid())]);
        assert!(result.is_ok());
        assert_eq!(http.bodies.lock().unwrap().len(), 2);
    }
    let (result, http) = run(vec![response(429, "secret"), response(429, "secret")]);
    assert_eq!(result.unwrap_err(), VisionError::RateLimit);
    assert_eq!(http.bodies.lock().unwrap().len(), 2);
}
#[test]
fn local_or_missing_consent_never_constructs_a_cloud_provider() {
    let http = Arc::new(MockHttp {
        replies: Mutex::new(VecDeque::new()),
        bodies: Mutex::new(Vec::new()),
    });
    assert!(OpenAiProvider::new(
        AiSettings::default(),
        Arc::new(Keys),
        http.clone(),
        Arc::new(|| true)
    )
    .is_err());
    let mut settings = config();
    settings.cloud_consent = false;
    assert!(
        OpenAiProvider::new(settings, Arc::new(Keys), http.clone(), Arc::new(|| true)).is_err()
    );
    assert!(http.bodies.lock().unwrap().is_empty());
}

#[test]
#[ignore = "requires explicit opt-in and a saved Windows test credential; uploads only the synthetic fixture"]
fn live_synthetic_windows_vision_check() {
    let registry = configured_registry(
        config(),
        Arc::new(screenshot_renamer_lib::credentials::OsCredentials::default()),
        Arc::new(|| true), // User opt-in for this synthetic fixture, independent of live folder settings.
    )
    .unwrap();
    let request = VisionRequest::new(include_bytes!("fixtures/ocr-text.png").to_vec(), "").unwrap();
    let result = registry.analyze(ProviderId::OpenAi, &request).unwrap();
    assert!(result.topic.as_ref().is_some_and(|s| !s.trim().is_empty()));
}
