use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_first-principles-agent")
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(bin()).args(args).output().unwrap()
}

#[test]
fn daemon_reconciles_terminal_handoffs_into_the_inbox() {
    let workspace = tempfile::tempdir().unwrap();
    let inbox = workspace.path().join("inbox.jsonl");
    let handoffs = workspace.path().join("handoffs.jsonl");
    let schedules = workspace.path().join("schedules.json");
    let journals = workspace.path().join("journals");

    let created = run(&[
        "handoff",
        "create",
        "--store",
        handoffs.to_str().unwrap(),
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
    ]);
    assert!(
        created.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&created.stderr)
    );
    let projected = run(&[
        "inbox",
        "sync",
        "--store",
        inbox.to_str().unwrap(),
        "--handoff-store",
        handoffs.to_str().unwrap(),
        "--recipient",
        "bob",
        "--apply",
    ]);
    assert!(
        projected.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&projected.stderr)
    );
    let accepted = run(&[
        "handoff",
        "accept",
        "--store",
        handoffs.to_str().unwrap(),
        "--id",
        "h1",
        "--actor",
        "bob",
    ]);
    assert!(
        accepted.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&accepted.stderr)
    );

    let daemon = run(&[
        "daemon",
        "--store",
        schedules.to_str().unwrap(),
        "--journal-dir",
        journals.to_str().unwrap(),
        "--once",
        "--reconcile-inbox-store",
        inbox.to_str().unwrap(),
        "--reconcile-handoff-store",
        handoffs.to_str().unwrap(),
        "--reconcile-apply",
    ]);
    assert!(
        daemon.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&daemon.stderr)
    );

    let listed = run(&[
        "inbox",
        "list",
        "--store",
        inbox.to_str().unwrap(),
        "--json",
    ]);
    assert!(listed.status.success());
    let items: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(items.as_array().unwrap().len(), 0);

    // A second tick has no candidates and stays successful.
    let repeat = run(&[
        "daemon",
        "--store",
        schedules.to_str().unwrap(),
        "--journal-dir",
        journals.to_str().unwrap(),
        "--once",
        "--reconcile-inbox-store",
        inbox.to_str().unwrap(),
        "--reconcile-handoff-store",
        handoffs.to_str().unwrap(),
        "--reconcile-apply",
    ]);
    assert!(
        repeat.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&repeat.stderr)
    );
}

#[test]
fn daemon_rejects_incomplete_reconcile_configuration() {
    let workspace = tempfile::tempdir().unwrap();
    let missing_apply = run(&[
        "daemon",
        "--store",
        workspace.path().join("schedules.json").to_str().unwrap(),
        "--journal-dir",
        workspace.path().join("journals").to_str().unwrap(),
        "--once",
        "--reconcile-inbox-store",
        workspace.path().join("inbox.jsonl").to_str().unwrap(),
        "--reconcile-handoff-store",
        workspace.path().join("handoffs.jsonl").to_str().unwrap(),
    ]);
    assert!(!missing_apply.status.success());
    assert!(
        String::from_utf8_lossy(&missing_apply.stderr).contains("--reconcile-apply"),
        "stderr: {}",
        String::from_utf8_lossy(&missing_apply.stderr)
    );

    let missing_handoffs = run(&[
        "daemon",
        "--store",
        workspace.path().join("schedules.json").to_str().unwrap(),
        "--journal-dir",
        workspace.path().join("journals").to_str().unwrap(),
        "--once",
        "--reconcile-apply",
    ]);
    assert!(!missing_handoffs.status.success());
    assert!(
        String::from_utf8_lossy(&missing_handoffs.stderr).contains("--reconcile-inbox-store"),
        "stderr: {}",
        String::from_utf8_lossy(&missing_handoffs.stderr)
    );
}
