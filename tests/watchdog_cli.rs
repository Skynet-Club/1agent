use std::process::Command;

#[test]
fn missing_and_stale_heartbeats_fail() {
    let workspace = tempfile::tempdir().unwrap();
    let heartbeat = workspace.path().join("heartbeat.json");
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");

    let missing = Command::new(binary)
        .args([
            "watchdog",
            "--heartbeat",
            &heartbeat.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!missing.status.success());
    let report: serde_json::Value = serde_json::from_slice(&missing.stdout).unwrap();
    assert_eq!(report["healthy"], false);

    std::fs::write(
        &heartbeat,
        serde_json::to_vec(&serde_json::json!({
            "pid": 1,
            "updated_at_ms": 0,
        }))
        .unwrap(),
    )
    .unwrap();
    let stale = Command::new(binary)
        .args([
            "watchdog",
            "--heartbeat",
            &heartbeat.display().to_string(),
            "--max-age-ms",
            "1000",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!stale.status.success());
    let report: serde_json::Value = serde_json::from_slice(&stale.stdout).unwrap();
    assert_eq!(report["healthy"], false);
    assert_eq!(report["pid"], 1);
}

#[test]
fn daemon_once_writes_a_fresh_heartbeat() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("schedules.json");
    let journals = workspace.path().join("journals");
    let heartbeat = workspace.path().join("heartbeat.json");
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");

    let daemon = Command::new(binary)
        .args([
            "daemon",
            "--store",
            &store.display().to_string(),
            "--journal-dir",
            &journals.display().to_string(),
            "--heartbeat",
            &heartbeat.display().to_string(),
            "--once",
        ])
        .output()
        .unwrap();
    assert!(
        daemon.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&daemon.stderr)
    );
    assert!(heartbeat.exists());

    let checked = Command::new(binary)
        .args([
            "watchdog",
            "--heartbeat",
            &heartbeat.display().to_string(),
            "--max-age-ms",
            "120000",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        checked.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&checked.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
    assert_eq!(report["healthy"], true);
    let recorded: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&heartbeat).unwrap()).unwrap();
    assert_eq!(report["pid"], recorded["pid"]);
}

#[test]
fn invalid_heartbeat_json_is_rejected() {
    let workspace = tempfile::tempdir().unwrap();
    let heartbeat = workspace.path().join("heartbeat.json");
    std::fs::write(&heartbeat, b"not-json").unwrap();
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(binary)
        .args(["watchdog", "--heartbeat", &heartbeat.display().to_string()])
        .output()
        .unwrap();
    assert!(!output.status.success());
}
