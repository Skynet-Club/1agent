use serde_json::{Value, json};
use std::path::Path;
use std::process::Command;

fn production_plan(workspace: &Path, approved: bool) -> Value {
    let workspace = workspace.display().to_string();
    json!({
        "schema_version": 1,
        "goal": {
            "goal_id": "goal-1",
            "master_conversation_id": "master-1",
            "objective": "交付两个可验证结果",
            "non_goals": ["不做无关扩张"],
            "constraints": ["失败关闭"],
            "key_results": [
                {
                    "id": "kr-1",
                    "statement": "实现并通过自动测试",
                    "baseline": "0 tests",
                    "target": "all tests pass",
                    "measurement_method": "cargo test",
                    "work_item_ids": ["a", "b"]
                },
                {
                    "id": "kr-2",
                    "statement": "生成可校验产物",
                    "baseline": "missing",
                    "target": "artifact verified",
                    "measurement_method": "sha256 verification",
                    "work_item_ids": ["c", "d"]
                }
            ]
        },
        "plan": {
            "schema_version": 1,
            "plan_id": "plan-1",
            "goal_id": "goal-1",
            "version": 1,
            "summary": "四任务生产计划",
            "work_items": [
                item("a", &[], &workspace),
                item("b", &["a"], &workspace),
                item("c", &[], &workspace),
                item("d", &["c"], &workspace)
            ],
            "max_parallel": 2,
            "approved": approved
        }
    })
}

fn item(id: &str, dependencies: &[&str], workspace: &str) -> Value {
    json!({
        "id": id,
        "title": format!("任务 {id}"),
        "objective": format!("完成 {id}"),
        "dependencies": dependencies,
        "required_capabilities": ["rust"],
        "required_tools": ["shell"],
        "permissions": {"read_roots": [workspace], "allow_network": false, "allow_write": false},
        "acceptance_criteria": [format!("{id} passes")],
        "expected_artifacts": [format!("{id}.log")],
        "workspace": workspace,
        "budget": {"max_steps": 6, "timeout_ms": 120000, "max_tool_calls": 8},
        "deadline_unix_ms": null,
        "risk": "low",
        "status": "pending"
    })
}

fn run_task_list(plan: &Path, state_root: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args([
            "master",
            "production-task-list",
            "--plan",
            plan.to_str().unwrap(),
            "--state-root",
            state_root.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap()
}

#[test]
fn production_task_list_cli_writes_and_checks_immutable_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let plan_path = root.path().join("production-plan.json");
    std::fs::write(
        &plan_path,
        serde_json::to_vec_pretty(&production_plan(&workspace, false)).unwrap(),
    )
    .unwrap();
    let state_root = root.path().join("state");

    let first = run_task_list(&plan_path, &state_root);
    assert!(
        first.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let snapshot: Value = serde_json::from_slice(&first.stdout).unwrap();
    let task_list = state_root.join("goal-1/v1/任务清单.md");
    assert_eq!(snapshot["path"], task_list.display().to_string());
    assert_eq!(snapshot["plan_sha256"].as_str().unwrap().len(), 64);
    assert_eq!(snapshot["markdown_sha256"].as_str().unwrap().len(), 64);
    let markdown = std::fs::read_to_string(&task_list).unwrap();
    assert!(markdown.contains("### kr-1"));
    assert!(markdown.contains("### a — 任务 a"));

    let second = run_task_list(&plan_path, &state_root);
    assert!(second.status.success());
    assert_eq!(second.stdout, first.stdout);

    std::fs::write(&task_list, "tampered").unwrap();
    let conflict = run_task_list(&plan_path, &state_root);
    assert!(!conflict.status.success());
    assert!(
        String::from_utf8_lossy(&conflict.stderr).contains("immutable task list already exists")
    );
}

#[test]
fn production_task_list_cli_rejects_self_approved_plan_before_writing() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let plan_path = root.path().join("self-approved.json");
    std::fs::write(
        &plan_path,
        serde_json::to_vec_pretty(&production_plan(&workspace, true)).unwrap(),
    )
    .unwrap();
    let state_root = root.path().join("state");

    let output = run_task_list(&plan_path, &state_root);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("must remain unapproved"));
    assert!(!state_root.join("goal-1").exists());
}
