use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_first-principles-agent")
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(bin()).args(args).output().unwrap()
}

fn add_item(store: &std::path::Path, title: &str) -> serde_json::Value {
    let output = run(&[
        "inbox",
        "add",
        "--store",
        store.to_str().unwrap(),
        "--kind",
        "review",
        "--title",
        title,
        "--json",
    ]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn inbox_escalate_reports_only_open_items_at_their_age_level() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("inbox.jsonl");
    let open = add_item(&store, "needs a decision");
    let resolved = add_item(&store, "already handled");
    let resolve = run(&[
        "inbox",
        "resolve",
        "--store",
        store.to_str().unwrap(),
        "--item-id",
        resolved["item_id"].as_str().unwrap(),
        "--status",
        "dismissed",
    ]);
    assert!(
        resolve.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&resolve.stderr)
    );

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        + 60_000;
    let output = run(&[
        "inbox",
        "escalate",
        "--store",
        store.to_str().unwrap(),
        "--step",
        "1000=inbox",
        "--step",
        "5000=notify",
        "--step",
        "50000=urgent",
        "--now-ms",
        &now.to_string(),
        "--json",
    ]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rows: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(rows.len(), 1, "resolved items must not escalate: {rows:?}");
    assert_eq!(rows[0]["item_id"], open["item_id"]);
    assert_eq!(rows[0]["level"], "urgent");
    assert!(rows[0]["elapsed_ms"].as_u64().unwrap() >= 60_000);
    assert!(rows[0]["reason"].as_str().unwrap().contains("50000 ms"));
}

#[test]
fn inbox_escalate_below_the_first_threshold_reports_none() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("inbox.jsonl");
    let item = add_item(&store, "fresh item");
    let output = run(&[
        "inbox",
        "escalate",
        "--store",
        store.to_str().unwrap(),
        "--step",
        "999999999=urgent",
        "--json",
    ]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rows: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["item_id"], item["item_id"]);
    assert!(rows[0]["level"].is_null());
    assert_eq!(rows[0]["reason"], "no escalation threshold reached");
}

#[test]
fn inbox_escalate_rejects_invalid_ladders() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("inbox.jsonl");
    let missing_equals = run(&[
        "inbox",
        "escalate",
        "--store",
        store.to_str().unwrap(),
        "--step",
        "1000",
    ]);
    assert!(!missing_equals.status.success());
    assert!(
        String::from_utf8_lossy(&missing_equals.stderr).contains("AFTER_MS=ACTION"),
        "stderr: {}",
        String::from_utf8_lossy(&missing_equals.stderr)
    );

    let bad_action = run(&[
        "inbox",
        "escalate",
        "--store",
        store.to_str().unwrap(),
        "--step",
        "1000=pager",
    ]);
    assert!(!bad_action.status.success());
    assert!(
        String::from_utf8_lossy(&bad_action.stderr).contains("inbox, digest, notify or urgent"),
        "stderr: {}",
        String::from_utf8_lossy(&bad_action.stderr)
    );

    let unordered = run(&[
        "inbox",
        "escalate",
        "--store",
        store.to_str().unwrap(),
        "--step",
        "5000=inbox",
        "--step",
        "1000=notify",
    ]);
    assert!(!unordered.status.success());
    assert!(
        String::from_utf8_lossy(&unordered.stderr).contains("strictly increasing"),
        "stderr: {}",
        String::from_utf8_lossy(&unordered.stderr)
    );
}
