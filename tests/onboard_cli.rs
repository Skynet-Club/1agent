use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn onboard_writes_profile_and_user_model_reads_it() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("memory-journal.jsonl");
    let payloads = dir.path().join("payloads");
    let key = dir.path().join("memory.key");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");

    let onboard = Command::new(bin)
        .args(["onboard", "--memory-journal"])
        .arg(&journal)
        .arg("--memory-payloads")
        .arg(&payloads)
        .arg("--memory-key")
        .arg(&key)
        .args([
            "--bot-id",
            "bot-1",
            "--voice",
            "advisor",
            "--autonomy",
            "workspace=act",
            "--autonomy",
            "email=observe",
            "--boundary",
            "never touch production",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        onboard.status.success(),
        "onboard stderr: {}",
        String::from_utf8_lossy(&onboard.stderr)
    );
    let written: serde_json::Value = serde_json::from_slice(&onboard.stdout).unwrap();
    assert_eq!(written["written"], 4);
    let model = &written["user_model"];
    assert_eq!(model["voice"]["mode"], "advisor");
    assert_eq!(model["autonomy"]["workspace"]["level"], "act");
    assert_eq!(model["autonomy"]["email"]["level"], "observe");
    assert_eq!(model["boundaries"][0]["rule"], "never touch production");

    let read = Command::new(bin)
        .args(["user-model", "--journal"])
        .arg(&journal)
        .arg("--payloads")
        .arg(&payloads)
        .arg("--key")
        .arg(&key)
        .args(["--bot-id", "bot-1", "--json"])
        .output()
        .unwrap();
    assert!(
        read.status.success(),
        "user-model stderr: {}",
        String::from_utf8_lossy(&read.stderr)
    );
    let model: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(model["voice"]["mode"], "advisor");
    assert_eq!(model["autonomy"]["workspace"]["level"], "act");
    assert_eq!(model["boundaries"][0]["rule"], "never touch production");
}

#[test]
fn onboard_rejects_invalid_autonomy_and_voice() {
    let dir = tempfile::tempdir().unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let bad_autonomy = Command::new(bin)
        .args(["onboard", "--autonomy", "workspace=maybe"])
        .arg("--memory-journal")
        .arg(dir.path().join("journal.jsonl"))
        .arg("--memory-payloads")
        .arg(dir.path().join("payloads"))
        .arg("--memory-key")
        .arg(dir.path().join("key"))
        .output()
        .unwrap();
    assert!(!bad_autonomy.status.success());

    let bad_voice = Command::new(bin)
        .args(["onboard", "--voice", "robot"])
        .arg("--memory-journal")
        .arg(dir.path().join("journal2.jsonl"))
        .arg("--memory-payloads")
        .arg(dir.path().join("payloads2"))
        .arg("--memory-key")
        .arg(dir.path().join("key2"))
        .output()
        .unwrap();
    assert!(!bad_voice.status.success());
}

#[test]
fn onboard_can_seed_a_goal_and_commitment_for_inbox_sync() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("memory-journal.jsonl");
    let payloads = dir.path().join("payloads");
    let key = dir.path().join("memory.key");
    let goals = dir.path().join("goals.jsonl");
    let inbox = dir.path().join("inbox.jsonl");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");

    let onboard = Command::new(bin)
        .args(["onboard", "--memory-journal"])
        .arg(&journal)
        .arg("--memory-payloads")
        .arg(&payloads)
        .arg("--memory-key")
        .arg(&key)
        .args(["--bot-id", "bot-1", "--goal", "ship the report"])
        .arg("--goal-journal")
        .arg(&goals)
        .args(["--commitment", "send the report", "--commitment-due-ms"])
        .arg(now.saturating_sub(1_000).to_string())
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        onboard.status.success(),
        "onboard stderr: {}",
        String::from_utf8_lossy(&onboard.stderr)
    );
    let seeded: serde_json::Value = serde_json::from_slice(&onboard.stdout).unwrap();
    assert!(seeded["goal_id"].as_str().is_some());
    assert!(seeded["commitment_id"].as_str().is_some());

    let sync = Command::new(bin)
        .args(["inbox", "sync", "--store"])
        .arg(&inbox)
        .arg("--goal-journal")
        .arg(&goals)
        .args(["--apply", "--json"])
        .output()
        .unwrap();
    assert!(
        sync.status.success(),
        "sync stderr: {}",
        String::from_utf8_lossy(&sync.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&sync.stdout).unwrap();
    assert_eq!(report["added"], 1);

    let list = Command::new(bin)
        .args(["inbox", "list", "--store"])
        .arg(&inbox)
        .arg("--json")
        .output()
        .unwrap();
    let items: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap();
    assert_eq!(items.as_array().unwrap().len(), 1);
    assert_eq!(items[0]["kind"], "review");
    assert_eq!(items[0]["title"], "send the report");
}

#[test]
fn onboard_interactive_reads_stdin_answers() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("memory-journal.jsonl");
    let payloads = dir.path().join("payloads");
    let key = dir.path().join("memory.key");
    let goals = dir.path().join("goals.jsonl");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let mut child = Command::new(bin)
        .args(["onboard", "--interactive", "--memory-journal"])
        .arg(&journal)
        .arg("--memory-payloads")
        .arg(&payloads)
        .arg("--memory-key")
        .arg(&key)
        .args(["--bot-id", "bot-1", "--goal-journal"])
        .arg(&goals)
        .arg("--json")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    stdin
        .write_all(b"advisor\nship the report\nsend the report\nnever touch production\n")
        .unwrap();
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["user_model"]["voice"]["mode"], "advisor");
    assert!(report["goal_id"].as_str().is_some());
    assert!(report["commitment_id"].as_str().is_some());
    assert_eq!(
        report["user_model"]["boundaries"][0]["rule"],
        "never touch production"
    );
}

