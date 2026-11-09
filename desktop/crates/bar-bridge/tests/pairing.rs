use std::{
    fs,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tempfile::TempDir;

#[test]
fn pairing_subcommands_lifecycle_and_watch_synchronization() {
    let bin_path = env!("CARGO_BIN_EXE_t3code-bar-bridge");
    let temp_dir = TempDir::new().expect("temp dir");
    let state_dir = temp_dir.path().join("state");
    fs::create_dir_all(&state_dir).expect("create state dir");

    // Disposable keyring namespace (valid UUID)
    let keyring_ns = "a1b2c3d4-e5f6-4a1b-8c2d-3e4f5a6b7c8d";

    // Create server-runtime.json with current PID and a mock port
    let runtime_json = serde_json::json!({
        "pid": std::process::id(),
        "port": 49152,
    });
    fs::write(
        state_dir.join("server-runtime.json"),
        serde_json::to_string(&runtime_json).unwrap(),
    )
    .expect("write server-runtime.json");

    // 1. Initial connect with non-pairing bearer/synthetic token
    let output_connect = Command::new(bin_path)
        .arg("connect")
        .arg("synthetic-access-token-12345")
        .env("T3_BAR_FIXTURE_STATE", "1")
        .env("T3_BAR_FIXTURE_KEYRING_NAMESPACE", keyring_ns)
        .env("T3_CODE_BAR_STATE_DIR", &state_dir)
        .output()
        .expect("run connect");

    assert!(
        output_connect.status.success(),
        "connect failed: {}",
        String::from_utf8_lossy(&output_connect.stderr)
    );
    let stdout_connect: serde_json::Value =
        serde_json::from_slice(&output_connect.stdout).expect("parse json connect stdout");
    assert_eq!(stdout_connect["ok"], true);
    assert_eq!(stdout_connect["status"], "connected");

    // 2. Reconnect
    let output_reconnect = Command::new(bin_path)
        .arg("reconnect")
        .env("T3_BAR_FIXTURE_STATE", "1")
        .env("T3_BAR_FIXTURE_KEYRING_NAMESPACE", keyring_ns)
        .env("T3_CODE_BAR_STATE_DIR", &state_dir)
        .output()
        .expect("run reconnect");

    assert!(
        output_reconnect.status.success(),
        "reconnect failed: {}",
        String::from_utf8_lossy(&output_reconnect.stderr)
    );
    let stdout_reconnect: serde_json::Value =
        serde_json::from_slice(&output_reconnect.stdout).expect("parse json reconnect stdout");
    assert_eq!(stdout_reconnect["ok"], true);
    assert_eq!(stdout_reconnect["status"], "reconnected");

    // 3. Disconnect
    let output_disconnect = Command::new(bin_path)
        .arg("disconnect")
        .env("T3_BAR_FIXTURE_STATE", "1")
        .env("T3_BAR_FIXTURE_KEYRING_NAMESPACE", keyring_ns)
        .env("T3_CODE_BAR_STATE_DIR", &state_dir)
        .output()
        .expect("run disconnect");

    assert!(
        output_disconnect.status.success(),
        "disconnect failed: {}",
        String::from_utf8_lossy(&output_disconnect.stderr)
    );
    let stdout_disconnect: serde_json::Value =
        serde_json::from_slice(&output_disconnect.stdout).expect("parse json disconnect stdout");
    assert_eq!(stdout_disconnect["ok"], true);
    assert_eq!(stdout_disconnect["status"], "disconnected");

    // 4. Reconnect after disconnect should fail because credential was cleared
    let output_reconnect_fail = Command::new(bin_path)
        .arg("reconnect")
        .env("T3_BAR_FIXTURE_STATE", "1")
        .env("T3_BAR_FIXTURE_KEYRING_NAMESPACE", keyring_ns)
        .env("T3_CODE_BAR_STATE_DIR", &state_dir)
        .output()
        .expect("run reconnect after disconnect");

    assert!(
        !output_reconnect_fail.status.success(),
        "reconnect after disconnect must fail when no saved credential exists"
    );
}

#[test]
fn watch_reflects_pairing_and_disconnect_within_one_second() {
    let bin_path = env!("CARGO_BIN_EXE_t3code-bar-bridge");
    let temp_dir = TempDir::new().expect("temp dir");
    let state_dir = temp_dir.path().join("state");
    fs::create_dir_all(&state_dir).expect("create state dir");

    let keyring_ns = "b2c3d4e5-f6a1-4b2c-9d3e-4f5a6b7c8d9e";

    let runtime_json = serde_json::json!({
        "pid": std::process::id(),
        "port": 49153,
    });
    fs::write(
        state_dir.join("server-runtime.json"),
        serde_json::to_string(&runtime_json).unwrap(),
    )
    .expect("write server-runtime.json");

    // Start watch without credentials (unpaired)
    let mut child = Command::new(bin_path)
        .arg("watch")
        .arg("--state-dir")
        .arg(&state_dir)
        .env("T3_BAR_FIXTURE_STATE", "1")
        .env("T3_BAR_FIXTURE_KEYRING_NAMESPACE", keyring_ns)
        .env("T3_CODE_BAR_STATE_DIR", &state_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn watch");

    let state_file = state_dir.join("state.json");

    // Wait for initial state to settle
    let start = Instant::now();
    let mut initial_settled = false;
    while start.elapsed() < Duration::from_secs(5) {
        if let Ok(content) = fs::read_to_string(&state_file) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if val.get("loading").and_then(|v| v.as_bool()) == Some(false) {
                    assert_eq!(val["connection"]["paired"], false);
                    initial_settled = true;
                    break;
                }
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    assert!(initial_settled, "initial state did not settle");

    // Now connect via CLI
    let output_connect = Command::new(bin_path)
        .arg("connect")
        .arg("synthetic-token-paired-test")
        .env("T3_BAR_FIXTURE_STATE", "1")
        .env("T3_BAR_FIXTURE_KEYRING_NAMESPACE", keyring_ns)
        .env("T3_CODE_BAR_STATE_DIR", &state_dir)
        .output()
        .expect("run connect");
    assert!(output_connect.status.success());

    // Watch should pick up the pairing within ~1-2 ticks
    let start_paired = Instant::now();
    let mut became_paired = false;
    while start_paired.elapsed() < Duration::from_secs(4) {
        if let Ok(content) = fs::read_to_string(&state_file) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if val["connection"]["paired"] == true {
                    became_paired = true;
                    break;
                }
            }
        }
        thread::sleep(Duration::from_millis(100));
    }

    assert!(became_paired, "watch did not reflect paired state within 4s");

    // Now disconnect via CLI
    let output_disconnect = Command::new(bin_path)
        .arg("disconnect")
        .env("T3_BAR_FIXTURE_STATE", "1")
        .env("T3_BAR_FIXTURE_KEYRING_NAMESPACE", keyring_ns)
        .env("T3_CODE_BAR_STATE_DIR", &state_dir)
        .output()
        .expect("run disconnect");
    assert!(output_disconnect.status.success());

    // Watch should pick up disconnect within ~1-2 ticks
    let start_unpaired = Instant::now();
    let mut became_unpaired = false;
    while start_unpaired.elapsed() < Duration::from_secs(4) {
        if let Ok(content) = fs::read_to_string(&state_file) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if val["connection"]["paired"] == false {
                    became_unpaired = true;
                    break;
                }
            }
        }
        thread::sleep(Duration::from_millis(100));
    }

    drop(child.stdin.take());
    let _ = child.kill();

    assert!(became_unpaired, "watch did not reflect disconnect within 4s");
}
