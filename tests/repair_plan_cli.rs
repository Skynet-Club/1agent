use std::process::Command;

#[test]
fn repair_plan_reports_an_empty_store_as_clean() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("task-graph.jsonl");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args(["repair-plan", "--store"])
        .arg(&store)
        .args(["--require-clean", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let runbook: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(runbook["clean"], true);
    assert_eq!(runbook["compensate"], 0);
    assert_eq!(runbook["manual"], 0);
    assert_eq!(runbook["plans"].as_array().unwrap().len(), 0);
}

#[test]
fn repair_plan_reports_a_demo_side_effect_as_compensation() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("task-graph.jsonl");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let demo = Command::new(bin)
        .args(["task-graph", "demo", "--store"])
        .arg(&store)
        .output()
        .unwrap();
    assert!(
        demo.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&demo.stderr)
    );

    let runbook = Command::new(bin)
        .args(["repair-plan", "--store"])
        .arg(&store)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        runbook.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&runbook.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&runbook.stdout).unwrap();
    assert_eq!(report["clean"], false);
    assert_eq!(report["compensate"], 1);
    assert_eq!(report["manual"], 0);
    assert_eq!(report["plans"].as_array().unwrap().len(), 1);
    assert_eq!(report["plans"][0]["action"], "compensate");

    let require_clean = Command::new(bin)
        .args(["repair-plan", "--store"])
        .arg(&store)
        .args(["--require-clean"])
        .output()
        .unwrap();
    assert!(!require_clean.status.success());
}

#[test]
fn repair_plan_rejects_a_corrupt_store() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("task-graph.jsonl");
    std::fs::write(&store, b"not-json").unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args(["repair-plan", "--store"])
        .arg(&store)
        .output()
        .unwrap();
    assert!(!output.status.success());
}
