use bar_core::{
    host::*,
    projection::*,
    threads::Configuration,
    usage::{self, Period, PriceBook, Window},
};
use chrono::{TimeZone, Utc};
use rusqlite::Connection;
use std::{collections::HashMap, fs, path::PathBuf, sync::Arc};
use tempfile::TempDir;

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn frozen_now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 6, 9, 8, 0).unwrap()
}

fn setup_fixtures() -> (TempDir, Configuration) {
    let dir = TempDir::new().unwrap();

    // Isolate local provider roots (opencode/antigravity session stores) from
    // the developer's real home: the shared host runtime discovers them for
    // usage scans, and without this the goldens absorb machine-local usage
    // data and stop being reproducible.
    std::env::set_var("OPENCODE_DATA_DIR", dir.path().join("isolated-local-roots"));
    std::env::set_var("ANTIGRAVITY_DATA_DIR", dir.path().join("isolated-local-roots"));

    // 1. Thread database from threads.sql (containing hostile markup strings)
    let db = Connection::open(dir.path().join("statev2.sqlite")).unwrap();
    db.execute_batch(&fs::read_to_string(fixtures_root().join("threads.sql")).unwrap())
        .unwrap();

    // 2. Limits cache
    let caches = dir.path().join("caches");
    fs::create_dir_all(&caches).unwrap();
    fs::copy(
        fixtures_root().join("limits/codex.json"),
        caches.join("codex.json"),
    )
    .unwrap();

    // 3. Usage scan cache and pricing
    let usage_src = fixtures_root().join("usage/state");
    let fixed_mtime = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1793956080);
    for entry in fs::read_dir(&usage_src).unwrap() {
        let entry = entry.unwrap();
        let dest = dir.path().join(entry.file_name());
        fs::copy(entry.path(), &dest).unwrap();
        if let Ok(file) = fs::File::options().write(true).open(&dest) {
            let _ = file.set_times(fs::FileTimes::new().set_modified(fixed_mtime));
        }
    }

    let config = Configuration::from_environment(Some(dir.path().to_owned()));
    (dir, config)
}

