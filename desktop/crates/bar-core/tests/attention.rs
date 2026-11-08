#[path = "../src/attention.rs"]
mod attention;
use attention::AttentionTracker;
use bar_threads::ThreadStatus::*;

#[test]
fn first_snapshot_is_quiet_but_new_and_changed_attention_threads_alert() {
    let mut tracker = AttentionTracker::default();
    assert!(tracker.update([("a", "first", Approval)], false).is_empty());
    assert!(tracker.update([("a", "first", Approval)], false).is_empty());
    let alerts = tracker.update([("a", "first", Input), ("b", "second", Request)], false);
    assert_eq!(alerts.len(), 2);
    assert_eq!(alerts[0].title(), "Input needed");
    assert_eq!(alerts[1].title(), "Guest request");
    assert_eq!(alerts[1].thread_id, "b");
}
#[test]
fn muted_snapshots_advance_history_and_removed_threads_are_forgotten() {
    let mut tracker = AttentionTracker::default();
    tracker.update([("a", "first", Working)], false);
    assert!(tracker.update([("a", "first", Approval)], true).is_empty());
    assert!(tracker.update([("a", "first", Approval)], false).is_empty());
    assert!(tracker.update([], false).is_empty());
    assert_eq!(
        tracker.update([("a", "first", Limited)], false)[0].title(),
        "Usage limit reached"
    );
}
