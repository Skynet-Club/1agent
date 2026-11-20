//! MAST-inspired fault injection for Master/Work Item collaboration.
//!
//! These are deterministic, no-Workshop tests that map the MAST FC3
//! verification/termination failures (and a few FC1/FC2 cases) onto 1Agent
//! primitives: fake artifact SHA, missing artifact, missing criterion result,
//! self-verification, stale/duplicate receipts and repeated blocked actions.

use std::path::Path;

use first_principles_agent::master_coordination::STALL_THRESHOLD;
use first_principles_agent::{
    AgentAvailability, AgentProfile, AssignmentCandidate, AssignmentProposal, CriterionResult,
    ExecutionObservation, ExecutionReceipt, ExecutionStatus, ExecutionTarget,
    MASTER_PLAN_SCHEMA_VERSION, MasterCoordinationError, MasterState, PermissionSet, PlanProposal,
    RiskLevel, VerificationInput, WorkBudget, WorkItem, WorkItemStatus, artifact_expectation,
    inspect_artifacts,
};
use tempfile::tempdir;

fn item(workspace: &Path, expected_artifact: &str, criterion: &str) -> WorkItem {
    WorkItem {
        id: "task".into(),
        title: "Fault task".into(),
        objective: "Produce the expected artifact".into(),
        dependencies: Vec::new(),
        required_capabilities: vec!["rust".into()],
        required_tools: vec!["shell".into()],
        permissions: PermissionSet {
            read_roots: vec![workspace.to_path_buf()],
            allow_network: false,
            allow_write: true,
        },
        acceptance_criteria: vec![criterion.into()],
        expected_artifacts: vec![expected_artifact.into()],
        workspace: workspace.to_path_buf(),
        budget: WorkBudget::default(),
        deadline_unix_ms: None,
        risk: RiskLevel::Low,
        status: WorkItemStatus::Pending,
    }
}

fn plan(workspace: &Path, expected_artifact: &str, criterion: &str) -> PlanProposal {
    PlanProposal {
        schema_version: MASTER_PLAN_SCHEMA_VERSION,
        plan_id: "plan-mast".into(),
        goal_id: "goal-mast".into(),
        version: 1,
        summary: "MAST fault injection".into(),
        task_contract: None,
        work_items: vec![item(workspace, expected_artifact, criterion)],
        max_parallel: 1,
        approved: false,
    }
}

fn assignment() -> AssignmentProposal {
    AssignmentProposal {
        schema_version: MASTER_PLAN_SCHEMA_VERSION,
        plan_id: "plan-mast".into(),
        assignments: vec![AssignmentCandidate {
            task_id: "task".into(),
            agent_id: "worker".into(),
            runtime: "pi".into(),
            selection_reason: "mast".into(),
        }],
    }
}

fn agent() -> AgentProfile {
    AgentProfile {
        agent_id: "worker".into(),
        runtime: "pi".into(),
        display_name: None,
        capabilities: vec!["rust".into()],
        tools: vec!["shell".into()],
        runtime_command: vec!["pi".into()],
        permissions: PermissionSet::default(),
        availability: AgentAvailability::Idle,
        current_load: 0,
        observed_success_permille: Some(1_000),
        observed_runs: 1,
        cost_micros: None,
        max_risk: RiskLevel::Low,
    }
}

fn started_state(
    workspace: &Path,
    expected_artifact: &str,
    criterion: &str,
) -> (MasterState, u64, String) {
    let mut state = MasterState::initialize(
        "trace-mast",
        plan(workspace, expected_artifact, criterion),
        assignment(),
        1,
    )
    .unwrap();
    state.approve("user", 2).unwrap();
    let lease = state
        .lease_task("task", &agent(), "term-mast".into(), 1_000, 3)
        .unwrap();
    let key = lease.idempotency_key.clone();
    state.begin_starting("task", lease.generation, 4).unwrap();
    (state, lease.generation, key)
}

fn receipt(generation: u64, key: &str) -> ExecutionReceipt {
    ExecutionReceipt {
        task_id: "task".into(),
        agent_id: "worker".into(),
        lease_generation: generation,
        idempotency_key: key.into(),
        execution_target: ExecutionTarget {
            session_id: Some("term-mast".into()),
            pane_id: None,
            job_id: None,
        },
        status: ExecutionStatus::Completed,
        exit_code: Some(0),
        artifacts: vec!["result.txt".into()],
        result_digest: None,
        error: None,
        started_unix_ms: 10,
        finished_unix_ms: 11,
    }
}

fn verify_input(generation: u64, criteria_results: Vec<CriterionResult>) -> VerificationInput {
    VerificationInput {
        task_id: "task".into(),
        lease_generation: generation,
        criteria_results,
        artifact_checks: Vec::new(),
        test_results: Vec::new(),
        evidence: vec!["independent verifier inspected the artifact".into()],
        verified_by: "verifier:mast".into(),
        verified_at_unix_ms: 20,
    }
}

#[test]
fn fake_artifact_sha_is_rejected() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("result.txt"), "artifact").unwrap();
    let expected = format!("result.txt@sha256:{}", "0".repeat(64));
    let (mut state, generation, key) = started_state(dir.path(), &expected, "result exists");
    state.record_receipt(receipt(generation, &key), 10).unwrap();
    let expectations = vec![artifact_expectation(&expected)];
    let checks = inspect_artifacts(dir.path(), &expectations).unwrap();
    let mut input = verify_input(
        generation,
        vec![CriterionResult {
            criterion: "result exists".into(),
            passed: true,
            evidence: vec!["claimed artifact matches".into()],
        }],
    );
    input.artifact_checks = checks;
    let result = state.verify(input, 20).unwrap();
    assert_eq!(result.status, WorkItemStatus::Rejected);
    assert!(
        result
            .failure_reason
            .unwrap_or_default()
            .contains("SHA-256 mismatch")
    );
}

