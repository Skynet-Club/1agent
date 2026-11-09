//! Surface API v0 contract skeleton.
//!
//! This module freezes request/response/error shapes, trace and idempotency
//! rules, the single outbound message shape and the SSE envelope. It does not
//! implement an HTTP server.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::action_log::ActionKind;
use crate::audit::AuditKind;
use crate::memory::MemoryScope;

pub const SURFACE_PROTOCOL_VERSION: &str = "v0";
const MAX_TEXT_CHARS: usize = 32_000;
const MAX_MESSAGE_CHARS: usize = 512;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SurfaceContractError {
    #[error("invalid surface request: {0}")]
    InvalidRequest(String),
    #[error("surface scope is required: {0}")]
    Forbidden(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SurfaceRequest {
    pub protocol_version: String,
    pub request_id: String,
    pub trace_id: String,
    #[serde(default)]
    pub idempotency_key: Option<String>,
    pub method: SurfaceMethod,
}

impl SurfaceRequest {
    pub fn validate(&self) -> Result<(), SurfaceError> {
        if self.protocol_version != SURFACE_PROTOCOL_VERSION {
            return Err(SurfaceError::invalid(format!(
                "unsupported protocol version {}",
                self.protocol_version
            )));
        }
        safe_id(&self.request_id, "request_id")?;
        safe_id(&self.trace_id, "trace_id")?;
        if let Some(key) = &self.idempotency_key {
            safe_id(key, "idempotency_key")?;
        }
        self.method.validate()
    }

    pub fn fingerprint(&self) -> Result<String, SurfaceError> {
        let bytes = serde_json::to_vec(&self.method)
            .map_err(|_| SurfaceError::invalid("method cannot be serialized"))?;
        Ok(hex::encode(Sha256::digest(bytes)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "name", content = "params", rename_all = "snake_case")]
pub enum SurfaceMethod {
    ThreadGet {
        thread_id: String,
    },
    ThreadList {
        limit: Option<usize>,
        #[serde(default)]
        cursor: Option<String>,
    },
    TicketGet {
        ticket_id: String,
    },
    TicketApprove {
        ticket_id: String,
        actor_id: String,
        decision: String,
        expires_ms: Option<u128>,
    },
    AuditQuery {
        kind: String,
        filter: AuditFilter,
    },
    ActionLogQuery {
        actor: Option<String>,
        kind: Option<String>,
        target: Option<String>,
        #[serde(default)]
        after_seq: Option<u64>,
        limit: usize,
    },
    ActionLogSummary,
    GoalList,
    GoalCreate {
        goal_id: String,
        actor_id: String,
        title: String,
        due_ms: Option<u128>,
    },
    GoalSetStatus {
        goal_id: String,
        actor_id: String,
        status: String,
        reason: String,
    },
    CommitmentList {
        goal_id: Option<String>,
    },
    CommitmentCreate {
        commitment_id: String,
        goal_id: String,
        actor_id: String,
        description: String,
        due_ms: Option<u128>,
    },
    CommitmentClose {
        commitment_id: String,
        actor_id: String,
        status: String,
        note: String,
    },
    CommitmentLinkTicket {
        commitment_id: String,
        ticket_id: String,
        actor_id: String,
    },
    AttentionCandidates {
        lead_ms: u128,
        limit: usize,
        include_missed: bool,
    },
    ContextSummary {
        bot_id: String,
    },
    ContextExplain {
        node_id: String,
        bot_id: String,
    },
    DelegationGet {
        actor_id: String,
        domain: String,
    },
    DelegationRegister {
        profile_id: String,
        actor_id: String,
        domain: String,
        mode: String,
        max_risk: String,
        allowed_tools: Vec<String>,
        expires_ms: Option<u128>,
    },
    DelegationRevoke {
        actor_id: String,
        domain: String,
    },
    EvolutionLedger,
    EvolutionPropose {
        proposal_id: String,
        proposer: String,
        kind: String,
        summary: String,
        evidence: Vec<String>,
        rollback_plan: String,
        risk: String,
    },
    EvolutionTransition {
        proposal_id: String,
        actor_id: String,
        action: String,
    },
    MemoryRecall {
        scope: MemoryScope,
        query: String,
        limit: usize,
    },
    MemoryForget {
        memory_id: String,
        scope: MemoryScope,
        mode: String,
    },
    SkillList,
    SkillEnable {
        name: String,
        version: String,
    },
    SkillDisable {
        name: String,
    },
    SkillRollback {
        name: String,
        version: String,
    },
    MessageSend {
        thread_id: String,
        channel: String,
        text: String,
    },
}

impl SurfaceMethod {
    fn validate(&self) -> Result<(), SurfaceError> {
        match self {
            Self::ThreadGet { thread_id } => safe_id(thread_id, "thread_id"),
            Self::ThreadList { limit, cursor } => {
                validate_limit(*limit, true)?;
                if let Some(cursor) = cursor {
                    safe_id(cursor, "cursor")?;
                }
                Ok(())
            }
            Self::TicketGet { ticket_id } => safe_id(ticket_id, "ticket_id"),
            Self::TicketApprove {
                ticket_id,
                actor_id,
                decision,
                ..
            } => {
                safe_id(ticket_id, "ticket_id")?;
                safe_id(actor_id, "actor_id")?;
                if !matches!(decision.as_str(), "allow_once" | "deny") {
                    return Err(SurfaceError::invalid("decision must be allow_once or deny"));
                }
                Ok(())
            }
            Self::AuditQuery { kind, filter } => {
                kind.parse::<AuditKind>()
                    .map_err(|_| SurfaceError::invalid("unknown audit kind"))?;
                if let Some(limit) = filter.limit {
                    validate_limit(Some(limit), false)?;
                }
                for (value, label) in [
                    (filter.thread_id.as_deref(), "thread_id"),
                    (filter.ticket_id.as_deref(), "ticket_id"),
                    (filter.memory_id.as_deref(), "memory_id"),
                    (filter.skill.as_deref(), "skill"),
                ] {
                    if let Some(value) = value {
                        safe_id(value, label)?;
                    }
                }
                Ok(())
            }
            Self::ActionLogQuery {
                actor,
                kind,
                target,
                after_seq: _,
                limit,
            } => {
                if let Some(actor) = actor {
                    safe_id(actor, "actor")?;
                }
                if let Some(target) = target {
                    safe_id(target, "target")?;
                }
                if let Some(kind) = kind {
                    kind.parse::<ActionKind>()
                        .map_err(|_| SurfaceError::invalid("unknown action kind"))?;
                }
                validate_limit(Some(*limit), false)
            }
            Self::GoalList | Self::EvolutionLedger | Self::ActionLogSummary => Ok(()),
            Self::EvolutionPropose {
                proposal_id,
                proposer,
                kind,
                summary,
                evidence,
                rollback_plan,
                risk,
            } => {
                safe_id(proposal_id, "proposal_id")?;
                safe_id(proposer, "proposer")?;
                if !matches!(
                    kind.as_str(),
                    "skill_draft" | "routine_draft" | "strategy_note"
                ) {
                    return Err(SurfaceError::invalid("unknown evolution kind"));
                }
                if !matches!(risk.as_str(), "low" | "medium" | "high") {
                    return Err(SurfaceError::invalid("unknown risk level"));
                }
                validate_text(summary, "summary")?;
                validate_text(rollback_plan, "rollback_plan")?;
                if evidence.is_empty() {
                    return Err(SurfaceError::invalid("evidence is required"));
                }
                for item in evidence {
                    validate_text(item, "evidence")?;
                }
                Ok(())
            }
            Self::EvolutionTransition {
                proposal_id,
                actor_id,
                action,
            } => {
                safe_id(proposal_id, "proposal_id")?;
                safe_id(actor_id, "actor_id")?;
                if !matches!(action.as_str(), "approve" | "reject" | "apply" | "rollback") {
                    return Err(SurfaceError::invalid("unknown evolution action"));
                }
                Ok(())
            }
            Self::GoalCreate {
                goal_id,
                actor_id,
                title,
                ..
            } => {
                safe_id(goal_id, "goal_id")?;
                safe_id(actor_id, "actor_id")?;
                validate_text(title, "title")
            }
            Self::GoalSetStatus {
                goal_id,
                actor_id,
                status,
                ..
            } => {
                safe_id(goal_id, "goal_id")?;
                safe_id(actor_id, "actor_id")?;
                status
                    .parse::<crate::goal::GoalStatus>()
                    .map_err(|_| SurfaceError::invalid("unknown goal status"))?;
                Ok(())
            }
            Self::CommitmentCreate {
                commitment_id,
                goal_id,
                actor_id,
                description,
                ..
            } => {
                safe_id(commitment_id, "commitment_id")?;
                safe_id(goal_id, "goal_id")?;
                safe_id(actor_id, "actor_id")?;
                validate_text(description, "description")
            }
            Self::CommitmentClose {
                commitment_id,
                actor_id,
                status,
                ..
            } => {
                safe_id(commitment_id, "commitment_id")?;
                safe_id(actor_id, "actor_id")?;
                status
                    .parse::<crate::goal::CommitmentStatus>()
                    .map_err(|_| SurfaceError::invalid("unknown commitment status"))?;
                Ok(())
            }
            Self::CommitmentLinkTicket {
                commitment_id,
                ticket_id,
                actor_id,
            } => {
                safe_id(commitment_id, "commitment_id")?;
                safe_id(ticket_id, "ticket_id")?;
                safe_id(actor_id, "actor_id")
            }
            Self::ContextSummary { bot_id } => safe_id(bot_id, "bot_id"),
            Self::ContextExplain { node_id, bot_id } => {
                safe_node_id(node_id)?;
                safe_id(bot_id, "bot_id")
            }
            Self::CommitmentList { goal_id } => {
                if let Some(goal_id) = goal_id {
                    safe_id(goal_id, "goal_id")?;
                }
                Ok(())
            }
            Self::AttentionCandidates { lead_ms, limit, .. } => {
                if *lead_ms == 0 {
                    return Err(SurfaceError::invalid("lead_ms must be greater than zero"));
                }
                validate_limit(Some(*limit), false)
            }
            Self::DelegationGet { actor_id, domain } => {
                safe_id(actor_id, "actor_id")?;
                safe_domain(domain)
            }
            Self::DelegationRegister {
                profile_id,
                actor_id,
                domain,
                mode,
                max_risk,
                allowed_tools,
                ..
            } => {
                safe_id(profile_id, "profile_id")?;
                safe_id(actor_id, "actor_id")?;
                safe_domain(domain)?;
                mode.parse::<crate::delegation::DelegationMode>()
                    .map_err(|_| SurfaceError::invalid("unknown delegation mode"))?;
                if !matches!(max_risk.as_str(), "low" | "medium" | "high") {
                    return Err(SurfaceError::invalid("unknown risk level"));
                }
                for tool in allowed_tools {
                    safe_id(tool, "allowed_tool")?;
                }
                Ok(())
            }
            Self::DelegationRevoke { actor_id, domain } => {
                safe_id(actor_id, "actor_id")?;
                safe_domain(domain)
            }
            Self::MemoryRecall {
                scope,
                query,
                limit,
            } => {
                safe_id(&scope.bot_id, "bot_id")?;
                if query.chars().count() > 1_000 {
                    return Err(SurfaceError::invalid("query exceeds 1000 chars"));
                }
                validate_limit(Some(*limit), false)
            }
            Self::MemoryForget {
                memory_id,
                scope,
                mode,
            } => {
                safe_id(memory_id, "memory_id")?;
                safe_id(&scope.bot_id, "bot_id")?;
                if !matches!(mode.as_str(), "soft" | "hard") {
                    return Err(SurfaceError::invalid("mode must be soft or hard"));
                }
                Ok(())
            }
            Self::SkillList => Ok(()),
            Self::SkillEnable { name, version } | Self::SkillRollback { name, version } => {
                safe_id(name, "name")?;
                safe_id(version, "version")
            }
            Self::SkillDisable { name } => safe_id(name, "name"),
            Self::MessageSend {
                thread_id,
                channel,
                text,
            } => {
                safe_id(thread_id, "thread_id")?;
                safe_id(channel, "channel")?;
                validate_text(text, "text")
            }
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuditFilter {
    pub thread_id: Option<String>,
    pub ticket_id: Option<String>,
    pub memory_id: Option<String>,
    pub scope: Option<String>,
    pub skill: Option<String>,
    pub version: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceErrorCode {
    InvalidRequest,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    IdempotencyConflict,
    RateLimited,
    Unavailable,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SurfaceError {
    pub code: SurfaceErrorCode,
    pub message: String,
    pub retryable: bool,
}

impl std::fmt::Display for SurfaceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for SurfaceError {}

impl SurfaceError {
    pub fn new(code: SurfaceErrorCode, message: impl Into<String>, retryable: bool) -> Self {
        let message = bounded_message(&message.into());
        Self {
            code,
            message,
            retryable,
        }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(SurfaceErrorCode::InvalidRequest, message, false)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(SurfaceErrorCode::Forbidden, message, false)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SurfaceResponse {
    pub protocol_version: String,
    pub request_id: String,
    pub trace_id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<SurfaceError>,
}

impl SurfaceResponse {
    pub fn success(request: &SurfaceRequest, result: serde_json::Value) -> Self {
        Self {
            protocol_version: SURFACE_PROTOCOL_VERSION.into(),
            request_id: request.request_id.clone(),
            trace_id: request.trace_id.clone(),
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn failure(request: &SurfaceRequest, error: SurfaceError) -> Self {
        Self {
            protocol_version: SURFACE_PROTOCOL_VERSION.into(),
            request_id: request.request_id.clone(),
            trace_id: request.trace_id.clone(),
            ok: false,
            result: None,
            error: Some(error),
        }
    }
}

/// The single outbound message shape. Channels render this; they do not invent
/// their own message schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OutboundMessage {
    pub message_id: String,
    pub thread_id: String,
    pub channel: String,
    pub text: String,
    pub created_ms: u128,
    pub trace_id: String,
}

impl OutboundMessage {
    pub fn validate(&self) -> Result<(), SurfaceError> {
        safe_id(&self.message_id, "message_id")?;
        safe_id(&self.thread_id, "thread_id")?;
        safe_id(&self.channel, "channel")?;
        safe_id(&self.trace_id, "trace_id")?;
        validate_text(&self.text, "text")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SurfaceSseEvent {
    pub id: u64,
    pub event: String,
    pub data: serde_json::Value,
}

impl SurfaceSseEvent {
    pub fn to_sse(&self) -> Result<String, SurfaceError> {
        safe_id(&self.event, "event")?;
        let data = serde_json::to_string(&self.data)
            .map_err(|_| SurfaceError::new(SurfaceErrorCode::Internal, "SSE data", false))?;
        if data.contains('\n') || data.contains('\r') {
            return Err(SurfaceError::invalid("SSE data must be one line"));
        }
        Ok(format!(
            "id: {}\nevent: {}\ndata: {}\n\n",
            self.id, self.event, data
        ))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuthContext {
    pub actor_id: String,
    pub scopes: Vec<String>,
}

impl AuthContext {
    pub fn require(&self, scope: &str) -> Result<(), SurfaceError> {
        safe_id(&self.actor_id, "actor_id")?;
        if self
            .scopes
            .iter()
            .any(|candidate| candidate == "*" || candidate == scope)
        {
            return Ok(());
        }
        Err(SurfaceError::forbidden(format!(
            "actor {} lacks scope {}",
            self.actor_id, scope
        )))
    }
}

#[derive(Debug, Default)]
pub struct IdempotencyRegistry {
    entries: BTreeMap<String, IdempotencyEntry>,
}

#[derive(Debug, Clone)]
struct IdempotencyEntry {
    fingerprint: String,
    response: SurfaceResponse,
}

impl IdempotencyRegistry {
    pub fn replay(
        &self,
        key: &str,
        fingerprint: &str,
    ) -> Result<Option<SurfaceResponse>, SurfaceError> {
        let Some(entry) = self.entries.get(key) else {
            return Ok(None);
        };
        if entry.fingerprint != fingerprint {
            return Err(SurfaceError::new(
                SurfaceErrorCode::IdempotencyConflict,
                "idempotency key reused with a different request",
                false,
            ));
        }
        Ok(Some(entry.response.clone()))
    }

    pub fn record(&mut self, key: &str, fingerprint: &str, response: SurfaceResponse) {
        self.entries.insert(
            key.to_string(),
            IdempotencyEntry {
                fingerprint: fingerprint.to_string(),
                response,
            },
        );
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn validate_limit(limit: Option<usize>, optional: bool) -> Result<(), SurfaceError> {
    match limit {
        None if optional => Ok(()),
        None => Err(SurfaceError::invalid("limit is required")),
        Some(limit) if !(1..=1_000).contains(&limit) => {
            Err(SurfaceError::invalid("limit must be 1..=1000"))
        }
        Some(_) => Ok(()),
    }
}

fn safe_id(value: &str, label: &str) -> Result<(), SurfaceError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        || value.starts_with('.')
    {
        return Err(SurfaceError::invalid(format!("{label} is unsafe")));
    }
    Ok(())
}

fn safe_node_id(value: &str) -> Result<(), SurfaceError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'))
        || value.starts_with('.')
    {
        return Err(SurfaceError::invalid("node_id is unsafe"));
    }
    Ok(())
}

fn safe_domain(value: &str) -> Result<(), SurfaceError> {
    if value.is_empty()
        || value.len() > 128
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':' | b'/' | b'*')
        })
        || value.starts_with('.')
    {
        return Err(SurfaceError::invalid("domain is unsafe"));
    }
    Ok(())
}

fn validate_text(value: &str, label: &str) -> Result<(), SurfaceError> {
    if value.trim().is_empty() || value.chars().count() > MAX_TEXT_CHARS {
        return Err(SurfaceError::invalid(format!(
            "{label} must be 1..={MAX_TEXT_CHARS} chars"
        )));
    }
    if value
        .chars()
        .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\r' | '\t'))
    {
        return Err(SurfaceError::invalid(format!(
            "{label} contains control characters"
        )));
    }
    Ok(())
}

fn bounded_message(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !ch.is_control() || matches!(ch, '\n' | '\r' | '\t'))
        .take(MAX_MESSAGE_CHARS)
        .collect()
}

pub fn openapi_document() -> serde_json::Value {
    serde_json::json!({
        "openapi": "3.1.0",
        "info": {
            "title": "1Agent Surface API",
            "version": SURFACE_PROTOCOL_VERSION
        },
        "paths": {
            "/v0/threads": {
                "get": {
                    "operationId": "listThreads",
                    "summary": "List threads",
                    "description": "Cursor is the last thread id from the previous page; results are exclusive of it."
                }
            },
            "/v0/threads/{thread_id}": {
                "get": {"operationId": "getThread", "summary": "Read one thread"}
            },
            "/v0/tickets/{ticket_id}": {
                "get": {"operationId": "getTicket", "summary": "Read one ticket"}
            },
            "/v0/tickets/{ticket_id}/approve": {
                "post": {"operationId": "approveTicket", "summary": "Approve or deny a ticket"}
            },
            "/v0/audit/query": {
                "post": {"operationId": "queryAudit", "summary": "Query an append-only audit store"}
            },
            "/v0/action-log/query": {
                "post": {"operationId": "queryActionLog", "summary": "Query security actions"}
            },
            "/v0/action-log/summary": {
                "post": {"operationId": "summarizeActionLog", "summary": "Summarize security actions"}
            },
            "/v0/goals": {
                "get": {"operationId": "listGoals", "summary": "List goals and commitments"}
            },
            "/v0/goals/create": {
                "post": {"operationId": "createGoal", "summary": "Create a goal"}
            },
            "/v0/goals/status": {
                "post": {"operationId": "setGoalStatus", "summary": "Transition a goal"}
            },
            "/v0/commitments": {
                "get": {"operationId": "listCommitments", "summary": "List commitments"}
            },
            "/v0/commitments/create": {
                "post": {"operationId": "createCommitment", "summary": "Add a commitment"}
            },
            "/v0/commitments/close": {
                "post": {"operationId": "closeCommitment", "summary": "Close a commitment"}
            },
            "/v0/commitments/link": {
                "post": {"operationId": "linkCommitmentTicket", "summary": "Link a commitment to a ticket"}
            },
            "/v0/attention/candidates": {
                "get": {"operationId": "listAttentionCandidates", "summary": "List deterministic attention candidates"}
            },
            "/v0/context/summary": {
                "get": {
                    "operationId": "getContextSummary",
                    "summary": "Read the rebuildable personal-context view",
                    "description": "Scoped to one bot_id; requires context:read and memory:read."
                }
            },
            "/v0/context/explain": {
                "post": {
                    "operationId": "explainContextNode",
                    "summary": "Explain one context node and its edges",
                    "description": "Scoped to one bot_id; requires context:read and memory:read."
                }
            },
            "/v0/delegation/{actor_id}/{domain}": {
                "get": {"operationId": "getDelegation", "summary": "Read one delegation profile"}
            },
            "/v0/delegation/register": {
                "post": {"operationId": "registerDelegation", "summary": "Create a delegation profile"}
            },
            "/v0/delegation/revoke": {
                "post": {"operationId": "revokeDelegation", "summary": "Revoke a delegation profile"}
            },
            "/v0/evolution/ledger": {
                "get": {"operationId": "getEvolutionLedger", "summary": "Read the evolution ledger"}
            },
            "/v0/evolution/propose": {
                "post": {"operationId": "proposeEvolution", "summary": "Propose a skill/routine/strategy change"}
            },
            "/v0/evolution/transition": {
                "post": {"operationId": "transitionEvolution", "summary": "Approve, reject, apply or roll back"}
            },
            "/v0/memory/recall": {
                "post": {"operationId": "recallMemory", "summary": "Recall scoped memory"}
            },
            "/v0/memory/forget": {
                "post": {"operationId": "forgetMemory", "summary": "Soft- or hard-forget a memory"}
            },
            "/v0/skills": {
                "get": {"operationId": "listSkills", "summary": "List skills"}
            },
            "/v0/skills/enable": {
                "post": {"operationId": "enableSkill", "summary": "Activate a skill version"}
            },
            "/v0/skills/disable": {
                "post": {"operationId": "disableSkill", "summary": "Disable a skill"}
            },
            "/v0/skills/rollback": {
                "post": {"operationId": "rollbackSkill", "summary": "Roll back a skill version"}
            },
            "/v0/messages": {
                "post": {"operationId": "sendMessage", "summary": "Send one outbound message"}
            },
            "/v0/events": {
                "get": {"operationId": "streamEvents", "summary": "SSE event stream"}
            }
        },
        "components": {
            "schemas": {
                "SurfaceRequest": {"type": "object", "required": ["protocol_version", "request_id", "trace_id", "method"]},
                "SurfaceResponse": {"type": "object", "required": ["protocol_version", "request_id", "trace_id", "ok"]},
                "SurfaceError": {"type": "object", "required": ["code", "message", "retryable"]},
                "OutboundMessage": {"type": "object", "required": ["message_id", "thread_id", "channel", "text", "created_ms", "trace_id"]}
            }
        }
    })
}

pub fn validate_openapi(document: &serde_json::Value) -> Result<(), String> {
    let version = document
        .get("openapi")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if !version.starts_with("3.") {
        return Err("openapi version must be 3.x".into());
    }
    let paths = document
        .get("paths")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "missing paths".to_string())?;
    for path in [
        "/v0/threads",
        "/v0/threads/{thread_id}",
        "/v0/tickets/{ticket_id}",
        "/v0/tickets/{ticket_id}/approve",
        "/v0/audit/query",
        "/v0/action-log/query",
        "/v0/action-log/summary",
        "/v0/goals",
        "/v0/goals/create",
        "/v0/goals/status",
        "/v0/commitments",
        "/v0/commitments/create",
        "/v0/commitments/close",
        "/v0/commitments/link",
        "/v0/attention/candidates",
        "/v0/context/summary",
        "/v0/context/explain",
        "/v0/delegation/{actor_id}/{domain}",
        "/v0/delegation/register",
        "/v0/delegation/revoke",
        "/v0/evolution/ledger",
        "/v0/evolution/propose",
        "/v0/evolution/transition",
        "/v0/memory/recall",
        "/v0/memory/forget",
        "/v0/skills",
        "/v0/skills/enable",
        "/v0/skills/disable",
        "/v0/skills/rollback",
        "/v0/messages",
        "/v0/events",
    ] {
        if !paths.contains_key(path) {
            return Err(format!("missing path {path}"));
        }
    }
    let schemas = document
        .get("components")
        .and_then(|components| components.get("schemas"))
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "missing component schemas".to_string())?;
    for schema in [
        "SurfaceRequest",
        "SurfaceResponse",
        "SurfaceError",
        "OutboundMessage",
    ] {
        if !schemas.contains_key(schema) {
            return Err(format!("missing schema {schema}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(method: SurfaceMethod) -> SurfaceRequest {
        SurfaceRequest {
            protocol_version: SURFACE_PROTOCOL_VERSION.into(),
            request_id: "request-1".into(),
            trace_id: "trace-1".into(),
            idempotency_key: Some("idem-1".into()),
            method,
        }
    }

    #[test]
    fn request_response_error_and_openapi_roundtrip() {
        let request = request(SurfaceMethod::TicketGet {
            ticket_id: "ticket-1".into(),
        });
        request.validate().unwrap();
        let json = serde_json::to_vec(&request).unwrap();
        assert_eq!(
            serde_json::from_slice::<SurfaceRequest>(&json).unwrap(),
            request
        );
        let response = SurfaceResponse::failure(&request, SurfaceError::invalid("bad"));
        assert!(!response.ok);
        assert_eq!(
            response.error.unwrap().code,
            SurfaceErrorCode::InvalidRequest
        );
        let document = openapi_document();
        validate_openapi(&document).unwrap();
    }

    #[test]
    fn validation_rejects_control_characters_and_bad_ids() {
        let mut bad = request(SurfaceMethod::ThreadGet {
            thread_id: "thread/1".into(),
        });
        assert!(bad.validate().is_err());
        bad.method = SurfaceMethod::MessageSend {
            thread_id: "thread-1".into(),
            channel: "web".into(),
            text: "hello\u{0}world".into(),
        };
        assert!(bad.validate().is_err());
        bad.method = SurfaceMethod::MessageSend {
            thread_id: "thread-1".into(),
            channel: "web".into(),
            text: "hello".into(),
        };
        bad.validate().unwrap();
        assert!(bad.fingerprint().is_ok());
    }

    #[test]
    fn idempotency_replays_same_request_and_rejects_conflicts() {
        let request = request(SurfaceMethod::ThreadGet {
            thread_id: "thread-1".into(),
        });
        let fingerprint = request.fingerprint().unwrap();
        let response = SurfaceResponse::success(&request, serde_json::json!({"ok": true}));
        let mut registry = IdempotencyRegistry::default();
        assert!(registry.replay("idem-1", &fingerprint).unwrap().is_none());
        registry.record("idem-1", &fingerprint, response.clone());
        assert_eq!(
            registry.replay("idem-1", &fingerprint).unwrap(),
            Some(response)
        );
        assert_eq!(
            registry.replay("idem-1", "different").unwrap_err().code,
            SurfaceErrorCode::IdempotencyConflict
        );
    }

    #[test]
    fn auth_and_sse_contracts_are_explicit() {
        let auth = AuthContext {
            actor_id: "actor-1".into(),
            scopes: vec!["ticket:approve".into()],
        };
        auth.require("ticket:approve").unwrap();
        assert_eq!(
            auth.require("ticket:write").unwrap_err().code,
            SurfaceErrorCode::Forbidden
        );
        let sse = SurfaceSseEvent {
            id: 7,
            event: "message".into(),
            data: serde_json::json!({"text": "hello"}),
        };
        assert_eq!(
            sse.to_sse().unwrap(),
            "id: 7\nevent: message\ndata: {\"text\":\"hello\"}\n\n"
        );
    }

    #[test]
    fn outbound_message_is_the_single_validated_shape() {
        let message = OutboundMessage {
            message_id: "message-1".into(),
            thread_id: "thread-1".into(),
            channel: "web".into(),
            text: "hello".into(),
            created_ms: 10,
            trace_id: "trace-1".into(),
        };
        message.validate().unwrap();
        let mut bad = message;
        bad.text = "\u{7}".into();
        assert!(bad.validate().is_err());
    }
}
