use std::{
    collections::HashSet,
    fmt,
    fs::OpenOptions,
    io::Read,
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::{is_supported_image_file, ScreenshotFile};

const READINESS_TIMEOUT: Duration = Duration::from_secs(2);
const READINESS_POLL_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug)]
pub enum ReadinessError {
    TimedOut { path: PathBuf, timeout: Duration },
}

impl fmt::Display for ReadinessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TimedOut { path, timeout } => write!(
                formatter,
                "file did not become ready within {:?}: {}",
                timeout,
                path.display()
            ),
        }
    }
}

impl std::error::Error for ReadinessError {}

pub fn wait_for_file_readiness(
    path: &std::path::Path,
    timeout: Duration,
    poll_interval: Duration,
) -> Result<(), ReadinessError> {
    let deadline = Instant::now() + timeout;

    loop {
        if file_is_ready(path, poll_interval) {
            return Ok(());
        }

        let now = Instant::now();
        if now >= deadline {
            return Err(ReadinessError::TimedOut {
                path: path.to_path_buf(),
                timeout,
            });
        }

        thread::sleep(poll_interval.min(deadline.saturating_duration_since(now)));
    }
}

fn file_is_ready(path: &std::path::Path, poll_interval: Duration) -> bool {
    let Ok(initial_metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !initial_metadata.is_file() {
        return false;
    }

    let Ok(mut file) = OpenOptions::new().read(true).write(true).open(path) else {
        return false;
    };
    let mut contents = Vec::new();
    if file.read_to_end(&mut contents).is_err() {
        return false;
    }

    thread::sleep(poll_interval);

    let Ok(final_metadata) = std::fs::metadata(path) else {
        return false;
    };
    final_metadata.is_file() && final_metadata.len() == initial_metadata.len()
}

#[derive(Debug)]
pub enum FolderWatcherError {
    WatcherEvent(String),
    InvalidDirectory(PathBuf),
    Watcher(String),
    Stop(String),
}

impl fmt::Display for FolderWatcherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WatcherEvent(message) => write!(formatter, "watcher event error: {message}"),
            Self::InvalidDirectory(path) => {
                write!(formatter, "not a valid directory: {}", path.display())
            }
            Self::Watcher(message) => write!(formatter, "watcher error: {message}"),
            Self::Stop(message) => write!(formatter, "watcher stop error: {message}"),
        }
    }
}

impl std::error::Error for FolderWatcherError {}

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

            if wait_for_file_readiness(path, READINESS_TIMEOUT, READINESS_POLL_INTERVAL).is_err() {
                self.emitted_paths.remove(&canonical_path);
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

pub struct FolderWatcher {
    stop_sender: Option<Sender<()>>,
    worker: Option<JoinHandle<()>>,
}

impl FolderWatcher {
    pub fn start<F>(directory: PathBuf, on_file: F) -> Result<Self, FolderWatcherError>
    where
        F: Fn(ScreenshotFile) + Send + Sync + 'static,
    {
        if !directory.is_dir() {
            return Err(FolderWatcherError::InvalidDirectory(directory));
        }

        let (event_sender, event_receiver) = mpsc::sync_channel(64);
        let mut watcher = notify::recommended_watcher(move |event| {
            let _ = event_sender.send(event);
        })
        .map_err(|error| FolderWatcherError::Watcher(error.to_string()))?;
        watcher
            .watch(&directory, RecursiveMode::NonRecursive)
            .map_err(|error| FolderWatcherError::Watcher(error.to_string()))?;

        let (stop_sender, stop_receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            run_worker(watcher, event_receiver, stop_receiver, on_file);
        });

        Ok(Self {
            stop_sender: Some(stop_sender),
            worker: Some(worker),
        })
    }

    pub fn stop(&mut self) -> Result<(), FolderWatcherError> {
        if let Some(sender) = self.stop_sender.take() {
            let _ = sender.send(());
        }

        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| FolderWatcherError::Stop("watcher worker panicked".to_owned()))?;
        }

        Ok(())
    }
}

