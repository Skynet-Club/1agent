use std::process::Command;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_first-principles-agent")
}

#[test]
fn cascade_bench_runs_passing_and_failing_scenarios() {
    let workspace = tempfile::tempdir().unwrap();
    let passing = workspace.path().join("passing.jsonl");
    std::fs::write(
        &passing,
        serde_json::to_vec(&serde_json::json!({
            "id": "contained",
            "root": "a",
            "edges": [
                {"parent": "a", "child": "b"},
                {"parent": "b", "child": "c"}
            ],
            "contained": ["b"],
            "policy": {
                "min_containment_bps": 5000,
                "max_depth": 1,
                "max_blast_radius": 2
            }
        }))
        .unwrap(),
    )
    .unwrap();
    let output = Command::new(binary())
        .args([
            "cascade-bench",
            "--input",
            &passing.display().to_string(),
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
    assert_eq!(report["scenarios"], 1);
    assert_eq!(report["passed"], 1);
    assert_eq!(report["failed"], 0);
    assert_eq!(report["min_containment_bps"], 5_000);

    let failing = workspace.path().join("failing.jsonl");
    std::fs::write(
        &failing,
        serde_json::to_vec(&serde_json::json!({
            "id": "uncontained",
            "root": "a",
            "edges": [{"parent": "a", "child": "b"}],
            "policy": {
                "min_containment_bps": 5000,
                "max_depth": 1,
                "max_blast_radius": 2
            }
        }))
        .unwrap(),
    )
    .unwrap();
    let output = Command::new(binary())
        .args([
            "cascade-bench",
            "--input",
            &failing.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["failed"], 1);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("cascade bench failed"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn cascade_bench_rejects_a_malformed_line() {
    let workspace = tempfile::tempdir().unwrap();
    let input = workspace.path().join("broken.jsonl");
    let valid = serde_json::json!({
        "id": "ok",
        "root": "a",
        "edges": [],
        "policy": {"min_containment_bps": 0, "max_depth": 1, "max_blast_radius": 1}
    });
    std::fs::write(
        &input,
        format!(
            "{}\n{{\"broken\":true}}\n",
            serde_json::to_string(&valid).unwrap()
        ),
    )
    .unwrap();
    let output = Command::new(binary())
        .args(["cascade-bench", "--input", &input.display().to_string()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("line 2"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
