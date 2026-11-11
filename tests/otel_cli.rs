use std::process::Command;

#[test]
fn otel_cli_projects_agent_chat_and_tool_spans() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let note = workspace.join("note.txt");
    std::fs::write(&note, "hello from otel").unwrap();
    let journal_dir = dir.path().join("journals");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let run = Command::new(bin)
        .args(["run", "--task"])
        .arg(format!("@read-file:\"{}\"", note.display()))
        .arg("--read-root")
        .arg(&workspace)
        .arg("--journal-dir")
        .arg(&journal_dir)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "run stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let journal = std::fs::read_dir(&journal_dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().and_then(|value| value.to_str()) == Some("jsonl"))
        .expect("journal");
    let otel = Command::new(bin)
        .args(["otel", "--journal"])
        .arg(&journal)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        otel.status.success(),
        "otel stderr: {}",
        String::from_utf8_lossy(&otel.stderr)
    );
    let spans: serde_json::Value = serde_json::from_slice(&otel.stdout).unwrap();
    let spans = spans.as_array().unwrap();
    assert!(
        spans
            .iter()
            .any(|span| span["name"].as_str().unwrap().starts_with("invoke_agent"))
    );
    let chat = spans
        .iter()
        .find(|span| span["name"].as_str().unwrap().starts_with("chat"))
        .expect("chat span");
    assert_eq!(chat["attributes"]["gen_ai.provider.name"], "deterministic");
    let tool = spans
        .iter()
        .find(|span| span["name"] == "execute_tool read_file")
        .expect("tool span");
    assert_eq!(tool["attributes"]["gen_ai.tool.name"], "read_file");
}
