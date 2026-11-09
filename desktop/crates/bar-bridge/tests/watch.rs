use std::{
    fs,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tempfile::TempDir;

#[test]
fn watch_subcommand_lifecycle_flock_and_change_gated_persistence() {
    let bin_path = env!("CARGO_BIN_EXE_t3code-bar-bridge");
    let temp_dir = TempDir::new().expect("create temp dir");
    let state_dir = temp_dir.path().to_path_buf();

    // 1. Spawn primary watch process with piped stdin
    let mut child1 = Command::new(bin_path)
        .arg("watch")
        .arg("--state-dir")
        .arg(&state_dir)
        .env("T3_BAR_FIXTURE_STATE", "1")
        .env("T3_BAR_FIXTURE_POLL_DELAY_MS", "300")
        .env("T3_BAR_FROZEN_NOW", "2026-11-06T09:08:00+00:00")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("failed to spawn primary watch process");

    // 2. Verify initial state.json is written with loading: true
    let state_file = state_dir.join("state.json");
    let start = Instant::now();
    let mut found_initial = false;
    while start.elapsed() < Duration::from_secs(5) {
        if state_file.exists() {
            if let Ok(content) = fs::read_to_string(&state_file) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                    if val.get("loading").and_then(|v| v.as_bool()) == Some(true) {
                        found_initial = true;
                        break;
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(found_initial, "initial state.json with loading: true must be created before polling");

    // 3. Test flock serialization: concurrent watch instance must exit 0 immediately
    let output2 = Command::new(bin_path)
        .arg("watch")
        .arg("--state-dir")
        .arg(&state_dir)
        .env("T3_BAR_FIXTURE_STATE", "1")
        .output()
        .expect("failed to run second watch");
    assert_eq!(
        output2.status.code(),
        Some(0),
        "second watch contending for flock must exit 0 immediately"
    );

    // 4. Wait for first poll to populate state and settle
    let start_poll = Instant::now();
    let mut settled_mtime = None;
    while start_poll.elapsed() < Duration::from_secs(5) {
        if let Ok(content) = fs::read_to_string(&state_file) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if val.get("loading").and_then(|v| v.as_bool()) == Some(false) {
                    let mtime = fs::metadata(&state_file).unwrap().modified().unwrap();
                    settled_mtime = Some(mtime);
                    break;
                }
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
    let mtime_after_poll = settled_mtime.expect("state.json should settle with loading: false");

    // 5. Test change-gating: sleep over at least 1 tick (1.2s), verify mtime and content are untouched
    thread::sleep(Duration::from_millis(1200));
    let mtime_after_sleep = fs::metadata(&state_file).unwrap().modified().unwrap();
    assert_eq!(
        mtime_after_poll, mtime_after_sleep,
        "clean state ticks must leave file content and mtime untouched"
    );

    // 6. Test stdin-EOF termination: drop child1's stdin
    drop(child1.stdin.take());
    let start_exit = Instant::now();
    let mut exited = false;
    while start_exit.elapsed() < Duration::from_secs(4) {
        if let Ok(Some(status)) = child1.try_wait() {
            assert!(status.success(), "primary watch should exit 0 on stdin-EOF");
            exited = true;
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    if !exited {
        let _ = child1.kill();
        panic!("watch process did not terminate on stdin EOF within 4s");
    }
}
