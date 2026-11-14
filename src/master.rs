use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::domain::{PermissionSet, RiskLevel};

pub const MASTER_PLAN_SCHEMA_VERSION: u32 = 1;
pub const MAX_WORK_ITEMS: usize = 8;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkItemStatus {
    #[default]
    Pending,
    Ready,
    Assigned,
    Starting,
    Running,
    Blocked,
    AwaitingInput,
    ArtifactReady,
    Verifying,
    Verified,
    Rejected,
    Retrying,
    Replanning,
    Cancelled,
}

impl WorkItemStatus {
    pub fn can_transition_to(self, next: Self) -> bool {
        use WorkItemStatus::*;
        matches!(
            (self, next),
            (Pending, Ready | Cancelled)
                | (Ready, Assigned | Cancelled)
                | (Assigned, Starting | Retrying | Cancelled)
                | (
                    Starting,
                    Running | Blocked | Rejected | Retrying | Cancelled
                )
                | (
                    Running,
                    AwaitingInput | ArtifactReady | Blocked | Rejected | Retrying | Cancelled
                )
                | (AwaitingInput, Running | Blocked | Retrying | Cancelled)
                | (Blocked, Running | Retrying | Replanning | Cancelled)
                | (ArtifactReady, Verifying | Rejected)
                | (Verifying, Verified | Rejected | Replanning)
                | (Retrying, Assigned | Starting | Replanning | Cancelled)
                | (Replanning, Ready | Cancelled)
                | (Rejected, Retrying | Replanning | Cancelled)
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkBudget {
    #[serde(default = "default_work_steps")]
    pub max_steps: usize,
    #[serde(default = "default_work_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_work_tool_calls")]
    pub max_tool_calls: usize,
}

impl Default for WorkBudget {
    fn default() -> Self {
        Self {
            max_steps: default_work_steps(),
            timeout_ms: default_work_timeout_ms(),
            max_tool_calls: default_work_tool_calls(),
        }
    }
}

fn default_work_steps() -> usize {
    6
}

fn default_work_timeout_ms() -> u64 {
    120_000
}

fn default_work_tool_calls() -> usize {
    8
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkItem {
    pub id: String,
    pub title: String,
    pub objective: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    #[serde(default)]
    pub required_tools: Vec<String>,
    #[serde(default)]
    pub permissions: PermissionSet,
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
    #[serde(default)]
    pub expected_artifacts: Vec<String>,
    pub workspace: PathBuf,
    #[serde(default)]
    pub budget: WorkBudget,
    #[serde(default)]
    pub deadline_unix_ms: Option<u128>,
    #[serde(default = "default_low_risk")]
    pub risk: RiskLevel,
    #[serde(default)]
    pub status: WorkItemStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanProposal {
    #[serde(default = "default_plan_schema_version")]
    pub schema_version: u32,
    pub plan_id: String,
    pub goal_id: String,
    #[serde(default = "default_plan_version")]
    pub version: u32,
    pub summary: String,
    pub work_items: Vec<WorkItem>,
    #[serde(default = "default_max_parallel")]
    pub max_parallel: usize,
    #[serde(default)]
    pub approved: bool,
}

fn default_plan_schema_version() -> u32 {
    MASTER_PLAN_SCHEMA_VERSION
}

fn default_plan_version() -> u32 {
    1
}

fn default_max_parallel() -> usize {
    3
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MasterPlanError {
    #[error("unsupported master plan schema version {0}")]
    UnsupportedVersion(u32),
    #[error("plan id and goal id must not be empty")]
    EmptyIdentity,
    #[error("plan must contain between 1 and {MAX_WORK_ITEMS} work items")]
    InvalidTaskCount,
    #[error("work item id '{0}' is empty or unsafe")]
    InvalidTaskId(String),
    #[error("work item id '{0}' is duplicated")]
    DuplicateTaskId(String),
    #[error("work item '{0}' has an empty title or objective")]
    EmptyTaskDescription(String),
    #[error("work item '{0}' title or objective contains control characters")]
    UnsafeTaskDescription(String),
    #[error("work item '{0}' exceeds a bounded field limit")]
    TaskFieldTooLarge(String),
    #[error("work item '{0}' has no required capability")]
    MissingCapability(String),
    #[error("work item '{0}' has no acceptance criterion")]
    MissingAcceptanceCriterion(String),
    #[error("work item '{0}' has no expected artifact")]
    MissingExpectedArtifact(String),
    #[error("work item '{task}' has invalid artifact spec '{spec}': {reason}")]
    InvalidArtifactSpec {
        task: String,
        spec: String,
        reason: String,
    },
    #[error("work item '{0}' contains a blank capability, tool, criterion or artifact")]
    BlankTaskRequirement(String),
    #[error("work item '{0}' workspace must be absolute")]
    WorkspaceNotAbsolute(String),
    #[error("work item '{0}' read root is outside its workspace")]
    ReadRootOutsideWorkspace(String),
    #[error("work item '{0}' depends on itself")]
    SelfDependency(String),
    #[error("work item '{task}' depends on unknown work item '{dependency}'")]
    UnknownDependency { task: String, dependency: String },
    #[error("work item dependency graph contains a cycle")]
    DependencyCycle,
    #[error("work item budgets must be non-zero")]
    InvalidBudget,
    #[error("max_parallel must be greater than zero")]
    InvalidParallelism,
    #[error("Agent profile set is empty or exceeds the bounded assignment limit")]
    InvalidAgentCount,
    #[error("assignment exceeds the three-runtime V1 limit")]
    TooManyRuntimeTypes,
    #[error("work item '{0}' has no capable idle Agent")]
    NoCapableAgent(String),
}

impl PlanProposal {
    pub fn validate(&self) -> Result<(), MasterPlanError> {
        if self.schema_version != MASTER_PLAN_SCHEMA_VERSION {
            return Err(MasterPlanError::UnsupportedVersion(self.schema_version));
        }
        if self.plan_id.trim().is_empty() || self.goal_id.trim().is_empty() {
            return Err(MasterPlanError::EmptyIdentity);
        }
        if self.plan_id.len() > 128
            || self.goal_id.len() > 128
            || self.summary.chars().count() > 2_000
        {
            return Err(MasterPlanError::TaskFieldTooLarge("plan".into()));
        }
        if self.work_items.is_empty() || self.work_items.len() > MAX_WORK_ITEMS {
            return Err(MasterPlanError::InvalidTaskCount);
        }
        if self.max_parallel == 0 || self.max_parallel > 3 {
            return Err(MasterPlanError::InvalidParallelism);
        }

        let mut ids = BTreeSet::new();
        for item in &self.work_items {
            if item.id.is_empty()
                || item.id.len() > 128
                || !item
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
            {
                return Err(MasterPlanError::InvalidTaskId(item.id.clone()));
            }
            if !ids.insert(item.id.as_str()) {
                return Err(MasterPlanError::DuplicateTaskId(item.id.clone()));
            }
            if item.title.trim().is_empty() || item.objective.trim().is_empty() {
                return Err(MasterPlanError::EmptyTaskDescription(item.id.clone()));
            }
            if item.title.chars().any(char::is_control)
                || item.objective.chars().any(char::is_control)
            {
                return Err(MasterPlanError::UnsafeTaskDescription(item.id.clone()));
            }
            if item.title.chars().count() > 256
                || item.objective.chars().count() > 8_192
                || item.dependencies.len() > MAX_WORK_ITEMS
                || item.required_capabilities.len() > 32
                || item.required_tools.len() > 32
                || item.acceptance_criteria.len() > 32
                || item.expected_artifacts.len() > 16
                || item.permissions.read_roots.len() > 32
                || item
                    .required_capabilities
                    .iter()
                    .chain(&item.required_tools)
                    .any(|value| value.len() > 128)
                || item
                    .acceptance_criteria
                    .iter()
                    .any(|value| value.chars().count() > 2_000)
                || item
                    .expected_artifacts
                    .iter()
                    .any(|value| value.len() > 512)
            {
                return Err(MasterPlanError::TaskFieldTooLarge(item.id.clone()));
            }
            if !item.workspace.is_absolute() {
                return Err(MasterPlanError::WorkspaceNotAbsolute(item.id.clone()));
            }
            if item
                .permissions
                .read_roots
                .iter()
                .any(|root| !root.is_absolute() || !root.starts_with(&item.workspace))
            {
                return Err(MasterPlanError::ReadRootOutsideWorkspace(item.id.clone()));
            }
            if item
                .required_capabilities
                .iter()
                .all(|cap| cap.trim().is_empty())
            {
                return Err(MasterPlanError::MissingCapability(item.id.clone()));
            }
            if item
                .required_capabilities
                .iter()
                .chain(&item.required_tools)
                .chain(&item.acceptance_criteria)
                .chain(&item.expected_artifacts)
                .any(|value| value.trim().is_empty())
            {
                return Err(MasterPlanError::BlankTaskRequirement(item.id.clone()));
            }
            if item
                .acceptance_criteria
                .iter()
                .all(|criterion| criterion.trim().is_empty())
            {
                return Err(MasterPlanError::MissingAcceptanceCriterion(item.id.clone()));
            }
            if item
                .expected_artifacts
                .iter()
                .all(|artifact| artifact.trim().is_empty())
            {
                return Err(MasterPlanError::MissingExpectedArtifact(item.id.clone()));
            }
            for artifact in &item.expected_artifacts {
                validate_artifact_spec(&item.id, artifact)?;
            }
            if item.budget.max_steps == 0
                || item.budget.timeout_ms == 0
                || item.budget.max_tool_calls == 0
            {
                return Err(MasterPlanError::InvalidBudget);
            }
            for dependency in &item.dependencies {
                if dependency == &item.id {
                    return Err(MasterPlanError::SelfDependency(item.id.clone()));
                }
                if !self
                    .work_items
                    .iter()
                    .any(|candidate| &candidate.id == dependency)
                {
                    return Err(MasterPlanError::UnknownDependency {
                        task: item.id.clone(),
                        dependency: dependency.clone(),
                    });
                }
            }
        }
        self.topological_order().map(|_| ())
    }

    pub fn topological_order(&self) -> Result<Vec<String>, MasterPlanError> {
        let mut remaining = self
            .work_items
            .iter()
            .map(|item| {
                (
                    item.id.as_str(),
                    item.dependencies
                        .iter()
                        .map(String::as_str)
                        .collect::<BTreeSet<_>>(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut ready = remaining
            .iter()
            .filter_map(|(id, dependencies)| dependencies.is_empty().then_some(*id))
            .collect::<BTreeSet<_>>();
        let mut order = Vec::with_capacity(remaining.len());
        while !ready.is_empty() {
            let id = *ready.iter().next().expect("ready is non-empty");
            ready.remove(id);
            remaining.remove(id);
            order.push(id.to_string());
            for (candidate, dependencies) in &mut remaining {
                dependencies.remove(id);
                if dependencies.is_empty() {
                    ready.insert(candidate);
                }
            }
        }
        if remaining.is_empty() {
            Ok(order)
        } else {
            Err(MasterPlanError::DependencyCycle)
        }
    }

    pub fn to_markdown(&self) -> String {
        let mut output = format!(
            "# {}\n\n{}\n\nPlan: `{}`  Goal: `{}`  Version: {}\n\n",
            self.summary, self.summary, self.plan_id, self.goal_id, self.version
        );
        let Ok(order) = self.topological_order() else {
            return format!("{output}Invalid dependency graph.\n");
        };
        for id in order {
            let item = self
                .work_items
                .iter()
                .find(|item| item.id == id)
                .expect("topological order references a work item");
            output.push_str(&format!(
                "## {} — {}\n\n{}\n\n- Capabilities: {}\n- Tools: {}\n- Depends on: {}\n- Acceptance: {}\n- Expected artifacts: {}\n- Workspace: `{}`\n- Risk: {:?}\n\n",
                item.id,
                item.title,
                item.objective,
                item.required_capabilities.join(", "),
                item.required_tools.join(", "),
                if item.dependencies.is_empty() {
                    "none".into()
                } else {
                    item.dependencies.join(", ")
                },
                item.acceptance_criteria.join("; "),
                item.expected_artifacts.join(", "),
                item.workspace.display(),
                item.risk
            ));
        }
        output
    }
}

fn validate_artifact_spec(task: &str, spec: &str) -> Result<(), MasterPlanError> {
    let path = spec
        .rsplit_once("@sha256:")
        .map(|(path, _)| path)
        .unwrap_or(spec);
    let path = std::path::Path::new(path);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || spec.chars().any(char::is_control)
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(MasterPlanError::InvalidArtifactSpec {
            task: task.into(),
            spec: spec.into(),
            reason: "artifact path must be a relative path without '..'".into(),
        });
    }
    if let Some((_, digest)) = spec.rsplit_once("@sha256:")
        && (digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return Err(MasterPlanError::InvalidArtifactSpec {
            task: task.into(),
            spec: spec.into(),
            reason: "SHA-256 must contain exactly 64 hexadecimal characters".into(),
        });
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentAvailability {
    #[default]
    Idle,
    Busy,
    Offline,
}

impl AgentAvailability {
    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Idle => "空闲",
            Self::Busy => "忙碌",
            Self::Offline => "离线",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentProfile {
    pub agent_id: String,
    pub runtime: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub runtime_command: Vec<String>,
    #[serde(default)]
    pub permissions: PermissionSet,
    #[serde(default)]
    pub availability: AgentAvailability,
    #[serde(default)]
    pub current_load: u32,
    #[serde(default)]
    pub observed_success_permille: Option<u16>,
    #[serde(default)]
    pub observed_runs: u32,
    #[serde(default)]
    pub cost_micros: Option<u64>,
    #[serde(default = "default_low_risk")]
    pub max_risk: RiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AssignmentCandidate {
    pub task_id: String,
    pub agent_id: String,
    pub runtime: String,
    pub selection_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AssignmentProposal {
    pub schema_version: u32,
    pub plan_id: String,
    pub assignments: Vec<AssignmentCandidate>,
}

impl PlanProposal {
    pub fn assign(&self, agents: &[AgentProfile]) -> Result<AssignmentProposal, MasterPlanError> {
        self.validate()?;
        if agents.is_empty() || agents.len() > 64 {
            return Err(MasterPlanError::InvalidAgentCount);
        }
        let mut assignments = Vec::new();
        let mut assigned_load = BTreeMap::<&str, u32>::new();
        for item in &self.work_items {
            let mut candidates = agents
                .iter()
                .filter(|agent| agent.availability == AgentAvailability::Idle)
                .filter(|agent| agent.max_risk >= item.risk)
                .filter(|agent| {
                    item.required_capabilities
                        .iter()
                        .all(|cap| agent.capabilities.iter().any(|candidate| candidate == cap))
                })
                .filter(|agent| {
                    item.required_tools
                        .iter()
                        .all(|tool| agent.tools.iter().any(|candidate| candidate == tool))
                })
                .filter(|agent| !item.permissions.allow_write || agent.permissions.allow_write)
                .collect::<Vec<_>>();
            candidates.sort_by(|left, right| {
                right
                    .observed_success_permille
                    .cmp(&left.observed_success_permille)
                    .then_with(|| {
                        (left.current_load
                            + assigned_load
                                .get(left.agent_id.as_str())
                                .copied()
                                .unwrap_or(0))
                        .cmp(
                            &(right.current_load
                                + assigned_load
                                    .get(right.agent_id.as_str())
                                    .copied()
                                    .unwrap_or(0)),
                        )
                    })
                    .then_with(|| {
                        left.cost_micros
                            .unwrap_or(u64::MAX)
                            .cmp(&right.cost_micros.unwrap_or(u64::MAX))
                    })
                    .then_with(|| left.agent_id.cmp(&right.agent_id))
            });
            let selected = candidates
                .first()
                .ok_or_else(|| MasterPlanError::NoCapableAgent(item.id.clone()))?;
            let effective_load = selected.current_load
                + assigned_load
                    .get(selected.agent_id.as_str())
                    .copied()
                    .unwrap_or(0);
            assignments.push(AssignmentCandidate {
                task_id: item.id.clone(),
                agent_id: selected.agent_id.clone(),
                runtime: selected.runtime.clone(),
                selection_reason: format!(
                    "capabilities and tools match; success={}; observed_runs={}; load={}; cost={}",
                    selected
                        .observed_success_permille
                        .map(|value| format!("{value}‰"))
                        .unwrap_or_else(|| "unmeasured".into()),
                    selected.observed_runs,
                    effective_load,
                    selected
                        .cost_micros
                        .map(|value| format!("{value}µ"))
                        .unwrap_or_else(|| "unmeasured".into())
                ),
            });
            *assigned_load.entry(selected.agent_id.as_str()).or_default() += 1;
        }
        if assignments
            .iter()
            .map(|assignment| assignment.runtime.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            > 3
        {
            return Err(MasterPlanError::TooManyRuntimeTypes);
        }
        Ok(AssignmentProposal {
            schema_version: MASTER_PLAN_SCHEMA_VERSION,
            plan_id: self.plan_id.clone(),
            assignments,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TaskLease {
    pub task_id: String,
    pub agent_id: String,
    pub execution_target: String,
    pub idempotency_key: String,
    pub expires_unix_ms: u128,
    pub generation: u64,
}

impl TaskLease {
    pub fn is_active(&self, now_unix_ms: u128) -> bool {
        now_unix_ms < self.expires_unix_ms
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VerificationResult {
    pub task_id: String,
    pub status: WorkItemStatus,
    pub checked_artifacts: Vec<ArtifactCheck>,
    pub criteria_results: Vec<CriterionResult>,
    pub test_results: Vec<TestResult>,
    pub evidence: Vec<String>,
    pub verified_by: String,
    pub verified_at_unix_ms: u128,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ArtifactCheck {
    pub path: String,
    pub exists: bool,
    pub regular_file: bool,
    pub bytes: u64,
    pub sha256: Option<String>,
    pub expected_sha256: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Completed,
    Failed,
    TimedOut,
    Cancelled,
    Interrupted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CriterionResult {
    pub criterion: String,
    pub passed: bool,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    pub evidence: Option<String>,
}

fn default_low_risk() -> RiskLevel {
    RiskLevel::Low
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, dependencies: &[&str]) -> WorkItem {
        WorkItem {
            id: id.into(),
            title: format!("Task {id}"),
            objective: format!("Complete {id}"),
            dependencies: dependencies.iter().map(|value| (*value).into()).collect(),
            required_capabilities: vec!["rust".into()],
            required_tools: vec!["shell".into()],
            permissions: PermissionSet::default(),
            acceptance_criteria: vec!["cargo test passes".into()],
            expected_artifacts: vec![format!("{id}.log")],
            workspace: PathBuf::from("/tmp"),
            budget: WorkBudget::default(),
            deadline_unix_ms: None,
            risk: RiskLevel::Low,
            status: WorkItemStatus::Pending,
        }
    }

    fn plan(items: Vec<WorkItem>) -> PlanProposal {
        PlanProposal {
            schema_version: MASTER_PLAN_SCHEMA_VERSION,
            plan_id: "plan-1".into(),
            goal_id: "goal-1".into(),
            version: 1,
            summary: "Test plan".into(),
            work_items: items,
            max_parallel: 2,
            approved: false,
        }
    }

    #[test]
    fn valid_plan_produces_deterministic_topological_order() {
        let plan = plan(vec![item("build", &[]), item("test", &["build"])]);
        plan.validate().unwrap();
        assert_eq!(plan.topological_order().unwrap(), ["build", "test"]);
        assert!(plan.to_markdown().contains("## test — Task test"));
    }

    #[test]
    fn cycles_and_missing_acceptance_data_fail_closed() {
        let mut first = item("a", &["b"]);
        let second = item("b", &["a"]);
        assert_eq!(
            plan(vec![first.clone(), second]).validate(),
            Err(MasterPlanError::DependencyCycle)
        );

        first.dependencies.clear();
        first.acceptance_criteria.clear();
        assert_eq!(
            plan(vec![first]).validate(),
            Err(MasterPlanError::MissingAcceptanceCriterion("a".into()))
        );
    }

    #[test]
    fn assignment_uses_hard_filters_then_measured_quality() {
        let plan = plan(vec![item("a", &[])]);
        let agents = vec![
            AgentProfile {
                agent_id: "busy".into(),
                runtime: "pi".into(),
                display_name: None,
                capabilities: vec!["rust".into()],
                tools: vec!["shell".into()],
                runtime_command: Vec::new(),
                permissions: PermissionSet::default(),
                availability: AgentAvailability::Busy,
                current_load: 0,
                observed_success_permille: Some(1_000),
                observed_runs: 10,
                cost_micros: Some(1),
                max_risk: RiskLevel::Low,
            },
            AgentProfile {
                agent_id: "slower".into(),
                runtime: "hermes".into(),
                display_name: None,
                capabilities: vec!["rust".into()],
                tools: vec!["shell".into()],
                runtime_command: Vec::new(),
                permissions: PermissionSet::default(),
                availability: AgentAvailability::Idle,
                current_load: 1,
                observed_success_permille: Some(700),
                observed_runs: 10,
                cost_micros: Some(5),
                max_risk: RiskLevel::Low,
            },
            AgentProfile {
                agent_id: "best".into(),
                runtime: "claude".into(),
                display_name: None,
                capabilities: vec!["rust".into()],
                tools: vec!["shell".into()],
                runtime_command: Vec::new(),
                permissions: PermissionSet::default(),
                availability: AgentAvailability::Idle,
                current_load: 2,
                observed_success_permille: Some(900),
                observed_runs: 10,
                cost_micros: Some(10),
                max_risk: RiskLevel::Low,
            },
        ];
        let assignment = plan.assign(&agents).unwrap();
        assert_eq!(assignment.assignments[0].task_id, "a");
        assert_eq!(assignment.assignments[0].agent_id, "best");
        assert!(
            assignment.assignments[0]
                .selection_reason
                .contains("success=900")
        );
    }

    #[test]
    fn assignment_accumulates_load_within_one_plan() {
        let plan = plan(vec![item("a", &[]), item("b", &[])]);
        let agents = vec![
            AgentProfile {
                agent_id: "agent-a".into(),
                runtime: "pi".into(),
                display_name: None,
                capabilities: vec!["rust".into()],
                tools: vec!["shell".into()],
                runtime_command: Vec::new(),
                permissions: PermissionSet::default(),
                availability: AgentAvailability::Idle,
                current_load: 0,
                observed_success_permille: Some(900),
                observed_runs: 10,
                cost_micros: Some(5),
                max_risk: RiskLevel::Low,
            },
            AgentProfile {
                agent_id: "agent-b".into(),
                runtime: "hermes".into(),
                display_name: None,
                capabilities: vec!["rust".into()],
                tools: vec!["shell".into()],
                runtime_command: Vec::new(),
                permissions: PermissionSet::default(),
                availability: AgentAvailability::Idle,
                current_load: 0,
                observed_success_permille: Some(900),
                observed_runs: 10,
                cost_micros: Some(5),
                max_risk: RiskLevel::Low,
            },
        ];
        let assignment = plan.assign(&agents).unwrap();
        assert_eq!(assignment.assignments[0].agent_id, "agent-a");
        assert_eq!(assignment.assignments[1].agent_id, "agent-b");
    }

    #[test]
    fn state_machine_and_lease_treat_expiry_as_inactive() {
        assert!(WorkItemStatus::Assigned.can_transition_to(WorkItemStatus::Starting));
        assert!(WorkItemStatus::Verifying.can_transition_to(WorkItemStatus::Verified));
        assert!(!WorkItemStatus::Verified.can_transition_to(WorkItemStatus::Running));

        let lease = TaskLease {
            task_id: "a".into(),
            agent_id: "best".into(),
            execution_target: "term-1".into(),
            idempotency_key: "plan-1:a:1".into(),
            expires_unix_ms: 100,
            generation: 1,
        };
        assert!(lease.is_active(99));
        assert!(!lease.is_active(100));
    }
}
