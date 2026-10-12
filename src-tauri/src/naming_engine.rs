//! Pure deterministic naming. No OCR, window or AI adapter types enter this API.
use crate::{
    filename_sanitizer::{sanitize_stem, SanitizeError},
    screenshot_metadata::ScreenshotMetadata,
};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct NamingRules {
    pub max_stem_length: usize,
    pub separator: char,
    pub preset: crate::naming_templates::NamingPreset,
    pub template: Option<String>,
}
impl Default for NamingRules {
    fn default() -> Self {
        Self {
            max_stem_length: 120,
            separator: '-',
            preset: Default::default(),
            template: None,
        }
    }
}
impl NamingRules {
    pub fn validate(&self) -> Result<(), NamingError> {
        if !(16..=240).contains(&self.max_stem_length) {
            return Err(NamingError(
                "Stem length must be between 16 and 240 UTF-16 units.".into(),
            ));
        }
        if !matches!(self.separator, '-' | '_') {
            return Err(NamingError("Use a hyphen or underscore separator.".into()));
        }
        if let Some(template) = &self.template {
            crate::naming_templates::validate_template(template)?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamingError(pub String);
impl fmt::Display for NamingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for NamingError {}

pub(crate) struct NamingTokens {
    pub app: String,
    pub topic: String,
    pub detail: String,
    pub timestamp: String,
}
pub(crate) fn tokens(metadata: &ScreenshotMetadata) -> NamingTokens {
    let context = metadata
        .window_context
        .as_ref()
        .filter(|context| !crate::window_context::is_capture_tool(&context.app_name));
    let text = metadata
        .cleaned_ocr_text
        .as_deref()
        .filter(|text| !text.trim().is_empty())
        .or_else(|| context.and_then(|context| context.window_title.as_deref()))
        .unwrap_or("");
    let terms = naming_terms(text);
    NamingTokens {
        app: context
            .map(|context| naming_terms(&context.app_name).join("-"))
            .unwrap_or_default(),
        topic: metadata
            .visual_context
            .as_ref()
            .and_then(|visual| visual.topic.as_ref())
            .map(|topic| naming_terms(topic).join("-"))
            .filter(|topic| !topic.is_empty())
            .unwrap_or_else(|| terms.iter().take(3).cloned().collect::<Vec<_>>().join("-")),
        detail: metadata
            .visual_context
            .as_ref()
            .and_then(|visual| visual.detail.as_ref())
            .map(|detail| naming_terms(detail).join("-"))
            .filter(|detail| !detail.is_empty())
            .unwrap_or_else(|| {
                terms
                    .iter()
                    .skip(3)
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("-")
            }),
        timestamp: metadata.captured_at_ms.to_string(),
    }
}
fn naming_terms(text: &str) -> Vec<String> {
    let normalized: String = text
        .chars()
        .take(4096)
        .flat_map(char::to_lowercase)
        .map(|c| {
            if c.is_alphanumeric() || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect();
    normalized
        .split('-')
        .map(|word| word.trim_matches('.'))
        .filter(|word| {
            !word.is_empty()
                && ![
                    "a", "an", "the", "and", "or", "of", "to", "in", "for", "on", "is", "with",
                ]
                .contains(word)
        })
        .take(12)
        .map(str::to_owned)
        .collect()
}

pub fn name(metadata: &ScreenshotMetadata, rules: &NamingRules) -> Result<String, NamingError> {
    let candidate = crate::naming_templates::render(
        rules.template.as_deref().unwrap_or(rules.preset.template()),
        metadata,
    )?;
    finish_stem(metadata, rules, &candidate)
}

pub(crate) fn finish_stem(
    metadata: &ScreenshotMetadata,
    rules: &NamingRules,
    candidate: &str,
) -> Result<String, NamingError> {
    rules.validate()?;
    // Existing rename service tries suffixes up to " (10000)". Reserve eight units plus dot/extension.
    let extension_units = metadata
        .source_path
        .extension()
        .map(|e| e.to_string_lossy().encode_utf16().count() + 1)
        .unwrap_or(0);
    let limit = rules
        .max_stem_length
        .min(255usize.saturating_sub(extension_units + 8));
    if limit < 16 {
        return Err(NamingError(
            "The source extension leaves insufficient filename space.".into(),
        ));
    }
    let fallback = format!("screenshot-{}", metadata.captured_at_ms);
    let candidate = if candidate.trim_matches(['-', '_', ' ', '.']).is_empty() {
        &fallback
    } else {
        candidate
    };
    let mut separated = String::new();
    for c in candidate.chars().flat_map(char::to_lowercase) {
        if matches!(c, '-' | '_') || c.is_whitespace() {
            if !separated.is_empty() && !separated.ends_with(rules.separator) {
                separated.push(rules.separator);
            }
        } else {
            separated.push(c);
        }
    }
    let sanitized = match sanitize_stem(separated.trim_matches(rules.separator)) {
        Ok(stem) => stem,
        Err(SanitizeError::ReservedDeviceName) => {
            format!("screenshot{}{}", rules.separator, separated)
        }
        Err(error) => return Err(NamingError(error.to_string())),
    };
    let mut units = 0;
    let bounded: String = sanitized
        .chars()
        .take_while(|c| {
            units += c.len_utf16();
            units <= limit
        })
        .collect();
    sanitize_stem(bounded.trim_end_matches([rules.separator, '.', ' ']))
        .map_err(|error| NamingError(error.to_string()))
}
