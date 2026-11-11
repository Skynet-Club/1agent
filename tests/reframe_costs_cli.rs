use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_first-principles-agent")
}

fn proposal(path: &std::path::Path, added_costs: serde_json::Value) {
    std::fs::write(
        path,
        serde_json::to_vec(&serde_json::json!({
            "schema_version": 1,
            "stated_request": "buy a drill",
            "classification": "means",
            "question": "What will you use it for?",
            "options": ["drill", "rent"],
            "added_costs": added_costs,
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

fn run(input: &std::path::Path, extra: &[&str]) -> std::process::Output {
    let mut command = Command::new(bin());
    command.args(["reframe", "--input"]).arg(input);
    command
        .args(["--journal-dir"])
        .arg(input.with_extension("journals"));
    command.args(extra);
    command.output().unwrap()
}

#[test]
fn require_costs_accepts_a_complete_statement() {
    let workspace = tempfile::tempdir().unwrap();
    let input = workspace.path().join("proposal.json");
    proposal(
        &input,
        serde_json::json!({
            "drill": "costs money and storage",
            "rent": "repeated trips and deposit"
        }),
    );
    let output = run(&input, &["--require-costs", "--json"]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["pending"], true);
}

#[test]
fn require_costs_rejects_missing_unknown_and_empty_costs() {
    let workspace = tempfile::tempdir().unwrap();
    let input = workspace.path().join("proposal.json");

    proposal(&input, serde_json::json!({"drill": "costs money"}));
    let missing = run(&input, &["--require-costs"]);
    assert!(!missing.status.success());
    assert!(
        String::from_utf8_lossy(&missing.stderr).contains("added cost"),
        "stderr: {}",
        String::from_utf8_lossy(&missing.stderr)
    );

    proposal(
        &input,
        serde_json::json!({
            "drill": "costs money",
            "rent": "deposit",
            "saw": "not an option"
        }),
    );
    let unknown = run(&input, &["--require-costs"]);
    assert!(!unknown.status.success());
    assert!(
        String::from_utf8_lossy(&unknown.stderr).contains("unknown option"),
        "stderr: {}",
        String::from_utf8_lossy(&unknown.stderr)
    );

    proposal(
        &input,
        serde_json::json!({"drill": "costs money", "rent": "  "}),
    );
    let empty = run(&input, &["--require-costs"]);
    assert!(!empty.status.success());
    assert!(
        String::from_utf8_lossy(&empty.stderr).contains("empty"),
        "stderr: {}",
        String::from_utf8_lossy(&empty.stderr)
    );
}

#[test]
fn costs_are_optional_without_the_flag() {
    let workspace = tempfile::tempdir().unwrap();
    let input = workspace.path().join("proposal.json");
    proposal(&input, serde_json::json!({}));
    let output = run(&input, &["--json"]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["pending"], true);
}
