use std::process::Command;

#[test]
fn task_graph_demo_is_persisted_and_replayable() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("task-graph.jsonl");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let demo = Command::new(bin)
        .args(["task-graph", "demo", "--store"])
        .arg(&store)
        .arg("--json")
        .output()
        .unwrap();
    assert!(demo.status.success(), "{demo:?}");
    let demo_json: serde_json::Value = serde_json::from_slice(&demo.stdout).unwrap();
    assert_eq!(demo_json["tickets"].as_array().unwrap().len(), 1);

    let snapshot = Command::new(bin)
        .args(["task-graph", "snapshot", "--store"])
        .arg(&store)
        .arg("--json")
        .output()
        .unwrap();
    assert!(snapshot.status.success(), "{snapshot:?}");
    let value: serde_json::Value = serde_json::from_slice(&snapshot.stdout).unwrap();
    assert_eq!(value["threads"].as_array().unwrap().len(), 1);
    assert_eq!(value["tickets"].as_array().unwrap().len(), 1);
    assert_eq!(value["reports"].as_array().unwrap().len(), 1);
    assert_eq!(value["side_effects"].as_array().unwrap().len(), 1);
}
