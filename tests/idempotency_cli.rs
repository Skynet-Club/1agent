use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn store_path() -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "1agent-idempotency-{}-{unique}.jsonl",
        std::process::id()
    ))
}

#[test]
fn records_replay_across_processes_and_conflicts_fail() {
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let path = store_path();
    let store = path.to_str().unwrap();

    let recorded = Command::new(bin)
        .args([
            "idempotency",
            "record",
            "--store",
            store,
            "--key",
            "job-1",
            "--fingerprint",
            "fp-1",
            "--response",
            "{\"ticket\":42}",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        recorded.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&recorded.stderr)
    );
    let record: serde_json::Value = serde_json::from_slice(&recorded.stdout).unwrap();
    assert_eq!(record["key"], "job-1");
    assert_eq!(record["response"]["ticket"], 42);

    let replayed = Command::new(bin)
        .args([
            "idempotency",
            "replay",
            "--store",
            store,
            "--key",
            "job-1",
            "--fingerprint",
            "fp-1",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(replayed.status.success());
    let replay: serde_json::Value = serde_json::from_slice(&replayed.stdout).unwrap();
    assert_eq!(replay["hit"], true);
    assert_eq!(replay["response"]["ticket"], 42);

    let missed = Command::new(bin)
        .args([
            "idempotency",
            "replay",
            "--store",
            store,
            "--key",
            "job-2",
            "--fingerprint",
            "fp-2",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(missed.status.success());
    let miss: serde_json::Value = serde_json::from_slice(&missed.stdout).unwrap();
    assert_eq!(miss["hit"], false);

    let conflict = Command::new(bin)
        .args([
            "idempotency",
            "replay",
            "--store",
            store,
            "--key",
            "job-1",
            "--fingerprint",
            "different",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!conflict.status.success());

    let bad_json = Command::new(bin)
        .args([
            "idempotency",
            "record",
            "--store",
            store,
            "--key",
            "job-3",
            "--fingerprint",
            "fp-3",
            "--response",
            "not-json",
        ])
        .output()
        .unwrap();
    assert!(!bad_json.status.success());

    std::fs::remove_file(&path).ok();
}
