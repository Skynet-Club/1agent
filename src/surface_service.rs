//! Minimal Surface API handler wired to GovernedTaskGraph.
//!
//! v0 deliberately implements the ticket/thread read and approve paths. Other
//! methods return `unavailable` until their stores and policies are wired.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::json;

use crate::action_log::ActionKind;
use crate::attention::{AttentionPolicy, AttentionState, attention_candidates};
use crate::audit::{AuditError, AuditKind, AuditQuery, audit_export};
use crate::context_graph::build_context_graph;
use crate::delegation::{
    DelegationError, DelegationMode, DelegationProfile, DelegationStore, DelegationStoreError,
};
use crate::domain::RiskLevel;
use crate::evolution::{
    EvolutionError, EvolutionKind, EvolutionProposalDraft, EvolutionStore, EvolutionStoreError,
};
use crate::goal::{CommitmentStatus, GoalError, GoalStatus, GoalStore};
use crate::governed_task_graph::{GovernedTaskGraph, GovernedTaskGraphError};
use crate::memory_journal::MemoryJournalError;
use crate::memory_service::{MemoryService, MemoryServiceError};
use crate::skill_journal::SkillJournalError;
use crate::skill_service::{SkillService, SkillServiceError};
use crate::surface_api::{
    AuditFilter, AuthContext, OutboundMessage, SurfaceError, SurfaceErrorCode, SurfaceMethod,
    SurfaceRequest, SurfaceResponse,
};
use crate::task_graph::{ApprovalDecision, TaskGraphError};
use crate::task_graph_store::TaskGraphStoreError;
use uuid::Uuid;

