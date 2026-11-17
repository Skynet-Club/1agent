//! Black-box CLI tests for `1agent goal status|link|close`.

use std::path::Path;
use std::process::{Command, Output};

use first_principles_agent::goal::{CommitmentStatus, GoalStore};
use first_principles_agent::verification::{VerificationOutcome, VerificationReport};
use first_principles_agent::verification_store::{VerificationRecord, VerificationStore};
use serde_json::Value;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_first-principles-agent")
}

fn run(args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("failed to run {args:?}: {error}"))
}

fn parse_stdout(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout is not JSON: {error}\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn seed(path: &Path) {
    let mut store = GoalStore::open(path).unwrap();
    store
        .with_graph_mut(|graph| {
            graph.create_goal("goal-1", "owner-1", "ship the release", None, 1)?;
            graph.add_commitment("commit-1", "goal-1", "write release notes", None, 2)
        })
        .unwrap();
}

fn append_verification(
    path: &Path,
    verification_id: &str,
    run_id: &str,
    outcome: VerificationOutcome,
) {
    let mut store = VerificationStore::open(path).unwrap();
    store
        .append(VerificationRecord {
            verification_id: verification_id.into(),
            run_id: run_id.into(),
            at_ms: 10,
            report: VerificationReport {
                outcome,
                executor_id: "executor-1".into(),
                verifier_id: "verifier-1".into(),
                public_passed: outcome == VerificationOutcome::Verified,
                hidden_passed: outcome == VerificationOutcome::Verified,
                failures: Vec::new(),
            },
        })
        .unwrap();
}

fn close_achieved(journal: &Path, store: &Path, verification_id: &str) -> Output {
    run(&[
        "goal",
        "close",
        "--journal",
        journal.to_str().unwrap(),
        "--goal-id",
        "goal-1",
        "--closure",
        "achieved",
        "--reason",
        "verified",
        "--run",
        "run-1",
        "--verification",
        "passed",
        "--verification-store",
        store.to_str().unwrap(),
        "--verification-id",
        verification_id,
        "--json",
    ])
}

#[test]
fn goal_status_and_link_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("goals.jsonl");
    seed(&journal);
    let journal = journal.to_str().unwrap();

    let status = run(&["goal", "status", "--journal", journal, "--json"]);
    let value = parse_stdout(&status);
    assert!(status.status.success(), "{value}");
    assert_eq!(value["goals"][0]["goal_id"], "goal-1");
    assert_eq!(value["goals"][0]["status"], "active");
    assert_eq!(value["commitments"][0]["commitment_id"], "commit-1");

    let session = run(&[
        "goal",
        "link",
        "--journal",
        journal,
        "--goal-id",
        "goal-1",
        "--session",
        "ses-1",
        "--json",
    ]);
    let value = parse_stdout(&session);
    assert!(session.status.success(), "{value}");
    assert_eq!(value["session_id"], "ses-1");

    let run_link = run(&[
        "goal",
        "link",
        "--journal",
        journal,
        "--goal-id",
        "goal-1",
        "--run",
        "run-1",
        "--json",
    ]);
    assert!(run_link.status.success());

    let status = run(&[
        "goal",
        "status",
        "--journal",
        journal,
        "--goal-id",
        "goal-1",
        "--json",
    ]);
    let value = parse_stdout(&status);
    assert!(status.status.success(), "{value}");
    let binding = &value["bindings"][0];
    assert_eq!(binding["session_id"], "ses-1");
    assert_eq!(binding["run_ids"], serde_json::json!(["run-1"]));
    assert_eq!(binding["commitment_ids"], serde_json::json!(["commit-1"]));
}

