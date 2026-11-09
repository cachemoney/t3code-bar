use bar_core::attention::AttentionAlert;
use bar_core::host::HostEventSink;
use bar_core::threads::ThreadStatus;
use notify_rust::{Hint, Notification, Urgency};

pub fn build_attention_notification(alert: &AttentionAlert) -> Notification {
    let mut notification = Notification::new();

    let exec_argv = serde_json::to_string(&[
        "t3code-bar-bridge",
        "thread",
        alert.thread_id.as_str(),
        "open",
    ])
    .unwrap_or_else(|_| "[]".to_string());

    let urgency = match alert.status {
        ThreadStatus::Approval | ThreadStatus::Failed => Urgency::Critical,
        _ => Urgency::Normal,
    };

    let glyph = match alert.status {
        ThreadStatus::Approval => "!",
        ThreadStatus::Input => "?",
        ThreadStatus::Request => "💬",
        ThreadStatus::Limited => "⏳",
        ThreadStatus::Failed => "✕",
        _ => "•",
    };

    notification
        .appname("T3 Code Bar")
        .summary(alert.title())
        .body(&alert.body)
        .urgency(urgency)
        .hint(Hint::Custom("omarchy-exec-argv".into(), exec_argv))
        .hint(Hint::Custom("omarchy-glyph".into(), glyph.into()));

    notification
}

#[derive(Clone, Copy, Default)]
pub struct BridgeEventSink;

impl HostEventSink for BridgeEventSink {
    fn on_snapshot_changed(&self, _snapshot: &bar_core::host::AppSnapshot) {}
    fn on_tray_summary(&self, _summary: &bar_core::threads::ThreadSummary) {}
    fn on_settings_changed(&self, _settings: &bar_core::settings::Settings) {}

    fn on_attention_alerts(&self, alerts: &[AttentionAlert]) {
        for alert in alerts {
            let notification = build_attention_notification(alert);
            if let Err(e) = notification.show() {
                eprintln!("Failed to show notification: {e}");
            }
        }
    }
}
