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

    let page = Command::new(bin)
        .args(["audit-store", "--kind", "task-graph", "--path"])
        .arg(&graph_path)
        .arg("--thread")
        .arg(thread_id)
        .args(["--limit", "4", "--include-records", "--json"])
        .output()
        .unwrap();
    assert!(page.status.success(), "{page:?}");
    let result: serde_json::Value = serde_json::from_slice(&page.stdout).unwrap();
    let records = result["records"].as_array().unwrap();
    assert_eq!(records.len(), 4);
    let last_seq = records[3]["seq"].as_u64().unwrap();
    let next = Command::new(bin)
        .args(["audit-store", "--kind", "task-graph", "--path"])
        .arg(&graph_path)
        .arg("--thread")
        .arg(thread_id)
        .args(["--limit", "4", "--include-records", "--json", "--after-seq"])
        .arg(last_seq.to_string())
        .output()
        .unwrap();
    assert!(next.status.success(), "{next:?}");
    let result: serde_json::Value = serde_json::from_slice(&next.stdout).unwrap();
    let next_records = result["records"].as_array().unwrap();
    assert!(!next_records.is_empty());
    assert!(next_records[0]["seq"].as_u64().unwrap() > last_seq);

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
