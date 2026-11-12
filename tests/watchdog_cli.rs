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

#[test]
fn policy_escalates_and_records_only_transitions() {
    let workspace = tempfile::tempdir().unwrap();
    let heartbeat = workspace.path().join("heartbeat.json");
    let log = workspace.path().join("interventions.jsonl");
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    std::fs::write(
        &heartbeat,
        serde_json::to_vec(&serde_json::json!({
            "pid": 1,
            "updated_at_ms": 0,
        }))
        .unwrap(),
    )
    .unwrap();

    let args = || {
        vec![
            "watchdog".to_string(),
            "--heartbeat".to_string(),
            heartbeat.display().to_string(),
            "--warn-after-ms".to_string(),
            "1000".to_string(),
            "--pause-after-ms".to_string(),
            "2000".to_string(),
            "--repair-after-ms".to_string(),
            "4000".to_string(),
            "--intervention-log".to_string(),
            log.display().to_string(),
            "--json".to_string(),
        ]
    };
    let first = Command::new(binary).args(args()).output().unwrap();
    assert!(!first.status.success());
    let report: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(report["action"], "repair");
    assert_eq!(report["pid"], 1);
    assert_eq!(std::fs::read_to_string(&log).unwrap().lines().count(), 1);

    let repeated = Command::new(binary).args(args()).output().unwrap();
    assert!(!repeated.status.success());
    assert_eq!(
        std::fs::read_to_string(&log).unwrap().lines().count(),
        1,
        "the same action must not be logged twice"
    );

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    std::fs::write(
        &heartbeat,
        serde_json::to_vec(&serde_json::json!({
            "pid": 1,
            "updated_at_ms": now_ms,
        }))
        .unwrap(),
    )
    .unwrap();
    let recovered = Command::new(binary).args(args()).output().unwrap();
    assert!(
        recovered.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&recovered.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&recovered.stdout).unwrap();
    assert_eq!(report["action"], "ok");
    assert_eq!(
        std::fs::read_to_string(&log).unwrap().lines().count(),
        2,
        "recovery should be recorded once"
    );
}

#[test]
fn invalid_policy_thresholds_are_rejected() {
    let workspace = tempfile::tempdir().unwrap();
    let heartbeat = workspace.path().join("heartbeat.json");
    std::fs::write(
        &heartbeat,
        serde_json::to_vec(&serde_json::json!({
            "pid": 1,
            "updated_at_ms": 0,
        }))
        .unwrap(),
    )
    .unwrap();
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(binary)
        .args([
            "watchdog",
            "--heartbeat",
            &heartbeat.display().to_string(),
            "--warn-after-ms",
            "2000",
            "--pause-after-ms",
            "1000",
            "--repair-after-ms",
            "4000",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("strictly increasing"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
