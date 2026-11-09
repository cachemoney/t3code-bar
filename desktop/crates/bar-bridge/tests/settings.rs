use std::{
    fs,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tempfile::TempDir;

#[test]
fn set_setting_updates_settings_store_and_rejects_invalid_inputs() {
    let bin_path = env!("CARGO_BIN_EXE_t3code-bar-bridge");
    let temp_dir = TempDir::new().expect("temp dir");
    let settings_path = temp_dir.path().join("tauri-bar-settings.json");

    // 1. Initial set-setting notificationsMuted true
    let output1 = Command::new(bin_path)
        .arg("set-setting")
        .arg("notificationsMuted")
        .arg("true")
        .env("T3_BAR_FIXTURE_SETTINGS", &settings_path)
        .output()
        .expect("run set-setting");

    assert!(
        output1.status.success(),
        "set-setting failed: {}",
        String::from_utf8_lossy(&output1.stderr)
    );
    let stdout1: serde_json::Value =
        serde_json::from_slice(&output1.stdout).expect("valid json stdout");
    assert_eq!(stdout1["ok"], true);
    assert_eq!(stdout1["key"], "notificationsMuted");
    assert_eq!(stdout1["value"], true);

    // Verify file content
    let content1 = fs::read_to_string(&settings_path).expect("read settings file");
    let val1: serde_json::Value = serde_json::from_str(&content1).expect("parse settings json");
    assert_eq!(val1["notificationsMuted"], true);
    assert_eq!(val1["showsPreviews"], true); // default

    // 2. set-setting showsPreviews false (using kebab-case alias)
    let output2 = Command::new(bin_path)
        .arg("set-setting")
        .arg("shows-previews")
        .arg("false")
        .env("T3_BAR_FIXTURE_SETTINGS", &settings_path)
        .output()
        .expect("run set-setting 2");

    assert!(output2.status.success());
    let content2 = fs::read_to_string(&settings_path).expect("read settings file");
    let val2: serde_json::Value = serde_json::from_str(&content2).expect("parse settings json");
    assert_eq!(val2["notificationsMuted"], true);
    assert_eq!(val2["showsPreviews"], false);

    // 3. Reject invalid key (Tauri-only updater settings are not supported in bridge)
    let output_bad_key = Command::new(bin_path)
        .arg("set-setting")
        .arg("automaticallyChecksForUpdates")
        .arg("true")
        .env("T3_BAR_FIXTURE_SETTINGS", &settings_path)
        .output()
        .expect("run set-setting bad key");
    assert!(
        !output_bad_key.status.success(),
        "unsupported keys must be rejected with nonzero exit"
    );

    // 4. Reject invalid value
    let output_bad_val = Command::new(bin_path)
        .arg("set-setting")
        .arg("notificationsMuted")
        .arg("not-a-boolean")
        .env("T3_BAR_FIXTURE_SETTINGS", &settings_path)
        .output()
        .expect("run set-setting bad val");
    assert!(
        !output_bad_val.status.success(),
        "invalid boolean value must fail with nonzero exit"
    );
}

#[test]
fn watch_reflects_setting_change_within_one_second_tick() {
    let bin_path = env!("CARGO_BIN_EXE_t3code-bar-bridge");
    let temp_dir = TempDir::new().expect("temp dir");
    let state_dir = temp_dir.path().join("state");
    let settings_path = temp_dir.path().join("tauri-bar-settings.json");

    // Spawn watch
    let mut child = Command::new(bin_path)
        .arg("watch")
        .arg("--state-dir")
        .arg(&state_dir)
        .env("T3_BAR_FIXTURE_STATE", "1")
        .env("T3_BAR_FIXTURE_SETTINGS", &settings_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn watch");

    let state_file = state_dir.join("state.json");

    // Wait until state settles with loading: false
    let start = Instant::now();
    let mut settled = false;
    while start.elapsed() < Duration::from_secs(5) {
        if let Ok(content) = fs::read_to_string(&state_file) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if val.get("loading").and_then(|v| v.as_bool()) == Some(false) {
                    assert_eq!(val["settings"]["notificationsMuted"], false);
                    settled = true;
                    break;
                }
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    assert!(settled, "state.json did not settle in time");

    // Mutate setting via set-setting
    let output = Command::new(bin_path)
        .arg("set-setting")
        .arg("notificationsMuted")
        .arg("true")
        .env("T3_BAR_FIXTURE_SETTINGS", &settings_path)
        .output()
        .expect("run set-setting");
    assert!(output.status.success());

    // Verify watch reflects changes within ~1s
    let start_sync = Instant::now();
    let mut synced = false;
    while start_sync.elapsed() < Duration::from_secs(3) {
        if let Ok(content) = fs::read_to_string(&state_file) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if val["settings"]["notificationsMuted"] == true {
                    synced = true;
                    break;
                }
            }
        }
        thread::sleep(Duration::from_millis(100));
    }

    drop(child.stdin.take());
    let _ = child.kill();

    assert!(synced, "watch did not reflect notificationsMuted change in state.json within 3s");
}
