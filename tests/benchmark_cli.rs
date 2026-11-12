//! End-to-end coverage for `1agent benchmark`.

use std::process::Command;

fn write_samples(path: &std::path::Path, samples: &[&str]) {
    std::fs::write(path, samples.join("\n")).unwrap();
}

fn run(path: &std::path::Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(["benchmark", "--input"])
        .arg(path)
        .arg("--json")
        .output()
        .unwrap()
}

#[test]
fn a_clean_window_passes_the_gate() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("samples.jsonl");
    write_samples(
        &input,
        &[
            r#"{"completed":true,"crashed":false,"replay_verified":true}"#,
            r#"{"completed":true,"crashed":false,"replay_verified":true}"#,
            r#"{"completed":true,"crashed":false,"replay_verified":true}"#,
        ],
    );
    let output = run(&input);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["iterations"], 3);
    assert_eq!(report["completed"], 3);
    assert_eq!(report["crashed"], 0);
    assert_eq!(report["completion_bps"], 10_000);
    assert_eq!(report["passed"], true);
}

#[test]
fn a_crash_fails_the_gate_but_still_prints_the_report() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("samples.jsonl");
    write_samples(
        &input,
        &[
            r#"{"completed":true,"crashed":false,"replay_verified":true}"#,
            r#"{"completed":true,"crashed":false,"replay_verified":true}"#,
            r#"{"completed":true,"crashed":true,"replay_verified":true}"#,
        ],
    );
    let output = run(&input);
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["completed"], 2);
    assert_eq!(report["crashed"], 1);
    assert_eq!(report["completion_bps"], 6_666);
    assert_eq!(report["crash_free_bps"], 6_666);
    assert_eq!(report["passed"], false);
}

#[test]
fn malformed_input_reports_the_line() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("samples.jsonl");
    write_samples(
        &input,
        &[
            r#"{"completed":true,"crashed":false,"replay_verified":true}"#,
            r#"{"completed":true}"#,
        ],
    );
    let output = run(&input);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("line 2"), "stderr: {stderr}");
}
