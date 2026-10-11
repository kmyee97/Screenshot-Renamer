use crate::vision_provider::{ProviderId, VisionError};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProcessingMode {
    #[default]
    Local,
    Ai,
    Hybrid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct AiSettings {
    pub mode: ProcessingMode,
    pub enabled: bool,
    pub cloud_consent: bool,
    pub provider: ProviderId,
    pub model: String,
}
impl Default for AiSettings {
    fn default() -> Self {
        Self {
            mode: ProcessingMode::Local,
            enabled: false,
            cloud_consent: false,
            provider: ProviderId::OpenAi,
            model: "gpt-4.1-mini".into(),
        }
    }
}
impl AiSettings {
    pub fn validate(&self) -> Result<(), VisionError> {
        if self.model.is_empty()
            || self.model.len() > 80
            || !self
                .model
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-._".contains(&b))
        {
            return Err(VisionError::Configuration);
        }
        if self.mode != ProcessingMode::Local {
            self.authorize_cloud()?;
        }
        Ok(())
    }
    pub fn authorize_cloud(&self) -> Result<(), VisionError> {
        if self.mode == ProcessingMode::Local || !self.enabled || !self.cloud_consent {
            Err(VisionError::Configuration)
        } else {
            Ok(())
        }
    }
}
