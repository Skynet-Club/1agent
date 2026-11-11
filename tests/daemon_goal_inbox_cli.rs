use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_first-principles-agent")
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(bin()).args(args).output().unwrap()
}

#[test]
fn daemon_projects_overdue_commitments_into_the_inbox() {
    use first_principles_agent::GoalStore;

    let workspace = tempfile::tempdir().unwrap();
    let goals = workspace.path().join("goals.jsonl");
    let inbox = workspace.path().join("inbox.jsonl");
    let schedules = workspace.path().join("schedules.json");
    let journals = workspace.path().join("journals");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let mut store = GoalStore::open(&goals).unwrap();
    store
        .with_graph_mut(|graph| {
            graph.create_goal("g1", "owner", "ship the report", None, now)?;
            graph.add_commitment(
                "c1",
                "g1",
                "send the report",
                Some(now.saturating_sub(1_000)),
                now + 1,
            )?;
            Ok(())
        })
        .unwrap();
    drop(store);

    let daemon = run(&[
        "daemon",
        "--store",
        schedules.to_str().unwrap(),
        "--journal-dir",
        journals.to_str().unwrap(),
        "--once",
        "--goal-journal",
        goals.to_str().unwrap(),
        "--goal-inbox-store",
        inbox.to_str().unwrap(),
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
    assert_eq!(items.as_array().unwrap().len(), 1);
    assert_eq!(items[0]["kind"], "review");
    assert!(
        items[0]["source"]
            .as_str()
            .unwrap()
            .starts_with("attention:")
    );

    let repeat = run(&[
        "daemon",
        "--store",
        schedules.to_str().unwrap(),
        "--journal-dir",
        journals.to_str().unwrap(),
        "--once",
        "--goal-journal",
        goals.to_str().unwrap(),
        "--goal-inbox-store",
        inbox.to_str().unwrap(),
    ]);
    assert!(
        repeat.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&repeat.stderr)
    );
    let listed = run(&[
        "inbox",
        "list",
        "--store",
        inbox.to_str().unwrap(),
        "--json",
    ]);
    let items: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(items.as_array().unwrap().len(), 1);
}

#[test]
fn daemon_requires_both_goal_flags() {
    let workspace = tempfile::tempdir().unwrap();
    let missing_inbox = run(&[
        "daemon",
        "--store",
        workspace.path().join("schedules.json").to_str().unwrap(),
        "--journal-dir",
        workspace.path().join("journals").to_str().unwrap(),
        "--once",
        "--goal-journal",
        workspace.path().join("goals.jsonl").to_str().unwrap(),
    ]);
    assert!(!missing_inbox.status.success());
    assert!(
        String::from_utf8_lossy(&missing_inbox.stderr)
            .contains("--goal-journal and --goal-inbox-store"),
        "stderr: {}",
        String::from_utf8_lossy(&missing_inbox.stderr)
    );

    let missing_journal = run(&[
        "daemon",
        "--store",
        workspace.path().join("schedules.json").to_str().unwrap(),
        "--journal-dir",
        workspace.path().join("journals").to_str().unwrap(),
        "--once",
        "--goal-inbox-store",
        workspace.path().join("inbox.jsonl").to_str().unwrap(),
    ]);
    assert!(!missing_journal.status.success());
}
