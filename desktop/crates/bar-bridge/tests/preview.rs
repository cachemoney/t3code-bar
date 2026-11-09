use rusqlite::Connection;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::Instant,
};
use tempfile::TempDir;

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn setup_fixture_db() -> TempDir {
    let dir = TempDir::new().unwrap();
    let db = Connection::open(dir.path().join("statev2.sqlite")).unwrap();
    db.execute_batch(&fs::read_to_string(fixtures_root().join("threads.sql")).unwrap())
        .unwrap();
    dir
}

#[test]
fn preview_subcommand_returns_preview_json_for_valid_thread() {
    let bin_path = env!("CARGO_BIN_EXE_t3code-bar-bridge");
    let fixture_dir = setup_fixture_db();
    let state_dir = TempDir::new().unwrap();

    let output = Command::new(bin_path)
        .arg("preview")
        .arg("approval")
        .arg("--state-dir")
        .arg(state_dir.path())
        .env("T3_BAR_FIXTURE_STATE", fixture_dir.path())
        .env("T3_CODE_BAR_STATE_DIR", fixture_dir.path())
        .output()
        .expect("failed to execute binary");

    assert!(output.status.success(), "preview failed: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    let val: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON stdout");

    assert!(val.get("text").is_some(), "text field must be present");
    assert!(val["text"].as_str().unwrap().contains("The proposed change is ready for review"));
}

#[test]
fn preview_subcommand_rejects_unknown_thread() {
    let bin_path = env!("CARGO_BIN_EXE_t3code-bar-bridge");
    let fixture_dir = setup_fixture_db();
    let state_dir = TempDir::new().unwrap();

    let output = Command::new(bin_path)
        .arg("preview")
        .arg("unknown-thread-999")
        .arg("--state-dir")
        .arg(state_dir.path())
        .env("T3_BAR_FIXTURE_STATE", fixture_dir.path())
        .env("T3_CODE_BAR_STATE_DIR", fixture_dir.path())
        .output()
        .expect("failed to execute binary");

    assert!(!output.status.success(), "preview for unknown thread must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Unknown thread"), "stderr should report 'Unknown thread', got: {stderr}");
}

#[test]
fn preview_subcommand_returns_null_when_shows_previews_disabled() {
    let bin_path = env!("CARGO_BIN_EXE_t3code-bar-bridge");
    let fixture_dir = setup_fixture_db();
    let state_dir = TempDir::new().unwrap();

    // Write state.json with showsPreviews: false in state_dir
    let state_json = serde_json::json!({
        "loading": false,
        "settings": {
            "showsPreviews": false
        },
        "threads": [
            { "id": "approval", "title": "Review a proposed action" }
        ]
    });
    fs::write(state_dir.path().join("state.json"), state_json.to_string()).unwrap();

    let output = Command::new(bin_path)
        .arg("preview")
        .arg("approval")
        .arg("--state-dir")
        .arg(state_dir.path())
        .env("T3_BAR_FIXTURE_STATE", fixture_dir.path())
        .env("T3_CODE_BAR_STATE_DIR", fixture_dir.path())
        .output()
        .expect("failed to execute binary");

    assert!(output.status.success(), "preview should succeed when previews disabled");
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    assert_eq!(stdout.trim(), "null");
}

#[test]
fn preview_subcommand_honors_delay_ms() {
    let bin_path = env!("CARGO_BIN_EXE_t3code-bar-bridge");
    let fixture_dir = setup_fixture_db();
    let state_dir = TempDir::new().unwrap();

    let start = Instant::now();
    let output = Command::new(bin_path)
        .arg("preview")
        .arg("approval")
        .arg("--state-dir")
        .arg(state_dir.path())
        .env("T3_BAR_FIXTURE_STATE", fixture_dir.path())
        .env("T3_CODE_BAR_STATE_DIR", fixture_dir.path())
        .env("T3_BAR_FIXTURE_PREVIEW_DELAY_MS", "250")
        .output()
        .expect("failed to execute binary");

    let elapsed = start.elapsed();
    assert!(output.status.success());
    assert!(elapsed.as_millis() >= 200, "expected at least 200ms delay, got {}ms", elapsed.as_millis());
}
