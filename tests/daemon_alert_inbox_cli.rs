use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_first-principles-agent")
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(bin()).args(args).output().unwrap()
}

#[test]
fn daemon_projects_alert_log_records_into_the_inbox() {
    use first_principles_agent::{AlertLog, AlertRecord};

    let workspace = tempfile::tempdir().unwrap();
    let alert_log = workspace.path().join("alerts.jsonl");
    let inbox = workspace.path().join("inbox.jsonl");
    let schedules = workspace.path().join("schedules.json");
    let journals = workspace.path().join("journals");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    AlertLog::open(&alert_log)
        .unwrap()
        .append(&AlertRecord {
            alert_id: "a1".into(),
            at_ms: now,
            kind: "schedule_run_failed".into(),
            provider_id: None,
            model: None,
            observed: 0,
            threshold: 0,
            message: "scheduled job nightly failed".into(),
        })
        .unwrap();

    let daemon = run(&[
        "daemon",
        "--store",
        schedules.to_str().unwrap(),
        "--journal-dir",
        journals.to_str().unwrap(),
        "--once",
        "--alert-log",
        alert_log.to_str().unwrap(),
        "--alert-inbox-store",
        inbox.to_str().unwrap(),
    ]);
    assert!(
        daemon.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&daemon.stderr)
    );

    let listed = run(&[
        "inbox",
        "list",
        "--store",
        inbox.to_str().unwrap(),
        "--json",
    ]);
    assert!(listed.status.success());
    let items: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(items.as_array().unwrap().len(), 1);
    assert_eq!(items[0]["item_id"], "alert:a1");
    assert_eq!(items[0]["kind"], "review");
    assert_eq!(items[0]["title"], "scheduled job nightly failed");
    assert_eq!(items[0]["source"], "alerts");

    let repeat = run(&[
        "daemon",
        "--store",
        schedules.to_str().unwrap(),
        "--journal-dir",
        journals.to_str().unwrap(),
        "--once",
        "--alert-log",
        alert_log.to_str().unwrap(),
        "--alert-inbox-store",
        inbox.to_str().unwrap(),
    ]);
    assert!(
        repeat.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&repeat.stderr)
    );
    let listed = run(&[
        "inbox",
        "list",
        "--store",
        inbox.to_str().unwrap(),
        "--json",
    ]);
    let items: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(items.as_array().unwrap().len(), 1);
}

#[test]
fn daemon_requires_an_alert_log_for_alert_inbox_sync() {
    let workspace = tempfile::tempdir().unwrap();
    let output = run(&[
        "daemon",
        "--store",
        workspace.path().join("schedules.json").to_str().unwrap(),
        "--journal-dir",
        workspace.path().join("journals").to_str().unwrap(),
        "--once",
        "--alert-inbox-store",
        workspace.path().join("inbox.jsonl").to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--alert-log"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
