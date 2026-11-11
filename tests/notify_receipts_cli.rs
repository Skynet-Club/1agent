use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_first-principles-agent")
}

#[test]
fn notify_receipts_summarizes_inbox_and_digest_logs() {
    let workspace = tempfile::tempdir().unwrap();
    let log = workspace.path().join("receipts.jsonl");
    std::fs::write(
        &log,
        concat!(
            "{\"sent_at_ms\":100,\"open\":2,\"new_items\":1,\"format\":\"slack\"}\n",
            "{\"sent_at_ms\":200,\"runs\":3,\"alerts\":1,\"inbox_open\":2,\"format\":\"discord\"}\n"
        ),
    )
    .unwrap();
    let output = Command::new(bin())
        .args(["notify-receipts", "--log"])
        .arg(&log)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["summary"]["total"], 2);
    assert_eq!(report["summary"]["inbox"], 1);
    assert_eq!(report["summary"]["digest"], 1);
    assert_eq!(report["summary"]["last_sent_ms"], 200);
    assert_eq!(report["summary"]["formats"]["slack"], 1);
    assert_eq!(report["summary"]["formats"]["discord"], 1);
    assert_eq!(report["receipts"].as_array().unwrap().len(), 2);
}

#[test]
fn notify_receipts_rejects_malformed_and_unknown_lines() {
    let workspace = tempfile::tempdir().unwrap();
    let log = workspace.path().join("receipts.jsonl");
    std::fs::write(
        &log,
        "{\"sent_at_ms\":100,\"open\":1,\"new_items\":1,\"format\":\"slack\"}\nnot-json\n",
    )
    .unwrap();
    let output = Command::new(bin())
        .args(["notify-receipts", "--log"])
        .arg(&log)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("line 2"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    std::fs::write(&log, "{\"sent_at_ms\":100}\n").unwrap();
    let unknown = Command::new(bin())
        .args(["notify-receipts", "--log"])
        .arg(&log)
        .output()
        .unwrap();
    assert!(!unknown.status.success());
    assert!(
        String::from_utf8_lossy(&unknown.stderr).contains("not a notification receipt"),
        "stderr: {}",
        String::from_utf8_lossy(&unknown.stderr)
    );
}

#[test]
fn notify_receipts_treats_a_missing_log_as_empty() {
    let workspace = tempfile::tempdir().unwrap();
    let output = Command::new(bin())
        .args(["notify-receipts", "--log"])
        .arg(workspace.path().join("missing.jsonl"))
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["summary"]["total"], 0);
    assert_eq!(report["receipts"].as_array().unwrap().len(), 0);
}
