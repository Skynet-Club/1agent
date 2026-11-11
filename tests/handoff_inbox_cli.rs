use std::process::Command;

#[test]
fn handoffs_project_into_the_recipient_inbox_once() {
    let workspace = tempfile::tempdir().unwrap();
    let handoffs = workspace.path().join("handoffs.jsonl");
    let inbox = workspace.path().join("inbox.jsonl");
    let handoffs = handoffs.to_str().unwrap();
    let inbox = inbox.to_str().unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");

    let created = Command::new(bin)
        .args([
            "handoff",
            "create",
            "--store",
            handoffs,
            "--id",
            "h1",
            "--from",
            "alice",
            "--to",
            "bob",
            "--scope",
            "release",
            "--summary",
            "prepared the release candidate",
            "--evidence",
            "journal:run-1",
        ])
        .output()
        .unwrap();
    assert!(created.status.success());

    let dry = Command::new(bin)
        .args([
            "inbox",
            "sync",
            "--store",
            inbox,
            "--handoff-store",
            handoffs,
            "--recipient",
            "bob",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(dry.status.success());
    let report: serde_json::Value = serde_json::from_slice(&dry.stdout).unwrap();
    assert_eq!(report["added"], 1);
    assert_eq!(report["dry_run"], true);

    let other = Command::new(bin)
        .args([
            "inbox",
            "sync",
            "--store",
            inbox,
            "--handoff-store",
            handoffs,
            "--recipient",
            "alice",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(other.status.success());
    let report: serde_json::Value = serde_json::from_slice(&other.stdout).unwrap();
    assert_eq!(report["scanned"], 0);

    for expected in [1, 0] {
        let applied = Command::new(bin)
            .args([
                "inbox",
                "sync",
                "--store",
                inbox,
                "--handoff-store",
                handoffs,
                "--recipient",
                "bob",
                "--apply",
                "--json",
            ])
            .output()
            .unwrap();
        assert!(applied.status.success());
        let report: serde_json::Value = serde_json::from_slice(&applied.stdout).unwrap();
        assert_eq!(report["added"], expected);
    }

    let listed = Command::new(bin)
        .args(["inbox", "list", "--store", inbox, "--json"])
        .output()
        .unwrap();
    assert!(listed.status.success());
    let items: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(items.as_array().unwrap().len(), 1);
    assert_eq!(items[0]["item_id"], "handoff:h1");
    assert_eq!(items[0]["kind"], "review");
}

#[test]
fn inbox_reconcile_closes_accepted_and_cancelled_handoffs() {
    let workspace = tempfile::tempdir().unwrap();
    let handoffs = workspace.path().join("handoffs.jsonl");
    let inbox = workspace.path().join("inbox.jsonl");
    let handoffs = handoffs.to_str().unwrap();
    let inbox = inbox.to_str().unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");

    for (id, title) in [("h1", "cancelled"), ("h2", "accepted")] {
        let created = Command::new(bin)
            .args(["handoff", "create", "--store", handoffs, "--id", id])
            .args([
                "--from",
                "alice",
                "--to",
                "bob",
                "--summary",
                title,
                "--evidence",
                "journal:run-1",
            ])
            .output()
            .unwrap();
        assert!(created.status.success());
    }
    let synced = Command::new(bin)
        .args([
            "inbox",
            "sync",
            "--store",
            inbox,
            "--handoff-store",
            handoffs,
            "--recipient",
            "bob",
            "--apply",
        ])
        .output()
        .unwrap();
    assert!(synced.status.success());

    let cancelled = Command::new(bin)
        .args([
            "handoff", "cancel", "--store", handoffs, "--id", "h1", "--actor", "alice",
        ])
        .output()
        .unwrap();
    assert!(cancelled.status.success());
    let accepted = Command::new(bin)
        .args([
            "handoff", "accept", "--store", handoffs, "--id", "h2", "--actor", "bob",
        ])
        .output()
        .unwrap();
    assert!(accepted.status.success());

    let dry = Command::new(bin)
        .args([
            "inbox",
            "reconcile",
            "--store",
            inbox,
            "--handoff-store",
            handoffs,
            "--json",
        ])
        .output()
        .unwrap();
    assert!(dry.status.success());
    let report: serde_json::Value = serde_json::from_slice(&dry.stdout).unwrap();
    assert_eq!(report["resolved"], 2);
    assert_eq!(report["dry_run"], true);

    let applied = Command::new(bin)
        .args([
            "inbox",
            "reconcile",
            "--store",
            inbox,
            "--handoff-store",
            handoffs,
            "--apply",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(applied.status.success());
    let report: serde_json::Value = serde_json::from_slice(&applied.stdout).unwrap();
    assert_eq!(report["resolved"], 2);
    assert_eq!(report["dry_run"], false);

    let listed = Command::new(bin)
        .args(["inbox", "list", "--store", inbox, "--json"])
        .output()
        .unwrap();
    let items: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(items.as_array().unwrap().len(), 0);
}

#[test]
fn inbox_resolve_accepts_the_linked_handoff() {
    let workspace = tempfile::tempdir().unwrap();
    let handoffs = workspace.path().join("handoffs.jsonl");
    let inbox = workspace.path().join("inbox.jsonl");
    let handoffs = handoffs.to_str().unwrap();
    let inbox = inbox.to_str().unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");

    let created = Command::new(bin)
        .args([
            "handoff",
            "create",
            "--store",
            handoffs,
            "--id",
            "h1",
            "--from",
            "alice",
            "--to",
            "bob",
            "--summary",
            "prepared the release candidate",
            "--evidence",
            "journal:run-1",
        ])
        .output()
        .unwrap();
    assert!(created.status.success());
    let synced = Command::new(bin)
        .args([
            "inbox",
            "sync",
            "--store",
            inbox,
            "--handoff-store",
            handoffs,
            "--recipient",
            "bob",
            "--apply",
        ])
        .output()
        .unwrap();
    assert!(synced.status.success());

    let without_store = Command::new(bin)
        .args([
            "inbox",
            "resolve",
            "--store",
            inbox,
            "--item-id",
            "handoff:h1",
            "--status",
            "approved",
        ])
        .output()
        .unwrap();
    assert!(!without_store.status.success());

    let wrong_actor = Command::new(bin)
        .args([
            "inbox",
            "resolve",
            "--store",
            inbox,
            "--item-id",
            "handoff:h1",
            "--status",
            "approved",
            "--handoff-store",
            handoffs,
            "--actor",
            "alice",
        ])
        .output()
        .unwrap();
    assert!(!wrong_actor.status.success());

    let accepted = Command::new(bin)
        .args([
            "inbox",
            "resolve",
            "--store",
            inbox,
            "--item-id",
            "handoff:h1",
            "--status",
            "approved",
            "--handoff-store",
            handoffs,
            "--actor",
            "bob",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        accepted.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&accepted.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&accepted.stdout).unwrap();
    assert_eq!(result["item"]["status"], "approved");
    assert_eq!(result["handoff"]["status"], "accepted");
    assert_eq!(result["handoff"]["accepted_by"], "bob");

    let listed = Command::new(bin)
        .args(["inbox", "list", "--store", inbox, "--json"])
        .output()
        .unwrap();
    let items: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(items.as_array().unwrap().len(), 0);
}

#[test]
fn recipient_requires_a_handoff_store() {
    let workspace = tempfile::tempdir().unwrap();
    let inbox = workspace.path().join("inbox.jsonl");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args([
            "inbox",
            "sync",
            "--store",
            inbox.to_str().unwrap(),
            "--recipient",
            "bob",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
}
