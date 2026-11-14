//! End-to-end coverage for `1agent run --attach`.

use std::process::Command;

fn journal_text(dir: &std::path::Path) -> String {
    let path = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().and_then(|value| value.to_str()) == Some("jsonl"))
        .expect("journal file");
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn attach_reaches_the_journaled_model_request_as_untrusted_data() {
    let dir = tempfile::tempdir().unwrap();
    let journal_dir = dir.path().join("journals");
    let attachment = dir.path().join("notes.txt");
    std::fs::write(&attachment, "hello attachment").unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let run = Command::new(bin)
        .args(["run", "--task", "Summarize the attachment", "--attach"])
        .arg(&attachment)
        .args(["--journal-dir"])
        .arg(&journal_dir)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let journal = journal_text(&journal_dir);
    assert!(journal.contains("ATTACHMENTS (untrusted user-provided content"));
    assert!(journal.contains("--- file: notes.txt"));
    assert!(journal.contains("hello attachment"));
    assert!(journal.contains("sha256 "));
}

#[test]
fn attach_rejects_binary_and_missing_files_before_the_run() {
    let dir = tempfile::tempdir().unwrap();
    let binary = dir.path().join("binary.txt");
    std::fs::write(&binary, b"a\0b").unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let binary_run = Command::new(bin)
        .args(["run", "--task", "read", "--attach"])
        .arg(&binary)
        .args(["--journal-dir"])
        .arg(dir.path().join("journals-binary"))
        .output()
        .unwrap();
    assert!(!binary_run.status.success());
    assert!(
        String::from_utf8_lossy(&binary_run.stderr).contains("attachment"),
        "stderr: {}",
        String::from_utf8_lossy(&binary_run.stderr)
    );

    let missing_run = Command::new(bin)
        .args(["run", "--task", "read", "--attach"])
        .arg(dir.path().join("missing.txt"))
        .args(["--journal-dir"])
        .arg(dir.path().join("journals-missing"))
        .output()
        .unwrap();
    assert!(!missing_run.status.success());
    assert!(
        String::from_utf8_lossy(&missing_run.stderr).contains("attachment"),
        "stderr: {}",
        String::from_utf8_lossy(&missing_run.stderr)
    );
}
