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
    assert_eq!(report["suite_passed"], true);
    assert_eq!(report["tasks_total"], 16);
    assert_eq!(report["tasks_passed"], 16);
    assert_eq!(report["task_completion_bps"], 10_000);
    assert_eq!(report["completed_cases"], 2);
    assert_eq!(report["completed_replay_ok"], 2);
    assert_eq!(report["completed_replay_bps"], 10_000);
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
fn benchmark_estimate_only_does_not_run_the_suite() {
    let workspace = tempfile::tempdir().unwrap();
    let suite = format!("{}/evals/core.json", env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(binary())
        .current_dir(workspace.path())
        .args([
            "benchmark-run",
            "--suite",
            &suite,
            "--runs",
            "320",
            "--estimate-only",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["runs"], 320);
    assert_eq!(report["cases"], 2560);
    assert!(report["input_tokens"].as_u64().unwrap() > 0);
    assert!(report["output_tokens"].as_u64().unwrap() > 0);
    assert_eq!(report["priced"], false);
    assert_eq!(report["cost_micros"], 0);
}

#[test]
fn benchmark_run_rejects_out_of_range_task_thresholds() {
    let workspace = tempfile::tempdir().unwrap();
    let suite = format!("{}/evals/core.json", env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(binary())
        .current_dir(workspace.path())
        .args([
            "benchmark-run",
            "--suite",
            &suite,
            "--runs",
            "1",
            "--min-task-completion-bps",
            "10001",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("at most 10000"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
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
    assert_eq!(report["suite_passed"], false);
    assert_eq!(report["tasks_total"], 1);
    assert_eq!(report["tasks_passed"], 0);
    assert_eq!(report["task_completion_bps"], 0);
    assert_eq!(report["passed"], false);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("benchmark gate failed"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn benchmark_run_stops_at_a_failing_checkpoint() {
    let workspace = tempfile::tempdir().unwrap();
    let suite = workspace.path().join("failing.json");
    let cases: Vec<serde_json::Value> = (0..8)
        .map(|index| {
            serde_json::json!({
                "id": format!("fail-{index}"),
                "objective": "hello",
                "expect": {
                    "stop_reason": "completed",
                    "answer_status": "completed"
                }
            })
        })
        .collect();
    std::fs::write(
        &suite,
        serde_json::to_vec(&serde_json::json!({
            "name": "failing",
            "suite_version": "1",
            "kind": "contract_fixture",
            "cases": cases
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
            "10",
            "--checkpoint-every",
            "1",
            "--min-task-completion-bps",
            "5000",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["iterations"], 1);
    assert_eq!(report["stopped_early"], true);
    assert_eq!(report["stop_reason"], "task_completion");
    assert_eq!(report["passed"], false);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("benchmark gate failed"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
