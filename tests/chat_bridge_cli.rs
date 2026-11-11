use std::process::Command;

#[test]
fn chat_bridge_converts_messages_and_results() {
    let workspace = tempfile::tempdir().unwrap();
    let message = workspace.path().join("message.json");
    let task_out = workspace.path().join("task.json");
    let result_path = workspace.path().join("result.json");
    let reply_out = workspace.path().join("reply.json");
    let journals = workspace.path().join("journals");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");

    std::fs::write(
        &message,
        serde_json::to_vec(&serde_json::json!({
            "channel": "slack",
            "sender_id": "owner",
            "thread_id": "thread-1",
            "text": "  compare A and B  "
        }))
        .unwrap(),
    )
    .unwrap();
    let task = Command::new(bin)
        .args(["chat-bridge", "task", "--message"])
        .arg(&message)
        .arg("--workspace")
        .arg(workspace.path())
        .arg("--out")
        .arg(&task_out)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        task.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&task.stderr)
    );
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&task_out).unwrap()).unwrap();
    assert_eq!(saved["objective"], "compare A and B");

    let run = Command::new(bin)
        .args(["run", "--task", "用一句话回答 1+1", "--journal-dir"])
        .arg(&journals)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    std::fs::write(&result_path, &run.stdout).unwrap();

    let reply = Command::new(bin)
        .args(["chat-bridge", "reply", "--result"])
        .arg(&result_path)
        .args(["--thread-id", "thread-1", "--inbox-open", "2", "--out"])
        .arg(&reply_out)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        reply.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&reply.stderr)
    );
    let reply: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&reply_out).unwrap()).unwrap();
    assert_eq!(reply["thread_id"], "thread-1");
    assert_eq!(reply["inbox_open"], 2);
    assert!(reply["text"].as_str().unwrap().contains("证据"));
}

#[test]
fn chat_bridge_ingest_processes_a_spool_directory() {
    let workspace = tempfile::tempdir().unwrap();
    let inbox = workspace.path().join("inbox");
    let out = workspace.path().join("tasks");
    std::fs::create_dir_all(&inbox).unwrap();
    for (name, text) in [("a.json", "first task"), ("b.json", "second task")] {
        std::fs::write(
            inbox.join(name),
            serde_json::to_vec(&serde_json::json!({
                "channel": "web",
                "sender_id": "owner",
                "thread_id": "thread-1",
                "text": text
            }))
            .unwrap(),
        )
        .unwrap();
    }
    std::fs::write(
        inbox.join("c.json"),
        serde_json::to_vec(&serde_json::json!({
            "channel": "web",
            "sender_id": "owner",
            "thread_id": "thread-1",
            "text": " "
        }))
        .unwrap(),
    )
    .unwrap();

    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let run = Command::new(bin)
        .args(["chat-bridge", "ingest", "--inbox"])
        .arg(&inbox)
        .arg("--workspace")
        .arg(workspace.path())
        .arg("--out")
        .arg(&out)
        .arg("--json")
        .output()
        .unwrap();
    assert!(!run.status.success());
    let report: serde_json::Value = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(report["scanned"], 3);
    assert_eq!(report["converted"], 2);
    assert_eq!(report["failed"], 1);
    assert_eq!(report["task_files"].as_array().unwrap().len(), 2);
    assert_eq!(std::fs::read_dir(&inbox).unwrap().count(), 2);

    let second = Command::new(bin)
        .args(["chat-bridge", "ingest", "--inbox"])
        .arg(&inbox)
        .arg("--workspace")
        .arg(workspace.path())
        .arg("--out")
        .arg(&out)
        .arg("--json")
        .output()
        .unwrap();
    assert!(second.status.success());
    let report: serde_json::Value = serde_json::from_slice(&second.stdout).unwrap();
    assert_eq!(report["scanned"], 0);
}

#[test]
fn chat_bridge_rejects_an_invalid_message() {
    let workspace = tempfile::tempdir().unwrap();
    let message = workspace.path().join("message.json");
    std::fs::write(
        &message,
        serde_json::to_vec(&serde_json::json!({
            "channel": "slack",
            "sender_id": "owner",
            "thread_id": "thread-1",
            "text": " "
        }))
        .unwrap(),
    )
    .unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args(["chat-bridge", "task", "--message"])
        .arg(&message)
        .arg("--workspace")
        .arg(workspace.path())
        .arg("--out")
        .arg(workspace.path().join("task.json"))
        .output()
        .unwrap();
    assert!(!output.status.success());
}
