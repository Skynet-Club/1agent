use std::process::Command;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_first-principles-agent")
}

fn write_json(workspace: &tempfile::TempDir, name: &str, value: &serde_json::Value) -> String {
    let path = workspace.path().join(name);
    std::fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
    path.display().to_string()
}

fn question(id: &str, prompt: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "prompt": prompt,
        "options": [
            {"id": "slow", "label": "Do it slowly", "recommended": false},
            {"id": "fast", "label": "Do it now", "recommended": true}
        ]
    })
}

#[test]
fn clarify_normalizes_one_question_with_other_and_recommended_first() {
    let workspace = tempfile::tempdir().unwrap();
    let input = write_json(
        &workspace,
        "questions.json",
        &serde_json::json!([
            question("q1", "Should we migrate now?"),
            question("q2", "Which environment?")
        ]),
    );
    let output = Command::new(binary())
        .args(["clarify", "--questions", &input, "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let decision: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(decision["decision"], "ask");
    assert_eq!(decision["timeout_ms"], 300_000);
    let questions = decision["questions"].as_array().unwrap();
    assert_eq!(questions.len(), 1);
    assert_eq!(questions[0]["id"], "q1");
    let options = questions[0]["options"].as_array().unwrap();
    assert_eq!(options[0]["id"], "fast");
    assert_eq!(options[0]["recommended"], true);
    assert_eq!(options.last().unwrap()["id"], "other");
}

#[test]
fn clarify_accepts_an_object_and_skips_an_empty_batch() {
    let workspace = tempfile::tempdir().unwrap();
    let empty = write_json(
        &workspace,
        "empty.json",
        &serde_json::json!({"questions": []}),
    );
    let output = Command::new(binary())
        .args(["clarify", "--questions", &empty, "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let decision: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(decision["decision"], "skip");
    assert!(decision["reason"].as_str().unwrap().contains("no new"));
}

#[test]
fn clarify_resolves_answers_and_keeps_the_default_answer_valid() {
    let workspace = tempfile::tempdir().unwrap();
    let mut first = question("q1", "Should we migrate now?");
    first["blocking"] = serde_json::json!(true);
    let questions = write_json(
        &workspace,
        "questions.json",
        &serde_json::json!([first, question("q2", "Which environment?")]),
    );
    let answers = write_json(
        &workspace,
        "answers.json",
        &serde_json::json!([
            {"question_id": "q1", "option_id": "fast", "status": "answered"},
            {"question_id": "q2", "status": "skipped"}
        ]),
    );
    let output = Command::new(binary())
        .args([
            "clarify",
            "--questions",
            &questions,
            "--max-questions",
            "2",
            "--answers",
            &answers,
            "--outcome",
            "submitted",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let summary: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(summary["outcome"], "submitted");
    assert_eq!(summary["answered"], 1);
    assert_eq!(summary["skipped"], 1);
    assert_eq!(summary["unanswered"], 0);
    assert_eq!(summary["blocking_unanswered"], 0);
    assert_eq!(summary["default_answer_valid"], true);

    let cancelled = write_json(&workspace, "answers-empty.json", &serde_json::json!([]));
    let output = Command::new(binary())
        .args([
            "clarify",
            "--questions",
            &questions,
            "--max-questions",
            "2",
            "--answers",
            &cancelled,
            "--outcome",
            "cancelled",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let summary: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(summary["outcome"], "cancelled");
    assert_eq!(summary["answered"], 0);
    assert_eq!(summary["unanswered"], 2);
    assert_eq!(summary["blocking_unanswered"], 1);
    assert_eq!(summary["default_answer_valid"], true);

    let bad = write_json(
        &workspace,
        "bad-answers.json",
        &serde_json::json!([{"question_id": "q1", "status": "answered"}]),
    );
    let output = Command::new(binary())
        .args([
            "clarify",
            "--questions",
            &questions,
            "--answers",
            &bad,
            "--outcome",
            "submitted",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("must carry an option id"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn clarify_rejects_invalid_questions_and_oversized_blocking_batches() {
    let workspace = tempfile::tempdir().unwrap();
    let invalid = write_json(
        &workspace,
        "invalid.json",
        &serde_json::json!([{
            "id": "q1",
            "prompt": "Only one option?",
            "options": [{"id": "a", "label": "A"}]
        }]),
    );
    let output = Command::new(binary())
        .args(["clarify", "--questions", &invalid, "--json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("2..=4 options"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let blocking = (0..6)
        .map(|index| {
            let mut question = question(&format!("q{index}"), &format!("Question {index}?"));
            question["blocking"] = serde_json::json!(true);
            question
        })
        .collect::<Vec<_>>();
    let oversized = write_json(
        &workspace,
        "blocking.json",
        &serde_json::Value::Array(blocking),
    );
    let output = Command::new(binary())
        .args([
            "clarify",
            "--questions",
            &oversized,
            "--max-questions",
            "5",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("five blocking questions"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
