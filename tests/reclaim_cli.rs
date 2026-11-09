//! CLI check for expired-lease reclaim: no active lease replacement, explicit reclaim,
//! then a new lease with a higher generation.

use first_principles_agent::{
    AgentProfile, MasterJournalEventKind, MasterState, MasterStore, PlanProposal,
};
use serde_json::json;
use std::process::Command;
use tempfile::tempdir;

fn plan(workspace: &str) -> PlanProposal {
    serde_json::from_value(json!({
        "schema_version": 1,
        "plan_id": "plan-reclaim",
        "goal_id": "goal-reclaim",
        "version": 1,
        "summary": "reclaim test",
        "work_items": [{
            "id": "T1",
            "title": "Reclaim",
            "objective": "Expire and reclaim a lease",
            "dependencies": [],
            "required_capabilities": ["rust"],
            "required_tools": ["shell"],
            "permissions": {
                "read_roots": [workspace],
                "allow_network": false,
                "allow_write": false
            },
            "acceptance_criteria": ["reclaimed"],
            "expected_artifacts": ["out.txt"],
            "workspace": workspace,
            "budget": {"max_steps": 6, "timeout_ms": 120000, "max_tool_calls": 8},
            "deadline_unix_ms": null,
            "risk": "low",
            "status": "pending"
        }],
        "max_parallel": 1,
        "approved": false
    }))
    .unwrap()
}

#[test]
fn cli_reclaim_replaces_an_expired_lease_with_a_new_generation() {
    let root = tempdir().unwrap();
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let proposal = plan(&workspace.display().to_string());
    let agents: Vec<AgentProfile> = serde_json::from_value(json!([{
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
    }]))
    .unwrap();
    let assignment = proposal.assign(&agents).unwrap();
    let trace_id = "trace-reclaim";
    let state_root = root.path().join("state");

    let store = MasterStore::new(&state_root, trace_id).unwrap();
    let state = MasterState::initialize(trace_id, proposal, assignment, 1).unwrap();
    store.initialize(state, 1).unwrap();

    let mut state = store.load().unwrap();
    state.approve("user", 2).unwrap();
    let state = store
        .append(
            state,
            MasterJournalEventKind::Approved,
            None,
            "user",
            "approve",
            2,
        )
        .unwrap();
    let mut state = state;
    let lease = state
        .lease_task("T1", &agents[0], "term-old".into(), 100, 100)
        .unwrap();
    let state = store
        .append(
            state,
            MasterJournalEventKind::Leased,
            Some("T1".into()),
            "master",
            "lease",
            100,
        )
        .unwrap();
    let mut state = state;
    state.begin_starting("T1", lease.generation, 150).unwrap();
    let state = store
        .append(
            state,
            MasterJournalEventKind::Starting,
            Some("T1".into()),
            "master",
            "start",
            150,
        )
        .unwrap();
    assert!(state.tasks["T1"].active_lease.is_some());

    let output = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args([
            "master",
            "reclaim",
            "--trace-id",
            trace_id,
            "--state-root",
            state_root.to_str().unwrap(),
            "--task",
            "T1",
            "--reason",
            "lease expired",
            "--force",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state = store.load().unwrap();
    assert!(state.tasks["T1"].active_lease.is_none());
    assert_eq!(
        state.tasks["T1"].status,
        first_principles_agent::WorkItemStatus::Retrying
    );

    let mut state = state;
    let replacement = state
        .lease_task("T1", &agents[0], "term-new".into(), 100, 300)
        .unwrap();
    assert_eq!(replacement.generation, lease.generation + 1);
}
