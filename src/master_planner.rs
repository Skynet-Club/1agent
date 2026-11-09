use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::domain::{
    AgentAnswer, CognitiveMode, ModelMessage, ModelRequest, ModelTurn, PermissionSet, RiskLevel,
};
use crate::master::{AgentProfile, MasterPlanError, PlanProposal};
use crate::production_line::{ProductionLineError, ProductionPlan};
use crate::provider::{ModelProvider, ProviderError};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkRoute {
    DirectAnswer,
    SingleAgent,
    MultiStep,
    MultiAgent,
    ApprovalRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TriageDecision {
    pub route: WorkRoute,
    pub reason: String,
    pub max_work_items: usize,
    pub requires_plan: bool,
    pub requires_user_approval: bool,
    pub allows_dispatch: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TriageSignals {
    #[serde(default)]
    pub requested_items: Option<usize>,
    #[serde(default)]
    pub requires_write: bool,
    #[serde(default)]
    pub requires_external_action: bool,
    #[serde(default = "default_triage_risk")]
    pub risk: RiskLevel,
}

impl Default for TriageSignals {
    fn default() -> Self {
        Self {
            requested_items: None,
            requires_write: false,
            requires_external_action: false,
            risk: RiskLevel::Low,
        }
    }
}

fn default_triage_risk() -> RiskLevel {
    RiskLevel::Low
}

pub fn triage_goal(goal: &str, signals: &TriageSignals) -> TriageDecision {
    let normalized = goal.to_lowercase();
    let chars = goal.chars().count();
    let multi_agent_markers = [
        "多个 agent",
        "多个agent",
        "多智能体",
        "并行执行",
        "分派给",
        "分配给",
        "不同窗体",
        "不同窗口",
        "master-agent",
        "multi-agent",
        "delegate",
        "parallel agents",
    ];
    let step_markers = [
        "先",
        "然后",
        "再",
        "接着",
        "依赖",
        "流水线",
        "work item",
        "任务拆解",
        "拆成",
        "pipeline",
        "first",
        "then",
    ];
    let action_markers = [
        "实现",
        "开发",
        "设计",
        "分析",
        "审计",
        "检查",
        "调研",
        "优化",
        "封装",
        "重构",
        "迁移",
        "清理",
        "整理",
        "生成",
        "下载",
        "上传",
        "打开",
        "读取",
        "修改",
        "修复",
        "运行",
        "测试",
        "创建",
        "写入",
        "删除",
        "提交",
        "部署",
        "implement",
        "design",
        "analyze",
        "audit",
        "inspect",
        "optimize",
        "refactor",
        "migrate",
        "download",
        "upload",
        "open",
        "read",
        "modify",
        "fix",
        "run",
        "test",
        "create",
        "write",
        "deploy",
    ];
    let multi_agent = multi_agent_markers
        .iter()
        .any(|marker| normalized.contains(marker))
        || signals.requested_items.is_some_and(|count| count > 1);
    let multi_step = step_markers
        .iter()
        .any(|marker| normalized.contains(marker));
    let has_action = action_markers
        .iter()
        .any(|marker| normalized.contains(marker));
    let asks_explanation = ["如何", "为什么", "是什么", "怎么", "what", "why", "how"]
        .iter()
        .any(|marker| normalized.starts_with(marker));
    let explicit_request = ["请你", "帮我", "给我", "需要你", "请直接", "please "]
        .iter()
        .any(|marker| normalized.contains(marker));
    let high_risk = signals.risk == RiskLevel::High
        || signals.requires_write
        || signals.requires_external_action;

    if asks_explanation && !explicit_request {
        return TriageDecision {
            route: WorkRoute::DirectAnswer,
            reason: "目标是解释或咨询问题，没有要求实际执行".into(),
            max_work_items: 0,
            requires_plan: false,
            requires_user_approval: false,
            allows_dispatch: false,
        };
    }
    if high_risk {
        return TriageDecision {
            route: WorkRoute::ApprovalRequired,
            reason: "任务包含写入、外部动作或高风险权限，必须先生成计划并人工批准".into(),
            max_work_items: 8,
            requires_plan: true,
            requires_user_approval: true,
            allows_dispatch: false,
        };
    }
    if multi_agent {
        return TriageDecision {
            route: WorkRoute::MultiAgent,
            reason: "目标明确要求多个执行者、窗体或并行协作".into(),
            max_work_items: 8,
            requires_plan: true,
            requires_user_approval: true,
            allows_dispatch: false,
        };
    }
    if multi_step {
        return TriageDecision {
            route: WorkRoute::MultiStep,
            reason: "目标包含可观察的顺序、依赖或多个交付步骤".into(),
            max_work_items: 8,
            requires_plan: true,
            requires_user_approval: true,
            allows_dispatch: false,
        };
    }
    if has_action || chars > 160 {
        return TriageDecision {
            route: WorkRoute::SingleAgent,
            reason: "目标需要实际交付，但未要求多个执行者".into(),
            max_work_items: 1,
            requires_plan: true,
            requires_user_approval: true,
            allows_dispatch: false,
        };
    }
    TriageDecision {
        route: WorkRoute::DirectAnswer,
        reason: "目标是一个无需外部动作的简短问题，可由主对话直接回答".into(),
        max_work_items: 0,
        requires_plan: false,
        requires_user_approval: false,
        allows_dispatch: false,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanningContext {
    pub goal: String,
    pub plan_id: String,
    pub goal_id: String,
    pub workspace: PathBuf,
    pub risk: RiskLevel,
    pub max_parallel: usize,
    pub max_work_items: usize,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub agents: Vec<AgentProfile>,
}

impl PlanningContext {
    pub fn new(
        goal: impl Into<String>,
        workspace: PathBuf,
        triage: &TriageDecision,
    ) -> Result<Self, MasterPlannerError> {
        let goal = goal.into();
        if goal.trim().is_empty() || goal.chars().any(char::is_control) {
            return Err(MasterPlannerError::InvalidGoal);
        }
        Ok(Self {
            goal,
            plan_id: format!("plan-{}", Uuid::new_v4()),
            goal_id: format!("goal-{}", Uuid::new_v4()),
            workspace,
            risk: RiskLevel::Low,
            max_parallel: triage.max_work_items.clamp(1, 3),
            max_work_items: triage.max_work_items,
            constraints: Vec::new(),
            agents: Vec::new(),
        })
    }

    fn validate(&self) -> Result<(), MasterPlannerError> {
        if self.goal.trim().is_empty() || self.goal.chars().any(char::is_control) {
            return Err(MasterPlannerError::InvalidGoal);
        }
        if self.plan_id.trim().is_empty()
            || self.goal_id.trim().is_empty()
            || !self.workspace.is_dir()
            || self.max_parallel == 0
            || !(1..=crate::master::MAX_WORK_ITEMS).contains(&self.max_work_items)
        {
            return Err(MasterPlannerError::InvalidContext);
        }
        Ok(())
    }
}

#[async_trait]
pub trait PlanGenerator: Send + Sync {
    async fn generate(
        &self,
        context: &PlanningContext,
        cancellation: &CancellationToken,
    ) -> Result<PlanProposal, MasterPlannerError>;
}

pub struct ModelPlanGenerator {
    provider: Arc<dyn ModelProvider>,
    journal_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionPlanningContext {
    pub planning: PlanningContext,
    pub master_conversation_id: String,
}

impl ProductionPlanningContext {
    pub fn new(
        planning: PlanningContext,
        master_conversation_id: impl Into<String>,
    ) -> Result<Self, MasterPlannerError> {
        let master_conversation_id = master_conversation_id.into();
        if !safe_planner_identity(&master_conversation_id) {
            return Err(MasterPlannerError::InvalidContext);
        }
        Ok(Self {
            planning,
            master_conversation_id,
        })
    }

    fn validate(&self) -> Result<(), MasterPlannerError> {
        self.planning.validate()?;
        if !safe_planner_identity(&self.master_conversation_id) {
            return Err(MasterPlannerError::InvalidContext);
        }
        Ok(())
    }
}

pub struct ModelProductionPlanGenerator {
    provider: Arc<dyn ModelProvider>,
    journal_dir: Option<PathBuf>,
}

impl ModelProductionPlanGenerator {
    pub fn new(provider: Arc<dyn ModelProvider>) -> Self {
        Self {
            provider,
            journal_dir: None,
        }
    }

    pub fn with_journal_dir(mut self, journal_dir: impl Into<PathBuf>) -> Self {
        self.journal_dir = Some(journal_dir.into());
        self
    }

    fn trace(
        &self,
        plan_id: &str,
        run_id: Uuid,
        phase: &str,
        payload: serde_json::Value,
    ) -> Result<(), MasterPlannerError> {
        trace_planning_event(
            self.journal_dir.as_deref(),
            plan_id,
            run_id,
            self.provider.id(),
            self.provider.model(),
            phase,
            payload,
        )
    }

    pub async fn generate(
        &self,
        context: &ProductionPlanningContext,
        cancellation: &CancellationToken,
    ) -> Result<ProductionPlan, MasterPlannerError> {
        context.validate()?;
        let mut request =
            ModelRequest {
                run_id: Uuid::new_v4(),
                mode: CognitiveMode::DecisionAnalysis,
                messages: vec![
                    ModelMessage::system(production_plan_system_prompt()),
                    ModelMessage::user(serde_json::to_string(context).map_err(|error| {
                        MasterPlannerError::InvalidContextJson(error.to_string())
                    })?),
                ],
                tools: Vec::new(),
            };
        self.trace(
            &context.planning.plan_id,
            request.run_id,
            "request",
            serde_json::json!({"attempt": 1, "request": &request}),
        )?;
        let response = self
            .provider
            .complete(request.clone(), cancellation)
            .await
            .map_err(|error| {
                let _ = self.trace(
                    &context.planning.plan_id,
                    request.run_id,
                    "error",
                    serde_json::json!({"attempt": 1, "error": &error}),
                );
                MasterPlannerError::Provider(error)
            })?;
        self.trace(
            &context.planning.plan_id,
            request.run_id,
            "response",
            serde_json::json!({"attempt": 1, "response": &response}),
        )?;
        match response.turn {
            ModelTurn::Final { answer } => {
                return parse_production_plan_from_answer(&answer, context);
            }
            ModelTurn::InvalidFinal { content, error } => {
                if let Ok(plan) = parse_production_plan_text(&content, context) {
                    return Ok(plan);
                }
                request.messages.push(ModelMessage::user(format!(
                    "The previous answer was not a valid final response ({error}). \
                     Return one complete AgentAnswer JSON object now. No tools are available."
                )));
            }
            ModelTurn::ToolCalls { calls } => {
                request.messages.push(ModelMessage::user(format!(
                    "The previous attempt requested {} tool call(s), but planning has no tools. \
                     Return the complete final AgentAnswer JSON object now.",
                    calls.len()
                )));
            }
        }
        self.trace(
            &context.planning.plan_id,
            request.run_id,
            "request",
            serde_json::json!({"attempt": 2, "request": &request}),
        )?;
        let response = self
            .provider
            .complete(request.clone(), cancellation)
            .await
            .map_err(|error| {
                let _ = self.trace(
                    &context.planning.plan_id,
                    request.run_id,
                    "error",
                    serde_json::json!({"attempt": 2, "error": &error}),
                );
                MasterPlannerError::Provider(error)
            })?;
        self.trace(
            &context.planning.plan_id,
            request.run_id,
            "response",
            serde_json::json!({"attempt": 2, "response": &response}),
        )?;
        match response.turn {
            ModelTurn::Final { answer } => parse_production_plan_from_answer(&answer, context),
            ModelTurn::InvalidFinal { content, .. } => {
                parse_production_plan_text(&content, context)
            }
            ModelTurn::ToolCalls { .. } => Err(MasterPlannerError::UnexpectedToolCall),
        }
    }
}

fn trace_planning_event(
    journal_dir: Option<&Path>,
    plan_id: &str,
    run_id: Uuid,
    provider_id: &str,
    model: &str,
    phase: &str,
    payload: serde_json::Value,
) -> Result<(), MasterPlannerError> {
    let Some(directory) = journal_dir else {
        return Ok(());
    };
    std::fs::create_dir_all(directory)
        .map_err(|error| MasterPlannerError::Trace(error.to_string()))?;
    let path = planning_journal_path(directory, plan_id);
    let event = serde_json::json!({
        "schema_version": 1,
        "plan_id": plan_id,
        "run_id": run_id,
        "timestamp_unix_ms": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default(),
        "phase": phase,
        "provider_id": provider_id,
        "model": model,
        "payload": payload,
    });
    let bytes =
        serde_json::to_vec(&event).map_err(|error| MasterPlannerError::Trace(error.to_string()))?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err(MasterPlannerError::Trace(
            "planning journal event exceeds 4 MiB".into(),
        ));
    }
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| MasterPlannerError::Trace(error.to_string()))?;
    file.write_all(&bytes)
        .and_then(|_| file.write_all(b"\n"))
        .map_err(|error| MasterPlannerError::Trace(error.to_string()))
}

impl ModelPlanGenerator {
    pub fn new(provider: Arc<dyn ModelProvider>) -> Self {
        Self {
            provider,
            journal_dir: None,
        }
    }

    pub fn with_journal_dir(mut self, journal_dir: impl Into<PathBuf>) -> Self {
        self.journal_dir = Some(journal_dir.into());
        self
    }

    fn trace(
        &self,
        plan_id: &str,
        run_id: Uuid,
        phase: &str,
        payload: serde_json::Value,
    ) -> Result<(), MasterPlannerError> {
        trace_planning_event(
            self.journal_dir.as_deref(),
            plan_id,
            run_id,
            self.provider.id(),
            self.provider.model(),
            phase,
            payload,
        )
    }
}

#[async_trait]
impl PlanGenerator for ModelPlanGenerator {
    async fn generate(
        &self,
        context: &PlanningContext,
        cancellation: &CancellationToken,
    ) -> Result<PlanProposal, MasterPlannerError> {
        context.validate()?;
        let mut request =
            ModelRequest {
                run_id: Uuid::new_v4(),
                mode: CognitiveMode::DecisionAnalysis,
                messages: vec![
                    ModelMessage::system(master_plan_system_prompt()),
                    ModelMessage::user(serde_json::to_string(context).map_err(|error| {
                        MasterPlannerError::InvalidContextJson(error.to_string())
                    })?),
                ],
                tools: Vec::new(),
            };
        self.trace(
            &context.plan_id,
            request.run_id,
            "request",
            serde_json::json!({"attempt": 1, "request": &request}),
        )?;
        let response = self
            .provider
            .complete(request.clone(), cancellation)
            .await
            .map_err(|error| {
                let _ = self.trace(
                    &context.plan_id,
                    request.run_id,
                    "error",
                    serde_json::json!({"attempt": 1, "error": &error}),
                );
                MasterPlannerError::Provider(error)
            })?;
        self.trace(
            &context.plan_id,
            request.run_id,
            "response",
            serde_json::json!({"attempt": 1, "response": &response}),
        )?;
        match response.turn {
            ModelTurn::Final { answer } => return parse_plan_from_answer(&answer, context),
            ModelTurn::InvalidFinal { content, error } => {
                if let Ok(plan) = parse_plan_text(&content, context) {
                    return Ok(plan);
                }
                request.messages.push(ModelMessage::user(format!(
                    "The previous answer was not a valid final response ({error}). \
                     Return one complete AgentAnswer JSON object now. No tools are available."
                )));
            }
            ModelTurn::ToolCalls { calls } => {
                request.messages.push(ModelMessage::user(format!(
                    "The previous attempt requested {} tool call(s), but planning has no tools. \
                     Return the complete final AgentAnswer JSON object now.",
                    calls.len()
                )));
            }
        }
        self.trace(
            &context.plan_id,
            request.run_id,
            "request",
            serde_json::json!({"attempt": 2, "request": &request}),
        )?;
        let response = self
            .provider
            .complete(request.clone(), cancellation)
            .await
            .map_err(|error| {
                let _ = self.trace(
                    &context.plan_id,
                    request.run_id,
                    "error",
                    serde_json::json!({"attempt": 2, "error": &error}),
                );
                MasterPlannerError::Provider(error)
            })?;
        self.trace(
            &context.plan_id,
            request.run_id,
            "response",
            serde_json::json!({"attempt": 2, "response": &response}),
        )?;
        match response.turn {
            ModelTurn::Final { answer } => parse_plan_from_answer(&answer, context),
            ModelTurn::InvalidFinal { content, .. } => parse_plan_text(&content, context),
            ModelTurn::ToolCalls { .. } => Err(MasterPlannerError::UnexpectedToolCall),
        }
    }
}

pub fn planning_journal_path(directory: &Path, plan_id: &str) -> PathBuf {
    let safe = plan_id
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.') {
                byte as char
            } else {
                '_'
            }
        })
        .collect::<String>();
    directory.join(format!("{safe}.jsonl"))
}

