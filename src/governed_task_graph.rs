//! Task-graph service guarded by Authority/Policy checks.
//!
//! `TaskGraphService` remains the low-level append-only store. Product paths
//! should use this wrapper so approval, dispatch, report and side-effect calls
//! require a live authority profile before reaching the graph.

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::action_log::{ActionKind, ActionLog, ActionLogError};
use crate::action_log_journal::{ActionLogJournal, ActionLogJournalError};
use crate::authority::{
    AuthorityError, AuthorityProfile, AuthorityRegistry, PolicyAction, PolicyOutcome, PolicyRequest,
};
use crate::domain::RiskLevel;
use crate::gate::{GateError, GateInput, GatePolicy, evaluate as evaluate_gate};
use crate::identity::{IdentityDirectory, IdentityError};
use crate::standing_approval::{StandingApproval, StandingApprovalError, StandingApprovalRegistry};
use crate::task_graph::{
    ApprovalDecision, Decision, MandateStrength, ReportStatus, SideEffectDraft, TaskGraph,
    TaskGraphError, TaskGraphSnapshot, Trigger, TriggerIngest,
};
use crate::task_graph_store::{TaskGraphService, TaskGraphStoreError};
use crate::worker_runtime::{Dispatch, RouterRuntime, WorkerRuntimeError, WorkerTask};

