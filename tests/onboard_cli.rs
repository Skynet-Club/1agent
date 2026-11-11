use std::process::Command;

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
