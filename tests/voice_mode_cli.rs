use std::process::Command;

fn journal_text(dir: &tempfile::TempDir, mode: &str) -> String {
    let journal_dir = dir.path().join(format!("journals-{mode}"));
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let run = Command::new(bin)
        .args([
            "run",
            "--task",
            "Say hello",
            "--journal-dir",
            &journal_dir.display().to_string(),
            "--voice-mode",
            mode,
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{mode} stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let path = std::fs::read_dir(&journal_dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().and_then(|value| value.to_str()) == Some("jsonl"))
        .expect("journal file");
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn voice_mode_reaches_the_journaled_system_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let advisor = journal_text(&dir, "advisor");
    assert!(advisor.contains("VOICE: advisor"));
    assert!(advisor.contains("what would change it"));
    assert!(advisor.contains("FORM: execution"));

    let companion = journal_text(&dir, "companion");
    assert!(companion.contains("VOICE: companion"));
    assert!(companion.contains("direct and warm"));
    assert!(companion.contains("FORM: execution"));
}

#[test]
fn invalid_voice_mode_fails_closed() {
    let dir = tempfile::tempdir().unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args([
            "run",
            "--task",
            "Say hello",
            "--journal-dir",
            &dir.path().join("journals").display().to_string(),
            "--voice-mode",
            "robot",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("voice mode must be"));
}