#[test]
fn onboard_writes_and_reads_quiet_hours() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("memory-journal.jsonl");
    let payloads = dir.path().join("payloads");
    let key = dir.path().join("memory.key");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");

    let onboard = Command::new(bin)
        .args(["onboard", "--memory-journal"])
        .arg(&journal)
        .arg("--memory-payloads")
        .arg(&payloads)
        .arg("--memory-key")
        .arg(&key)
        .args([
            "--bot-id",
            "bot-1",
            "--quiet-hours",
            "22:00-07:00",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        onboard.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&onboard.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&onboard.stdout).unwrap();
    assert_eq!(report["written"], 1);
    assert_eq!(
        report["user_model"]["quiet_hours"]["hours"]["start_minute"],
        22 * 60
    );
    assert_eq!(
        report["user_model"]["quiet_hours"]["hours"]["end_minute"],
        7 * 60
    );

    let read = Command::new(bin)
        .args(["user-model", "--journal"])
        .arg(&journal)
        .arg("--payloads")
        .arg(&payloads)
        .arg("--key")
        .arg(&key)
        .args(["--bot-id", "bot-1", "--json"])
        .output()
        .unwrap();
    assert!(read.status.success());
    let model: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(model["quiet_hours"]["hours"]["start_minute"], 22 * 60);

    let invalid = Command::new(bin)
        .args(["onboard", "--memory-journal"])
        .arg(dir.path().join("journal2.jsonl"))
        .arg("--memory-payloads")
        .arg(dir.path().join("payloads2"))
        .arg("--memory-key")
        .arg(dir.path().join("key2"))
        .args(["--quiet-hours", "bedtime"])
        .output()
        .unwrap();
    assert!(!invalid.status.success());
}

#[test]
fn onboard_imports_plain_text_context() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("memory-journal.jsonl");
    let payloads = dir.path().join("payloads");
    let key = dir.path().join("memory.key");
    let import = dir.path().join("context.txt");
    std::fs::write(
        &import,
        "# profile\nvoice_mode=advisor\n\nsecond line is ignored for voice\n",
    )
    .unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");

    let output = Command::new(bin)
        .args(["onboard", "--memory-journal"])
        .arg(&journal)
        .arg("--memory-payloads")
        .arg(&payloads)
        .arg("--memory-key")
        .arg(&key)
        .args(["--bot-id", "bot-1", "--import-file"])
        .arg(&import)
        .args(["--import-kind", "preference", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["imported"], 2);
    assert_eq!(report["written"], 2);
    assert_eq!(report["user_model"]["voice"]["mode"], "advisor");

    let facts = dir.path().join("facts.txt");
    std::fs::write(&facts, "fact one\n# skip\nfact two\n").unwrap();
    let second = Command::new(bin)
        .args(["onboard", "--memory-journal"])
        .arg(&journal)
        .arg("--memory-payloads")
        .arg(&payloads)
        .arg("--memory-key")
        .arg(&key)
        .args(["--bot-id", "bot-2", "--import-file"])
        .arg(&facts)
        .args(["--import-kind", "fact", "--json"])
        .output()
        .unwrap();
    assert!(second.status.success());
    let report: serde_json::Value = serde_json::from_slice(&second.stdout).unwrap();
    assert_eq!(report["imported"], 2);
    assert_eq!(report["written"], 2);
}

#[test]
fn onboard_import_rejects_bad_files_and_kinds() {
    let dir = tempfile::tempdir().unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let comments = dir.path().join("comments.txt");
    std::fs::write(&comments, "# only comments\n\n").unwrap();

    for (file, kind) in [(dir.path().join("missing.txt"), "fact"), (comments, "fact")] {
        let output = Command::new(bin)
            .args(["onboard", "--memory-journal"])
            .arg(dir.path().join("journal.jsonl"))
            .arg("--memory-payloads")
            .arg(dir.path().join("payloads"))
            .arg("--memory-key")
            .arg(dir.path().join("key"))
            .args(["--import-file"])
            .arg(&file)
            .args(["--import-kind", kind])
            .output()
            .unwrap();
        assert!(!output.status.success());
    }

    let facts = dir.path().join("facts.txt");
    std::fs::write(&facts, "one fact\n").unwrap();
    let bad_kind = Command::new(bin)
        .args(["onboard", "--memory-journal"])
        .arg(dir.path().join("journal2.jsonl"))
        .arg("--memory-payloads")
        .arg(dir.path().join("payloads2"))
        .arg("--memory-key")
        .arg(dir.path().join("key2"))
        .args(["--import-file"])
        .arg(&facts)
        .args(["--import-kind", "gossip"])
        .output()
        .unwrap();
    assert!(!bad_kind.status.success());
}

#[test]
fn onboard_commitment_without_goal_fails_closed() {
    let dir = tempfile::tempdir().unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args(["onboard", "--commitment", "send the report"])
        .arg("--memory-journal")
        .arg(dir.path().join("journal.jsonl"))
        .arg("--memory-payloads")
        .arg(dir.path().join("payloads"))
        .arg("--memory-key")
        .arg(dir.path().join("key"))
        .output()
        .unwrap();
    assert!(!output.status.success());
}
