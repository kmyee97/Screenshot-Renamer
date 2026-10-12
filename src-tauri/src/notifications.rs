use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::Instant,
};
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NotificationPreferences {
    pub errors: bool,
    pub successes: bool,
}
impl Default for NotificationPreferences {
    fn default() -> Self {
        Self {
            errors: true,
            successes: false,
        }
    }
}
pub enum Outcome<'a> {
    Renamed { id: &'a str, name: &'a str },
    RenameFailed,
    AnalysisFailed,
    WatcherFailed,
    UndoFailed,
}
pub struct Notice {
    pub title: &'static str,
    pub body: String,
}
#[derive(Default)]
pub struct NotificationPolicy {
    successes: VecDeque<String>,
    errors: HashMap<&'static str, u64>,
}
impl NotificationPolicy {
    pub fn select(
        &mut self,
        preferences: &NotificationPreferences,
        outcome: Outcome<'_>,
        now: u64,
    ) -> Option<Notice> {
        let (key, title, body) = match outcome {
            Outcome::Renamed { id, name } => {
                if !preferences.successes || self.successes.iter().any(|seen| seen == id) {
                    return None;
                }
                self.successes.push_back(id.to_owned());
                if self.successes.len() > 128 {
                    self.successes.pop_front();
                }
                let body: String = name
                    .rsplit(['\\', '/'])
                    .next()
                    .unwrap_or("")
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(64)
                    .collect();
                return Some(Notice {
                    title: "Screenshot renamed",
                    body: if body.is_empty() {
                        "Open the app to view the renamed screenshot.".into()
                    } else {
                        body
                    },
                });
            }
            Outcome::RenameFailed => (
                "rename",
                "Rename needs attention",
                "Open Screenshot Renamer to review the failed rename and retry.",
            ),
            Outcome::AnalysisFailed => (
                "analysis",
                "Analysis needs attention",
                "Open Screenshot Renamer to review analysis settings and retry.",
            ),
            Outcome::WatcherFailed => (
                "watcher",
                "Screenshot monitoring stopped",
                "Open Screenshot Renamer to check the folder and resume monitoring.",
            ),
            Outcome::UndoFailed => (
                "undo",
                "Undo needs attention",
                "Open Screenshot Renamer to review the history and undo failure.",
            ),
        };
        if !preferences.errors {
            return None;
        }
        if self
            .errors
            .get(key)
            .is_some_and(|last| now.saturating_sub(*last) < 30_000)
        {
            return None;
        }
        self.errors.insert(key, now);
        Some(Notice {
            title,
            body: body.into(),
        })
    }
    pub fn deliver(
        &mut self,
        preferences: &NotificationPreferences,
        outcome: Outcome<'_>,
        now: u64,
        send: impl FnOnce(Notice) -> Result<(), String>,
    ) {
        if let Some(notice) = self.select(preferences, outcome, now) {
            let _ = send(notice);
        }
    }
}
pub struct NotificationState {
    policy: Mutex<NotificationPolicy>,
    settings: Arc<Mutex<crate::settings::ApplicationSettings>>,
    started: Instant,
}
impl NotificationState {
    pub fn new(settings: Arc<Mutex<crate::settings::ApplicationSettings>>) -> Self {
        Self {
            policy: Default::default(),
            settings,
            started: Instant::now(),
        }
    }
    pub fn deliver(&self, outcome: Outcome<'_>, send: impl FnOnce(Notice) -> Result<(), String>) {
        let Ok(mut policy) = self.policy.lock() else {
            return;
        };
        // Wait for earlier delivery before sampling the latest saved preference.
        let preferences = match self.settings.lock() {
            Ok(settings) => settings.notifications.clone(),
            Err(_) => return,
        };
        policy.deliver(
            &preferences,
            outcome,
            self.started.elapsed().as_millis() as u64,
            send,
        );
    }
}
pub fn publish(app: &AppHandle, outcome: Outcome<'_>) {
    // Only confirmed outcome facts cross this boundary; never raw error details.
    let (kind, id, name) = match outcome {
        Outcome::Renamed { id, name } => (0, id.to_owned(), name.to_owned()),
        Outcome::RenameFailed => (1, String::new(), String::new()),
        Outcome::AnalysisFailed => (2, String::new(), String::new()),
        Outcome::WatcherFailed => (3, String::new(), String::new()),
        Outcome::UndoFailed => (4, String::new(), String::new()),
    };
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(state) = app.try_state::<NotificationState>() else {
            return;
        };
        let outcome = match kind {
            0 => Outcome::Renamed {
                id: &id,
                name: &name,
            },
            1 => Outcome::RenameFailed,
            2 => Outcome::AnalysisFailed,
            3 => Outcome::WatcherFailed,
            _ => Outcome::UndoFailed,
        };
        state.deliver(outcome, |notice| {
            app.notification()
                .builder()
                .title(notice.title)
                .body(notice.body)
                .show()
                .map_err(|_| "Notification delivery unavailable.".into())
        });
    });
}
