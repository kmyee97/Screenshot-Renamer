use screenshot_renamer_lib::notifications::{NotificationPolicy, NotificationPreferences, Outcome};
#[test]
fn errors_default_on_successes_opt_in_and_notices_do_not_leak_paths() {
    let mut policy = NotificationPolicy::default();
    let defaults = NotificationPreferences::default();
    assert!(policy
        .select(
            &defaults,
            Outcome::Renamed {
                id: "a",
                name: "C:\\Private\\receipt.png"
            },
            0
        )
        .is_none());
    let error = policy.select(&defaults, Outcome::RenameFailed, 0).unwrap();
    assert!(error.body.contains("Open"));
    assert!(!error.body.contains("Private"));
    let enabled = NotificationPreferences {
        errors: true,
        successes: true,
    };
    let success = policy
        .select(
            &enabled,
            Outcome::Renamed {
                id: "a",
                name: "C:\\Private\\receipt.png",
            },
            1,
        )
        .unwrap();
    assert_eq!(success.body, "receipt.png");
    assert!(policy
        .select(
            &enabled,
            Outcome::Renamed {
                id: "a",
                name: "receipt.png"
            },
            2
        )
        .is_none());
}
#[test]
fn repeated_errors_are_coalesced_and_delivery_failure_is_nonfatal() {
    let mut policy = NotificationPolicy::default();
    let preferences = NotificationPreferences::default();
    assert!(policy
        .select(&preferences, Outcome::RenameFailed, 0)
        .is_some());
    assert!(policy
        .select(&preferences, Outcome::RenameFailed, 10_000)
        .is_none());
    assert!(policy
        .select(&preferences, Outcome::RenameFailed, 30_000)
        .is_some());
    let disabled = NotificationPreferences {
        errors: false,
        successes: false,
    };
    assert!(policy
        .select(&disabled, Outcome::AnalysisFailed, 40_000)
        .is_none());
    policy.deliver(&preferences, Outcome::AnalysisFailed, 40_000, |_| {
        Err("Notifications disabled".into())
    });
    assert!(policy
        .select(&preferences, Outcome::AnalysisFailed, 40_001)
        .is_none());
}

#[test]
fn queued_delivery_uses_preferences_saved_while_it_waited() {
    use screenshot_renamer_lib::{notifications::NotificationState, settings::ApplicationSettings};
    use std::sync::{mpsc, Arc, Mutex};
    let mut settings = ApplicationSettings::default();
    settings.notifications.successes = true;
    let settings = Arc::new(Mutex::new(settings));
    let state = Arc::new(NotificationState::new(settings.clone()));
    let (blocked_tx, blocked_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let first = state.clone();
    let thread1 = std::thread::spawn(move || {
        first.deliver(Outcome::RenameFailed, |_| {
            blocked_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            Ok(())
        })
    });
    blocked_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    let (started_tx, started_rx) = mpsc::channel();
    let (sent_tx, sent_rx) = mpsc::channel();
    let second = state.clone();
    let thread2 = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        second.deliver(
            Outcome::Renamed {
                id: "queued",
                name: "private.png",
            },
            |notice| {
                sent_tx.send(notice.body).unwrap();
                Ok(())
            },
        )
    });
    started_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));
    settings.lock().unwrap().notifications.successes = false;
    release_tx.send(()).unwrap();
    thread1.join().unwrap();
    thread2.join().unwrap();
    assert!(
        sent_rx.try_recv().is_err(),
        "a disabled success notification was sent"
    );
}
