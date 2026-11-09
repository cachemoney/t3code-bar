use std::process::Command;

#[test]
fn snapshot_subcommand_outputs_host_bridge_app_snapshot_json() {
    let bin_path = env!("CARGO_BIN_EXE_t3code-bar-bridge");
    let output = Command::new(bin_path)
        .arg("snapshot")
        .env("T3_BAR_FIXTURE_STATE", "1")
        .output()
        .expect("failed to execute binary");

    assert!(output.status.success(), "snapshot subcommand failed: {}", String::from_utf8_lossy(&output.stderr));
    let stdout_str = String::from_utf8(output.stdout).expect("stdout not utf-8");
    assert_eq!(
        stdout_str.trim().lines().count(),
        1,
        "snapshot output must be a single JSON line on stdout"
    );
    let val: serde_json::Value = serde_json::from_str(&stdout_str).expect("stdout not valid json");

    assert!(val.get("summary").is_some(), "summary must be present in HostBridge mode");
    assert!(val.get("update").is_none(), "update must be omitted in HostBridge mode");
    assert!(val.get("threads").is_some(), "threads list must be present");
    assert!(val.get("settings").is_some(), "settings must be present");
    assert!(val.get("connection").is_some(), "connection must be present");
}
