use rusqlite::Connection;
use std::{
    fs,
    path::PathBuf,
    process::Command,
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
    fs::write(dir.path().join("environment-id"), "env-123\n").unwrap();
    fs::write(
        dir.path().join("server-runtime.json"),
        r#"{"origin":"http://127.0.0.1:4567","port":4567}"#,
    )
    .unwrap();
    dir
}

#[test]
fn thread_subcommand_rejects_unknown_thread() {
    let bin_path = env!("CARGO_BIN_EXE_t3code-bar-bridge");
    let fixture_dir = setup_fixture_db();
    let state_dir = TempDir::new().unwrap();

    let output = Command::new(bin_path)
        .arg("thread")
        .arg("unknown-999")
        .arg("open")
        .arg("--state-dir")
        .arg(state_dir.path())
        .env("T3_BAR_FIXTURE_STATE", fixture_dir.path())
        .env("T3_CODE_BAR_STATE_DIR", fixture_dir.path())
        .output()
        .expect("failed to execute binary");

    assert!(!output.status.success(), "thread with unknown id should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Unknown thread"), "stderr should report 'Unknown thread', got: {stderr}");
}

#[test]
fn thread_subcommand_rejects_unknown_action() {
    let bin_path = env!("CARGO_BIN_EXE_t3code-bar-bridge");
    let fixture_dir = setup_fixture_db();
    let state_dir = TempDir::new().unwrap();

    let output = Command::new(bin_path)
        .arg("thread")
        .arg("approval")
        .arg("invalid-action")
        .arg("--state-dir")
        .arg(state_dir.path())
        .env("T3_BAR_FIXTURE_STATE", fixture_dir.path())
        .env("T3_CODE_BAR_STATE_DIR", fixture_dir.path())
        .output()
        .expect("failed to execute binary");

    assert!(!output.status.success(), "thread with unknown action should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Unknown thread action"), "stderr should report 'Unknown thread action', got: {stderr}");
}

#[test]
fn thread_subcommand_open_succeeds_with_json_output() {
    let bin_path = env!("CARGO_BIN_EXE_t3code-bar-bridge");
    let fixture_dir = setup_fixture_db();
    let state_dir = TempDir::new().unwrap();

    let output = Command::new(bin_path)
        .arg("thread")
        .arg("approval")
        .arg("open")
        .arg("--state-dir")
        .arg(state_dir.path())
        .env("T3_BAR_FIXTURE_STATE", fixture_dir.path())
        .env("T3_CODE_BAR_STATE_DIR", fixture_dir.path())
        .env("T3_BAR_FIXTURE_OPENER", "noop")
        .output()
        .expect("failed to execute binary");

    assert!(output.status.success(), "thread open failed: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    let val: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON stdout");
    assert_eq!(val["ok"], true);
    assert!(val["url"].as_str().unwrap().starts_with("t3code://threads/env-123/approval"));
}

#[test]
fn thread_subcommand_browser_succeeds_with_loopback_url() {
    let bin_path = env!("CARGO_BIN_EXE_t3code-bar-bridge");
    let fixture_dir = setup_fixture_db();
    let state_dir = TempDir::new().unwrap();

    let output = Command::new(bin_path)
        .arg("thread")
        .arg("approval")
        .arg("browser")
        .arg("--state-dir")
        .arg(state_dir.path())
        .env("T3_BAR_FIXTURE_STATE", fixture_dir.path())
        .env("T3_CODE_BAR_STATE_DIR", fixture_dir.path())
        .env("T3_BAR_FIXTURE_OPENER", "noop")
        .output()
        .expect("failed to execute binary");

    assert!(output.status.success(), "thread browser failed: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    let val: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON stdout");
    assert_eq!(val["ok"], true);
    assert_eq!(val["url"], "http://127.0.0.1:4567/env-123/approval");
}

#[test]
fn thread_subcommand_copy_succeeds_with_json_output() {
    let bin_path = env!("CARGO_BIN_EXE_t3code-bar-bridge");
    let fixture_dir = setup_fixture_db();
    let state_dir = TempDir::new().unwrap();

    let output = Command::new(bin_path)
        .arg("thread")
        .arg("approval")
        .arg("copy")
        .arg("--state-dir")
        .arg(state_dir.path())
        .env("T3_BAR_FIXTURE_STATE", fixture_dir.path())
        .env("T3_CODE_BAR_STATE_DIR", fixture_dir.path())
        .output()
        .expect("failed to execute binary");

    assert!(output.status.success(), "thread copy failed: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    let val: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON stdout");
    assert_eq!(val["ok"], true);
    assert!(val["url"].as_str().unwrap().starts_with("t3code://threads/env-123/approval"));
}

#[test]
fn new_thread_subcommand_opens_new_thread_url() {
    let bin_path = env!("CARGO_BIN_EXE_t3code-bar-bridge");
    let fixture_dir = setup_fixture_db();
    let state_dir = TempDir::new().unwrap();

    let output = Command::new(bin_path)
        .arg("new-thread")
        .arg("--state-dir")
        .arg(state_dir.path())
        .env("T3_BAR_FIXTURE_STATE", fixture_dir.path())
        .env("T3_CODE_BAR_STATE_DIR", fixture_dir.path())
        .env("T3_BAR_FIXTURE_OPENER", "noop")
        .output()
        .expect("failed to execute binary");

    assert!(output.status.success(), "new-thread failed: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    let val: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON stdout");
    assert_eq!(val["ok"], true);
    assert_eq!(val["url"], "t3code://new");
}
