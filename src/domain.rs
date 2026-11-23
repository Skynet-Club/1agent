use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const DEFAULT_MAX_CONTEXT_TOKENS: usize = 1_000_000;
pub const DEFAULT_AUTO_COMPACT_TOKENS: usize = 786_432;

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
#[serde(from = "RunBudgetWire")]
pub struct RunBudget {
    /// Maximum model/tool steps. `0` means no step limit.
    pub max_steps: usize,
    /// Maximum tool calls across the run. `0` means no tool-call limit.
    pub max_tool_calls: usize,
    /// Maximum provider attempts including fallbacks. `0` means no limit.
    pub max_provider_attempts: usize,
    pub provider_retry_backoff_ms: u64,
    /// Wall-clock limit for the whole run. `0` means no run timeout.
    pub timeout_ms: u64,
    pub max_context_tokens: usize,
    pub auto_compact_tokens: usize,
    pub max_provider_p95_latency_ms: Option<u64>,
    pub max_unit_cost_micros_per_k: Option<u64>,
    /// Maximum accumulated provider cost for the run.
    pub max_cost_micros: Option<u64>,
    /// Maximum accumulated input+output tokens for the run.
    pub max_tokens: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RunBudgetWire {
    max_steps: usize,
    max_tool_calls: usize,
    #[serde(default = "default_max_provider_attempts")]
    max_provider_attempts: usize,
    #[serde(default = "default_provider_retry_backoff_ms")]
    provider_retry_backoff_ms: u64,
    timeout_ms: u64,
    #[serde(default)]
    max_context_tokens: Option<usize>,
    #[serde(default)]
    auto_compact_tokens: Option<usize>,
    #[serde(default)]
    max_context_chars: Option<usize>,
    #[serde(default)]
    auto_compact_percent: Option<u8>,
    #[serde(default)]
    max_provider_p95_latency_ms: Option<u64>,
    #[serde(default)]
    max_unit_cost_micros_per_k: Option<u64>,
    #[serde(default)]
    max_cost_micros: Option<u64>,
    #[serde(default)]
    max_tokens: Option<u64>,
}

impl From<RunBudgetWire> for RunBudget {
    fn from(wire: RunBudgetWire) -> Self {
        let max_context_tokens =
            migrate_max_context_tokens(wire.max_context_tokens, wire.max_context_chars);
        let auto_compact_tokens = migrate_auto_compact_tokens(
            wire.auto_compact_tokens,
            wire.auto_compact_percent,
            max_context_tokens,
        );
        Self {
            max_steps: wire.max_steps,
            max_tool_calls: wire.max_tool_calls,
            max_provider_attempts: wire.max_provider_attempts,
            provider_retry_backoff_ms: wire.provider_retry_backoff_ms,
            timeout_ms: wire.timeout_ms,
            max_context_tokens,
            auto_compact_tokens,
            max_provider_p95_latency_ms: wire.max_provider_p95_latency_ms,
            max_unit_cost_micros_per_k: wire.max_unit_cost_micros_per_k,
            max_cost_micros: wire.max_cost_micros,
            max_tokens: wire.max_tokens,
        }
    }
}

impl Default for RunBudget {
    fn default() -> Self {
        Self {
            max_steps: 6,
            max_tool_calls: 8,
            max_provider_attempts: 8,
            provider_retry_backoff_ms: 50,
            timeout_ms: 120_000,
            max_context_tokens: DEFAULT_MAX_CONTEXT_TOKENS,
            auto_compact_tokens: DEFAULT_AUTO_COMPACT_TOKENS,
            max_provider_p95_latency_ms: None,
            max_unit_cost_micros_per_k: None,
            max_cost_micros: None,
            max_tokens: None,
        }
    }
}

pub(crate) fn migrate_max_context_tokens(new: Option<usize>, legacy_chars: Option<usize>) -> usize {
    new.or_else(|| {
        legacy_chars.map(|chars| {
            // Preserve the old default as the new aligned default; custom
            // character budgets are converted conservatively (~3 chars/token).
            if chars == 100_000 {
                DEFAULT_MAX_CONTEXT_TOKENS
            } else {
                chars.div_ceil(3).clamp(1_000, DEFAULT_MAX_CONTEXT_TOKENS)
            }
        })
    })
    .unwrap_or(DEFAULT_MAX_CONTEXT_TOKENS)
    .clamp(1_000, DEFAULT_MAX_CONTEXT_TOKENS)
}

pub(crate) fn migrate_auto_compact_tokens(
    new: Option<usize>,
    legacy_percent: Option<u8>,
    max_context_tokens: usize,
) -> usize {
    let fallback =
        max_context_tokens.saturating_mul(DEFAULT_AUTO_COMPACT_TOKENS) / DEFAULT_MAX_CONTEXT_TOKENS;
    new.or_else(|| {
        legacy_percent.map(|percent| {
            if percent == 80 && max_context_tokens == DEFAULT_MAX_CONTEXT_TOKENS {
                DEFAULT_AUTO_COMPACT_TOKENS
            } else {
                max_context_tokens.saturating_mul(usize::from(percent)) / 100
            }
        })
    })
    .unwrap_or(fallback)
    .clamp(1, max_context_tokens)
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
    pub observed_unix_ms: Option<u64>,
    #[serde(default)]
    pub expires_unix_ms: Option<u64>,
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

/// How much a task depends on facts that can change, local state or an action.
/// The model fills this in; the runtime uses it for evidence verification and
/// for deciding whether the tool-free chat path was the wrong choice.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum NeedLevel {
    #[default]
    None,
    Low,
    Medium,
    High,
}

impl NeedLevel {
    pub fn requires_evidence(self) -> bool {
        self >= Self::Medium
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TaskNeed {
    #[serde(default)]
    pub freshness: NeedLevel,
    #[serde(default)]
    pub locality: NeedLevel,
    #[serde(default)]
    pub action: NeedLevel,
    #[serde(default = "default_risk_level")]
    pub risk: RiskLevel,
    #[serde(default)]
    pub ambiguity: NeedLevel,
    #[serde(default)]
    pub budget: NeedLevel,
}

fn default_risk_level() -> RiskLevel {
    RiskLevel::Low
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_need: Option<TaskNeed>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnswerBrief {
    pub summary: String,
    pub details: Vec<String>,
}

impl AgentAnswer {
    /// Backward-compatible answer contract: the first two sentences are the
    /// brief; the rest stays available in details. No new required JSON field,
    /// so existing journals and 1.0 callers keep parsing.
    pub fn brief(&self) -> AnswerBrief {
        let mut sentences = answer_sentences(&self.conclusion);
        if sentences.is_empty() {
            return AnswerBrief {
                summary: String::new(),
                details: Vec::new(),
            };
        }
        let mut summary = String::new();
        let mut consumed = 0;
        for sentence in &sentences {
            if consumed >= 2 || summary.chars().count() + sentence.chars().count() > 280 {
                break;
            }
            if !summary.is_empty() {
                summary.push(' ');
            }
            summary.push_str(sentence);
            consumed += 1;
        }
        if summary.is_empty() {
            summary = sentences[0].chars().take(280).collect::<String>();
            if sentences[0].chars().count() > 280 {
                summary.push('…');
            }
            consumed = 1;
        }
        let details = sentences.split_off(consumed.min(sentences.len()));
        AnswerBrief {
            summary,
            details: details.into_iter().take(8).collect(),
        }
    }

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
                    // A resolution may name one of the conflicting claims as
                    // the winner (for example "the older refusal was wrong;
                    // claim c1 is the record"). Unknown ids and repeats are
                    // still rejected.
                    let mut resolution_claim_ids = std::collections::BTreeSet::new();
                    for claim_id in &conflict.resolution_claim_ids {
                        if !claim_ids.contains(claim_id.as_str())
                            || !resolution_claim_ids.insert(claim_id.as_str())
                        {
                            return Err(AnswerValidationError::InvalidConflictClaimReference(
                                conflict.id.clone(),
                            ));
                        }
                    }
                    let mut declared_claim_ids = conflict
                        .claim_ids
                        .iter()
                        .map(String::as_str)
                        .collect::<std::collections::BTreeSet<_>>();
                    declared_claim_ids.extend(resolution_claim_ids.iter().copied());
                    let declared_sources = self
                        .claims
                        .iter()
                        .filter(|claim| declared_claim_ids.contains(claim.id.as_str()))
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
                        .is_some_and(|expires| u128::from(expires) <= now_unix_ms())
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
                        let age_ms = now_unix_ms().saturating_sub(u128::from(observed));
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

pub fn has_fenced_code(text: &str) -> bool {
    text.lines()
        .any(|line| line.trim_start().starts_with("```"))
}

fn answer_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut start = 0;
    for (index, character) in text.char_indices() {
        if matches!(character, '。' | '！' | '？' | '；' | ';' | '!' | '?') {
            let end = index + character.len_utf8();
            let sentence = text[start..end].trim();
            if !sentence.is_empty() {
                sentences.push(sentence.to_string());
            }
            start = end;
        }
    }
    let rest = text[start..].trim();
    if !rest.is_empty() {
        sentences.push(rest.to_string());
    }
    sentences
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
    /// Whether this run used the fast delivery path. Resume must rebuild the
    /// same tool surface, so the decision is stored instead of re-derived.
    #[serde(default)]
    pub fast_mode: bool,
}

/// Checkpoint contents appended since the previous checkpoint.
///
/// `base_seq` must be the seq of the event that established the base
/// checkpoint (a full `Checkpoint`, `CheckpointDelta`, or `ResumeStarted`).
/// Resume reconstructs the full message history by applying deltas in order.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCheckpointDelta {
    pub base_seq: u64,
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

/// A question about a concept or consequence is not an instruction to perform
/// the action it mentions. "BufWriter 不 flush 会怎样" and "追加写入的区别"
/// must not be short-circuited by the read-only authorization path.
fn looks_conceptual(text: &str) -> bool {
    let trimmed = text.trim_end();
    let question_shape = trimmed.ends_with('?')
        || trimmed.ends_with('？')
        || contains_any(
            text,
            &[
                "什么是",
                "是什么",
                "为什么",
                "区别",
                "差异",
                "原理",
                "机制",
                "后果",
                "解释",
                "怎么理解",
                "如何理解",
                "what is",
                "why ",
                "difference",
                "explain",
            ],
        );
    let imperative = contains_any(
        text,
        &[
            "帮我",
            "替我",
            "给我",
            "请帮我",
            "立刻",
            "马上",
            "现在就",
            "do it",
            "go ahead",
            "please ",
        ],
    );
    question_shape && !imperative
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
    if !looks_conceptual(&text)
        && (contains_any_word(&text, &["execute", "write", "delete", "send", "buy"])
            || contains_any(
                &text,
                &[
                    "执行", "写入", "删除", "删了", "删掉", "清空", "清理", "移除", "发送", "购买",
                ],
            ))
    {
        return CognitiveMode::Execute;
    }
    if !request.unknowns.is_empty()
        || contains_any(
            &text,
            &["search", "evidence", "source", "检索", "证据", "来源"],
        )
    {
        return CognitiveMode::EvidenceResearch;
    }
    CognitiveMode::DirectAnswer
}

/// Whether a task is conversational enough for the one-call chat fast path.
/// This is deliberately conservative: an explicit action, an imperative, or a
/// continuation of earlier work stays on the full agent loop.
pub fn looks_like_chat(objective: &str) -> bool {
    let text = objective.trim();
    let length = text.chars().count();
    if length == 0 || length > 120 {
        return false;
    }
    let lower = text.to_lowercase();

    if contains_any(
        &lower,
        &[
            "继续",
            "接着",
            "然后呢",
            "下一步",
            "go on",
            "continue",
            "keep going",
        ],
    ) {
        return false;
    }
    if contains_any(
        &lower,
        &[
            "帮我",
            "给我",
            "替我",
            "麻烦你",
            "please ",
            "go ahead",
            "do it",
        ],
    ) {
        return false;
    }
    // A question word does not make a task conversational. Retrieval, local
    // state, current facts and repository work must keep the tools and the
    // full evidence loop even when the sentence ends with "哪些" or "如何".
    if contains_any(
        &lower,
        &[
            "查一下",
            "查查",
            "搜一下",
            "搜索",
            "检索",
            "读取",
            "写入",
            "执行",
            "运行",
            "安装",
            "下载",
            "上传",
            "部署",
            "重启",
            "删除",
            "创建",
            "修改",
            "编辑",
            "提交",
            "打开",
            "分析",
            "审查",
            "重构",
            "迁移",
            "优化",
            "实现",
            "设计",
            "计划",
            "报告",
            "项目",
            "架构",
            "代码",
            "仓库",
            "文件",
            "目录",
            "本地",
            "当前",
            "最新",
            "实时",
            "新闻",
            "大事件",
            "价格",
            "多少钱",
            "余额",
            "日志",
            "报错",
            "错误",
            "配置",
            "版本",
            "密钥",
            "数据库",
            "网址",
            "链接",
            "天气",
            "几点",
        ],
    ) || contains_any_word(
        &lower,
        &[
            "read",
            "show",
            "open",
            "write",
            "edit",
            "create",
            "delete",
            "remove",
            "run",
            "execute",
            "install",
            "build",
            "test",
            "search",
            "find",
            "download",
            "upload",
            "fetch",
            "git",
            "ls",
            "cat",
            "grep",
            "project",
            "architecture",
            "codebase",
            "repository",
            "file",
            "directory",
            "local",
            "current",
            "latest",
            "recent",
            "today",
            "real-time",
            "news",
            "price",
            "cost",
            "balance",
            "log",
            "error",
            "config",
            "version",
            "api",
            "database",
            "url",
            "link",
            "weather",
            "time",
        ],
    ) {
        return false;
    }
    if contains_any(
        &lower,
        &[
            "？",
            "?",
            "吗",
            "呢",
            "什么",
            "怎么",
            "为什么",
            "如何",
            "哪个",
            "哪些",
            "哪里",
            "哪儿",
            "在哪",
            "谁",
            "什么时候",
            "多久",
            "多少",
            "介绍",
            "解释",
            "区别",
            "是什么",
            "能不能",
            "可以吗",
        ],
    ) || contains_any_word(
        &lower,
        &[
            "who",
            "what",
            "why",
            "how",
            "which",
            "explain",
            "tell me",
            "difference",
        ],
    ) {
        return true;
    }
    if contains_any(
        &lower,
        &[
            "写",
            "读一下",
            "读这个",
            "读文件",
            "写下",
            "写个",
            "写一个",
            "写一下",
            "改一下",
            "改这个",
            "删掉",
            "查一下",
            "查查",
            "跑一下",
            "装一下",
            "装个",
            "建一个",
            "开一下",
            "关掉",
            "发给",
            "买个",
            "买一个",
            "读取",
            "打开",
            "查看",
            "写入",
            "修改",
            "编辑",
            "创建",
            "新建",
            "删除",
            "移除",
            "运行",
            "执行",
            "启动",
            "安装",
            "编译",
            "构建",
            "测试",
            "搜索",
            "查找",
            "下载",
            "上传",
            "拉取",
            "提交",
            "部署",
            "重启",
            "停止",
            "read ",
            "show ",
            "open ",
            "write ",
            "edit ",
            "create ",
            "delete ",
            "remove ",
            "run ",
            "exec",
            "install",
            "build",
            "test ",
            "search ",
            "find ",
            "download",
            "upload",
            "fetch",
            "git ",
            "ls ",
            "cat ",
            "grep ",
        ],
    ) {
        return false;
    }
    length <= 80
}

/// Conservative fallback for answers with no `task_need`: objectives that
/// clearly depend on facts which change. This only feeds the evidence check; it
/// is not the primary router and it does not select tools.
pub fn looks_like_current_facts(objective: &str) -> bool {
    let lower = objective.to_lowercase();
    contains_any(
        &lower,
        &[
            "今天",
            "今晚",
            "现在",
            "当前",
            "目前",
            "实时",
            "市值",
            "股价",
            "价格",
            "多少钱",
            "天气",
            "汇率",
            "财报",
            "涨了",
            "跌了",
            "发布了吗",
            "新闻",
            "大事件",
            "科技圈",
            "诺贝尔",
            "值得关注",
            "ceo",
            "market cap",
            "stock price",
            "weather",
            "exchange rate",
            "earnings",
            "today",
        ],
    )
}

/// Whether the user is asking about 1-Agent itself, not about a project or
/// another product. Capability questions skip the tool-free chat path so the
/// model can call `describe_self` instead of reciting a static catalog.
pub fn asks_about_capabilities(objective: &str) -> bool {
    let lower = objective.to_lowercase();
    let self_reference = contains_any(
        &lower,
        &[
            "你",
            "1-agent",
            "1agent",
            "super-agent",
            "super agent",
            "sagent",
            "s-agent",
        ],
    );
    let topic = contains_any(
        &lower,
        &[
            "功能",
            "能做什么",
            "能干什么",
            "会什么",
            "能力",
            "特点",
            "优势",
            "区别",
            "不同",
            "介绍",
            "是谁",
            "工具",
            "联网",
            "上网",
            "网络",
            "记忆",
            "审计",
            "回放",
            "成本",
            "预算",
            "限制",
            "平台",
            "windows",
            "macos",
            "linux",
            "模型",
            "权限",
            "终端",
            "命令",
            "文件",
            "技能",
            "mcp",
            "上下文",
            "日志",
            "证据",
            "安全",
            "隐私",
            "浏览器",
            "搜索",
            "任务",
            "多长",
            "部署",
            "安装",
            "插件",
            "连接器",
            "沙箱",
            "who are you",
            "what can you do",
            "capabilities",
            "features",
            "difference",
        ],
    );
    let question_shape = contains_any(
        &lower,
        &[
            "？",
            "?",
            "吗",
            "什么",
            "哪些",
            "怎么",
            "如何",
            "能不能",
            "能否",
            "支持",
            "是不是",
            "有没有",
            "多少",
            "多长",
            "介绍",
            "区别",
            "限制",
            "是谁",
            "who",
            "what",
            "how",
            "can you",
            "do you",
            "support",
        ],
    );
    if self_reference && topic && question_shape {
        return true;
    }
    contains_any(
        &lower,
        &[
            "介绍一下自己",
            "介绍下自己",
            "介绍你自己",
            "介绍下你自己",
            "你是谁",
            "回放",
            "怎么审计",
            "如何审计",
            "who are you",
            "what can you do",
        ],
    )
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

/// Word-boundary match for English cues, so "show" does not match "how" and
/// "sandwich" does not match "which".
fn contains_any_word(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| {
        haystack.match_indices(needle).any(|(start, _)| {
            let before = haystack[..start].chars().next_back();
            let after = haystack[start + needle.len()..].chars().next();
            !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
        })
    })
}

/// A follow-up that points back at earlier turns is safer on the full agent
/// path, where it can use tools and the complete evidence rules.
pub fn has_context_reference(objective: &str) -> bool {
    let lower = objective.to_lowercase();
    contains_any(
        &lower,
        &[
            "它", "这个", "那个", "该", "其", "上述", "刚才", "前面", "上面", "此",
        ],
    ) || contains_any_word(
        &lower,
        &["it", "its", "this", "that", "they", "them", "their"],
    )
}

/// True when the user is asking about an earlier conversation rather than
/// starting a fresh task. This only decides whether to retrieve cross-session
/// records; it is not evidence that a record exists.
pub fn asks_about_past_conversation(objective: &str) -> bool {
    let lower = objective.to_lowercase();
    let direct = contains_any(
        &lower,
        &[
            "昨天",
            "昨晚",
            "前天",
            "上次",
            "上回",
            "印象",
            "聊过",
            "聊的",
            "说过",
            "提到过",
            "记忆",
            "会话",
            "历史对话",
            "我们聊",
            "我们说过",
            "我们讨论过",
        ],
    );
    let remember = contains_any(&lower, &["记得", "还记"])
        && contains_any(&lower, &["你", "吗", "我们", "还"]);
    direct
        || remember
        || contains_any_word(
            &lower,
            &[
                "remember",
                "recall",
                "yesterday",
                "last time",
                "we talked",
                "we discussed",
                "previous conversation",
            ],
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_character_context_budgets_migrate_to_tokens() {
        let legacy: RunBudget = serde_json::from_value(serde_json::json!({
            "max_steps": 6,
            "max_tool_calls": 8,
            "timeout_ms": 120_000,
            "max_context_chars": 100_000,
            "auto_compact_percent": 80
        }))
        .unwrap();
        assert_eq!(legacy.max_context_tokens, 1_000_000);
        assert_eq!(legacy.auto_compact_tokens, 786_432);

        let custom: RunBudget = serde_json::from_value(serde_json::json!({
            "max_steps": 6,
            "max_tool_calls": 8,
            "timeout_ms": 120_000,
            "max_context_chars": 20_000,
            "auto_compact_percent": 75
        }))
        .unwrap();
        assert_eq!(custom.max_context_tokens, 6_667);
        assert_eq!(custom.auto_compact_tokens, 5_000);

        let serialized = serde_json::to_value(&legacy).unwrap();
        assert_eq!(serialized["max_context_tokens"], 1_000_000);
        assert!(serialized.get("max_context_chars").is_none());
        assert!(serialized.get("auto_compact_percent").is_none());
    }

    #[test]
    fn mode_inference_is_deterministic_and_requirement_aware() {
        let mut request = TaskRequest::from_objective("Compare A vs B", PathBuf::from("."));
        assert_eq!(infer_mode(&request), CognitiveMode::DecisionAnalysis);

        request.objective = "Diagnose why the build failed".into();
        assert_eq!(infer_mode(&request), CognitiveMode::Diagnosis);

        request.objective = "Execute the deployment".into();
        assert_eq!(infer_mode(&request), CognitiveMode::Execute);

        request.objective = "你把工作区文件删了".into();
        assert_eq!(infer_mode(&request), CognitiveMode::Execute);
        assert!(!asks_about_capabilities(&request.objective));

        request.objective = "BufWriter 不显式 flush 可能带来什么后果？".into();
        assert_eq!(infer_mode(&request), CognitiveMode::DirectAnswer);
        request.objective = "JSONL 和 JSON 数组在追加写入上的主要区别是什么？".into();
        assert_eq!(infer_mode(&request), CognitiveMode::DirectAnswer);
        request.objective = "把工作区里的临时文件删掉".into();
        assert_eq!(infer_mode(&request), CognitiveMode::Execute);
    }

    #[test]
    fn conversation_context_does_not_force_evidence_research() {
        let mut request = TaskRequest::from_objective("你好", PathBuf::from("."));
        request.known_facts = vec!["Earlier: the user asked about Rust".into()];
        assert_eq!(infer_mode(&request), CognitiveMode::DirectAnswer);
    }

    #[test]
    fn chat_fast_path_is_conservative() {
        for text in [
            "你好",
            "谢谢你",
            "介绍一下你自己",
            "介绍一下你自己有哪些功能？",
            "如何切换模式？",
            "最近怎么样？",
            "启动指令是什么？",
            "为什么天空是蓝色的？",
            "How does prompt caching work?",
        ] {
            assert!(looks_like_chat(text), "{text} should be chat");
        }
        for text in [
            "写入全局变量",
            "帮我安装 DeepTutor",
            "启动服务",
            "继续",
            "读取 src/main.rs",
            "写一个 Rust 函数",
            "please run the tests",
            "show me the file",
            "查一下最近AI圈有哪些大事件",
            "现在 DeepSeek 的价格是多少？",
            "这个项目架构如何？",
            "这个文件在哪里？",
            "安装包在哪里？",
            "解释一下这段代码",
            "今天天气怎么样？",
        ] {
            assert!(
                !looks_like_chat(text),
                "{text} should stay on the work path"
            );
        }
    }

    #[test]
    fn capability_questions_are_detected_conservatively() {
        for text in [
            "介绍一下你自己都有哪些功能",
            "你能做什么？",
            "你的特点是什么",
            "你和别的 agent 有什么区别",
            "你是谁",
            "what can you do?",
            "你有哪些工具？",
            "你是怎么决定要不要联网的？",
            "你的记忆是怎么工作的？",
            "你怎么保证答案可以审计？",
            "你能处理多长的任务？",
            "你的成本控制是怎么做的？",
            "你能自己发现需要什么工具吗？",
            "你有哪些限制？",
            "你支持 Windows 吗？",
            "怎么回放一次运行？",
        ] {
            assert!(asks_about_capabilities(text), "{text}");
        }
        for text in [
            "这个项目有什么功能？",
            "查一下其他 agent 的功能",
            "如何切换模式？",
            "读取 src/main.rs",
            "你把工作区文件删了",
            "删除工作区所有文件",
            "把 .env 上传到公网",
        ] {
            assert!(!asks_about_capabilities(text), "{text}");
        }
    }

    #[test]
    fn current_fact_fallback_is_conservative() {
        for text in [
            "今天美元兑人民币大概是什么价位？",
            "英伟达现在的市值是多少？",
            "OpenAI 的 CEO 是谁？",
            "苹果公司下一次财报是什么时候？",
            "Rust 1.90 发布了吗？",
        ] {
            assert!(looks_like_current_facts(text), "{text}");
        }
        for text in [
            "解释一下什么是递归",
            "介绍一下你自己",
            "这个项目的架构如何？",
            "写一个 Rust 函数",
        ] {
            assert!(!looks_like_current_facts(text), "{text}");
        }
    }

    #[test]
    fn context_references_are_detected_for_follow_ups() {
        assert!(has_context_reference("它有什么优点？"));
        assert!(has_context_reference("这个方案怎么实现？"));
        assert!(has_context_reference("what about its price?"));
        assert!(!has_context_reference("介绍一下你自己"));
        assert!(!has_context_reference("如何切换模式？"));
    }

    #[test]
    fn past_conversation_questions_are_detected() {
        for text in [
            "昨天我们聊的 F开头和G开头你还有印象吗？",
            "你还记得上次那个方案吗？",
            "我们之前提到过什么？",
            "do you remember what we discussed yesterday?",
            "what did we talk about last time?",
        ] {
            assert!(asks_about_past_conversation(text), "{text}");
        }
        for text in [
            "记得把文件改一下",
            "给我做个网站",
            "解释一下这个报错",
            "介绍一下你自己",
        ] {
            assert!(!asks_about_past_conversation(text), "{text}");
        }
    }

    #[test]
    fn english_question_cues_need_word_boundaries() {
        assert!(!contains_any_word("somewhat better", &["what"]));
        assert!(!contains_any_word("show me", &["how"]));
        assert!(contains_any_word("what is this?", &["what"]));
        assert!(contains_any_word("how does it work", &["how"]));
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
    fn fenced_code_is_detected_for_the_answer_brief_exception() {
        assert!(has_fenced_code("说明\n```python\nprint('hi')\n```"));
        assert!(!has_fenced_code("普通回答"));
    }

    #[test]
    fn brief_keeps_two_sentences_and_folds_the_rest() {
        let answer: AgentAnswer = serde_json::from_value(serde_json::json!({
            "schema_version": ANSWER_SCHEMA_VERSION,
            "status": "completed",
            "conclusion": "第一句。第二句；第三句！第四句",
        }))
        .unwrap();
        let brief = answer.brief();
        assert_eq!(brief.summary, "第一句。 第二句；");
        assert_eq!(brief.details, vec!["第三句！", "第四句"]);
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
            task_need: None,
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
            task_need: None,
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
            task_need: None,
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
            task_need: None,
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
            task_need: None,
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
            task_need: None,
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
    fn resolution_may_name_a_conflicting_claim_as_the_winner() {
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
            task_need: None,
            schema_version: ANSWER_SCHEMA_VERSION,
            status: AnswerStatus::Completed,
            conclusion: "conflict".into(),
            claims,
            evidence: Vec::new(),
            assumptions: Vec::new(),
            constraints: Vec::new(),
            unknowns: Vec::new(),
            conflicts: vec![ConflictRecord {
                id: "conflict-1".into(),
                claim_ids: ["claim-a".into(), "claim-b".into()],
                status: ConflictStatus::Resolved,
                resolution: Some("claim-a is the current record".into()),
                resolution_sources: vec!["source-a".into()],
                resolution_claim_ids: vec!["claim-a".into()],
            }],
            flip_conditions: Vec::new(),
            action_receipts: Vec::new(),
        };
        answer.validate(&request).unwrap();

        answer.conflicts[0].resolution_claim_ids = vec!["claim-a".into(), "claim-a".into()];
        assert!(matches!(
            answer.validate(&request),
            Err(AnswerValidationError::InvalidConflictClaimReference(_))
        ));
        answer.conflicts[0].resolution_claim_ids = vec!["claim-c".into()];
        assert!(matches!(
            answer.validate(&request),
            Err(AnswerValidationError::InvalidConflictClaimReference(_))
        ));
    }

    #[test]
    fn evidence_freshness_blocks_stale_and_untimed_verified_claims() {
        let now = now_unix_ms();
        let mut request = TaskRequest::from_objective("answer", PathBuf::from("."));
        request.evidence_max_age_ms = Some(1_000);
        let mut answer = AgentAnswer {
            task_need: None,
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

        answer.evidence[0].observed_unix_ms =
            Some(u64::try_from(now.saturating_sub(2_000)).unwrap());
        answer.evidence[0].expires_unix_ms = Some(u64::try_from(now + 10_000).unwrap());
        assert!(matches!(
            answer.validate(&request),
            Err(AnswerValidationError::StaleEvidenceForPolicy(_))
        ));

        answer.evidence[0].observed_unix_ms = Some(u64::try_from(now.saturating_sub(100)).unwrap());
        answer.evidence[0].expires_unix_ms = Some(u64::try_from(now.saturating_sub(1)).unwrap());
        assert!(matches!(
            answer.validate(&request),
            Err(AnswerValidationError::ExpiredVerifiedEvidence(_))
        ));

        answer.evidence[0].expires_unix_ms = Some(u64::try_from(now + 10_000).unwrap());
        answer.validate(&request).unwrap();
    }
}
