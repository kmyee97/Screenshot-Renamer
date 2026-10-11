use crate::{
    settings::{ApplicationSettings, SettingsSnapshot, SettingsStore},
    FolderWatcher,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
pub trait WatchSession {
    fn stop(&mut self) -> Result<(), String>;
}
impl WatchSession for FolderWatcher {
    fn stop(&mut self) -> Result<(), String> {
        FolderWatcher::stop(self).map_err(|error| error.to_string())
    }
}
struct Active<W: WatchSession> {
    watcher: W,
    enabled: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
}
impl<W: WatchSession> Drop for Active<W> {
    fn drop(&mut self) {
        self.enabled.store(false, Ordering::SeqCst);
        let _ = self.watcher.stop();
    }
}
pub struct SettingsController<W: WatchSession> {
    store: Result<SettingsStore, String>,
    snapshot: SettingsSnapshot,
    active: Option<Active<W>>,
    naming_rules: Arc<Mutex<crate::naming_engine::NamingRules>>,
    processing_settings: Arc<Mutex<ApplicationSettings>>,
}
impl<W: WatchSession> SettingsController<W> {
    pub fn new(store: Result<SettingsStore, String>) -> Self {
        let snapshot = match &store {
            Ok(store) => store.load(),
            Err(error) => SettingsSnapshot::new(Default::default(), Some(error.clone())),
        };
        Self {
            processing_settings: Arc::new(Mutex::new(snapshot.settings.clone())),
            naming_rules: Arc::new(Mutex::new(snapshot.settings.naming.clone())),
            store,
            snapshot,
            active: None,
        }
    }
    pub fn snapshot(&self) -> SettingsSnapshot {
        let mut snapshot = self.snapshot.clone();
        snapshot.watcher_status = if self.is_watching() {
            "Watching"
        } else if snapshot.settings.auto_rename {
            "Error"
        } else {
            "Paused"
        }
        .into();
        snapshot.watcher_error = if snapshot.watcher_status == "Error" {
            self.active
                .as_ref()
                .and_then(|active| active.error.lock().ok().and_then(|error| error.clone()))
                .or_else(|| snapshot.warning.clone())
                .or_else(|| {
                    Some("Watcher is unavailable. Turn Auto Rename off and on to retry.".into())
                })
        } else {
            None
        };
        snapshot
    }
    pub fn naming_rules(&self) -> Arc<Mutex<crate::naming_engine::NamingRules>> {
        self.naming_rules.clone()
    }
    pub fn processing_settings(&self) -> Arc<Mutex<ApplicationSettings>> {
        self.processing_settings.clone()
    }
    fn commit_naming(&self, settings: &ApplicationSettings) {
        *self
            .processing_settings
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = settings.clone();
        *self.naming_rules.lock().unwrap_or_else(|e| e.into_inner()) = settings.naming.clone();
    }
    pub fn is_watching(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| active.enabled.load(Ordering::SeqCst))
    }
    pub fn shutdown(&mut self) {
        self.active.take();
    }
    pub fn set_warning(&mut self, message: String) {
        self.snapshot.warning = Some(message);
    }
    pub fn restore<F>(&mut self, start: F)
    where
        F: FnOnce(&str, Arc<AtomicBool>, Arc<Mutex<Option<String>>>) -> Result<W, String>,
    {
        if !self.snapshot.settings.auto_rename {
            return;
        }
        let enabled = Arc::new(AtomicBool::new(false));
        let error = Arc::new(Mutex::new(None));
        if let Some(folder) = self.snapshot.settings.watched_folder.as_deref() {
            match start(folder, enabled.clone(), error.clone()) {
                Ok(watcher) => {
                    enabled.store(true, Ordering::SeqCst);
                    self.active = Some(Active {
                        watcher,
                        enabled,
                        error,
                    });
                }
                Err(error) => {
                    self.snapshot.warning = Some(format!("Cannot start saved folder: {error}"))
                }
            }
        }
    }
    pub fn apply<F>(
        &mut self,
        settings: ApplicationSettings,
        start: F,
    ) -> Result<SettingsSnapshot, String>
    where
        F: FnOnce(&str, Arc<AtomicBool>, Arc<Mutex<Option<String>>>) -> Result<W, String>,
    {
        // Pausing an unavailable folder must remain possible; validate a folder only when enabling or replacing it.
        settings.ai.validate().map_err(|error| error.to_string())?;
        settings
            .naming
            .validate()
            .map_err(|error| error.to_string())?;
        let pausing = !settings.auto_rename
            && settings.watched_folder == self.snapshot.settings.watched_folder;
        if pausing {
            if settings.version != 1 {
                return Err("Unsupported settings version".into());
            }
        } else {
            settings.validate()?;
        }
        if settings.watched_folder == self.snapshot.settings.watched_folder
            && settings.auto_rename == self.snapshot.settings.auto_rename
            && (!settings.auto_rename || self.is_watching())
        {
            self.store
                .as_ref()
                .map_err(Clone::clone)?
                .save_preference(&settings, pausing)?;
            self.commit_naming(&settings);
            self.snapshot = SettingsSnapshot::new(settings, None);
            return Ok(self.snapshot());
        }
        let enabled = Arc::new(AtomicBool::new(false));
        let error = Arc::new(Mutex::new(None));
        let mut candidate = if !pausing {
            match settings.watched_folder.as_deref() {
                Some(folder) => Some(Active {
                    watcher: start(folder, enabled.clone(), error.clone())?,
                    enabled: enabled.clone(),
                    error,
                }),
                None => None,
            }
        } else {
            None
        };
        if !settings.auto_rename {
            candidate.take();
        }
        self.store
            .as_ref()
            .map_err(Clone::clone)?
            .save_preference(&settings, pausing)?;
        self.active.take();
        self.commit_naming(&settings);
        enabled.store(settings.auto_rename, Ordering::SeqCst);
        self.active = candidate;
        self.snapshot = SettingsSnapshot::new(settings, None);
        Ok(self.snapshot())
    }
}
