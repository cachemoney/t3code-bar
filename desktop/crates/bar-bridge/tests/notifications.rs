use bar_bridge::notification::{build_attention_notification, BridgeEventSink};
use bar_core::attention::{AttentionAlert, AttentionTracker};
use bar_core::host::{HostEventSink, HostRuntime, SnapshotMode};
use bar_core::threads::{Configuration, ThreadStatus};
use notify_rust::{Hint, Urgency};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tempfile::TempDir;

#[test]
fn test_notification_construction_and_omarchy_hints() {
    let alert = AttentionAlert {
        thread_id: "thread-42".into(),
        body: "Review PR #12 changes".into(),
        status: ThreadStatus::Approval,
    };

    let notification = build_attention_notification(&alert);

    assert_eq!(notification.appname, "T3 Code Bar");
    assert_eq!(notification.summary, "Approval needed");
    assert_eq!(notification.body, "Review PR #12 changes");

    let hints: Vec<&Hint> = notification.get_hints().collect();

    // Verify urgency is Critical for Approval
    let has_critical = hints.iter().any(|h| matches!(h, Hint::Urgency(Urgency::Critical)));
    assert!(has_critical, "Expected Urgency::Critical for Approval status");

    // Verify omarchy-exec-argv hint
    let exec_argv = hints.iter().find_map(|h| match h {
        Hint::Custom(k, v) if k == "omarchy-exec-argv" => Some(v.as_str()),
        _ => None,
    });
    assert_eq!(
        exec_argv,
        Some(r#"["t3code-bar-bridge","thread","thread-42","open"]"#)
    );

    // Verify omarchy-glyph hint is not present
    let glyph = hints.iter().find_map(|h| match h {
        Hint::Custom(k, v) if k == "omarchy-glyph" => Some(v.as_str()),
        _ => None,
    });
    assert_eq!(glyph, None, "omarchy-glyph hint should not be set");

    // Status: Failed -> Critical
    let alert_failed = AttentionAlert {
        thread_id: "thread-failed".into(),
        body: "Test run failed".into(),
        status: ThreadStatus::Failed,
    };
    let n_failed = build_attention_notification(&alert_failed);
    assert_eq!(n_failed.summary, "Failed needed");
    let has_critical = n_failed.get_hints().any(|h| matches!(h, Hint::Urgency(Urgency::Critical)));
    assert!(has_critical, "Expected Urgency::Critical for Failed status");

    // Status: Input -> Normal
    let alert_input = AttentionAlert {
        thread_id: "thread-input".into(),
        body: "Provide confirmation".into(),
        status: ThreadStatus::Input,
    };
    let n_input = build_attention_notification(&alert_input);
    assert_eq!(n_input.summary, "Input needed");
    let has_normal = n_input.get_hints().any(|h| matches!(h, Hint::Urgency(Urgency::Normal)));
    assert!(has_normal, "Expected Urgency::Normal for Input status");
}

#[test]
fn test_notification_muting_semantics() {
    let mut tracker = AttentionTracker::default();

    // 1. Initial snapshot seeds known threads; returns no alerts
    let initial = [("t1", "Thread 1", ThreadStatus::Working)];
    let alerts = tracker.update(initial.iter().copied(), false);
    assert!(alerts.is_empty());

    // 2. Unmuted transition to Approval produces alert
    let step2 = [("t1", "Thread 1", ThreadStatus::Approval)];
    let alerts = tracker.update(step2.iter().copied(), false);
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].thread_id, "t1");
    assert_eq!(alerts[0].status, ThreadStatus::Approval);

    // 3. Muted transition to Input produces NO alert
    let step3 = [("t1", "Thread 1", ThreadStatus::Input)];
    let alerts = tracker.update(step3.iter().copied(), true); // muted = true
    assert!(alerts.is_empty(), "Muted tracker must not emit alerts");
}

#[derive(Default)]
struct AlertCounterSink {
    alert_count: AtomicUsize,
}

impl HostEventSink for AlertCounterSink {
    fn on_attention_alerts(&self, alerts: &[AttentionAlert]) {
        self.alert_count.fetch_add(alerts.len(), Ordering::SeqCst);
    }
}

#[test]
fn test_host_runtime_fixture_notification_muting_flags() {
    let tmp = TempDir::new().unwrap();
    let config = Configuration::from_environment(Some(tmp.path().to_path_buf()));
    let favicons_dir = tmp.path().join("favicons");

    let counter_muted = Arc::new(AlertCounterSink::default());
    // Runtime with fixture: true, fixture_notifications: false -> muted
    let runtime_muted = HostRuntime::new(
        config.clone(),
        SnapshotMode::HostBridge,
        None,
        chrono_tz::UTC,
        true,  // fixture = true
        false, // fixture_notifications = false
        favicons_dir.clone(),
        counter_muted.clone(),
    );

    runtime_muted.refresh_threads_sync();
    assert_eq!(
        counter_muted.alert_count.load(Ordering::SeqCst),
        0,
        "Fixture without fixture_notifications must not emit alerts"
    );
}

#[test]
fn test_bridge_event_sink_does_not_panic() {
    let sink = BridgeEventSink;
    let alerts = [AttentionAlert {
        thread_id: "t-test".into(),
        body: "Test Body".into(),
        status: ThreadStatus::Approval,
    }];
    // Even if no DBus session exists in CI or headless, on_attention_alerts handles errors gracefully
    sink.on_attention_alerts(&alerts);
}
