//! CLI wiring for `skill validate` and `skill list`.

use std::process::{Command, Output};

use serde_json::Value;

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn skill_validate_accepts_a_well_formed_directory() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("SKILL.md"),
        "---\nname: release\ndescription: release process\nversion: 1.0.0\n---\n# Release\n1. verify\n",
    )
    .unwrap();
    let path = dir.path().display().to_string();
    let output = run(&["skill", "validate", "--path", &path, "--json"]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["ok"], true);
    assert_eq!(report["name"], "release");
    assert_eq!(report["version"], "1.0.0");
}

#[test]
fn skill_validate_rejects_a_symlink_in_scripts() {
    #[cfg(unix)]
    {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("SKILL.md"),
            "---\nname: release\ndescription: release\nversion: 1.0.0\n---\nbody\n",
        )
        .unwrap();
        std::fs::create_dir(dir.path().join("scripts")).unwrap();
        std::os::unix::fs::symlink("/etc/passwd", dir.path().join("scripts/key")).unwrap();
        let path = dir.path().display().to_string();
        let output = run(&["skill", "validate", "--path", &path, "--json"]);
        assert!(!output.status.success());
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["ok"], false);
        assert!(
            report["errors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|error| error.as_str().unwrap_or_default().contains("symlink"))
        );
    }
}

#[test]
fn skill_activate_requires_eval_or_explicit_force() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("skills.jsonl");
    let skill = dir.path().join("SKILL.md");
    std::fs::write(
        &skill,
        "---\nname: release\ndescription: release\nversion: 1.0.0\n---\nbody\n",
    )
    .unwrap();
    let store_arg = store.display().to_string();
    let skill_arg = skill.display().to_string();
    let register = run(&[
        "skill", "register", "--store", &store_arg, "--path", &skill_arg,
    ]);
    assert!(
        register.status.success(),
        "register stderr: {}",
        String::from_utf8_lossy(&register.stderr)
    );

    let rejected = run(&[
        "skill",
        "activate",
        "--store",
        &store_arg,
        "--name",
        "release",
        "--version",
        "1.0.0",
    ]);
    assert!(!rejected.status.success());
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("requires a passing"),
        "stderr: {}",
        String::from_utf8_lossy(&rejected.stderr)
    );

    let forced = run(&[
        "skill",
        "activate",
        "--store",
        &store_arg,
        "--name",
        "release",
        "--version",
        "1.0.0",
        "--force",
    ]);
    assert!(
        forced.status.success(),
        "force stderr: {}",
        String::from_utf8_lossy(&forced.stderr)
    );
}

#[test]
fn skill_list_is_empty_for_a_fresh_store() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("skills.jsonl").display().to_string();
    let output = run(&["skill", "list", "--store", &store, "--json"]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let items: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(items.as_array().unwrap().is_empty());
}

#[test]
fn skill_compare_reports_blind_lift_and_fails_a_tie() {
    let dir = tempfile::tempdir().unwrap();
    let cases = (0..20)
        .map(|index| {
            serde_json::json!({
                "case_id": format!("case-{index}"),
                "baseline_output": "wrong",
                "candidate_output": "verify build",
                "required_markers": ["verify"],
                "forbidden_markers": []
            })
        })
        .collect::<Vec<_>>();
    let suite = serde_json::json!({
        "suite_id": "blind-cli",
        "seed": "seed-1",
        "cases": cases
    });
    let path = dir.path().join("blind.json");
    std::fs::write(&path, serde_json::to_vec(&suite).unwrap()).unwrap();
    let output = run(&[
        "skill",
        "compare",
        "--suite",
        &path.display().to_string(),
        "--json",
    ]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["ok"], true);
    assert_eq!(report["report"]["significant"], true);
    assert_eq!(report["report"]["candidate_wins"], 20);

    let tie = serde_json::json!({
        "suite_id": "blind-cli",
        "seed": "seed-1",
        "cases": [{
            "case_id": "same",
            "baseline_output": "verify",
            "candidate_output": "verify",
            "required_markers": ["verify"],
            "forbidden_markers": []
        }]
    });
    std::fs::write(&path, serde_json::to_vec(&tie).unwrap()).unwrap();
    let output = run(&[
        "skill",
        "compare",
        "--suite",
        &path.display().to_string(),
        "--json",
    ]);
    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["ok"], false);
}

#[test]
fn skill_lock_pins_the_active_set_and_rejects_drift() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("skills.jsonl");
    for (name, body) in [("alpha", "alpha body"), ("beta", "beta body")] {
        let skill = dir.path().join(format!("{name}.md"));
        std::fs::write(
            &skill,
            format!("---\nname: {name}\ndescription: {name} skill\nversion: 1.0.0\n---\n{body}\n"),
        )
        .unwrap();
        let output = run(&[
            "skill",
            "register",
            "--store",
            &store.display().to_string(),
            "--path",
            &skill.display().to_string(),
            "--json",
        ]);
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let output = run(&[
        "skill",
        "activate",
        "--store",
        &store.display().to_string(),
        "--name",
        "alpha",
        "--version",
        "1.0.0",
        "--force",
        "--json",
    ]);
    assert!(output.status.success());
    let lock = dir.path().join("skills.lock");
    let output = run(&[
        "skill",
        "lock",
        "--store",
        &store.display().to_string(),
        "--out",
        &lock.display().to_string(),
        "--json",
    ]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["skills"], 1);
    let output = run(&[
        "skill",
        "verify-lock",
        "--store",
        &store.display().to_string(),
        "--lock",
        &lock.display().to_string(),
        "--json",
    ]);
    assert!(output.status.success());

    let output = run(&[
        "skill",
        "activate",
        "--store",
        &store.display().to_string(),
        "--name",
        "beta",
        "--version",
        "1.0.0",
        "--force",
        "--json",
    ]);
    assert!(output.status.success());
    let output = run(&[
        "skill",
        "verify-lock",
        "--store",
        &store.display().to_string(),
        "--lock",
        &lock.display().to_string(),
        "--json",
    ]);
    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["ok"], false);
}

#[test]
fn skill_run_script_uses_bwrap_and_rejects_traversal() {
    if Command::new("bwrap")
        .arg("--version")
        .output()
        .map(|output| !output.status.success())
        .unwrap_or(true)
    {
        eprintln!("skipping: bwrap is not available");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("SKILL.md"),
        "---\nname: sandbox\ndescription: sandbox\nversion: 1.0.0\n---\nbody\n",
    )
    .unwrap();
    std::fs::create_dir(dir.path().join("scripts")).unwrap();
    std::fs::write(
        dir.path().join("scripts/ok.sh"),
        "#!/bin/sh\necho cli-sandbox-ok\n",
    )
    .unwrap();
    let output = run(&[
        "skill",
        "run-script",
        "--path",
        &dir.path().display().to_string(),
        "--script",
        "ok.sh",
        "--json",
    ]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["sandbox"], "bwrap");
    assert_eq!(report["network"], false);
    assert!(
        report["stdout"]
            .as_str()
            .unwrap()
            .contains("cli-sandbox-ok")
    );

    let output = run(&[
        "skill",
        "run-script",
        "--path",
        &dir.path().display().to_string(),
        "--script",
        "../SKILL.md",
        "--json",
    ]);
    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["ok"], false);
}
