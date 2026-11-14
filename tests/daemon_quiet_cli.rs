use std::process::Command;

fn quiet_window_covering_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let minute = ((now / 60_000) % 1_440) as u16;
    let end = (minute + 2) % 1_440;
    format!(
        "{:02}:{:02}-{:02}:{:02}",
        minute / 60,
        minute % 60,
        end / 60,
        end % 60
    )
}

#[test]
fn daemon_skips_due_jobs_during_quiet_hours() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("schedules.json");
    let journals = workspace.path().join("journals");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");

    let add = Command::new(bin)
        .args(["schedule", "add", "--name", "quiet-job", "--task"])
        .arg("用一句话回答 1+1")
        .args([
            "--every-seconds",
            "3600",
            "--start-now",
            "--store",
            store.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        add.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&add.stderr)
    );

    let quiet = Command::new(bin)
        .args(["daemon", "--store"])
        .arg(&store)
        .arg("--journal-dir")
        .arg(&journals)
        .args(["--once", "--quiet-hours", &quiet_window_covering_now()])
        .output()
        .unwrap();
    assert!(
        quiet.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&quiet.stderr)
    );
    let state: serde_json::Value = serde_json::from_slice(&std::fs::read(&store).unwrap()).unwrap();
    assert!(state["jobs"][0]["last_run_ms"].is_null());
    assert_eq!(std::fs::read_dir(&journals).unwrap().count(), 0);

    let normal = Command::new(bin)
        .args(["daemon", "--store"])
        .arg(&store)
        .arg("--journal-dir")
        .arg(&journals)
        .arg("--once")
        .output()
        .unwrap();
    assert!(
        normal.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&normal.stderr)
    );
    let state: serde_json::Value = serde_json::from_slice(&std::fs::read(&store).unwrap()).unwrap();
    assert!(state["jobs"][0]["last_run_ms"].is_u64());
}

#[test]
fn daemon_rejects_invalid_quiet_hours() {
    let workspace = tempfile::tempdir().unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args(["daemon", "--store"])
        .arg(workspace.path().join("schedules.json"))
        .arg("--journal-dir")
        .arg(workspace.path().join("journals"))
        .args(["--once", "--quiet-hours", "bedtime"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}
