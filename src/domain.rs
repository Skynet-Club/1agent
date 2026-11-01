use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TaskRequest {
    pub objective: String,
    #[serde(default)]
    pub deliverable: Option<String>,
    #[serde(default)]
    pub known_facts: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub unknowns: Vec<UnknownItem>,
    #[serde(default)]
    pub decision_criteria: Vec<String>,
    #[serde(default)]
    pub success_criteria: Vec<String>,
    #[serde(default)]
    pub experience_scope: Option<String>,
    #[serde(default)]
    pub permissions: PermissionSet,
    #[serde(default)]
    pub budget: RunBudget,
}

impl TaskRequest {
    pub fn from_objective(objective: impl Into<String>, read_root: PathBuf) -> Self {
        Self {
            objective: objective.into(),
            deliverable: None,
            known_facts: Vec::new(),
            constraints: Vec::new(),
            unknowns: Vec::new(),
            decision_criteria: Vec::new(),
            success_criteria: Vec::new(),
            experience_scope: None,
            permissions: PermissionSet {
                read_roots: vec![read_root],
                allow_network: false,
                allow_write: false,
            },
            budget: RunBudget::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UnknownItem {
    pub description: String,
    #[serde(default)]
    pub impact: ImpactLevel,
    #[serde(default)]
    pub why_it_matters: Option<String>,
    #[serde(default)]
    pub reduction_method: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ImpactLevel {
    Low,
    #[default]
    Medium,
    High,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PermissionSet {
    #[serde(default)]
    pub read_roots: Vec<PathBuf>,
    #[serde(default)]
    pub allow_network: bool,
    #[serde(default)]
    pub allow_write: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunBudget {
    pub max_steps: usize,
    pub max_tool_calls: usize,
    pub timeout_ms: u64,
    #[serde(default = "default_max_context_chars")]
    pub max_context_chars: usize,
    #[serde(default)]
    pub max_provider_p95_latency_ms: Option<u64>,
    #[serde(default)]
    pub max_unit_cost_micros_per_k: Option<u64>,
}

impl Default for RunBudget {
    fn default() -> Self {
        Self {
            max_steps: 6,
            max_tool_calls: 8,
            timeout_ms: 120_000,
            max_context_chars: 100_000,
            max_provider_p95_latency_ms: None,
            max_unit_cost_micros_per_k: None,
        }
    }
}

fn default_max_context_chars() -> usize {
    100_000
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CognitiveMode {
    DirectAnswer,
    EvidenceResearch,
    DecisionAnalysis,
    Execute,
    Diagnosis,
    Escalate,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClaimStatus {
    Verified,
    UserAsserted,
    Derived,
    Assumed,
    ModelPrior,
    Conflicting,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    pub id: String,
    pub text: String,
    pub status: ClaimStatus,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnswerStatus {
    Completed,
    Conditional,
    NeedsEvidence,
    NeedsAuthorization,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    Completed,
    NeedsEvidence,
    NeedsAuthorization,
    BudgetExhausted,
    Cancelled,
    ProviderError,
    ToolFailure,
    InvalidOutput,
    InternalError,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub source: String,
    pub statement: String,
    #[serde(default)]
    pub claim_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FlipCondition {
    pub if_observed: String,
    pub then: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AssumptionItem {
    pub description: String,
    pub impact: ImpactLevel,
    pub verification_method: Option<String>,
}

impl<'de> Deserialize<'de> for AssumptionItem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Representation {
            Legacy(String),
            Structured(StructuredAssumption),
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct StructuredAssumption {
            description: String,
            #[serde(default)]
            impact: ImpactLevel,
            #[serde(default)]
            verification_method: Option<String>,
        }
        match Representation::deserialize(deserializer)? {
            Representation::Legacy(description) => Ok(Self {
                description,
                impact: ImpactLevel::Medium,
                verification_method: None,
            }),
            Representation::Structured(StructuredAssumption {
                description,
                impact,
                verification_method,
            }) => Ok(Self {
                description,
                impact,
                verification_method,
            }),
        }
    }
}

impl From<&str> for AssumptionItem {
    fn from(value: &str) -> Self {
        Self {
            description: value.into(),
            impact: ImpactLevel::Medium,
            verification_method: None,
        }
    }
}

impl From<String> for AssumptionItem {
    fn from(value: String) -> Self {
        Self {
            description: value,
            impact: ImpactLevel::Medium,
            verification_method: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentAnswer {
    pub status: AnswerStatus,
    pub conclusion: String,
    #[serde(default)]
    pub claims: Vec<Claim>,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    #[serde(default)]
    pub assumptions: Vec<AssumptionItem>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub unknowns: Vec<UnknownItem>,
    #[serde(default)]
    pub flip_conditions: Vec<FlipCondition>,
    #[serde(default)]
    pub action_receipts: Vec<String>,
}

impl AgentAnswer {
    pub fn validate(&self, request: &TaskRequest) -> Result<(), AnswerValidationError> {
        if self.conclusion.trim().is_empty() {
            return Err(AnswerValidationError::EmptyConclusion);
        }

        if !request.permissions.allow_write && !self.action_receipts.is_empty() {
            return Err(AnswerValidationError::UnauthorizedActionReceipt);
        }
        let claim_ids: std::collections::BTreeSet<_> =
            self.claims.iter().map(|claim| claim.id.as_str()).collect();
        if claim_ids.len() != self.claims.len() {
            return Err(AnswerValidationError::DuplicateClaimId);
        }
        for claim in &self.claims {
            match claim.status {
                ClaimStatus::Verified
                    if !claim.sources.iter().any(|source| !source.trim().is_empty()) =>
                {
                    return Err(AnswerValidationError::VerifiedClaimWithoutSource(
                        claim.id.clone(),
                    ));
                }
                ClaimStatus::Derived | ClaimStatus::Assumed
                    if claim
                        .scope
                        .as_deref()
                        .map(str::trim)
                        .unwrap_or_default()
                        .is_empty() =>
                {
                    return Err(AnswerValidationError::ScopedClaimMissingScope(
                        claim.id.clone(),
                    ));
                }
                ClaimStatus::ModelPrior
                    if claim.sources.iter().any(|source| !source.trim().is_empty()) =>
                {
                    return Err(AnswerValidationError::ModelPriorHasSource(claim.id.clone()));
                }
                ClaimStatus::Conflicting
                    if claim
                        .sources
                        .iter()
                        .filter(|source| !source.trim().is_empty())
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        < 2 =>
                {
                    return Err(AnswerValidationError::ConflictingClaimNeedsTwoSources(
                        claim.id.clone(),
                    ));
                }
                _ => {}
            }
        }
        for unknown in &self.unknowns {
            if unknown.description.trim().is_empty() {
                return Err(AnswerValidationError::EmptyUnknownDescription);
            }
            if unknown.impact == ImpactLevel::High
                && unknown
                    .reduction_method
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or_default()
                    .is_empty()
            {
                return Err(AnswerValidationError::HighImpactUnknownWithoutReduction(
                    unknown.description.clone(),
                ));
            }
        }
        for assumption in &self.assumptions {
            if assumption.description.trim().is_empty() {
                return Err(AnswerValidationError::EmptyAssumptionDescription);
            }
            if assumption.impact == ImpactLevel::High
                && assumption
                    .verification_method
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or_default()
                    .is_empty()
            {
                return Err(
                    AnswerValidationError::HighImpactAssumptionWithoutVerification(
                        assumption.description.clone(),
                    ),
                );
            }
        }
        for evidence in &self.evidence {
            if evidence.source.trim().is_empty() {
                return Err(AnswerValidationError::EmptyEvidenceSource);
            }
            if evidence.statement.trim().is_empty() {
                return Err(AnswerValidationError::EmptyEvidenceStatement);
            }
            if evidence.claim_ids.is_empty() {
                return Err(AnswerValidationError::EvidenceWithoutClaim);
            }
            let mut seen_claims = std::collections::BTreeSet::new();
            for claim_id in &evidence.claim_ids {
                if !seen_claims.insert(claim_id.as_str()) {
                    return Err(AnswerValidationError::DuplicateEvidenceClaimReference(
                        claim_id.clone(),
                    ));
                }
                if !claim_ids.contains(claim_id.as_str()) {
                    return Err(AnswerValidationError::UnknownClaimReference(
                        claim_id.clone(),
                    ));
                }
                let claim = self
                    .claims
                    .iter()
                    .find(|claim| claim.id == *claim_id)
                    .expect("claim id was checked above");
                if !claim.sources.contains(&evidence.source) {
                    return Err(AnswerValidationError::EvidenceSourceNotDeclared {
                        evidence_source: evidence.source.clone(),
                        claim_id: claim_id.clone(),
                    });
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AnswerValidationError {
    #[error("answer conclusion must not be empty")]
    EmptyConclusion,
    #[error("read-only task reported an external action receipt")]
    UnauthorizedActionReceipt,
    #[error("answer contains duplicate claim ids")]
    DuplicateClaimId,
    #[error("evidence references unknown claim id '{0}'")]
    UnknownClaimReference(String),
    #[error("verified claim '{0}' has no source")]
    VerifiedClaimWithoutSource(String),
    #[error("derived or assumed claim '{0}' has no scope")]
    ScopedClaimMissingScope(String),
    #[error("model-prior claim '{0}' must not declare an external source")]
    ModelPriorHasSource(String),
    #[error("conflicting claim '{0}' requires at least two distinct sources")]
    ConflictingClaimNeedsTwoSources(String),
    #[error("evidence source must not be empty")]
    EmptyEvidenceSource,
    #[error("evidence statement must not be empty")]
    EmptyEvidenceStatement,
    #[error("evidence record must reference at least one claim")]
    EvidenceWithoutClaim,
    #[error("evidence repeats claim reference '{0}'")]
    DuplicateEvidenceClaimReference(String),
    #[error("evidence source '{evidence_source}' is not declared by claim '{claim_id}'")]
    EvidenceSourceNotDeclared {
        evidence_source: String,
        claim_id: String,
    },
    #[error("unknown description must not be empty")]
    EmptyUnknownDescription,
    #[error("high-impact unknown '{0}' requires a reduction method")]
    HighImpactUnknownWithoutReduction(String),
    #[error("assumption description must not be empty")]
    EmptyAssumptionDescription,
    #[error("high-impact assumption '{0}' requires a verification method")]
    HighImpactAssumptionWithoutVerification(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ModelMessage {
    pub role: MessageRole,
    pub content: String,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default)]
    pub tool_call_id: Option<String>,
}

impl ModelMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn assistant(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            tool_calls,
            tool_call_id: None,
        }
    }

    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Tool,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Ok,
    InvalidArguments,
    PermissionDenied,
    TimedOut,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ToolObservation {
    pub call_id: String,
    pub tool: String,
    pub status: ToolStatus,
    pub summary: String,
    pub content: String,
    pub error_code: Option<String>,
    pub metadata: BTreeMap<String, String>,
}

impl ToolObservation {
    pub fn model_content(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| {
            format!(
                r#"{{"call_id":"{}","tool":"{}","status":"failed","summary":"observation serialization failed"}}"#,
                self.call_id, self.tool
            )
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ModelRequest {
    pub run_id: Uuid,
    pub mode: CognitiveMode,
    pub messages: Vec<ModelMessage>,
    pub tools: Vec<ToolSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ModelTurn {
    Final { answer: AgentAnswer },
    ToolCalls { calls: Vec<ToolCall> },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_micros: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ModelResponse {
    pub provider_id: String,
    pub model: String,
    pub turn: ModelTurn,
    pub usage: Usage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunResult {
    pub run_id: Uuid,
    pub journal_path: PathBuf,
    pub answer: AgentAnswer,
    pub stop_reason: StopReason,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct ProviderRouteKey {
    pub provider_id: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCheckpoint {
    pub mode: CognitiveMode,
    pub messages: Vec<ModelMessage>,
    pub tool_calls_used: usize,
    pub unavailable_routes: Vec<ProviderRouteKey>,
    pub next_step: usize,
    pub elapsed_ms: u64,
    #[serde(default)]
    pub route_plan_sha256: Option<String>,
    #[serde(default)]
    pub route_policy_sources: Option<crate::router::RoutePolicySources>,
    #[serde(default)]
    pub route_objective: Option<crate::router::RouteObjective>,
    #[serde(default)]
    pub tool_capabilities_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InjectedExperience {
    pub id: Uuid,
    pub version: u32,
    pub reason: String,
}

pub fn infer_mode(request: &TaskRequest) -> CognitiveMode {
    let text = format!(
        "{} {} {}",
        request.objective,
        request.decision_criteria.join(" "),
        request
            .unknowns
            .iter()
            .map(|u| &u.description)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ")
    )
    .to_lowercase();

    if contains_any(
        &text,
        &[
            "diagnose",
            "root cause",
            "why did",
            "failure",
            "bug",
            "诊断",
            "根因",
            "故障",
            "报错",
        ],
    ) {
        return CognitiveMode::Diagnosis;
    }
    if contains_any(
        &text,
        &[
            "compare", "choose", "decision", "tradeoff", "versus", " vs ", "选择", "决策", "比较",
            "权衡",
        ],
    ) {
        return CognitiveMode::DecisionAnalysis;
    }
    if contains_any(
        &text,
        &[
            "execute", "write", "delete", "send", "buy", "执行", "写入", "删除", "发送", "购买",
        ],
    ) {
        return CognitiveMode::Execute;
    }
    if !request.known_facts.is_empty()
        || !request.unknowns.is_empty()
        || contains_any(
            &text,
            &["search", "evidence", "source", "检索", "证据", "来源"],
        )
    {
        return CognitiveMode::EvidenceResearch;
    }
    CognitiveMode::DirectAnswer
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_inference_is_deterministic_and_requirement_aware() {
        let mut request = TaskRequest::from_objective("Compare A vs B", PathBuf::from("."));
        assert_eq!(infer_mode(&request), CognitiveMode::DecisionAnalysis);

        request.objective = "Diagnose why the build failed".into();
        assert_eq!(infer_mode(&request), CognitiveMode::Diagnosis);

        request.objective = "Execute the deployment".into();
        assert_eq!(infer_mode(&request), CognitiveMode::Execute);
    }

    #[test]
    fn read_only_answer_rejects_action_receipts() {
        let request = TaskRequest::from_objective("answer", PathBuf::from("."));
        let answer = AgentAnswer {
            status: AnswerStatus::Completed,
            conclusion: "done".into(),
            claims: Vec::new(),
            evidence: Vec::new(),
            assumptions: Vec::new(),
            constraints: Vec::new(),
            unknowns: Vec::new(),
            flip_conditions: Vec::new(),
            action_receipts: vec!["deleted file".into()],
        };

        assert_eq!(
            answer.validate(&request),
            Err(AnswerValidationError::UnauthorizedActionReceipt)
        );
    }

    #[test]
    fn evidence_must_reference_a_declared_claim() {
        let request = TaskRequest::from_objective("answer", PathBuf::from("."));
        let answer = AgentAnswer {
            status: AnswerStatus::Completed,
            conclusion: "done".into(),
            claims: Vec::new(),
            evidence: vec![Evidence {
                source: "fixture".into(),
                statement: "verified".into(),
                claim_ids: vec!["missing".into()],
            }],
            assumptions: Vec::new(),
            constraints: Vec::new(),
            unknowns: Vec::new(),
            flip_conditions: Vec::new(),
            action_receipts: Vec::new(),
        };
        assert_eq!(
            answer.validate(&request),
            Err(AnswerValidationError::UnknownClaimReference(
                "missing".into()
            ))
        );
    }

    #[test]
    fn legacy_checkpoint_without_route_metadata_deserializes() {
        let checkpoint: RuntimeCheckpoint = serde_json::from_value(serde_json::json!({
            "mode": "direct_answer",
            "messages": [],
            "tool_calls_used": 0,
            "unavailable_routes": [],
            "next_step": 0,
            "elapsed_ms": 0
        }))
        .unwrap();
        assert_eq!(checkpoint.route_plan_sha256, None);
        assert_eq!(checkpoint.route_policy_sources, None);
        assert_eq!(checkpoint.route_objective, None);
        assert_eq!(checkpoint.tool_capabilities_sha256, None);
    }

    #[test]
    fn claim_statuses_enforce_evidence_provenance_invariants() {
        let request = TaskRequest::from_objective("answer", PathBuf::from("."));
        let mut claim = Claim {
            id: "claim-1".into(),
            text: "claim".into(),
            status: ClaimStatus::Verified,
            sources: Vec::new(),
            scope: None,
        };
        let mut answer = AgentAnswer {
            status: AnswerStatus::Completed,
            conclusion: "answer".into(),
            claims: vec![claim.clone()],
            evidence: Vec::new(),
            assumptions: Vec::new(),
            constraints: Vec::new(),
            unknowns: Vec::new(),
            flip_conditions: Vec::new(),
            action_receipts: Vec::new(),
        };
        assert_eq!(
            answer.validate(&request),
            Err(AnswerValidationError::VerifiedClaimWithoutSource(
                "claim-1".into()
            ))
        );

        claim.status = ClaimStatus::Assumed;
        claim.scope = None;
        answer.claims = vec![claim.clone()];
        assert_eq!(
            answer.validate(&request),
            Err(AnswerValidationError::ScopedClaimMissingScope(
                "claim-1".into()
            ))
        );

        claim.status = ClaimStatus::ModelPrior;
        claim.sources = vec!["fabricated-source".into()];
        answer.claims = vec![claim.clone()];
        assert_eq!(
            answer.validate(&request),
            Err(AnswerValidationError::ModelPriorHasSource("claim-1".into()))
        );

        claim.status = ClaimStatus::Conflicting;
        claim.sources = vec!["source-a".into()];
        answer.claims = vec![claim.clone()];
        assert_eq!(
            answer.validate(&request),
            Err(AnswerValidationError::ConflictingClaimNeedsTwoSources(
                "claim-1".into()
            ))
        );
    }

    #[test]
    fn evidence_references_are_unique_and_declared_by_claims() {
        let request = TaskRequest::from_objective("answer", PathBuf::from("."));
        let claim = Claim {
            id: "claim-1".into(),
            text: "claim".into(),
            status: ClaimStatus::Verified,
            sources: vec!["source-a".into()],
            scope: None,
        };
        let mut answer = AgentAnswer {
            status: AnswerStatus::Completed,
            conclusion: "answer".into(),
            claims: vec![claim],
            evidence: vec![Evidence {
                source: "source-a".into(),
                statement: "evidence".into(),
                claim_ids: vec!["claim-1".into(), "claim-1".into()],
            }],
            assumptions: Vec::new(),
            constraints: Vec::new(),
            unknowns: Vec::new(),
            flip_conditions: Vec::new(),
            action_receipts: Vec::new(),
        };
        assert_eq!(
            answer.validate(&request),
            Err(AnswerValidationError::DuplicateEvidenceClaimReference(
                "claim-1".into()
            ))
        );

        answer.evidence[0].claim_ids = vec!["claim-1".into()];
        answer.evidence[0].source = "source-b".into();
        assert_eq!(
            answer.validate(&request),
            Err(AnswerValidationError::EvidenceSourceNotDeclared {
                evidence_source: "source-b".into(),
                claim_id: "claim-1".into(),
            })
        );
    }

    #[test]
    fn high_impact_unknown_requires_reduction_method() {
        let request = TaskRequest::from_objective("answer", PathBuf::from("."));
        let mut answer = AgentAnswer {
            status: AnswerStatus::NeedsEvidence,
            conclusion: "need evidence".into(),
            claims: Vec::new(),
            evidence: Vec::new(),
            assumptions: Vec::new(),
            constraints: Vec::new(),
            unknowns: vec![UnknownItem {
                description: "critical missing input".into(),
                impact: ImpactLevel::High,
                why_it_matters: Some("it can flip the decision".into()),
                reduction_method: None,
            }],
            flip_conditions: Vec::new(),
            action_receipts: Vec::new(),
        };
        assert_eq!(
            answer.validate(&request),
            Err(AnswerValidationError::HighImpactUnknownWithoutReduction(
                "critical missing input".into()
            ))
        );
        answer.unknowns[0].reduction_method = Some("request the missing input".into());
        answer.validate(&request).unwrap();
    }

    #[test]
    fn assumptions_are_structured_and_high_impact_requires_verification() {
        let mut answer: AgentAnswer = serde_json::from_value(serde_json::json!({
            "status": "conditional",
            "conclusion": "conditional",
            "claims": [],
            "evidence": [],
            "assumptions": ["legacy assumption"],
            "constraints": [],
            "unknowns": [],
            "flip_conditions": [],
            "action_receipts": []
        }))
        .unwrap();
        assert_eq!(answer.assumptions[0].description, "legacy assumption");
        assert_eq!(answer.assumptions[0].impact, ImpactLevel::Medium);

        answer.assumptions[0].impact = ImpactLevel::High;
        answer.assumptions[0].verification_method = None;
        let request = TaskRequest::from_objective("answer", PathBuf::from("."));
        assert_eq!(
            answer.validate(&request),
            Err(
                AnswerValidationError::HighImpactAssumptionWithoutVerification(
                    "legacy assumption".into()
                )
            )
        );
        answer.assumptions[0].verification_method = Some("verify against current data".into());
        answer.validate(&request).unwrap();
    }
}
