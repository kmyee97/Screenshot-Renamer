use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplicationSettings {
    pub version: u32,
    pub watched_folder: Option<String>,
    pub auto_rename: bool,
}
impl Default for ApplicationSettings {
    fn default() -> Self {
        Self {
            version: 1,
            watched_folder: None,
            auto_rename: false,
        }
    }
}
impl ApplicationSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("Unsupported settings version; defaults restored.".into());
        }
        if let Some(folder) = &self.watched_folder {
            validate_folder(folder)?;
        }
        if self.auto_rename && self.watched_folder.is_none() {
            return Err("Choose a folder before enabling Auto Rename.".into());
        }
        Ok(())
    }
}
pub fn validate_folder(folder: &str) -> Result<PathBuf, String> {
    let path = Path::new(folder);
    if !path.is_absolute() || !path.is_dir() {
        return Err("Choose an existing, absolute folder path.".into());
    }
    fs::read_dir(path).map_err(|error| format!("Cannot read screenshot folder: {error}"))?;
    path.canonicalize()
        .map_err(|error| format!("Cannot access screenshot folder: {error}"))
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshot {
    pub settings: ApplicationSettings,
    pub warning: Option<String>,
}

pub struct SettingsStore {
    path: PathBuf,
}
impl SettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
    pub fn load(&self) -> SettingsSnapshot {
        let result = fs::read(&self.path)
            .map_err(|error| format!("Settings could not be loaded; using defaults: {error}"))
            .and_then(|data| {
                serde_json::from_slice::<ApplicationSettings>(&data)
                    .map_err(|error| format!("Invalid settings; using defaults: {error}"))
            })
            .and_then(|settings| {
                settings.validate()?;
                Ok(settings)
            });
        match result {
            Ok(settings) => SettingsSnapshot {
                settings,
                warning: None,
            },
            Err(warning) => SettingsSnapshot {
                settings: ApplicationSettings::default(),
                warning: Some(warning),
            },
        }
    }
    pub fn save(&self, settings: &ApplicationSettings) -> Result<(), String> {
        settings.validate()?;
        let parent = self
            .path
            .parent()
            .ok_or("Settings directory is unavailable.")?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("Cannot create settings directory: {error}"))?;
        let temporary = parent.join(format!(".settings-{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(|error| error.to_string())?;
            let data = serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?;
            file.write_all(&data)
                .and_then(|_| file.sync_all())
                .map_err(|error| error.to_string())?;
            drop(file);
            fs::rename(&temporary, &self.path)
                .map_err(|error| format!("Cannot save settings: {error}"))
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}
