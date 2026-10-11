pub fn startup_command(executable: &str) -> Result<String, String> {
    if executable.is_empty() || executable.contains('"') || executable.chars().any(char::is_control)
    {
        return Err("Executable path is invalid.".into());
    }
    Ok(format!("\"{executable}\" --autostart"))
}

pub trait StartupRegistration {
    fn is_enabled(&self) -> Result<bool, String>;
    fn set_enabled(&self, enabled: bool) -> Result<(), String>;
}
pub struct OsStartup(pub tauri::AppHandle);
impl StartupRegistration for OsStartup {
    #[cfg(windows)]
    fn is_enabled(&self) -> Result<bool, String> {
        use winreg::{enums::HKEY_CURRENT_USER, RegKey};
        let root = RegKey::predef(HKEY_CURRENT_USER);
        let name = &self.0.config().identifier;
        let command = match root
            .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run")
            .and_then(|key| key.get_value::<String, _>(name))
        {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(_) => return Err("Could not read Windows startup registration.".into()),
        };
        let expected = startup_command(
            std::env::current_exe()
                .map_err(|_| "Executable path unavailable.")?
                .to_str()
                .ok_or("Executable path is invalid.")?,
        )?;
        if !command.eq_ignore_ascii_case(&expected) {
            return Ok(false);
        }
        match root
            .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run")
            .and_then(|key| key.get_raw_value(name))
        {
            Ok(value) if value.bytes.len() >= 12 => Ok(matches!(value.bytes[0], 2 | 6)),
            Ok(_) => Err("Windows startup approval could not be read.".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
            Err(_) => Err("Windows startup approval could not be read.".into()),
        }
    }
    #[cfg(windows)]
    fn set_enabled(&self, enabled: bool) -> Result<(), String> {
        use winreg::{
            enums::{HKEY_CURRENT_USER, REG_BINARY},
            RegKey, RegValue,
        };
        let root = RegKey::predef(HKEY_CURRENT_USER);
        let name = &self.0.config().identifier;
        let run = root
            .create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run")
            .map_err(|_| "Could not change Windows startup registration.")?
            .0;
        if enabled {
            let command = startup_command(
                std::env::current_exe()
                    .map_err(|_| "Executable path unavailable.")?
                    .to_str()
                    .ok_or("Executable path is invalid.")?,
            )?;
            run.set_value(name, &command)
                .map_err(|_| "Could not change Windows startup registration.")?;
            let approval = root
                .create_subkey(
                    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run",
                )
                .map_err(|_| "Could not enable Windows startup approval.")?
                .0;
            let mut bytes = vec![0; 12];
            bytes[0] = 2;
            approval
                .set_raw_value(
                    name,
                    &RegValue {
                        bytes,
                        vtype: REG_BINARY,
                    },
                )
                .map_err(|_| "Could not enable Windows startup approval.")?;
        } else {
            match run.delete_value(name) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err("Could not remove Windows startup registration.".into()),
            }
        }
        Ok(())
    }
    #[cfg(not(windows))]
    fn is_enabled(&self) -> Result<bool, String> {
        Err("Startup is supported on Windows.".into())
    }
    #[cfg(not(windows))]
    fn set_enabled(&self, _: bool) -> Result<(), String> {
        Err("Startup is supported on Windows.".into())
    }
}
pub fn set_startup<R: StartupRegistration>(
    os: &R,
    enabled: bool,
    persist: impl FnOnce(bool) -> Result<(), String>,
) -> Result<bool, String> {
    let previous = os.is_enabled()?;
    let result = os.set_enabled(enabled).and_then(|_| match os.is_enabled() {
        Ok(actual) if actual == enabled => persist(actual).map(|_| actual),
        _ => Err("Windows did not confirm the requested startup setting.".into()),
    });
    if result.is_err() && os.set_enabled(previous).is_err() {
        return Err("Could not save startup preference or restore registration. Recheck Start with Windows.".into());
    }
    result
}
pub fn start_hidden(
    login_launch: bool,
    tray_ready: bool,
    manual_requested: bool,
    startup_error: bool,
) -> bool {
    login_launch && tray_ready && !manual_requested && !startup_error
}
#[derive(Default)]
pub struct LaunchState(pub std::sync::atomic::AtomicBool);
#[derive(Default)]
pub struct StartupGate(pub std::sync::Mutex<()>);
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupResult {
    pub enabled: bool,
    pub snapshot: crate::settings::SettingsSnapshot,
}
#[tauri::command]
pub async fn get_startup_enabled(app: tauri::AppHandle) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || OsStartup(app).is_enabled())
        .await
        .map_err(|_| "Startup registration is unavailable.".to_string())?
}
#[tauri::command]
pub async fn set_startup_enabled(
    app: tauri::AppHandle,
    enabled: bool,
) -> Result<StartupResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        use tauri::Manager;
        let gate = app.state::<StartupGate>();
        let _guard = gate
            .0
            .lock()
            .map_err(|_| "Startup settings are unavailable.")?;
        let state = app.state::<crate::SettingsState>();
        let mut controller = state
            .controller
            .lock()
            .map_err(|_| "Settings are unavailable.")?;
        let actual = set_startup(&OsStartup(app.clone()), enabled, |actual| {
            let mut desktop = controller.snapshot().settings.desktop;
            desktop.start_with_windows = actual;
            controller.save_desktop_preferences(desktop).map(|_| ())
        })?;
        drop(controller);
        let snapshot = crate::publish_watcher_state(&app)?;
        Ok(StartupResult {
            enabled: actual,
            snapshot,
        })
    })
    .await
    .map_err(|_| "Startup registration is unavailable.".to_string())?
}
