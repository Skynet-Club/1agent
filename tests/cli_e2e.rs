use std::process::Command;

use first_principles_agent::{
    DecisionCompletenessFloorSource, EvalReport, EvalReportSummary, JournalAudit, ReplayState,
    RunResult, StopReason,
};

#[test]
fn cli_run_audit_and_replay_form_a_complete_flywheel() {
    let workspace = tempfile::tempdir().unwrap();
    let evidence_dir = workspace.path().join("evidence dir");
    std::fs::create_dir_all(&evidence_dir).unwrap();
    let evidence = evidence_dir.join("decision.txt");
    std::fs::write(
        &evidence,
        "Verified fact: option A has lower total cost under the stated constraints.",
    )
    .unwrap();
    let journal_dir = workspace.path().join("journal");
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");

    let output = Command::new(binary)
        .args([
            "run",
            "--task",
            &format!(
                "Compare option A and B using @read-file:\"{}\"",
                evidence.display()
            ),
            "--read-root",
            &workspace.path().display().to_string(),
            "--journal-dir",
            &journal_dir.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: RunResult = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result.stop_reason, StopReason::Completed);
    assert_eq!(result.answer.evidence.len(), 1);
    assert!(result.journal_path.exists());

    let audit_output = Command::new(binary)
        .args([
            "audit",
            "--journal",
            &result.journal_path.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        audit_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&audit_output.stderr)
    );
    let audit: JournalAudit = serde_json::from_slice(&audit_output.stdout).unwrap();
    assert!(audit.ok, "{:?}", audit.issues);
    assert!(audit.findings.is_empty());

    let replay_output = Command::new(binary)
        .args([
            "replay",
            "--journal",
            &result.journal_path.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(
        replay_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&replay_output.stderr)
    );
    let replay: ReplayState = serde_json::from_slice(&replay_output.stdout).unwrap();
    assert_eq!(replay.stop_reason, Some(StopReason::Completed));
    assert_eq!(replay.tool_calls.len(), 1);
    assert_eq!(replay.observations.len(), 1);
}

#[test]
fn cli_eval_reports_versioned_completeness_policy() {
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let suite = format!("{}/evals/core.json", env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(binary)
        .args(["eval", "--suite", &suite, "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: EvalReport = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report.decision_completeness_policy.version, 1);
    assert_eq!(
        report.decision_completeness_policy.floor_source,
        DecisionCompletenessFloorSource::Default
    );

    let report_path = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(report_path.path(), &output.stdout).unwrap();
    let inspect_output = Command::new(binary)
        .args([
            "eval-inspect",
            "--report",
            &report_path.path().display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        inspect_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&inspect_output.stderr)
    );
    let inspected: EvalReport = serde_json::from_slice(&inspect_output.stdout).unwrap();
    assert_eq!(inspected, report);

    let summary_output = Command::new(binary)
        .args([
            "eval-inspect",
            "--report",
            &report_path.path().display().to_string(),
            "--summary",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        summary_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&summary_output.stderr)
    );
    let summary: EvalReportSummary = serde_json::from_slice(&summary_output.stdout).unwrap();
    assert_eq!(summary.schema_version, 2);
    assert_eq!(summary.total, report.total);
    assert_eq!(summary.passed, report.passed);
    assert_eq!(summary.report_content_sha256, report.content_sha256);
    assert_eq!(
        summary.report_content_sha256.as_deref().map(str::len),
        Some(64)
    );
    let summary_json: serde_json::Value = serde_json::from_slice(&summary_output.stdout).unwrap();
    assert!(summary_json.get("cases").is_none());
}

#[test]
fn cli_eval_inspect_rejects_unsupported_policy_version() {
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let suite = format!("{}/evals/core.json", env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(binary)
        .args(["eval", "--suite", &suite, "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let mut report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    report["decision_completeness_policy"]["version"] = serde_json::json!(2);
    let report_path = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(report_path.path(), serde_json::to_vec(&report).unwrap()).unwrap();

    let inspect_output = Command::new(binary)
        .args([
            "eval-inspect",
            "--report",
            &report_path.path().display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!inspect_output.status.success());
    assert!(
        String::from_utf8_lossy(&inspect_output.stderr)
            .contains("unsupported decision-completeness policy version: 2")
    );

    report["decision_completeness_policy"]["version"] = serde_json::json!(1);
    report["total"] = serde_json::json!(99);
    std::fs::write(report_path.path(), serde_json::to_vec(&report).unwrap()).unwrap();
    let inspect_output = Command::new(binary)
        .args([
            "eval-inspect",
            "--report",
            &report_path.path().display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!inspect_output.status.success());
    assert!(
        String::from_utf8_lossy(&inspect_output.stderr).contains("case count does not match total")
    );
}
