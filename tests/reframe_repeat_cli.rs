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
fn a_rejected_request_is_not_reproposed_unless_forced() {
    let workspace = tempfile::tempdir().unwrap();
    let journals = workspace.path().join("journals");
    let input = workspace.path().join("proposal.json");
    proposal(&input, "buy a drill");

    let rejected = Command::new(bin())
        .args(["reframe", "--input"])
        .arg(&input)
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
    let journal_count = std::fs::read_dir(&journals)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("jsonl"))
        .count();
    assert_eq!(journal_count, 1);

    let repeated = Command::new(bin())
        .args(["reframe", "--input"])
        .arg(&input)
        .arg("--journal-dir")
        .arg(&journals)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        repeated.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&repeated.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&repeated.stdout).unwrap();
    assert_eq!(report["repeated_rejection"], true);
    assert!(report["journal"].as_str().unwrap().ends_with(".jsonl"));
    let after = std::fs::read_dir(&journals)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("jsonl"))
        .count();
    assert_eq!(
        after, journal_count,
        "a rejection check must not write a journal"
    );

    let forced = Command::new(bin())
        .args(["reframe", "--input"])
        .arg(&input)
        .arg("--journal-dir")
        .arg(&journals)
        .args(["--force", "--json"])
        .output()
        .unwrap();
    assert!(
        forced.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&forced.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&forced.stdout).unwrap();
    assert_eq!(report["pending"], true);

    let other = workspace.path().join("other.json");
    proposal(&other, "buy a saw");
    let different = Command::new(bin())
        .args(["reframe", "--input"])
        .arg(&other)
        .arg("--journal-dir")
        .arg(&journals)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        different.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&different.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&different.stdout).unwrap();
    assert_eq!(report["pending"], true);
    assert!(report.get("repeated_rejection").is_none());
}
