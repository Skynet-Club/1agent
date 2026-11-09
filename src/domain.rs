use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PROTOCOL_VERSION: u32 = 1;
pub const LEGACY_ANSWER_SCHEMA_VERSION: u32 = 1;
pub const ANSWER_SCHEMA_VERSION: u32 = 2;
pub const MAX_PROVIDER_RETRY_BACKOFF_MS: u64 = 1_000;
pub const MAX_PROVIDER_RETRY_ELAPSED_MS: u64 = 2_000;

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
    pub evidence_max_age_ms: Option<u64>,
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
            evidence_max_age_ms: None,
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
    #[serde(default)]
    pub id: String,
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
    #[serde(default = "default_max_provider_attempts")]
    pub max_provider_attempts: usize,
    #[serde(default = "default_provider_retry_backoff_ms")]
    pub provider_retry_backoff_ms: u64,
    pub timeout_ms: u64,
    #[serde(default = "default_max_context_chars")]
    pub max_context_chars: usize,
    #[serde(default = "default_auto_compact_percent")]
    pub auto_compact_percent: u8,
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
            max_provider_attempts: 4,
            provider_retry_backoff_ms: 50,
            timeout_ms: 120_000,
            max_context_chars: 100_000,
            auto_compact_percent: default_auto_compact_percent(),
            max_provider_p95_latency_ms: None,
            max_unit_cost_micros_per_k: None,
        }
    }
}

fn default_max_context_chars() -> usize {
    100_000
}

fn default_auto_compact_percent() -> u8 {
    80
}

fn default_max_provider_attempts() -> usize {
    4
}