pub fn master_plan_system_prompt() -> &'static str {
    r#"You are the planning component of 1-Agent, a Master-Agent that proposes work; deterministic code owns IDs, limits, approval and acceptance.

Create a PlanProposal for the user's goal. Do not execute tools or claim that work happened.

Rules:
- Use only the context's plan_id and goal_id; set approved=false.
- Produce 1..max_work_items Work Items. Prefer the fewest items that preserve real dependencies and independent verification.
- Every Work Item must include required_capabilities, required_tools, acceptance_criteria and expected_artifacts.
- Every expected_artifact must be a relative file path inside the Work Item workspace, never a prose result.
- Use only Agent capabilities visible in context. Never select a brand by reputation.
- Keep sessions, panes and jobs out of the plan; they are execution containers assigned later.
- Write and external actions must retain their required permissions and may require approval.
- If context risk is high, write or external-action Work Items must also use high risk.
- Use context.workspace (or a path below it) for workspace and read_roots. Never broaden read access to a parent directory.
- Do not invent evidence or existing artifacts.
- The Work Item status must be "pending".
- Return one valid 1-Agent AgentAnswer JSON object with status "completed". Its conclusion must be a JSON string containing only the PlanProposal object. Keep all other AgentAnswer arrays empty.

PlanProposal shape:
{
  "schema_version": 1,
  "plan_id": "context plan_id",
  "goal_id": "context goal_id",
  "version": 1,
  "summary": "short user-facing plan summary",
  "work_items": [{
    "id": "safe-id",
    "title": "short title",
    "objective": "specific deliverable",
    "dependencies": ["other-safe-id"],
    "required_capabilities": ["capability"],
    "required_tools": ["tool"],
    "permissions": {"read_roots":["/allowed/root"],"allow_network":false,"allow_write":false},
    "acceptance_criteria": ["observable criterion"],
    "expected_artifacts": ["relative/file/path"],
    "workspace": "/context/workspace",
    "budget": {"max_steps":6,"timeout_ms":120000,"max_tool_calls":8},
    "deadline_unix_ms": null,
    "risk": "low",
    "status": "pending"
  }],
  "max_parallel": 1,
  "approved": false
}
"#
}

