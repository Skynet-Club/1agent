//! CLI-level check that a Master state change projects a typed host event.

use serde_json::json;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn master_init_writes_a_typed_host_event() {
    let root = tempdir().unwrap();
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let plan = json!({
        "schema_version": 1,
        "plan_id": "plan-host",
        "goal_id": "goal-host",
        "version": 1,
        "summary": "host event test",
        "work_items": [{
            "id": "T1",
            "title": "Host event",
            "objective": "Emit a goal.proposed event",
            "dependencies": [],
            "required_capabilities": ["rust"],
            "required_tools": ["shell"],
            "permissions": {
                "read_roots": [workspace.display().to_string()],
                "allow_network": false,
                "allow_write": false
            },
            "acceptance_criteria": ["event exists"],
            "expected_artifacts": ["out.txt"],
            "workspace": workspace.display().to_string(),
            "budget": {"max_steps": 6, "timeout_ms": 120000, "max_tool_calls": 8},
            "deadline_unix_ms": null,
            "risk": "low",
            "status": "pending"
        }],
        "max_parallel": 1,
        "approved": false
    });
    let agents = json!([{
        "agent_id": "agent-1",
        "runtime": "pi",
        "capabilities": ["rust"],
        "tools": ["shell"],
        "permissions": {
            "read_roots": [workspace.display().to_string()],
            "allow_network": false,
            "allow_write": false
        },
        "availability": "idle",
        "current_load": 0,
        "observed_success_permille": 900,
        "cost_micros": 10,
        "max_risk": "low"
    }]);
    let plan_path = root.path().join("plan.json");
    let agents_path = root.path().join("agents.json");
    let state_root = root.path().join("state");
    let event_dir = root.path().join("events");
    std::fs::write(&plan_path, serde_json::to_vec_pretty(&plan).unwrap()).unwrap();
    std::fs::write(&agents_path, serde_json::to_vec_pretty(&agents).unwrap()).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args([
            "master",
            "init",
            "--plan",
            plan_path.to_str().unwrap(),
            "--agents",
            agents_path.to_str().unwrap(),
            "--state-root",
            state_root.to_str().unwrap(),
            "--trace-id",
            "trace-host",
        ])
        .env("SKYTERM_HOST_EVENT_DIR", &event_dir)
        .env("SKYTERM_MASTER_CONVERSATION_ID", "master-test")
        .env("SKYTERM_SESSION_ID", "term-1790000000000-1234-1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let raw = std::fs::read_to_string(event_dir.join("host-events.jsonl")).unwrap();
    let event: serde_json::Value =
        serde_json::from_str(raw.lines().next().expect("one event")).unwrap();
    assert_eq!(event["schema"], "skynet.term.host-event/v1");
    assert_eq!(event["event"], "goal.proposed");
    assert_eq!(event["conversation_id"], "master-test");
    assert_eq!(event["goal_id"], "goal-host");
    assert_eq!(
        event["payload"]["journal_kind"],
        serde_json::Value::String("initialized".into())
    );
}
