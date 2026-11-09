use serde_json::{Value, json};
use std::process::Command;

fn run_scheduler(input: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args([
            "master",
            "schedule",
            "--input",
            input,
            "--global-active",
            "3",
            "--per-conversation",
            "2",
            "--json",
        ])
        .output()
        .unwrap()
}

fn task(id: &str) -> Value {
    json!({
        "task_id": id,
        "dependencies": [],
        "workspace": "/tmp/workspace",
        "allow_write": false,
        "status": "pending"
    })
}

#[test]
fn scheduler_cli_arbitrates_two_conversations_deterministically() {
    let root = tempfile::tempdir().unwrap();
    let input = root.path().join("queues.json");
    std::fs::write(
        &input,
        serde_json::to_vec_pretty(&json!([
            {
                "conversation_id": "master-1",
                "tasks": [task("t1"), task("t2"), task("t3"), task("t4")]
            },
            {
                "conversation_id": "master-2",
                "tasks": [task("t1"), task("t2"), task("t3"), task("t4"), task("t5")]
            }
        ]))
        .unwrap(),
    )
    .unwrap();

    let output = run_scheduler(input.to_str().unwrap());
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let decision: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        decision["starts"],
        json!([
            {"conversation_id": "master-1", "task_id": "t1", "stable_id": "master-1:t1"},
            {"conversation_id": "master-2", "task_id": "t1", "stable_id": "master-2:t1"},
            {"conversation_id": "master-1", "task_id": "t2", "stable_id": "master-1:t2"}
        ])
    );
    assert_eq!(decision["waiting"].as_array().unwrap().len(), 6);
}

#[test]
fn scheduler_cli_rejects_unknown_dependencies() {
    let root = tempfile::tempdir().unwrap();
    let input = root.path().join("bad-queues.json");
    let mut bad_task = task("t1");
    bad_task["dependencies"] = json!(["missing"]);
    std::fs::write(
        &input,
        serde_json::to_vec_pretty(&json!([{
            "conversation_id": "master-1",
            "tasks": [bad_task]
        }]))
        .unwrap(),
    )
    .unwrap();

    let output = run_scheduler(input.to_str().unwrap());
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown stable id"));
}