pub fn production_plan_system_prompt() -> &'static str {
    r#"You are the production-line planning component of 1-Agent. Deterministic code owns IDs, limits, approval, dispatch and acceptance.

Create a ProductionPlan for the user's Goal. Do not execute tools or claim that work happened.

Rules:
- Use only context.planning.plan_id, context.planning.goal_id and context.master_conversation_id.
- goal.objective must exactly equal context.planning.goal.
- The wrapped plan must have approved=false. A separate session-bound user action owns approval.
- Produce 1..max_work_items Work Items and the fewest real dependency groups.
- Every Work Item must include capabilities, tools, acceptance criteria and expected artifacts.
- Every expected artifact must be a relative path below the Work Item workspace.
- Keep sessions, panes and jobs out of the plan.
- A multi-task Goal must include at least one measurable Key Result.
- Every Work Item must belong to exactly one Key Result.
- Each Key Result must include a non-empty baseline, target and measurement method.
- Risk may not downgrade high-risk write or external actions.
- Return one valid 1-Agent AgentAnswer JSON object with status "completed". Its conclusion must contain only the ProductionPlan JSON object. Keep all other AgentAnswer arrays empty.

ProductionPlan shape:
{
  "schema_version": 1,
  "goal": {
    "goal_id": "context goal_id",
    "master_conversation_id": "context master_conversation_id",
    "objective": "context goal",
    "non_goals": ["explicit non-goal"],
    "constraints": ["bounded constraint"],
    "key_results": [{
      "id": "safe-kr-id",
      "statement": "measurable result",
      "baseline": "current measured state",
      "target": "target measured state",
      "measurement_method": "observable method",
      "work_item_ids": ["safe-task-id"]
    }]
  },
  "plan": {
    "schema_version": 1,
    "plan_id": "context plan_id",
    "goal_id": "context goal_id",
    "version": 1,
    "summary": "short user-facing summary",
    "work_items": [{
      "id": "safe-task-id",
      "title": "short title",
      "objective": "specific deliverable",
      "dependencies": [],
      "required_capabilities": ["capability"],
      "required_tools": ["tool"],
      "permissions": {"read_roots":["/allowed/root"],"allow_network":false,"allow_write":false},
      "acceptance_criteria": ["observable criterion"],
      "expected_artifacts": ["relative/file/path"],
      "workspace": "/context/workspace",
      "budget": {"max_steps":6,"timeout_ms":120000,"max_tool_calls":8},
      "deadline_unix_ms": null,
      "risk": "low",
      "status": "pending"
    }],
    "max_parallel": 1,
    "approved": false
  }
}
"#
}

