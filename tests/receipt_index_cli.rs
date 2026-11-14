use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_first-principles-agent")
}

fn proposal(path: &std::path::Path, request: &str) {
    std::fs::write(
        path,
        serde_json::to_vec(&serde_json::json!({
            "schema_version": 1,
            "stated_request": request,
            "classification": "means",
            "question": "What will you use it for?",
            "stop_rule": {
                "verifiable": true,
                "deliverable": true,
                "better_substitute_exists": true
            },
            "stop_rule_source": "asserted"
        }))
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn receipt_index_lists_decisions_newest_first() {
    let workspace = tempfile::tempdir().unwrap();
    let journals = workspace.path().join("journals");
    let first = workspace.path().join("first.json");
    proposal(&first, "buy a drill");
    let rejected = Command::new(bin())
        .args(["reframe", "--input"])
        .arg(&first)
        .args(["--reject", "prefer_literal", "--journal-dir"])
        .arg(&journals)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        rejected.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&rejected.stderr)
    );

    std::thread::sleep(std::time::Duration::from_millis(5));
    let second = workspace.path().join("second.json");
    proposal(&second, "buy a saw");
    let deferred = Command::new(bin())
        .args(["reframe", "--input"])
        .arg(&second)
        .args(["--defer", "--journal-dir"])
        .arg(&journals)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        deferred.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&deferred.stderr)
    );

    std::fs::write(journals.join("corrupt.jsonl"), b"not-json").unwrap();

    let output = Command::new(bin())
        .args(["receipt-index", "--journal-dir"])
        .arg(&journals)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let entries: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["stated_request"], "buy a saw");
    assert_eq!(entries[0]["choice"], "deferred");
    assert_eq!(entries[0]["actor"], "user");
    assert_eq!(entries[1]["stated_request"], "buy a drill");
    assert_eq!(entries[1]["choice"]["rejected"]["reason"], "prefer_literal");
    assert!(std::path::Path::new(entries[1]["journal"].as_str().unwrap()).exists());

    let text = Command::new(bin())
        .args(["receipt-index", "--journal-dir"])
        .arg(&journals)
        .output()
        .unwrap();
    assert!(text.status.success());
    let text = String::from_utf8_lossy(&text.stdout);
    assert!(text.contains("1Agent Reframe Receipts: 2 条"), "{text}");
    assert!(text.contains("buy a drill"), "{text}");
}

#[test]
fn receipt_index_treats_a_missing_directory_as_empty() {
    let workspace = tempfile::tempdir().unwrap();
    let output = Command::new(bin())
        .args(["receipt-index", "--journal-dir"])
        .arg(workspace.path().join("missing"))
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let entries: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert!(entries.is_empty());
}
