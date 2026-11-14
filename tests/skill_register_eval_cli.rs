//! End-to-end coverage for `skill register`, `skill eval` and
//! `skill activate-if-passed`.

use std::process::Command;

fn skill_markdown(version: &str, body: &str) -> String {
    format!(
        "---\nname: release\ndescription: Release procedure\nversion: {version}\nscope: repo:test\n---\n{body}\n"
    )
}

fn write_suite(path: &std::path::Path) {
    std::fs::write(
        path,
        serde_json::to_vec(&serde_json::json!({
            "suite_id": "release-suite",
            "min_score_bps": 10_000,
            "cases": [{
                "case_id": "verify",
                "required_markers": ["verify"],
                "forbidden_markers": ["rm -rf"]
            }]
        }))
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn register_eval_and_activate_if_passed() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("skills.jsonl");
    let skill = dir.path().join("SKILL.md");
    let suite = dir.path().join("suite.json");
    std::fs::write(&skill, skill_markdown("1.0.0", "1. verify\n2. build")).unwrap();
    write_suite(&suite);

    let registered = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(["skill", "register", "--store"])
        .arg(&store)
        .arg("--path")
        .arg(&skill)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        registered.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&registered.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&registered.stdout).unwrap();
    assert_eq!(report["ok"], true);
    assert_eq!(report["name"], "release");
    assert_eq!(report["version"], "1.0.0");

    let duplicate = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(["skill", "register", "--store"])
        .arg(&store)
        .arg("--path")
        .arg(&skill)
        .output()
        .unwrap();
    assert!(!duplicate.status.success());

    let evaluated = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(["skill", "eval", "--store"])
        .arg(&store)
        .arg("--name")
        .arg("release")
        .arg("--version")
        .arg("1.0.0")
        .arg("--suite")
        .arg(&suite)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        evaluated.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&evaluated.stderr)
    );
    let evaluation: serde_json::Value = serde_json::from_slice(&evaluated.stdout).unwrap();
    assert_eq!(evaluation["passed"], true);
    assert_eq!(evaluation["score_bps"], 10_000);
    assert_eq!(evaluation["cases"][0]["case_id"], "verify");

    let activated = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(["skill", "activate-if-passed", "--store"])
        .arg(&store)
        .arg("--name")
        .arg("release")
        .arg("--version")
        .arg("1.0.0")
        .arg("--suite")
        .arg(&suite)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        activated.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&activated.stderr)
    );
    let activation: serde_json::Value = serde_json::from_slice(&activated.stdout).unwrap();
    assert_eq!(activation["activated"], true);
    assert_eq!(activation["score_bps"], 10_000);

    let journal = std::fs::read_to_string(&store).unwrap();
    assert!(
        journal.lines().any(|line| {
            line.contains("\"activated\"") && line.contains("\"version\":\"1.0.0\"")
        })
    );
}

#[test]
fn failing_eval_reports_and_does_not_activate() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("skills.jsonl");
    let skill = dir.path().join("SKILL.md");
    let suite = dir.path().join("suite.json");
    std::fs::write(&skill, skill_markdown("1.1.0", "1. rm -rf /")).unwrap();
    write_suite(&suite);

    let registered = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(["skill", "register", "--store"])
        .arg(&store)
        .arg("--path")
        .arg(&skill)
        .output()
        .unwrap();
    assert!(registered.status.success());

    let evaluated = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(["skill", "eval", "--store"])
        .arg(&store)
        .arg("--name")
        .arg("release")
        .arg("--version")
        .arg("1.1.0")
        .arg("--suite")
        .arg(&suite)
        .arg("--json")
        .output()
        .unwrap();
    assert!(evaluated.status.success());
    let evaluation: serde_json::Value = serde_json::from_slice(&evaluated.stdout).unwrap();
    assert_eq!(evaluation["passed"], false);
    assert_eq!(evaluation["cases"][0]["forbidden_hits"][0], "rm -rf");

    let activated = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(["skill", "activate-if-passed", "--store"])
        .arg(&store)
        .arg("--name")
        .arg("release")
        .arg("--version")
        .arg("1.1.0")
        .arg("--suite")
        .arg(&suite)
        .output()
        .unwrap();
    assert!(!activated.status.success());
    let journal = std::fs::read_to_string(&store).unwrap();
    assert!(
        !journal.lines().any(|line| line.contains("\"activated\"")),
        "a failing evaluation must not append an activation event"
    );
}

#[test]
fn invalid_suite_fails_closed() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("skills.jsonl");
    let skill = dir.path().join("SKILL.md");
    let suite = dir.path().join("suite.json");
    std::fs::write(&skill, skill_markdown("1.0.0", "1. verify")).unwrap();
    std::fs::write(
        &suite,
        serde_json::to_vec(&serde_json::json!({
            "suite_id": "bad suite",
            "min_score_bps": 20_000,
            "cases": []
        }))
        .unwrap(),
    )
    .unwrap();
    let registered = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(["skill", "register", "--store"])
        .arg(&store)
        .arg("--path")
        .arg(&skill)
        .output()
        .unwrap();
    assert!(registered.status.success());

    let evaluated = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(["skill", "eval", "--store"])
        .arg(&store)
        .arg("--name")
        .arg("release")
        .arg("--version")
        .arg("1.0.0")
        .arg("--suite")
        .arg(&suite)
        .output()
        .unwrap();
    assert!(!evaluated.status.success());
    assert!(
        String::from_utf8_lossy(&evaluated.stderr).contains("skill eval"),
        "stderr: {}",
        String::from_utf8_lossy(&evaluated.stderr)
    );
}