pub fn parse_production_plan_from_answer(
    answer: &AgentAnswer,
    context: &ProductionPlanningContext,
) -> Result<ProductionPlan, MasterPlannerError> {
    let candidate = json_object_candidate(answer.conclusion.trim())
        .ok_or_else(|| MasterPlannerError::InvalidPlanJson("no JSON object found".into()))?;
    let production: ProductionPlan = serde_json::from_str(candidate)
        .map_err(|error| MasterPlannerError::InvalidPlanJson(error.to_string()))?;
    validate_parsed_production_plan(production, context)
}

fn parse_production_plan_text(
    text: &str,
    context: &ProductionPlanningContext,
) -> Result<ProductionPlan, MasterPlannerError> {
    let candidate = json_object_candidate(text.trim())
        .ok_or_else(|| MasterPlannerError::InvalidPlanJson("no JSON object found".into()))?;
    let value: serde_json::Value = serde_json::from_str(candidate)
        .map_err(|error| MasterPlannerError::InvalidPlanJson(error.to_string()))?;
    let plan_candidate = value
        .get("conclusion")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(candidate);
    let production: ProductionPlan = serde_json::from_str(plan_candidate)
        .map_err(|error| MasterPlannerError::InvalidPlanJson(error.to_string()))?;
    validate_parsed_production_plan(production, context)
}

