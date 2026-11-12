use std::process::Command;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_first-principles-agent")
}

#[test]
fn benchmark_run_repeats_the_core_suite() {
    let workspace = tempfile::tempdir().unwrap();
    let suite = format!("{}/evals/core.json", env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(binary())
        .current_dir(workspace.path())
        .args(["benchmark-run", "--suite", &suite, "--runs", "2", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["iterations"], 2);
    assert_eq!(report["completed"], 2);
    assert_eq!(report["crashed"], 0);
    assert_eq!(report["replay_verified"], 2);
    assert_eq!(report["completion_bps"], 10_000);
    assert_eq!(report["crash_free_bps"], 10_000);
    assert_eq!(report["replay_bps"], 10_000);
    assert_eq!(report["passed"], true);

    let human = Command::new(binary())
        .current_dir(workspace.path())
        .args(["benchmark-run", "--suite", &suite, "--runs", "1"])
        .output()
        .unwrap();
    assert!(human.status.success());
    let stdout = String::from_utf8_lossy(&human.stdout);
    assert!(stdout.contains("passed: true"), "stdout: {stdout}");
}

#[test]
fn benchmark_run_fails_a_failing_suite() {
    let workspace = tempfile::tempdir().unwrap();
    let suite = workspace.path().join("failing.json");
    std::fs::write(
        &suite,
        serde_json::to_vec(&serde_json::json!({
            "name": "failing",
            "suite_version": "1",
            "kind": "contract_fixture",
            "cases": [{
                "id": "fail",
                "objective": "hello",
                "expect": {
                    "stop_reason": "completed",
                    "answer_status": "completed"
                }
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let output = Command::new(binary())
        .current_dir(workspace.path())
        .args([
            "benchmark-run",
            "--suite",
            &suite.display().to_string(),
            "--runs",
            "1",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["iterations"], 1);
    assert_eq!(report["completed"], 0);
    assert_eq!(report["completion_bps"], 0);
    assert_eq!(report["passed"], false);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("benchmark gate failed"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
