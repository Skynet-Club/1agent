use std::process::Command;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_first-principles-agent")
}

#[test]
fn answer_quality_reports_m19_to_m23() {
    let workspace = tempfile::tempdir().unwrap();
    let input = workspace.path().join("samples.jsonl");
    std::fs::write(
        &input,
        concat!(
            "{\"first_answer_usable\":true,\"pairwise_win\":true}\n",
            "{\"first_answer_usable\":true,\"pairwise_win\":false,\"over_structured\":true,\"unnecessary_followup\":true,\"followup\":true,\"followup_adopted\":true}\n",
            "{\"first_answer_usable\":false,\"followup\":true}\n",
            "{\"first_answer_usable\":true}\n"
        ),
    )
    .unwrap();
    let output = Command::new(binary())
        .args([
            "answer-quality",
            "--input",
            &input.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["samples"], 4);
    assert_eq!(report["first_answer_usable_bps"], 7_500);
    assert_eq!(report["pairwise_compared"], 2);
    assert_eq!(report["pairwise_win_bps"], 5_000);
    assert_eq!(report["over_structured_bps"], 2_500);
    assert_eq!(report["unnecessary_followup_bps"], 2_500);
    assert_eq!(report["followups"], 2);
    assert_eq!(report["followup_adoption_bps"], 5_000);

    let human = Command::new(binary())
        .args(["answer-quality", "--input", &input.display().to_string()])
        .output()
        .unwrap();
    assert!(human.status.success());
    let stdout = String::from_utf8_lossy(&human.stdout);
    assert!(stdout.contains("M19_usable=75.00%"), "stdout: {stdout}");
    assert!(stdout.contains("M20_win=50.00%"), "stdout: {stdout}");
}

#[test]
fn answer_quality_rejects_invalid_lines_and_adoption_without_followup() {
    let workspace = tempfile::tempdir().unwrap();
    let binary = binary();
    let invalid = workspace.path().join("invalid.jsonl");
    std::fs::write(
        &invalid,
        "{\"first_answer_usable\":true}\n{\"first_answer_usable\":\"yes\"}\n",
    )
    .unwrap();
    let output = Command::new(binary)
        .args(["answer-quality", "--input", &invalid.display().to_string()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("line 2"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let adopted = workspace.path().join("adopted.jsonl");
    std::fs::write(
        &adopted,
        "{\"first_answer_usable\":true,\"followup_adopted\":true}\n",
    )
    .unwrap();
    let output = Command::new(binary)
        .args(["answer-quality", "--input", &adopted.display().to_string()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("follow-up cannot be adopted"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
