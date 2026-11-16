//! Black-box CLI tests for `1agent goal status|link|close`.

use std::path::Path;
use std::process::{Command, Output};

use first_principles_agent::goal::{CommitmentStatus, GoalStore};
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
    let journal = dir.path().join("goals.jsonl");
    seed(&journal);
    let mut store = GoalStore::open(&journal).unwrap();
    store
        .with_graph_mut(|graph| {
            graph.close_commitment("commit-1", CommitmentStatus::Kept, "done", 3)
        })
        .unwrap();
    drop(store);
    let journal = journal.to_str().unwrap();

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

    let failed = run(&[
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
        "failed",
        "--run",
        "run-1",
    ]);
    assert!(!failed.status.success());

    let closed = run(&[
        "goal",
        "close",
        "--journal",
        journal,
        "--goal-id",
        "goal-1",
        "--closure",
        "achieved",
        "--reason",
        "verified",
        "--verification",
        "passed",
        "--run",
        "run-1",
        "--json",
    ]);
    let value = parse_stdout(&closed);
    assert!(closed.status.success(), "{value}");
    assert_eq!(value["status"], "已达成");

    let status = run(&["goal", "status", "--journal", journal, "--json"]);
    let value = parse_stdout(&status);
    assert_eq!(value["goals"][0]["status"], "achieved");
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