fn default_provider_retry_backoff_ms() -> u64 {
    50
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

impl CognitiveMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DirectAnswer => "direct_answer",
            Self::EvidenceResearch => "evidence_research",
            Self::DecisionAnalysis => "decision_analysis",
            Self::Execute => "execute",
            Self::Diagnosis => "diagnosis",
            Self::Escalate => "escalate",
        }
    }

    pub fn label_zh(self) -> &'static str {
        match self {
            Self::DirectAnswer => "直接回答",
            Self::EvidenceResearch => "证据研究",
            Self::DecisionAnalysis => "决策分析",
            Self::Execute => "执行任务",
            Self::Diagnosis => "诊断分析",
            Self::Escalate => "升级处理",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

impl RiskLevel {
    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Low => "低",
            Self::Medium => "中等",
            Self::High => "高",
        }
    }
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
    pub observed_unix_ms: Option<u128>,
    #[serde(default)]
    pub expires_unix_ms: Option<u128>,
    #[serde(default)]
    pub claim_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FlipCondition {
    pub if_observed: String,
    pub then: String,
    #[serde(default)]
    pub target_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AssumptionItem {
    pub description: String,
    pub impact: ImpactLevel,
    pub verification_method: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConflictStatus {
    Unresolved,
    Resolved,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ConflictRecord {
    pub id: String,
    pub claim_ids: [String; 2],
    pub status: ConflictStatus,
    #[serde(default)]
    pub resolution: Option<String>,
    #[serde(default)]
    pub resolution_sources: Vec<String>,
    #[serde(default)]
    pub resolution_claim_ids: Vec<String>,
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
    #[serde(default = "default_answer_schema_version")]
    pub schema_version: u32,
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
    pub conflicts: Vec<ConflictRecord>,
    #[serde(default)]
    pub flip_conditions: Vec<FlipCondition>,
    #[serde(default)]
    pub action_receipts: Vec<String>,
}

impl AgentAnswer {
    pub fn validate(&self, request: &TaskRequest) -> Result<(), AnswerValidationError> {
        if !matches!(
            self.schema_version,
            LEGACY_ANSWER_SCHEMA_VERSION | ANSWER_SCHEMA_VERSION
        ) {
            return Err(AnswerValidationError::UnsupportedAnswerVersion);
        }
        if self.conclusion.trim().is_empty() {
            return Err(AnswerValidationError::EmptyConclusion);
        }

        if !request.permissions.allow_write && !self.action_receipts.is_empty() {
            return Err(AnswerValidationError::UnauthorizedActionReceipt);
        }
        let claim_ids: std::collections::BTreeSet<_> =
            self.claims.iter().map(|claim| claim.id.as_str()).collect();
        let unknown_ids = self
            .unknowns
            .iter()
            .map(|unknown| unknown.id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        if claim_ids.len() != self.claims.len() {
            return Err(AnswerValidationError::DuplicateClaimId);
        }
        if self.schema_version == ANSWER_SCHEMA_VERSION && unknown_ids.len() != self.unknowns.len()
        {
            return Err(AnswerValidationError::DuplicateUnknownId);
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
            if self.schema_version == ANSWER_SCHEMA_VERSION && unknown.id.trim().is_empty() {
                return Err(AnswerValidationError::UnknownWithoutId);
            }
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
        if self.schema_version == ANSWER_SCHEMA_VERSION {
            if self.status == AnswerStatus::Conditional && self.flip_conditions.is_empty() {
                return Err(AnswerValidationError::ConditionalWithoutFlipCondition);
            }
            if self.status == AnswerStatus::Completed
                && self
                    .assumptions
                    .iter()
                    .any(|assumption| assumption.impact == ImpactLevel::High)
            {
                return Err(AnswerValidationError::CompletedWithHighImpactAssumption);
            }
            for flip_condition in &self.flip_conditions {
                if flip_condition.if_observed.trim().is_empty()
                    || flip_condition.then.trim().is_empty()
                {
                    return Err(AnswerValidationError::EmptyFlipCondition);
                }
                if flip_condition.target_ids.is_empty() {
                    return Err(AnswerValidationError::FlipConditionWithoutTarget);
                }
                let mut targets = std::collections::BTreeSet::new();
                for target_id in &flip_condition.target_ids {
                    if !targets.insert(target_id.as_str()) {
                        return Err(AnswerValidationError::DuplicateFlipConditionTarget(
                            target_id.clone(),
                        ));
                    }
                    if !claim_ids.contains(target_id.as_str())
                        && !unknown_ids.contains(target_id.as_str())
                    {
                        return Err(AnswerValidationError::UnknownFlipConditionTarget(
                            target_id.clone(),
                        ));
                    }
                }
            }
        }
        let conflict_ids = self
            .conflicts
            .iter()
            .map(|conflict| conflict.id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        if conflict_ids.len() != self.conflicts.len() {
            return Err(AnswerValidationError::DuplicateConflictId);
        }
        for conflict in &self.conflicts {
            if conflict.id.trim().is_empty()
                || conflict.claim_ids[0] == conflict.claim_ids[1]
                || conflict
                    .claim_ids
                    .iter()
                    .any(|claim_id| !claim_ids.contains(claim_id.as_str()))
            {
                return Err(AnswerValidationError::InvalidConflictClaimReference(
                    conflict.id.clone(),
                ));
            }
            match conflict.status {
                ConflictStatus::Unresolved => {
                    if conflict.resolution.is_some() || !conflict.resolution_sources.is_empty() {
                        return Err(AnswerValidationError::UnresolvedConflictHasResolution(
                            conflict.id.clone(),
                        ));
                    }
                }
                ConflictStatus::Resolved => {
                    if conflict
                        .resolution
                        .as_deref()
                        .map(str::trim)
                        .unwrap_or_default()
                        .is_empty()
                        || !conflict
                            .resolution_sources
                            .iter()
                            .any(|source| !source.trim().is_empty())
                    {
                        return Err(AnswerValidationError::ResolvedConflictWithoutEvidence(
                            conflict.id.clone(),
                        ));
                    }
                    if conflict.claim_ids.iter().any(|claim_id| {
                        self.claims
                            .iter()
                            .find(|claim| claim.id == *claim_id)
                            .is_some_and(|claim| claim.status == ClaimStatus::Conflicting)
                    }) {
                        return Err(AnswerValidationError::ResolvedConflictStillMarked(
                            conflict.id.clone(),
                        ));
                    }
                    let mut resolution_claim_ids = conflict
                        .claim_ids
                        .iter()
                        .map(String::as_str)
                        .collect::<std::collections::BTreeSet<_>>();
                    for claim_id in &conflict.resolution_claim_ids {
                        if !claim_ids.contains(claim_id.as_str())
                            || resolution_claim_ids.contains(claim_id.as_str())
                        {
                            return Err(AnswerValidationError::InvalidConflictClaimReference(
                                conflict.id.clone(),
                            ));
                        }
                        resolution_claim_ids.insert(claim_id);
                    }
                    let declared_sources = self
                        .claims
                        .iter()
                        .filter(|claim| resolution_claim_ids.contains(claim.id.as_str()))
                        .flat_map(|claim| claim.sources.iter())
                        .collect::<std::collections::BTreeSet<_>>();
                    if conflict
                        .resolution_sources
                        .iter()
                        .any(|source| !declared_sources.contains(source))
                    {
                        return Err(AnswerValidationError::ResolutionSourceNotDeclared(
                            conflict.id.clone(),
                        ));
                    }
                }
            }
        }
        for claim in self
            .claims
            .iter()
            .filter(|claim| claim.status == ClaimStatus::Conflicting)
        {
            if !self.conflicts.iter().any(|conflict| {
                conflict.status == ConflictStatus::Unresolved
                    && conflict.claim_ids.contains(&claim.id)
            }) {
                return Err(AnswerValidationError::ConflictingClaimWithoutOpenConflict(
                    claim.id.clone(),
                ));
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
            if let (Some(observed), Some(expires)) =
                (evidence.observed_unix_ms, evidence.expires_unix_ms)
                && expires <= observed
            {
                return Err(AnswerValidationError::InvalidEvidenceWindow);
            }
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
                if claim.status == ClaimStatus::Verified {
                    if evidence
                        .expires_unix_ms
                        .is_some_and(|expires| expires <= now_unix_ms())
                    {
                        return Err(AnswerValidationError::ExpiredVerifiedEvidence(
                            evidence.source.clone(),
                        ));
                    }
                    if let Some(max_age_ms) = request.evidence_max_age_ms {
                        let Some(observed) = evidence.observed_unix_ms else {
                            return Err(AnswerValidationError::MissingEvidenceTimestamp(
                                evidence.source.clone(),
                            ));
                        };
                        let age_ms = now_unix_ms().saturating_sub(observed);
                        if age_ms > u128::from(max_age_ms) {
                            return Err(AnswerValidationError::StaleEvidenceForPolicy(
                                evidence.source.clone(),
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AnswerValidationError {
    #[error("unsupported answer schema version")]
    UnsupportedAnswerVersion,
    #[error("answer conclusion must not be empty")]
    EmptyConclusion,
    #[error("read-only task reported an external action receipt")]
    UnauthorizedActionReceipt,
    #[error("answer contains duplicate claim ids")]
    DuplicateClaimId,
    #[error("answer contains duplicate unknown ids")]
    DuplicateUnknownId,
    #[error("unknown item must have a non-empty id")]
    UnknownWithoutId,
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
    #[error("conditional answer requires at least one flip condition")]
    ConditionalWithoutFlipCondition,
    #[error("completed answer cannot rely on a high-impact assumption")]
    CompletedWithHighImpactAssumption,
    #[error("flip condition must have a non-empty observation and consequence")]
    EmptyFlipCondition,
    #[error("flip condition must reference at least one claim or unknown")]
    FlipConditionWithoutTarget,
    #[error("flip condition contains duplicate target id '{0}'")]
    DuplicateFlipConditionTarget(String),
    #[error("flip condition references undeclared target id '{0}'")]
    UnknownFlipConditionTarget(String),
    #[error("answer contains duplicate conflict ids")]
    DuplicateConflictId,
    #[error("conflict '{0}' has invalid or duplicate claim references")]
    InvalidConflictClaimReference(String),
    #[error("unresolved conflict '{0}' must not carry resolution evidence")]
    UnresolvedConflictHasResolution(String),
    #[error("resolved conflict '{0}' requires a resolution and source")]
    ResolvedConflictWithoutEvidence(String),
    #[error("resolved conflict '{0}' still contains a conflicting claim")]
    ResolvedConflictStillMarked(String),
    #[error("conflicting claim '{0}' has no open conflict record")]
    ConflictingClaimWithoutOpenConflict(String),
    #[error("conflict '{0}' uses an undeclared resolution source")]
    ResolutionSourceNotDeclared(String),
    #[error("evidence expiration must be later than observation")]
    InvalidEvidenceWindow,
    #[error("expired evidence '{0}' cannot support a verified claim")]
    ExpiredVerifiedEvidence(String),
    #[error("evidence '{0}' has no observation timestamp under freshness policy")]
    MissingEvidenceTimestamp(String),
    #[error("evidence '{0}' exceeds the task freshness limit")]
    StaleEvidenceForPolicy(String),
}

fn default_answer_schema_version() -> u32 {
    LEGACY_ANSWER_SCHEMA_VERSION
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
    InvalidFinal { content: String, error: String },
    ToolCalls { calls: Vec<ToolCall> },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_micros: u64,
    /// Prompt tokens served from a provider cache, when the provider reports it.
    #[serde(default)]
    pub cache_hit_tokens: u64,
    /// Prompt tokens that missed the provider cache. Zero with a zero hit count
    /// means the provider did not report cache statistics.
    #[serde(default)]
    pub cache_miss_tokens: u64,
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
    #[serde(default)]
    pub provider_attempts_used: usize,
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

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
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
    fn cognitive_mode_labels_match_serialized_contract() {
        for mode in [
            CognitiveMode::DirectAnswer,
            CognitiveMode::EvidenceResearch,
            CognitiveMode::DecisionAnalysis,
            CognitiveMode::Execute,
            CognitiveMode::Diagnosis,
            CognitiveMode::Escalate,
        ] {
            assert_eq!(
                serde_json::to_value(mode).unwrap(),
                serde_json::Value::String(mode.as_str().into())
            );
        }
    }

    #[test]
    fn answer_schema_version_is_explicit_and_legacy_json_is_version_one() {
        let request = TaskRequest::from_objective("answer", PathBuf::from("."));
        let answer: AgentAnswer = serde_json::from_value(serde_json::json!({
            "status": "completed",
            "conclusion": "legacy answer",
        }))
        .unwrap();

        assert_eq!(answer.schema_version, LEGACY_ANSWER_SCHEMA_VERSION);
        answer.validate(&request).unwrap();
        let serialized = serde_json::to_value(answer).unwrap();
        assert_eq!(serialized["schema_version"], LEGACY_ANSWER_SCHEMA_VERSION);
    }

    #[test]
    fn unsupported_answer_schema_version_fails_closed() {
        let request = TaskRequest::from_objective("answer", PathBuf::from("."));
        let mut answer: AgentAnswer = serde_json::from_value(serde_json::json!({
            "schema_version": ANSWER_SCHEMA_VERSION + 1,
            "status": "completed",
            "conclusion": "future answer",
        }))
        .unwrap();

        assert_eq!(
            answer.validate(&request),
            Err(AnswerValidationError::UnsupportedAnswerVersion)
        );
        answer.schema_version = ANSWER_SCHEMA_VERSION;
        answer.validate(&request).unwrap();
    }

    #[test]
    fn decision_completeness_requires_and_binds_flip_conditions() {
        let request = TaskRequest::from_objective("answer", PathBuf::from("."));
        let mut answer: AgentAnswer = serde_json::from_value(serde_json::json!({
            "schema_version": ANSWER_SCHEMA_VERSION,
            "status": "conditional",
            "conclusion": "use A if cost remains lower",
            "claims": [],
            "evidence": [],
            "assumptions": [],
            "constraints": [],
            "unknowns": [{
                "id": "current-cost",
                "description": "future cost of A",
                "impact": "high",
                "reduction_method": "obtain a current quote"
            }],
            "flip_conditions": [{
                "if_observed": "B becomes cheaper",
                "then": "switch to B",
                "target_ids": ["current-cost"]
            }]
        }))
        .unwrap();
        answer.validate(&request).unwrap();

        answer.flip_conditions[0].target_ids = vec!["missing".into()];
        assert_eq!(
            answer.validate(&request),
            Err(AnswerValidationError::UnknownFlipConditionTarget(
                "missing".into()
            ))
        );

        answer.flip_conditions.clear();
        assert_eq!(
            answer.validate(&request),
            Err(AnswerValidationError::ConditionalWithoutFlipCondition)
        );

        answer.status = AnswerStatus::Completed;
        answer.assumptions = vec![AssumptionItem {
            description: "current cost holds".into(),
            impact: ImpactLevel::High,
            verification_method: Some("refresh the quote".into()),
        }];
        assert_eq!(
            answer.validate(&request),
            Err(AnswerValidationError::CompletedWithHighImpactAssumption)
        );
    }

    #[test]
    fn read_only_answer_rejects_action_receipts() {
        let request = TaskRequest::from_objective("answer", PathBuf::from("."));
        let answer = AgentAnswer {
            schema_version: ANSWER_SCHEMA_VERSION,
            status: AnswerStatus::Completed,
            conclusion: "done".into(),
            claims: Vec::new(),
            evidence: Vec::new(),
            assumptions: Vec::new(),
            constraints: Vec::new(),
            unknowns: Vec::new(),
            conflicts: Vec::new(),
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
            schema_version: ANSWER_SCHEMA_VERSION,
            status: AnswerStatus::Completed,
            conclusion: "done".into(),
            claims: Vec::new(),
            evidence: vec![Evidence {
                source: "fixture".into(),
                statement: "verified".into(),
                observed_unix_ms: None,
                expires_unix_ms: None,
                claim_ids: vec!["missing".into()],
            }],
            assumptions: Vec::new(),
            constraints: Vec::new(),
            unknowns: Vec::new(),
            conflicts: Vec::new(),
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
            schema_version: ANSWER_SCHEMA_VERSION,
            status: AnswerStatus::Completed,
            conclusion: "answer".into(),
            claims: vec![claim.clone()],
            evidence: Vec::new(),
            assumptions: Vec::new(),
            constraints: Vec::new(),
            unknowns: Vec::new(),
            conflicts: Vec::new(),
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
            schema_version: ANSWER_SCHEMA_VERSION,
            status: AnswerStatus::Completed,
            conclusion: "answer".into(),
            claims: vec![claim],
            evidence: vec![Evidence {
                source: "source-a".into(),
                statement: "evidence".into(),
                observed_unix_ms: None,
                expires_unix_ms: None,
                claim_ids: vec!["claim-1".into(), "claim-1".into()],
            }],
            assumptions: Vec::new(),
            constraints: Vec::new(),
            unknowns: Vec::new(),
            conflicts: Vec::new(),
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
            schema_version: ANSWER_SCHEMA_VERSION,
            status: AnswerStatus::NeedsEvidence,
            conclusion: "need evidence".into(),
            claims: Vec::new(),
            evidence: Vec::new(),
            assumptions: Vec::new(),
            constraints: Vec::new(),
            unknowns: vec![UnknownItem {
                id: "critical-input".into(),
                description: "critical missing input".into(),
                impact: ImpactLevel::High,
                why_it_matters: Some("it can flip the decision".into()),
                reduction_method: None,
            }],
            conflicts: Vec::new(),
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
            "schema_version": ANSWER_SCHEMA_VERSION,
            "status": "conditional",
            "conclusion": "conditional",
            "claims": [],
            "evidence": [],
            "assumptions": ["legacy assumption"],
            "constraints": [],
            "unknowns": [{
                "id": "assumption-check",
                "description": "whether the assumption holds",
                "impact": "medium"
            }],
            "flip_conditions": [{
                "if_observed": "current data invalidates the assumption",
                "then": "stop relying on the conditional conclusion",
                "target_ids": ["assumption-check"]
            }],
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

    #[test]
    fn conflicts_require_two_sides_and_explicit_resolution_state() {
        let request = TaskRequest::from_objective("answer", PathBuf::from("."));
        let claims = vec![
            Claim {
                id: "claim-a".into(),
                text: "claim A".into(),
                status: ClaimStatus::Verified,
                sources: vec!["source-a".into()],
                scope: None,
            },
            Claim {
                id: "claim-b".into(),
                text: "claim B".into(),
                status: ClaimStatus::Verified,
                sources: vec!["source-b".into()],
                scope: None,
            },
        ];
        let mut answer = AgentAnswer {
            schema_version: ANSWER_SCHEMA_VERSION,
            status: AnswerStatus::Conditional,
            conclusion: "conflict".into(),
            claims,
            evidence: Vec::new(),
            assumptions: Vec::new(),
            constraints: Vec::new(),
            unknowns: Vec::new(),
            conflicts: vec![ConflictRecord {
                id: "conflict-1".into(),
                claim_ids: ["claim-a".into(), "claim-b".into()],
                status: ConflictStatus::Unresolved,
                resolution: None,
                resolution_sources: Vec::new(),
                resolution_claim_ids: Vec::new(),
            }],
            flip_conditions: vec![FlipCondition {
                if_observed: "new evidence resolves the conflict".into(),
                then: "prefer the better-supported claim".into(),
                target_ids: vec!["claim-a".into()],
            }],
            action_receipts: Vec::new(),
        };
        answer.validate(&request).unwrap();
        answer.conflicts[0].status = ConflictStatus::Resolved;
        assert!(matches!(
            answer.validate(&request),
            Err(AnswerValidationError::ResolvedConflictWithoutEvidence(_))
        ));

        answer.conflicts[0].resolution = Some("source-c resolves the conflict".into());
        answer.conflicts[0].resolution_sources = vec!["source-c".into()];
        assert!(matches!(
            answer.validate(&request),
            Err(AnswerValidationError::ResolutionSourceNotDeclared(_))
        ));
        answer.conflicts[0].resolution_sources = vec!["source-a".into()];
        answer.claims[0].status = ClaimStatus::Conflicting;
        answer.claims[0].sources = vec!["source-a".into(), "source-b".into()];
        assert!(matches!(
            answer.validate(&request),
            Err(AnswerValidationError::ResolvedConflictStillMarked(_))
        ));
    }

    #[test]
    fn evidence_freshness_blocks_stale_and_untimed_verified_claims() {
        let now = now_unix_ms();
        let mut request = TaskRequest::from_objective("answer", PathBuf::from("."));
        request.evidence_max_age_ms = Some(1_000);
        let mut answer = AgentAnswer {
            schema_version: ANSWER_SCHEMA_VERSION,
            status: AnswerStatus::Completed,
            conclusion: "answer".into(),
            claims: vec![Claim {
                id: "claim-1".into(),
                text: "fresh claim".into(),
                status: ClaimStatus::Verified,
                sources: vec!["source-a".into()],
                scope: None,
            }],
            evidence: vec![Evidence {
                source: "source-a".into(),
                statement: "evidence".into(),
                observed_unix_ms: None,
                expires_unix_ms: None,
                claim_ids: vec!["claim-1".into()],
            }],
            assumptions: Vec::new(),
            constraints: Vec::new(),
            unknowns: Vec::new(),
            conflicts: Vec::new(),
            flip_conditions: Vec::new(),
            action_receipts: Vec::new(),
        };
        assert!(matches!(
            answer.validate(&request),
            Err(AnswerValidationError::MissingEvidenceTimestamp(_))
        ));

        answer.evidence[0].observed_unix_ms = Some(now.saturating_sub(2_000));
        answer.evidence[0].expires_unix_ms = Some(now + 10_000);
        assert!(matches!(
            answer.validate(&request),
            Err(AnswerValidationError::StaleEvidenceForPolicy(_))
        ));

        answer.evidence[0].observed_unix_ms = Some(now.saturating_sub(100));
        answer.evidence[0].expires_unix_ms = Some(now.saturating_sub(1));
        assert!(matches!(
            answer.validate(&request),
            Err(AnswerValidationError::ExpiredVerifiedEvidence(_))
        ));

        answer.evidence[0].expires_unix_ms = Some(now + 10_000);
        answer.validate(&request).unwrap();
    }
}