fn validate_parsed_production_plan(
    mut production: ProductionPlan,
    context: &ProductionPlanningContext,
) -> Result<ProductionPlan, MasterPlannerError> {
    if production.goal.goal_id != context.planning.goal_id
        || production.goal.master_conversation_id != context.master_conversation_id
        || production.plan.plan_id != context.planning.plan_id
        || production.plan.goal_id != context.planning.goal_id
    {
        return Err(MasterPlannerError::IdentityMismatch);
    }
    if production.goal.objective != context.planning.goal {
        return Err(MasterPlannerError::IdentityMismatch);
    }
    if production.plan.version != 1 {
        return Err(MasterPlannerError::InvalidContext);
    }
    if production.plan.approved {
        return Err(MasterPlannerError::PlanSelfApproved);
    }
    production.plan = validate_parsed_plan(production.plan, &context.planning)?;
    production.validate()?;
    Ok(production)
}

pub fn parse_plan_from_answer(
    answer: &AgentAnswer,
    context: &PlanningContext,
) -> Result<PlanProposal, MasterPlannerError> {
    let candidate = json_object_candidate(answer.conclusion.trim())
        .ok_or_else(|| MasterPlannerError::InvalidPlanJson("no JSON object found".into()))?;
    let plan: PlanProposal = serde_json::from_str(candidate)
        .map_err(|error| MasterPlannerError::InvalidPlanJson(error.to_string()))?;
    validate_parsed_plan(plan, context)
}

fn parse_plan_text(
    text: &str,
    context: &PlanningContext,
) -> Result<PlanProposal, MasterPlannerError> {
    let candidate = json_object_candidate(text.trim())
        .ok_or_else(|| MasterPlannerError::InvalidPlanJson("no JSON object found".into()))?;
    let value: serde_json::Value = serde_json::from_str(candidate)
        .map_err(|error| MasterPlannerError::InvalidPlanJson(error.to_string()))?;
    let plan_candidate = value
        .get("conclusion")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(candidate);
    let plan: PlanProposal = serde_json::from_str(plan_candidate)
        .map_err(|error| MasterPlannerError::InvalidPlanJson(error.to_string()))?;
    validate_parsed_plan(plan, context)
}

fn validate_parsed_plan(
    plan: PlanProposal,
    context: &PlanningContext,
) -> Result<PlanProposal, MasterPlannerError> {
    if plan.plan_id != context.plan_id || plan.goal_id != context.goal_id {
        return Err(MasterPlannerError::IdentityMismatch);
    }
    if plan.approved {
        return Err(MasterPlannerError::PlanSelfApproved);
    }
    if plan.work_items.len() > context.max_work_items {
        return Err(MasterPlannerError::TooManyWorkItems);
    }
    let root = context
        .workspace
        .canonicalize()
        .map_err(|_| MasterPlannerError::InvalidContext)?;
    for item in &plan.work_items {
        let workspace = item
            .workspace
            .canonicalize()
            .map_err(|_| MasterPlannerError::WorkspaceOutsideRoot(item.id.clone()))?;
        if !workspace.starts_with(&root) {
            return Err(MasterPlannerError::WorkspaceOutsideRoot(item.id.clone()));
        }
        for read_root in &item.permissions.read_roots {
            let read_root = read_root
                .canonicalize()
                .map_err(|_| MasterPlannerError::ReadRootOutsideWorkspace(item.id.clone()))?;
            if !read_root.starts_with(&root) {
                return Err(MasterPlannerError::ReadRootOutsideWorkspace(
                    item.id.clone(),
                ));
            }
        }
        if context.risk == RiskLevel::High
            && (item.permissions.allow_write || item.permissions.allow_network)
            && item.risk != RiskLevel::High
        {
            return Err(MasterPlannerError::RiskDowngraded(item.id.clone()));
        }
    }
    plan.validate().map_err(MasterPlannerError::Plan)?;
    Ok(plan)
}

fn json_object_candidate(text: &str) -> Option<&str> {
    let without_prefix = text
        .strip_prefix("```json")
        .or_else(|| text.strip_prefix("```"))
        .unwrap_or(text);
    let trimmed = without_prefix
        .strip_suffix("```")
        .unwrap_or(without_prefix)
        .trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Some(trimmed);
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    (start < end).then_some(&trimmed[start..=end])
}

fn safe_planner_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

