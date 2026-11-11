use std::process::Command;

fn input(dir: &std::path::Path, name: &str, hidden: bool) -> std::path::PathBuf {
    let path = dir.join(name);
    let mut value = serde_json::json!({
        "executor_id": "executor",
        "verifier_id": "verifier",
        "acceptance_criteria": ["public"],
        "criteria_results": [
            {"criterion": "public", "passed": true, "evidence": ["log"]}
        ]
    });
    if hidden {
        value["hidden_criteria"] = serde_json::json!(["hidden"]);
        value["criteria_results"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "criterion": "hidden",
                "passed": false,
                "evidence": ["log"]
            }));
    }
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    path
}

#[test]
fn verification_store_records_summarizes_and_feeds_value_report() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("verification.jsonl");
    let journals = workspace.path().join("journals");
    std::fs::create_dir_all(&journals).unwrap();
    let verified_input = input(workspace.path(), "verified.json", false);
    let hacked_input = input(workspace.path(), "hacked.json", true);
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");

    let inbox = workspace.path().join("inbox.jsonl");
    for (id, path) in [("v1", &verified_input), ("v2", &hacked_input)] {
        let output = Command::new(bin)
            .args(["verification", "record", "--store"])
            .arg(&store)
            .args(["--verification-id", id, "--run-id"])
            .arg(format!("run-{id}"))
            .arg("--input")
            .arg(path)
            .arg("--inbox-store")
            .arg(&inbox)
            .args(["--at-ms", "10", "--json"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let duplicate = Command::new(bin)
        .args(["verification", "record", "--store"])
        .arg(&store)
        .args(["--verification-id", "v1", "--run-id", "run-v1", "--input"])
        .arg(&verified_input)
        .args(["--json"])
        .output()
        .unwrap();
    assert!(!duplicate.status.success());

    let summary = Command::new(bin)
        .args(["verification", "summary", "--store"])
        .arg(&store)
        .args(["--json"])
        .output()
        .unwrap();
    assert!(summary.status.success());
    let summary: serde_json::Value = serde_json::from_slice(&summary.stdout).unwrap();
    assert_eq!(summary["verified"], 1);
    assert_eq!(summary["hacked"], 1);
    assert_eq!(summary["rejected"], 0);
    assert_eq!(summary["resolved"], 2);
    assert_eq!(summary["hacked_resolved_bps"], 5000);

    let inbox_list = Command::new(bin)
        .args(["inbox", "list", "--store"])
        .arg(&inbox)
        .args(["--json"])
        .output()
        .unwrap();
    assert!(inbox_list.status.success());
    let items: serde_json::Value = serde_json::from_slice(&inbox_list.stdout).unwrap();
    assert_eq!(items.as_array().unwrap().len(), 1);
    assert_eq!(items[0]["item_id"], "verification:v2");
    assert_eq!(items[0]["kind"], "review");

    let value = Command::new(bin)
        .args(["value-report", "--journal-dir"])
        .arg(&journals)
        .arg("--verification-store")
        .arg(&store)
        .args([
            "--since-ms",
            "0",
            "--until-ms",
            "99999999999999",
            "--inbox-store",
        ])
        .arg(workspace.path().join("inbox.jsonl"))
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        value.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&value.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&value.stdout).unwrap();
    assert_eq!(report["verified_outcomes"], 1);
    assert_eq!(report["cost_per_verified_outcome_micros"], 0);

    let conflict = Command::new(bin)
        .args(["value-report", "--journal-dir"])
        .arg(&journals)
        .arg("--verification-store")
        .arg(&store)
        .args(["--verified-outcomes", "1", "--json"])
        .output()
        .unwrap();
    assert!(!conflict.status.success());
}
