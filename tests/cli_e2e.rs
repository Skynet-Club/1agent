use std::process::Command;

use first_principles_agent::{
    AuthenticityStatus, ContentBindingStatus, DecisionCompletenessFloorSource, EvalReport,
    EvalReportSummary, JournalAudit, ReplayState, RunResult, StopReason,
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
    assert_eq!(summary.schema_version, 4);
    assert_eq!(summary.total, report.total);
    assert_eq!(summary.passed, report.passed);
    assert_eq!(summary.report_content_sha256, report.content_sha256);
    assert_eq!(
        summary.report_content_sha256.as_deref().map(str::len),
        Some(64)
    );
    assert!(summary.verification.integrity_checked);
    assert!(!summary.verification.signature_present);
    assert_eq!(
        summary.verification.content_binding,
        ContentBindingStatus::Bound
    );
    assert_eq!(
        summary.verification.authenticity,
        AuthenticityStatus::Unauthenticated
    );
    let summary_json: serde_json::Value = serde_json::from_slice(&summary_output.stdout).unwrap();
    assert!(summary_json.get("cases").is_none());

    let binding_output = Command::new(binary)
        .args([
            "eval-inspect",
            "--report",
            &report_path.path().display().to_string(),
            "--require-content-binding",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        binding_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&binding_output.stderr)
    );

    let authenticity_output = Command::new(binary)
        .args([
            "eval-inspect",
            "--report",
            &report_path.path().display().to_string(),
            "--require-authenticity",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!authenticity_output.status.success());
    assert!(
        String::from_utf8_lossy(&authenticity_output.stderr).contains(
            "report authenticity is required but no verified report signature was provided"
        )
    );
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

#[test]
fn cli_eval_inspect_verifies_signed_report() {
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let workspace = tempfile::tempdir().unwrap();
    let suite = format!("{}/evals/core.json", env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(binary)
        .args(["eval", "--suite", &suite, "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report_path = workspace.path().join("report.json");
    std::fs::write(&report_path, &output.stdout).unwrap();

    let registry_path = workspace.path().join("approval-registry.json");
    let key_add = Command::new(binary)
        .args([
            "approval",
            "key-add",
            "--registry",
            &registry_path.display().to_string(),
            "--key-id",
            "report-key",
        ])
        .output()
        .unwrap();
    assert!(
        key_add.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&key_add.stderr)
    );

    let approval_path = workspace.path().join("report.approval.json");
    let sign = Command::new(binary)
        .env("REPORT_APPROVAL_KEY", "test-only-report-key-32-bytes!!!")
        .args([
            "approval",
            "sign",
            "--artifact",
            &report_path.display().to_string(),
            "--artifact-kind",
            "evaluation_report",
            "--approver",
            "qa",
            "--purpose",
            "report-inspection",
            "--key-id",
            "report-key",
            "--key-env",
            "REPORT_APPROVAL_KEY",
            "--registry",
            &registry_path.display().to_string(),
            "--output",
            &approval_path.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(
        sign.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&sign.stderr)
    );

    let inspect = Command::new(binary)
        .env("REPORT_APPROVAL_KEY", "test-only-report-key-32-bytes!!!")
        .args([
            "eval-inspect",
            "--report",
            &report_path.display().to_string(),
            "--summary",
            "--json",
            "--require-authenticity",
            "--approval",
            &approval_path.display().to_string(),
            "--approval-key-env",
            "REPORT_APPROVAL_KEY",
            "--approval-registry",
            &registry_path.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(
        inspect.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&inspect.stderr)
    );
    let summary: EvalReportSummary = serde_json::from_slice(&inspect.stdout).unwrap();
    assert!(summary.verification.signature_present);
    assert_eq!(
        summary.verification.authenticity,
        AuthenticityStatus::Verified
    );

    let partial = Command::new(binary)
        .args([
            "eval-inspect",
            "--report",
            &report_path.display().to_string(),
            "--require-authenticity",
            "--approval",
            &approval_path.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(!partial.status.success());
    assert!(String::from_utf8_lossy(&partial.stderr).contains("must be supplied together"));

    let wrong_kind_path = workspace.path().join("wrong-kind.approval.json");
    let wrong_kind = Command::new(binary)
        .env("REPORT_APPROVAL_KEY", "test-only-report-key-32-bytes!!!")
        .args([
            "approval",
            "sign",
            "--artifact",
            &report_path.display().to_string(),
            "--artifact-kind",
            "gold_set",
            "--approver",
            "qa",
            "--purpose",
            "wrong-kind",
            "--key-id",
            "report-key",
            "--key-env",
            "REPORT_APPROVAL_KEY",
            "--registry",
            &registry_path.display().to_string(),
            "--output",
            &wrong_kind_path.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(wrong_kind.status.success());
    let wrong_kind_inspect = Command::new(binary)
        .env("REPORT_APPROVAL_KEY", "test-only-report-key-32-bytes!!!")
        .args([
            "eval-inspect",
            "--report",
            &report_path.display().to_string(),
            "--require-authenticity",
            "--approval",
            &wrong_kind_path.display().to_string(),
            "--approval-key-env",
            "REPORT_APPROVAL_KEY",
            "--approval-registry",
            &registry_path.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(!wrong_kind_inspect.status.success());
    assert!(
        String::from_utf8_lossy(&wrong_kind_inspect.stderr)
            .contains("approval artifact kind does not match the required use")
    );

    let revoke = Command::new(binary)
        .args([
            "approval",
            "key-revoke",
            "--registry",
            &registry_path.display().to_string(),
            "--key-id",
            "report-key",
            "--reason",
            "test rotation",
        ])
        .output()
        .unwrap();
    assert!(revoke.status.success());
    let revoked_inspect = Command::new(binary)
        .env("REPORT_APPROVAL_KEY", "test-only-report-key-32-bytes!!!")
        .args([
            "eval-inspect",
            "--report",
            &report_path.display().to_string(),
            "--require-authenticity",
            "--approval",
            &approval_path.display().to_string(),
            "--approval-key-env",
            "REPORT_APPROVAL_KEY",
            "--approval-registry",
            &registry_path.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(!revoked_inspect.status.success());
    assert!(String::from_utf8_lossy(&revoked_inspect.stderr).contains("approval key is revoked"));
}
