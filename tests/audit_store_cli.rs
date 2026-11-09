use std::process::Command;

#[test]
fn audit_store_reads_task_graph_and_memory_receipts() {
    let dir = tempfile::tempdir().unwrap();
    let graph_path = dir.path().join("task-graph.jsonl");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");

    let demo = Command::new(bin)
        .args(["task-graph", "demo", "--store"])
        .arg(&graph_path)
        .arg("--json")
        .output()
        .unwrap();
    assert!(demo.status.success(), "{demo:?}");
    let snapshot: serde_json::Value = serde_json::from_slice(&demo.stdout).unwrap();
    let thread_id = snapshot["threads"][0]["thread_id"].as_str().unwrap();
    let ticket_id = snapshot["tickets"][0]["ticket_id"].as_str().unwrap();

    let export_path = dir.path().join("events.jsonl");
    let graph = Command::new(bin)
        .args(["audit-store", "--kind", "task-graph", "--path"])
        .arg(&graph_path)
        .arg("--thread")
        .arg(thread_id)
        .arg("--export")
        .arg(&export_path)
        .arg("--json")
        .output()
        .unwrap();
    assert!(graph.status.success(), "{graph:?}");
    let result: serde_json::Value = serde_json::from_slice(&graph.stdout).unwrap();
    assert_eq!(result["receipt"]["verified"], true);
    assert_eq!(result["matched"], 10);
    assert_eq!(
        std::fs::read_to_string(&export_path)
            .unwrap()
            .lines()
            .count(),
        10
    );

    let ticket = Command::new(bin)
        .args(["audit-store", "--kind", "task-graph", "--path"])
        .arg(&graph_path)
        .arg("--ticket")
        .arg(ticket_id)
        .arg("--json")
        .output()
        .unwrap();
    assert!(ticket.status.success(), "{ticket:?}");
    let result: serde_json::Value = serde_json::from_slice(&ticket.stdout).unwrap();
    assert_eq!(result["matched"], 6);

    let memory_path = dir.path().join("memory-journal.jsonl");
    let memory = Command::new(bin)
        .args(["audit-store", "--kind", "memory", "--path"])
        .arg(&memory_path)
        .arg("--json")
        .output()
        .unwrap();
    assert!(!memory.status.success(), "missing journal must fail closed");
}