#[derive(Debug, Default)]
pub struct SurfaceResources {
    pub memory: Option<MemoryService>,
    pub skills: Option<SkillService>,
    pub goals: Option<GoalStore>,
    pub attention: Option<AttentionState>,
    pub delegation: Option<DelegationStore>,
    pub evolution: Option<EvolutionStore>,
    pub audit_paths: BTreeMap<AuditKind, PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceQuota {
    max_requests: u32,
    window_ms: u128,
    buckets: BTreeMap<String, QuotaBucket>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct QuotaBucket {
    window_start_ms: u128,
    used: u32,
}

impl SurfaceQuota {
    pub fn new(max_requests: u32, window_ms: u128) -> Option<Self> {
        (max_requests > 0 && window_ms > 0).then(|| Self {
            max_requests,
            window_ms,
            buckets: BTreeMap::new(),
        })
    }

    pub fn check(&mut self, actor_id: &str, now_ms: u128) -> Result<(), SurfaceError> {
        if actor_id.trim().is_empty() {
            return Err(SurfaceError::forbidden("actor id is required"));
        }
        let bucket = self.buckets.entry(actor_id.into()).or_insert(QuotaBucket {
            window_start_ms: now_ms,
            used: 0,
        });
        if now_ms.saturating_sub(bucket.window_start_ms) >= self.window_ms {
            bucket.window_start_ms = now_ms;
            bucket.used = 0;
        }
        if bucket.used >= self.max_requests {
            return Err(SurfaceError::new(
                SurfaceErrorCode::RateLimited,
                "surface request quota exceeded",
                true,
            ));
        }
        bucket.used = bucket.used.saturating_add(1);
        // Bound the actor table even when callers rotate identities. A bucket
        // older than two windows contributes no rate-limit state.
        let expiry = self.window_ms.saturating_mul(2);
        self.buckets
            .retain(|_, bucket| now_ms.saturating_sub(bucket.window_start_ms) < expiry);
        if self.buckets.len() > 4_096
            && let Some(oldest) = self
                .buckets
                .iter()
                .min_by_key(|(_, bucket)| bucket.window_start_ms)
                .map(|(actor, _)| actor.clone())
        {
            self.buckets.remove(&oldest);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct SurfaceService {
    graph: GovernedTaskGraph,
    resources: SurfaceResources,
    quota: Option<SurfaceQuota>,
}

impl SurfaceService {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, GovernedTaskGraphError> {
        let path = path.into();
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("task-graph");
        let action_log = path.with_file_name(format!("{stem}-action-log.jsonl"));
        Ok(Self {
            graph: GovernedTaskGraph::open_with_action_log(path, action_log)?,
            resources: SurfaceResources::default(),
            quota: None,
        })
    }

    pub fn from_graph(graph: GovernedTaskGraph) -> Self {
        Self {
            graph,
            resources: SurfaceResources::default(),
            quota: None,
        }
    }

    pub fn with_resources(mut self, resources: SurfaceResources) -> Self {
        self.resources = resources;
        self
    }

    pub fn with_quota(mut self, quota: SurfaceQuota) -> Self {
        self.quota = Some(quota);
        self
    }

    pub fn resources(&self) -> &SurfaceResources {
        &self.resources
    }

    pub fn resources_mut(&mut self) -> &mut SurfaceResources {
        &mut self.resources
    }

    pub fn graph(&self) -> &GovernedTaskGraph {
        &self.graph
    }

    pub fn graph_mut(&mut self) -> &mut GovernedTaskGraph {
        &mut self.graph
    }

    pub fn handle(
        &mut self,
        request: &SurfaceRequest,
        auth: &AuthContext,
        now_ms: u128,
    ) -> SurfaceResponse {
        if let Err(error) = request.validate() {
            return SurfaceResponse::failure(request, error);
        }
        if let Some(quota) = &mut self.quota
            && let Err(error) = quota.check(&auth.actor_id, now_ms)
        {
            return SurfaceResponse::failure(request, error);
        }
        let result = match &request.method {
            SurfaceMethod::TicketApprove {
                ticket_id,
                actor_id,
                decision,
                expires_ms,
            } => self.approve_ticket(auth, ticket_id, actor_id, decision, *expires_ms, now_ms),
            SurfaceMethod::TicketGet { ticket_id } => self.ticket_get(auth, ticket_id),
            SurfaceMethod::ThreadGet { thread_id } => self.thread_get(auth, thread_id),
            SurfaceMethod::ThreadList { limit, cursor } => {
                self.thread_list(auth, *limit, cursor.as_deref())
            }
            SurfaceMethod::MemoryRecall {
                scope,
                query,
                limit,
            } => self.memory_recall(auth, scope, query, *limit, now_ms),
            SurfaceMethod::MemoryForget {
                memory_id,
                scope,
                mode,
            } => self.memory_forget(auth, memory_id, scope, mode, now_ms),
            SurfaceMethod::SkillList => self.skill_list(auth),
            SurfaceMethod::SkillEnable { name, version } => {
                self.skill_enable(auth, name, version, now_ms)
            }
            SurfaceMethod::SkillDisable { name } => self.skill_disable(auth, name, now_ms),
            SurfaceMethod::SkillRollback { name, version } => {
                self.skill_rollback(auth, name, version, now_ms)
            }
            SurfaceMethod::AuditQuery { kind, filter } => {
                self.audit_query(auth, kind, filter, now_ms)
            }
            SurfaceMethod::ActionLogQuery {
                actor,
                kind,
                target,
                after_seq,
                limit,
            } => self.action_log_query(
                auth,
                actor.as_deref(),
                kind.as_deref(),
                target.as_deref(),
                *after_seq,
                *limit,
            ),
            SurfaceMethod::ActionLogSummary => self.action_log_summary(auth),
            SurfaceMethod::GoalList => self.goal_list(auth),
            SurfaceMethod::GoalCreate {
                goal_id,
                actor_id,
                title,
                due_ms,
            } => self.goal_create(auth, goal_id, actor_id, title, *due_ms, now_ms),
            SurfaceMethod::GoalSetStatus {
                goal_id,
                actor_id,
                status,
                reason,
            } => self.goal_set_status(auth, goal_id, actor_id, status, reason, now_ms),
            SurfaceMethod::CommitmentList { goal_id } => {
                self.commitment_list(auth, goal_id.as_deref())
            }
            SurfaceMethod::CommitmentCreate {
                commitment_id,
                goal_id,
                actor_id,
                description,
                due_ms,
            } => self.commitment_create(
                auth,
                CommitmentCreateInput {
                    commitment_id,
                    goal_id,
                    actor_id,
                    description,
                    due_ms: *due_ms,
                },
                now_ms,
            ),
            SurfaceMethod::CommitmentClose {
                commitment_id,
                actor_id,
                status,
                note,
            } => self.commitment_close(auth, commitment_id, actor_id, status, note, now_ms),
            SurfaceMethod::CommitmentLinkTicket {
                commitment_id,
                ticket_id,
                actor_id,
            } => self.commitment_link_ticket(auth, commitment_id, ticket_id, actor_id, now_ms),
            SurfaceMethod::AttentionCandidates {
                lead_ms,
                limit,
                include_missed,
            } => self.attention_candidates(auth, *lead_ms, *limit, *include_missed, now_ms),
            SurfaceMethod::ContextSummary { bot_id } => self.context_summary(auth, bot_id, now_ms),
            SurfaceMethod::ContextExplain { node_id, bot_id } => {
                self.context_explain(auth, node_id, bot_id, now_ms)
            }
            SurfaceMethod::DelegationGet { actor_id, domain } => {
                self.delegation_get(auth, actor_id, domain)
            }
            SurfaceMethod::DelegationRegister {
                profile_id,
                actor_id,
                domain,
                mode,
                max_risk,
                allowed_tools,
                expires_ms,
            } => self.delegation_register(
                auth,
                DelegationRegisterInput {
                    profile_id,
                    actor_id,
                    domain,
                    mode,
                    max_risk,
                    allowed_tools,
                    expires_ms: *expires_ms,
                },
            ),
            SurfaceMethod::DelegationRevoke { actor_id, domain } => {
                self.delegation_revoke(auth, actor_id, domain, now_ms)
            }
            SurfaceMethod::EvolutionLedger => self.evolution_ledger(auth),
            SurfaceMethod::EvolutionPropose {
                proposal_id,
                proposer,
                kind,
                summary,
                evidence,
                rollback_plan,
                risk,
            } => self.evolution_propose(
                auth,
                EvolutionProposalInput {
                    proposal_id,
                    proposer,
                    kind,
                    summary,
                    evidence,
                    rollback_plan,
                    risk,
                },
                now_ms,
            ),
            SurfaceMethod::EvolutionTransition {
                proposal_id,
                actor_id,
                action,
            } => self.evolution_transition(auth, proposal_id, actor_id, action, now_ms),
            SurfaceMethod::MessageSend {
                thread_id,
                channel,
                text,
            } => self.message_send(auth, request, thread_id, channel, text, now_ms),
        };
        match result {
            Ok(value) => SurfaceResponse::success(request, value),
            Err(error) => SurfaceResponse::failure(request, error),
        }
    }

    fn approve_ticket(
        &mut self,
        auth: &AuthContext,
        ticket_id: &str,
        actor_id: &str,
        decision: &str,
        expires_ms: Option<u128>,
        now_ms: u128,
    ) -> Result<serde_json::Value, SurfaceError> {
        auth.require("ticket:approve")?;
        if auth.actor_id != actor_id {
            return Err(SurfaceError::forbidden(
                "actor_id must match the authenticated actor",
            ));
        }
        let decision = match decision {
            "allow_once" => ApprovalDecision::AllowOnce,
            "deny" => ApprovalDecision::Deny,
            _ => return Err(SurfaceError::invalid("unknown approval decision")),
        };
        let approval_id = self
            .graph
            .approve_ticket(ticket_id, actor_id, decision, expires_ms, now_ms)
            .map_err(map_governed_error)?;
        Ok(json!({"approval_id": approval_id, "ticket_id": ticket_id}))
    }

    fn ticket_get(
        &self,
        auth: &AuthContext,
        ticket_id: &str,
    ) -> Result<serde_json::Value, SurfaceError> {
        auth.require("ticket:read")?;
        let ticket = self.graph.graph().ticket(ticket_id).ok_or_else(|| {
            SurfaceError::new(SurfaceErrorCode::NotFound, "ticket not found", false)
        })?;
        serde_json::to_value(ticket)
            .map_err(|_| SurfaceError::new(SurfaceErrorCode::Internal, "ticket encode", false))
    }

    fn thread_get(
        &self,
        auth: &AuthContext,
        thread_id: &str,
    ) -> Result<serde_json::Value, SurfaceError> {
        auth.require("thread:read")?;
        let thread = self.graph.graph().thread(thread_id).map_err(|_| {
            SurfaceError::new(SurfaceErrorCode::NotFound, "thread not found", false)
        })?;
        serde_json::to_value(thread)
            .map_err(|_| SurfaceError::new(SurfaceErrorCode::Internal, "thread encode", false))
    }

    fn thread_list(
        &self,
        auth: &AuthContext,
        limit: Option<usize>,
        cursor: Option<&str>,
    ) -> Result<serde_json::Value, SurfaceError> {
        auth.require("thread:read")?;
        let mut threads = self.graph.snapshot().threads;
        if let Some(cursor) = cursor {
            let index = threads
                .iter()
                .position(|thread| thread.thread_id == cursor)
                .ok_or_else(|| SurfaceError::invalid("cursor does not match a thread"))?;
            threads.drain(..=index);
        }
        if let Some(limit) = limit {
            threads.truncate(limit);
        }
        serde_json::to_value(threads)
            .map_err(|_| SurfaceError::new(SurfaceErrorCode::Internal, "thread encode", false))
    }

    fn goal_list(&self, auth: &AuthContext) -> Result<serde_json::Value, SurfaceError> {
        auth.require("goal:read")?;
        let goals =
            self.resources.goals.as_ref().ok_or_else(|| {
                SurfaceError::new(SurfaceErrorCode::Unavailable, "goal store", false)
            })?;
        serde_json::to_value(goals.graph().snapshot())
            .map_err(|_| SurfaceError::new(SurfaceErrorCode::Internal, "goal encode", false))
    }

    fn goal_create(
        &mut self,
        auth: &AuthContext,
        goal_id: &str,
        actor_id: &str,
        title: &str,
        due_ms: Option<u128>,
        now_ms: u128,
    ) -> Result<serde_json::Value, SurfaceError> {
        auth.require("goal:write")?;
        ensure_actor(auth, actor_id)?;
        let goals = self.goals_mut()?;
        goals
            .with_graph_mut(|graph| graph.create_goal(goal_id, actor_id, title, due_ms, now_ms))
            .map_err(map_goal_error)?;
        let goal =
            goals.graph().goal(goal_id).cloned().ok_or_else(|| {
                SurfaceError::new(SurfaceErrorCode::Internal, "goal missing", false)
            })?;
        serde_json::to_value(goal)
            .map_err(|_| SurfaceError::new(SurfaceErrorCode::Internal, "goal encode", false))
    }

    fn goal_set_status(
        &mut self,
        auth: &AuthContext,
        goal_id: &str,
        actor_id: &str,
        status: &str,
        reason: &str,
        now_ms: u128,
    ) -> Result<serde_json::Value, SurfaceError> {
        auth.require("goal:write")?;
        ensure_actor(auth, actor_id)?;
        let status = status
            .parse::<GoalStatus>()
            .map_err(|_| SurfaceError::invalid("unknown goal status"))?;
        let goals = self.goals_mut()?;
        ensure_goal_owner(goals, goal_id, actor_id)?;
        goals
            .with_graph_mut(|graph| graph.set_goal_status(goal_id, status, reason, now_ms))
            .map_err(map_goal_error)?;
        let goal =
            goals.graph().goal(goal_id).cloned().ok_or_else(|| {
                SurfaceError::new(SurfaceErrorCode::Internal, "goal missing", false)
            })?;
        serde_json::to_value(goal)
            .map_err(|_| SurfaceError::new(SurfaceErrorCode::Internal, "goal encode", false))
    }

    fn commitment_create(
        &mut self,
        auth: &AuthContext,
        input: CommitmentCreateInput<'_>,
        now_ms: u128,
    ) -> Result<serde_json::Value, SurfaceError> {
        auth.require("goal:write")?;
        ensure_actor(auth, input.actor_id)?;
        let goals = self.goals_mut()?;
        ensure_goal_owner(goals, input.goal_id, input.actor_id)?;
        goals
            .with_graph_mut(|graph| {
                graph.add_commitment(
                    input.commitment_id,
                    input.goal_id,
                    input.description,
                    input.due_ms,
                    now_ms,
                )
            })
            .map_err(map_goal_error)?;
        let commitment = goals
            .graph()
            .commitment(input.commitment_id)
            .cloned()
            .ok_or_else(|| {
                SurfaceError::new(SurfaceErrorCode::Internal, "commitment missing", false)
            })?;
        serde_json::to_value(commitment)
            .map_err(|_| SurfaceError::new(SurfaceErrorCode::Internal, "commitment encode", false))
    }

    fn commitment_close(
        &mut self,
        auth: &AuthContext,
        commitment_id: &str,
        actor_id: &str,
        status: &str,
        note: &str,
        now_ms: u128,
    ) -> Result<serde_json::Value, SurfaceError> {
        auth.require("goal:write")?;
        ensure_actor(auth, actor_id)?;
        let status = status
            .parse::<CommitmentStatus>()
            .map_err(|_| SurfaceError::invalid("unknown commitment status"))?;
        if status == CommitmentStatus::Pending {
            return Err(SurfaceError::invalid("commitment cannot close as pending"));
        }
        let goals = self.goals_mut()?;
        ensure_commitment_owner(goals, commitment_id, actor_id)?;
        goals
            .with_graph_mut(|graph| graph.close_commitment(commitment_id, status, note, now_ms))
            .map_err(map_goal_error)?;
        let commitment = goals
            .graph()
            .commitment(commitment_id)
            .cloned()
            .ok_or_else(|| {
                SurfaceError::new(SurfaceErrorCode::Internal, "commitment missing", false)
            })?;
        serde_json::to_value(commitment)
            .map_err(|_| SurfaceError::new(SurfaceErrorCode::Internal, "commitment encode", false))
    }

    fn commitment_link_ticket(
        &mut self,
        auth: &AuthContext,
        commitment_id: &str,
        ticket_id: &str,
        actor_id: &str,
        now_ms: u128,
    ) -> Result<serde_json::Value, SurfaceError> {
        auth.require("goal:write")?;
        ensure_actor(auth, actor_id)?;
        if self.graph.graph().ticket(ticket_id).is_none() {
            return Err(SurfaceError::new(
                SurfaceErrorCode::NotFound,
                "ticket not found",
                false,
            ));
        }
        let goals = self.goals_mut()?;
        ensure_commitment_owner(goals, commitment_id, actor_id)?;
        goals
            .with_graph_mut(|graph| graph.link_commitment_ticket(commitment_id, ticket_id, now_ms))
            .map_err(map_goal_error)?;
        let commitment = goals
            .graph()
            .commitment(commitment_id)
            .cloned()
            .ok_or_else(|| {
                SurfaceError::new(SurfaceErrorCode::Internal, "commitment missing", false)
            })?;
        serde_json::to_value(commitment)
            .map_err(|_| SurfaceError::new(SurfaceErrorCode::Internal, "commitment encode", false))
    }

    fn goals_mut(&mut self) -> Result<&mut GoalStore, SurfaceError> {
        self.resources
            .goals
            .as_mut()
            .ok_or_else(|| SurfaceError::new(SurfaceErrorCode::Unavailable, "goal store", false))
    }

    fn commitment_list(
        &self,
        auth: &AuthContext,
        goal_id: Option<&str>,
    ) -> Result<serde_json::Value, SurfaceError> {
        auth.require("goal:read")?;
        let goals =
            self.resources.goals.as_ref().ok_or_else(|| {
                SurfaceError::new(SurfaceErrorCode::Unavailable, "goal store", false)
            })?;
        let commitments: Vec<_> = goals
            .graph()
            .snapshot()
            .commitments
            .into_iter()
            .filter(|commitment| goal_id.is_none_or(|goal_id| commitment.goal_id == goal_id))
            .collect();
        serde_json::to_value(commitments)
            .map_err(|_| SurfaceError::new(SurfaceErrorCode::Internal, "commitment encode", false))
    }

    fn attention_candidates(
        &mut self,
        auth: &AuthContext,
        lead_ms: u128,
        limit: usize,
        include_missed: bool,
        now_ms: u128,
    ) -> Result<serde_json::Value, SurfaceError> {
        auth.require("goal:read")?;
        let goals =
            self.resources.goals.as_ref().ok_or_else(|| {
                SurfaceError::new(SurfaceErrorCode::Unavailable, "goal store", false)
            })?;
        let candidates = attention_candidates(
            goals.graph(),
            now_ms,
            &AttentionPolicy {
                lead_ms,
                max_candidates: limit,
                include_missed,
            },
        );
        let candidates = if let Some(state) = self.resources.attention.as_mut() {
            state.filter_new(candidates, now_ms, state.min_interval_ms())
        } else {
            candidates
        };
        serde_json::to_value(candidates)
            .map_err(|_| SurfaceError::new(SurfaceErrorCode::Internal, "attention encode", false))
    }

    fn context_summary(
        &self,
        auth: &AuthContext,
        bot_id: &str,
        now_ms: u128,
    ) -> Result<serde_json::Value, SurfaceError> {
        let graph = self.context_graph(auth, bot_id, now_ms)?;
        serde_json::to_value(graph)
            .map_err(|_| SurfaceError::new(SurfaceErrorCode::Internal, "context encode", false))
    }

    fn context_explain(
        &self,
        auth: &AuthContext,
        node_id: &str,
        bot_id: &str,
        now_ms: u128,
    ) -> Result<serde_json::Value, SurfaceError> {
        let graph = self.context_graph(auth, bot_id, now_ms)?;
        let explanation = graph.explain(node_id).ok_or_else(|| {
            SurfaceError::new(SurfaceErrorCode::NotFound, "context node not found", false)
        })?;
        serde_json::to_value(explanation)
            .map_err(|_| SurfaceError::new(SurfaceErrorCode::Internal, "context encode", false))
    }

    fn context_graph(
        &self,
        auth: &AuthContext,
        bot_id: &str,
        now_ms: u128,
    ) -> Result<crate::context_graph::ContextGraph, SurfaceError> {
        auth.require("context:read")?;
        auth.require("memory:read")?;
        let empty = crate::goal::GoalGraph::default();
        let goals = self
            .resources
            .goals
            .as_ref()
            .map(|store| store.graph())
            .unwrap_or(&empty);
        let scope = crate::memory::MemoryScope::bot(bot_id);
        let memories: Vec<_> = self
            .resources
            .memory
            .as_ref()
            .map(|memory| {
                memory
                    .store()
                    .records()
                    .filter(|record| record.scope == scope)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        Ok(build_context_graph(
            goals,
            &self.graph.snapshot(),
            &memories,
            now_ms,
        ))
    }

    fn delegation_get(
        &self,
        auth: &AuthContext,
        actor_id: &str,
        domain: &str,
    ) -> Result<serde_json::Value, SurfaceError> {
        auth.require("delegation:read")?;
        let registry = self.resources.delegation.as_ref().ok_or_else(|| {
            SurfaceError::new(SurfaceErrorCode::Unavailable, "delegation registry", false)
        })?;
        let profile = registry
            .registry()
            .profile(actor_id, domain)
            .ok_or_else(|| {
                SurfaceError::new(
                    SurfaceErrorCode::NotFound,
                    "delegation profile not found",
                    false,
                )
            })?;
        serde_json::to_value(profile)
            .map_err(|_| SurfaceError::new(SurfaceErrorCode::Internal, "delegation encode", false))
    }

    fn delegation_register(
        &mut self,
        auth: &AuthContext,
        input: DelegationRegisterInput<'_>,
    ) -> Result<serde_json::Value, SurfaceError> {
        auth.require("delegation:write")?;
        ensure_actor(auth, input.actor_id)?;
        let mode = input
            .mode
            .parse::<DelegationMode>()
            .map_err(|_| SurfaceError::invalid("unknown delegation mode"))?;
        let max_risk = parse_risk(input.max_risk)?;
        let registry = self.resources.delegation.as_mut().ok_or_else(|| {
            SurfaceError::new(SurfaceErrorCode::Unavailable, "delegation registry", false)
        })?;
        registry
            .with_registry_mut(|registry| {
                registry.register(DelegationProfile {
                    profile_id: input.profile_id.into(),
                    actor_id: input.actor_id.into(),
                    domain: input.domain.into(),
                    mode,
                    max_risk,
                    allowed_tools: input.allowed_tools.to_vec(),
                    expires_ms: input.expires_ms,
                })
            })
            .map_err(map_delegation_store_error)?;
        let profile = registry
            .registry()
            .profile(input.actor_id, input.domain)
            .cloned()
            .ok_or_else(|| {
                SurfaceError::new(SurfaceErrorCode::Internal, "delegation missing", false)
            })?;
        serde_json::to_value(profile)
            .map_err(|_| SurfaceError::new(SurfaceErrorCode::Internal, "delegation encode", false))
    }

    fn delegation_revoke(
        &mut self,
        auth: &AuthContext,
        actor_id: &str,
        domain: &str,
        now_ms: u128,
    ) -> Result<serde_json::Value, SurfaceError> {
        auth.require("delegation:write")?;
        ensure_actor(auth, actor_id)?;
        let registry = self.resources.delegation.as_mut().ok_or_else(|| {
            SurfaceError::new(SurfaceErrorCode::Unavailable, "delegation registry", false)
        })?;
        registry
            .with_registry_mut(|registry| registry.revoke(actor_id, domain, now_ms))
            .map_err(map_delegation_store_error)?;
        Ok(serde_json::json!({"actor_id": actor_id, "domain": domain, "revoked": true}))
    }

    fn evolution_propose(
        &mut self,
        auth: &AuthContext,
        input: EvolutionProposalInput<'_>,
        now_ms: u128,
    ) -> Result<serde_json::Value, SurfaceError> {
        auth.require("evolution:write")?;
        ensure_actor(auth, input.proposer)?;
        let kind = parse_evolution_kind(input.kind)?;
        let risk = parse_risk(input.risk)?;
        let store = self.evolution_store_mut()?;
        store
            .with_registry_mut(|registry| {
                registry.propose(
                    EvolutionProposalDraft {
                        proposal_id: input.proposal_id.into(),
                        proposer: input.proposer.into(),
                        kind,
                        summary: input.summary.into(),
                        evidence: input.evidence.to_vec(),
                        rollback_plan: input.rollback_plan.into(),
                        risk,
                    },
                    now_ms,
                )
            })
            .map_err(map_evolution_store_error)?;
        let proposal = store
            .registry()
            .proposal(input.proposal_id)
            .cloned()
            .ok_or_else(|| {
                SurfaceError::new(SurfaceErrorCode::Internal, "proposal missing", false)
            })?;
        serde_json::to_value(proposal)
            .map_err(|_| SurfaceError::new(SurfaceErrorCode::Internal, "proposal encode", false))
    }

    fn evolution_transition(
        &mut self,
        auth: &AuthContext,
        proposal_id: &str,
        actor_id: &str,
        action: &str,
        now_ms: u128,
    ) -> Result<serde_json::Value, SurfaceError> {
        auth.require("evolution:write")?;
        ensure_actor(auth, actor_id)?;
        let store = self.evolution_store_mut()?;
        store
            .with_registry_mut(|registry| match action {
                "approve" => registry.approve(proposal_id, actor_id, now_ms),
                "reject" => registry.reject(proposal_id, actor_id, now_ms),
                "apply" => registry.apply(proposal_id, actor_id, now_ms),
                "rollback" => registry.rollback(proposal_id, actor_id, now_ms),
                _ => Err(EvolutionError::Corrupt("unknown evolution action".into())),
            })
            .map_err(map_evolution_store_error)?;
        let proposal = store
            .registry()
            .proposal(proposal_id)
            .cloned()
            .ok_or_else(|| {
                SurfaceError::new(SurfaceErrorCode::Internal, "proposal missing", false)
            })?;
        serde_json::to_value(proposal)
            .map_err(|_| SurfaceError::new(SurfaceErrorCode::Internal, "proposal encode", false))
    }

    fn evolution_store_mut(&mut self) -> Result<&mut EvolutionStore, SurfaceError> {
        self.resources.evolution.as_mut().ok_or_else(|| {
            SurfaceError::new(SurfaceErrorCode::Unavailable, "evolution registry", false)
        })
    }

    fn evolution_ledger(&self, auth: &AuthContext) -> Result<serde_json::Value, SurfaceError> {
        auth.require("evolution:read")?;
        let store = self.resources.evolution.as_ref().ok_or_else(|| {
            SurfaceError::new(SurfaceErrorCode::Unavailable, "evolution registry", false)
        })?;
        serde_json::to_value(store.registry().ledger())
            .map_err(|_| SurfaceError::new(SurfaceErrorCode::Internal, "evolution encode", false))
    }

    fn action_log_query(
        &self,
        auth: &AuthContext,
        actor: Option<&str>,
        kind: Option<&str>,
        target: Option<&str>,
        after_seq: Option<u64>,
        limit: usize,
    ) -> Result<serde_json::Value, SurfaceError> {
        auth.require("audit:read")?;
        let kind = match kind {
            Some(kind) => Some(
                kind.parse::<ActionKind>()
                    .map_err(|_| SurfaceError::invalid("unknown action kind"))?,
            ),
            None => None,
        };
        let entries = self
            .graph
            .actions()
            .query_page(actor, kind, target, after_seq, limit);
        serde_json::to_value(entries)
            .map_err(|_| SurfaceError::new(SurfaceErrorCode::Internal, "action log encode", false))
    }

    fn action_log_summary(&self, auth: &AuthContext) -> Result<serde_json::Value, SurfaceError> {
        auth.require("audit:read")?;
        serde_json::to_value(self.graph.actions().summary())
            .map_err(|_| SurfaceError::new(SurfaceErrorCode::Internal, "action log encode", false))
    }

    fn memory_recall(
        &self,
        auth: &AuthContext,
        scope: &crate::memory::MemoryScope,
        query: &str,
        limit: usize,
        now_ms: u128,
    ) -> Result<serde_json::Value, SurfaceError> {
        auth.require("memory:read")?;
        let memory = self
            .resources
            .memory
            .as_ref()
            .ok_or_else(|| unavailable("memory service"))?;
        let hits = memory
            .recall(query, scope, now_ms, limit)
            .into_iter()
            .map(|hit| json!({"record": hit.record, "score": hit.score}))
            .collect::<Vec<_>>();
        Ok(serde_json::Value::Array(hits))
    }

    fn memory_forget(
        &mut self,
        auth: &AuthContext,
        memory_id: &str,
        scope: &crate::memory::MemoryScope,
        mode: &str,
        now_ms: u128,
    ) -> Result<serde_json::Value, SurfaceError> {
        auth.require("memory:write")?;
        let (mode, mode_label) = match mode {
            "soft" => (crate::memory::ForgetMode::Soft, "soft"),
            "hard" => (crate::memory::ForgetMode::Hard, "hard"),
            _ => return Err(SurfaceError::invalid("mode must be soft or hard")),
        };
        let memory = self
            .resources
            .memory
            .as_mut()
            .ok_or_else(|| unavailable("memory service"))?;
        memory
            .forget(memory_id, scope, mode, now_ms)
            .map_err(|error| {
                let retryable = matches!(
                    &error,
                    MemoryServiceError::Journal(MemoryJournalError::ConcurrentWrite { .. })
                );
                SurfaceError::new(SurfaceErrorCode::Conflict, error.to_string(), retryable)
            })?;
        Ok(serde_json::json!({"memory_id": memory_id, "forgotten": true, "mode": mode_label}))
    }

    fn skill_list(&self, auth: &AuthContext) -> Result<serde_json::Value, SurfaceError> {
        auth.require("skill:read")?;
        let skills = self
            .resources
            .skills
            .as_ref()
            .ok_or_else(|| unavailable("skill service"))?;
        serde_json::to_value(skills.list())
            .map_err(|_| SurfaceError::new(SurfaceErrorCode::Internal, "skill encode", false))
    }

    fn skill_enable(
        &mut self,
        auth: &AuthContext,
        name: &str,
        version: &str,
        now_ms: u128,
    ) -> Result<serde_json::Value, SurfaceError> {
        auth.require("skill:write")?;
        let skills = self
            .resources
            .skills
            .as_mut()
            .ok_or_else(|| unavailable("skill service"))?;
        skills
            .activate(name, version, now_ms)
            .map_err(map_skill_error)?;
        Ok(serde_json::json!({"name": name, "version": version, "active": true}))
    }

    fn skill_disable(
        &mut self,
        auth: &AuthContext,
        name: &str,
        now_ms: u128,
    ) -> Result<serde_json::Value, SurfaceError> {
        auth.require("skill:write")?;
        let skills = self
            .resources
            .skills
            .as_mut()
            .ok_or_else(|| unavailable("skill service"))?;
        skills.disable(name, now_ms).map_err(map_skill_error)?;
        Ok(serde_json::json!({"name": name, "active": false}))
    }

    fn skill_rollback(
        &mut self,
        auth: &AuthContext,
        name: &str,
        version: &str,
        now_ms: u128,
    ) -> Result<serde_json::Value, SurfaceError> {
        auth.require("skill:write")?;
        let skills = self
            .resources
            .skills
            .as_mut()
            .ok_or_else(|| unavailable("skill service"))?;
        skills
            .rollback(name, version, now_ms)
            .map_err(map_skill_error)?;
        Ok(serde_json::json!({"name": name, "version": version, "active": true}))
    }

    fn audit_query(
        &self,
        auth: &AuthContext,
        kind: &str,
        filter: &AuditFilter,
        now_ms: u128,
    ) -> Result<serde_json::Value, SurfaceError> {
        auth.require("audit:read")?;
        let kind: AuditKind = kind
            .parse()
            .map_err(|_| SurfaceError::invalid("unknown audit kind"))?;
        let path = self
            .resources
            .audit_paths
            .get(&kind)
            .ok_or_else(|| unavailable("audit store"))?;
        let query = AuditQuery {
            thread_id: filter.thread_id.clone(),
            ticket_id: filter.ticket_id.clone(),
            memory_id: filter.memory_id.clone(),
            scope: filter.scope.clone(),
            skill: filter.skill.clone(),
            version: filter.version.clone(),
            limit: filter.limit,
        };
        let result = audit_export(kind, path, &query, None, now_ms).map_err(map_audit_error)?;
        serde_json::to_value(result)
            .map_err(|_| SurfaceError::new(SurfaceErrorCode::Internal, "audit encode", false))
    }

    fn message_send(
        &self,
        auth: &AuthContext,
        request: &SurfaceRequest,
        thread_id: &str,
        channel: &str,
        text: &str,
        now_ms: u128,
    ) -> Result<serde_json::Value, SurfaceError> {
        auth.require("message:send")?;
        let message = OutboundMessage {
            message_id: format!("message-{}", Uuid::new_v4()),
            thread_id: thread_id.into(),
            channel: channel.into(),
            text: text.into(),
            created_ms: now_ms,
            trace_id: request.trace_id.clone(),
        };
        message.validate()?;
        Ok(json!({"accepted": true, "message": message}))
    }
}

fn unavailable(resource: &str) -> SurfaceError {
    SurfaceError::new(
        SurfaceErrorCode::Unavailable,
        format!("{resource} is not configured"),
        false,
    )
}

fn map_audit_error(error: AuditError) -> SurfaceError {
    match error {
        AuditError::NotFound(_) => {
            SurfaceError::new(SurfaceErrorCode::NotFound, "audit store not found", false)
        }
        _ => SurfaceError::new(SurfaceErrorCode::Internal, "audit query failed", false),
    }
}

fn map_governed_error(error: GovernedTaskGraphError) -> SurfaceError {
    match error {
        GovernedTaskGraphError::PolicyDenied(_) => {
            SurfaceError::new(SurfaceErrorCode::Forbidden, "policy denied", false)
        }
        GovernedTaskGraphError::Graph(TaskGraphError::UnknownTicket(_))
        | GovernedTaskGraphError::Graph(TaskGraphError::UnknownThread(_)) => {
            SurfaceError::new(SurfaceErrorCode::NotFound, "resource not found", false)
        }
        GovernedTaskGraphError::Graph(TaskGraphError::UnauthorizedActor(_)) => SurfaceError::new(
            SurfaceErrorCode::Forbidden,
            "actor is not authorized",
            false,
        ),
        GovernedTaskGraphError::Graph(TaskGraphError::ApprovalExpired(_))
        | GovernedTaskGraphError::Graph(TaskGraphError::ApprovalRequired(_)) => {
            SurfaceError::new(SurfaceErrorCode::Conflict, "approval is not active", false)
        }
        GovernedTaskGraphError::Store(TaskGraphStoreError::ConcurrentWrite { .. })
        | GovernedTaskGraphError::ActionJournal(
            crate::action_log_journal::ActionLogJournalError::ConcurrentWrite { .. },
        ) => SurfaceError::new(
            SurfaceErrorCode::Conflict,
            "store changed concurrently; retry the request",
            true,
        ),
        _ => SurfaceError::new(
            SurfaceErrorCode::Internal,
            "governed task graph error",
            false,
        ),
    }
}

struct CommitmentCreateInput<'a> {
    commitment_id: &'a str,
    goal_id: &'a str,
    actor_id: &'a str,
    description: &'a str,
    due_ms: Option<u128>,
}

struct DelegationRegisterInput<'a> {
    profile_id: &'a str,
    actor_id: &'a str,
    domain: &'a str,
    mode: &'a str,
    max_risk: &'a str,
    allowed_tools: &'a [String],
    expires_ms: Option<u128>,
}

struct EvolutionProposalInput<'a> {
    proposal_id: &'a str,
    proposer: &'a str,
    kind: &'a str,
    summary: &'a str,
    evidence: &'a [String],
    rollback_plan: &'a str,
    risk: &'a str,
}

fn parse_evolution_kind(value: &str) -> Result<EvolutionKind, SurfaceError> {
    match value {
        "skill_draft" => Ok(EvolutionKind::SkillDraft),
        "routine_draft" => Ok(EvolutionKind::RoutineDraft),
        "strategy_note" => Ok(EvolutionKind::StrategyNote),
        _ => Err(SurfaceError::invalid("unknown evolution kind")),
    }
}

fn map_evolution_store_error(error: EvolutionStoreError) -> SurfaceError {
    match error {
        EvolutionStoreError::Evolution(error) => map_evolution_error(error),
        EvolutionStoreError::ConcurrentWrite { .. } => SurfaceError::new(
            SurfaceErrorCode::Conflict,
            "evolution store changed concurrently; retry the request",
            true,
        ),
        EvolutionStoreError::Io(_)
        | EvolutionStoreError::Json(_)
        | EvolutionStoreError::Corrupt(_) => {
            SurfaceError::new(SurfaceErrorCode::Internal, error.to_string(), false)
        }
    }
}

fn map_evolution_error(error: EvolutionError) -> SurfaceError {
    match error {
        EvolutionError::EmptyProposalId
        | EvolutionError::EmptyProposer
        | EvolutionError::InvalidSummary
        | EvolutionError::MissingEvidence
        | EvolutionError::EmptyEvidence
        | EvolutionError::MissingRollback => SurfaceError::invalid(error.to_string()),
        EvolutionError::ProposalExists(_) | EvolutionError::InvalidState(_, _) => {
            SurfaceError::new(SurfaceErrorCode::Conflict, error.to_string(), false)
        }
        EvolutionError::UnknownProposal(_) => {
            SurfaceError::new(SurfaceErrorCode::NotFound, error.to_string(), false)
        }
        EvolutionError::SelfApproval => SurfaceError::forbidden(error.to_string()),
        EvolutionError::HashMismatch(_)
        | EvolutionError::ProposalDigestMismatch(_)
        | EvolutionError::NonContiguousSequence
        | EvolutionError::Json(_)
        | EvolutionError::Corrupt(_) => {
            SurfaceError::new(SurfaceErrorCode::Internal, error.to_string(), false)
        }
    }
}

fn map_skill_error(error: SkillServiceError) -> SurfaceError {
    let retryable = matches!(
        &error,
        SkillServiceError::Journal(SkillJournalError::ConcurrentWrite { .. })
    );
    SurfaceError::new(SurfaceErrorCode::Conflict, error.to_string(), retryable)
}

fn parse_risk(value: &str) -> Result<RiskLevel, SurfaceError> {
    match value {
        "low" => Ok(RiskLevel::Low),
        "medium" => Ok(RiskLevel::Medium),
        "high" => Ok(RiskLevel::High),
        _ => Err(SurfaceError::invalid("unknown risk level")),
    }
}

fn map_delegation_store_error(error: DelegationStoreError) -> SurfaceError {
    match error {
        DelegationStoreError::Delegation(error) => map_delegation_error(error),
        DelegationStoreError::ConcurrentWrite { .. } => SurfaceError::new(
            SurfaceErrorCode::Conflict,
            "delegation store changed concurrently; retry the request",
            true,
        ),
        DelegationStoreError::Io(_)
        | DelegationStoreError::Json(_)
        | DelegationStoreError::Corrupt(_) => {
            SurfaceError::new(SurfaceErrorCode::Internal, error.to_string(), false)
        }
    }
}

fn map_delegation_error(error: DelegationError) -> SurfaceError {
    match error {
        DelegationError::EmptyProfileId
        | DelegationError::EmptyActor
        | DelegationError::EmptyDomain
        | DelegationError::ModeExceeded { .. }
        | DelegationError::RiskExceeded { .. }
        | DelegationError::ToolNotAllowed(_)
        | DelegationError::ApprovalRequired
        | DelegationError::ReversibilityRequired => SurfaceError::invalid(error.to_string()),
        DelegationError::DuplicateProfile(_, _) | DelegationError::Revoked => {
            SurfaceError::new(SurfaceErrorCode::Conflict, error.to_string(), false)
        }
        DelegationError::UnknownProfile(_, _) => {
            SurfaceError::new(SurfaceErrorCode::NotFound, error.to_string(), false)
        }
        DelegationError::Expired | DelegationError::AuthorityDenied(_) => {
            SurfaceError::forbidden(error.to_string())
        }
    }
}

fn ensure_actor(auth: &AuthContext, actor_id: &str) -> Result<(), SurfaceError> {
    if auth.actor_id != actor_id {
        return Err(SurfaceError::forbidden(
            "actor_id must match the authenticated actor",
        ));
    }
    Ok(())
}

fn ensure_goal_owner(goals: &GoalStore, goal_id: &str, actor_id: &str) -> Result<(), SurfaceError> {
    let goal = goals
        .graph()
        .goal(goal_id)
        .ok_or_else(|| SurfaceError::new(SurfaceErrorCode::NotFound, "goal not found", false))?;
    if goal.owner_id != actor_id {
        return Err(SurfaceError::forbidden("actor does not own the goal"));
    }
    Ok(())
}

fn ensure_commitment_owner(
    goals: &GoalStore,
    commitment_id: &str,
    actor_id: &str,
) -> Result<(), SurfaceError> {
    let commitment = goals.graph().commitment(commitment_id).ok_or_else(|| {
        SurfaceError::new(SurfaceErrorCode::NotFound, "commitment not found", false)
    })?;
    ensure_goal_owner(goals, &commitment.goal_id, actor_id)
}

fn map_goal_error(error: GoalError) -> SurfaceError {
    match error {
        GoalError::EmptyGoalId
        | GoalError::EmptyOwner
        | GoalError::InvalidTitle
        | GoalError::EmptyCommitmentId
        | GoalError::InvalidDescription
        | GoalError::EmptyTicketId => SurfaceError::invalid(error.to_string()),
        GoalError::GoalExists(_)
        | GoalError::CommitmentExists(_)
        | GoalError::GoalNotActive(_)
        | GoalError::CommitmentNotPending(_)
        | GoalError::InvalidGoalTransition(_, _) => {
            SurfaceError::new(SurfaceErrorCode::Conflict, error.to_string(), false)
        }
        GoalError::ConcurrentWrite { .. } => {
            SurfaceError::new(SurfaceErrorCode::Conflict, error.to_string(), true)
        }
        GoalError::UnknownGoal(_) | GoalError::UnknownCommitment(_) => {
            SurfaceError::new(SurfaceErrorCode::NotFound, error.to_string(), false)
        }
        GoalError::HashMismatch(_)
        | GoalError::NonContiguousSequence
        | GoalError::Io(_)
        | GoalError::Json(_) => {
            SurfaceError::new(SurfaceErrorCode::Internal, error.to_string(), false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authority::AuthorityProfile;
    use crate::domain::RiskLevel;
    use crate::memory::{
        MemoryKind, MemoryLayer, MemoryRecord, MemoryScope, MemorySource, MemorySourceKind,
    };
    use crate::skill::{SkillManifest, SkillRisk};
    use crate::task_graph::{Decision, MandateStrength, TicketDraft, Trigger, TriggerSource};

    #[test]
    fn concurrent_task_graph_conflict_maps_to_retryable_conflict() {
        let error = map_governed_error(GovernedTaskGraphError::Store(
            TaskGraphStoreError::ConcurrentWrite {
                expected_bytes: 1,
                actual_bytes: 2,
            },
        ));
        assert_eq!(error.code, SurfaceErrorCode::Conflict);
        assert!(error.retryable);
    }

    fn service(dir: &std::path::Path) -> (SurfaceService, String) {
        let mut graph = GovernedTaskGraph::open(dir.join("task-graph.jsonl")).unwrap();
        graph
            .register_profile(AuthorityProfile {
                profile_id: "owner-profile".into(),
                actor_id: "owner-1".into(),
                scopes: vec!["read*".into()],
                max_strength: MandateStrength::ReadOnly,
                allowed_tools: Vec::new(),
                denied_tools: Vec::new(),
                max_risk: RiskLevel::Medium,
                expires_ms: None,
            })
            .unwrap();
        graph.bind_actor("owner-1", "owner-profile").unwrap();
        let thread_id = graph.open_thread("surface", 10).unwrap();
        let trigger_id = graph
            .ingest_trigger(
                &thread_id,
                Trigger {
                    trigger_id: "trigger-1".into(),
                    source: TriggerSource::Web,
                    source_event_id: "event-1".into(),
                    summary: "surface".into(),
                    received_ms: 1,
                },
                11,
            )
            .unwrap()
            .trigger_id;
        let eval_id = graph.begin_eval(&thread_id, &trigger_id, 12).unwrap();
        graph
            .decide_eval(&eval_id, Decision::Accept, "ok", 13)
            .unwrap();
        let ticket_id = graph
            .create_ticket_from_eval(
                &eval_id,
                "owner-1",
                TicketDraft {
                    owner_id: "owner-1".into(),
                    scope: "read".into(),
                    strength: MandateStrength::ReadOnly,
                    policy_version: "v1".into(),
                    requires_approval: true,
                },
                14,
            )
            .unwrap();
        (SurfaceService::from_graph(graph), ticket_id)
    }

    fn request(ticket_id: &str) -> SurfaceRequest {
        SurfaceRequest {
            protocol_version: "v0".into(),
            request_id: "request-1".into(),
            trace_id: "trace-1".into(),
            idempotency_key: Some("idem-1".into()),
            method: SurfaceMethod::TicketApprove {
                ticket_id: ticket_id.into(),
                actor_id: "owner-1".into(),
                decision: "allow_once".into(),
                expires_ms: None,
            },
        }
    }

    fn auth() -> AuthContext {
        AuthContext {
            actor_id: "owner-1".into(),
            scopes: vec![
                "ticket:approve".into(),
                "ticket:read".into(),
                "thread:read".into(),
            ],
        }
    }

    #[test]
    fn concurrent_external_write_is_retryable_not_wedged() {
        let dir = tempfile::tempdir().unwrap();
        let (mut service, ticket_id) = service(dir.path());
        let mut external =
            crate::task_graph_store::TaskGraphService::open(dir.path().join("task-graph.jsonl"))
                .unwrap();
        external
            .with_graph_mut(|graph| graph.open_thread("external", 20))
            .unwrap();

        let response = service.handle(&request(&ticket_id), &auth(), 20);
        assert!(!response.ok);
        let error = response.error.unwrap();
        assert_eq!(error.code, SurfaceErrorCode::Conflict);
        assert!(error.retryable);

        let mut retry = request(&ticket_id);
        retry.request_id = "request-2".into();
        retry.idempotency_key = Some("idem-2".into());
        let response = service.handle(&retry, &auth(), 21);
        assert!(response.ok, "{:?}", response.error);
    }

    #[test]
    fn surface_approve_is_authorized_and_policy_checked() {
        let dir = tempfile::tempdir().unwrap();
        let (mut service, ticket_id) = service(dir.path());
        let response = service.handle(&request(&ticket_id), &auth(), 20);
        assert!(response.ok, "{response:?}");
        assert_eq!(
            service.graph().graph().ticket(&ticket_id).unwrap().status,
            crate::task_graph::TicketStatus::Approved
        );
    }

    #[test]
    fn surface_rejects_missing_scope_actor_mismatch_and_policy_denial() {
        let dir = tempfile::tempdir().unwrap();
        let (mut service, ticket_id) = service(dir.path());
        let no_scope = AuthContext {
            actor_id: "owner-1".into(),
            scopes: Vec::new(),
        };
        let response = service.handle(&request(&ticket_id), &no_scope, 20);
        assert!(!response.ok);
        assert_eq!(response.error.unwrap().code, SurfaceErrorCode::Forbidden);

        let mut mismatched = request(&ticket_id);
        if let SurfaceMethod::TicketApprove { actor_id, .. } = &mut mismatched.method {
            *actor_id = "attacker".into();
        }
        let response = service.handle(&mismatched, &auth(), 20);
        assert!(!response.ok);

        let unbound = AuthContext {
            actor_id: "attacker".into(),
            scopes: vec!["ticket:approve".into()],
        };
        let mut attacker = request(&ticket_id);
        if let SurfaceMethod::TicketApprove { actor_id, .. } = &mut attacker.method {
            *actor_id = "attacker".into();
        }
        let response = service.handle(&attacker, &unbound, 20);
        assert!(!response.ok);
        assert_eq!(response.error.unwrap().code, SurfaceErrorCode::Forbidden);
    }

    #[test]
    fn surface_memory_skill_audit_and_message_handlers() {
        let dir = tempfile::tempdir().unwrap();
        let (mut service, _ticket_id) = service(dir.path());
        let mut memory = MemoryService::open(
            dir.path().join("memory-journal.jsonl"),
            dir.path().join("payloads"),
            dir.path().join("memory.key"),
        )
        .unwrap();
        memory
            .remember_at(
                MemoryRecord::new(
                    "mem-1",
                    MemoryLayer::Semantic,
                    MemoryKind::Preference,
                    MemoryScope::bot("bot-1"),
                    "prefers pnpm",
                    MemorySource::new(MemorySourceKind::UserMessage, "event-1"),
                ),
                10,
            )
            .unwrap();
        let mut skills = SkillService::open(dir.path().join("skill-journal.jsonl")).unwrap();
        skills
            .register(
                SkillManifest {
                    name: "release".into(),
                    description: "release process".into(),
                    version: "1.0.0".into(),
                    platforms: vec!["linux".into()],
                    scope: "repo".into(),
                    requires_tools: Vec::new(),
                    risk: SkillRisk::Low,
                    body: "1. verify".into(),
                    content_sha256: String::new(),
                },
                10,
            )
            .unwrap();
        let mut resources = SurfaceResources {
            memory: Some(memory),
            skills: Some(skills),
            ..SurfaceResources::default()
        };
        resources
            .audit_paths
            .insert(AuditKind::TaskGraph, dir.path().join("task-graph.jsonl"));
        service = service.with_resources(resources);

        let auth = AuthContext {
            actor_id: "owner-1".into(),
            scopes: vec![
                "memory:read".into(),
                "memory:write".into(),
                "skill:read".into(),
                "skill:write".into(),
                "audit:read".into(),
                "message:send".into(),
            ],
        };
        let request = SurfaceRequest {
            protocol_version: "v0".into(),
            request_id: "request-1".into(),
            trace_id: "trace-1".into(),
            idempotency_key: None,
            method: SurfaceMethod::MemoryRecall {
                scope: MemoryScope::bot("bot-1"),
                query: "pnpm".into(),
                limit: 5,
            },
        };
        assert!(service.handle(&request, &auth, 20).ok);

        let request = SurfaceRequest {
            method: SurfaceMethod::SkillList,
            ..request
        };
        assert!(service.handle(&request, &auth, 20).ok);

        let request = SurfaceRequest {
            method: SurfaceMethod::SkillEnable {
                name: "release".into(),
                version: "1.0.0".into(),
            },
            ..request
        };
        assert!(service.handle(&request, &auth, 20).ok);
        let request = SurfaceRequest {
            method: SurfaceMethod::SkillDisable {
                name: "release".into(),
            },
            ..request
        };
        assert!(service.handle(&request, &auth, 20).ok);
        let request = SurfaceRequest {
            method: SurfaceMethod::SkillEnable {
                name: "release".into(),
                version: "1.0.0".into(),
            },
            ..request
        };
        assert!(service.handle(&request, &auth, 20).ok);
        let request = SurfaceRequest {
            method: SurfaceMethod::SkillRollback {
                name: "release".into(),
                version: "1.0.0".into(),
            },
            ..request
        };
        assert!(service.handle(&request, &auth, 20).ok);

        let request = SurfaceRequest {
            method: SurfaceMethod::MemoryForget {
                memory_id: "mem-1".into(),
                scope: MemoryScope::bot("bot-1"),
                mode: "hard".into(),
            },
            ..request
        };
        let response = service.handle(&request, &auth, 21);
        assert!(response.ok, "{response:?}");
        assert!(
            !service
                .resources()
                .memory
                .as_ref()
                .unwrap()
                .payload_contains("mem-1", &MemoryScope::bot("bot-1"))
                .unwrap()
        );

        let request = SurfaceRequest {
            method: SurfaceMethod::AuditQuery {
                kind: "task-graph".into(),
                filter: AuditFilter::default(),
            },
            ..request
        };
        assert!(service.handle(&request, &auth, 20).ok);

        let request = SurfaceRequest {
            method: SurfaceMethod::MessageSend {
                thread_id: "thread-1".into(),
                channel: "web".into(),
                text: "hello".into(),
            },
            ..request
        };
        let response = service.handle(&request, &auth, 20);
        assert!(response.ok, "{response:?}");
        assert_eq!(response.result.unwrap()["accepted"], true);
    }

    fn thread_request(limit: Option<usize>, cursor: Option<&str>) -> SurfaceRequest {
        SurfaceRequest {
            protocol_version: "v0".into(),
            request_id: "request-threads".into(),
            trace_id: "trace-threads".into(),
            idempotency_key: None,
            method: SurfaceMethod::ThreadList {
                limit,
                cursor: cursor.map(str::to_string),
            },
        }
    }

    fn thread_ids(response: &crate::surface_api::SurfaceResponse) -> Vec<String> {
        response
            .result
            .as_ref()
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|thread| thread["thread_id"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn surface_v1_goal_attention_context_delegation_and_evolution() {
        use crate::MemoryService;
        use crate::attention::AttentionState;
        use crate::delegation::{DelegationMode, DelegationProfile, DelegationStore};
        use crate::domain::RiskLevel;
        use crate::evolution::{EvolutionKind, EvolutionProposalDraft, EvolutionStore};
        use crate::goal::GoalStore;
        use crate::memory::{
            MemoryKind, MemoryLayer, MemoryRecord, MemoryScope, MemorySource, MemorySourceKind,
        };

        let dir = tempfile::tempdir().unwrap();
        let (mut service, _) = service(dir.path());
        let mut memory = MemoryService::open(
            dir.path().join("memory-v1.jsonl"),
            dir.path().join("payloads-v1"),
            dir.path().join("memory-v1.key"),
        )
        .unwrap();
        for (id, bot) in [("mem-bot-1", "bot-1"), ("mem-bot-2", "bot-2")] {
            memory
                .remember_at(
                    MemoryRecord::new(
                        id,
                        MemoryLayer::Semantic,
                        MemoryKind::Preference,
                        MemoryScope::bot(bot),
                        format!("{bot} secret"),
                        MemorySource::new(MemorySourceKind::UserMessage, "event"),
                    ),
                    13,
                )
                .unwrap();
        }
        let mut goals = GoalStore::open(dir.path().join("goals-v1.jsonl")).unwrap();
        goals
            .with_graph_mut(|graph| {
                graph.create_goal("goal-1", "owner-1", "ship", Some(500), 10)?;
                graph.add_commitment("commit-1", "goal-1", "write notes", Some(200), 11)
            })
            .unwrap();
        let mut delegation = DelegationStore::open(dir.path().join("delegation-v1.json")).unwrap();
        delegation
            .with_registry_mut(|registry| {
                registry.register(DelegationProfile {
                    profile_id: "delegation-1".into(),
                    actor_id: "owner-1".into(),
                    domain: "repo:1-agent".into(),
                    mode: DelegationMode::Suggest,
                    max_risk: RiskLevel::Low,
                    allowed_tools: Vec::new(),
                    expires_ms: None,
                })
            })
            .unwrap();
        let mut evolution = EvolutionStore::open(dir.path().join("evolution-v1.json")).unwrap();
        evolution
            .with_registry_mut(|registry| {
                registry.propose(
                    EvolutionProposalDraft {
                        proposal_id: "proposal-1".into(),
                        proposer: "owner-1".into(),
                        kind: EvolutionKind::StrategyNote,
                        summary: "tune reminders".into(),
                        evidence: vec!["journal:1".into()],
                        rollback_plan: "restore previous policy".into(),
                        risk: RiskLevel::Low,
                    },
                    12,
                )
            })
            .unwrap();
        service = service.with_resources(SurfaceResources {
            memory: Some(memory),
            goals: Some(goals),
            attention: Some(AttentionState::new(1_000)),
            delegation: Some(delegation),
            evolution: Some(evolution),
            ..SurfaceResources::default()
        });
        let auth = AuthContext {
            actor_id: "owner-1".into(),
            scopes: vec![
                "goal:read".into(),
                "context:read".into(),
                "memory:read".into(),
                "delegation:read".into(),
                "delegation:write".into(),
                "evolution:read".into(),
                "evolution:write".into(),
            ],
        };

        let goals_response = service.handle(
            &SurfaceRequest {
                protocol_version: "v0".into(),
                request_id: "req-goals".into(),
                trace_id: "trace".into(),
                idempotency_key: None,
                method: SurfaceMethod::GoalList,
            },
            &auth,
            100,
        );
        assert!(goals_response.ok, "{goals_response:?}");
        assert_eq!(
            goals_response.result.unwrap()["goals"]
                .as_array()
                .unwrap()
                .len(),
            1
        );

        let attention = service.handle(
            &SurfaceRequest {
                protocol_version: "v0".into(),
                request_id: "req-attention".into(),
                trace_id: "trace".into(),
                idempotency_key: None,
                method: SurfaceMethod::AttentionCandidates {
                    lead_ms: 1_000,
                    limit: 10,
                    include_missed: true,
                },
            },
            &auth,
            100,
        );
        assert!(attention.ok, "{attention:?}");
        let candidates = attention.result.unwrap();
        assert_eq!(candidates.as_array().unwrap().len(), 2);

        let repeated = service.handle(
            &SurfaceRequest {
                protocol_version: "v0".into(),
                request_id: "req-attention-repeat".into(),
                trace_id: "trace".into(),
                idempotency_key: None,
                method: SurfaceMethod::AttentionCandidates {
                    lead_ms: 1_000,
                    limit: 10,
                    include_missed: true,
                },
            },
            &auth,
            100,
        );
        assert!(repeated.ok, "{repeated:?}");
        assert!(repeated.result.unwrap().as_array().unwrap().is_empty());

        let context = service.handle(
            &SurfaceRequest {
                protocol_version: "v0".into(),
                request_id: "req-context".into(),
                trace_id: "trace".into(),
                idempotency_key: None,
                method: SurfaceMethod::ContextSummary {
                    bot_id: "bot-1".into(),
                },
            },
            &auth,
            100,
        );
        assert!(context.ok, "{context:?}");
        let context = context.result.unwrap();
        assert!(!context["edges"].as_array().unwrap().is_empty());
        let memory_nodes: Vec<_> = context["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|node| node["kind"] == "memory")
            .collect();
        assert_eq!(memory_nodes.len(), 1);
        assert_eq!(memory_nodes[0]["id"], "memory:mem-bot-1");

        let explained = service.handle(
            &SurfaceRequest {
                protocol_version: "v0".into(),
                request_id: "req-context-explain".into(),
                trace_id: "trace".into(),
                idempotency_key: None,
                method: SurfaceMethod::ContextExplain {
                    node_id: "commitment:commit-1".into(),
                    bot_id: "bot-1".into(),
                },
            },
            &auth,
            100,
        );
        assert!(explained.ok, "{explained:?}");
        let explained = explained.result.unwrap();
        assert_eq!(explained["node"]["id"], "commitment:commit-1");
        assert_eq!(explained["incoming"].as_array().unwrap().len(), 1);

        let hidden = service.handle(
            &SurfaceRequest {
                protocol_version: "v0".into(),
                request_id: "req-context-hidden".into(),
                trace_id: "trace".into(),
                idempotency_key: None,
                method: SurfaceMethod::ContextExplain {
                    node_id: "memory:mem-bot-2".into(),
                    bot_id: "bot-1".into(),
                },
            },
            &auth,
            100,
        );
        assert!(!hidden.ok);
        assert_eq!(hidden.error.unwrap().code, SurfaceErrorCode::NotFound);

        let without_memory_scope = AuthContext {
            actor_id: "owner-1".into(),
            scopes: vec!["context:read".into()],
        };
        let denied = service.handle(
            &SurfaceRequest {
                protocol_version: "v0".into(),
                request_id: "req-context-denied".into(),
                trace_id: "trace".into(),
                idempotency_key: None,
                method: SurfaceMethod::ContextSummary {
                    bot_id: "bot-1".into(),
                },
            },
            &without_memory_scope,
            100,
        );
        assert!(!denied.ok);
        assert_eq!(denied.error.unwrap().code, SurfaceErrorCode::Forbidden);

        let delegation_response = service.handle(
            &SurfaceRequest {
                protocol_version: "v0".into(),
                request_id: "req-delegation".into(),
                trace_id: "trace".into(),
                idempotency_key: None,
                method: SurfaceMethod::DelegationGet {
                    actor_id: "owner-1".into(),
                    domain: "repo:1-agent".into(),
                },
            },
            &auth,
            100,
        );
        assert!(delegation_response.ok, "{delegation_response:?}");
        assert_eq!(delegation_response.result.unwrap()["mode"], "suggest");

        let registered = service.handle(
            &SurfaceRequest {
                protocol_version: "v0".into(),
                request_id: "req-delegation-register".into(),
                trace_id: "trace".into(),
                idempotency_key: None,
                method: SurfaceMethod::DelegationRegister {
                    profile_id: "delegation-2".into(),
                    actor_id: "owner-1".into(),
                    domain: "repo:other".into(),
                    mode: "observe".into(),
                    max_risk: "low".into(),
                    allowed_tools: Vec::new(),
                    expires_ms: None,
                },
            },
            &auth,
            100,
        );
        assert!(registered.ok, "{registered:?}");
        assert_eq!(registered.result.unwrap()["mode"], "observe");

        let revoked = service.handle(
            &SurfaceRequest {
                protocol_version: "v0".into(),
                request_id: "req-delegation-revoke".into(),
                trace_id: "trace".into(),
                idempotency_key: None,
                method: SurfaceMethod::DelegationRevoke {
                    actor_id: "owner-1".into(),
                    domain: "repo:other".into(),
                },
            },
            &auth,
            101,
        );
        assert!(revoked.ok, "{revoked:?}");
        assert_eq!(revoked.result.unwrap()["revoked"], true);

        let proposal = service.handle(
            &SurfaceRequest {
                protocol_version: "v0".into(),
                request_id: "req-evolution-propose".into(),
                trace_id: "trace".into(),
                idempotency_key: None,
                method: SurfaceMethod::EvolutionPropose {
                    proposal_id: "proposal-2".into(),
                    proposer: "owner-1".into(),
                    kind: "strategy_note".into(),
                    summary: "adjust reminders".into(),
                    evidence: vec!["journal:2".into()],
                    rollback_plan: "restore previous".into(),
                    risk: "low".into(),
                },
            },
            &auth,
            102,
        );
        assert!(proposal.ok, "{proposal:?}");

        let owner_two = AuthContext {
            actor_id: "owner-2".into(),
            scopes: vec!["evolution:write".into()],
        };
        let approved = service.handle(
            &SurfaceRequest {
                protocol_version: "v0".into(),
                request_id: "req-evolution-approve".into(),
                trace_id: "trace".into(),
                idempotency_key: None,
                method: SurfaceMethod::EvolutionTransition {
                    proposal_id: "proposal-2".into(),
                    actor_id: "owner-2".into(),
                    action: "approve".into(),
                },
            },
            &owner_two,
            103,
        );
        assert!(approved.ok, "{approved:?}");
        let applied = service.handle(
            &SurfaceRequest {
                protocol_version: "v0".into(),
                request_id: "req-evolution-apply".into(),
                trace_id: "trace".into(),
                idempotency_key: None,
                method: SurfaceMethod::EvolutionTransition {
                    proposal_id: "proposal-2".into(),
                    actor_id: "owner-2".into(),
                    action: "apply".into(),
                },
            },
            &owner_two,
            104,
        );
        assert!(applied.ok, "{applied:?}");

        let ledger = service.handle(
            &SurfaceRequest {
                protocol_version: "v0".into(),
                request_id: "req-evolution".into(),
                trace_id: "trace".into(),
                idempotency_key: None,
                method: SurfaceMethod::EvolutionLedger,
            },
            &auth,
            100,
        );
        assert!(ledger.ok, "{ledger:?}");
        assert_eq!(
            ledger.result.unwrap()["receipts"].as_array().unwrap().len(),
            4
        );
    }

    #[test]
    fn open_persists_the_action_log_across_restarts() {
        use crate::authority::AuthorityProfile;
        use crate::domain::RiskLevel;
        use crate::task_graph::{Decision, MandateStrength, TicketDraft, Trigger, TriggerSource};

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("task-graph.jsonl");
        let mut service = SurfaceService::open(&path).unwrap();
        service
            .graph_mut()
            .register_profile(AuthorityProfile {
                profile_id: "owner-profile".into(),
                actor_id: "owner-1".into(),
                scopes: vec!["read repo*".into()],
                max_strength: MandateStrength::ReadOnly,
                allowed_tools: Vec::new(),
                denied_tools: Vec::new(),
                max_risk: RiskLevel::Medium,
                expires_ms: None,
            })
            .unwrap();
        service
            .graph_mut()
            .bind_actor("owner-1", "owner-profile")
            .unwrap();
        let ticket_id = {
            let thread_id = service.graph_mut().open_thread("restart", 1).unwrap();
            let trigger_id = service
                .graph_mut()
                .ingest_trigger(
                    &thread_id,
                    Trigger {
                        trigger_id: "trigger".into(),
                        source: TriggerSource::Web,
                        source_event_id: "event".into(),
                        summary: "restart".into(),
                        received_ms: 1,
                    },
                    2,
                )
                .unwrap()
                .trigger_id;
            let eval_id = service
                .graph_mut()
                .begin_eval(&thread_id, &trigger_id, 3)
                .unwrap();
            service
                .graph_mut()
                .decide_eval(&eval_id, Decision::Accept, "ok", 4)
                .unwrap();
            service
                .graph_mut()
                .create_ticket_from_eval(
                    &eval_id,
                    "owner-1",
                    TicketDraft {
                        owner_id: "owner-1".into(),
                        scope: "read repo".into(),
                        strength: MandateStrength::ReadOnly,
                        policy_version: "v1".into(),
                        requires_approval: true,
                    },
                    5,
                )
                .unwrap()
        };
        service
            .graph_mut()
            .approve_ticket(
                &ticket_id,
                "owner-1",
                crate::task_graph::ApprovalDecision::AllowOnce,
                None,
                6,
            )
            .unwrap();
        drop(service);

        let reopened = SurfaceService::open(&path).unwrap();
        assert_eq!(reopened.graph().actions().len(), 1);
        reopened.graph().actions().verify().unwrap();

        let other = SurfaceService::open(dir.path().join("other-task-graph.jsonl")).unwrap();
        assert!(other.graph().actions().is_empty());
    }

    #[test]
    fn surface_goal_writes_are_owner_scoped_and_persisted() {
        use crate::goal::{CommitmentStatus, GoalStatus, GoalStore};

        let dir = tempfile::tempdir().unwrap();
        let (mut service, ticket_id) = service(dir.path());
        let goal_path = dir.path().join("goals.jsonl");
        service = service.with_resources(SurfaceResources {
            goals: Some(GoalStore::open(&goal_path).unwrap()),
            ..SurfaceResources::default()
        });
        let auth = AuthContext {
            actor_id: "owner-1".into(),
            scopes: vec!["goal:read".into(), "goal:write".into()],
        };
        let request = |id: &str, method| SurfaceRequest {
            protocol_version: "v0".into(),
            request_id: id.into(),
            trace_id: "trace-goals".into(),
            idempotency_key: None,
            method,
        };

        let created = service.handle(
            &request(
                "req-goal-create",
                SurfaceMethod::GoalCreate {
                    goal_id: "goal-1".into(),
                    actor_id: "owner-1".into(),
                    title: "ship".into(),
                    due_ms: Some(500),
                },
            ),
            &auth,
            100,
        );
        assert!(created.ok, "{created:?}");

        let commitment = service.handle(
            &request(
                "req-commitment-create",
                SurfaceMethod::CommitmentCreate {
                    commitment_id: "commit-1".into(),
                    goal_id: "goal-1".into(),
                    actor_id: "owner-1".into(),
                    description: "write notes".into(),
                    due_ms: Some(200),
                },
            ),
            &auth,
            101,
        );
        assert!(commitment.ok, "{commitment:?}");

        let linked = service.handle(
            &request(
                "req-commitment-link",
                SurfaceMethod::CommitmentLinkTicket {
                    commitment_id: "commit-1".into(),
                    ticket_id: ticket_id.clone(),
                    actor_id: "owner-1".into(),
                },
            ),
            &auth,
            102,
        );
        assert!(linked.ok, "{linked:?}");

        let closed = service.handle(
            &request(
                "req-commitment-close",
                SurfaceMethod::CommitmentClose {
                    commitment_id: "commit-1".into(),
                    actor_id: "owner-1".into(),
                    status: "kept".into(),
                    note: "done".into(),
                },
            ),
            &auth,
            103,
        );
        assert!(closed.ok, "{closed:?}");

        let paused = service.handle(
            &request(
                "req-goal-status",
                SurfaceMethod::GoalSetStatus {
                    goal_id: "goal-1".into(),
                    actor_id: "owner-1".into(),
                    status: "paused".into(),
                    reason: "waiting".into(),
                },
            ),
            &auth,
            104,
        );
        assert!(paused.ok, "{paused:?}");

        let attacker = service.handle(
            &request(
                "req-goal-attacker",
                SurfaceMethod::GoalCreate {
                    goal_id: "goal-attacker".into(),
                    actor_id: "owner-1".into(),
                    title: "bad".into(),
                    due_ms: None,
                },
            ),
            &AuthContext {
                actor_id: "attacker".into(),
                scopes: vec!["goal:write".into()],
            },
            105,
        );
        assert!(!attacker.ok);
        assert_eq!(attacker.error.unwrap().code, SurfaceErrorCode::Forbidden);

        let reopened = GoalStore::open(&goal_path).unwrap();
        let goal = reopened.graph().goal("goal-1").unwrap();
        assert_eq!(goal.status, GoalStatus::Paused);
        let commitment = reopened.graph().commitment("commit-1").unwrap();
        assert_eq!(commitment.status, CommitmentStatus::Kept);
        assert_eq!(commitment.ticket_id.as_deref(), Some(ticket_id.as_str()));
    }

    #[test]
    fn action_log_query_reads_security_actions() {
        let dir = tempfile::tempdir().unwrap();
        let (mut service, ticket_id) = service(dir.path());
        let auth = AuthContext {
            actor_id: "owner-1".into(),
            scopes: vec!["ticket:approve".into(), "audit:read".into()],
        };
        assert!(service.handle(&request(&ticket_id), &auth, 20).ok);
        let query = SurfaceRequest {
            protocol_version: "v0".into(),
            request_id: "request-actions".into(),
            trace_id: "trace-actions".into(),
            idempotency_key: None,
            method: SurfaceMethod::ActionLogQuery {
                actor: Some("owner-1".into()),
                kind: Some("approve".into()),
                target: Some(ticket_id.clone()),
                after_seq: None,
                limit: 10,
            },
        };
        let response = service.handle(&query, &auth, 21);
        assert!(response.ok, "{response:?}");
        let entries = response.result.unwrap();
        let entries = entries.as_array().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["kind"], "approve");
        assert_eq!(entries[0]["actor"], "owner-1");

        let summary = service.handle(
            &SurfaceRequest {
                protocol_version: "v0".into(),
                request_id: "request-action-summary".into(),
                trace_id: "trace-actions".into(),
                idempotency_key: None,
                method: SurfaceMethod::ActionLogSummary,
            },
            &auth,
            22,
        );
        assert!(summary.ok, "{summary:?}");
        assert_eq!(summary.result.unwrap()["total"], 1);

        let mut invalid = query;
        if let SurfaceMethod::ActionLogQuery { kind, .. } = &mut invalid.method {
            *kind = Some("bogus".into());
        }
        let response = service.handle(&invalid, &auth, 22);
        assert!(!response.ok);
        assert_eq!(
            response.error.unwrap().code,
            SurfaceErrorCode::InvalidRequest
        );
    }

    #[test]
    fn thread_list_paginates_by_cursor() {
        let dir = tempfile::tempdir().unwrap();
        let mut service = SurfaceService::open(dir.path().join("graph.jsonl")).unwrap();
        service.graph_mut().open_thread("first", 1).unwrap();
        service.graph_mut().open_thread("second", 2).unwrap();
        service.graph_mut().open_thread("third", 3).unwrap();
        let ordered: Vec<String> = service
            .graph()
            .snapshot()
            .threads
            .into_iter()
            .map(|thread| thread.thread_id)
            .collect();
        let auth = AuthContext {
            actor_id: "owner-1".into(),
            scopes: vec!["thread:read".into()],
        };

        let page = service.handle(&thread_request(Some(2), None), &auth, 10);
        assert!(page.ok, "{page:?}");
        assert_eq!(thread_ids(&page), ordered[..2].to_vec());

        let page = service.handle(&thread_request(Some(2), Some(&ordered[1])), &auth, 11);
        assert!(page.ok, "{page:?}");
        assert_eq!(thread_ids(&page), vec![ordered[2].clone()]);

        let page = service.handle(&thread_request(None, Some("missing")), &auth, 12);
        assert!(!page.ok);
        assert_eq!(page.error.unwrap().code, SurfaceErrorCode::InvalidRequest);
    }

    #[test]
    fn quota_is_per_actor_and_resets_per_window() {
        let dir = tempfile::tempdir().unwrap();
        let mut service = SurfaceService::open(dir.path().join("graph.jsonl"))
            .unwrap()
            .with_quota(SurfaceQuota::new(2, 100).unwrap());
        service.graph_mut().open_thread("thread", 1).unwrap();
        let owner = AuthContext {
            actor_id: "owner-1".into(),
            scopes: vec!["thread:read".into()],
        };
        let other = AuthContext {
            actor_id: "owner-2".into(),
            scopes: vec!["thread:read".into()],
        };

        assert!(service.handle(&thread_request(None, None), &owner, 10).ok);
        assert!(service.handle(&thread_request(None, None), &owner, 11).ok);
        let limited = service.handle(&thread_request(None, None), &owner, 12);
        assert!(!limited.ok);
        assert_eq!(limited.error.unwrap().code, SurfaceErrorCode::RateLimited);
        assert!(service.handle(&thread_request(None, None), &other, 12).ok);
        assert!(service.handle(&thread_request(None, None), &owner, 110).ok);
    }

    #[test]
    fn quota_requires_positive_limits() {
        assert!(SurfaceQuota::new(0, 100).is_none());
        assert!(SurfaceQuota::new(1, 0).is_none());
    }
}
