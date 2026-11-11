use std::process::Command;

#[test]
fn digest_counts_inbox_and_open_handoffs() {
    let workspace = tempfile::tempdir().unwrap();
    let journals = workspace.path().join("journals");
    std::fs::create_dir_all(&journals).unwrap();
    let schedules = workspace.path().join("schedules.json");
    let inbox = workspace.path().join("inbox.jsonl");
    let handoffs = workspace.path().join("handoffs.jsonl");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");

    let added = Command::new(bin)
        .args(["inbox", "add", "--store"])
        .arg(&inbox)
        .args(["--title", "Approve the deploy", "--kind", "review"])
        .output()
        .unwrap();
    assert!(added.status.success());

    let created = Command::new(bin)
        .args(["handoff", "create", "--store"])
        .arg(&handoffs)
        .args([
            "--id",
            "h1",
            "--from",
            "alice",
            "--to",
            "bob",
            "--summary",
            "prepared the release",
            "--evidence",
            "journal:run-1",
        ])
        .output()
        .unwrap();
    assert!(created.status.success());

    let digest = Command::new(bin)
        .args(["digest", "--journal-dir"])
        .arg(&journals)
        .args(["--schedule-store"])
        .arg(&schedules)
        .args(["--inbox-store"])
        .arg(&inbox)
        .args(["--handoff-store"])
        .arg(&handoffs)
        .args(["--recipient", "bob", "--json"])
        .output()
        .unwrap();
    assert!(
        digest.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&digest.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&digest.stdout).unwrap();
    assert_eq!(report["inbox"]["open"], 1);
    assert_eq!(report["handoffs"]["open"], 1);
    assert_eq!(report["handoffs"]["recipient"], "bob");

    let accepted = Command::new(bin)
        .args(["handoff", "accept", "--store"])
        .arg(&handoffs)
        .args(["--id", "h1", "--actor", "bob"])
        .output()
        .unwrap();
    assert!(accepted.status.success());

    let after = Command::new(bin)
        .args(["digest", "--journal-dir"])
        .arg(&journals)
        .args(["--schedule-store"])
        .arg(&schedules)
        .args(["--inbox-store"])
        .arg(&inbox)
        .args(["--handoff-store"])
        .arg(&handoffs)
        .args(["--recipient", "bob", "--json"])
        .output()
        .unwrap();
    assert!(after.status.success());
    let report: serde_json::Value = serde_json::from_slice(&after.stdout).unwrap();
    assert_eq!(report["handoffs"]["open"], 0);
}

#[test]
fn digest_rejects_recipient_without_handoff_store() {
    let workspace = tempfile::tempdir().unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args(["digest", "--journal-dir"])
        .arg(workspace.path().join("journals"))
        .args(["--schedule-store"])
        .arg(workspace.path().join("schedules.json"))
        .args(["--inbox-store"])
        .arg(workspace.path().join("inbox.jsonl"))
        .args(["--recipient", "bob"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}
