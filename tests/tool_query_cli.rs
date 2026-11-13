use std::process::Command;

fn model_tool_names(journal: &str) -> Vec<String> {
    for line in journal.lines() {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if event["event"]["type"] == "model_requested" {
            return event["event"]["data"]["request"]["tools"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|tool| tool["name"].as_str().map(str::to_string))
                .collect();
        }
    }
    Vec::new()
}

fn journal_text(dir: &tempfile::TempDir, name: &str, args: &[&str]) -> String {
    let journal_dir = dir.path().join(name);
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let run = Command::new(bin)
        .args([
            "run",
            "--task",
            "Read the file and answer briefly",
            "--journal-dir",
        ])
        .arg(&journal_dir)
        .args(args)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "run stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let path = std::fs::read_dir(&journal_dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().and_then(|value| value.to_str()) == Some("jsonl"))
        .expect("journal file");
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn tool_query_limits_the_journaled_tool_list() {
    let dir = tempfile::tempdir().unwrap();
    let selected = model_tool_names(&journal_text(&dir, "selected", &["--tool-query", "shell"]));
    assert!(selected.iter().any(|name| name == "terminal_exec"));
    assert!(!selected.iter().any(|name| name == "browser_read"));

    let full = model_tool_names(&journal_text(&dir, "full", &[]));
    assert!(full.iter().any(|name| name == "browser_read"));
}
