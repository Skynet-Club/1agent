use std::process::Command;

fn run_one_journal(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let journal_dir = dir.path().join("journals");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let run = Command::new(bin)
        .args([
            "run",
            "--task",
            "Summarize the repository result",
            "--journal-dir",
        ])
        .arg(&journal_dir)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "run stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    journal_dir
}

#[test]
fn value_report_cli_reads_a_real_journal() {
    let dir = tempfile::tempdir().unwrap();
    let journal_dir = run_one_journal(&dir);
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args(["value-report", "--journal-dir"])
        .arg(&journal_dir)
        .arg("--inbox-store")
        .arg(dir.path().join("inbox.jsonl"))
        .args([
            "--since-ms",
            "0",
            "--until-ms",
            "99999999999999",
            "--verified-outcomes",
            "1",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "value-report stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["runs"], 1);
    assert_eq!(report["journals"], 1);
    assert_eq!(report["verified_outcomes"], 1);
    assert!(
        report["cost_per_verified_outcome_micros"]
            .as_u64()
            .is_some()
    );
    assert!(report["tokens"].as_u64().is_some());
    assert_eq!(report["alerts"], 0);
    assert_eq!(report["inbox_open"], 0);
    assert!(report["pending_decisions"].as_u64().is_some());
}

#[test]
fn value_report_cli_human_output_is_readable() {
    let dir = tempfile::tempdir().unwrap();
    let journal_dir = run_one_journal(&dir);
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args(["value-report", "--journal-dir"])
        .arg(&journal_dir)
        .arg("--inbox-store")
        .arg(dir.path().join("inbox.jsonl"))
        .args(["--since-ms", "0", "--until-ms", "99999999999999"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("1Agent Value Report"));
    assert!(text.contains("runs: 1"));
    assert!(text.contains("cost:"));
    assert!(text.contains("inbox: open=0"));
}

#[test]
fn value_report_includes_open_handoffs() {
    let dir = tempfile::tempdir().unwrap();
    let handoffs = dir.path().join("handoffs.jsonl");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let created = Command::new(bin)
        .args(["handoff", "create", "--store"])
        .arg(&handoffs)
        .args([
            "--id",
            "h1",
            "--from",
            "alice",
            "--to",
            "bob",
            "--summary",
            "handoff",
            "--evidence",
            "journal:run-1",
        ])
        .output()
        .unwrap();
    assert!(created.status.success());

    let output = Command::new(bin)
        .args(["value-report", "--journal-dir"])
        .arg(dir.path().join("journals"))
        .arg("--handoff-store")
        .arg(&handoffs)
        .args(["--recipient", "bob", "--inbox-store"])
        .arg(dir.path().join("inbox.jsonl"))
        .args(["--since-ms", "0", "--until-ms", "99999999999999", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["handoffs_open"], 1);

    let missing = Command::new(bin)
        .args(["value-report", "--journal-dir"])
        .arg(dir.path().join("journals"))
        .args(["--recipient", "bob"])
        .output()
        .unwrap();
    assert!(!missing.status.success());
}

#[test]
fn value_report_includes_open_inbox_items() {
    let dir = tempfile::tempdir().unwrap();
    let journal_dir = run_one_journal(&dir);
    let inbox = dir.path().join("inbox.jsonl");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let add = Command::new(bin)
        .args(["inbox", "add", "--store"])
        .arg(&inbox)
        .args([
            "--kind",
            "review",
            "--title",
            "Approve the deploy",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(add.status.success());

    let output = Command::new(bin)
        .args(["value-report", "--journal-dir"])
        .arg(&journal_dir)
        .arg("--inbox-store")
        .arg(&inbox)
        .args(["--since-ms", "0", "--until-ms", "99999999999999", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["inbox_open"], 1);
}
