use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_first-principles-agent")
}

#[test]
fn repair_mark_records_a_manual_compensation() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("task-graph.jsonl");
    let demo = Command::new(bin())
        .args(["task-graph", "demo", "--store"])
        .arg(&store)
        .output()
        .unwrap();
    assert!(
        demo.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&demo.stderr)
    );

    let runbook = Command::new(bin())
        .args(["repair-plan", "--store"])
        .arg(&store)
        .arg("--json")
        .output()
        .unwrap();
    assert!(runbook.status.success());
    let runbook: serde_json::Value = serde_json::from_slice(&runbook.stdout).unwrap();
    let side_effect_id = runbook["plans"][0]["side_effect_id"]
        .as_str()
        .unwrap()
        .to_string();

    let snapshot = Command::new(bin())
        .args(["task-graph", "snapshot", "--store"])
        .arg(&store)
        .arg("--json")
        .output()
        .unwrap();
    assert!(snapshot.status.success());
    let snapshot: serde_json::Value = serde_json::from_slice(&snapshot.stdout).unwrap();
    let effect = snapshot["side_effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["side_effect_id"] == side_effect_id)
        .unwrap();
    let ticket_id = effect["ticket_id"].as_str().unwrap().to_string();

    let marked = Command::new(bin())
        .args(["repair-mark", "--store"])
        .arg(&store)
        .args([
            "--ticket-id",
            &ticket_id,
            "--side-effect-id",
            &side_effect_id,
        ])
        .args([
            "--actor",
            "alice",
            "--evidence",
            "ran undo.sh; verified the balance",
        ])
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        marked.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&marked.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&marked.stdout).unwrap();
    assert_eq!(report["status"], "compensated");
    assert_eq!(report["runbook"]["compensate"], 0);
    assert_eq!(report["runbook"]["clean"], true);
    assert!(
        report["evidence"]
            .as_str()
            .unwrap()
            .contains("manual compensation by alice")
    );

    let clean = Command::new(bin())
        .args(["repair-plan", "--store"])
        .arg(&store)
        .arg("--require-clean")
        .output()
        .unwrap();
    assert!(
        clean.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&clean.stderr)
    );

    let repeat = Command::new(bin())
        .args(["repair-mark", "--store"])
        .arg(&store)
        .args([
            "--ticket-id",
            &ticket_id,
            "--side-effect-id",
            &side_effect_id,
        ])
        .args(["--actor", "alice", "--evidence", "again"])
        .output()
        .unwrap();
    assert!(!repeat.status.success());
    assert!(
        String::from_utf8_lossy(&repeat.stderr).contains("already compensated"),
        "stderr: {}",
        String::from_utf8_lossy(&repeat.stderr)
    );

    let wrong_ticket = Command::new(bin())
        .args(["repair-mark", "--store"])
        .arg(&store)
        .args([
            "--ticket-id",
            "ticket-other",
            "--side-effect-id",
            &side_effect_id,
        ])
        .args(["--actor", "alice", "--evidence", "undo"])
        .output()
        .unwrap();
    assert!(!wrong_ticket.status.success());
    assert!(
        String::from_utf8_lossy(&wrong_ticket.stderr).contains("belongs to ticket"),
        "stderr: {}",
        String::from_utf8_lossy(&wrong_ticket.stderr)
    );

    let unknown = Command::new(bin())
        .args(["repair-mark", "--store"])
        .arg(&store)
        .args(["--ticket-id", &ticket_id, "--side-effect-id", "missing"])
        .args(["--actor", "alice", "--evidence", "undo"])
        .output()
        .unwrap();
    assert!(!unknown.status.success());
    assert!(
        String::from_utf8_lossy(&unknown.stderr).contains("was not found"),
        "stderr: {}",
        String::from_utf8_lossy(&unknown.stderr)
    );

    let empty_evidence = Command::new(bin())
        .args(["repair-mark", "--store"])
        .arg(&store)
        .args([
            "--ticket-id",
            &ticket_id,
            "--side-effect-id",
            &side_effect_id,
        ])
        .args(["--actor", "alice", "--evidence", "   "])
        .output()
        .unwrap();
    assert!(!empty_evidence.status.success());
    assert!(
        String::from_utf8_lossy(&empty_evidence.stderr).contains("--evidence must not be empty"),
        "stderr: {}",
        String::from_utf8_lossy(&empty_evidence.stderr)
    );
}