#[test]
fn missing_artifact_is_rejected() {
    let dir = tempdir().unwrap();
    let (mut state, generation, key) = started_state(dir.path(), "result.txt", "result exists");
    state.record_receipt(receipt(generation, &key), 10).unwrap();
    let expectations = vec![artifact_expectation("result.txt")];
    let checks = inspect_artifacts(dir.path(), &expectations).unwrap();
    let mut input = verify_input(
        generation,
        vec![CriterionResult {
            criterion: "result exists".into(),
            passed: true,
            evidence: vec!["claimed artifact exists".into()],
        }],
    );
    input.artifact_checks = checks;
    let result = state.verify(input, 20).unwrap();
    assert_eq!(result.status, WorkItemStatus::Rejected);
    assert!(
        result
            .failure_reason
            .unwrap_or_default()
            .contains("missing or is not a file")
    );
}

#[test]
fn missing_criterion_result_is_rejected() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("result.txt"), "artifact").unwrap();
    let (mut state, generation, key) = started_state(dir.path(), "result.txt", "result exists");
    state.record_receipt(receipt(generation, &key), 10).unwrap();
    let expectations = vec![artifact_expectation("result.txt")];
    let checks = inspect_artifacts(dir.path(), &expectations).unwrap();
    let mut input = verify_input(generation, Vec::new());
    input.artifact_checks = checks;
    let result = state.verify(input, 20).unwrap();
    assert_eq!(result.status, WorkItemStatus::Rejected);
    assert!(
        result
            .failure_reason
            .unwrap_or_default()
            .contains("exactly one result")
    );
}

#[test]
fn self_verification_is_rejected_by_acceptance() {
    let contract = first_principles_agent::acceptance::AcceptanceContract {
        task_id: "task".into(),
        objective: "verify".into(),
        acceptance_criteria: vec!["result exists".into()],
        hidden_criteria: Vec::new(),
        expected_artifacts: Vec::new(),
        risk: RiskLevel::Low,
        required_verifier: first_principles_agent::acceptance::VerifierKind::Independent,
        residual_risks: Vec::new(),
    };
    let input = first_principles_agent::verification::VerificationInput {
        executor_id: "agent:worker".into(),
        verifier_id: "agent:worker".into(),
        acceptance_criteria: vec!["result exists".into()],
        hidden_criteria: Vec::new(),
        criteria_results: vec![first_principles_agent::verification::CriterionResult {
            criterion: "result exists".into(),
            passed: true,
            evidence: vec!["self-check".into()],
        }],
        expected_artifacts: Vec::new(),
        artifact_checks: Vec::new(),
    };
    assert!(first_principles_agent::acceptance::close(&contract, &input, 1).is_err());
}

#[test]
fn duplicate_and_stale_receipts_fail_closed() {
    let dir = tempdir().unwrap();
    let (mut state, generation, key) = started_state(dir.path(), "result.txt", "result exists");
    state.record_receipt(receipt(generation, &key), 10).unwrap();
    assert!(matches!(
        state.record_receipt(receipt(generation, &key), 11),
        Err(MasterCoordinationError::DuplicateReceipt)
    ));

    let (mut state, generation, key) = started_state(dir.path(), "result.txt", "result exists");
    assert!(matches!(
        state.record_receipt(receipt(generation + 1, &key), 10),
        Err(MasterCoordinationError::StaleReceipt)
    ));
}

#[test]
fn repeated_blocked_observations_stall_and_replan() {
    let dir = tempdir().unwrap();
    let (mut state, _, _) = started_state(dir.path(), "result.txt", "result exists");
    state
        .record_observation("task", ExecutionObservation::Running, 5)
        .unwrap();
    for i in 0..=STALL_THRESHOLD {
        let result = state.record_observation(
            "task",
            ExecutionObservation::Blocked {
                reason: "waiting for the same input".into(),
            },
            6 + u128::from(i),
        );
        if i < STALL_THRESHOLD {
            assert!(result.is_ok(), "unexpected stall at {i}: {result:?}");
        } else {
            assert!(matches!(result, Err(MasterCoordinationError::Stalled(_))));
        }
    }
    assert_eq!(state.tasks["task"].status, WorkItemStatus::Replanning);
    assert_eq!(state.tasks["task"].progress.stall_count, STALL_THRESHOLD);
}

#[test]
fn task_contract_coverage_failure_is_rejected() {
    let dir = tempdir().unwrap();
    let mut plan = plan(dir.path(), "result.txt", "result exists");
    plan.task_contract = Some(first_principles_agent::TaskContract {
        objective: "cover the task".into(),
        deliverables: vec![first_principles_agent::Deliverable {
            id: "deliverable-1".into(),
            statement: "deliver".into(),
            acceptance: vec!["result exists".into()],
            work_item_ids: Vec::new(),
        }],
        constraints: Vec::new(),
        non_goals: Vec::new(),
        sources: Vec::new(),
        acceptance: Vec::new(),
    });
    assert!(plan.validate().is_err());
}
