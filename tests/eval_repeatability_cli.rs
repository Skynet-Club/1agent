use std::process::Command;

use first_principles_agent::EvalReport;

fn run_eval() -> EvalReport {
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
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn core_suite_is_repeatable_across_eight_cases() {
    let first = run_eval();
    let second = run_eval();
    assert_eq!(first.total, 8);
    assert_eq!(first.passed, 8);
    assert_eq!(first.failed, 0);
    assert_eq!(first.hard_gate_failures, 0);
    assert_eq!(second.total, 8);
    assert_eq!(second.passed, 8);
    assert!(first.content_sha256.is_some());
    assert!(second.content_sha256.is_some());
    first.validate_integrity().unwrap();
    second.validate_integrity().unwrap();

    // The report digest includes per-case duration, so it is intentionally
    // not compared. Compare the verdict projection instead.
    let verdicts = |report: &EvalReport| {
        serde_json::json!(
            report
                .cases
                .iter()
                .map(|case| serde_json::json!({
                    "id": case.id,
                    "passed": case.passed,
                    "stop_reason": case.stop_reason,
                    "answer_status": case.answer_status,
                    "evidence": case.evidence_count,
                    "claims": case.claim_count,
                    "unknowns": case.unknown_count,
                    "flips": case.flip_condition_count,
                    "audit_ok": case.audit_ok
                }))
                .collect::<Vec<_>>()
        )
    };
    assert_eq!(verdicts(&first), verdicts(&second));

    let ids = first
        .cases
        .iter()
        .map(|case| case.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        [
            "read-evidence",
            "needs-authorization",
            "honest-without-reasoning-provider",
            "hidden-control-rejected",
            "missing-file",
            "zero-width-control-rejected",
            "read-directory-rejected",
            "sleep-then-honest"
        ]
    );
    assert_eq!(
        first
            .cases
            .iter()
            .map(|case| (case.id.clone(), case.passed))
            .collect::<Vec<_>>(),
        second
            .cases
            .iter()
            .map(|case| (case.id.clone(), case.passed))
            .collect::<Vec<_>>()
    );
}
