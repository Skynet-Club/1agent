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
