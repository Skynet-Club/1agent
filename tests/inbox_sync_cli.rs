use std::path::Path;
use std::process::Command;

use first_principles_agent::{AlertLog, AlertRecord, GoalStore};

fn write_failed_alert(path: &Path) {
    let log = AlertLog::open(path).unwrap();
    log.append(&AlertRecord {
        alert_id: "alert-1".into(),
        at_ms: 10,
        kind: "schedule_run_failed".into(),
        provider_id: None,
        model: None,
        observed: 1,
        threshold: 1,
        message: "scheduled run failed".into(),
    })
    .unwrap();
}

#[test]
fn inbox_sync_is_dry_run_by_default_then_dedupes() {
    let dir = tempfile::tempdir().unwrap();
    let inbox = dir.path().join("inbox.jsonl");
    let alerts = dir.path().join("alerts.jsonl");
    write_failed_alert(&alerts);

    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let dry = Command::new(bin)
        .args([
            "inbox",
            "sync",
            "--store",
            &inbox.display().to_string(),
            "--alert-log",
            &alerts.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        dry.status.success(),
        "dry run stderr: {}",
        String::from_utf8_lossy(&dry.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&dry.stdout).unwrap();
    assert_eq!(report["scanned"], 1);
    assert_eq!(report["added"], 1);
    assert_eq!(report["skipped"], 0);
    assert_eq!(report["dry_run"], true);

    let empty = Command::new(bin)
        .args([
            "inbox",
            "list",
            "--store",
            &inbox.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    let empty: serde_json::Value = serde_json::from_slice(&empty.stdout).unwrap();
    assert!(empty.as_array().unwrap().is_empty());

    let apply = Command::new(bin)
        .args([
            "inbox",
            "sync",
            "--store",
            &inbox.display().to_string(),
            "--alert-log",
            &alerts.display().to_string(),
            "--apply",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(apply.status.success());
    let applied: serde_json::Value = serde_json::from_slice(&apply.stdout).unwrap();
    assert_eq!(applied["added"], 1);
    assert_eq!(applied["dry_run"], false);

    let listed = Command::new(bin)
        .args([
            "inbox",
            "list",
            "--store",
            &inbox.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    let listed: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(listed[0]["kind"], "review");

    let again = Command::new(bin)
        .args([
            "inbox",
            "sync",
            "--store",
            &inbox.display().to_string(),
            "--alert-log",
            &alerts.display().to_string(),
            "--apply",
            "--json",
        ])
        .output()
        .unwrap();
    let again: serde_json::Value = serde_json::from_slice(&again.stdout).unwrap();
    assert_eq!(again["added"], 0);
    assert_eq!(again["skipped"], 1);
}

#[test]
fn inbox_sync_projects_an_overdue_goal_commitment() {
    let dir = tempfile::tempdir().unwrap();
    let inbox = dir.path().join("inbox.jsonl");
    let goals = dir.path().join("goals.jsonl");
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

    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let sync = Command::new(bin)
        .args([
            "inbox",
            "sync",
            "--store",
            &inbox.display().to_string(),
            "--goal-journal",
            &goals.display().to_string(),
            "--apply",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        sync.status.success(),
        "sync stderr: {}",
        String::from_utf8_lossy(&sync.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&sync.stdout).unwrap();
    assert_eq!(report["scanned"], 1);
    assert_eq!(report["added"], 1);

    let list = Command::new(bin)
        .args([
            "inbox",
            "list",
            "--store",
            &inbox.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    let items: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap();
    assert_eq!(items.as_array().unwrap().len(), 1);
    assert_eq!(items[0]["kind"], "review");
    assert_eq!(items[0]["title"], "send the report");
}
