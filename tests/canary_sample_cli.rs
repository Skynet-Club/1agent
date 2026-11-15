//! End-to-end coverage for `run --canary-sample`.

use std::process::Command;

#[test]
fn a_run_appends_a_canary_sample() {
    let dir = tempfile::tempdir().unwrap();
    let sample = dir.path().join("canary.jsonl");
    let output = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .env("ONEAGENT_HOME", dir.path().join("home"))
        .args(["run", "--task", "Say hello", "--canary-sample"])
        .arg(&sample)
        .args(["--journal-dir"])
        .arg(dir.path().join("journals"))
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let lines = std::fs::read_to_string(&sample).unwrap();
    let lines = lines.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 1);
    let report: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    // The offline mock provider returns needs_evidence, so the sample records
    // a failed canary observation even though the CLI run itself succeeds.
    assert_eq!(report["success"], false);
    assert!(report["latency_ms"].as_u64().is_some());
    assert!(
        !lines[0].contains("Say hello"),
        "samples carry no task text"
    );
}

#[test]
fn repeated_runs_append_without_overwriting() {
    let dir = tempfile::tempdir().unwrap();
    let sample = dir.path().join("canary.jsonl");
    for index in 0..2 {
        let output = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
            .env("ONEAGENT_HOME", dir.path().join("home"))
            .args(["run", "--task", "Say hello", "--canary-sample"])
            .arg(&sample)
            .args(["--journal-dir"])
            .arg(dir.path().join(format!("journals-{index}")))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let lines = std::fs::read_to_string(&sample).unwrap();
    assert_eq!(lines.lines().count(), 2);
}
