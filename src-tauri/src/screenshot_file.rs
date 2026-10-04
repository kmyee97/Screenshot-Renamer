use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScreenshotFile {
    original_path: PathBuf,
    filename: OsString,
    extension: Option<OsString>,
    created_at: Option<SystemTime>,
    state: ProcessingState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScreenshotFileError {
    MissingFilename { path: PathBuf },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProcessingState {
    Detected,
    Ready,
    Analyzing,
    Naming,
    Renaming,
    Renamed,
    Failed {
        stage: ProcessingStage,
        message: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProcessingStage {
    Readiness,
    Analysis,
    Naming,
    Rename,
}

impl ScreenshotFile {
    pub fn new(
        original_path: PathBuf,
        created_at: Option<SystemTime>,
    ) -> Result<Self, ScreenshotFileError> {
        let filename = original_path
            .file_name()
            .map(OsStr::to_owned)
            .ok_or_else(|| ScreenshotFileError::MissingFilename {
                path: original_path.clone(),
            })?;
        let extension = original_path.extension().map(OsStr::to_owned);

        Ok(Self {
            original_path,
            filename,
            extension,
            created_at,
            state: ProcessingState::Detected,
        })
    }

    pub fn original_path(&self) -> &Path {
        &self.original_path
    }

    pub fn filename(&self) -> &OsStr {
        &self.filename
    }

    pub fn extension(&self) -> Option<&OsStr> {
        self.extension.as_deref()
    }

    pub fn created_at(&self) -> Option<&SystemTime> {
        self.created_at.as_ref()
    }

    pub fn state(&self) -> &ProcessingState {
        &self.state
    }

    pub fn transition_to(&mut self, state: ProcessingState) {
        self.state = state;
    }
}

#[cfg(test)]
mod tests {
    use std::{
        ffi::OsStr,
        path::{Path, PathBuf},
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use super::{ProcessingStage, ProcessingState, ScreenshotFile, ScreenshotFileError};

    #[test]
    fn preserves_source_facts_for_a_screenshot_file() {
        let original_path = PathBuf::from(r"C:\Screenshots\Screenshot_2026-10-03_141922.png");
        let created_at = UNIX_EPOCH + Duration::from_secs(1_728_000_000);

        let screenshot = ScreenshotFile::new(original_path.clone(), Some(created_at)).unwrap();

        assert_eq!(screenshot.original_path(), Path::new(&original_path));
        assert_eq!(
            screenshot.filename(),
            OsStr::new("Screenshot_2026-10-03_141922.png")
        );
        assert_eq!(screenshot.extension(), Some(OsStr::new("png")));
        assert_eq!(screenshot.created_at(), Some(&created_at));
        assert_eq!(screenshot.state(), &ProcessingState::Detected);
    }

    #[test]
    fn keeps_an_extensionless_filename_intact() {
        let screenshot =
            ScreenshotFile::new(PathBuf::from(r"C:\Screenshots\Screenshot_141922"), None).unwrap();

        assert_eq!(screenshot.filename(), OsStr::new("Screenshot_141922"));
        assert_eq!(screenshot.extension(), None);
    }

    #[test]
    fn uses_only_the_final_extension_for_multi_dot_filenames() {
        let screenshot =
            ScreenshotFile::new(PathBuf::from(r"C:\Screenshots\report.final.png"), None).unwrap();

        assert_eq!(screenshot.filename(), OsStr::new("report.final.png"));
        assert_eq!(screenshot.extension(), Some(OsStr::new("png")));
    }

    #[test]
    fn rejects_a_path_without_a_filename() {
        let original_path = PathBuf::from(r"C:\");

        let error = ScreenshotFile::new(original_path.clone(), None).unwrap_err();

        assert_eq!(
            error,
            ScreenshotFileError::MissingFilename {
                path: original_path
            }
        );
    }

    #[test]
    fn transitions_through_processing_and_retains_failure_details() {
        let mut screenshot = ScreenshotFile::new(
            PathBuf::from(r"C:\Screenshots\Screenshot_2026-10-03_141922.png"),
            Some(SystemTime::now()),
        )
        .unwrap();

        screenshot.transition_to(ProcessingState::Ready);
        assert_eq!(screenshot.state(), &ProcessingState::Ready);

        screenshot.transition_to(ProcessingState::Renaming);
        assert_eq!(screenshot.state(), &ProcessingState::Renaming);

        let failed_state = ProcessingState::Failed {
            stage: ProcessingStage::Rename,
            message: "destination already exists".to_owned(),
        };
        screenshot.transition_to(failed_state.clone());

        assert_eq!(screenshot.state(), &failed_state);
    }
}