pub fn plan_triage_constraints(context: &PlanningContext) -> PermissionSet {
    if context.risk == RiskLevel::High {
        PermissionSet {
            read_roots: vec![context.workspace.clone()],
            allow_network: false,
            allow_write: true,
        }
    } else {
        PermissionSet {
            read_roots: vec![context.workspace.clone()],
            allow_network: false,
            allow_write: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MasterPlannerError {
    #[error("goal must be non-empty and contain no control characters")]
    InvalidGoal,
    #[error("planning context is invalid")]
    InvalidContext,
    #[error("planning context could not be serialized: {0}")]
    InvalidContextJson(String),
    #[error("planning provider failed: {0}")]
    Provider(ProviderError),
    #[error("planning provider requested a tool; planning is a single structured turn")]
    UnexpectedToolCall,
    #[error("planning journal failed: {0}")]
    Trace(String),
    #[error("provider did not return a valid PlanProposal JSON object: {0}")]
    InvalidPlanJson(String),
    #[error("provider changed the deterministic plan or goal identity")]
    IdentityMismatch,
    #[error("provider attempted to self-approve the plan")]
    PlanSelfApproved,
    #[error("provider exceeded the work-item limit")]
    TooManyWorkItems,
    #[error("Work Item '{0}' workspace is outside the approved planning root")]
    WorkspaceOutsideRoot(String),
    #[error("Work Item '{0}' requests read access outside the approved planning root")]
    ReadRootOutsideWorkspace(String),
    #[error("Work Item '{0}' downgraded a high-risk write or external action")]
    RiskDowngraded(String),
    #[error("provider returned an invalid plan: {0}")]
    Plan(#[from] MasterPlanError),
    #[error("provider returned an invalid production-line plan: {0}")]
    ProductionLine(#[from] ProductionLineError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AnswerStatus, ModelResponse, Usage};
    use crate::master::{MASTER_PLAN_SCHEMA_VERSION, WorkBudget, WorkItem, WorkItemStatus};
    use crate::production_line::{GoalContract, KeyResult, PRODUCTION_LINE_SCHEMA_VERSION};
    use crate::provider::ProviderError;
    use tempfile::tempdir;

    fn context() -> PlanningContext {
        let triage = TriageDecision {
            route: WorkRoute::MultiStep,
            reason: "test".into(),
            max_work_items: 4,
            requires_plan: true,
            requires_user_approval: true,
            allows_dispatch: false,
        };
        PlanningContext::new("先实现接口，然后测试", tempdir().unwrap().keep(), &triage).unwrap()
    }

    #[test]
    fn triage_has_direct_single_and_multi_agent_fast_paths() {
        assert_eq!(
            triage_goal("1A 是什么？", &TriageSignals::default()).route,
            WorkRoute::DirectAnswer
        );
        assert_eq!(
            triage_goal("修复这个测试", &TriageSignals::default()).route,
            WorkRoute::SingleAgent
        );
        assert_eq!(
            triage_goal("如何优化并封装这个项目？", &TriageSignals::default()).route,
            WorkRoute::DirectAnswer
        );
        assert_eq!(
            triage_goal(
                "请你打开项目，优化并且封装为通用技能",
                &TriageSignals::default()
            )
            .route,
            WorkRoute::SingleAgent
        );
        assert_eq!(
            triage_goal("先审计，然后修复，最后测试", &TriageSignals::default()).route,
            WorkRoute::MultiStep
        );
        assert_eq!(
            triage_goal("并行分派给多个 Agent 执行", &TriageSignals::default()).route,
            WorkRoute::MultiAgent
        );
        assert_eq!(
            triage_goal(
                "修改生产文件",
                &TriageSignals {
                    requires_write: true,
                    ..TriageSignals::default()
                }
            )
            .route,
            WorkRoute::ApprovalRequired
        );
    }

    #[test]
    fn plan_parser_rejects_identity_changes_and_self_approval() {
        let context = context();
        let mut plan = PlanProposal {
            schema_version: MASTER_PLAN_SCHEMA_VERSION,
            plan_id: context.plan_id.clone(),
            goal_id: context.goal_id.clone(),
            version: 1,
            summary: "Plan".into(),
            work_items: vec![WorkItem {
                id: "implement".into(),
                title: "Implement".into(),
                objective: "Implement the interface".into(),
                dependencies: Vec::new(),
                required_capabilities: vec!["rust".into()],
                required_tools: vec!["shell".into()],
                permissions: PermissionSet::default(),
                acceptance_criteria: vec!["cargo test passes".into()],
                expected_artifacts: vec!["target/test.log".into()],
                workspace: context.workspace.clone(),
                budget: WorkBudget::default(),
                deadline_unix_ms: None,
                risk: RiskLevel::Low,
                status: WorkItemStatus::Pending,
            }],
            max_parallel: 1,
            approved: false,
        };
        let answer = AgentAnswer {
            schema_version: 2,
            status: AnswerStatus::Completed,
            conclusion: serde_json::to_string(&plan).unwrap(),
            claims: Vec::new(),
            evidence: Vec::new(),
            assumptions: Vec::new(),
            constraints: Vec::new(),
            unknowns: Vec::new(),
            conflicts: Vec::new(),
            flip_conditions: Vec::new(),
            action_receipts: Vec::new(),
        };
        assert_eq!(parse_plan_from_answer(&answer, &context).unwrap(), plan);
        plan.approved = true;
        let answer = AgentAnswer {
            conclusion: serde_json::to_string(&plan).unwrap(),
            ..answer
        };
        assert!(matches!(
            parse_plan_from_answer(&answer, &context),
            Err(MasterPlannerError::PlanSelfApproved)
        ));
        plan.approved = false;
        plan.plan_id = "forged".into();
        let answer = AgentAnswer {
            conclusion: serde_json::to_string(&plan).unwrap(),
            ..answer
        };
        assert!(matches!(
            parse_plan_from_answer(&answer, &context),
            Err(MasterPlannerError::IdentityMismatch)
        ));
    }

    struct FakePlannerProvider {
        plan: PlanProposal,
    }

    #[async_trait]
    impl ModelProvider for FakePlannerProvider {
        fn id(&self) -> &str {
            "fake-planner"
        }

        fn model(&self) -> &str {
            "fake-model"
        }

        async fn complete(
            &self,
            _request: ModelRequest,
            _cancellation: &CancellationToken,
        ) -> Result<ModelResponse, ProviderError> {
            Ok(ModelResponse {
                provider_id: self.id().into(),
                model: self.model().into(),
                turn: ModelTurn::Final {
                    answer: AgentAnswer {
                        schema_version: 2,
                        status: AnswerStatus::Completed,
                        conclusion: serde_json::to_string(&self.plan).unwrap(),
                        claims: Vec::new(),
                        evidence: Vec::new(),
                        assumptions: Vec::new(),
                        constraints: Vec::new(),
                        unknowns: Vec::new(),
                        conflicts: Vec::new(),
                        flip_conditions: Vec::new(),
                        action_receipts: Vec::new(),
                    },
                },
                usage: Usage::default(),
            })
        }
    }

    #[tokio::test]
    async fn model_generator_returns_only_a_validated_unapproved_plan() {
        let context = context();
        let plan = PlanProposal {
            schema_version: MASTER_PLAN_SCHEMA_VERSION,
            plan_id: context.plan_id.clone(),
            goal_id: context.goal_id.clone(),
            version: 1,
            summary: "Implement and verify".into(),
            work_items: vec![WorkItem {
                id: "implement".into(),
                title: "Implement".into(),
                objective: "Implement the interface".into(),
                dependencies: Vec::new(),
                required_capabilities: vec!["rust".into()],
                required_tools: vec!["shell".into()],
                permissions: PermissionSet::default(),
                acceptance_criteria: vec!["cargo test passes".into()],
                expected_artifacts: vec!["target/test.log".into()],
                workspace: context.workspace.clone(),
                budget: WorkBudget::default(),
                deadline_unix_ms: None,
                risk: RiskLevel::Low,
                status: WorkItemStatus::Pending,
            }],
            max_parallel: 1,
            approved: false,
        };
        let generator =
            ModelPlanGenerator::new(Arc::new(FakePlannerProvider { plan: plan.clone() }));
        let generated = generator
            .generate(&context, &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(generated, plan);
        assert!(!generated.approved);
    }

    struct FakeProductionPlannerProvider {
        plan: ProductionPlan,
    }

    #[async_trait]
    impl ModelProvider for FakeProductionPlannerProvider {
        fn id(&self) -> &str {
            "production-planner"
        }

        fn model(&self) -> &str {
            "fake-model"
        }

        async fn complete(
            &self,
            _request: ModelRequest,
            _cancellation: &CancellationToken,
        ) -> Result<ModelResponse, ProviderError> {
            Ok(ModelResponse {
                provider_id: self.id().into(),
                model: self.model().into(),
                turn: ModelTurn::Final {
                    answer: AgentAnswer {
                        schema_version: 2,
                        status: AnswerStatus::Completed,
                        conclusion: serde_json::to_string(&self.plan).unwrap(),
                        claims: Vec::new(),
                        evidence: Vec::new(),
                        assumptions: Vec::new(),
                        constraints: Vec::new(),
                        unknowns: Vec::new(),
                        conflicts: Vec::new(),
                        flip_conditions: Vec::new(),
                        action_receipts: Vec::new(),
                    },
                },
                usage: Usage::default(),
            })
        }
    }

    fn production(context: &ProductionPlanningContext) -> ProductionPlan {
        let workspace = context.planning.workspace.clone();
        ProductionPlan {
            schema_version: PRODUCTION_LINE_SCHEMA_VERSION,
            goal: GoalContract {
                goal_id: context.planning.goal_id.clone(),
                master_conversation_id: context.master_conversation_id.clone(),
                objective: context.planning.goal.clone(),
                non_goals: vec!["do not expand scope".into()],
                constraints: vec!["fail closed".into()],
                key_results: vec![KeyResult {
                    id: "kr-1".into(),
                    statement: "implementation passes tests".into(),
                    baseline: "not implemented".into(),
                    target: "all tests pass".into(),
                    measurement_method: "cargo test".into(),
                    work_item_ids: vec!["implement".into(), "verify".into()],
                }],
            },
            plan: PlanProposal {
                schema_version: MASTER_PLAN_SCHEMA_VERSION,
                plan_id: context.planning.plan_id.clone(),
                goal_id: context.planning.goal_id.clone(),
                version: 1,
                summary: "Implement and verify".into(),
                work_items: vec![
                    WorkItem {
                        id: "implement".into(),
                        title: "Implement".into(),
                        objective: "Implement the interface".into(),
                        dependencies: Vec::new(),
                        required_capabilities: vec!["rust".into()],
                        required_tools: vec!["shell".into()],
                        permissions: PermissionSet::default(),
                        acceptance_criteria: vec!["code compiles".into()],
                        expected_artifacts: vec!["implement.log".into()],
                        workspace: workspace.clone(),
                        budget: WorkBudget::default(),
                        deadline_unix_ms: None,
                        risk: RiskLevel::Low,
                        status: WorkItemStatus::Pending,
                    },
                    WorkItem {
                        id: "verify".into(),
                        title: "Verify".into(),
                        objective: "Verify the interface".into(),
                        dependencies: vec!["implement".into()],
                        required_capabilities: vec!["rust".into()],
                        required_tools: vec!["shell".into()],
                        permissions: PermissionSet::default(),
                        acceptance_criteria: vec!["tests pass".into()],
                        expected_artifacts: vec!["verify.log".into()],
                        workspace,
                        budget: WorkBudget::default(),
                        deadline_unix_ms: None,
                        risk: RiskLevel::Low,
                        status: WorkItemStatus::Pending,
                    },
                ],
                max_parallel: 2,
                approved: false,
            },
        }
    }

    #[tokio::test]
    async fn production_planner_preserves_identity_and_rejects_self_approval() {
        let context = ProductionPlanningContext::new(context(), "master-1").unwrap();
        let expected = production(&context);
        let generator =
            ModelProductionPlanGenerator::new(Arc::new(FakeProductionPlannerProvider {
                plan: expected.clone(),
            }));
        let generated = generator
            .generate(&context, &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(generated, expected);

        let mut self_approved = expected;
        self_approved.plan.approved = true;
        let generator =
            ModelProductionPlanGenerator::new(Arc::new(FakeProductionPlannerProvider {
                plan: self_approved,
            }));
        assert!(matches!(
            generator
                .generate(&context, &CancellationToken::new())
                .await,
            Err(MasterPlannerError::PlanSelfApproved)
        ));
    }

    struct RepairingPlannerProvider {
        plan: PlanProposal,
        calls: std::sync::atomic::AtomicUsize,
    }

    #[async_trait]
    impl ModelProvider for RepairingPlannerProvider {
        fn id(&self) -> &str {
            "repairing-planner"
        }

        fn model(&self) -> &str {
            "fake-model"
        }

        async fn complete(
            &self,
            _request: ModelRequest,
            _cancellation: &CancellationToken,
        ) -> Result<ModelResponse, ProviderError> {
            let call = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if call == 0 {
                return Ok(ModelResponse {
                    provider_id: self.id().into(),
                    model: self.model().into(),
                    turn: ModelTurn::ToolCalls { calls: Vec::new() },
                    usage: Usage::default(),
                });
            }
            Ok(ModelResponse {
                provider_id: self.id().into(),
                model: self.model().into(),
                turn: ModelTurn::Final {
                    answer: AgentAnswer {
                        schema_version: 2,
                        status: AnswerStatus::Completed,
                        conclusion: serde_json::to_string(&self.plan).unwrap(),
                        claims: Vec::new(),
                        evidence: Vec::new(),
                        assumptions: Vec::new(),
                        constraints: Vec::new(),
                        unknowns: Vec::new(),
                        conflicts: Vec::new(),
                        flip_conditions: Vec::new(),
                        action_receipts: Vec::new(),
                    },
                },
                usage: Usage::default(),
            })
        }
    }

    #[tokio::test]
    async fn planner_repairs_one_tool_call_and_journals_every_attempt() {
        let context = context();
        let plan = PlanProposal {
            schema_version: MASTER_PLAN_SCHEMA_VERSION,
            plan_id: context.plan_id.clone(),
            goal_id: context.goal_id.clone(),
            version: 1,
            summary: "Implement and verify".into(),
            work_items: vec![WorkItem {
                id: "implement".into(),
                title: "Implement".into(),
                objective: "Implement the interface".into(),
                dependencies: Vec::new(),
                required_capabilities: vec!["rust".into()],
                required_tools: vec!["shell".into()],
                permissions: PermissionSet::default(),
                acceptance_criteria: vec!["cargo test passes".into()],
                expected_artifacts: vec!["target/test.log".into()],
                workspace: context.workspace.clone(),
                budget: WorkBudget::default(),
                deadline_unix_ms: None,
                risk: RiskLevel::Low,
                status: WorkItemStatus::Pending,
            }],
            max_parallel: 1,
            approved: false,
        };
        let journal_dir = tempdir().unwrap();
        let provider = Arc::new(RepairingPlannerProvider {
            plan: plan.clone(),
            calls: std::sync::atomic::AtomicUsize::new(0),
        });
        let generator =
            ModelPlanGenerator::new(provider.clone()).with_journal_dir(journal_dir.path());
        let generated = generator
            .generate(&context, &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(generated, plan);
        assert_eq!(provider.calls.load(std::sync::atomic::Ordering::SeqCst), 2);
        let journal =
            std::fs::read_to_string(planning_journal_path(journal_dir.path(), &context.plan_id))
                .unwrap();
        assert_eq!(journal.lines().count(), 4);
        assert!(journal.contains("\"phase\":\"request\""));
        assert!(journal.contains("\"phase\":\"response\""));
    }

    #[test]
    fn direct_answers_do_not_create_plans_or_agents() {
        let triage = triage_goal("1A 是什么？", &TriageSignals::default());
        assert!(!triage.requires_plan);
        assert_eq!(triage.max_work_items, 0);
        assert!(!triage.allows_dispatch);
    }

    #[test]
    fn parsed_plan_cannot_escape_workspace_or_downgrade_high_risk_write() {
        let mut context = context();
        context.risk = RiskLevel::High;
        let mut item = WorkItem {
            id: "write".into(),
            title: "Write".into(),
            objective: "Write a file".into(),
            dependencies: Vec::new(),
            required_capabilities: vec!["shell".into()],
            required_tools: vec!["shell".into()],
            permissions: PermissionSet {
                read_roots: vec![context.workspace.clone()],
                allow_network: false,
                allow_write: true,
            },
            acceptance_criteria: vec!["file exists".into()],
            expected_artifacts: vec!["result.txt".into()],
            workspace: context.workspace.clone(),
            budget: WorkBudget::default(),
            deadline_unix_ms: None,
            risk: RiskLevel::Low,
            status: WorkItemStatus::Pending,
        };
        let plan = PlanProposal {
            schema_version: MASTER_PLAN_SCHEMA_VERSION,
            plan_id: context.plan_id.clone(),
            goal_id: context.goal_id.clone(),
            version: 1,
            summary: "Plan".into(),
            work_items: vec![item.clone()],
            max_parallel: 1,
            approved: false,
        };
        let answer = AgentAnswer {
            schema_version: 2,
            status: AnswerStatus::Completed,
            conclusion: serde_json::to_string(&plan).unwrap(),
            claims: Vec::new(),
            evidence: Vec::new(),
            assumptions: Vec::new(),
            constraints: Vec::new(),
            unknowns: Vec::new(),
            conflicts: Vec::new(),
            flip_conditions: Vec::new(),
            action_receipts: Vec::new(),
        };
        assert!(matches!(
            parse_plan_from_answer(&answer, &context),
            Err(MasterPlannerError::RiskDowngraded(task)) if task == "write"
        ));
        item.risk = RiskLevel::High;
        item.workspace = std::env::temp_dir();
        let plan = PlanProposal {
            work_items: vec![item],
            ..plan
        };
        let answer = AgentAnswer {
            conclusion: serde_json::to_string(&plan).unwrap(),
            ..answer
        };
        assert!(matches!(
            parse_plan_from_answer(&answer, &context),
            Err(MasterPlannerError::WorkspaceOutsideRoot(task)) if task == "write"
        ));
    }
}
