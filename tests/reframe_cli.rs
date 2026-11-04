use serde_json::{Value, json};
use std::fs;
use std::process::Command;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
}

fn proposal() -> Value {
    json!({
        "schema_version": 1,
        "stated_request": "我要买个电钻",
        "classification": "means",
        "confidence_bps": 8200,
        "question": "打孔是为了穿线、挂东西，还是施工改造？",
        "options": ["穿线", "挂东西", "施工改造"],
        "hypotheses": ["联网", "装书架"],
        "substitutes": ["无线路由器"],
        "stop_rule": {
            "verifiable": true,
            "deliverable": true,
            "better_substitute_exists": true
        }
    })
}

fn write_proposal(dir: &std::path::Path, value: &Value) -> std::path::PathBuf {
    let path = dir.join("proposal.json");
    fs::write(&path, serde_json::to_string_pretty(value).unwrap()).unwrap();
    path
}

#[test]
fn reframe_then_receipt_round_trips_through_the_journal() {
    let dir = tempfile::tempdir().unwrap();
    let input = write_proposal(dir.path(), &proposal());
    let journal_dir = dir.path().join("journal");
    let journal_dir_text = journal_dir.display().to_string();

    let output = binary()
        .args([
            "reframe",
            "--input",
            &input.display().to_string(),
            "--accept",
            "1",
            "--journal-dir",
            &journal_dir_text,
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let created: Value = serde_json::from_slice(&output.stdout).unwrap();
    let journal = created["journal"].as_str().unwrap().to_string();
    assert!(std::path::Path::new(&journal).is_file());

    let output = binary()
        .args(["receipt", "--journal", &journal, "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(receipt["actor"], "user");
    assert_eq!(receipt["choice"]["accepted"]["option_index"], 0);
    let text = receipt["receipt"].as_str().unwrap();
    assert!(text.contains("我要买个电钻"));
    assert!(text.contains("采纳第 1 项"));
}

#[test]
fn a_rejection_reason_is_recorded_verbatim() {
    let dir = tempfile::tempdir().unwrap();
    let input = write_proposal(dir.path(), &proposal());
    let journal_dir = dir.path().join("journal");

    let output = binary()
        .args([
            "reframe",
            "--input",
            &input.display().to_string(),
            "--reject",
            "prefer_literal",
            "--journal-dir",
            &journal_dir.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());

    let created: Value = serde_json::from_slice(&output.stdout).unwrap();
    let journal = created["journal"].as_str().unwrap().to_string();
    let events = fs::read_to_string(&journal).unwrap();
    assert!(events.contains("\"reason\":\"prefer_literal\""));
}

#[test]
fn an_out_of_range_option_is_refused_before_anything_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let input = write_proposal(dir.path(), &proposal());
    let journal_dir = dir.path().join("journal");

    let output = binary()
        .args([
            "reframe",
            "--input",
            &input.display().to_string(),
            "--accept",
            "9",
            "--journal-dir",
            &journal_dir.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!journal_dir.exists(), "no journal should be created");
}

#[test]
fn exactly_one_decision_is_required() {
    let dir = tempfile::tempdir().unwrap();
    let input = write_proposal(dir.path(), &proposal());
    let journal_dir = dir.path().join("journal");

    let output = binary()
        .args([
            "reframe",
            "--input",
            &input.display().to_string(),
            "--accept",
            "1",
            "--defer",
            "--journal-dir",
            &journal_dir.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("exactly one of"), "stderr: {stderr}");
}

#[test]
fn an_invalid_proposal_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = proposal();
    value["classification"] = json!("end");
    let input = write_proposal(dir.path(), &value);
    let journal_dir = dir.path().join("journal");

    let output = binary()
        .args([
            "reframe",
            "--input",
            &input.display().to_string(),
            "--accept",
            "1",
            "--journal-dir",
            &journal_dir.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn no_decision_prints_a_menu_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let input = write_proposal(dir.path(), &proposal());
    let journal_dir = dir.path().join("journal");

    let output = binary()
        .args([
            "reframe",
            "--input",
            &input.display().to_string(),
            "--journal-dir",
            &journal_dir.display().to_string(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[1] 采纳「穿线」"), "stdout: {stdout}");
    assert!(stdout.contains("--accept 3"), "every option is actionable");
    assert!(stdout.contains("--reject prefer_literal"));
    let has_journal = journal_dir.exists()
        && fs::read_dir(&journal_dir).unwrap().any(|entry| {
            entry
                .unwrap()
                .path()
                .extension()
                .is_some_and(|ext| ext == "jsonl")
        });
    assert!(
        !has_journal,
        "an undecided proposal must not create a journal"
    );
    assert!(
        journal_dir.join("pending").is_dir(),
        "the proposal must be persisted so the menu stays actionable"
    );
}

#[test]
fn choice_numbers_start_at_one() {
    let dir = tempfile::tempdir().unwrap();
    let input = write_proposal(dir.path(), &proposal());
    let journal_dir = dir.path().join("journal");

    let output = binary()
        .args([
            "reframe",
            "--input",
            &input.display().to_string(),
            "--accept",
            "0",
            "--journal-dir",
            &journal_dir.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("start at 1"));
}

#[test]
fn task_mode_requires_network_and_a_base_url() {
    let dir = tempfile::tempdir().unwrap();

    let output = binary()
        .args([
            "reframe",
            "--task",
            "我要买个电钻",
            "--journal-dir",
            &dir.path().join("journal").display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--allow-network"));

    let output = binary()
        .args(["reframe", "--task", "我要买个电钻", "--allow-network"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--base-url"));
}

#[test]
fn task_and_input_are_mutually_exclusive() {
    let dir = tempfile::tempdir().unwrap();
    let input = write_proposal(dir.path(), &proposal());
    let output = binary()
        .args([
            "reframe",
            "--task",
            "x",
            "--input",
            &input.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn an_end_proposal_gets_no_menu_and_no_pending_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = proposal();
    value["classification"] = json!("end");
    value["question"] = json!(null);
    value["options"] = json!([]);
    value["substitutes"] = json!([]);
    let input = write_proposal(dir.path(), &value);
    let journal_dir = dir.path().join("journal");

    let output = binary()
        .args([
            "reframe",
            "--input",
            &input.display().to_string(),
            "--journal-dir",
            &journal_dir.display().to_string(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("目的类请求"), "stdout: {stdout}");
    assert!(!stdout.contains("采纳"), "an end has no accept menu");
    assert!(!journal_dir.join("pending").exists());
}
