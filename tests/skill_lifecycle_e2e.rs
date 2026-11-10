//! End-to-end evidence for the skill lifecycle:
//! demonstration capture -> draft -> static evaluation -> activation -> rollback.
//!
//! The scenario runs against a real on-disk skill journal, rejects a failing
//! candidate without touching the active version, and replays the journal after
//! a restart.

use first_principles_agent::{
    AuditKind, CandidateStatus, SkillCandidateStore, SkillEvalCase, SkillEvalSuite, SkillEventKind,
    SkillManifest, SkillRisk, SkillService, audit_store,
};

fn manifest(version: &str, body: &str) -> SkillManifest {
    SkillManifest {
        name: "release".into(),
        description: "release process".into(),
        version: version.into(),
        platforms: vec!["linux".into()],
        scope: "repo:1-agent".into(),
        requires_tools: vec!["shell".into()],
        risk: SkillRisk::Low,
        body: body.into(),
        content_sha256: String::new(),
    }
}

fn suite() -> SkillEvalSuite {
    SkillEvalSuite {
        suite_id: "release-heldout".into(),
        min_score_bps: 10_000,
        cases: vec![
            SkillEvalCase {
                case_id: "verify".into(),
                required_markers: vec!["verify".into()],
                forbidden_markers: vec!["rm -rf".into()],
            },
            SkillEvalCase {
                case_id: "build".into(),
                required_markers: vec!["build".into()],
                forbidden_markers: Vec::new(),
            },
        ],
    }
}

#[test]
fn skill_lifecycle_capture_to_rollback_is_replayable() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("skill-journal.jsonl");

    // Baseline: v1.0.0 is registered and active before any candidate exists.
    let mut skills = SkillService::open(&journal).unwrap();
    skills
        .register(manifest("1.0.0", "1. verify\n2. build"), 10)
        .unwrap();
    skills.activate("release", "1.0.0", 11).unwrap();

    // Capture: demonstrations become hashes and lengths, never raw content.
    let mut candidates = SkillCandidateStore::default();
    let draft = candidates.propose(manifest("1.1.0", "1. verify\n2. build\n3. report"), 12);
    assert_eq!(draft.status, CandidateStatus::Proposed);
    let demonstration = candidates
        .capture(
            &draft.candidate_id,
            "private demo input",
            "private demo output",
            true,
            13,
        )
        .unwrap();
    assert_eq!(demonstration.input_sha256.len(), 64);
    assert_eq!(demonstration.output_sha256.len(), 64);
    assert_eq!(demonstration.input_chars, 18);
    assert!(!format!("{demonstration:?}").contains("private"));
    let serialized = serde_json::to_string(candidates.get(&draft.candidate_id).unwrap()).unwrap();
    assert!(!serialized.contains("private demo input"));
    assert!(!serialized.contains("private demo output"));

    // Static evaluation is a hard gate; v1.1.0 passes every held-out case.
    let report = candidates.evaluate(&draft.candidate_id, &suite()).unwrap();
    assert!(report.passed);
    assert_eq!(report.score_bps, 10_000);
    assert_eq!(report.cases.len(), 2);

    // Activation goes through the evaluated candidate, not a bare register.
    candidates
        .activate(&draft.candidate_id, &mut skills, 14)
        .unwrap();
    assert_eq!(skills.active("release").unwrap().version, "1.1.0");
    assert_eq!(
        candidates.get(&draft.candidate_id).unwrap().status,
        CandidateStatus::Activated
    );

    // Negative migration: a failing candidate is rejected and cannot replace
    // the active version.
    let bad = candidates.propose(manifest("1.2.0", "1. verify\n2. rm -rf /"), 15);
    let bad_report = candidates.evaluate(&bad.candidate_id, &suite()).unwrap();
    assert!(!bad_report.passed);
    assert!(bad_report.score_bps < 10_000);
    assert!(
        candidates
            .activate(&bad.candidate_id, &mut skills, 16)
            .is_err()
    );
    assert_eq!(
        candidates.get(&bad.candidate_id).unwrap().status,
        CandidateStatus::Rejected
    );
    assert_eq!(skills.active("release").unwrap().version, "1.1.0");

    // Rollback restores the previous active version and is journaled.
    skills.rollback("release", "1.0.0", 17).unwrap();
    assert_eq!(skills.active("release").unwrap().version, "1.0.0");

    let events = skills.events();
    assert_eq!(events.len(), 5);
    assert!(events.windows(2).all(|pair| pair[0].seq + 1 == pair[1].seq));
    assert!(matches!(
        &events.last().unwrap().kind,
        SkillEventKind::Activated { name, version }
            if name.as_str() == "release" && version.as_str() == "1.0.0"
    ));
    let event_count = events.len();
    let journal_text = std::fs::read_to_string(&journal).unwrap();
    assert!(!journal_text.contains("private demo input"));
    assert!(!journal_text.contains("private demo output"));

    // Restart: replay from disk preserves the active version and the evidence.
    drop(skills);
    let reopened = SkillService::open(&journal).unwrap();
    assert_eq!(reopened.active("release").unwrap().version, "1.0.0");
    assert_eq!(reopened.events().len(), event_count);
    let receipt = audit_store(AuditKind::Skill, &journal, 18).unwrap();
    assert!(receipt.verified);
    assert_eq!(receipt.events, event_count);
}
