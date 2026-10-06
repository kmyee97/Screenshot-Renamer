use std::{collections::HashSet, path::PathBuf};

use notify::{Event, EventKind};

use crate::{is_supported_image_file, ScreenshotFile};

#[derive(Debug)]
pub enum FolderWatcherError {
    WatcherEvent(String),
    InvalidDirectory(PathBuf),
    Watcher(String),
    Stop(String),
}

pub struct FolderWatcherProcessor {
    emitted_paths: HashSet<PathBuf>,
}

impl FolderWatcherProcessor {
    pub fn new() -> Self {
        Self {
            emitted_paths: HashSet::new(),
        }
    }

    pub fn process_event(
        &mut self,
        event: &Event,
    ) -> Result<Vec<ScreenshotFile>, FolderWatcherError> {
        if matches!(event.kind, EventKind::Other) {
            return Err(FolderWatcherError::WatcherEvent(
                "filesystem watcher reported an unknown event".to_owned(),
            ));
        }

        if !matches!(event.kind, EventKind::Create(_)) {
            return Ok(Vec::new());
        }

        let mut emitted = Vec::new();
        for path in &event.paths {
            if !is_supported_image_file(path) {
                continue;
            }

            let Ok(canonical_path) = path.canonicalize() else {
                continue;
            };
            if !self.emitted_paths.insert(canonical_path.clone()) {
                continue;
            }

            let created_at = std::fs::metadata(path)
                .ok()
                .and_then(|metadata| metadata.created().ok());
            if let Ok(screenshot) = ScreenshotFile::new(path.clone(), created_at) {
                emitted.push(screenshot);
            }
        }

        Ok(emitted)
    }
}

impl Default for FolderWatcherProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use notify::{event::CreateKind, Event, EventKind};
    use tempfile::tempdir;

    use super::{FolderWatcherError, FolderWatcherProcessor};
    use crate::ProcessingState;

    fn create_event(path: PathBuf) -> Event {
        Event {
            kind: EventKind::Create(CreateKind::File),
            paths: vec![path],
            attrs: Default::default(),
        }
    }

    #[test]
    fn emits_one_screenshot_file_for_a_supported_create_event() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("Screenshot.png");
        std::fs::write(&path, b"image").unwrap();
        let mut processor = FolderWatcherProcessor::new();

        let emitted = processor
            .process_event(&create_event(path.clone()))
            .unwrap();

        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].original_path(), path);
        assert_eq!(emitted[0].state(), &ProcessingState::Detected);
    }

    #[test]
    fn ignores_unsupported_files_directories_and_non_create_events() {
        let directory = tempdir().unwrap();
        let text_path = directory.path().join("notes.txt");
        let nested_path = directory.path().join("nested");
        std::fs::write(&text_path, b"text").unwrap();
        std::fs::create_dir(&nested_path).unwrap();
        let mut processor = FolderWatcherProcessor::new();

        assert!(processor
            .process_event(&create_event(text_path))
            .unwrap()
            .is_empty());
        assert!(processor
            .process_event(&create_event(nested_path))
            .unwrap()
            .is_empty());
        assert!(processor
            .process_event(&Event {
                kind: EventKind::Modify(notify::event::ModifyKind::Any),
                paths: vec![],
                attrs: Default::default(),
            })
            .unwrap()
            .is_empty());
    }

    #[test]
    fn deduplicates_repeated_create_events_by_canonical_path() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("Screenshot.jpg");
        std::fs::write(&path, b"image").unwrap();
        let mut processor = FolderWatcherProcessor::new();

        assert_eq!(
            processor
                .process_event(&create_event(path.clone()))
                .unwrap()
                .len(),
            1
        );
        assert!(processor
            .process_event(&create_event(path))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn continues_after_a_file_disappears_before_inspection() {
        let directory = tempdir().unwrap();
        let missing_path = directory.path().join("missing.png");
        let existing_path = directory.path().join("existing.png");
        std::fs::write(&existing_path, b"image").unwrap();
        let mut processor = FolderWatcherProcessor::new();

        assert!(processor
            .process_event(&create_event(missing_path))
            .unwrap()
            .is_empty());
        assert_eq!(
            processor
                .process_event(&create_event(existing_path))
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn accepts_mixed_case_supported_extensions() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("Screenshot.JpEg");
        std::fs::write(&path, b"image").unwrap();
        let mut processor = FolderWatcherProcessor::new();

        assert_eq!(
            processor.process_event(&create_event(path)).unwrap().len(),
            1
        );
    }

    #[test]
    fn returns_watcher_errors_without_poisoning_the_processor() {
        let mut processor = FolderWatcherProcessor::new();
        let error_event = Event {
            kind: EventKind::Other,
            paths: vec![],
            attrs: Default::default(),
        };

        assert!(matches!(
            processor.process_event(&error_event),
            Err(FolderWatcherError::WatcherEvent(_))
        ));
    }
}