fn run_worker<F>(
    _watcher: RecommendedWatcher,
    event_receiver: Receiver<Result<Event, notify::Error>>,
    stop_receiver: Receiver<()>,
    on_file: F,
) where
    F: Fn(ScreenshotFile),
{
    let mut processor = FolderWatcherProcessor::new();

    loop {
        match stop_receiver.try_recv() {
            Ok(()) | Err(TryRecvError::Disconnected) => break,
            Err(TryRecvError::Empty) => {}
        }

        match event_receiver.recv_timeout(Duration::from_millis(50)) {
            Ok(Ok(event)) => {
                if let Ok(files) = processor.process_event(&event) {
                    for file in files {
                        on_file(file);
                    }
                }
            }
            Ok(Err(_error)) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs::OpenOptions,
        path::PathBuf,
        sync::{Arc, Mutex},
        thread,
        time::{Duration, Instant},
    };

    use notify::{event::CreateKind, Event, EventKind};
    use tempfile::tempdir;

    use super::{
        wait_for_file_readiness, FolderWatcher, FolderWatcherError, FolderWatcherProcessor,
        ReadinessError,
    };
    use crate::ProcessingState;

    fn create_event(path: PathBuf) -> Event {
        Event {
            kind: EventKind::Create(CreateKind::File),
            paths: vec![path],
            attrs: Default::default(),
        }
    }

    #[test]
    fn waits_until_a_screenshot_is_created_and_stable() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("Screenshot.png");
        let writer_path = path.clone();

        thread::spawn(move || {
            thread::sleep(Duration::from_millis(40));
            std::fs::write(&writer_path, b"image data").unwrap();
        });

        wait_for_file_readiness(&path, Duration::from_millis(500), Duration::from_millis(10))
            .unwrap();
    }

    #[test]
    fn times_out_when_a_screenshot_never_becomes_ready() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("Screenshot.png");
        let started = Instant::now();

        let error =
            wait_for_file_readiness(&path, Duration::from_millis(60), Duration::from_millis(10))
                .unwrap_err();

        assert!(matches!(error, ReadinessError::TimedOut { .. }));
        assert!(started.elapsed() >= Duration::from_millis(60));
    }

    #[test]
    fn waits_for_a_file_that_is_still_being_written() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("Screenshot.png");
        std::fs::write(&path, b"first").unwrap();
        let writer_path = path.clone();

        thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            let mut file = OpenOptions::new().append(true).open(writer_path).unwrap();
            std::io::Write::write_all(&mut file, b" second").unwrap();
        });

        wait_for_file_readiness(&path, Duration::from_millis(500), Duration::from_millis(10))
            .unwrap();
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

    #[test]
    fn rejects_missing_and_non_directory_paths() {
        let missing = PathBuf::from("C:\\does-not-exist\\screenshot-renamer");
        assert!(matches!(
            FolderWatcher::start(missing, |_| {}),
            Err(FolderWatcherError::InvalidDirectory(_))
        ));

        let directory = tempdir().unwrap();
        let file = directory.path().join("not-a-directory.png");
        std::fs::write(&file, b"image").unwrap();
        assert!(matches!(
            FolderWatcher::start(file, |_| {}),
            Err(FolderWatcherError::InvalidDirectory(_))
        ));
    }

    #[test]
    fn stops_idempotently_and_emits_a_new_supported_file_once() {
        let directory = tempdir().unwrap();
        let received = Arc::new(Mutex::new(Vec::new()));
        let callback_received = Arc::clone(&received);
        let mut watcher = FolderWatcher::start(directory.path().to_path_buf(), move |file| {
            callback_received.lock().unwrap().push(file);
        })
        .unwrap();

        let path = directory.path().join("Screenshot.png");
        std::fs::write(&path, b"image").unwrap();

        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while std::time::Instant::now() < deadline && received.lock().unwrap().is_empty() {
            std::thread::sleep(Duration::from_millis(25));
        }

        assert_eq!(received.lock().unwrap().len(), 1);
        watcher.stop().unwrap();
        watcher.stop().unwrap();
    }
}