#[test]
fn differential_app_snapshot_and_state_projection_goldens() {
    let (_dir, config) = setup_fixtures();
    let sink = Arc::new(NoopEventSink);
    let favicons_dir = TempDir::new().unwrap();
    let now = frozen_now();
    let tz = chrono_tz::UTC;

    // Build Tauri runtime and Bridge runtime
    let tauri_runtime = HostRuntime::new(
        config.clone(),
        SnapshotMode::Tauri,
        Some(now),
        tz,
        true,
        false,
        favicons_dir.path().to_path_buf(),
        sink.clone(),
    );

    let bridge_runtime = HostRuntime::new(
        config.clone(),
        SnapshotMode::HostBridge,
        Some(now),
        tz,
        true,
        false,
        favicons_dir.path().to_path_buf(),
        sink.clone(),
    );

    // Refresh threads and limits
    tauri_runtime.refresh_threads_sync();
    bridge_runtime.refresh_threads_sync();
    tauri_runtime.refresh_limits_sync(true);
    bridge_runtime.refresh_limits_sync(true);

    // Refresh usage
    let _ = tauri_runtime.set_view(
        Some("usage".into()),
        Some("cost".into()),
        Some("24h".into()),
        Some(true),
    );
    let _ = bridge_runtime.set_view(
        Some("usage".into()),
        Some("cost".into()),
        Some("24h".into()),
        Some(true),
    );
    tauri_runtime.refresh_visible_sync();
    bridge_runtime.refresh_visible_sync();

    let tauri_snap = tauri_runtime.snapshot();
    let bridge_snap = bridge_runtime.snapshot();

    // Verify documented deltas
    assert!(tauri_snap.update.is_some());
    assert!(bridge_snap.update.is_none());
    assert!(bridge_snap.summary.is_some());
    assert!(tauri_snap.summary.is_none());

    let mut tauri_val = serde_json::to_value(&tauri_snap).unwrap();
    let mut bridge_val = serde_json::to_value(&bridge_snap).unwrap();

    // 1. Remove summary (delta 3)
    bridge_val.as_object_mut().unwrap().remove("summary");

    // 2. Remove update (delta 2)
    tauri_val.as_object_mut().unwrap().remove("update");

    // 3. Normalize favicons (delta 1)
    for thread in tauri_val["threads"].as_array_mut().unwrap() {
        thread.as_object_mut().unwrap().remove("projectFavicon");
    }
    for thread in bridge_val["threads"].as_array_mut().unwrap() {
        thread.as_object_mut().unwrap().remove("projectFavicon");
    }

    // Byte-identical except documented deltas
    assert_eq!(tauri_val, bridge_val);

    // Generate snapshot JSON strings
    let tauri_json = serde_json::to_string_pretty(&tauri_snap).unwrap() + "\n";
    let bridge_json = serde_json::to_string_pretty(&bridge_snap).unwrap() + "\n";

    // Build multi-period usage reports for StateProjection
    let usage_src = fixtures_root().join("usage/state");
    let dataset =
        usage::decode_cache(&fs::read(usage_src.join("usage-scan-cache-v11.json")).unwrap())
            .unwrap();
    let prices = PriceBook::load(&usage_src);
    let mut usage_reports = HashMap::new();
    for period in [Period::Day, Period::Week, Period::Month, Period::Quarter] {
        let r = usage::UsageReport::build(&dataset, &Window::new(period, now, tz), &prices);
        usage_reports.insert(period, r);
    }

    let view = ViewState {
        visible: true,
        screen: "usage".into(),
        metric: "cost".into(),
        period: "24h".into(),
        generation: 1,
    };

    let mut projection = StateProjection::new();
    let projected = projection.project(&bridge_snap, &usage_reports, &view, tz);

    // Verify dirty flags initial tick
    assert!(projected.dirty_state);
    assert!(projected.dirty_limits);
    assert!(projected.dirty_view);
    assert!(projected.dirty_usage);

    // Second tick with same inputs -> all dirty flags false
    let tick2 = projection.project(&bridge_snap, &usage_reports, &view, tz);
    assert!(!tick2.dirty_state);
    assert!(!tick2.dirty_limits);
    assert!(!tick2.dirty_view);
    assert!(!tick2.dirty_usage);

    // Hostile string verification: verify hostile markup survives into state_json
    assert!(projected.state_json.contains("Review a <b>proposed</b> action"));
    assert!(projected.state_json.contains("Answer a <img onerror=alert(1)> question"));
    assert!(projected.state_json.contains("Waiting on \u{202E}another thread") || projected.state_json.contains("Waiting on \\u202eanother thread"));

    // Additive reconciliation verification in usage_json
    let parsed_usage: serde_json::Value = serde_json::from_str(&projected.usage_json).unwrap();
    for period_key in ["24h", "7d", "30d", "90d"] {
        let p = &parsed_usage["periods"][period_key];
        let period_tokens = p["processedTokens"].as_i64().unwrap();

        // 1. Check timeline slot reconciliation
        for slot in p["timeline"].as_array().unwrap() {
            let slot_cost = slot["costUsd"].as_f64().unwrap();
            let slot_tokens = slot["tokens"].as_i64().unwrap();
            let sum_provider_cost: f64 = slot["providers"]
                .as_array()
                .unwrap()
                .iter()
                .map(|pr| pr["costUsd"].as_f64().unwrap())
                .sum();
            let sum_provider_tokens: i64 = slot["providers"]
                .as_array()
                .unwrap()
                .iter()
                .map(|pr| pr["tokens"].as_i64().unwrap())
                .sum();
            assert!((slot_cost - sum_provider_cost).abs() < 1e-6);
            assert_eq!(slot_tokens, sum_provider_tokens);
        }

        // 2. Check period providers reconciliation
        let sum_period_provider_tokens: i64 = p["providers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pr| pr["tokens"].as_i64().unwrap())
            .sum();
        assert_eq!(period_tokens, sum_period_provider_tokens);

        // 3. Check period models reconciliation
        let sum_period_model_tokens: i64 = p["models"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["tokens"].as_i64().unwrap())
            .sum();
        assert_eq!(period_tokens, sum_period_model_tokens);
    }

    // Golden directories & files
    let expected_dir = fixtures_root().join("expected");
    let omarchy_dir = expected_dir.join("omarchy");
    let snapshot_tauri_path = expected_dir.join("snapshot-tauri.json");
    let snapshot_bridge_path = expected_dir.join("snapshot-bridge.json");
    let omarchy_state_path = omarchy_dir.join("state.json");
    let omarchy_usage_path = omarchy_dir.join("usage.json");
    let omarchy_limits_path = omarchy_dir.join("limits.json");
    let omarchy_view_path = omarchy_dir.join("view.json");

    if std::env::var("REGENERATE_GOLDENS").as_deref() == Ok("1") {
        fs::create_dir_all(&omarchy_dir).unwrap();
        fs::write(&snapshot_tauri_path, &tauri_json).unwrap();
        fs::write(&snapshot_bridge_path, &bridge_json).unwrap();
        fs::write(&omarchy_state_path, &projected.state_json).unwrap();
        fs::write(&omarchy_usage_path, &projected.usage_json).unwrap();
        fs::write(&omarchy_limits_path, &projected.limits_json).unwrap();
        fs::write(&omarchy_view_path, &projected.view_json).unwrap();
    } else {
        let expected_tauri = fs::read_to_string(&snapshot_tauri_path).expect("snapshot-tauri.json golden missing");
        if expected_tauri != tauri_json {
            for (idx, (l1, l2)) in expected_tauri.lines().zip(tauri_json.lines()).enumerate() {
                if l1 != l2 {
                    eprintln!("Mismatch at line {}:\nExpected: {}\nActual:   {}", idx + 1, l1, l2);
                    break;
                }
            }
        }
        assert_eq!(expected_tauri, tauri_json);
        assert_eq!(
            fs::read_to_string(&snapshot_bridge_path).expect("snapshot-bridge.json golden missing"),
            bridge_json
        );
        assert_eq!(
            fs::read_to_string(&omarchy_state_path).expect("omarchy/state.json golden missing"),
            projected.state_json
        );
        assert_eq!(
            fs::read_to_string(&omarchy_usage_path).expect("omarchy/usage.json golden missing"),
            projected.usage_json
        );
        assert_eq!(
            fs::read_to_string(&omarchy_limits_path).expect("omarchy/limits.json golden missing"),
            projected.limits_json
        );
        assert_eq!(
            fs::read_to_string(&omarchy_view_path).expect("omarchy/view.json golden missing"),
            projected.view_json
        );
    }
}