#[test]
fn goal_close_requires_a_linked_run_and_passed_verification() {
    let dir = tempfile::tempdir().unwrap();
    let journal_path = dir.path().join("goals.jsonl");
    let verification_path = dir.path().join("verification.jsonl");
    seed(&journal_path);
    let mut store = GoalStore::open(&journal_path).unwrap();
    store
        .with_graph_mut(|graph| {
            graph.close_commitment("commit-1", CommitmentStatus::Kept, "done", 3)
        })
        .unwrap();
    drop(store);
    let journal = journal_path.to_str().unwrap();

    let missing_run = run(&[
        "goal",
        "close",
        "--journal",
        journal,
        "--goal-id",
        "goal-1",
        "--closure",
        "achieved",
        "--reason",
        "done",
        "--verification",
        "passed",
    ]);
    assert!(!missing_run.status.success());

    let link = run(&[
        "goal",
        "link",
        "--journal",
        journal,
        "--goal-id",
        "goal-1",
        "--run",
        "run-1",
    ]);
    assert!(link.status.success());

    let no_record = close_achieved(&journal_path, &verification_path, "missing");
    assert!(!no_record.status.success());

    append_verification(
        &verification_path,
        "v-1",
        "run-1",
        VerificationOutcome::Verified,
    );
    let closed = close_achieved(&journal_path, &verification_path, "v-1");
    let value = parse_stdout(&closed);
    assert!(closed.status.success(), "{value}");
    assert_eq!(value["status"], "已达成");

    let status = run(&["goal", "status", "--journal", journal, "--json"]);
    let value = parse_stdout(&status);
    assert_eq!(value["goals"][0]["status"], "achieved");

    let reopened = GoalStore::open(&journal_path).unwrap();
    let evidence = reopened
        .graph()
        .events()
        .iter()
        .find_map(|event| match &event.kind {
            first_principles_agent::goal::GoalEventKind::GoalStatusChanged {
                status: first_principles_agent::goal::GoalStatus::Achieved,
                verification_evidence,
                ..
            } => verification_evidence.clone(),
            _ => None,
        })
        .expect("closure evidence");
    assert_eq!(evidence.verification_id, "v-1");
    assert_eq!(evidence.run_id, "run-1");
}

#[test]
fn goal_close_rejects_mismatched_or_unverified_records() {
    let dir = tempfile::tempdir().unwrap();
    let journal_path = dir.path().join("goals.jsonl");
    let verification_path = dir.path().join("verification.jsonl");
    seed(&journal_path);
    let mut store = GoalStore::open(&journal_path).unwrap();
    store
        .with_graph_mut(|graph| {
            graph.close_commitment("commit-1", CommitmentStatus::Kept, "done", 3)
        })
        .unwrap();
    drop(store);
    let journal = journal_path.to_str().unwrap();
    let link = run(&[
        "goal",
        "link",
        "--journal",
        journal,
        "--goal-id",
        "goal-1",
        "--run",
        "run-1",
    ]);
    assert!(link.status.success());

    append_verification(
        &verification_path,
        "v-other-run",
        "run-2",
        VerificationOutcome::Verified,
    );
    assert!(
        !close_achieved(&journal_path, &verification_path, "v-other-run")
            .status
            .success()
    );

    append_verification(
        &verification_path,
        "v-rejected",
        "run-1",
        VerificationOutcome::Rejected,
    );
    assert!(
        !close_achieved(&journal_path, &verification_path, "v-rejected")
            .status
            .success()
    );

    append_verification(
        &verification_path,
        "v-hacked",
        "run-1",
        VerificationOutcome::Hacked,
    );
    assert!(
        !close_achieved(&journal_path, &verification_path, "v-hacked")
            .status
            .success()
    );

    let status = run(&["goal", "status", "--journal", journal, "--json"]);
    let value = parse_stdout(&status);
    assert_eq!(value["goals"][0]["status"], "active");
}

#[test]
fn goal_close_abandoned_requires_listing_pending_commitments() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("goals.jsonl");
    seed(&journal);
    let journal = journal.to_str().unwrap();

    let unlisted = run(&[
        "goal",
        "close",
        "--journal",
        journal,
        "--goal-id",
        "goal-1",
        "--closure",
        "abandoned",
        "--reason",
        "scope dropped",
    ]);
    assert!(!unlisted.status.success());

    let closed = run(&[
        "goal",
        "close",
        "--journal",
        journal,
        "--goal-id",
        "goal-1",
        "--closure",
        "abandoned",
        "--reason",
        "scope dropped",
        "--open-commitment",
        "commit-1",
        "--json",
    ]);
    let value = parse_stdout(&closed);
    assert!(closed.status.success(), "{value}");
    assert_eq!(value["status"], "已放弃");

    let status = run(&["goal", "status", "--journal", journal, "--json"]);
    let value = parse_stdout(&status);
    assert_eq!(value["goals"][0]["status"], "abandoned");
    assert_eq!(value["commitments"][0]["status"], "cancelled");
}

#[test]
fn goal_link_requires_a_session_or_run() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("goals.jsonl");
    seed(&journal);
    let output = run(&[
        "goal",
        "link",
        "--journal",
        journal.to_str().unwrap(),
        "--goal-id",
        "goal-1",
    ]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--session"),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}
