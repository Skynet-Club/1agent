use std::process::Command;

fn run_one_journal(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let journal_dir = dir.path().join("journals");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let run = Command::new(bin)
        .args(["run", "--task", "Summarize the result", "--journal-dir"])
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
        .args(["--since-ms", "0", "--until-ms", "99999999999999", "--json"])
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
    assert!(report["tokens"].as_u64().is_some());
    assert_eq!(report["alerts"], 0);
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
        .args(["--since-ms", "0", "--until-ms", "99999999999999"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("1Agent Value Report"));
    assert!(text.contains("runs: 1"));
    assert!(text.contains("cost:"));
}
