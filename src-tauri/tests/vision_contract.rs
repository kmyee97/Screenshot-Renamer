use screenshot_renamer_lib::{
    screenshot_metadata::ScreenshotMetadata, vision_provider::*, ScreenshotFile,
};

#[test]
fn bounded_request_rejects_unsupported_or_oversized_images() {
    assert_eq!(
        VisionRequest::new(vec![0; MAX_IMAGE_BYTES + 1], "").err(),
        Some(VisionError::Configuration)
    );
    assert_eq!(
        VisionRequest::new(b"secret".to_vec(), "").err(),
        Some(VisionError::Configuration)
    );
    let request =
        VisionRequest::new(b"\x89PNG\r\n\x1a\nfixture".to_vec(), &"x".repeat(1000)).unwrap();
    assert_eq!(request.context().chars().count(), 512);
}

struct Fake(Result<VisionResult, VisionError>);
#[test]
fn app_only_result_preserves_the_local_title_for_naming() {
    let file = ScreenshotFile::new(std::env::temp_dir().join("fixture.png"), None).unwrap();
    let mut metadata = ScreenshotMetadata::from_file(&file);
    metadata.window_context = Some(screenshot_renamer_lib::screenshot_metadata::WindowContext {
        app_name: "browser.exe".into(),
        window_title: Some("Order receipt".into()),
    });
    VisionResult {
        app: Some("Browser".into()),
        ..Default::default()
    }
    .apply(&mut metadata);
    assert_eq!(
        metadata.window_context.unwrap().window_title.as_deref(),
        Some("Order receipt")
    );
}
impl VisionProvider for Fake {
    fn analyze(&self, _: &VisionRequest) -> Result<VisionResult, VisionError> {
        self.0.clone()
    }
}
#[test]
fn registry_validates_partial_results_and_enriches_common_metadata() {
    let result = VisionResult {
        app: Some("Browser".into()),
        topic: Some("Order receipt".into()),
        detail: None,
        confidence: Some(0.8),
    };
    let mut registry = ProviderRegistry::default();
    registry.register(ProviderId::OpenAi, Box::new(Fake(Ok(result.clone()))));
    let request = VisionRequest::new(b"\xff\xd8\xfffixture".to_vec(), "").unwrap();
    let result = registry.analyze(ProviderId::OpenAi, &request).unwrap();
    let file = ScreenshotFile::new(std::env::temp_dir().join("fixture.png"), None).unwrap();
    let mut metadata = ScreenshotMetadata::from_file(&file);
    result.apply(&mut metadata);
    assert_eq!(
        metadata.visual_context.unwrap().topic.as_deref(),
        Some("Order receipt")
    );
    assert_eq!(metadata.window_context.unwrap().app_name, "Browser");
}
#[test]
fn malformed_data_and_typed_errors_stay_at_the_provider_boundary() {
    let request = VisionRequest::new(b"\xff\xd8\xfffixture".to_vec(), "").unwrap();
    for error in [
        VisionError::Timeout,
        VisionError::Authentication,
        VisionError::RateLimit,
        VisionError::Network,
    ] {
        let mut registry = ProviderRegistry::default();
        registry.register(ProviderId::OpenAi, Box::new(Fake(Err(error))));
        assert_eq!(
            registry.analyze(ProviderId::OpenAi, &request).unwrap_err(),
            error
        );
    }
    for result in [
        VisionResult::default(),
        VisionResult {
            topic: Some("x".repeat(129)),
            ..Default::default()
        },
        VisionResult {
            topic: Some("topic".into()),
            confidence: Some(f32::NAN),
            ..Default::default()
        },
    ] {
        assert_eq!(result.validate(), Err(VisionError::InvalidResponse));
    }
    assert!(!serde_json::to_string(&VisionError::Authentication)
        .unwrap()
        .contains("secret"));
}
