use crate::{
    settings::{ApplicationSettings, SettingsSnapshot, SettingsStore},
    FolderWatcher,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
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
}
impl<W: WatchSession> SettingsController<W> {
    pub fn new(store: Result<SettingsStore, String>) -> Self {
        let snapshot = match &store {
            Ok(store) => store.load(),
            Err(error) => SettingsSnapshot {
                settings: Default::default(),
                warning: Some(error.clone()),
            },
        };
        Self {
            store,
            snapshot,
            active: None,
        }
    }
    pub fn snapshot(&self) -> SettingsSnapshot {
        self.snapshot.clone()
    }
    pub fn is_watching(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| active.enabled.load(Ordering::SeqCst))
    }
    pub fn restore<F>(&mut self, start: F)
    where
        F: FnOnce(&str, Arc<AtomicBool>) -> Result<W, String>,
    {
        if !self.snapshot.settings.auto_rename {
            return;
        }
        let enabled = Arc::new(AtomicBool::new(false));
        if let Some(folder) = self.snapshot.settings.watched_folder.as_deref() {
            match start(folder, enabled.clone()) {
                Ok(watcher) => {
                    enabled.store(true, Ordering::SeqCst);
                    self.active = Some(Active { watcher, enabled });
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
        F: FnOnce(&str, Arc<AtomicBool>) -> Result<W, String>,
    {
        settings.validate()?;
        // Prepare a disabled watcher first. Neither the old settings nor active watcher change if preparation or persistence fails.
        let enabled = Arc::new(AtomicBool::new(false));
        let mut candidate = match settings.watched_folder.as_deref() {
            Some(folder) => Some(Active {
                watcher: start(folder, enabled.clone())?,
                enabled: enabled.clone(),
            }),
            None => None,
        };
        if !settings.auto_rename {
            candidate.take();
        }
        self.store.as_ref().map_err(Clone::clone)?.save(&settings)?;
        self.active.take();
        enabled.store(settings.auto_rename, Ordering::SeqCst);
        self.active = candidate;
        self.snapshot = SettingsSnapshot {
            settings,
            warning: None,
        };
        Ok(self.snapshot())
    }
}
