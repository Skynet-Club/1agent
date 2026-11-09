use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::master::{
    AgentProfile, ArtifactCheck, AssignmentProposal, CriterionResult, ExecutionStatus,
    MasterPlanError, PlanProposal, TaskLease, TestResult, VerificationResult, WorkItem,
    WorkItemStatus,
};

pub const MASTER_STATE_SCHEMA_VERSION: u32 = 1;
pub const MASTER_JOURNAL_SCHEMA_VERSION: u32 = 1;
pub const MAX_AUTOMATIC_RETRIES: u32 = 1;
pub const MAX_AUTOMATIC_REPLANS: u32 = 1;
const MAX_STATE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_JOURNAL_EVENT_BYTES: usize = 4 * 1024 * 1024;
const MAX_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExecutionTarget {
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub pane_id: Option<u64>,
    #[serde(default)]
    pub job_id: Option<String>,
}

impl ExecutionTarget {
    pub fn is_connected(&self) -> bool {
        self.session_id.is_some() || self.pane_id.is_some() || self.job_id.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExecutionReceipt {
    pub task_id: String,
    pub agent_id: String,
    pub lease_generation: u64,
    pub idempotency_key: String,
    pub execution_target: ExecutionTarget,
    pub status: ExecutionStatus,
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub artifacts: Vec<String>,
    #[serde(default)]
    pub result_digest: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    pub started_unix_ms: u128,
    pub finished_unix_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExecutionSubmission {
    pub task_id: String,
    pub agent_id: String,
    pub lease_generation: u64,
    pub idempotency_key: String,
    pub execution_target: ExecutionTarget,
    pub submitted_unix_ms: u128,
    pub deadline_unix_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ExecutionObservation {
    Running,
    AwaitingInput { reason: String },
    Blocked { reason: String },
    Finished { receipt: Box<ExecutionReceipt> },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionKind {
    PlanApproved,
    PlanRejected,
    PlanRevised,
    TaskLeased,
    TaskTakenOver,
    TakeoverEnded,
    LeaseReclaimed,
    GoalAccepted,
    RetryRequested,
    ReplanRequested,
    ReplanApplied,
    TaskCancelled,
    RunPaused,
    RunResumed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DecisionRecord {
    pub decision_id: String,
    pub kind: DecisionKind,
    #[serde(default)]
    pub task_id: Option<String>,
    pub actor: String,
    pub reason: String,
    pub created_unix_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VerificationInput {
    pub task_id: String,
    pub lease_generation: u64,
    pub criteria_results: Vec<CriterionResult>,
    pub artifact_checks: Vec<ArtifactCheck>,
    #[serde(default)]
    pub test_results: Vec<TestResult>,
    #[serde(default)]
    pub evidence: Vec<String>,
    pub verified_by: String,
    pub verified_at_unix_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TaskRuntimeState {
    pub status: WorkItemStatus,
    pub attempts: u32,
    pub retries: u32,
    pub generation: u64,
    #[serde(default)]
    pub active_lease: Option<TaskLease>,
    #[serde(default)]
    pub execution_target: Option<ExecutionTarget>,
    #[serde(default)]
    pub submitted_unix_ms: Option<u128>,
    #[serde(default)]
    pub receipt: Option<ExecutionReceipt>,
    #[serde(default)]
    pub verification: Option<VerificationResult>,
    #[serde(default)]
    pub human_takeover: bool,
    #[serde(default)]
    pub last_error: Option<String>,
}

impl Default for TaskRuntimeState {
    fn default() -> Self {
        Self {
            status: WorkItemStatus::Pending,
            attempts: 0,
            retries: 0,
            generation: 0,
            active_lease: None,
            execution_target: None,
            submitted_unix_ms: None,
            receipt: None,
            verification: None,
            human_takeover: false,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MasterState {
    pub schema_version: u32,
    pub trace_id: String,
    pub plan: PlanProposal,
    pub assignment: AssignmentProposal,
    pub tasks: BTreeMap<String, TaskRuntimeState>,
    pub decisions: Vec<DecisionRecord>,
    #[serde(default)]
    pub replan_scope: Option<ReplanRequest>,
    pub retry_count: u32,
    pub replan_count: u32,
    pub paused: bool,
    #[serde(default)]
    pub plan_rejected: bool,
    #[serde(default)]
    pub goal_accepted: bool,
    pub approved_at_unix_ms: Option<u128>,
    pub updated_unix_ms: u128,
    pub event_seq: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReplanRequest {
    pub source_task: String,
    pub affected_tasks: Vec<String>,
    pub preserved_tasks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactExpectation {
    pub path: String,
    pub expected_sha256: Option<String>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MasterCoordinationError {
    #[error("master plan is invalid: {0}")]
    Plan(#[from] MasterPlanError),
    #[error("unsupported master state schema version {0}")]
    UnsupportedStateVersion(u32),
    #[error("master trace id is empty or unsafe")]
    InvalidTraceId,
    #[error("assignment must contain exactly one entry for every work item")]
    InvalidAssignment,
    #[error("unknown task '{0}'")]
    UnknownTask(String),
    #[error("plan has not been approved")]
    NotApproved,
    #[error("plan has been rejected")]
    PlanRejected,
    #[error("a new master plan must be initialized unapproved")]
    PlanMustInitializeUnapproved,
    #[error("plan revision is no longer allowed after execution starts")]
    PlanAlreadyStarted,
    #[error("run is paused")]
    Paused,
    #[error("task '{0}' is under human takeover")]
    HumanTakeover(String),
    #[error("task '{0}' is not under human takeover")]
    NotUnderTakeover(String),
    #[error("goal has already been accepted")]
    GoalAlreadyAccepted,
    #[error("task '{0}' is not verified; goal acceptance is blocked")]
    GoalNotVerified(String),
    #[error("task '{task}' cannot transition from {from:?} to {to:?}")]
    InvalidTransition {
        task: String,
        from: WorkItemStatus,
        to: WorkItemStatus,
    },
    #[error("task '{0}' still has an active lease")]
    ActiveLease(String),
    #[error("task '{0}' has no active lease")]
    NoActiveLease(String),
    #[error("task '{0}' lease has not expired")]
    LeaseNotExpired(String),
    #[error("task '{0}' execution is not confirmed terminated")]
    ExecutionNotTerminated(String),
    #[error("task '{0}' has unmet dependencies")]
    DependenciesNotVerified(String),
    #[error("task '{task}' was leased to a different agent than '{agent}'")]
    LeaseMismatch { task: String, agent: String },
    #[error("Agent '{0}' is not currently available")]
    AgentUnavailable(String),
    #[error("receipt generation does not match the active task lease")]
    StaleReceipt,
    #[error("receipt has already been processed")]
    DuplicateReceipt,
    #[error("task '{0}' has no execution receipt")]
    MissingReceipt(String),
    #[error("task '{0}' exceeded its automatic retry limit")]
    RetryLimit(String),
    #[error("automatic replanning limit has been reached")]
    ReplanLimit,
    #[error("replacement plan must keep the same plan and goal IDs and advance the version")]
    InvalidReplacementPlan,
    #[error("replacement plan failed closed verification: {0}")]
    VerificationRejected(String),
    #[error("replacement plan must preserve verified task '{0}' unchanged")]
    VerifiedTaskChanged(String),
    #[error("replacement plan is not approved")]
    ReplacementNotApproved,
    #[error("artifact path '{0}' escapes the work-item workspace")]
    UnsafeArtifact(String),
    #[error("execution target is not connected")]
    InvalidExecutionTarget,
    #[error("master store failed: {0}")]
    Store(String),
    #[error("master journal audit failed: {0}")]
    JournalAudit(String),
}

impl MasterState {
    pub fn initialize(
        trace_id: impl Into<String>,
        plan: PlanProposal,
        assignment: AssignmentProposal,
        now_unix_ms: u128,
    ) -> Result<Self, MasterCoordinationError> {
        let trace_id = trace_id.into();
        validate_component(&trace_id)?;
        plan.validate()?;
        if plan.approved {
            return Err(MasterCoordinationError::PlanMustInitializeUnapproved);
        }
        if assignment.schema_version != crate::master::MASTER_PLAN_SCHEMA_VERSION
            || assignment.plan_id != plan.plan_id
            || assignment
                .assignments
                .iter()
                .any(|item| item.agent_id.trim().is_empty() || item.runtime.trim().is_empty())
        {
            return Err(MasterCoordinationError::InvalidAssignment);
        }
        let expected = plan
            .work_items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<BTreeSet<_>>();
        let assignment_ids = assignment
            .assignments
            .iter()
            .map(|item| item.task_id.as_str())
            .collect::<BTreeSet<_>>();
        if expected.len() != plan.work_items.len()
            || assignment.assignments.len() != plan.work_items.len()
            || assignment_ids.len() != assignment.assignments.len()
        {
            return Err(MasterCoordinationError::InvalidAssignment);
        }
        let tasks = plan
            .work_items
            .iter()
            .map(|item| (item.id.clone(), TaskRuntimeState::default()))
            .collect();
        let mut state = Self {
            schema_version: MASTER_STATE_SCHEMA_VERSION,
            trace_id,
            plan,
            assignment,
            tasks,
            decisions: Vec::new(),
            replan_scope: None,
            retry_count: 0,
            replan_count: 0,
            paused: false,
            plan_rejected: false,
            goal_accepted: false,
            approved_at_unix_ms: None,
            updated_unix_ms: now_unix_ms,
            event_seq: 0,
        };
        state.advance_ready(now_unix_ms);
        Ok(state)
    }

    pub fn approve(
        &mut self,
        actor: impl Into<String>,
        now_unix_ms: u128,
    ) -> Result<(), MasterCoordinationError> {
        if self.plan_rejected {
            return Err(MasterCoordinationError::PlanRejected);
        }
        if self.plan.approved {
            return Err(MasterCoordinationError::InvalidTransition {
                task: "plan".into(),
                from: WorkItemStatus::Assigned,
                to: WorkItemStatus::Ready,
            });
        }
        self.plan.approved = true;
        self.approved_at_unix_ms = Some(now_unix_ms);
        self.record_decision(
            DecisionKind::PlanApproved,
            None,
            actor,
            "plan approved",
            now_unix_ms,
        );
        self.advance_ready(now_unix_ms);
        Ok(())
    }

    pub fn accept_goal(
        &mut self,
        actor: impl Into<String>,
        reason: impl Into<String>,
        now_unix_ms: u128,
    ) -> Result<(), MasterCoordinationError> {
        if !self.plan.approved {
            return Err(MasterCoordinationError::NotApproved);
        }
        if self.goal_accepted {
            return Err(MasterCoordinationError::GoalAlreadyAccepted);
        }
        if self.tasks.is_empty() {
            return Err(MasterCoordinationError::GoalNotVerified(
                "no work items".into(),
            ));
        }
        if let Some((task_id, _)) = self
            .tasks
            .iter()
            .find(|(_, task)| task.status != WorkItemStatus::Verified)
        {
            return Err(MasterCoordinationError::GoalNotVerified(task_id.clone()));
        }
        self.goal_accepted = true;
        self.record_decision(
            DecisionKind::GoalAccepted,
            None,
            actor,
            reason.into(),
            now_unix_ms,
        );
        self.updated_unix_ms = now_unix_ms;
        Ok(())
    }

    pub fn reject_plan(
        &mut self,
        actor: impl Into<String>,
        reason: impl Into<String>,
        now_unix_ms: u128,
    ) -> Result<(), MasterCoordinationError> {
        if self.plan.approved
            || self.tasks.values().any(|task| {
                !matches!(
                    task.status,
                    WorkItemStatus::Pending | WorkItemStatus::Ready | WorkItemStatus::Cancelled
                )
            })
        {
            return Err(MasterCoordinationError::PlanAlreadyStarted);
        }
        self.plan_rejected = true;
        for task in self.tasks.values_mut() {
            if matches!(task.status, WorkItemStatus::Pending | WorkItemStatus::Ready) {
                task.status = WorkItemStatus::Cancelled;
            }
        }
        self.record_decision(
            DecisionKind::PlanRejected,
            None,
            actor,
            reason.into(),
            now_unix_ms,
        );
        self.updated_unix_ms = now_unix_ms;
        Ok(())
    }

    pub fn revise_plan(
        &mut self,
        replacement: PlanProposal,
        assignment: AssignmentProposal,
        actor: impl Into<String>,
        reason: impl Into<String>,
        now_unix_ms: u128,
    ) -> Result<(), MasterCoordinationError> {
        if self.plan.approved
            || self.plan_rejected
            || self
                .tasks
                .values()
                .any(|task| !matches!(task.status, WorkItemStatus::Pending | WorkItemStatus::Ready))
        {
            return Err(MasterCoordinationError::PlanAlreadyStarted);
        }
        if replacement.approved {
            return Err(MasterCoordinationError::ReplacementNotApproved);
        }
        if replacement.plan_id != self.plan.plan_id
            || replacement.goal_id != self.plan.goal_id
            || replacement.version <= self.plan.version
            || assignment.plan_id != replacement.plan_id
            || assignment.schema_version != crate::master::MASTER_PLAN_SCHEMA_VERSION
        {
            return Err(MasterCoordinationError::InvalidReplacementPlan);
        }
        replacement.validate()?;
        self.plan = replacement;
        self.assignment = assignment;
        self.tasks = self
            .plan
            .work_items
            .iter()
            .map(|item| (item.id.clone(), TaskRuntimeState::default()))
            .collect();
        self.record_decision(
            DecisionKind::PlanRevised,
            None,
            actor,
            reason.into(),
            now_unix_ms,
        );
        self.updated_unix_ms = now_unix_ms;
        self.advance_ready(now_unix_ms);
        Ok(())
    }

    pub fn advance_ready(&mut self, now_unix_ms: u128) {
        let verified = self
            .tasks
            .iter()
            .filter_map(|(id, task)| {
                (task.status == WorkItemStatus::Verified).then_some(id.clone())
            })
            .collect::<BTreeSet<_>>();
        for item in &self.plan.work_items {
            let Some(task) = self.tasks.get_mut(&item.id) else {
                continue;
            };
            if task.status == WorkItemStatus::Pending
                && item.dependencies.iter().all(|id| verified.contains(id))
                && (self.plan.approved || item.dependencies.is_empty())
            {
                task.status = WorkItemStatus::Ready;
            }
        }
        self.updated_unix_ms = now_unix_ms;
    }

    pub fn lease_task(
        &mut self,
        task_id: &str,
        agent: &AgentProfile,
        execution_target: String,
        lease_duration_ms: u64,
        now_unix_ms: u128,
    ) -> Result<TaskLease, MasterCoordinationError> {
        if !self.plan.approved {
            return Err(MasterCoordinationError::NotApproved);
        }
        if self.plan_rejected {
            return Err(MasterCoordinationError::PlanRejected);
        }
        if self.paused {
            return Err(MasterCoordinationError::Paused);
        }
        let item = self.work_item(task_id)?.clone();
        let state = self.task(task_id)?;
        if state.human_takeover {
            return Err(MasterCoordinationError::HumanTakeover(task_id.into()));
        }
        if state.active_lease.is_some() {
            return Err(MasterCoordinationError::ActiveLease(task_id.into()));
        }
        if !matches!(
            state.status,
            WorkItemStatus::Ready | WorkItemStatus::Retrying
        ) {
            return Err(MasterCoordinationError::InvalidTransition {
                task: task_id.into(),
                from: state.status,
                to: WorkItemStatus::Assigned,
            });
        }
        if item.dependencies.iter().any(|dependency| {
            self.tasks
                .get(dependency)
                .is_none_or(|task| task.status != WorkItemStatus::Verified)
        }) {
            return Err(MasterCoordinationError::DependenciesNotVerified(
                task_id.into(),
            ));
        }
        let assignment = self
            .assignment
            .assignments
            .iter()
            .find(|candidate| candidate.task_id == item.id)
            .ok_or(MasterCoordinationError::InvalidAssignment)?;
        if agent.availability != crate::master::AgentAvailability::Idle {
            return Err(MasterCoordinationError::AgentUnavailable(
                agent.agent_id.clone(),
            ));
        }
        if assignment.runtime != agent.runtime {
            return Err(MasterCoordinationError::LeaseMismatch {
                task: task_id.into(),
                agent: agent.agent_id.clone(),
            });
        }
        let selection_reason = assignment.selection_reason.clone();
        let generation = state.generation.saturating_add(1);
        let lease = TaskLease {
            task_id: task_id.into(),
            agent_id: agent.agent_id.clone(),
            execution_target,
            idempotency_key: format!("{}:{task_id}:{generation}", self.plan.plan_id),
            expires_unix_ms: now_unix_ms.saturating_add(u128::from(lease_duration_ms)),
            generation,
        };
        let state = self.task_mut(task_id)?;
        state.generation = generation;
        state.attempts = state.attempts.saturating_add(1);
        state.active_lease = Some(lease.clone());
        set_task_status(state, WorkItemStatus::Assigned);
        self.record_decision(
            DecisionKind::TaskLeased,
            Some(task_id.into()),
            agent.agent_id.clone(),
            selection_reason,
            now_unix_ms,
        );
        self.updated_unix_ms = now_unix_ms;
        Ok(lease)
    }

    pub fn begin_starting(
        &mut self,
        task_id: &str,
        generation: u64,
        now_unix_ms: u128,
    ) -> Result<(), MasterCoordinationError> {
        let state = self.task_mut(task_id)?;
        if !lease_matches(state, generation)
            || state
                .active_lease
                .as_ref()
                .is_none_or(|lease| !lease.is_active(now_unix_ms))
        {
            return Err(MasterCoordinationError::StaleReceipt);
        }
        set_task_status(state, WorkItemStatus::Starting);
        self.updated_unix_ms = now_unix_ms;
        Ok(())
    }

    pub fn renew_lease(
        &mut self,
        task_id: &str,
        generation: u64,
        expires_unix_ms: u128,
        now_unix_ms: u128,
    ) -> Result<(), MasterCoordinationError> {
        let state = self.task_mut(task_id)?;
        let lease = state
            .active_lease
            .as_mut()
            .filter(|lease| lease.generation == generation)
            .ok_or(MasterCoordinationError::StaleReceipt)?;
        lease.expires_unix_ms = expires_unix_ms;
        self.updated_unix_ms = now_unix_ms;
        Ok(())
    }

    /// Explicitly reclaim an expired lease after the caller has terminated the old
    /// execution. This is manual recovery, not an automatic retry: it does not
    /// consume the automatic retry budget.
    pub fn reclaim_expired_lease(
        &mut self,
        task_id: &str,
        actor: impl Into<String>,
        reason: impl Into<String>,
        execution_terminated: bool,
        now_unix_ms: u128,
    ) -> Result<(), MasterCoordinationError> {
        let state = self.task_mut(task_id)?;
        let lease = state
            .active_lease
            .as_ref()
            .ok_or_else(|| MasterCoordinationError::NoActiveLease(task_id.into()))?;
        if lease.is_active(now_unix_ms) {
            return Err(MasterCoordinationError::LeaseNotExpired(task_id.into()));
        }
        if !execution_terminated {
            return Err(MasterCoordinationError::ExecutionNotTerminated(
                task_id.into(),
            ));
        }
        if !matches!(
            state.status,
            WorkItemStatus::Assigned
                | WorkItemStatus::Starting
                | WorkItemStatus::Running
                | WorkItemStatus::AwaitingInput
                | WorkItemStatus::Blocked
        ) {
            return Err(MasterCoordinationError::InvalidTransition {
                task: task_id.into(),
                from: state.status,
                to: WorkItemStatus::Retrying,
            });
        }
        state.active_lease = None;
        set_task_status(state, WorkItemStatus::Retrying);
        self.record_decision(
            DecisionKind::LeaseReclaimed,
            Some(task_id.into()),
            actor,
            reason.into(),
            now_unix_ms,
        );
        self.updated_unix_ms = now_unix_ms;
        Ok(())
    }

    pub fn record_receipt(
        &mut self,
        receipt: ExecutionReceipt,
        now_unix_ms: u128,
    ) -> Result<(), MasterCoordinationError> {
        let task_id = receipt.task_id.clone();
        let state = self.task_mut(&task_id)?;
        if state
            .receipt
            .as_ref()
            .is_some_and(|previous| previous.idempotency_key == receipt.idempotency_key)
        {
            return Err(MasterCoordinationError::DuplicateReceipt);
        }
        if !state.active_lease.as_ref().is_some_and(|lease| {
            lease.generation == receipt.lease_generation
                && lease.agent_id == receipt.agent_id
                && lease.idempotency_key == receipt.idempotency_key
                && lease.is_active(now_unix_ms)
        }) {
            return Err(MasterCoordinationError::StaleReceipt);
        }
        if !matches!(
            state.status,
            WorkItemStatus::Starting
                | WorkItemStatus::Running
                | WorkItemStatus::AwaitingInput
                | WorkItemStatus::Blocked
        ) {
            return Err(MasterCoordinationError::InvalidTransition {
                task: task_id,
                from: state.status,
                to: WorkItemStatus::ArtifactReady,
            });
        }
        if matches!(
            state.status,
            WorkItemStatus::Starting | WorkItemStatus::AwaitingInput | WorkItemStatus::Blocked
        ) {
            state.status = WorkItemStatus::Running;
        }
        let next = match receipt.status {
            ExecutionStatus::Completed => WorkItemStatus::ArtifactReady,
            ExecutionStatus::Cancelled => WorkItemStatus::Cancelled,
            ExecutionStatus::Failed | ExecutionStatus::Interrupted => WorkItemStatus::Rejected,
            ExecutionStatus::TimedOut => WorkItemStatus::Blocked,
        };
        state.last_error = receipt.error.clone();
        state.receipt = Some(receipt);
        state.active_lease = None;
        set_task_status(state, next);
        self.updated_unix_ms = now_unix_ms;
        Ok(())
    }

    pub fn record_submission(
        &mut self,
        submission: ExecutionSubmission,
        now_unix_ms: u128,
    ) -> Result<(), MasterCoordinationError> {
        let state = self.task_mut(&submission.task_id)?;
        if !state.active_lease.as_ref().is_some_and(|lease| {
            lease.generation == submission.lease_generation
                && lease.agent_id == submission.agent_id
                && lease.idempotency_key == submission.idempotency_key
        }) {
            return Err(MasterCoordinationError::StaleReceipt);
        }
        if !submission.execution_target.is_connected() {
            return Err(MasterCoordinationError::InvalidExecutionTarget);
        }
        if state.status != WorkItemStatus::Starting {
            return Err(MasterCoordinationError::InvalidTransition {
                task: submission.task_id,
                from: state.status,
                to: WorkItemStatus::Running,
            });
        }
        state.execution_target = Some(submission.execution_target);
        state.submitted_unix_ms = Some(submission.submitted_unix_ms);
        state.status = WorkItemStatus::Running;
        self.updated_unix_ms = now_unix_ms;
        Ok(())
    }

    pub fn record_observation(
        &mut self,
        task_id: &str,
        observation: ExecutionObservation,
        now_unix_ms: u128,
    ) -> Result<(), MasterCoordinationError> {
        match observation {
            ExecutionObservation::Finished { receipt } => {
                self.record_receipt(*receipt, now_unix_ms)
            }
            ExecutionObservation::Running => {
                let state = self.task_mut(task_id)?;
                if !matches!(
                    state.status,
                    WorkItemStatus::Assigned
                        | WorkItemStatus::Starting
                        | WorkItemStatus::Running
                        | WorkItemStatus::AwaitingInput
                        | WorkItemStatus::Blocked
                ) {
                    return Err(MasterCoordinationError::InvalidTransition {
                        task: task_id.into(),
                        from: state.status,
                        to: WorkItemStatus::Running,
                    });
                }
                state.status = WorkItemStatus::Running;
                self.updated_unix_ms = now_unix_ms;
                Ok(())
            }
            ExecutionObservation::AwaitingInput { reason } => {
                let state = self.task_mut(task_id)?;
                if !matches!(
                    state.status,
                    WorkItemStatus::Running | WorkItemStatus::AwaitingInput
                ) {
                    return Err(MasterCoordinationError::InvalidTransition {
                        task: task_id.into(),
                        from: state.status,
                        to: WorkItemStatus::AwaitingInput,
                    });
                }
                state.status = WorkItemStatus::AwaitingInput;
                state.last_error = Some(reason);
                self.updated_unix_ms = now_unix_ms;
                Ok(())
            }
            ExecutionObservation::Blocked { reason } => {
                let state = self.task_mut(task_id)?;
                if !matches!(
                    state.status,
                    WorkItemStatus::Running | WorkItemStatus::AwaitingInput
                ) {
                    return Err(MasterCoordinationError::InvalidTransition {
                        task: task_id.into(),
                        from: state.status,
                        to: WorkItemStatus::Blocked,
                    });
                }
                state.status = WorkItemStatus::Blocked;
                state.last_error = Some(reason);
                self.updated_unix_ms = now_unix_ms;
                Ok(())
            }
        }
    }

    pub fn verify(
        &mut self,
        input: VerificationInput,
        now_unix_ms: u128,
    ) -> Result<VerificationResult, MasterCoordinationError> {
        let task_id = input.task_id.clone();
        let item = self.work_item(&task_id)?.clone();
        let task = self.task(&task_id)?;
        if task.status != WorkItemStatus::ArtifactReady {
            return Err(MasterCoordinationError::InvalidTransition {
                task: task_id,
                from: task.status,
                to: WorkItemStatus::Verifying,
            });
        }
        if task.generation != input.lease_generation {
            return Err(MasterCoordinationError::StaleReceipt);
        }
        let receipt = task
            .receipt
            .as_ref()
            .ok_or_else(|| MasterCoordinationError::MissingReceipt(task_id.clone()))?;
        let mut failures = Vec::new();
        if receipt.status != ExecutionStatus::Completed || receipt.exit_code != Some(0) {
            failures.push("execution did not complete with exit code 0".into());
        }
        if input.verified_by.trim().is_empty() {
            failures.push("independent verifier identity is empty".into());
        }
        for expected in &item.acceptance_criteria {
            let matches = input
                .criteria_results
                .iter()
                .filter(|result| &result.criterion == expected)
                .collect::<Vec<_>>();
            if matches.len() != 1 {
                failures.push(format!(
                    "criterion must have exactly one result: {expected}"
                ));
            } else if !matches[0].passed || matches[0].evidence.is_empty() {
                failures.push(format!(
                    "criterion failed without passing evidence: {expected}"
                ));
            }
        }
        for expected in &item.expected_artifacts {
            let expectation = artifact_expectation(expected);
            let checks = input
                .artifact_checks
                .iter()
                .filter(|check| check.path == expectation.path)
                .collect::<Vec<_>>();
            if checks.len() != 1 {
                failures.push(format!(
                    "expected artifact must have exactly one check: {}",
                    expectation.path
                ));
                continue;
            }
            let check = checks[0];
            if !check.exists || !check.regular_file {
                failures.push(format!(
                    "artifact is missing or is not a file: {}",
                    check.path
                ));
            }
            if let Some(expected_sha256) = expectation.expected_sha256
                && check.sha256.as_deref() != Some(expected_sha256.as_str())
            {
                failures.push(format!("artifact SHA-256 mismatch: {}", check.path));
            }
        }
        for test in &input.test_results {
            if !test.passed {
                failures.push(format!("test failed: {}", test.name));
            }
        }
        if input
            .evidence
            .iter()
            .all(|evidence| evidence.trim().is_empty())
        {
            failures.push("verification contains no independent evidence".into());
        }

        let status = if failures.is_empty() {
            WorkItemStatus::Verified
        } else {
            WorkItemStatus::Rejected
        };
        let result = VerificationResult {
            task_id: task_id.clone(),
            status,
            checked_artifacts: input.artifact_checks,
            criteria_results: input.criteria_results,
            test_results: input.test_results,
            evidence: input.evidence,
            verified_by: input.verified_by,
            verified_at_unix_ms: input.verified_at_unix_ms,
            failure_reason: (!failures.is_empty()).then(|| failures.join("; ")),
        };
        let state = self.task_mut(&task_id)?;
        set_task_status(state, WorkItemStatus::Verifying);
        state.verification = Some(result.clone());
        if status == WorkItemStatus::Verified {
            set_task_status(state, WorkItemStatus::Verified);
        } else {
            state.last_error = result.failure_reason.clone();
            set_task_status(state, WorkItemStatus::Rejected);
        }
        self.updated_unix_ms = now_unix_ms;
        self.advance_ready(now_unix_ms);
        Ok(result)
    }

    pub fn request_retry(
        &mut self,
        task_id: &str,
        actor: impl Into<String>,
        reason: impl Into<String>,
        now_unix_ms: u128,
    ) -> Result<(), MasterCoordinationError> {
        let state = self.task_mut(task_id)?;
        if state.retries >= MAX_AUTOMATIC_RETRIES {
            return Err(MasterCoordinationError::RetryLimit(task_id.into()));
        }
        if !matches!(
            state.status,
            WorkItemStatus::Rejected | WorkItemStatus::Blocked
        ) {
            return Err(MasterCoordinationError::InvalidTransition {
                task: task_id.into(),
                from: state.status,
                to: WorkItemStatus::Retrying,
            });
        }
        state.retries = state.retries.saturating_add(1);
        set_task_status(state, WorkItemStatus::Retrying);
        state.active_lease = None;
        self.retry_count = self.retry_count.saturating_add(1);
        let reason = reason.into();
        self.record_decision(
            DecisionKind::RetryRequested,
            Some(task_id.into()),
            actor,
            reason,
            now_unix_ms,
        );
        self.updated_unix_ms = now_unix_ms;
        Ok(())
    }

    pub fn request_replan(
        &mut self,
        task_id: &str,
        actor: impl Into<String>,
        reason: impl Into<String>,
        now_unix_ms: u128,
    ) -> Result<ReplanRequest, MasterCoordinationError> {
        if self.replan_count >= MAX_AUTOMATIC_REPLANS {
            return Err(MasterCoordinationError::ReplanLimit);
        }
        let state = self.task(task_id)?;
        if !matches!(
            state.status,
            WorkItemStatus::Rejected | WorkItemStatus::Blocked
        ) {
            return Err(MasterCoordinationError::InvalidTransition {
                task: task_id.into(),
                from: state.status,
                to: WorkItemStatus::Replanning,
            });
        }
        let preserved = self
            .tasks
            .iter()
            .filter_map(|(id, task)| {
                (task.status == WorkItemStatus::Verified).then_some(id.clone())
            })
            .collect::<Vec<_>>();
        let affected = self
            .descendants(task_id)
            .into_iter()
            .filter(|id| !preserved.contains(id))
            .collect::<Vec<_>>();
        self.replan_count = self.replan_count.saturating_add(1);
        let state = self.task_mut(task_id)?;
        set_task_status(state, WorkItemStatus::Replanning);
        state.active_lease = None;
        let reason = reason.into();
        self.record_decision(
            DecisionKind::ReplanRequested,
            Some(task_id.into()),
            actor,
            reason,
            now_unix_ms,
        );
        self.updated_unix_ms = now_unix_ms;
        let request = ReplanRequest {
            source_task: task_id.into(),
            affected_tasks: affected,
            preserved_tasks: preserved,
        };
        self.replan_scope = Some(request.clone());
        Ok(request)
    }

    pub fn apply_replan(
        &mut self,
        replacement: PlanProposal,
        assignment: AssignmentProposal,
        actor: impl Into<String>,
        reason: impl Into<String>,
        now_unix_ms: u128,
    ) -> Result<(), MasterCoordinationError> {
        if self.replan_count == 0 {
            return Err(MasterCoordinationError::InvalidReplacementPlan);
        }
        let scope = self
            .replan_scope
            .clone()
            .ok_or(MasterCoordinationError::InvalidReplacementPlan)?;
        if !replacement.approved {
            return Err(MasterCoordinationError::ReplacementNotApproved);
        }
        replacement.validate()?;
        if replacement.plan_id != self.plan.plan_id
            || replacement.goal_id != self.plan.goal_id
            || replacement.version <= self.plan.version
            || assignment.plan_id != replacement.plan_id
            || assignment.schema_version != crate::master::MASTER_PLAN_SCHEMA_VERSION
            || assignment
                .assignments
                .iter()
                .any(|item| item.agent_id.trim().is_empty() || item.runtime.trim().is_empty())
        {
            return Err(MasterCoordinationError::InvalidReplacementPlan);
        }
        let mut preserved = BTreeMap::new();
        for (id, task) in &self.tasks {
            if task.status == WorkItemStatus::Verified {
                preserved.insert(id.clone(), self.work_item(id)?.clone());
            }
        }
        for (id, previous) in &preserved {
            let Some(current) = replacement.work_items.iter().find(|item| &item.id == id) else {
                return Err(MasterCoordinationError::VerifiedTaskChanged(id.clone()));
            };
            if current.objective != previous.objective
                || current.expected_artifacts != previous.expected_artifacts
                || current.acceptance_criteria != previous.acceptance_criteria
                || current.workspace != previous.workspace
            {
                return Err(MasterCoordinationError::VerifiedTaskChanged(id.clone()));
            }
        }
        let old_tasks = self.tasks.clone();
        let old_items = self
            .plan
            .work_items
            .iter()
            .map(|item| (item.id.clone(), item.clone()))
            .collect::<BTreeMap<_, _>>();
        for (id, task) in &old_tasks {
            if task.status == WorkItemStatus::Verified
                || scope.affected_tasks.contains(id)
                || scope.source_task == *id
            {
                continue;
            }
            let current = replacement
                .work_items
                .iter()
                .find(|item| &item.id == id)
                .ok_or_else(|| MasterCoordinationError::VerifiedTaskChanged(id.clone()))?;
            let previous = old_items
                .get(id)
                .ok_or_else(|| MasterCoordinationError::UnknownTask(id.clone()))?;
            if current != previous {
                return Err(MasterCoordinationError::VerifiedTaskChanged(id.clone()));
            }
        }
        self.plan = replacement;
        self.assignment = assignment;
        self.tasks = self
            .plan
            .work_items
            .iter()
            .map(|item| {
                let previous = old_tasks.get(&item.id);
                let preserve = previous.filter(|task| {
                    task.status == WorkItemStatus::Verified
                        || (!scope.affected_tasks.contains(&item.id)
                            && scope.source_task != item.id)
                });
                (
                    item.id.clone(),
                    preserve.cloned().unwrap_or_else(|| TaskRuntimeState {
                        attempts: old_tasks
                            .get(&item.id)
                            .map(|task| task.attempts)
                            .unwrap_or(0),
                        ..TaskRuntimeState::default()
                    }),
                )
            })
            .collect();
        self.replan_scope = None;
        self.record_decision(
            DecisionKind::ReplanApplied,
            None,
            actor,
            reason,
            now_unix_ms,
        );
        self.updated_unix_ms = now_unix_ms;
        self.advance_ready(now_unix_ms);
        Ok(())
    }

    pub fn set_paused(
        &mut self,
        paused: bool,
        actor: impl Into<String>,
        now_unix_ms: u128,
    ) -> Result<(), MasterCoordinationError> {
        self.paused = paused;
        self.record_decision(
            if paused {
                DecisionKind::RunPaused
            } else {
                DecisionKind::RunResumed
            },
            None,
            actor,
            if paused { "run paused" } else { "run resumed" },
            now_unix_ms,
        );
        self.updated_unix_ms = now_unix_ms;
        Ok(())
    }

    pub fn take_over(
        &mut self,
        task_id: &str,
        actor: impl Into<String>,
        reason: impl Into<String>,
        now_unix_ms: u128,
    ) -> Result<(), MasterCoordinationError> {
        let state = self.task_mut(task_id)?;
        if !matches!(
            state.status,
            WorkItemStatus::Assigned
                | WorkItemStatus::Starting
                | WorkItemStatus::Running
                | WorkItemStatus::AwaitingInput
                | WorkItemStatus::Blocked
        ) {
            return Err(MasterCoordinationError::InvalidTransition {
                task: task_id.into(),
                from: state.status,
                to: WorkItemStatus::Running,
            });
        }
        state.human_takeover = true;
        self.record_decision(
            DecisionKind::TaskTakenOver,
            Some(task_id.into()),
            actor,
            reason.into(),
            now_unix_ms,
        );
        self.updated_unix_ms = now_unix_ms;
        Ok(())
    }

    pub fn end_takeover(
        &mut self,
        task_id: &str,
        actor: impl Into<String>,
        reason: impl Into<String>,
        now_unix_ms: u128,
    ) -> Result<(), MasterCoordinationError> {
        let state = self.task_mut(task_id)?;
        if !state.human_takeover {
            return Err(MasterCoordinationError::NotUnderTakeover(task_id.into()));
        }
        state.human_takeover = false;
        self.record_decision(
            DecisionKind::TakeoverEnded,
            Some(task_id.into()),
            actor,
            reason.into(),
            now_unix_ms,
        );
        self.updated_unix_ms = now_unix_ms;
        Ok(())
    }

    pub fn cancel_task(
        &mut self,
        task_id: &str,
        actor: impl Into<String>,
        reason: impl Into<String>,
        now_unix_ms: u128,
    ) -> Result<(), MasterCoordinationError> {
        let state = self.task_mut(task_id)?;
        if !state.status.can_transition_to(WorkItemStatus::Cancelled) {
            return Err(MasterCoordinationError::InvalidTransition {
                task: task_id.into(),
                from: state.status,
                to: WorkItemStatus::Cancelled,
            });
        }
        set_task_status(state, WorkItemStatus::Cancelled);
        state.active_lease = None;
        self.record_decision(
            DecisionKind::TaskCancelled,
            Some(task_id.into()),
            actor,
            reason.into(),
            now_unix_ms,
        );
        self.updated_unix_ms = now_unix_ms;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), MasterCoordinationError> {
        if self.schema_version != MASTER_STATE_SCHEMA_VERSION {
            return Err(MasterCoordinationError::UnsupportedStateVersion(
                self.schema_version,
            ));
        }
        validate_component(&self.trace_id)?;
        if self.plan_rejected && self.plan.approved {
            return Err(MasterCoordinationError::PlanRejected);
        }
        self.plan.validate()?;
        if self.assignment.plan_id != self.plan.plan_id {
            return Err(MasterCoordinationError::InvalidAssignment);
        }
        if self.assignment.schema_version != crate::master::MASTER_PLAN_SCHEMA_VERSION
            || self
                .assignment
                .assignments
                .iter()
                .any(|item| item.agent_id.trim().is_empty() || item.runtime.trim().is_empty())
        {
            return Err(MasterCoordinationError::InvalidAssignment);
        }
        let plan_ids = self
            .plan
            .work_items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<BTreeSet<_>>();
        let assignment_ids = self
            .assignment
            .assignments
            .iter()
            .map(|item| item.task_id.as_str())
            .collect::<BTreeSet<_>>();
        let state_ids = self
            .tasks
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if plan_ids != assignment_ids || plan_ids != state_ids {
            return Err(MasterCoordinationError::InvalidAssignment);
        }
        for (id, task) in &self.tasks {
            if task.status == WorkItemStatus::Verified && task.verification.is_none() {
                return Err(MasterCoordinationError::VerificationRejected(format!(
                    "verified task '{id}' has no verification evidence"
                )));
            }
            if task
                .active_lease
                .as_ref()
                .is_some_and(|lease| lease.task_id != *id || lease.generation != task.generation)
            {
                return Err(MasterCoordinationError::StaleReceipt);
            }
            if matches!(
                task.status,
                WorkItemStatus::Running | WorkItemStatus::AwaitingInput | WorkItemStatus::Blocked
            ) && task
                .execution_target
                .as_ref()
                .is_none_or(|target| !target.is_connected())
            {
                return Err(MasterCoordinationError::InvalidExecutionTarget);
            }
            if task.status == WorkItemStatus::Verified && !self.plan.approved {
                return Err(MasterCoordinationError::NotApproved);
            }
        }
        Ok(())
    }

    fn work_item(&self, task_id: &str) -> Result<&WorkItem, MasterCoordinationError> {
        self.plan
            .work_items
            .iter()
            .find(|item| item.id == task_id)
            .ok_or_else(|| MasterCoordinationError::UnknownTask(task_id.into()))
    }

    fn task(&self, task_id: &str) -> Result<&TaskRuntimeState, MasterCoordinationError> {
        self.tasks
            .get(task_id)
            .ok_or_else(|| MasterCoordinationError::UnknownTask(task_id.into()))
    }

    fn task_mut(
        &mut self,
        task_id: &str,
    ) -> Result<&mut TaskRuntimeState, MasterCoordinationError> {
        self.tasks
            .get_mut(task_id)
            .ok_or_else(|| MasterCoordinationError::UnknownTask(task_id.into()))
    }

    fn descendants(&self, task_id: &str) -> BTreeSet<String> {
        let mut found = BTreeSet::from([task_id.to_string()]);
        loop {
            let mut changed = false;
            for item in &self.plan.work_items {
                if item.dependencies.iter().any(|id| found.contains(id))
                    && found.insert(item.id.clone())
                {
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        found
    }

    fn record_decision(
        &mut self,
        kind: DecisionKind,
        task_id: Option<String>,
        actor: impl Into<String>,
        reason: impl Into<String>,
        now_unix_ms: u128,
    ) {
        self.decisions.push(DecisionRecord {
            decision_id: Uuid::new_v4().to_string(),
            kind,
            task_id,
            actor: actor.into(),
            reason: reason.into(),
            created_unix_ms: now_unix_ms,
        });
    }
}

fn lease_matches(task: &TaskRuntimeState, generation: u64) -> bool {
    task.active_lease
        .as_ref()
        .is_some_and(|lease| lease.generation == generation)
}

fn set_task_status(task: &mut TaskRuntimeState, next: WorkItemStatus) {
    if task.status != next {
        task.status = next;
    }
}

pub fn artifact_expectation(spec: &str) -> ArtifactExpectation {
    let Some((path, digest)) = spec.rsplit_once("@sha256:") else {
        return ArtifactExpectation {
            path: spec.into(),
            expected_sha256: None,
        };
    };
    let expected = (digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| digest.to_ascii_lowercase());
    ArtifactExpectation {
        path: path.into(),
        expected_sha256: expected,
    }
}

pub fn inspect_artifacts(
    workspace: &Path,
    expectations: &[ArtifactExpectation],
) -> Result<Vec<ArtifactCheck>, MasterCoordinationError> {
    let canonical_workspace = workspace
        .canonicalize()
        .map_err(|error| MasterCoordinationError::Store(error.to_string()))?;
    expectations
        .iter()
        .map(|expectation| inspect_artifact(&canonical_workspace, expectation))
        .collect()
}

fn inspect_artifact(
    canonical_workspace: &Path,
    expectation: &ArtifactExpectation,
) -> Result<ArtifactCheck, MasterCoordinationError> {
    let relative = Path::new(&expectation.path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
    {
        return Err(MasterCoordinationError::UnsafeArtifact(
            expectation.path.clone(),
        ));
    }
    let target = canonical_workspace.join(relative);
    let metadata = match std::fs::metadata(&target) {
        Ok(metadata) => metadata,
        Err(error) => {
            return Ok(ArtifactCheck {
                path: expectation.path.clone(),
                exists: false,
                regular_file: false,
                bytes: 0,
                sha256: None,
                expected_sha256: expectation.expected_sha256.clone(),
                error: Some(error.to_string()),
            });
        }
    };
    let canonical = target
        .canonicalize()
        .map_err(|error| MasterCoordinationError::Store(error.to_string()))?;
    if !canonical.starts_with(canonical_workspace) {
        return Err(MasterCoordinationError::UnsafeArtifact(
            expectation.path.clone(),
        ));
    }
    if metadata.len() > MAX_ARTIFACT_BYTES {
        return Ok(ArtifactCheck {
            path: expectation.path.clone(),
            exists: true,
            regular_file: metadata.is_file(),
            bytes: metadata.len(),
            sha256: None,
            expected_sha256: expectation.expected_sha256.clone(),
            error: Some(format!("artifact exceeds {MAX_ARTIFACT_BYTES} bytes")),
        });
    }
    let digest = if metadata.is_file() {
        Some(sha256_file(&canonical).map_err(MasterCoordinationError::Store)?)
    } else {
        None
    };
    Ok(ArtifactCheck {
        path: expectation.path.clone(),
        exists: true,
        regular_file: metadata.is_file(),
        bytes: metadata.len(),
        sha256: digest,
        expected_sha256: expectation.expected_sha256.clone(),
        error: None,
    })
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MasterJournalEventKind {
    Initialized,
    Approved,
    PlanRejected,
    PlanRevised,
    Leased,
    Starting,
    Submitted,
    Observed,
    ReceiptRecorded,
    Verified,
    Rejected,
    RetryRequested,
    ReplanRequested,
    ReplanApplied,
    Paused,
    Resumed,
    TakenOver,
    TakeoverEnded,
    LeaseReclaimed,
    GoalAccepted,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MasterJournalEvent {
    pub schema_version: u32,
    pub seq: u64,
    pub trace_id: String,
    pub plan_id: String,
    pub timestamp_unix_ms: u128,
    pub kind: MasterJournalEventKind,
    #[serde(default)]
    pub task_id: Option<String>,
    pub actor: String,
    pub reason: String,
    #[serde(default)]
    pub previous_event_sha256: Option<String>,
    pub event_sha256: String,
    pub state: MasterState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MasterAudit {
    pub ok: bool,
    pub event_count: usize,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MasterStore {
    trace_dir: PathBuf,
}

impl MasterStore {
    pub fn new(root: impl AsRef<Path>, trace_id: &str) -> Result<Self, MasterCoordinationError> {
        validate_component(trace_id)?;
        Ok(Self {
            trace_dir: root.as_ref().join(trace_id),
        })
    }

    pub fn trace_dir(&self) -> &Path {
        &self.trace_dir
    }

    pub fn initialize(
        &self,
        mut state: MasterState,
        now_unix_ms: u128,
    ) -> Result<(), MasterCoordinationError> {
        if self.trace_dir.join("events.jsonl").exists() {
            return Err(MasterCoordinationError::Store(
                "refusing to overwrite an existing master journal".into(),
            ));
        }
        state.validate()?;
        state.event_seq = 1;
        state.updated_unix_ms = now_unix_ms;
        let event = journal_event(
            &state,
            1,
            MasterJournalEventKind::Initialized,
            None,
            "master",
            "master plan initialized in dry-run state",
            now_unix_ms,
        );
        self.append_event(event)?;
        self.save_state(&state)
    }

    pub fn append(
        &self,
        mut state: MasterState,
        kind: MasterJournalEventKind,
        task_id: Option<String>,
        actor: impl Into<String>,
        reason: impl Into<String>,
        now_unix_ms: u128,
    ) -> Result<MasterState, MasterCoordinationError> {
        state.validate()?;
        let seq = state.event_seq.saturating_add(1);
        state.event_seq = seq;
        state.updated_unix_ms = now_unix_ms;
        let event = journal_event(&state, seq, kind, task_id, actor, reason, now_unix_ms);
        self.append_event(event)?;
        self.save_state(&state)?;
        Ok(state)
    }

    pub fn load(&self) -> Result<MasterState, MasterCoordinationError> {
        let journal_path = self.trace_dir.join("events.jsonl");
        if journal_path.exists() {
            let (state, audit) = replay_master_journal(&journal_path)?;
            if !audit.ok {
                return Err(MasterCoordinationError::JournalAudit(
                    audit.issues.join("; "),
                ));
            }
            state.validate()?;
            return Ok(state);
        }
        let path = self.trace_dir.join("state.json");
        let file =
            File::open(&path).map_err(|error| MasterCoordinationError::Store(error.to_string()))?;
        let metadata = file
            .metadata()
            .map_err(|error| MasterCoordinationError::Store(error.to_string()))?;
        if metadata.len() > MAX_STATE_BYTES {
            return Err(MasterCoordinationError::Store(
                "master state exceeds the size limit".into(),
            ));
        }
        let state: MasterState = serde_json::from_reader(file)
            .map_err(|error| MasterCoordinationError::Store(error.to_string()))?;
        state.validate()?;
        Ok(state)
    }

    fn save_state(&self, state: &MasterState) -> Result<(), MasterCoordinationError> {
        create_private_dir(&self.trace_dir)?;
        let path = self.trace_dir.join("state.json");
        let temporary = self
            .trace_dir
            .join(format!(".state.{}.tmp", std::process::id()));
        let bytes = serde_json::to_vec_pretty(state)
            .map_err(|error| MasterCoordinationError::Store(error.to_string()))?;
        let mut file = private_file(&temporary)?;
        file.write_all(&bytes)
            .and_then(|_| file.write_all(b"\n"))
            .and_then(|_| file.sync_all())
            .map_err(|error| MasterCoordinationError::Store(error.to_string()))?;
        std::fs::rename(&temporary, &path)
            .map_err(|error| MasterCoordinationError::Store(error.to_string()))
    }

    fn append_event(&self, mut event: MasterJournalEvent) -> Result<(), MasterCoordinationError> {
        create_private_dir(&self.trace_dir)?;
        let path = self.trace_dir.join("events.jsonl");
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&path)
            .map_err(|error| MasterCoordinationError::Store(error.to_string()))?;
        lock_exclusive(&file)?;
        let previous = last_event_metadata(&path)?;
        if event.seq == 1 {
            if previous.is_some() {
                return Err(MasterCoordinationError::Store(
                    "refusing to overwrite an existing master journal".into(),
                ));
            }
        } else if previous.as_ref().map(|(seq, _)| *seq) != Some(event.seq - 1) {
            return Err(MasterCoordinationError::Store(
                "master journal changed during update; reload state before retrying".into(),
            ));
        }
        event.previous_event_sha256 = previous.map(|(_, hash)| hash);
        event.event_sha256 = event_sha256(&event)?;
        let bytes = serde_json::to_vec(&event)
            .map_err(|error| MasterCoordinationError::Store(error.to_string()))?;
        if bytes.len() > MAX_JOURNAL_EVENT_BYTES {
            return Err(MasterCoordinationError::Store(
                "master journal event exceeds the size limit".into(),
            ));
        }
        file.write_all(&bytes)
            .and_then(|_| file.write_all(b"\n"))
            .and_then(|_| file.sync_data())
            .map_err(|error| MasterCoordinationError::Store(error.to_string()))?;
        // Observability projection: the journal above is the authority. A failed
        // host-event append must not roll back or fail the task transition.
        let _ = crate::host_events::emit(&event);
        Ok(())
    }
}

fn lock_exclusive(file: &File) -> Result<(), MasterCoordinationError> {
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
        if result != 0 {
            return Err(MasterCoordinationError::Store(format!(
                "cannot lock master journal: {}",
                std::io::Error::last_os_error()
            )));
        }
    }
    #[cfg(not(unix))]
    let _ = file;
    Ok(())
}

fn journal_event(
    state: &MasterState,
    seq: u64,
    kind: MasterJournalEventKind,
    task_id: Option<String>,
    actor: impl Into<String>,
    reason: impl Into<String>,
    now_unix_ms: u128,
) -> MasterJournalEvent {
    MasterJournalEvent {
        schema_version: MASTER_JOURNAL_SCHEMA_VERSION,
        seq,
        trace_id: state.trace_id.clone(),
        plan_id: state.plan.plan_id.clone(),
        timestamp_unix_ms: now_unix_ms,
        kind,
        task_id,
        actor: actor.into(),
        reason: reason.into(),
        previous_event_sha256: None,
        event_sha256: String::new(),
        state: state.clone(),
    }
}

pub fn replay_master_journal(
    path: &Path,
) -> Result<(MasterState, MasterAudit), MasterCoordinationError> {
    let events = read_journal_events(path)?;
    let audit = audit_master_events(&events);
    let state = events
        .last()
        .map(|event| event.state.clone())
        .ok_or_else(|| MasterCoordinationError::JournalAudit("journal is empty".into()))?;
    Ok((state, audit))
}

pub fn read_master_journal_events(
    path: &Path,
) -> Result<Vec<MasterJournalEvent>, MasterCoordinationError> {
    read_journal_events(path)
}

fn event_sha256(event: &MasterJournalEvent) -> Result<String, MasterCoordinationError> {
    let mut copy = event.clone();
    copy.event_sha256.clear();
    let bytes = serde_json::to_vec(&copy)
        .map_err(|error| MasterCoordinationError::Store(error.to_string()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn last_event_metadata(path: &Path) -> Result<Option<(u64, String)>, MasterCoordinationError> {
    if !path.exists() {
        return Ok(None);
    }
    Ok(read_journal_events(path)?
        .last()
        .map(|event| (event.seq, event.event_sha256.clone())))
}

fn read_journal_events(path: &Path) -> Result<Vec<MasterJournalEvent>, MasterCoordinationError> {
    let file =
        File::open(path).map_err(|error| MasterCoordinationError::Store(error.to_string()))?;
    let mut reader = BufReader::new(file);
    let mut events = Vec::new();
    let mut pending = Vec::new();
    loop {
        let available = reader
            .fill_buf()
            .map_err(|error| MasterCoordinationError::Store(error.to_string()))?;
        if available.is_empty() {
            break;
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let take = newline.map_or(available.len(), |index| index + 1);
        if pending.len().saturating_add(take) > MAX_JOURNAL_EVENT_BYTES + 1 {
            return Err(MasterCoordinationError::JournalAudit(
                "journal event exceeds the size limit".into(),
            ));
        }
        pending.extend_from_slice(&available[..take]);
        reader.consume(take);
        if newline.is_some() {
            parse_journal_line(&pending, &mut events)?;
            pending.clear();
        }
    }
    parse_journal_line(&pending, &mut events)?;
    Ok(events)
}

fn parse_journal_line(
    bytes: &[u8],
    events: &mut Vec<MasterJournalEvent>,
) -> Result<(), MasterCoordinationError> {
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(());
    }
    if bytes.len() > MAX_JOURNAL_EVENT_BYTES {
        return Err(MasterCoordinationError::JournalAudit(
            "journal event exceeds the size limit".into(),
        ));
    }
    events.push(
        serde_json::from_slice(bytes)
            .map_err(|error| MasterCoordinationError::Store(error.to_string()))?,
    );
    Ok(())
}

pub fn audit_master_events(events: &[MasterJournalEvent]) -> MasterAudit {
    let mut issues = Vec::new();
    if events.is_empty() {
        issues.push("journal is empty".into());
    }
    let mut previous: Option<&MasterJournalEvent> = None;
    for (index, event) in events.iter().enumerate() {
        let expected_seq = index as u64 + 1;
        if event.schema_version != MASTER_JOURNAL_SCHEMA_VERSION {
            issues.push(format!("event {expected_seq}: unsupported schema version"));
        }
        if event.seq != expected_seq {
            issues.push(format!(
                "event at line {} has seq {}, expected {expected_seq}",
                index + 1,
                event.seq
            ));
        }
        if event.state.event_seq != event.seq {
            issues.push(format!("event {} state seq does not match", event.seq));
        }
        if event.state.trace_id != event.trace_id || event.state.plan.plan_id != event.plan_id {
            issues.push(format!(
                "event {} identity does not match its state",
                event.seq
            ));
        }
        if let Err(error) = event.state.validate() {
            issues.push(format!(
                "event {} contains invalid state: {error}",
                event.seq
            ));
        }
        let expected_previous = previous.and_then(|event| {
            (!event.event_sha256.is_empty()).then_some(event.event_sha256.as_str())
        });
        if event.previous_event_sha256.as_deref() != expected_previous {
            issues.push(format!("event {} journal hash chain is broken", event.seq));
        }
        match event_sha256(event) {
            Ok(digest) if digest == event.event_sha256 => {}
            Ok(_) => issues.push(format!("event {} content hash mismatch", event.seq)),
            Err(error) => issues.push(format!(
                "event {} content hash could not be computed: {error}",
                event.seq
            )),
        }
        if index == 0 && event.kind != MasterJournalEventKind::Initialized {
            issues.push("first journal event is not Initialized".into());
        }
        if let Some(previous) = previous {
            if event.timestamp_unix_ms < previous.timestamp_unix_ms {
                issues.push(format!("event {} timestamp moved backwards", event.seq));
            }
            for (task_id, task) in &event.state.tasks {
                let Some(before) = previous.state.tasks.get(task_id) else {
                    continue;
                };
                if before.status != task.status
                    && !before.status.can_transition_to(task.status)
                    && !matches!(
                        (before.status, task.status),
                        (WorkItemStatus::Assigned, WorkItemStatus::Running)
                            | (WorkItemStatus::Starting, WorkItemStatus::ArtifactReady)
                            | (WorkItemStatus::Starting, WorkItemStatus::Rejected)
                            | (WorkItemStatus::Starting, WorkItemStatus::Blocked)
                            | (WorkItemStatus::Starting, WorkItemStatus::Cancelled)
                            | (WorkItemStatus::ArtifactReady, WorkItemStatus::Verified)
                            | (WorkItemStatus::ArtifactReady, WorkItemStatus::Rejected)
                            | (WorkItemStatus::AwaitingInput, WorkItemStatus::Running)
                            | (WorkItemStatus::Blocked, WorkItemStatus::Running)
                            | (WorkItemStatus::AwaitingInput, WorkItemStatus::ArtifactReady)
                            | (WorkItemStatus::AwaitingInput, WorkItemStatus::Rejected)
                            | (WorkItemStatus::Blocked, WorkItemStatus::ArtifactReady)
                            | (WorkItemStatus::Blocked, WorkItemStatus::Rejected)
                    )
                {
                    issues.push(format!(
                        "event {}: task {task_id} transitioned {:?} -> {:?}",
                        event.seq, before.status, task.status
                    ));
                }
            }
            if event.state.replan_count < previous.state.replan_count
                || event.state.retry_count < previous.state.retry_count
            {
                issues.push(format!("event {} reduced a bounded counter", event.seq));
            }
        }
        previous = Some(event);
    }
    MasterAudit {
        ok: issues.is_empty(),
        event_count: events.len(),
        issues,
    }
}

fn validate_component(value: &str) -> Result<(), MasterCoordinationError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(MasterCoordinationError::InvalidTraceId);
    }
    Ok(())
}

fn create_private_dir(path: &Path) -> Result<(), MasterCoordinationError> {
    std::fs::create_dir_all(path)
        .map_err(|error| MasterCoordinationError::Store(error.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .map_err(|error| MasterCoordinationError::Store(error.to_string()))?;
    }
    Ok(())
}

fn private_file(path: &Path) -> Result<File, MasterCoordinationError> {
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .map_err(|error| MasterCoordinationError::Store(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::PermissionSet;
    use crate::domain::RiskLevel;
    use crate::master::{
        AgentAvailability, AssignmentCandidate, ExecutionStatus, MASTER_PLAN_SCHEMA_VERSION,
        WorkBudget,
    };
    use tempfile::tempdir;

    fn item(id: &str, dependencies: &[&str]) -> WorkItem {
        WorkItem {
            id: id.into(),
            title: format!("Task {id}"),
            objective: format!("Complete {id}"),
            dependencies: dependencies.iter().map(|value| (*value).into()).collect(),
            required_capabilities: vec!["rust".into()],
            required_tools: vec!["shell".into()],
            permissions: PermissionSet::default(),
            acceptance_criteria: vec!["artifact exists".into()],
            expected_artifacts: vec![format!("{id}.txt")],
            workspace: PathBuf::from("/tmp"),
            budget: WorkBudget::default(),
            deadline_unix_ms: None,
            risk: RiskLevel::Low,
            status: WorkItemStatus::Pending,
        }
    }

    fn plan() -> PlanProposal {
        PlanProposal {
            schema_version: MASTER_PLAN_SCHEMA_VERSION,
            plan_id: "plan-1".into(),
            goal_id: "goal-1".into(),
            version: 1,
            summary: "Test".into(),
            work_items: vec![item("a", &[]), item("b", &["a"])],
            max_parallel: 2,
            approved: false,
        }
    }

    fn assignment() -> AssignmentProposal {
        AssignmentProposal {
            schema_version: MASTER_PLAN_SCHEMA_VERSION,
            plan_id: "plan-1".into(),
            assignments: vec![
                AssignmentCandidate {
                    task_id: "a".into(),
                    agent_id: "a".into(),
                    runtime: "pi".into(),
                    selection_reason: "test".into(),
                },
                AssignmentCandidate {
                    task_id: "b".into(),
                    agent_id: "b".into(),
                    runtime: "pi".into(),
                    selection_reason: "test".into(),
                },
            ],
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

    fn ready_state() -> MasterState {
        let mut state = MasterState::initialize("trace-1", plan(), assignment(), 1).unwrap();
        state.approve("user", 2).unwrap();
        state
    }

    fn receipt(task: &str, generation: u64, key: &str) -> ExecutionReceipt {
        ExecutionReceipt {
            task_id: task.into(),
            agent_id: "worker".into(),
            lease_generation: generation,
            idempotency_key: key.into(),
            execution_target: ExecutionTarget {
                session_id: Some("term-test".into()),
                pane_id: Some(3),
                job_id: None,
            },
            status: ExecutionStatus::Completed,
            exit_code: Some(0),
            artifacts: vec![format!("{task}.txt")],
            result_digest: None,
            error: None,
            started_unix_ms: 10,
            finished_unix_ms: 11,
        }
    }

    #[test]
    fn lease_receipt_and_verification_unlock_dependencies() {
        let workspace = tempdir().unwrap();
        std::fs::write(workspace.path().join("a.txt"), "artifact").unwrap();
        let mut state = ready_state();
        state.plan.work_items[0].workspace = workspace.path().into();
        let lease = state
            .lease_task("a", &agent(), lease_key("a", 1), 1_000, 3)
            .unwrap();
        state.begin_starting("a", lease.generation, 4).unwrap();
        state
            .record_receipt(receipt("a", lease.generation, &lease_key("a", 1)), 5)
            .unwrap();
        let checks = inspect_artifacts(
            workspace.path(),
            &[ArtifactExpectation {
                path: "a.txt".into(),
                expected_sha256: None,
            }],
        )
        .unwrap();
        let result = state
            .verify(
                VerificationInput {
                    task_id: "a".into(),
                    lease_generation: lease.generation,
                    criteria_results: vec![CriterionResult {
                        criterion: "artifact exists".into(),
                        passed: true,
                        evidence: vec!["sha256 verified".into()],
                    }],
                    artifact_checks: checks,
                    test_results: Vec::new(),
                    evidence: vec!["independent artifact hash".into()],
                    verified_by: "verifier".into(),
                    verified_at_unix_ms: 6,
                },
                6,
            )
            .unwrap();
        assert_eq!(result.status, WorkItemStatus::Verified);
        assert_eq!(state.tasks["a"].status, WorkItemStatus::Verified);
        assert_eq!(state.tasks["b"].status, WorkItemStatus::Ready);
    }

    #[test]
    fn async_submission_and_observations_keep_the_lease_until_terminal() {
        let mut state = ready_state();
        let lease = state
            .lease_task("a", &agent(), lease_key("a", 1), 1_000, 3)
            .unwrap();
        state.begin_starting("a", lease.generation, 4).unwrap();
        state
            .record_submission(
                ExecutionSubmission {
                    task_id: "a".into(),
                    agent_id: "worker".into(),
                    lease_generation: lease.generation,
                    idempotency_key: lease.idempotency_key.clone(),
                    execution_target: ExecutionTarget {
                        session_id: Some("term-async".into()),
                        pane_id: None,
                        job_id: None,
                    },
                    submitted_unix_ms: 5,
                    deadline_unix_ms: 1_000,
                },
                5,
            )
            .unwrap();
        assert_eq!(state.tasks["a"].status, WorkItemStatus::Running);
        assert!(state.tasks["a"].active_lease.is_some());
        state
            .record_observation(
                "a",
                ExecutionObservation::AwaitingInput {
                    reason: "approval requested in the Agent session".into(),
                },
                6,
            )
            .unwrap();
        assert_eq!(state.tasks["a"].status, WorkItemStatus::AwaitingInput);
        state
            .record_observation(
                "a",
                ExecutionObservation::Finished {
                    receipt: Box::new(receipt("a", lease.generation, &lease_key("a", 1))),
                },
                7,
            )
            .unwrap();
        assert_eq!(state.tasks["a"].status, WorkItemStatus::ArtifactReady);
        assert!(state.tasks["a"].active_lease.is_none());
        assert_eq!(
            state.tasks["a"]
                .execution_target
                .as_ref()
                .and_then(|target| target.session_id.as_deref()),
            Some("term-async")
        );
    }

    #[test]
    fn duplicate_receipt_retry_and_replan_are_bounded() {
        let mut state = ready_state();
        let lease = state
            .lease_task("a", &agent(), lease_key("a", 1), 1_000, 3)
            .unwrap();
        state.begin_starting("a", lease.generation, 4).unwrap();
        let mut failed = receipt("a", lease.generation, &lease_key("a", 1));
        failed.status = ExecutionStatus::Failed;
        failed.exit_code = Some(1);
        failed.error = Some("test failed".into());
        state.record_receipt(failed.clone(), 5).unwrap();
        assert_eq!(
            state.record_receipt(failed, 5),
            Err(MasterCoordinationError::DuplicateReceipt)
        );
        state
            .request_retry("a", "master", "one bounded retry", 6)
            .unwrap();
        assert_eq!(
            state.request_retry("a", "master", "second retry", 7),
            Err(MasterCoordinationError::RetryLimit("a".into()))
        );
        state.tasks.get_mut("a").unwrap().status = WorkItemStatus::Rejected;
        assert_eq!(
            state.request_retry("a", "master", "third retry", 8),
            Err(MasterCoordinationError::RetryLimit("a".into()))
        );
        state
            .request_replan("a", "master", "replace failed task", 9)
            .unwrap();
        state.tasks.get_mut("a").unwrap().status = WorkItemStatus::Rejected;
        assert_eq!(
            state.request_replan("a", "master", "again", 10),
            Err(MasterCoordinationError::ReplanLimit)
        );
    }

    #[test]
    fn journal_replay_restores_the_latest_state_and_audits_sequence() {
        let root = tempdir().unwrap();
        let store = MasterStore::new(root.path(), "trace-1").unwrap();
        let state = ready_state();
        store.initialize(state, 3).unwrap();
        let state = store.load().unwrap();
        let state = store
            .append(
                state,
                MasterJournalEventKind::Paused,
                None,
                "user",
                "pause",
                4,
            )
            .unwrap();
        let replayed = store.load().unwrap();
        assert_eq!(replayed, state);
        let (_, audit) = replay_master_journal(&store.trace_dir.join("events.jsonl")).unwrap();
        assert!(audit.ok, "{:?}", audit.issues);
        assert_eq!(audit.event_count, 2);

        let line = std::fs::read_to_string(store.trace_dir.join("events.jsonl")).unwrap();
        let mut events = line
            .lines()
            .map(|line| serde_json::from_str::<MasterJournalEvent>(line).unwrap())
            .collect::<Vec<_>>();
        events[1].reason = "silently rewritten".into();
        let tampered = audit_master_events(&events);
        assert!(!tampered.ok);
        assert!(
            tampered
                .issues
                .iter()
                .any(|issue| issue.contains("content hash mismatch"))
        );
    }

    #[test]
    fn missing_or_tampered_artifacts_cannot_be_verified_and_expired_leases_do_not_duplicate() {
        let workspace = tempdir().unwrap();
        std::fs::write(workspace.path().join("a.txt"), "original").unwrap();
        let expected = sha256_file(&workspace.path().join("a.txt")).unwrap();
        let mut state = ready_state();
        state.plan.work_items[0].workspace = workspace.path().into();
        state.plan.work_items[0].expected_artifacts = vec![format!("a.txt@sha256:{expected}")];
        let lease = state
            .lease_task("a", &agent(), lease_key("a", 1), 100, 3)
            .unwrap();
        assert_eq!(
            state.lease_task("a", &agent(), lease_key("a", 2), 100, 4),
            Err(MasterCoordinationError::ActiveLease("a".into()))
        );
        state.begin_starting("a", lease.generation, 4).unwrap();
        state
            .record_receipt(receipt("a", lease.generation, &lease_key("a", 1)), 5)
            .unwrap();
        std::fs::write(workspace.path().join("a.txt"), "tampered").unwrap();
        let checks = inspect_artifacts(
            workspace.path(),
            &[ArtifactExpectation {
                path: "a.txt".into(),
                expected_sha256: Some(expected),
            }],
        )
        .unwrap();
        let result = state
            .verify(
                VerificationInput {
                    task_id: "a".into(),
                    lease_generation: lease.generation,
                    criteria_results: vec![CriterionResult {
                        criterion: "artifact exists".into(),
                        passed: true,
                        evidence: vec!["hash checked".into()],
                    }],
                    artifact_checks: checks,
                    test_results: Vec::new(),
                    evidence: vec!["independent hash check".into()],
                    verified_by: "verifier".into(),
                    verified_at_unix_ms: 6,
                },
                6,
            )
            .unwrap();
        assert_eq!(result.status, WorkItemStatus::Rejected);
        assert!(
            result
                .failure_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("SHA-256 mismatch"))
        );
    }

    #[test]
    fn expired_and_stale_leases_fail_closed_without_silent_replacement() {
        let mut state = ready_state();
        let lease = state
            .lease_task("a", &agent(), lease_key("a", 1), 100, 3)
            .unwrap();

        // Before expiry, a second lease is rejected.
        assert_eq!(
            state.lease_task("a", &agent(), lease_key("a", 2), 100, 50),
            Err(MasterCoordinationError::ActiveLease("a".into()))
        );

        // After expiry, the old lease cannot start and no new lease is silently
        // granted; the current contract has no explicit reclaim path yet.
        assert_eq!(
            state.begin_starting("a", lease.generation, 200),
            Err(MasterCoordinationError::StaleReceipt)
        );
        assert_eq!(
            state.lease_task("a", &agent(), lease_key("a", 2), 100, 200),
            Err(MasterCoordinationError::ActiveLease("a".into()))
        );

        // Stale generation cannot renew or finish.
        assert_eq!(
            state.renew_lease("a", lease.generation + 1, 300, 200),
            Err(MasterCoordinationError::StaleReceipt)
        );
        assert_eq!(
            state.record_receipt(receipt("a", lease.generation, &lease_key("a", 1)), 200),
            Err(MasterCoordinationError::StaleReceipt)
        );
    }

    #[test]
    fn goal_acceptance_requires_every_task_verified_and_is_idempotent() {
        let mut state = ready_state();
        assert!(matches!(
            state.accept_goal("user", "accept", 3),
            Err(MasterCoordinationError::GoalNotVerified(_))
        ));
        for task in state.tasks.values_mut() {
            task.status = WorkItemStatus::Verified;
        }
        state.accept_goal("user", "accept", 4).unwrap();
        assert!(state.goal_accepted);
        assert_eq!(
            state.accept_goal("user", "again", 5),
            Err(MasterCoordinationError::GoalAlreadyAccepted)
        );
    }

    #[test]
    fn takeover_can_be_explicitly_returned_to_the_master() {
        let mut state = ready_state();
        state
            .lease_task("a", &agent(), lease_key("a", 1), 1_000, 3)
            .unwrap();
        state.take_over("a", "user", "inspect", 4).unwrap();
        assert!(state.tasks["a"].human_takeover);
        state.end_takeover("a", "user", "done", 5).unwrap();
        assert!(!state.tasks["a"].human_takeover);
        assert_eq!(
            state.end_takeover("a", "user", "again", 6),
            Err(MasterCoordinationError::NotUnderTakeover("a".into()))
        );
    }

    #[test]
    fn expired_lease_can_be_reclaimed_and_replaced_with_monotonic_generation() {
        let mut state = ready_state();
        let lease = state
            .lease_task("a", &agent(), lease_key("a", 1), 100, 3)
            .unwrap();
        assert_eq!(
            state.reclaim_expired_lease("a", "master", "still active", true, 50),
            Err(MasterCoordinationError::LeaseNotExpired("a".into()))
        );
        assert_eq!(
            state.reclaim_expired_lease("a", "master", "not stopped", false, 200),
            Err(MasterCoordinationError::ExecutionNotTerminated("a".into()))
        );
        state
            .reclaim_expired_lease("a", "master", "lease expired", true, 200)
            .unwrap();
        assert!(state.tasks["a"].active_lease.is_none());
        assert_eq!(state.tasks["a"].status, WorkItemStatus::Retrying);

        let replacement = state
            .lease_task("a", &agent(), lease_key("a", 2), 100, 201)
            .unwrap();
        assert_eq!(replacement.generation, lease.generation + 1);
    }

    #[test]
    fn host_event_projection_writes_a_typed_goal_event() {
        let root = tempdir().unwrap();
        let state = ready_state();
        let event = journal_event(
            &state,
            1,
            MasterJournalEventKind::Initialized,
            None,
            "test",
            "init",
            3,
        );
        crate::host_events::emit_to(&root.path().join("events"), &event).unwrap();
        let raw = std::fs::read_to_string(root.path().join("events/host-events.jsonl")).unwrap();
        let value: serde_json::Value = serde_json::from_str(raw.lines().next().unwrap()).unwrap();
        assert_eq!(value["schema"], "skynet.term.host-event/v1");
        assert_eq!(value["event"], "goal.proposed");
        assert_eq!(value["goal_id"], state.plan.goal_id);
        assert!(value["conversation_id"].as_str().is_some());
    }

    #[test]
    fn concurrent_stale_writers_cannot_create_two_events_with_the_same_sequence() {
        let root = tempdir().unwrap();
        let store = MasterStore::new(root.path(), "trace-1").unwrap();
        store.initialize(ready_state(), 3).unwrap();
        let first_state = store.load().unwrap();
        let second_state = first_state.clone();
        let first_store = store.clone();
        let second_store = store.clone();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let first_barrier = barrier.clone();
        let first = std::thread::spawn(move || {
            first_barrier.wait();
            first_store.append(
                first_state,
                MasterJournalEventKind::Paused,
                None,
                "one",
                "pause",
                4,
            )
        });
        let second = std::thread::spawn(move || {
            barrier.wait();
            second_store.append(
                second_state,
                MasterJournalEventKind::Resumed,
                None,
                "two",
                "resume",
                4,
            )
        });
        let results = [first.join().unwrap(), second.join().unwrap()];
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(results.iter().filter(|result| result.is_err()).count(), 1);
        let (_, audit) = replay_master_journal(&store.trace_dir.join("events.jsonl")).unwrap();
        assert!(audit.ok, "{:?}", audit.issues);
        assert_eq!(audit.event_count, 2);
    }

    #[test]
    fn unstarted_plan_can_be_rejected_or_revised_without_losing_old_journal_state() {
        let rejected = MasterState::initialize("trace-reject", plan(), assignment(), 1).unwrap();
        let mut rejected = rejected;
        rejected
            .reject_plan("user", "requirements changed", 2)
            .unwrap();
        assert!(rejected.plan_rejected);
        assert!(
            rejected
                .tasks
                .values()
                .all(|task| task.status == WorkItemStatus::Cancelled)
        );
        assert_eq!(
            rejected.approve("user", 3),
            Err(MasterCoordinationError::PlanRejected)
        );

        let mut revised = MasterState::initialize("trace-revise", plan(), assignment(), 1).unwrap();
        let mut replacement = plan();
        replacement.version = 2;
        replacement.summary = "Revised".into();
        let replacement_assignment = replacement.assign(&[agent()]).unwrap();
        revised
            .revise_plan(
                replacement,
                replacement_assignment,
                "user",
                "clarify acceptance",
                2,
            )
            .unwrap();
        assert_eq!(revised.plan.version, 2);
        assert_eq!(revised.tasks["a"].status, WorkItemStatus::Ready);
        assert!(
            revised
                .decisions
                .iter()
                .any(|decision| decision.kind == DecisionKind::PlanRevised)
        );

        let mut preapproved = plan();
        preapproved.approved = true;
        assert_eq!(
            MasterState::initialize("trace-preapproved", preapproved, assignment(), 1),
            Err(MasterCoordinationError::PlanMustInitializeUnapproved)
        );
    }

    fn lease_key(task: &str, generation: u64) -> String {
        format!("plan-1:{task}:{generation}")
    }
}
