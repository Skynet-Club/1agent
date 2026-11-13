use std::path::Path;
use std::process::{Command, Output};

fn script() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/check-attribution.sh")
}

fn git(repo: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap()
}

fn init_repo(repo: &Path) {
    assert!(git(repo, &["init", "-q"]).status.success());
    assert!(
        git(repo, &["config", "user.name", "Attribution Test"])
            .status
            .success()
    );
    assert!(
        git(repo, &["config", "user.email", "attribution@example.test"])
            .status
            .success()
    );
    std::fs::create_dir_all(repo.join("scripts")).unwrap();
    std::fs::copy(script(), repo.join("scripts/check-attribution.sh")).unwrap();
}

fn commit(repo: &Path, message: &str) {
    assert!(git(repo, &["add", "."]).status.success());
    assert!(
        Command::new("git")
            .args(["commit", "-q", "-m", message])
            .current_dir(repo)
            .status()
            .unwrap()
            .success()
    );
}

fn check(repo: &Path) -> Output {
    Command::new("bash")
        .arg("scripts/check-attribution.sh")
        .current_dir(repo)
        .output()
        .unwrap()
}

fn forbidden_tool_name() -> String {
    ["Claude", " Code"].concat()
}

fn forbidden_byline() -> String {
    let header = "Co-Authored-By:";
    let email = ["noreply@", "anthropic.com"].concat();
    format!("subject\n\n{header} {} <{email}>", forbidden_tool_name())
}

#[test]
fn attribution_check_passes_a_clean_repository() {
    let repo = tempfile::tempdir().unwrap();
    init_repo(repo.path());
    std::fs::write(repo.path().join("file.txt"), "clean\n").unwrap();
    commit(repo.path(), "clean commit");
    let output = check(repo.path());
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("attribution: clean"));
}

#[test]
fn attribution_check_rejects_a_commit_byline() {
    let repo = tempfile::tempdir().unwrap();
    init_repo(repo.path());
    std::fs::write(repo.path().join("file.txt"), "clean\n").unwrap();
    commit(repo.path(), &forbidden_byline());
    let output = check(repo.path());
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("forbidden Claude marker"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn attribution_check_rejects_a_tree_marker() {
    let repo = tempfile::tempdir().unwrap();
    init_repo(repo.path());
    std::fs::write(
        repo.path().join("note.txt"),
        format!("{}\n", forbidden_tool_name()),
    )
    .unwrap();
    commit(repo.path(), "clean commit");
    let output = check(repo.path());
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("forbidden Claude marker"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
