//! Provider-independent signals used by naming. Never stores screenshot bytes.
use crate::ScreenshotFile;
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OcrStatus {
    NotAttempted,
    TextFound,
    NoText,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowContext {
    pub app_name: String,
    pub window_title: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualContext {
    pub topic: Option<String>,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotMetadata {
    pub source_path: PathBuf,
    pub captured_at_ms: u64,
    pub raw_ocr_text: Option<String>,
    pub cleaned_ocr_text: Option<String>,
    pub ocr_status: OcrStatus,
    pub window_context: Option<WindowContext>,
    pub visual_context: Option<VisualContext>,
}

impl ScreenshotMetadata {
    pub fn from_file(file: &ScreenshotFile) -> Self {
        Self::from_file_at(file, SystemTime::now())
    }
    pub fn from_file_at(file: &ScreenshotFile, observed: SystemTime) -> Self {
        Self {
            source_path: file.original_path().to_owned(),
            captured_at_ms: file
                .created_at()
                .copied()
                .unwrap_or(observed)
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                .min(u64::MAX as u128) as u64,
            raw_ocr_text: None,
            cleaned_ocr_text: None,
            ocr_status: OcrStatus::NotAttempted,
            window_context: file.window_context().cloned(),
            visual_context: None,
        }
    }
    pub fn set_ocr_text(&mut self, raw: String, cleaned: String) {
        self.ocr_status = if raw.trim().is_empty() {
            OcrStatus::NoText
        } else {
            OcrStatus::TextFound
        };
        self.raw_ocr_text = Some(raw);
        self.cleaned_ocr_text = Some(cleaned);
    }
    pub fn set_ocr_failure(&mut self, message: String) {
        self.ocr_status = OcrStatus::Failed(message);
        self.raw_ocr_text = None;
        self.cleaned_ocr_text = None;
    }
}
