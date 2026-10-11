//! Mode routing and session-only recovery. Every mode uses the same naming engine.
use crate::{
    ai_settings::{AiSettings, ProcessingMode},
    naming_engine::{self, NamingRules},
    screenshot_metadata::ScreenshotMetadata,
    vision_provider::{VisionError, VisionResult, MAX_IMAGE_BYTES},
    ScreenshotFile,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs::File, io::Read};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AnalysisSource {
    Local,
    Ai,
}
pub struct ProcessingResult {
    pub candidate: String,
    pub source: AnalysisSource,
    pub warning: Option<String>,
}
impl ProcessingResult {
    pub fn completion_message(&self) -> String {
        let source = match self.source {
            AnalysisSource::Local => "local",
            AnalysisSource::Ai => "AI",
        };
        match &self.warning {
            Some(warning) => format!("Renamed using {source} analysis. {warning}"),
            None => format!("Renamed using {source} analysis."),
        }
    }
}
#[derive(Debug)]
pub enum ProcessingError {
    Vision(VisionError),
    Naming(naming_engine::NamingError),
}
impl std::fmt::Display for ProcessingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Vision(e) => e.fmt(f),
            Self::Naming(e) => e.fmt(f),
        }
    }
}
pub fn route(
    file: &ScreenshotFile,
    settings: &AiSettings,
    rules: &NamingRules,
    local: impl FnOnce(&mut ScreenshotMetadata) -> Option<String>,
    cloud: impl FnOnce(&ScreenshotMetadata) -> Result<VisionResult, VisionError>,
) -> Result<ProcessingResult, ProcessingError> {
    let mut metadata = ScreenshotMetadata::from_file(file);
    let mut warning = None;
    let mut source = AnalysisSource::Local;
    if settings.mode != ProcessingMode::Ai {
        warning = local(&mut metadata);
    }
    let local_candidate = if settings.mode != ProcessingMode::Ai {
        Some(naming_engine::name(&metadata, rules).map_err(ProcessingError::Naming)?)
    } else {
        None
    };
    let fallback =
        naming_engine::finish_stem(&metadata, rules, "").map_err(ProcessingError::Naming)?;
    let generic = local_candidate
        .as_ref()
        .is_some_and(|name| name == &fallback);
    let needs_cloud = settings.mode == ProcessingMode::Ai
        || (settings.mode == ProcessingMode::Hybrid
            && (naming_engine::tokens(&metadata).topic.is_empty() || generic));
    if needs_cloud {
        let result = settings
            .authorize_cloud()
            .and_then(|_| cloud(&metadata))
            .and_then(|result| {
                result.validate()?;
                Ok(result)
            });
        match result {
            Ok(result) => {
                result.apply(&mut metadata);
                source = AnalysisSource::Ai;
                warning = None;
            }
            Err(error) if settings.mode == ProcessingMode::Hybrid => {
                warning = Some(format!("{error} Using local fallback naming."));
            }
            Err(error) => return Err(ProcessingError::Vision(error)),
        }
    }
    let candidate = naming_engine::name(&metadata, rules).map_err(ProcessingError::Naming)?;
    Ok(ProcessingResult {
        candidate,
        source,
        warning,
    })
}

pub fn read_image(file: &ScreenshotFile) -> Result<Vec<u8>, VisionError> {
    let source = File::open(file.original_path()).map_err(|_| VisionError::Configuration)?;
    let mut bytes = Vec::new();
    source
        .take((MAX_IMAGE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| VisionError::Configuration)?;
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(VisionError::Configuration);
    }
    Ok(bytes)
}
pub fn fingerprint(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
pub struct PendingAnalysis {
    pub file: ScreenshotFile,
    id: String,
    fingerprint: Option<[u8; 32]>,
}
impl PendingAnalysis {
    pub fn verify(&self) -> Result<(), String> {
        let bytes = read_image(&self.file)
            .map_err(|_| "The original screenshot is unavailable or too large.".to_string())?;
        if self.fingerprint != Some(fingerprint(&bytes)) {
            return Err("The original screenshot changed. Retry is unavailable.".into());
        }
        Ok(())
    }
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisFailure {
    pub id: String,
    pub source_path: String,
    pub kind: VisionError,
    pub message: String,
    pub retryable: bool,
}
#[derive(Default)]
pub struct AnalysisRecovery {
    next_id: u64,
    pending: BTreeMap<String, (PendingAnalysis, AnalysisFailure)>,
}
impl AnalysisRecovery {
    pub fn failures(&self) -> Vec<AnalysisFailure> {
        self.pending.values().map(|(_, f)| f.clone()).collect()
    }
    pub fn record(
        &mut self,
        file: ScreenshotFile,
        kind: VisionError,
        previous: Option<PendingAnalysis>,
        digest: Option<[u8; 32]>,
    ) -> AnalysisFailure {
        let pending = previous.unwrap_or_else(|| {
            self.next_id += 1;
            let digest = digest.or_else(|| read_image(&file).ok().map(|b| fingerprint(&b)));
            PendingAnalysis {
                file,
                id: format!("analysis-{}", self.next_id),
                fingerprint: digest,
            }
        });
        let retryable = pending.verify().is_ok();
        let failure = AnalysisFailure {
            id: pending.id.clone(),
            source_path: pending.file.original_path().to_string_lossy().into_owned(),
            kind,
            message: format!("{kind} The screenshot has not been renamed."),
            retryable,
        };
        // Bound session state without persisting image data or cloud context.
        if self.pending.len() >= 100 {
            if let Some(id) = self.pending.keys().next().cloned() {
                self.pending.remove(&id);
            }
        }
        self.pending
            .insert(failure.id.clone(), (pending, failure.clone()));
        failure
    }
    pub fn reserve(&mut self, id: &str) -> Result<PendingAnalysis, String> {
        let (pending, failure) = self
            .pending
            .get_mut(id)
            .ok_or("This analysis retry is unavailable.")?;
        if let Err(error) = pending.verify() {
            failure.retryable = false;
            return Err(error);
        }
        Ok(self.pending.remove(id).unwrap().0)
    }
}
