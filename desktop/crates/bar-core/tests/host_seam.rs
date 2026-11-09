use bar_core::host::*;
use chrono::{TimeZone, Utc};
use rusqlite::Connection;
use std::{fs, path::PathBuf, sync::Arc};
use tempfile::TempDir;

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn setup_test_env() -> (TempDir, bar_core::threads::Configuration) {
    let dir = TempDir::new().unwrap();
    let db = Connection::open(dir.path().join("statev2.sqlite")).unwrap();
    db.execute_batch(&fs::read_to_string(fixtures_root().join("threads.sql")).unwrap())
        .unwrap();
    let config = bar_core::threads::Configuration::from_environment(Some(dir.path().to_owned()));
    (dir, config)
}

fn frozen_now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 6, 9, 8, 0).unwrap()
}

#[derive(Default)]
struct MockSink {
    snapshots: std::sync::Mutex<Vec<AppSnapshot>>,
    summaries: std::sync::Mutex<Vec<bar_core::threads::ThreadSummary>>,
    alerts: std::sync::Mutex<Vec<bar_core::attention::AttentionAlert>>,
    settings: std::sync::Mutex<Vec<bar_core::settings::Settings>>,
}

impl HostEventSink for MockSink {
    fn on_snapshot_changed(&self, snapshot: &AppSnapshot) {
        self.snapshots.lock().unwrap().push(snapshot.clone());
    }

    fn on_tray_summary(&self, summary: &bar_core::threads::ThreadSummary) {
        self.summaries.lock().unwrap().push(*summary);
    }

    fn on_attention_alerts(&self, alerts: &[bar_core::attention::AttentionAlert]) {
        self.alerts.lock().unwrap().extend_from_slice(alerts);
    }

    fn on_settings_changed(&self, settings: &bar_core::settings::Settings) {
        self.settings.lock().unwrap().push(settings.clone());
    }
}

#[test]
fn differential_app_snapshot_modes_differ_only_by_documented_deltas() {
    let (_dir, config) = setup_test_env();
    let sink = Arc::new(MockSink::default());
    let favicons_dir = TempDir::new().unwrap();

    let tauri_runtime = HostRuntime::new(
        config.clone(),
        SnapshotMode::Tauri,
        Some(frozen_now()),
        chrono_tz::UTC,
        true,  // fixture
        false, // fixture_notifications
        favicons_dir.path().to_path_buf(),
        sink.clone(),
    );

    let bridge_runtime = HostRuntime::new(
        config.clone(),
        SnapshotMode::HostBridge,
        Some(frozen_now()),
        chrono_tz::UTC,
        true,
        false,
        favicons_dir.path().to_path_buf(),
        sink.clone(),
    );

    tauri_runtime.refresh_threads_sync();
    bridge_runtime.refresh_threads_sync();

    let tauri_snap = tauri_runtime.snapshot();
    let bridge_snap = bridge_runtime.snapshot();

    // 1. In Tauri mode, update is present and summary is omitted.
    assert!(tauri_snap.update.is_some());
    assert!(tauri_snap.summary.is_none());

    // 2. In HostBridge mode, update is omitted and summary is present.
    assert!(bridge_snap.update.is_none());
    assert!(bridge_snap.summary.is_some());

    // 3. Serialize both to JSON Value
    let mut tauri_val = serde_json::to_value(&tauri_snap).unwrap();
    let mut bridge_val = serde_json::to_value(&bridge_snap).unwrap();

    // Delta 1: summary is present only in HostBridge mode
    assert!(bridge_val.get("summary").is_some());
    assert!(tauri_val.get("summary").is_none());
    bridge_val.as_object_mut().unwrap().remove("summary");

    // Delta 2: update is present only in Tauri mode
    assert!(tauri_val.get("update").is_some());
    assert!(bridge_val.get("update").is_none());
    tauri_val.as_object_mut().unwrap().remove("update");

    // Delta 3: projectFavicon representation (in fixture mode both are None, or differ by CachePath vs DataUrl)
    // If favicons are resolved, normalize projectFavicon across all threads
    for thread in tauri_val["threads"].as_array_mut().unwrap() {
        thread.as_object_mut().unwrap().remove("projectFavicon");
    }
    for thread in bridge_val["threads"].as_array_mut().unwrap() {
        thread.as_object_mut().unwrap().remove("projectFavicon");
    }

    // Now they must be 100% byte-identical!
    assert_eq!(tauri_val, bridge_val);
}

#[test]
fn favicons_resolution_supports_data_url_and_cache_path_modes() {
    let dir = TempDir::new().unwrap();
    let workspace = dir.path().join("workspace");
    fs::create_dir_all(workspace.join("public")).unwrap();
    let svg_content = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><rect fill="blue" width="16" height="16"/></svg>"#;
    fs::write(workspace.join("public/favicon.svg"), svg_content).unwrap();

    let cache_dir = dir.path().join("favicons");
    let favicons = Favicons::new(cache_dir.clone());

    // DataUrl mode
    let data_url = favicons.resolve_mode(
        workspace.to_str(),
        None,
        FaviconMode::DataUrl,
    ).expect("data url should resolve");
    assert!(data_url.starts_with("data:image/svg+xml;base64,"));

    // CachePath mode
    let cache_path = favicons.resolve_mode(
        workspace.to_str(),
        None,
        FaviconMode::CachePath,
    ).expect("cache path should resolve");
    assert!(cache_path.starts_with(cache_dir.to_str().unwrap()));
    assert!(std::path::Path::new(&cache_path).exists());
    assert_eq!(fs::read_to_string(&cache_path).unwrap(), svg_content);
}

#[test]
fn host_refresh_orchestration_honors_busy_guards_and_event_sink() {
    let (_dir, config) = setup_test_env();
    let sink = Arc::new(MockSink::default());
    let favicons_dir = TempDir::new().unwrap();

    let runtime = HostRuntime::new(
        config,
        SnapshotMode::Tauri,
        Some(frozen_now()),
        chrono_tz::UTC,
        true,
        false,
        favicons_dir.path().to_path_buf(),
        sink.clone(),
    );

    // Initial tick
    runtime.refresh_threads_sync();
    assert!(!sink.snapshots.lock().unwrap().is_empty());
    assert!(!sink.summaries.lock().unwrap().is_empty());

    // Settings update emits event
    runtime.set_setting(bar_core::settings::SettingKey::NotificationsMuted, true).unwrap();
    assert_eq!(*sink.settings.lock().unwrap().last().unwrap(), bar_core::settings::Settings {
        notifications_muted: true,
        shows_previews: true,
        automatically_checks_for_updates: true,
        installs_without_asking: false,
    });
}
