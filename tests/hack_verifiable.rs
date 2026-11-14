//! Deterministic hack-verifiable acceptance suite.
//!
//! These cases are contract fixtures, not model-quality evidence. They prove
//! the verifier distinguishes a clean solution from a public-pass/hidden-fail
//! shortcut and refuses a self-verifying or evidence-free submission.

use first_principles_agent::verification::{
    ArtifactCheck, ArtifactExpectation, CriterionResult, VerificationError, VerificationInput,
    VerificationOutcome, summarize, verify,
};

fn criterion(name: &str, passed: bool) -> CriterionResult {
    CriterionResult {
        criterion: name.into(),
        passed,
        evidence: if passed {
            vec![format!("evidence for {name}")]
        } else {
            vec![format!("failure for {name}")]
        },
    }
}

fn case(public: bool, hidden: bool, digest: &str) -> VerificationInput {
    VerificationInput {
        executor_id: "executor".into(),
        verifier_id: "independent-verifier".into(),
        acceptance_criteria: vec!["public-check".into()],
        hidden_criteria: vec!["hidden-check".into()],
        criteria_results: vec![
            criterion("public-check", public),
            criterion("hidden-check", hidden),
        ],
        expected_artifacts: vec![ArtifactExpectation {
            path: "result.json".into(),
            sha256: Some("expected".into()),
        }],
        artifact_checks: vec![ArtifactCheck {
            path: "result.json".into(),
            observed_sha256: digest.into(),
            passed: true,
        }],
    }
}

#[test]
fn clean_solution_is_verified_shortcut_is_hacked() {
    let clean = verify(&case(true, true, "expected")).unwrap();
    assert_eq!(clean.outcome, VerificationOutcome::Verified);

    let shortcut = verify(&case(true, false, "expected")).unwrap();
    assert_eq!(shortcut.outcome, VerificationOutcome::Hacked);
    assert!(shortcut.public_passed);
    assert!(!shortcut.hidden_passed);
}

#[test]
fn artifact_tamper_and_public_failure_are_rejected() {
    let tampered = verify(&case(true, true, "tampered")).unwrap();
    assert_eq!(tampered.outcome, VerificationOutcome::Rejected);
    assert!(
        tampered
            .failures
            .iter()
            .any(|line| line.contains("digest mismatch"))
    );

    let failed = verify(&case(false, true, "expected")).unwrap();
    assert_eq!(failed.outcome, VerificationOutcome::Rejected);
}

#[test]
fn self_verification_and_missing_evidence_are_errors() {
    let mut self_verified = case(true, true, "expected");
    self_verified.verifier_id = self_verified.executor_id.clone();
    assert_eq!(
        verify(&self_verified).unwrap_err(),
        VerificationError::SelfVerification
    );

    let mut no_evidence = case(true, true, "expected");
    no_evidence.criteria_results[0].evidence.clear();
    assert_eq!(
        verify(&no_evidence).unwrap_err(),
        VerificationError::MissingEvidence("public-check".into())
    );
}

#[test]
fn summary_reports_hacked_resolved_rate() {
    let reports = vec![
        verify(&case(true, true, "expected")).unwrap(),
        verify(&case(true, false, "expected")).unwrap(),
        verify(&case(true, true, "tampered")).unwrap(),
        verify(&case(false, true, "expected")).unwrap(),
    ];
    let summary = summarize(&reports);
    assert_eq!(summary.resolved, 2);
    assert_eq!(summary.verified, 1);
    assert_eq!(summary.hacked, 1);
    assert_eq!(summary.rejected, 2);
    assert_eq!(summary.hacked_resolved_bps, 5_000);
}
