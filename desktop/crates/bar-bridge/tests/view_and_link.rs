use std::{fs, process::Command};
use tempfile::TempDir;

#[test]
fn set_view_updates_view_json_atomically_with_validation_and_generations() {
    let bin_path = env!("CARGO_BIN_EXE_t3code-bar-bridge");
    let temp_dir = TempDir::new().expect("create temp dir");
    let state_dir = temp_dir.path().to_path_buf();

    // 1. Valid initial set-view
    let output = Command::new(bin_path)
        .arg("set-view")
        .arg("--visible")
        .arg("--screen")
        .arg("usage")
        .arg("--metric")
        .arg("cost")
        .arg("--period")
        .arg("7d")
        .arg("--state-dir")
        .arg(&state_dir)
        .output()
        .expect("run set-view");

    assert!(output.status.success(), "set-view should succeed: {:?}", output);
    let out1: serde_json::Value = serde_json::from_slice(&output.stdout).expect("set-view stdout should be valid JSON");
    assert_eq!(out1, serde_json::json!({"ok": true}));

    let view_path = state_dir.join("view.json");
    assert!(view_path.exists(), "view.json should be created");
    let content = fs::read_to_string(&view_path).expect("read view.json");
    let val: serde_json::Value = serde_json::from_str(&content).expect("parse view.json");
    assert_eq!(val["visible"], true);
    assert_eq!(val["screen"], "usage");
    assert_eq!(val["metric"], "cost");
    assert_eq!(val["period"], "7d");
    assert_eq!(val["generation"], 1);

    // 2. Second update increments generation and preserves unspecified fields
    let output2 = Command::new(bin_path)
        .arg("set-view")
        .arg("--metric")
        .arg("tokens")
        .arg("--period")
        .arg("30d")
        .arg("--state-dir")
        .arg(&state_dir)
        .output()
        .expect("run second set-view");

    assert!(output2.status.success());
    let content2 = fs::read_to_string(&view_path).expect("read view.json");
    let val2: serde_json::Value = serde_json::from_str(&content2).expect("parse view.json");
    assert_eq!(val2["visible"], true);
    assert_eq!(val2["screen"], "usage");
    assert_eq!(val2["metric"], "tokens");
    assert_eq!(val2["period"], "30d");
    assert_eq!(val2["generation"], 2);

    // 3. Validation failures reject invalid values with non-zero exit code
    let bad_screen = Command::new(bin_path)
        .arg("set-view")
        .arg("--screen")
        .arg("invalid")
        .arg("--state-dir")
        .arg(&state_dir)
        .output()
        .unwrap();
    assert!(!bad_screen.status.success());

    let bad_metric = Command::new(bin_path)
        .arg("set-view")
        .arg("--metric")
        .arg("bad_metric")
        .arg("--state-dir")
        .arg(&state_dir)
        .output()
        .unwrap();
    assert!(!bad_metric.status.success());

    let bad_period = Command::new(bin_path)
        .arg("set-view")
        .arg("--period")
        .arg("bad_period")
        .arg("--state-dir")
        .arg(&state_dir)
        .output()
        .unwrap();
    assert!(!bad_period.status.success());
}

#[test]
fn open_link_validates_against_limits_snapshot_links() {
    let bin_path = env!("CARGO_BIN_EXE_t3code-bar-bridge");
    let temp_dir = TempDir::new().expect("create temp dir");
    let state_dir = temp_dir.path().to_path_buf();

    // Create limits.json with an allowed link
    let limits_json = r#"{
  "groups": [],
  "notices": [],
  "links": [
    { "label": "Manage limits", "url": "https://example.invalid/limits" }
  ],
  "checkedAt": "2026-11-08T13:55:00Z"
}"#;
    fs::write(state_dir.join("limits.json"), limits_json).expect("write limits.json");

    // 1. Rejected URL
    let bad_output = Command::new(bin_path)
        .arg("open-link")
        .arg("https://malicious.invalid/evil")
        .arg("--state-dir")
        .arg(&state_dir)
        .output()
        .expect("run open-link with bad url");

    assert!(!bad_output.status.success());
    let stderr = String::from_utf8_lossy(&bad_output.stderr);
    assert!(
        stderr.contains("Link is not in the current limits snapshot"),
        "stderr should reject unknown link: {}",
        stderr
    );

    // 2. Allowed URL with mock opener
    let good_output = Command::new(bin_path)
        .arg("open-link")
        .arg("https://example.invalid/limits")
        .arg("--state-dir")
        .arg(&state_dir)
        .env("T3_BAR_OPENER_COMMAND", "true")
        .output()
        .expect("run open-link with allowed url");

    assert!(good_output.status.success(), "open-link should succeed: {:?}", good_output);
    let out_link: serde_json::Value = serde_json::from_slice(&good_output.stdout).expect("open-link stdout should be valid JSON");
    assert_eq!(out_link, serde_json::json!({"ok": true, "url": "https://example.invalid/limits"}));
}

#[test]
fn watch_view_gating_scans_usage_only_when_usage_screen_and_not_limits() {
    let bin_path = env!("CARGO_BIN_EXE_t3code-bar-bridge");
    let temp_dir = TempDir::new().expect("create temp dir");
    let state_dir = temp_dir.path().to_path_buf();

    // 1. Initial view is screen = "usage", metric = "limits", visible = true
    let view_limits = serde_json::json!({
        "visible": true,
        "screen": "usage",
        "metric": "limits",
        "period": "24h",
        "generation": 1,
    });
    fs::write(state_dir.join("view.json"), serde_json::to_string(&view_limits).unwrap()).unwrap();

    let mut child = Command::new(bin_path)
        .arg("watch")
        .arg("--state-dir")
        .arg(&state_dir)
        .env("T3_BAR_FIXTURE_STATE", "1")
        .env("T3_BAR_FROZEN_NOW", "2026-11-06T09:08:00+00:00")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .expect("spawn watch");

    // Wait for initial poll
    let state_file = state_dir.join("state.json");
    let usage_file = state_dir.join("usage.json");
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(3) {
        if state_file.exists() {
            if let Ok(c) = fs::read_to_string(&state_file) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&c) {
                    if v.get("loading").and_then(|x| x.as_bool()) == Some(false) {
                        break;
                    }
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    // Under limits metric, usage.json should NOT be written by watch
    assert!(
        !usage_file.exists(),
        "usage.json should not exist under metric == limits"
    );

    // 2. Now switch view to metric = "cost" via set-view
    let set_view_output = Command::new(bin_path)
        .arg("set-view")
        .arg("--metric")
        .arg("cost")
        .arg("--state-dir")
        .arg(&state_dir)
        .output()
        .expect("run set-view to cost");
    assert!(set_view_output.status.success());

    // Wait up to 4s for watch tick to detect change and write usage.json
    let start2 = std::time::Instant::now();
    let mut scanned_usage = false;
    while start2.elapsed() < std::time::Duration::from_secs(4) {
        if usage_file.exists() {
            scanned_usage = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    drop(child.stdin.take());
    let _ = child.wait();

    assert!(scanned_usage, "usage.json should be scanned once metric switches to cost");
}