#[derive(Debug, Error)]
pub enum GovernedTaskGraphError {
    #[error("governed task graph store: {0}")]
    Store(#[from] TaskGraphStoreError),
    #[error("governed task graph: {0}")]
    Graph(#[from] TaskGraphError),
    #[error("governed authority: {0}")]
    Authority(#[from] AuthorityError),
    #[error("governed policy denied: {0}")]
    PolicyDenied(String),
    #[error("governed approval required for {0}")]
    ApprovalRequired(String),
    #[error("governed gate: {0}")]
    Gate(#[from] GateError),
    #[error("governed standing approval: {0}")]
    Standing(#[from] StandingApprovalError),
    #[error("governed worker runtime: {0}")]
    Worker(#[from] WorkerRuntimeError),
    #[error("governed identity: {0}")]
    Identity(#[from] IdentityError),
    #[error("governed action log: {0}")]
    ActionLog(#[from] ActionLogError),
    #[error("governed action log journal: {0}")]
    ActionJournal(#[from] ActionLogJournalError),
}

struct PolicyCheck<'a> {
    actor_id: &'a str,
    action: PolicyAction,
    scope: &'a str,
    strength: MandateStrength,
    tool: Option<&'a str>,
    risk: RiskLevel,
    now_ms: u128,
}

#[derive(Debug)]
pub struct GovernedTaskGraph {
    graph: TaskGraphService,
    authority: AuthorityRegistry,
    standing: StandingApprovalRegistry,
    actions: ActionLog,
    action_journal: Option<ActionLogJournal>,
    identity: IdentityDirectory,
}

impl GovernedTaskGraph {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, GovernedTaskGraphError> {
        Ok(Self {
            graph: TaskGraphService::open(path)?,
            authority: AuthorityRegistry::default(),
            standing: StandingApprovalRegistry::default(),
            actions: ActionLog::default(),
            action_journal: None,
            identity: IdentityDirectory::default(),
        })
    }

    /// Open with a persistent security action log. The journal is replayed and
    /// verified on open; every recorded action is appended before the call
    /// returns.
    pub fn open_with_action_log(
        graph_path: impl Into<PathBuf>,
        action_log_path: impl Into<PathBuf>,
    ) -> Result<Self, GovernedTaskGraphError> {
        Ok(Self {
            graph: TaskGraphService::open(graph_path)?,
            authority: AuthorityRegistry::default(),
            standing: StandingApprovalRegistry::default(),
            actions: ActionLog::default(),
            action_journal: Some(ActionLogJournal::open(action_log_path)?),
            identity: IdentityDirectory::default(),
        })
    }

    pub fn with_authority(graph: TaskGraphService, authority: AuthorityRegistry) -> Self {
        Self {
            graph,
            authority,
            standing: StandingApprovalRegistry::default(),
            actions: ActionLog::default(),
            action_journal: None,
            identity: IdentityDirectory::default(),
        }
    }

    pub fn graph(&self) -> &TaskGraph {
        self.graph.graph()
    }

    pub fn snapshot(&self) -> TaskGraphSnapshot {
        self.graph.snapshot()
    }

    pub fn store_path(&self) -> &Path {
        self.graph.store_path()
    }

    pub fn authority(&self) -> &AuthorityRegistry {
        &self.authority
    }

    pub fn authority_mut(&mut self) -> &mut AuthorityRegistry {
        &mut self.authority
    }

    pub fn actions(&self) -> &ActionLog {
        self.action_journal
            .as_ref()
            .map(ActionLogJournal::log)
            .unwrap_or(&self.actions)
    }

    pub fn identity(&self) -> &IdentityDirectory {
        &self.identity
    }

    pub fn identity_mut(&mut self) -> &mut IdentityDirectory {
        &mut self.identity
    }

    pub fn standing(&self) -> &StandingApprovalRegistry {
        &self.standing
    }

    pub fn standing_mut(&mut self) -> &mut StandingApprovalRegistry {
        &mut self.standing
    }

    pub fn grant_standing(
        &mut self,
        approval: StandingApproval,
    ) -> Result<(), GovernedTaskGraphError> {
        self.standing.grant(approval)?;
        Ok(())
    }

    pub fn register_profile(
        &mut self,
        profile: AuthorityProfile,
    ) -> Result<(), GovernedTaskGraphError> {
        self.authority.register(profile)?;
        Ok(())
    }

    pub fn bind_actor(
        &mut self,
        actor_id: &str,
        profile_id: &str,
    ) -> Result<(), GovernedTaskGraphError> {
        self.authority.bind(actor_id, profile_id)?;
        Ok(())
    }

    pub fn open_thread(
        &mut self,
        topic: impl Into<String>,
        now_ms: u128,
    ) -> Result<String, GovernedTaskGraphError> {
        Ok(self
            .graph
            .with_graph_mut(|graph| graph.open_thread(topic, now_ms))?)
    }

    pub fn ingest_trigger(
        &mut self,
        thread_id: &str,
        trigger: Trigger,
        now_ms: u128,
    ) -> Result<TriggerIngest, GovernedTaskGraphError> {
        Ok(self
            .graph
            .with_graph_mut(|graph| graph.ingest_trigger(thread_id, trigger, now_ms))?)
    }

    pub fn ingest_trigger_gated(
        &mut self,
        thread_id: &str,
        trigger: Trigger,
        gate_input: &GateInput,
        gate_policy: &GatePolicy,
        now_ms: u128,
    ) -> Result<(TriggerIngest, crate::gate::GateDecision), GovernedTaskGraphError> {
        let decision = evaluate_gate(gate_input, gate_policy)?;
        let ingest = self.graph.with_graph_mut(|graph| {
            let ingest = graph.ingest_trigger(thread_id, trigger, now_ms)?;
            graph.evaluate_gate(
                thread_id,
                &ingest.trigger_id,
                decision.outcome,
                decision.reason.clone(),
                now_ms.saturating_add(1),
            )?;
            Ok(ingest)
        })?;
        Ok((ingest, decision))
    }

    pub fn begin_eval(
        &mut self,
        thread_id: &str,
        trigger_id: &str,
        now_ms: u128,
    ) -> Result<String, GovernedTaskGraphError> {
        Ok(self
            .graph
            .with_graph_mut(|graph| graph.begin_eval(thread_id, trigger_id, now_ms))?)
    }

    pub fn decide_eval(
        &mut self,
        eval_id: &str,
        decision: Decision,
        reason: &str,
        now_ms: u128,
    ) -> Result<(), GovernedTaskGraphError> {
        self.graph
            .with_graph_mut(|graph| graph.decide_eval(eval_id, decision, reason, now_ms))?;
        Ok(())
    }

    pub fn create_ticket_from_eval(
        &mut self,
        eval_id: &str,
        actor_id: &str,
        draft: crate::task_graph::TicketDraft,
        now_ms: u128,
    ) -> Result<String, GovernedTaskGraphError> {
        if draft.owner_id != actor_id {
            return Err(GovernedTaskGraphError::PolicyDenied(
                "ticket owner must match the acting authority".into(),
            ));
        }
        self.ensure_policy(PolicyCheck {
            actor_id,
            action: PolicyAction::Dispatch,
            scope: &draft.scope,
            strength: draft.strength,
            tool: None,
            risk: RiskLevel::Medium,
            now_ms,
        })?;
        Ok(self
            .graph
            .with_graph_mut(|graph| graph.create_ticket_from_eval(eval_id, draft, now_ms))?)
    }

    pub fn approve_ticket(
        &mut self,
        ticket_id: &str,
        actor_id: &str,
        decision: ApprovalDecision,
        expires_ms: Option<u128>,
        now_ms: u128,
    ) -> Result<String, GovernedTaskGraphError> {
        self.ensure_ticket_policy(ticket_id, actor_id, PolicyAction::Approve, now_ms)?;
        let approval_id = self.graph.with_graph_mut(|graph| {
            graph.approve_ticket(ticket_id, actor_id, decision, expires_ms, now_ms)
        })?;
        self.record_action(
            actor_id,
            ActionKind::Approve,
            ticket_id,
            "allowed",
            "approval recorded",
            now_ms,
        )?;
        Ok(approval_id)
    }

    pub fn approve_ticket_as_subject(
        &mut self,
        ticket_id: &str,
        subject: &str,
        decision: ApprovalDecision,
        expires_ms: Option<u128>,
        now_ms: u128,
    ) -> Result<String, GovernedTaskGraphError> {
        let actor_id = self.identity.resolve(subject, now_ms)?.actor_id.clone();
        self.approve_ticket(ticket_id, &actor_id, decision, expires_ms, now_ms)
    }

    pub fn assign_ticket(
        &mut self,
        ticket_id: &str,
        actor_id: &str,
        worker_id: &str,
        session_id: Option<String>,
        now_ms: u128,
    ) -> Result<String, GovernedTaskGraphError> {
        let outcome =
            self.evaluate_ticket_policy(ticket_id, actor_id, PolicyAction::Dispatch, now_ms)?;
        if outcome == PolicyOutcome::RequireApproval {
            self.auto_approve_with_standing(ticket_id, actor_id, now_ms)?;
            self.ensure_live_approval(ticket_id, now_ms)?;
        }
        Ok(self.graph.with_graph_mut(|graph| {
            graph.assign_ticket(ticket_id, worker_id, session_id, now_ms)
        })?)
    }

    pub fn dispatch_ticket(
        &mut self,
        ticket_id: &str,
        actor_id: &str,
        runtime: &mut RouterRuntime,
        session_id: Option<String>,
        now_ms: u128,
    ) -> Result<Dispatch, GovernedTaskGraphError> {
        let ticket = self
            .graph
            .graph()
            .ticket(ticket_id)
            .cloned()
            .ok_or_else(|| TaskGraphError::UnknownTicket(ticket_id.into()))?;
        // Check the actor before occupying a worker slot. A denied dispatch
        // must not change scheduler state.
        let outcome =
            self.evaluate_ticket_policy(ticket_id, actor_id, PolicyAction::Dispatch, now_ms)?;
        if outcome == PolicyOutcome::RequireApproval {
            self.auto_approve_with_standing(ticket_id, actor_id, now_ms)?;
            self.ensure_live_approval(ticket_id, now_ms)?;
        }
        let dispatch = runtime.dispatch(WorkerTask {
            task_id: ticket_id.into(),
            thread_id: ticket.thread_id,
            summary: format!("ticket {ticket_id}"),
        })?;
        if !dispatch.queued
            && let Err(error) = self.graph.with_graph_mut(|graph| {
                graph.assign_ticket(ticket_id, &dispatch.worker_id, session_id, now_ms)
            })
        {
            let _ = runtime.release(&dispatch.worker_id);
            return Err(error.into());
        }
        Ok(dispatch)
    }

    pub fn start_ticket(
        &mut self,
        ticket_id: &str,
        worker_id: &str,
        now_ms: u128,
    ) -> Result<(), GovernedTaskGraphError> {
        let action = self.action_for_ticket(ticket_id, PolicyAction::Read)?;
        self.ensure_ticket_policy(ticket_id, worker_id, action, now_ms)?;
        self.graph
            .with_graph_mut(|graph| graph.start_ticket(ticket_id, worker_id, now_ms))?;
        Ok(())
    }

    pub fn report_ticket(
        &mut self,
        ticket_id: &str,
        worker_id: &str,
        status: ReportStatus,
        summary: impl Into<String>,
        artifact_sha256: Option<String>,
        now_ms: u128,
    ) -> Result<String, GovernedTaskGraphError> {
        let action = self.action_for_ticket(ticket_id, PolicyAction::Read)?;
        self.ensure_ticket_policy(ticket_id, worker_id, action, now_ms)?;
        Ok(self.graph.with_graph_mut(|graph| {
            graph.report_ticket(
                ticket_id,
                worker_id,
                status,
                summary,
                artifact_sha256,
                now_ms,
            )
        })?)
    }

    pub fn record_side_effect(
        &mut self,
        ticket_id: &str,
        actor_id: &str,
        draft: SideEffectDraft,
        now_ms: u128,
    ) -> Result<String, GovernedTaskGraphError> {
        let action = side_effect_action(&draft.action);
        self.ensure_ticket_policy(ticket_id, actor_id, action, now_ms)?;
        let effect_id = self
            .graph
            .with_graph_mut(|graph| graph.record_side_effect(ticket_id, draft, now_ms))?;
        self.record_action(
            actor_id,
            ActionKind::SideEffect,
            ticket_id,
            "recorded",
            "side effect recorded",
            now_ms,
        )?;
        Ok(effect_id)
    }

    pub fn set_side_effect_status(
        &mut self,
        ticket_id: &str,
        actor_id: &str,
        side_effect_id: &str,
        status: crate::task_graph::SideEffectStatus,
        evidence: impl Into<String>,
        now_ms: u128,
    ) -> Result<(), GovernedTaskGraphError> {
        self.ensure_ticket_policy(ticket_id, actor_id, PolicyAction::Write, now_ms)?;
        let evidence = evidence.into();
        self.graph.with_graph_mut(|graph| {
            graph.set_side_effect_status(
                ticket_id,
                side_effect_id,
                status,
                evidence.clone(),
                now_ms,
            )
        })?;
        self.record_action(
            actor_id,
            ActionKind::SideEffect,
            side_effect_id,
            "status_changed",
            evidence,
            now_ms,
        )?;
        Ok(())
    }

    pub fn compensate_side_effect(
        &mut self,
        ticket_id: &str,
        actor_id: &str,
        side_effect_id: &str,
        evidence: impl Into<String>,
        now_ms: u128,
    ) -> Result<(), GovernedTaskGraphError> {
        self.set_side_effect_status(
            ticket_id,
            actor_id,
            side_effect_id,
            crate::task_graph::SideEffectStatus::Compensated,
            evidence,
            now_ms,
        )
    }

    pub fn retry_ticket(
        &mut self,
        ticket_id: &str,
        actor_id: &str,
        reason: impl Into<String>,
        now_ms: u128,
    ) -> Result<(), GovernedTaskGraphError> {
        self.ensure_ticket_policy(ticket_id, actor_id, PolicyAction::Approve, now_ms)?;
        let reason = reason.into();
        self.graph.with_graph_mut(|graph| {
            graph.retry_ticket(ticket_id, actor_id, reason.clone(), now_ms)
        })?;
        self.record_action(
            actor_id,
            ActionKind::Retry,
            ticket_id,
            "allowed",
            reason,
            now_ms,
        )?;
        Ok(())
    }

    pub fn takeover_ticket(
        &mut self,
        ticket_id: &str,
        actor_id: &str,
        reason: impl Into<String>,
        now_ms: u128,
    ) -> Result<(), GovernedTaskGraphError> {
        self.ensure_ticket_policy(ticket_id, actor_id, PolicyAction::Approve, now_ms)?;
        let reason = reason.into();
        self.graph.with_graph_mut(|graph| {
            graph.takeover_ticket(ticket_id, actor_id, reason.clone(), now_ms)
        })?;
        self.record_action(
            actor_id,
            ActionKind::Takeover,
            ticket_id,
            "allowed",
            reason,
            now_ms,
        )?;
        Ok(())
    }

    pub fn release_takeover(
        &mut self,
        ticket_id: &str,
        actor_id: &str,
        reason: impl Into<String>,
        now_ms: u128,
    ) -> Result<(), GovernedTaskGraphError> {
        self.ensure_ticket_policy(ticket_id, actor_id, PolicyAction::Approve, now_ms)?;
        let reason = reason.into();
        self.graph.with_graph_mut(|graph| {
            graph.release_takeover(ticket_id, actor_id, reason.clone(), now_ms)
        })?;
        self.record_action(
            actor_id,
            ActionKind::ReleaseTakeover,
            ticket_id,
            "allowed",
            reason,
            now_ms,
        )?;
        Ok(())
    }

    pub fn cancel_ticket(
        &mut self,
        ticket_id: &str,
        actor_id: &str,
        reason: impl Into<String>,
        now_ms: u128,
    ) -> Result<(), GovernedTaskGraphError> {
        self.ensure_ticket_policy(ticket_id, actor_id, PolicyAction::Approve, now_ms)?;
        let reason = reason.into();
        self.graph.with_graph_mut(|graph| {
            graph.cancel_ticket(ticket_id, actor_id, reason.clone(), now_ms)
        })?;
        self.record_action(
            actor_id,
            ActionKind::Cancel,
            ticket_id,
            "allowed",
            reason,
            now_ms,
        )?;
        Ok(())
    }

    pub fn revoke_mandate(
        &mut self,
        mandate_id: &str,
        actor_id: &str,
        reason: impl Into<String>,
        now_ms: u128,
    ) -> Result<(), GovernedTaskGraphError> {
        let mandate = self
            .graph
            .graph()
            .mandate(mandate_id)
            .cloned()
            .ok_or_else(|| TaskGraphError::UnknownMandate(mandate_id.into()))?;
        self.ensure_policy(PolicyCheck {
            actor_id,
            action: PolicyAction::Approve,
            scope: &mandate.scope,
            strength: mandate.strength,
            tool: None,
            risk: RiskLevel::Medium,
            now_ms,
        })?;
        let reason = reason.into();
        self.graph.with_graph_mut(|graph| {
            graph.revoke_mandate(mandate_id, actor_id, reason.clone(), now_ms)
        })?;
        self.record_action(
            actor_id,
            ActionKind::RevokeMandate,
            mandate_id,
            "allowed",
            reason,
            now_ms,
        )?;
        Ok(())
    }

    fn action_for_ticket(
        &self,
        ticket_id: &str,
        read_action: PolicyAction,
    ) -> Result<PolicyAction, GovernedTaskGraphError> {
        let ticket = self
            .graph
            .graph()
            .ticket(ticket_id)
            .ok_or_else(|| TaskGraphError::UnknownTicket(ticket_id.into()))?;
        let mandate = self
            .graph
            .graph()
            .mandate(&ticket.mandate_id)
            .ok_or_else(|| TaskGraphError::UnknownMandate(ticket.mandate_id.clone()))?;
        Ok(match mandate.strength {
            MandateStrength::ReadOnly => read_action,
            MandateStrength::Write => PolicyAction::Write,
            MandateStrength::Execute => PolicyAction::Execute,
            MandateStrength::Network => PolicyAction::Network,
        })
    }

    fn ensure_ticket_policy(
        &self,
        ticket_id: &str,
        actor_id: &str,
        action: PolicyAction,
        now_ms: u128,
    ) -> Result<PolicyOutcome, GovernedTaskGraphError> {
        let outcome = self.evaluate_ticket_policy(ticket_id, actor_id, action, now_ms)?;
        if outcome == PolicyOutcome::RequireApproval && action != PolicyAction::Approve {
            self.ensure_live_approval(ticket_id, now_ms)?;
        }
        Ok(outcome)
    }

    fn evaluate_ticket_policy(
        &self,
        ticket_id: &str,
        actor_id: &str,
        action: PolicyAction,
        now_ms: u128,
    ) -> Result<PolicyOutcome, GovernedTaskGraphError> {
        let ticket = self
            .graph
            .graph()
            .ticket(ticket_id)
            .ok_or_else(|| TaskGraphError::UnknownTicket(ticket_id.into()))?;
        let mandate = self
            .graph
            .graph()
            .mandate(&ticket.mandate_id)
            .ok_or_else(|| TaskGraphError::UnknownMandate(ticket.mandate_id.clone()))?;
        self.ensure_policy(PolicyCheck {
            actor_id,
            action,
            scope: &mandate.scope,
            strength: mandate.strength,
            tool: None,
            risk: risk_for(action, mandate.strength),
            now_ms,
        })
    }

    fn ensure_live_approval(
        &self,
        ticket_id: &str,
        now_ms: u128,
    ) -> Result<(), GovernedTaskGraphError> {
        let ticket = self
            .graph
            .graph()
            .ticket(ticket_id)
            .ok_or_else(|| TaskGraphError::UnknownTicket(ticket_id.into()))?;
        let Some(approval_id) = &ticket.approval_id else {
            return Err(GovernedTaskGraphError::ApprovalRequired(ticket_id.into()));
        };
        let approval = self
            .graph
            .graph()
            .approval(approval_id)
            .ok_or_else(|| TaskGraphError::UnknownApproval(approval_id.clone()))?;
        if approval.expires_ms.is_some_and(|expires| now_ms >= expires) {
            return Err(GovernedTaskGraphError::ApprovalRequired(ticket_id.into()));
        }
        Ok(())
    }

    fn record_action(
        &mut self,
        actor: &str,
        kind: ActionKind,
        target: &str,
        outcome: &str,
        reason: impl Into<String>,
        now_ms: u128,
    ) -> Result<(), GovernedTaskGraphError> {
        if let Some(journal) = &mut self.action_journal {
            let reason = reason.into();
            for attempt in 0..2 {
                if attempt == 1 {
                    journal.reload()?;
                }
                journal
                    .log_mut()
                    .record(actor, kind, target, outcome, reason.clone(), now_ms)?;
                match journal.persist() {
                    Ok(_) => break,
                    Err(ActionLogJournalError::ConcurrentWrite { .. }) if attempt == 0 => {}
                    Err(error) => return Err(error.into()),
                }
            }
        } else {
            self.actions
                .record(actor, kind, target, outcome, reason, now_ms)?;
        }
        Ok(())
    }

    fn ensure_policy(
        &self,
        check: PolicyCheck<'_>,
    ) -> Result<PolicyOutcome, GovernedTaskGraphError> {
        let request = PolicyRequest {
            actor_id: check.actor_id.into(),
            action: check.action,
            scope: check.scope.into(),
            strength: check.strength,
            tool: check.tool.map(str::to_string),
            risk: check.risk,
        };
        match self.authority.evaluate(&request, check.now_ms).outcome {
            PolicyOutcome::Deny => Err(GovernedTaskGraphError::PolicyDenied(format!(
                "{} denied for {} on {}",
                check.action.as_str(),
                check.actor_id,
                check.scope
            ))),
            outcome @ (PolicyOutcome::Allow | PolicyOutcome::RequireApproval) => Ok(outcome),
        }
    }

    fn auto_approve_with_standing(
        &mut self,
        ticket_id: &str,
        actor_id: &str,
        now_ms: u128,
    ) -> Result<(), GovernedTaskGraphError> {
        let ticket = self
            .graph
            .graph()
            .ticket(ticket_id)
            .ok_or_else(|| TaskGraphError::UnknownTicket(ticket_id.into()))?
            .clone();
        // Only the owner may turn a standing grant into an approval. A worker
        // with a dispatch grant must not approve on the owner's behalf.
        if actor_id != ticket.owner_id
            || ticket.status != crate::task_graph::TicketStatus::Authorized
        {
            return Ok(());
        }
        let mandate = self
            .graph
            .graph()
            .mandate(&ticket.mandate_id)
            .ok_or_else(|| TaskGraphError::UnknownMandate(ticket.mandate_id.clone()))?;
        let request = PolicyRequest {
            actor_id: actor_id.into(),
            action: PolicyAction::Dispatch,
            scope: mandate.scope.clone(),
            strength: mandate.strength,
            tool: None,
            risk: RiskLevel::Medium,
        };
        if self.standing.authorize(&request, now_ms).is_none() {
            return Ok(());
        }
        self.graph.with_graph_mut(|graph| {
            graph.approve_ticket(
                ticket_id,
                ticket.owner_id.clone(),
                ApprovalDecision::AllowOnce,
                None,
                now_ms,
            )
        })?;
        self.record_action(
            actor_id,
            ActionKind::Approve,
            ticket_id,
            "standing_auto_approved",
            "standing approval matched and was consumed",
            now_ms,
        )?;
        Ok(())
    }
}

fn risk_for(action: PolicyAction, strength: MandateStrength) -> RiskLevel {
    if strength == MandateStrength::ReadOnly
        && matches!(action, PolicyAction::Read | PolicyAction::Approve)
    {
        return RiskLevel::Low;
    }
    if matches!(action, PolicyAction::Execute | PolicyAction::Network) {
        return RiskLevel::High;
    }
    RiskLevel::Medium
}

fn side_effect_action(action: &str) -> PolicyAction {
    match action {
        "read" => PolicyAction::Read,
        "write" => PolicyAction::Write,
        "network" => PolicyAction::Network,
        _ => PolicyAction::Execute,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::{GateInput, GateOutcome, GatePolicy, SourceTrust};
    use crate::identity::IdentityBinding;
    use crate::standing_approval::StandingApproval;
    use crate::task_graph::{TicketDraft, TriggerSource};
    use crate::worker_runtime::RouterRuntime;

    fn trigger() -> Trigger {
        Trigger {
            trigger_id: "trigger-1".into(),
            source: TriggerSource::Web,
            source_event_id: "event-1".into(),
            summary: "governed".into(),
            received_ms: 1,
        }
    }

    fn governed(dir: &Path) -> GovernedTaskGraph {
        let mut service = GovernedTaskGraph::open(dir.join("task-graph.jsonl")).unwrap();
        service
            .register_profile(AuthorityProfile {
                profile_id: "owner-profile".into(),
                actor_id: "owner-1".into(),
                scopes: vec!["read project alpha*".into()],
                max_strength: MandateStrength::ReadOnly,
                allowed_tools: Vec::new(),
                denied_tools: Vec::new(),
                max_risk: RiskLevel::Medium,
                expires_ms: None,
            })
            .unwrap();
        service.bind_actor("owner-1", "owner-profile").unwrap();
        service
            .register_profile(AuthorityProfile {
                profile_id: "worker-profile".into(),
                actor_id: "worker-1".into(),
                scopes: vec!["read project alpha*".into()],
                max_strength: MandateStrength::ReadOnly,
                allowed_tools: Vec::new(),
                denied_tools: Vec::new(),
                max_risk: RiskLevel::Medium,
                expires_ms: None,
            })
            .unwrap();
        service.bind_actor("worker-1", "worker-profile").unwrap();
        service
    }

    fn governed_write(dir: &Path) -> GovernedTaskGraph {
        let mut service = GovernedTaskGraph::open(dir.join("task-graph-write.jsonl")).unwrap();
        for actor in ["owner-1", "worker-1"] {
            service
                .register_profile(AuthorityProfile {
                    profile_id: format!("{actor}-write"),
                    actor_id: actor.into(),
                    scopes: vec!["write project*".into()],
                    max_strength: MandateStrength::Write,
                    allowed_tools: Vec::new(),
                    denied_tools: Vec::new(),
                    max_risk: RiskLevel::Medium,
                    expires_ms: None,
                })
                .unwrap();
            service
                .bind_actor(actor, &format!("{actor}-write"))
                .unwrap();
        }
        service
    }

    fn write_ticket(service: &mut GovernedTaskGraph, requires_approval: bool) -> String {
        let thread_id = service.open_thread("write", 10).unwrap();
        let trigger_id = service
            .ingest_trigger(&thread_id, trigger(), 11)
            .unwrap()
            .trigger_id;
        let eval_id = service.begin_eval(&thread_id, &trigger_id, 12).unwrap();
        service
            .decide_eval(&eval_id, Decision::Accept, "accepted", 13)
            .unwrap();
        service
            .create_ticket_from_eval(
                &eval_id,
                "owner-1",
                TicketDraft {
                    owner_id: "owner-1".into(),
                    scope: "write project".into(),
                    strength: MandateStrength::Write,
                    policy_version: "v1".into(),
                    requires_approval,
                },
                14,
            )
            .unwrap()
    }

    fn ticket(service: &mut GovernedTaskGraph) -> String {
        let thread_id = service.open_thread("governed", 10).unwrap();
        let trigger_id = service
            .ingest_trigger(&thread_id, trigger(), 11)
            .unwrap()
            .trigger_id;
        let eval_id = service.begin_eval(&thread_id, &trigger_id, 12).unwrap();
        service
            .decide_eval(&eval_id, Decision::Accept, "accepted", 13)
            .unwrap();
        service
            .create_ticket_from_eval(
                &eval_id,
                "owner-1",
                TicketDraft {
                    owner_id: "owner-1".into(),
                    scope: "read project alpha".into(),
                    strength: MandateStrength::ReadOnly,
                    policy_version: "v1".into(),
                    requires_approval: true,
                },
                14,
            )
            .unwrap()
    }

    #[test]
    fn governed_flow_requires_live_authority() {
        let dir = tempfile::tempdir().unwrap();
        let mut service = governed(dir.path());
        let ticket_id = ticket(&mut service);
        assert!(matches!(
            service
                .approve_ticket(
                    &ticket_id,
                    "attacker",
                    ApprovalDecision::AllowOnce,
                    None,
                    15
                )
                .unwrap_err(),
            GovernedTaskGraphError::PolicyDenied(_)
        ));
        service
            .approve_ticket(&ticket_id, "owner-1", ApprovalDecision::AllowOnce, None, 16)
            .unwrap();
        assert_eq!(service.actions().len(), 1);
        service.actions().verify().unwrap();
        assert_eq!(
            service.actions().entries()[0].kind,
            crate::action_log::ActionKind::Approve
        );
        service
            .assign_ticket(&ticket_id, "owner-1", "worker-1", None, 17)
            .unwrap();
        service.start_ticket(&ticket_id, "worker-1", 18).unwrap();
        service
            .report_ticket(
                &ticket_id,
                "worker-1",
                ReportStatus::Ready,
                "ready",
                Some("sha256:1".into()),
                19,
            )
            .unwrap();
    }

    #[test]
    fn action_log_journal_persists_across_reopen_and_detects_tamper() {
        let dir = tempfile::tempdir().unwrap();
        let graph_path = dir.path().join("task-graph.jsonl");
        let action_path = dir.path().join("action-log.jsonl");
        let mut service =
            GovernedTaskGraph::open_with_action_log(&graph_path, &action_path).unwrap();
        service
            .register_profile(AuthorityProfile {
                profile_id: "owner-profile".into(),
                actor_id: "owner-1".into(),
                scopes: vec!["read project alpha*".into()],
                max_strength: MandateStrength::ReadOnly,
                allowed_tools: Vec::new(),
                denied_tools: Vec::new(),
                max_risk: RiskLevel::Medium,
                expires_ms: None,
            })
            .unwrap();
        service.bind_actor("owner-1", "owner-profile").unwrap();
        let ticket_id = ticket(&mut service);
        service
            .approve_ticket(&ticket_id, "owner-1", ApprovalDecision::AllowOnce, None, 15)
            .unwrap();
        drop(service);

        let reopened = GovernedTaskGraph::open_with_action_log(&graph_path, &action_path).unwrap();
        assert_eq!(reopened.actions().len(), 1);
        reopened.actions().verify().unwrap();
        drop(reopened);

        let text = std::fs::read_to_string(&action_path)
            .unwrap()
            .replace("\"owner-1\"", "\"attacker\"");
        std::fs::write(&action_path, text).unwrap();
        assert!(GovernedTaskGraph::open_with_action_log(&graph_path, &action_path).is_err());
    }

    #[test]
    fn governed_denies_scope_and_side_effect_escalation() {
        let dir = tempfile::tempdir().unwrap();
        let mut service = governed(dir.path());
        let ticket_id = ticket(&mut service);
        service
            .approve_ticket(&ticket_id, "owner-1", ApprovalDecision::AllowOnce, None, 15)
            .unwrap();
        service
            .assign_ticket(&ticket_id, "owner-1", "worker-1", None, 16)
            .unwrap();
        service.start_ticket(&ticket_id, "worker-1", 17).unwrap();
        let draft = SideEffectDraft {
            action: "write".into(),
            target: "/workspace/out".into(),
            idempotency_key: "effect-1".into(),
            reversible: true,
            compensation: None,
            status: crate::task_graph::SideEffectStatus::Applied,
        };
        assert!(matches!(
            service
                .record_side_effect(&ticket_id, "worker-1", draft.clone(), 18)
                .unwrap_err(),
            GovernedTaskGraphError::PolicyDenied(_)
        ));
        let mut read = draft;
        read.action = "read".into();
        service
            .record_side_effect(&ticket_id, "worker-1", read, 19)
            .unwrap();
    }

    #[test]
    fn expired_profile_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let mut service = GovernedTaskGraph::open(dir.path().join("task-graph.jsonl")).unwrap();
        service
            .register_profile(AuthorityProfile {
                profile_id: "owner-profile".into(),
                actor_id: "owner-1".into(),
                scopes: vec!["*".into()],
                max_strength: MandateStrength::ReadOnly,
                allowed_tools: Vec::new(),
                denied_tools: Vec::new(),
                max_risk: RiskLevel::Medium,
                expires_ms: Some(5),
            })
            .unwrap();
        service.bind_actor("owner-1", "owner-profile").unwrap();
        let thread_id = service.open_thread("expired", 1).unwrap();
        let trigger_id = service
            .ingest_trigger(&thread_id, trigger(), 2)
            .unwrap()
            .trigger_id;
        let eval_id = service.begin_eval(&thread_id, &trigger_id, 3).unwrap();
        service
            .decide_eval(&eval_id, Decision::Accept, "ok", 4)
            .unwrap();
        assert!(matches!(
            service
                .create_ticket_from_eval(
                    &eval_id,
                    "owner-1",
                    TicketDraft {
                        owner_id: "owner-1".into(),
                        scope: "read".into(),
                        strength: MandateStrength::ReadOnly,
                        policy_version: "v1".into(),
                        requires_approval: false,
                    },
                    10,
                )
                .unwrap_err(),
            GovernedTaskGraphError::PolicyDenied(_)
        ));
    }

    #[test]
    fn gate_decisions_are_persisted_and_ignored_triggers_cannot_eval() {
        let dir = tempfile::tempdir().unwrap();
        let mut service = governed(dir.path());
        let policy = GatePolicy {
            observe_at_bps: 4_000,
            auto_at_bps: 8_000,
            max_auto_risk: RiskLevel::Medium,
            allow_untrusted_auto: false,
        };
        let thread_id = service.open_thread("gated", 10).unwrap();
        let ignored = GateInput {
            source_event_id: "event-ignored".into(),
            trust: SourceTrust::Trusted,
            risk: RiskLevel::Low,
            confidence_bps: 9_000,
            explicit_mention: true,
            duplicate: true,
        };
        let (ingest, decision) = service
            .ingest_trigger_gated(&thread_id, trigger(), &ignored, &policy, 11)
            .unwrap();
        assert_eq!(decision.outcome, GateOutcome::Ignore);
        let result = service.begin_eval(&thread_id, &ingest.trigger_id, 12);
        assert!(
            matches!(
                result,
                Err(GovernedTaskGraphError::Store(TaskGraphStoreError::Replay(
                    TaskGraphError::GateIgnored(_)
                )))
            ),
            "{result:?}"
        );

        let thread_id = service.open_thread("gated-auto", 13).unwrap();
        let auto = GateInput {
            duplicate: false,
            ..ignored
        };
        let (ingest, decision) = service
            .ingest_trigger_gated(&thread_id, trigger(), &auto, &policy, 14)
            .unwrap();
        assert_eq!(decision.outcome, GateOutcome::Auto);
        service
            .begin_eval(&thread_id, &ingest.trigger_id, 15)
            .unwrap();
    }

    #[test]
    fn standing_approval_can_auto_approve_one_assignment() {
        let dir = tempfile::tempdir().unwrap();
        let mut service = governed(dir.path());
        let ticket_id = ticket(&mut service);
        service
            .grant_standing(StandingApproval {
                approval_id: "standing-1".into(),
                actor_id: "owner-1".into(),
                scopes: vec!["read project alpha*".into()],
                max_strength: MandateStrength::ReadOnly,
                max_risk: RiskLevel::Medium,
                allowed_tools: Vec::new(),
                denied_tools: Vec::new(),
                expires_ms: 100,
                max_uses: 1,
                used: 0,
                revoked: false,
            })
            .unwrap();
        service
            .assign_ticket(&ticket_id, "owner-1", "worker-1", None, 16)
            .unwrap();
        let ticket = service.graph().ticket(&ticket_id).unwrap();
        assert_eq!(ticket.status, crate::task_graph::TicketStatus::Assigned);
        assert!(ticket.approval_id.is_some());
        assert_eq!(service.standing().get("standing-1").unwrap().used, 1);
        assert!(
            service
                .actions()
                .query(None, Some(ActionKind::Approve), Some(&ticket_id), 10)
                .iter()
                .any(|entry| entry.outcome == "standing_auto_approved")
        );
    }

    #[test]
    fn worker_standing_grant_cannot_approve_for_the_owner() {
        let dir = tempfile::tempdir().unwrap();
        let mut service = governed(dir.path());
        let ticket_id = ticket(&mut service);
        service
            .grant_standing(StandingApproval {
                approval_id: "standing-worker".into(),
                actor_id: "worker-1".into(),
                scopes: vec!["read project alpha*".into()],
                max_strength: MandateStrength::ReadOnly,
                max_risk: RiskLevel::Medium,
                allowed_tools: Vec::new(),
                denied_tools: Vec::new(),
                expires_ms: 100,
                max_uses: 1,
                used: 0,
                revoked: false,
            })
            .unwrap();
        assert!(
            service
                .assign_ticket(&ticket_id, "worker-1", "worker-1", None, 16)
                .is_err()
        );
        let ticket = service.graph().ticket(&ticket_id).unwrap();
        assert!(ticket.approval_id.is_none());
        assert_eq!(ticket.status, crate::task_graph::TicketStatus::Authorized);
        assert_eq!(service.standing().get("standing-worker").unwrap().used, 0);
    }

    #[test]
    fn write_strength_ticket_can_start_and_report_after_approval() {
        let dir = tempfile::tempdir().unwrap();
        let mut service = governed_write(dir.path());
        let ticket_id = write_ticket(&mut service, true);
        service
            .approve_ticket(&ticket_id, "owner-1", ApprovalDecision::AllowOnce, None, 15)
            .unwrap();
        service
            .assign_ticket(&ticket_id, "owner-1", "worker-1", None, 16)
            .unwrap();
        service.start_ticket(&ticket_id, "worker-1", 17).unwrap();
        service
            .report_ticket(
                &ticket_id,
                "worker-1",
                ReportStatus::Progress,
                "working",
                None,
                18,
            )
            .unwrap();
        assert_eq!(
            service.graph().ticket(&ticket_id).unwrap().status,
            crate::task_graph::TicketStatus::Running
        );
    }

    #[test]
    fn expired_approval_is_rejected_before_scheduling() {
        let dir = tempfile::tempdir().unwrap();
        let mut service = governed(dir.path());
        let ticket_id = ticket(&mut service);
        service
            .approve_ticket(
                &ticket_id,
                "owner-1",
                ApprovalDecision::AllowOnce,
                Some(20),
                15,
            )
            .unwrap();
        let mut runtime = RouterRuntime::new(4);
        runtime.register_worker("worker-1", 1).unwrap();
        assert!(matches!(
            service
                .dispatch_ticket(&ticket_id, "owner-1", &mut runtime, None, 20)
                .unwrap_err(),
            GovernedTaskGraphError::ApprovalRequired(_)
        ));
        assert_eq!(runtime.active("worker-1"), Some(0));
        assert_eq!(runtime.queued(), 0);
    }

    #[test]
    fn denied_dispatch_does_not_occupy_a_worker_slot() {
        let dir = tempfile::tempdir().unwrap();
        let mut service = governed(dir.path());
        let ticket_id = ticket(&mut service);
        service
            .approve_ticket(&ticket_id, "owner-1", ApprovalDecision::AllowOnce, None, 15)
            .unwrap();
        let mut runtime = RouterRuntime::new(4);
        runtime.register_worker("worker-1", 1).unwrap();
        assert!(matches!(
            service
                .dispatch_ticket(&ticket_id, "attacker", &mut runtime, None, 16)
                .unwrap_err(),
            GovernedTaskGraphError::Authority(_) | GovernedTaskGraphError::PolicyDenied(_)
        ));
        assert_eq!(runtime.active("worker-1"), Some(0));
        assert_eq!(runtime.queued(), 0);
    }

    #[test]
    fn policy_required_approval_blocks_a_ticket_without_one() {
        let dir = tempfile::tempdir().unwrap();
        let mut service = governed_write(dir.path());
        let ticket_id = write_ticket(&mut service, false);
        assert!(matches!(
            service
                .assign_ticket(&ticket_id, "owner-1", "worker-1", None, 15)
                .unwrap_err(),
            GovernedTaskGraphError::ApprovalRequired(_)
        ));
        assert_eq!(
            service.graph().ticket(&ticket_id).unwrap().status,
            crate::task_graph::TicketStatus::Authorized
        );
    }

    #[test]
    fn external_identity_resolves_to_the_bound_actor() {
        let dir = tempfile::tempdir().unwrap();
        let mut service = governed(dir.path());
        let ticket_id = ticket(&mut service);
        service
            .identity_mut()
            .bind(IdentityBinding {
                subject: "oidc:owner".into(),
                actor_id: "owner-1".into(),
                issuer: "test-idp".into(),
                issued_ms: 1,
                expires_ms: 100,
                revoked_ms: None,
            })
            .unwrap();
        service
            .approve_ticket_as_subject(
                &ticket_id,
                "oidc:owner",
                ApprovalDecision::AllowOnce,
                None,
                20,
            )
            .unwrap();
        assert!(matches!(
            service
                .approve_ticket_as_subject(
                    &ticket_id,
                    "oidc:missing",
                    ApprovalDecision::AllowOnce,
                    None,
                    21,
                )
                .unwrap_err(),
            GovernedTaskGraphError::Identity(_)
        ));
    }

    #[test]
    fn dispatch_assigns_a_worker_through_the_scheduler() {
        let dir = tempfile::tempdir().unwrap();
        let mut service = governed(dir.path());
        let ticket_id = ticket(&mut service);
        service
            .approve_ticket(&ticket_id, "owner-1", ApprovalDecision::AllowOnce, None, 15)
            .unwrap();
        let mut runtime = RouterRuntime::new(1);
        runtime.register_worker("worker-1", 1).unwrap();
        let dispatch = service
            .dispatch_ticket(&ticket_id, "owner-1", &mut runtime, None, 16)
            .unwrap();
        assert_eq!(dispatch.worker_id, "worker-1");
        assert!(!dispatch.queued);
        assert_eq!(
            service.graph().ticket(&ticket_id).unwrap().status,
            crate::task_graph::TicketStatus::Assigned
        );
    }
}
