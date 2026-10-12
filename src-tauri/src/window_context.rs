//! Optional foreground hints; never requests process arguments or full window contents.
use crate::screenshot_metadata::WindowContext;
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

pub(crate) fn is_capture_tool(app: &str) -> bool {
    let Some(stem) = Path::new(app).file_stem() else {
        return false;
    };
    let stem = stem.to_string_lossy();
    ["SnippingTool", "SnipAndSketch", "ScreenClippingHost"]
        .iter()
        .any(|tool| stem.eq_ignore_ascii_case(tool))
}

pub fn normalize_context(app: &str, title: &str) -> Option<WindowContext> {
    // Saving a snip foregrounds the capture UI, not the app shown in the image.
    if is_capture_tool(app) {
        return None;
    }
    let app = Path::new(app).file_stem()?.to_string_lossy();
    let app: String = app.chars().filter(|c| !c.is_control()).take(40).collect();
    if app.trim().is_empty() || app.eq_ignore_ascii_case("screenshot-renamer") {
        return None;
    }
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    let title: String = title.chars().filter(|c| !c.is_control()).take(80).collect();
    Some(WindowContext {
        app_name: app,
        window_title: (!title.is_empty()).then_some(title),
    })
}

pub fn capture_with_timeout<F>(lookup: F, timeout: Duration) -> Option<WindowContext>
where
    F: FnOnce() -> Option<WindowContext> + Send + 'static,
{
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("window-context".into())
        .spawn(move || {
            let _ = tx.send(lookup());
        })
        .ok()?;
    rx.recv_timeout(timeout).ok().flatten()
}

static LOOKUP_ACTIVE: AtomicBool = AtomicBool::new(false);
pub fn capture() -> Option<WindowContext> {
    if LOOKUP_ACTIVE.swap(true, Ordering::SeqCst) {
        return None;
    }
    struct Release;
    impl Drop for Release {
        fn drop(&mut self) {
            LOOKUP_ACTIVE.store(false, Ordering::SeqCst);
        }
    }
    let release = Release;
    capture_with_timeout(
        move || {
            let _release = release;
            native_capture()
        },
        Duration::from_millis(30),
    )
}

#[cfg(not(windows))]
fn native_capture() -> Option<WindowContext> {
    None
}

#[cfg(windows)]
fn native_capture() -> Option<WindowContext> {
    use windows::{
        core::PWSTR,
        Win32::{
            Foundation::CloseHandle,
            System::Threading::{
                OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION,
            },
            UI::WindowsAndMessaging::{
                GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
            },
        },
    };
    unsafe {
        let window = GetForegroundWindow();
        if window.0.is_null() {
            return None;
        }
        let mut process_id = 0;
        GetWindowThreadProcessId(window, Some(&mut process_id));
        if process_id == 0 || process_id == std::process::id() {
            return None;
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id).ok()?;
        let mut path = [0u16; 1024];
        let mut length = path.len() as u32;
        let result = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(path.as_mut_ptr()),
            &mut length,
        );
        let _ = CloseHandle(process);
        result.ok()?;
        let mut title = [0u16; 256];
        let count = GetWindowTextW(window, &mut title).max(0) as usize;
        if GetForegroundWindow() != window {
            return None;
        }
        normalize_context(
            &String::from_utf16_lossy(&path[..length as usize]),
            &String::from_utf16_lossy(&title[..count]),
        )
    }
}
