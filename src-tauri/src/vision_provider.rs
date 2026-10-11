//! Stable vision boundary. Requests are transient and deliberately not serializable/debuggable.
use crate::screenshot_metadata::{ScreenshotMetadata, VisualContext, WindowContext};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt};

pub const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderId {
    #[default]
    OpenAi,
}

pub struct VisionRequest {
    bytes: Vec<u8>,
    mime: &'static str,
    context: String,
}
impl VisionRequest {
    pub fn new(bytes: Vec<u8>, context: &str) -> Result<Self, VisionError> {
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err(VisionError::Configuration);
        }
        let mime = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            "image/png"
        } else if bytes.starts_with(b"\xff\xd8\xff") {
            "image/jpeg"
        } else {
            return Err(VisionError::Configuration);
        };
        Ok(Self {
            bytes,
            mime,
            context: context.chars().take(512).collect(),
        })
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn mime(&self) -> &str {
        self.mime
    }
    pub fn context(&self) -> &str {
        &self.context
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisionResult {
    pub app: Option<String>,
    pub topic: Option<String>,
    pub detail: Option<String>,
    pub confidence: Option<f32>,
}
impl VisionResult {
    pub fn validate(&self) -> Result<(), VisionError> {
        let fields = [&self.app, &self.topic, &self.detail];
        if !fields
            .iter()
            .any(|f| f.as_ref().is_some_and(|s| !s.trim().is_empty()))
            || fields.iter().any(|f| {
                f.as_ref()
                    .is_some_and(|s| s.chars().count() > 128 || s.chars().any(char::is_control))
            })
            || self
                .confidence
                .is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
        {
            return Err(VisionError::InvalidResponse);
        }
        Ok(())
    }
    pub fn apply(&self, metadata: &mut ScreenshotMetadata) {
        metadata.visual_context = Some(VisualContext {
            topic: self.topic.clone(),
            detail: self.detail.clone(),
        });
        if let Some(app) = &self.app {
            let window_title = metadata
                .window_context
                .as_ref()
                .and_then(|context| context.window_title.clone());
            metadata.window_context = Some(WindowContext {
                app_name: app.clone(),
                window_title,
            });
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VisionError {
    Configuration,
    Authentication,
    Timeout,
    RateLimit,
    Network,
    InvalidResponse,
}
impl fmt::Display for VisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Configuration => "AI configuration is unavailable. Check consent, provider settings and image format/size.",
            Self::Authentication => "AI authentication failed. Update the saved API key.",
            Self::Timeout => "AI analysis timed out.", Self::RateLimit => "AI provider rate limit reached.",
            Self::Network => "AI provider could not be reached.", Self::InvalidResponse => "AI provider returned invalid naming context.",
        })
    }
}
impl std::error::Error for VisionError {}
pub trait VisionProvider: Send + Sync {
    fn analyze(&self, request: &VisionRequest) -> Result<VisionResult, VisionError>;
}
#[derive(Default)]
pub struct ProviderRegistry {
    providers: HashMap<ProviderId, Box<dyn VisionProvider>>,
}
impl ProviderRegistry {
    pub fn register(&mut self, id: ProviderId, provider: Box<dyn VisionProvider>) {
        self.providers.insert(id, provider);
    }
    pub fn analyze(
        &self,
        id: ProviderId,
        request: &VisionRequest,
    ) -> Result<VisionResult, VisionError> {
        let result = self
            .providers
            .get(&id)
            .ok_or(VisionError::Configuration)?
            .analyze(request)?;
        result.validate()?;
        Ok(result)
    }
}
