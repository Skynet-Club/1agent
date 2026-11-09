//! Task graph primitives for the 1Agent event -> thread -> eval -> ticket path.
//!
//! This module is deliberately side-effect free. It owns the state machine and
//! an append-only, hash-chained event log; persistence and execution live in
//! later layers.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::gate::GateOutcome;

pub const TASK_GRAPH_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum TriggerSource {
    Web,
    Im,
    Email,
    Github,
    Ci,
    Calendar,
    Webhook,
    Other,
}

impl TriggerSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Im => "im",
            Self::Email => "email",
            Self::Github => "github",
            Self::Ci => "ci",
            Self::Calendar => "calendar",
            Self::Webhook => "webhook",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Trigger {
    pub trigger_id: String,
    pub source: TriggerSource,
    pub source_event_id: String,
    pub summary: String,
    pub received_ms: u128,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThreadStatus {
    Open,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Thread {
    pub thread_id: String,
    pub topic: String,
    pub status: ThreadStatus,
    pub trigger_ids: Vec<String>,
    pub subscription_ids: Vec<String>,
    pub ticket_ids: Vec<String>,
    pub created_ms: u128,
    pub updated_ms: u128,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    Active,
    Removed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Subscription {
    pub subscription_id: String,
    pub thread_id: String,
    pub ticket_id: String,
    pub status: SubscriptionStatus,
    pub created_ms: u128,
    pub removed_ms: Option<u128>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvalStatus {
    Pending,
    Decided,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Accept,
    Ignore,
    Stop,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Eval {
    pub eval_id: String,
    pub thread_id: String,
    pub trigger_id: String,
    pub status: EvalStatus,
    pub decision: Option<Decision>,
    pub reason: String,
    pub created_ms: u128,
    pub decided_ms: Option<u128>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TicketStatus {
    Authorized,
    Approved,
    Denied,
    Assigned,
    Running,
    WaitingInput,
    Ready,
    Blocked,
    Failed,
    Dropped,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MandateStrength {
    ReadOnly,
    Write,
    Execute,
    Network,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Mandate {
    pub mandate_id: String,
    pub ticket_id: String,
    pub scope: String,
    pub strength: MandateStrength,
    pub policy_version: String,
    pub source_identity: String,
    pub issued_ms: u128,
    pub revoked_ms: Option<u128>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    AllowOnce,
    Always,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Approval {
    pub approval_id: String,
    pub ticket_id: String,
    pub actor: String,
    pub decision: ApprovalDecision,
    pub expires_ms: Option<u128>,
    pub decided_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub assignment_id: String,
    pub ticket_id: String,
    pub worker_id: String,
    pub session_id: Option<String>,
    pub assigned_ms: u128,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReportStatus {
    Progress,
    Waiting,
    Ready,
    Blocked,
    Failed,
    Dropped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TicketReport {
    pub report_id: String,
    pub ticket_id: String,
    pub worker_id: String,
    pub status: ReportStatus,
    pub summary: String,
    pub artifact_sha256: Option<String>,
    pub reported_ms: u128,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SideEffectStatus {
    Planned,
    Applied,
    Compensated,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SideEffect {
    pub side_effect_id: String,
    pub ticket_id: String,
    pub action: String,
    pub target: String,
    pub idempotency_key: String,
    pub reversible: bool,
    pub compensation: Option<String>,
    pub status: SideEffectStatus,
    pub recorded_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SideEffectReconciliation {
    pub ticket_id: String,
    pub planned: usize,
    pub applied: usize,
    pub compensated: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Ticket {
    pub ticket_id: String,
    pub thread_id: String,
    pub eval_id: String,
    pub owner_id: String,
    pub mandate_id: String,
    pub status: TicketStatus,
    pub requires_approval: bool,
    pub approval_id: Option<String>,
    pub assignment_id: Option<String>,
    #[serde(default)]
    pub generation: u32,
    #[serde(default)]
    pub human_takeover: bool,
    pub created_ms: u128,
    pub updated_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TaskGraphSnapshot {
    pub threads: Vec<Thread>,
    pub triggers: Vec<Trigger>,
    pub subscriptions: Vec<Subscription>,
    pub evals: Vec<Eval>,
    pub tickets: Vec<Ticket>,
    pub mandates: Vec<Mandate>,
    pub approvals: Vec<Approval>,
    pub assignments: Vec<Assignment>,
    pub reports: Vec<TicketReport>,
    pub side_effects: Vec<SideEffect>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TicketDraft {
    pub owner_id: String,
    pub scope: String,
    pub strength: MandateStrength,
    pub policy_version: String,
    pub requires_approval: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SideEffectDraft {
    pub action: String,
    pub target: String,
    pub idempotency_key: String,
    pub reversible: bool,
    pub compensation: Option<String>,
    pub status: SideEffectStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum TaskGraphEventKind {
    ThreadOpened {
        thread_id: String,
        topic: String,
    },
    TriggerIngested {
        thread_id: String,
        trigger: Trigger,
    },
    GateEvaluated {
        thread_id: String,
        trigger_id: String,
        outcome: GateOutcome,
        reason: String,
    },
    EvalStarted {
        eval_id: String,
        thread_id: String,
        trigger_id: String,
    },
    EvalDecided {
        eval_id: String,
        decision: Decision,
        reason: String,
    },
    SubscriptionAdded {
        subscription_id: String,
        thread_id: String,
        ticket_id: String,
    },
    SubscriptionRemoved {
        subscription_id: String,
    },
    ThreadClosed {
        thread_id: String,
    },
    TicketOpened {
        ticket_id: String,
        thread_id: String,
        eval_id: String,
        owner_id: String,
        mandate: Mandate,
        requires_approval: bool,
    },
    TicketApproved {
        ticket_id: String,
        approval: Approval,
    },
    TicketAssigned {
        ticket_id: String,
        assignment: Assignment,
    },
    TicketStarted {
        ticket_id: String,
        assignment_id: String,
    },
    TicketReported {
        ticket_id: String,
        report: TicketReport,
    },
    TicketCancelled {
        ticket_id: String,
        actor: String,
        reason: String,
    },
    TicketRetried {
        ticket_id: String,
        actor: String,
        reason: String,
        generation: u32,
    },
    TicketTakenOver {
        ticket_id: String,
        actor: String,
        reason: String,
    },
    TicketTakeoverReleased {
        ticket_id: String,
        actor: String,
        reason: String,
    },
    MandateRevoked {
        mandate_id: String,
        actor: String,
        reason: String,
    },
    SideEffectRecorded {
        ticket_id: String,
        side_effect: SideEffect,
    },
    SideEffectStatusChanged {
        ticket_id: String,
        side_effect_id: String,
        status: SideEffectStatus,
        evidence: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TaskGraphEvent {
    pub schema_version: u32,
    pub seq: u64,
    pub event_id: String,
    pub timestamp_unix_ms: u128,
    pub kind: TaskGraphEventKind,
    pub previous_sha256: Option<String>,
    pub event_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerIngest {
    pub trigger_id: String,
    pub duplicate: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TaskGraphError {
    #[error("thread {0} not found")]
    UnknownThread(String),
    #[error("thread {0} already exists")]
    ThreadAlreadyExists(String),
    #[error("trigger {0} already exists")]
    TriggerAlreadyExists(String),
    #[error("trigger {0} not found")]
    UnknownTrigger(String),
    #[error("trigger {0} was gated off")]
    GateIgnored(String),
    #[error("ticket {0} is under human takeover")]
    HumanTakeover(String),
    #[error("ticket {0} cannot be retried from {1:?}")]
    InvalidRetryStatus(String, TicketStatus),
    #[error("side effect {0} cannot transition from {1:?} to {2:?}")]
    InvalidSideEffectTransition(String, SideEffectStatus, SideEffectStatus),
    #[error("eval {0} not found")]
    UnknownEval(String),
    #[error("eval already exists for trigger {0}")]
    EvalAlreadyStarted(String),
    #[error("subscription {0} not found")]
    UnknownSubscription(String),
    #[error("thread {0} is closed")]
    ThreadClosed(String),
    #[error("eval {0} is already decided")]
    EvalAlreadyDecided(String),
    #[error("trigger {0} does not belong to thread {1}")]
    TriggerNotInThread(String, String),
    #[error("trigger source id must not be empty")]
    EmptySourceEventId,
    #[error("thread topic must not be empty")]
    EmptyThreadTopic,
    #[error("ticket id must not be empty")]
    EmptyTicketId,
    #[error("ticket {0} not found")]
    UnknownTicket(String),
    #[error("mandate {0} not found")]
    UnknownMandate(String),
    #[error("approval {0} not found")]
    UnknownApproval(String),
    #[error("assignment {0} not found")]
    UnknownAssignment(String),
    #[error("side effect {0} not found")]
    UnknownSideEffect(String),
    #[error("ticket {0} already exists")]
    TicketAlreadyExists(String),
    #[error("ticket already exists for eval {0}")]
    TicketAlreadyExistsForEval(String),
    #[error("eval {0} did not accept the trigger")]
    TicketNotAccepted(String),
    #[error("ticket {0} requires approval")]
    ApprovalRequired(String),
    #[error("approval for ticket {0} is missing or expired")]
    ApprovalExpired(String),
    #[error("ticket {0} is already approved")]
    TicketAlreadyApproved(String),
    #[error("actor {0} is not the ticket owner")]
    UnauthorizedActor(String),
    #[error("worker {0} is not assigned to ticket {1}")]
    WorkerMismatch(String, String),
    #[error("ticket {0} cannot transition from {1}")]
    InvalidTicketTransition(String, String),
    #[error("mandate {0} is revoked")]
    MandateRevoked(String),
    #[error("owner id must not be empty")]
    EmptyOwnerId,
    #[error("worker id must not be empty")]
    EmptyWorkerId,
    #[error("mandate scope must not be empty")]
    EmptyScope,
    #[error("idempotency key must not be empty")]
    EmptyIdempotencyKey,
    #[error("side effect action must not be empty")]
    EmptySideEffectAction,
    #[error("side effect target must not be empty")]
    EmptySideEffectTarget,
    #[error("side effect {0} already exists for ticket {1}")]
    SideEffectAlreadyExists(String, String),
    #[error("side effect idempotency key conflict for ticket {0}")]
    SideEffectConflict(String),
    #[error("event sequence mismatch: expected {expected}, got {actual}")]
    SequenceMismatch { expected: u64, actual: u64 },
    #[error("event hash mismatch at sequence {0}")]
    EventHashMismatch(u64),
    #[error("previous event hash mismatch at sequence {0}")]
    PreviousHashMismatch(u64),
    #[error("event schema version {0} is unsupported")]
    UnsupportedSchemaVersion(u32),
}

#[derive(Debug, Clone, Default)]
pub struct TaskGraph {
    threads: BTreeMap<String, Thread>,
    triggers: BTreeMap<String, Trigger>,
    trigger_sources: BTreeMap<(TriggerSource, String), String>,
    gate_outcomes: BTreeMap<String, GateOutcome>,
    evals: BTreeMap<String, Eval>,
    eval_by_trigger: BTreeMap<(String, String), String>,
    subscriptions: BTreeMap<String, Subscription>,
    subscription_by_ticket: BTreeMap<(String, String), String>,
    tickets: BTreeMap<String, Ticket>,
    ticket_by_eval: BTreeMap<String, String>,
    mandates: BTreeMap<String, Mandate>,
    approvals: BTreeMap<String, Approval>,
    assignments: BTreeMap<String, Assignment>,
    reports: BTreeMap<String, TicketReport>,
    side_effects: BTreeMap<String, SideEffect>,
    side_effect_keys: BTreeMap<(String, String), String>,
    events: Vec<TaskGraphEvent>,
    next_seq: u64,
    last_sha256: Option<String>,
}

impl TaskGraph {
    pub fn open_thread(
        &mut self,
        topic: impl Into<String>,
        now_ms: u128,
    ) -> Result<String, TaskGraphError> {
        let topic = topic.into();
        if topic.trim().is_empty() {
            return Err(TaskGraphError::EmptyThreadTopic);
        }
        let thread_id = format!("thread-{}", Uuid::new_v4());
        self.append(
            TaskGraphEventKind::ThreadOpened {
                thread_id: thread_id.clone(),
                topic,
            },
            now_ms,
        )?;
        Ok(thread_id)
    }

    pub fn ingest_trigger(
        &mut self,
        thread_id: &str,
        trigger: Trigger,
        now_ms: u128,
    ) -> Result<TriggerIngest, TaskGraphError> {
        let thread = self.thread(thread_id)?;
        if thread.status == ThreadStatus::Closed {
            return Err(TaskGraphError::ThreadClosed(thread_id.to_string()));
        }
        if trigger.source_event_id.trim().is_empty() {
            return Err(TaskGraphError::EmptySourceEventId);
        }
        let source_key = (trigger.source, trigger.source_event_id.clone());
        if let Some(existing) = self.trigger_sources.get(&source_key).cloned() {
            let existing_trigger = self
                .triggers
                .get(&existing)
                .cloned()
                .ok_or_else(|| TaskGraphError::UnknownTrigger(existing.clone()))?;
            let duplicate = thread.trigger_ids.iter().any(|id| id == &existing);
            if !duplicate {
                self.append(
                    TaskGraphEventKind::TriggerIngested {
                        thread_id: thread_id.to_string(),
                        trigger: existing_trigger,
                    },
                    now_ms,
                )?;
            }
            return Ok(TriggerIngest {
                trigger_id: existing,
                duplicate,
            });
        }
        let trigger_id = trigger.trigger_id.clone();
        self.append(
            TaskGraphEventKind::TriggerIngested {
                thread_id: thread_id.to_string(),
                trigger,
            },
            now_ms,
        )?;
        Ok(TriggerIngest {
            trigger_id,
            duplicate: false,
        })
    }

    pub fn evaluate_gate(
        &mut self,
        thread_id: &str,
        trigger_id: &str,
        outcome: GateOutcome,
        reason: impl Into<String>,
        now_ms: u128,
    ) -> Result<(), TaskGraphError> {
        let thread = self.thread(thread_id)?;
        if thread.status == ThreadStatus::Closed {
            return Err(TaskGraphError::ThreadClosed(thread_id.to_string()));
        }
        if !self.triggers.contains_key(trigger_id) {
            return Err(TaskGraphError::UnknownTrigger(trigger_id.to_string()));
        }
        if !thread.trigger_ids.iter().any(|id| id == trigger_id) {
            return Err(TaskGraphError::TriggerNotInThread(
                trigger_id.to_string(),
                thread_id.to_string(),
            ));
        }
        self.append(
            TaskGraphEventKind::GateEvaluated {
                thread_id: thread_id.to_string(),
                trigger_id: trigger_id.to_string(),
                outcome,
                reason: reason.into(),
            },
            now_ms,
        )
    }

    pub fn begin_eval(
        &mut self,
        thread_id: &str,
        trigger_id: &str,
        now_ms: u128,
    ) -> Result<String, TaskGraphError> {
        let thread = self.thread(thread_id)?;
        if thread.status == ThreadStatus::Closed {
            return Err(TaskGraphError::ThreadClosed(thread_id.to_string()));
        }
        if !self.triggers.contains_key(trigger_id) {
            return Err(TaskGraphError::UnknownTrigger(trigger_id.to_string()));
        }
        if !thread.trigger_ids.iter().any(|id| id == trigger_id) {
            return Err(TaskGraphError::TriggerNotInThread(
                trigger_id.to_string(),
                thread_id.to_string(),
            ));
        }
        if self.gate_outcomes.get(trigger_id) == Some(&GateOutcome::Ignore) {
            return Err(TaskGraphError::GateIgnored(trigger_id.to_string()));
        }
        let eval_key = (thread_id.to_string(), trigger_id.to_string());
        if let Some(existing) = self.eval_by_trigger.get(&eval_key) {
            return Ok(existing.clone());
        }
        let eval_id = format!("eval-{}", Uuid::new_v4());
        self.append(
            TaskGraphEventKind::EvalStarted {
                eval_id: eval_id.clone(),
                thread_id: thread_id.to_string(),
                trigger_id: trigger_id.to_string(),
            },
            now_ms,
        )?;
        Ok(eval_id)
    }

    pub fn decide_eval(
        &mut self,
        eval_id: &str,
        decision: Decision,
        reason: impl Into<String>,
        now_ms: u128,
    ) -> Result<(), TaskGraphError> {
        let eval = self
            .evals
            .get(eval_id)
            .ok_or_else(|| TaskGraphError::UnknownEval(eval_id.to_string()))?;
        if eval.status == EvalStatus::Decided {
            return Err(TaskGraphError::EvalAlreadyDecided(eval_id.to_string()));
        }
        self.append(
            TaskGraphEventKind::EvalDecided {
                eval_id: eval_id.to_string(),
                decision,
                reason: reason.into(),
            },
            now_ms,
        )
    }

    pub fn create_ticket_from_eval(
        &mut self,
        eval_id: &str,
        draft: TicketDraft,
        now_ms: u128,
    ) -> Result<String, TaskGraphError> {
        if draft.owner_id.trim().is_empty() {
            return Err(TaskGraphError::EmptyOwnerId);
        }
        if draft.scope.trim().is_empty() {
            return Err(TaskGraphError::EmptyScope);
        }
        let eval = self
            .evals
            .get(eval_id)
            .cloned()
            .ok_or_else(|| TaskGraphError::UnknownEval(eval_id.to_string()))?;
        if eval.status != EvalStatus::Decided || eval.decision != Some(Decision::Accept) {
            return Err(TaskGraphError::TicketNotAccepted(eval_id.to_string()));
        }
        if self.ticket_by_eval.contains_key(eval_id) {
            return Err(TaskGraphError::TicketAlreadyExistsForEval(
                eval_id.to_string(),
            ));
        }
        let thread = self.thread(&eval.thread_id)?;
        if thread.status == ThreadStatus::Closed {
            return Err(TaskGraphError::ThreadClosed(eval.thread_id.clone()));
        }
        let ticket_id = format!("ticket-{}", Uuid::new_v4());
        let mandate = Mandate {
            mandate_id: format!("mandate-{}", Uuid::new_v4()),
            ticket_id: ticket_id.clone(),
            scope: draft.scope,
            strength: draft.strength,
            policy_version: draft.policy_version,
            source_identity: draft.owner_id.clone(),
            issued_ms: now_ms,
            revoked_ms: None,
        };
        self.append(
            TaskGraphEventKind::TicketOpened {
                ticket_id: ticket_id.clone(),
                thread_id: eval.thread_id,
                eval_id: eval_id.to_string(),
                owner_id: draft.owner_id,
                mandate,
                requires_approval: draft.requires_approval,
            },
            now_ms,
        )?;
        Ok(ticket_id)
    }

    pub fn approve_ticket(
        &mut self,
        ticket_id: &str,
        actor: impl Into<String>,
        decision: ApprovalDecision,
        expires_ms: Option<u128>,
        now_ms: u128,
    ) -> Result<String, TaskGraphError> {
        let actor = actor.into();
        let ticket = self
            .tickets
            .get(ticket_id)
            .cloned()
            .ok_or_else(|| TaskGraphError::UnknownTicket(ticket_id.to_string()))?;
        if actor != ticket.owner_id {
            return Err(TaskGraphError::UnauthorizedActor(actor));
        }
        if ticket.status != TicketStatus::Authorized {
            return Err(TaskGraphError::InvalidTicketTransition(
                ticket_id.to_string(),
                format!("{:?}", ticket.status),
            ));
        }
        let approval = Approval {
            approval_id: format!("approval-{}", Uuid::new_v4()),
            ticket_id: ticket_id.to_string(),
            actor,
            decision,
            expires_ms,
            decided_ms: now_ms,
        };
        let approval_id = approval.approval_id.clone();
        self.append(
            TaskGraphEventKind::TicketApproved {
                ticket_id: ticket_id.to_string(),
                approval,
            },
            now_ms,
        )?;
        Ok(approval_id)
    }

    pub fn assign_ticket(
        &mut self,
        ticket_id: &str,
        worker_id: impl Into<String>,
        session_id: Option<String>,
        now_ms: u128,
    ) -> Result<String, TaskGraphError> {
        let worker_id = worker_id.into();
        if worker_id.trim().is_empty() {
            return Err(TaskGraphError::EmptyWorkerId);
        }
        let ticket = self
            .tickets
            .get(ticket_id)
            .cloned()
            .ok_or_else(|| TaskGraphError::UnknownTicket(ticket_id.to_string()))?;
        self.ensure_assignable(&ticket, now_ms)?;
        let assignment = Assignment {
            assignment_id: format!("assignment-{}", Uuid::new_v4()),
            ticket_id: ticket_id.to_string(),
            worker_id,
            session_id,
            assigned_ms: now_ms,
        };
        let assignment_id = assignment.assignment_id.clone();
        self.append(
            TaskGraphEventKind::TicketAssigned {
                ticket_id: ticket_id.to_string(),
                assignment,
            },
            now_ms,
        )?;
        Ok(assignment_id)
    }

    pub fn start_ticket(
        &mut self,
        ticket_id: &str,
        worker_id: &str,
        now_ms: u128,
    ) -> Result<(), TaskGraphError> {
        let ticket = self
            .tickets
            .get(ticket_id)
            .cloned()
            .ok_or_else(|| TaskGraphError::UnknownTicket(ticket_id.to_string()))?;
        if ticket.human_takeover {
            return Err(TaskGraphError::HumanTakeover(ticket_id.to_string()));
        }
        if ticket.status != TicketStatus::Assigned {
            return Err(TaskGraphError::InvalidTicketTransition(
                ticket_id.to_string(),
                format!("{:?}", ticket.status),
            ));
        }
        let assignment = self
            .assignments
            .get(
                ticket
                    .assignment_id
                    .as_deref()
                    .ok_or_else(|| TaskGraphError::UnknownAssignment(ticket_id.to_string()))?,
            )
            .ok_or_else(|| TaskGraphError::UnknownAssignment(ticket_id.to_string()))?;
        if assignment.worker_id != worker_id {
            return Err(TaskGraphError::WorkerMismatch(
                worker_id.to_string(),
                ticket_id.to_string(),
            ));
        }
        self.ensure_mandate_active(&ticket)?;
        self.ensure_approval_active(&ticket, now_ms)?;
        self.append(
            TaskGraphEventKind::TicketStarted {
                ticket_id: ticket_id.to_string(),
                assignment_id: assignment.assignment_id.clone(),
            },
            now_ms,
        )
    }

    pub fn report_ticket(
        &mut self,
        ticket_id: &str,
        worker_id: &str,
        status: ReportStatus,
        summary: impl Into<String>,
        artifact_sha256: Option<String>,
        now_ms: u128,
    ) -> Result<String, TaskGraphError> {
        let ticket = self
            .tickets
            .get(ticket_id)
            .cloned()
            .ok_or_else(|| TaskGraphError::UnknownTicket(ticket_id.to_string()))?;
        if ticket.human_takeover {
            return Err(TaskGraphError::HumanTakeover(ticket_id.to_string()));
        }
        if !matches!(
            ticket.status,
            TicketStatus::Running | TicketStatus::WaitingInput
        ) {
            return Err(TaskGraphError::InvalidTicketTransition(
                ticket_id.to_string(),
                format!("{:?}", ticket.status),
            ));
        }
        self.ensure_mandate_active(&ticket)?;
        self.ensure_approval_active(&ticket, now_ms)?;
        let assignment = self
            .assignments
            .get(
                ticket
                    .assignment_id
                    .as_deref()
                    .ok_or_else(|| TaskGraphError::UnknownAssignment(ticket_id.to_string()))?,
            )
            .ok_or_else(|| TaskGraphError::UnknownAssignment(ticket_id.to_string()))?;
        if assignment.worker_id != worker_id {
            return Err(TaskGraphError::WorkerMismatch(
                worker_id.to_string(),
                ticket_id.to_string(),
            ));
        }
        let report = TicketReport {
            report_id: format!("report-{}", Uuid::new_v4()),
            ticket_id: ticket_id.to_string(),
            worker_id: worker_id.to_string(),
            status,
            summary: summary.into(),
            artifact_sha256,
            reported_ms: now_ms,
        };
        let report_id = report.report_id.clone();
        self.append(
            TaskGraphEventKind::TicketReported {
                ticket_id: ticket_id.to_string(),
                report,
            },
            now_ms,
        )?;
        Ok(report_id)
    }

    pub fn cancel_ticket(
        &mut self,
        ticket_id: &str,
        actor: impl Into<String>,
        reason: impl Into<String>,
        now_ms: u128,
    ) -> Result<(), TaskGraphError> {
        let actor = actor.into();
        let ticket = self
            .tickets
            .get(ticket_id)
            .cloned()
            .ok_or_else(|| TaskGraphError::UnknownTicket(ticket_id.to_string()))?;
        if actor != ticket.owner_id {
            return Err(TaskGraphError::UnauthorizedActor(actor));
        }
        if matches!(
            ticket.status,
            TicketStatus::Ready
                | TicketStatus::Failed
                | TicketStatus::Dropped
                | TicketStatus::Cancelled
                | TicketStatus::Denied
        ) {
            return Ok(());
        }
        self.append(
            TaskGraphEventKind::TicketCancelled {
                ticket_id: ticket_id.to_string(),
                actor,
                reason: reason.into(),
            },
            now_ms,
        )
    }

    pub fn retry_ticket(
        &mut self,
        ticket_id: &str,
        actor: impl Into<String>,
        reason: impl Into<String>,
        now_ms: u128,
    ) -> Result<(), TaskGraphError> {
        let actor = actor.into();
        let ticket = self
            .tickets
            .get(ticket_id)
            .cloned()
            .ok_or_else(|| TaskGraphError::UnknownTicket(ticket_id.to_string()))?;
        if actor != ticket.owner_id {
            return Err(TaskGraphError::UnauthorizedActor(actor));
        }
        if ticket.human_takeover {
            return Err(TaskGraphError::HumanTakeover(ticket_id.to_string()));
        }
        if !matches!(ticket.status, TicketStatus::Failed | TicketStatus::Blocked) {
            return Err(TaskGraphError::InvalidRetryStatus(
                ticket_id.to_string(),
                ticket.status,
            ));
        }
        self.append(
            TaskGraphEventKind::TicketRetried {
                ticket_id: ticket_id.to_string(),
                actor,
                reason: reason.into(),
                generation: ticket.generation.saturating_add(1),
            },
            now_ms,
        )
    }

    pub fn takeover_ticket(
        &mut self,
        ticket_id: &str,
        actor: impl Into<String>,
        reason: impl Into<String>,
        now_ms: u128,
    ) -> Result<(), TaskGraphError> {
        let actor = actor.into();
        let ticket = self
            .tickets
            .get(ticket_id)
            .cloned()
            .ok_or_else(|| TaskGraphError::UnknownTicket(ticket_id.to_string()))?;
        if actor != ticket.owner_id {
            return Err(TaskGraphError::UnauthorizedActor(actor));
        }
        if ticket.human_takeover {
            return Ok(());
        }
        self.append(
            TaskGraphEventKind::TicketTakenOver {
                ticket_id: ticket_id.to_string(),
                actor,
                reason: reason.into(),
            },
            now_ms,
        )
    }

    pub fn release_takeover(
        &mut self,
        ticket_id: &str,
        actor: impl Into<String>,
        reason: impl Into<String>,
        now_ms: u128,
    ) -> Result<(), TaskGraphError> {
        let actor = actor.into();
        let ticket = self
            .tickets
            .get(ticket_id)
            .cloned()
            .ok_or_else(|| TaskGraphError::UnknownTicket(ticket_id.to_string()))?;
        if actor != ticket.owner_id {
            return Err(TaskGraphError::UnauthorizedActor(actor));
        }
        if !ticket.human_takeover {
            return Ok(());
        }
        self.append(
            TaskGraphEventKind::TicketTakeoverReleased {
                ticket_id: ticket_id.to_string(),
                actor,
                reason: reason.into(),
            },
            now_ms,
        )
    }

    pub fn revoke_mandate(
        &mut self,
        mandate_id: &str,
        actor: impl Into<String>,
        reason: impl Into<String>,
        now_ms: u128,
    ) -> Result<(), TaskGraphError> {
        let actor = actor.into();
        let mandate = self
            .mandates
            .get(mandate_id)
            .cloned()
            .ok_or_else(|| TaskGraphError::UnknownMandate(mandate_id.to_string()))?;
        if actor != mandate.source_identity {
            return Err(TaskGraphError::UnauthorizedActor(actor));
        }
        if mandate.revoked_ms.is_some() {
            return Ok(());
        }
        self.append(
            TaskGraphEventKind::MandateRevoked {
                mandate_id: mandate_id.to_string(),
                actor,
                reason: reason.into(),
            },
            now_ms,
        )
    }

    pub fn record_side_effect(
        &mut self,
        ticket_id: &str,
        draft: SideEffectDraft,
        now_ms: u128,
    ) -> Result<String, TaskGraphError> {
        if draft.idempotency_key.trim().is_empty() {
            return Err(TaskGraphError::EmptyIdempotencyKey);
        }
        if draft.action.trim().is_empty() {
            return Err(TaskGraphError::EmptySideEffectAction);
        }
        if draft.target.trim().is_empty() {
            return Err(TaskGraphError::EmptySideEffectTarget);
        }
        let ticket = self
            .tickets
            .get(ticket_id)
            .cloned()
            .ok_or_else(|| TaskGraphError::UnknownTicket(ticket_id.to_string()))?;
        if ticket.human_takeover {
            return Err(TaskGraphError::HumanTakeover(ticket_id.to_string()));
        }
        if !matches!(
            ticket.status,
            TicketStatus::Approved
                | TicketStatus::Assigned
                | TicketStatus::Running
                | TicketStatus::WaitingInput
        ) {
            return Err(TaskGraphError::InvalidTicketTransition(
                ticket_id.to_string(),
                format!("{:?}", ticket.status),
            ));
        }
        self.ensure_mandate_active(&ticket)?;
        self.ensure_approval_active(&ticket, now_ms)?;
        let key = (ticket_id.to_string(), draft.idempotency_key.clone());
        if let Some(existing) = self.side_effect_keys.get(&key) {
            let existing = self
                .side_effects
                .get(existing)
                .ok_or_else(|| TaskGraphError::UnknownSideEffect(key.0.clone()))?;
            if existing.action != draft.action.as_str()
                || existing.target != draft.target.as_str()
                || existing.reversible != draft.reversible
                || existing.compensation.as_deref() != draft.compensation.as_deref()
            {
                return Err(TaskGraphError::SideEffectConflict(ticket_id.to_string()));
            }
            return Ok(existing.side_effect_id.clone());
        }
        let side_effect = SideEffect {
            side_effect_id: format!("effect-{}", Uuid::new_v4()),
            ticket_id: ticket.ticket_id,
            action: draft.action,
            target: draft.target,
            idempotency_key: draft.idempotency_key,
            reversible: draft.reversible,
            compensation: draft.compensation,
            status: draft.status,
            recorded_ms: now_ms,
        };
        let side_effect_id = side_effect.side_effect_id.clone();
        self.append(
            TaskGraphEventKind::SideEffectRecorded {
                ticket_id: ticket_id.to_string(),
                side_effect,
            },
            now_ms,
        )?;
        Ok(side_effect_id)
    }

    pub fn set_side_effect_status(
        &mut self,
        ticket_id: &str,
        side_effect_id: &str,
        status: SideEffectStatus,
        evidence: impl Into<String>,
        now_ms: u128,
    ) -> Result<(), TaskGraphError> {
        let side_effect = self
            .side_effects
            .get(side_effect_id)
            .cloned()
            .ok_or_else(|| TaskGraphError::UnknownSideEffect(side_effect_id.into()))?;
        if side_effect.ticket_id != ticket_id {
            return Err(TaskGraphError::InvalidTicketTransition(
                ticket_id.into(),
                "side effect ticket mismatch".into(),
            ));
        }
        if side_effect.status == status {
            return Ok(());
        }
        let allowed = match side_effect.status {
            SideEffectStatus::Planned => {
                matches!(status, SideEffectStatus::Applied | SideEffectStatus::Failed)
            }
            SideEffectStatus::Applied => matches!(
                status,
                SideEffectStatus::Compensated | SideEffectStatus::Failed
            ),
            SideEffectStatus::Failed => matches!(status, SideEffectStatus::Applied),
            SideEffectStatus::Compensated => false,
        };
        if !allowed {
            return Err(TaskGraphError::InvalidSideEffectTransition(
                side_effect_id.into(),
                side_effect.status,
                status,
            ));
        }
        self.append(
            TaskGraphEventKind::SideEffectStatusChanged {
                ticket_id: ticket_id.into(),
                side_effect_id: side_effect_id.into(),
                status,
                evidence: evidence.into(),
            },
            now_ms,
        )
    }

    pub fn compensate_side_effect(
        &mut self,
        ticket_id: &str,
        side_effect_id: &str,
        evidence: impl Into<String>,
        now_ms: u128,
    ) -> Result<(), TaskGraphError> {
        self.set_side_effect_status(
            ticket_id,
            side_effect_id,
            SideEffectStatus::Compensated,
            evidence,
            now_ms,
        )
    }

    pub fn reconcile_side_effects(&self, ticket_id: &str) -> SideEffectReconciliation {
        let mut reconciliation = SideEffectReconciliation {
            ticket_id: ticket_id.into(),
            planned: 0,
            applied: 0,
            compensated: 0,
            failed: 0,
        };
        for effect in self
            .side_effects
            .values()
            .filter(|effect| effect.ticket_id == ticket_id)
        {
            match effect.status {
                SideEffectStatus::Planned => reconciliation.planned += 1,
                SideEffectStatus::Applied => reconciliation.applied += 1,
                SideEffectStatus::Compensated => reconciliation.compensated += 1,
                SideEffectStatus::Failed => reconciliation.failed += 1,
            }
        }
        reconciliation
    }

    fn ensure_assignable(&self, ticket: &Ticket, now_ms: u128) -> Result<(), TaskGraphError> {
        if ticket.human_takeover {
            return Err(TaskGraphError::HumanTakeover(ticket.ticket_id.clone()));
        }
        if ticket.requires_approval && ticket.status != TicketStatus::Approved {
            return Err(TaskGraphError::ApprovalRequired(ticket.ticket_id.clone()));
        }
        if !matches!(
            ticket.status,
            TicketStatus::Authorized | TicketStatus::Approved
        ) {
            return Err(TaskGraphError::InvalidTicketTransition(
                ticket.ticket_id.clone(),
                format!("{:?}", ticket.status),
            ));
        }
        self.ensure_mandate_active(ticket)?;
        self.ensure_approval_active(ticket, now_ms)
    }

    fn ensure_mandate_active(&self, ticket: &Ticket) -> Result<(), TaskGraphError> {
        let mandate = self
            .mandates
            .get(&ticket.mandate_id)
            .ok_or_else(|| TaskGraphError::UnknownMandate(ticket.mandate_id.clone()))?;
        if mandate.revoked_ms.is_some() {
            return Err(TaskGraphError::MandateRevoked(mandate.mandate_id.clone()));
        }
        Ok(())
    }

    fn ensure_approval_active(&self, ticket: &Ticket, now_ms: u128) -> Result<(), TaskGraphError> {
        if !ticket.requires_approval {
            return Ok(());
        }
        let approval =
            self.approvals
                .get(
                    ticket.approval_id.as_deref().ok_or_else(|| {
                        TaskGraphError::ApprovalRequired(ticket.ticket_id.clone())
                    })?,
                )
                .ok_or_else(|| TaskGraphError::ApprovalRequired(ticket.ticket_id.clone()))?;
        if approval.decision == ApprovalDecision::Deny {
            return Err(TaskGraphError::ApprovalRequired(ticket.ticket_id.clone()));
        }
        if approval.expires_ms.is_some_and(|expires| expires <= now_ms) {
            return Err(TaskGraphError::ApprovalExpired(ticket.ticket_id.clone()));
        }
        Ok(())
    }

    /// Subscriptions only route thread context; they never grant authority.
    pub fn add_subscription(
        &mut self,
        thread_id: &str,
        ticket_id: impl Into<String>,
        now_ms: u128,
    ) -> Result<String, TaskGraphError> {
        let ticket_id = ticket_id.into();
        if ticket_id.trim().is_empty() {
            return Err(TaskGraphError::EmptyTicketId);
        }
        let thread = self.thread(thread_id)?;
        if thread.status == ThreadStatus::Closed {
            return Err(TaskGraphError::ThreadClosed(thread_id.to_string()));
        }
        let key = (thread_id.to_string(), ticket_id.clone());
        if let Some(existing) = self.subscription_by_ticket.get(&key)
            && let Some(subscription) = self.subscriptions.get(existing)
            && subscription.status == SubscriptionStatus::Active
        {
            return Ok(existing.clone());
        }
        let subscription_id = format!("sub-{}", Uuid::new_v4());
        self.append(
            TaskGraphEventKind::SubscriptionAdded {
                subscription_id: subscription_id.clone(),
                thread_id: thread_id.to_string(),
                ticket_id,
            },
            now_ms,
        )?;
        Ok(subscription_id)
    }

    pub fn remove_subscription(
        &mut self,
        subscription_id: &str,
        now_ms: u128,
    ) -> Result<(), TaskGraphError> {
        let subscription = self
            .subscriptions
            .get(subscription_id)
            .ok_or_else(|| TaskGraphError::UnknownSubscription(subscription_id.to_string()))?;
        if subscription.status == SubscriptionStatus::Removed {
            return Ok(());
        }
        self.append(
            TaskGraphEventKind::SubscriptionRemoved {
                subscription_id: subscription_id.to_string(),
            },
            now_ms,
        )
    }

    pub fn close_thread(&mut self, thread_id: &str, now_ms: u128) -> Result<(), TaskGraphError> {
        let thread = self.thread(thread_id)?;
        if thread.status == ThreadStatus::Closed {
            return Ok(());
        }
        self.append(
            TaskGraphEventKind::ThreadClosed {
                thread_id: thread_id.to_string(),
            },
            now_ms,
        )
    }

    pub fn thread(&self, thread_id: &str) -> Result<&Thread, TaskGraphError> {
        self.threads
            .get(thread_id)
            .ok_or_else(|| TaskGraphError::UnknownThread(thread_id.to_string()))
    }

    pub fn trigger(&self, trigger_id: &str) -> Option<&Trigger> {
        self.triggers.get(trigger_id)
    }

    pub fn eval(&self, eval_id: &str) -> Option<&Eval> {
        self.evals.get(eval_id)
    }

    pub fn subscription(&self, subscription_id: &str) -> Option<&Subscription> {
        self.subscriptions.get(subscription_id)
    }

    pub fn ticket(&self, ticket_id: &str) -> Option<&Ticket> {
        self.tickets.get(ticket_id)
    }

    pub fn mandate(&self, mandate_id: &str) -> Option<&Mandate> {
        self.mandates.get(mandate_id)
    }

    pub fn approval(&self, approval_id: &str) -> Option<&Approval> {
        self.approvals.get(approval_id)
    }

    pub fn assignment(&self, assignment_id: &str) -> Option<&Assignment> {
        self.assignments.get(assignment_id)
    }

    pub fn report(&self, report_id: &str) -> Option<&TicketReport> {
        self.reports.get(report_id)
    }

    pub fn side_effect(&self, side_effect_id: &str) -> Option<&SideEffect> {
        self.side_effects.get(side_effect_id)
    }

    pub fn snapshot(&self) -> TaskGraphSnapshot {
        TaskGraphSnapshot {
            threads: self.threads.values().cloned().collect(),
            triggers: self.triggers.values().cloned().collect(),
            subscriptions: self.subscriptions.values().cloned().collect(),
            evals: self.evals.values().cloned().collect(),
            tickets: self.tickets.values().cloned().collect(),
            mandates: self.mandates.values().cloned().collect(),
            approvals: self.approvals.values().cloned().collect(),
            assignments: self.assignments.values().cloned().collect(),
            reports: self.reports.values().cloned().collect(),
            side_effects: self.side_effects.values().cloned().collect(),
        }
    }

    pub fn events(&self) -> &[TaskGraphEvent] {
        &self.events
    }

    /// Resolve the thread an event belongs to, including events that only
    /// carry an eval, ticket, mandate or subscription id.
    pub fn event_thread_id(&self, event: &TaskGraphEvent) -> Option<String> {
        match &event.kind {
            TaskGraphEventKind::ThreadOpened { thread_id, .. }
            | TaskGraphEventKind::TriggerIngested { thread_id, .. }
            | TaskGraphEventKind::EvalStarted { thread_id, .. }
            | TaskGraphEventKind::GateEvaluated { thread_id, .. }
            | TaskGraphEventKind::SubscriptionAdded { thread_id, .. }
            | TaskGraphEventKind::ThreadClosed { thread_id } => Some(thread_id.clone()),
            TaskGraphEventKind::EvalDecided { eval_id, .. } => {
                self.evals.get(eval_id).map(|eval| eval.thread_id.clone())
            }
            TaskGraphEventKind::SubscriptionRemoved { subscription_id } => self
                .subscriptions
                .get(subscription_id)
                .map(|subscription| subscription.thread_id.clone()),
            TaskGraphEventKind::TicketOpened { thread_id, .. } => Some(thread_id.clone()),
            TaskGraphEventKind::TicketApproved { ticket_id, .. }
            | TaskGraphEventKind::TicketAssigned { ticket_id, .. }
            | TaskGraphEventKind::TicketStarted { ticket_id, .. }
            | TaskGraphEventKind::TicketReported { ticket_id, .. }
            | TaskGraphEventKind::TicketCancelled { ticket_id, .. }
            | TaskGraphEventKind::TicketRetried { ticket_id, .. }
            | TaskGraphEventKind::TicketTakenOver { ticket_id, .. }
            | TaskGraphEventKind::TicketTakeoverReleased { ticket_id, .. } => self
                .tickets
                .get(ticket_id)
                .map(|ticket| ticket.thread_id.clone()),
            TaskGraphEventKind::MandateRevoked { mandate_id, .. } => self
                .mandates
                .get(mandate_id)
                .and_then(|mandate| self.tickets.get(&mandate.ticket_id))
                .map(|ticket| ticket.thread_id.clone()),
            TaskGraphEventKind::SideEffectRecorded { ticket_id, .. }
            | TaskGraphEventKind::SideEffectStatusChanged { ticket_id, .. } => self
                .tickets
                .get(ticket_id)
                .map(|ticket| ticket.thread_id.clone()),
        }
    }

    pub fn event_ticket_id(&self, event: &TaskGraphEvent) -> Option<String> {
        match &event.kind {
            TaskGraphEventKind::SubscriptionAdded { ticket_id, .. } => Some(ticket_id.clone()),
            TaskGraphEventKind::SubscriptionRemoved { subscription_id } => self
                .subscriptions
                .get(subscription_id)
                .map(|subscription| subscription.ticket_id.clone()),
            TaskGraphEventKind::TicketOpened { ticket_id, .. }
            | TaskGraphEventKind::TicketApproved { ticket_id, .. }
            | TaskGraphEventKind::TicketAssigned { ticket_id, .. }
            | TaskGraphEventKind::TicketStarted { ticket_id, .. }
            | TaskGraphEventKind::TicketReported { ticket_id, .. }
            | TaskGraphEventKind::TicketCancelled { ticket_id, .. }
            | TaskGraphEventKind::TicketRetried { ticket_id, .. }
            | TaskGraphEventKind::TicketTakenOver { ticket_id, .. }
            | TaskGraphEventKind::TicketTakeoverReleased { ticket_id, .. }
            | TaskGraphEventKind::SideEffectRecorded { ticket_id, .. }
            | TaskGraphEventKind::SideEffectStatusChanged { ticket_id, .. } => {
                Some(ticket_id.clone())
            }
            TaskGraphEventKind::MandateRevoked { mandate_id, .. } => self
                .mandates
                .get(mandate_id)
                .map(|mandate| mandate.ticket_id.clone()),
            _ => None,
        }
    }

    pub fn replay(events: &[TaskGraphEvent]) -> Result<Self, TaskGraphError> {
        let mut graph = Self::default();
        for event in events {
            graph.apply_replayed(event.clone())?;
        }
        Ok(graph)
    }

    fn append(&mut self, kind: TaskGraphEventKind, now_ms: u128) -> Result<(), TaskGraphError> {
        let seq = self.next_seq + 1;
        let event_id = Uuid::new_v4().to_string();
        let event = self.seal_event(seq, event_id, now_ms, kind, self.last_sha256.clone())?;
        self.apply_event(&event)?;
        self.events.push(event);
        Ok(())
    }

    fn apply_replayed(&mut self, event: TaskGraphEvent) -> Result<(), TaskGraphError> {
        if event.schema_version != TASK_GRAPH_SCHEMA_VERSION {
            return Err(TaskGraphError::UnsupportedSchemaVersion(
                event.schema_version,
            ));
        }
        let expected = self.next_seq + 1;
        if event.seq != expected {
            return Err(TaskGraphError::SequenceMismatch {
                expected,
                actual: event.seq,
            });
        }
        if event.previous_sha256 != self.last_sha256 {
            return Err(TaskGraphError::PreviousHashMismatch(event.seq));
        }
        if event.event_sha256 != hash_event(&event)? {
            return Err(TaskGraphError::EventHashMismatch(event.seq));
        }
        self.apply_event(&event)?;
        self.events.push(event);
        Ok(())
    }

    fn apply_event(&mut self, event: &TaskGraphEvent) -> Result<(), TaskGraphError> {
        match &event.kind {
            TaskGraphEventKind::ThreadOpened { thread_id, topic } => {
                if self.threads.contains_key(thread_id) {
                    return Err(TaskGraphError::ThreadAlreadyExists(thread_id.clone()));
                }
                self.threads.insert(
                    thread_id.clone(),
                    Thread {
                        thread_id: thread_id.clone(),
                        topic: topic.clone(),
                        status: ThreadStatus::Open,
                        trigger_ids: Vec::new(),
                        subscription_ids: Vec::new(),
                        ticket_ids: Vec::new(),
                        created_ms: event.timestamp_unix_ms,
                        updated_ms: event.timestamp_unix_ms,
                    },
                );
            }
            TaskGraphEventKind::TriggerIngested { thread_id, trigger } => {
                let existing = self.triggers.get(&trigger.trigger_id).cloned();
                if let Some(existing) = existing {
                    if existing != *trigger {
                        return Err(TaskGraphError::TriggerAlreadyExists(
                            trigger.trigger_id.clone(),
                        ));
                    }
                    let thread = self
                        .threads
                        .get_mut(thread_id)
                        .ok_or_else(|| TaskGraphError::UnknownThread(thread_id.clone()))?;
                    if thread.status == ThreadStatus::Closed {
                        return Err(TaskGraphError::ThreadClosed(thread_id.clone()));
                    }
                    if !thread
                        .trigger_ids
                        .iter()
                        .any(|id| id == &trigger.trigger_id)
                    {
                        thread.trigger_ids.push(trigger.trigger_id.clone());
                        thread.updated_ms = event.timestamp_unix_ms;
                    }
                } else {
                    let thread = self
                        .threads
                        .get_mut(thread_id)
                        .ok_or_else(|| TaskGraphError::UnknownThread(thread_id.clone()))?;
                    if thread.status == ThreadStatus::Closed {
                        return Err(TaskGraphError::ThreadClosed(thread_id.clone()));
                    }
                    self.triggers
                        .insert(trigger.trigger_id.clone(), trigger.clone());
                    self.trigger_sources.insert(
                        (trigger.source, trigger.source_event_id.clone()),
                        trigger.trigger_id.clone(),
                    );
                    thread.trigger_ids.push(trigger.trigger_id.clone());
                    thread.updated_ms = event.timestamp_unix_ms;
                }
            }
            TaskGraphEventKind::GateEvaluated {
                thread_id,
                trigger_id,
                outcome,
                ..
            } => {
                let thread = self
                    .threads
                    .get(thread_id)
                    .ok_or_else(|| TaskGraphError::UnknownThread(thread_id.clone()))?;
                if thread.status == ThreadStatus::Closed {
                    return Err(TaskGraphError::ThreadClosed(thread_id.clone()));
                }
                if !self.triggers.contains_key(trigger_id)
                    || !thread.trigger_ids.iter().any(|id| id == trigger_id)
                {
                    return Err(TaskGraphError::TriggerNotInThread(
                        trigger_id.clone(),
                        thread_id.clone(),
                    ));
                }
                self.gate_outcomes.insert(trigger_id.clone(), *outcome);
            }
            TaskGraphEventKind::EvalStarted {
                eval_id,
                thread_id,
                trigger_id,
            } => {
                let thread = self
                    .threads
                    .get_mut(thread_id)
                    .ok_or_else(|| TaskGraphError::UnknownThread(thread_id.clone()))?;
                if thread.status == ThreadStatus::Closed {
                    return Err(TaskGraphError::ThreadClosed(thread_id.clone()));
                }
                if !self.triggers.contains_key(trigger_id) {
                    return Err(TaskGraphError::UnknownTrigger(trigger_id.clone()));
                }
                if !thread.trigger_ids.iter().any(|id| id == trigger_id) {
                    return Err(TaskGraphError::TriggerNotInThread(
                        trigger_id.clone(),
                        thread_id.clone(),
                    ));
                }
                let eval_key = (thread_id.clone(), trigger_id.clone());
                if self.evals.contains_key(eval_id) || self.eval_by_trigger.contains_key(&eval_key)
                {
                    return Err(TaskGraphError::EvalAlreadyStarted(trigger_id.clone()));
                }
                self.evals.insert(
                    eval_id.clone(),
                    Eval {
                        eval_id: eval_id.clone(),
                        thread_id: thread_id.clone(),
                        trigger_id: trigger_id.clone(),
                        status: EvalStatus::Pending,
                        decision: None,
                        reason: String::new(),
                        created_ms: event.timestamp_unix_ms,
                        decided_ms: None,
                    },
                );
                self.eval_by_trigger.insert(eval_key, eval_id.clone());
                thread.updated_ms = event.timestamp_unix_ms;
            }
            TaskGraphEventKind::EvalDecided {
                eval_id,
                decision,
                reason,
            } => {
                let eval = self
                    .evals
                    .get_mut(eval_id)
                    .ok_or_else(|| TaskGraphError::UnknownEval(eval_id.clone()))?;
                if eval.status == EvalStatus::Decided {
                    return Err(TaskGraphError::EvalAlreadyDecided(eval_id.clone()));
                }
                eval.status = EvalStatus::Decided;
                eval.decision = Some(*decision);
                eval.reason = reason.clone();
                eval.decided_ms = Some(event.timestamp_unix_ms);
            }
            TaskGraphEventKind::SubscriptionAdded {
                subscription_id,
                thread_id,
                ticket_id,
            } => {
                let thread = self
                    .threads
                    .get_mut(thread_id)
                    .ok_or_else(|| TaskGraphError::UnknownThread(thread_id.clone()))?;
                if thread.status == ThreadStatus::Closed {
                    return Err(TaskGraphError::ThreadClosed(thread_id.clone()));
                }
                self.subscriptions.insert(
                    subscription_id.clone(),
                    Subscription {
                        subscription_id: subscription_id.clone(),
                        thread_id: thread_id.clone(),
                        ticket_id: ticket_id.clone(),
                        status: SubscriptionStatus::Active,
                        created_ms: event.timestamp_unix_ms,
                        removed_ms: None,
                    },
                );
                self.subscription_by_ticket.insert(
                    (thread_id.clone(), ticket_id.clone()),
                    subscription_id.clone(),
                );
                thread.subscription_ids.push(subscription_id.clone());
                thread.updated_ms = event.timestamp_unix_ms;
            }
            TaskGraphEventKind::SubscriptionRemoved { subscription_id } => {
                let subscription = self
                    .subscriptions
                    .get_mut(subscription_id)
                    .ok_or_else(|| TaskGraphError::UnknownSubscription(subscription_id.clone()))?;
                subscription.status = SubscriptionStatus::Removed;
                subscription.removed_ms = Some(event.timestamp_unix_ms);
            }
            TaskGraphEventKind::ThreadClosed { thread_id } => {
                let thread = self
                    .threads
                    .get_mut(thread_id)
                    .ok_or_else(|| TaskGraphError::UnknownThread(thread_id.clone()))?;
                thread.status = ThreadStatus::Closed;
                thread.updated_ms = event.timestamp_unix_ms;
            }
            TaskGraphEventKind::TicketOpened {
                ticket_id,
                thread_id,
                eval_id,
                owner_id,
                mandate,
                requires_approval,
            } => {
                let eval = self
                    .evals
                    .get(eval_id)
                    .ok_or_else(|| TaskGraphError::UnknownEval(eval_id.clone()))?;
                if eval.status != EvalStatus::Decided || eval.decision != Some(Decision::Accept) {
                    return Err(TaskGraphError::TicketNotAccepted(eval_id.clone()));
                }
                if eval.thread_id != *thread_id {
                    return Err(TaskGraphError::TriggerNotInThread(
                        eval.trigger_id.clone(),
                        thread_id.clone(),
                    ));
                }
                if self.tickets.contains_key(ticket_id) {
                    return Err(TaskGraphError::TicketAlreadyExists(ticket_id.clone()));
                }
                if self.ticket_by_eval.contains_key(eval_id) {
                    return Err(TaskGraphError::TicketAlreadyExistsForEval(eval_id.clone()));
                }
                if mandate.ticket_id != *ticket_id {
                    return Err(TaskGraphError::InvalidTicketTransition(
                        ticket_id.clone(),
                        "mandate ticket mismatch".into(),
                    ));
                }
                let thread = self
                    .threads
                    .get_mut(thread_id)
                    .ok_or_else(|| TaskGraphError::UnknownThread(thread_id.clone()))?;
                if thread.status == ThreadStatus::Closed {
                    return Err(TaskGraphError::ThreadClosed(thread_id.clone()));
                }
                self.mandates
                    .insert(mandate.mandate_id.clone(), mandate.clone());
                self.tickets.insert(
                    ticket_id.clone(),
                    Ticket {
                        ticket_id: ticket_id.clone(),
                        thread_id: thread_id.clone(),
                        eval_id: eval_id.clone(),
                        owner_id: owner_id.clone(),
                        mandate_id: mandate.mandate_id.clone(),
                        status: TicketStatus::Authorized,
                        requires_approval: *requires_approval,
                        approval_id: None,
                        assignment_id: None,
                        generation: 0,
                        human_takeover: false,
                        created_ms: event.timestamp_unix_ms,
                        updated_ms: event.timestamp_unix_ms,
                    },
                );
                self.ticket_by_eval
                    .insert(eval_id.clone(), ticket_id.clone());
                thread.ticket_ids.push(ticket_id.clone());
                thread.updated_ms = event.timestamp_unix_ms;
            }
            TaskGraphEventKind::TicketApproved {
                ticket_id,
                approval,
            } => {
                let ticket = self
                    .tickets
                    .get_mut(ticket_id)
                    .ok_or_else(|| TaskGraphError::UnknownTicket(ticket_id.clone()))?;
                if approval.ticket_id != *ticket_id {
                    return Err(TaskGraphError::InvalidTicketTransition(
                        ticket_id.clone(),
                        "approval ticket mismatch".into(),
                    ));
                }
                if ticket.status != TicketStatus::Authorized {
                    return Err(TaskGraphError::InvalidTicketTransition(
                        ticket_id.clone(),
                        format!("{:?}", ticket.status),
                    ));
                }
                ticket.status = match approval.decision {
                    ApprovalDecision::Deny => TicketStatus::Denied,
                    ApprovalDecision::AllowOnce | ApprovalDecision::Always => {
                        TicketStatus::Approved
                    }
                };
                ticket.approval_id = Some(approval.approval_id.clone());
                ticket.updated_ms = event.timestamp_unix_ms;
                self.approvals
                    .insert(approval.approval_id.clone(), approval.clone());
            }
            TaskGraphEventKind::TicketAssigned {
                ticket_id,
                assignment,
            } => {
                let ticket = self
                    .tickets
                    .get_mut(ticket_id)
                    .ok_or_else(|| TaskGraphError::UnknownTicket(ticket_id.clone()))?;
                if assignment.ticket_id != *ticket_id {
                    return Err(TaskGraphError::InvalidTicketTransition(
                        ticket_id.clone(),
                        "assignment ticket mismatch".into(),
                    ));
                }
                if ticket.requires_approval && ticket.status != TicketStatus::Approved {
                    return Err(TaskGraphError::ApprovalRequired(ticket_id.clone()));
                }
                if !matches!(
                    ticket.status,
                    TicketStatus::Authorized | TicketStatus::Approved
                ) {
                    return Err(TaskGraphError::InvalidTicketTransition(
                        ticket_id.clone(),
                        format!("{:?}", ticket.status),
                    ));
                }
                let mandate = self
                    .mandates
                    .get(&ticket.mandate_id)
                    .ok_or_else(|| TaskGraphError::UnknownMandate(ticket.mandate_id.clone()))?;
                if mandate.revoked_ms.is_some() {
                    return Err(TaskGraphError::MandateRevoked(mandate.mandate_id.clone()));
                }
                ticket.status = TicketStatus::Assigned;
                ticket.assignment_id = Some(assignment.assignment_id.clone());
                ticket.updated_ms = event.timestamp_unix_ms;
                self.assignments
                    .insert(assignment.assignment_id.clone(), assignment.clone());
            }
            TaskGraphEventKind::TicketStarted {
                ticket_id,
                assignment_id,
            } => {
                let ticket = self
                    .tickets
                    .get_mut(ticket_id)
                    .ok_or_else(|| TaskGraphError::UnknownTicket(ticket_id.clone()))?;
                if ticket.status != TicketStatus::Assigned {
                    return Err(TaskGraphError::InvalidTicketTransition(
                        ticket_id.clone(),
                        format!("{:?}", ticket.status),
                    ));
                }
                if ticket.assignment_id.as_deref() != Some(assignment_id.as_str()) {
                    return Err(TaskGraphError::UnknownAssignment(assignment_id.clone()));
                }
                ticket.status = TicketStatus::Running;
                ticket.updated_ms = event.timestamp_unix_ms;
            }
            TaskGraphEventKind::TicketReported { ticket_id, report } => {
                let ticket = self
                    .tickets
                    .get_mut(ticket_id)
                    .ok_or_else(|| TaskGraphError::UnknownTicket(ticket_id.clone()))?;
                if !matches!(
                    ticket.status,
                    TicketStatus::Running | TicketStatus::WaitingInput
                ) {
                    return Err(TaskGraphError::InvalidTicketTransition(
                        ticket_id.clone(),
                        format!("{:?}", ticket.status),
                    ));
                }
                if report.ticket_id != *ticket_id {
                    return Err(TaskGraphError::InvalidTicketTransition(
                        ticket_id.clone(),
                        "report ticket mismatch".into(),
                    ));
                }
                let assignment =
                    self.assignments
                        .get(
                            ticket.assignment_id.as_deref().ok_or_else(|| {
                                TaskGraphError::UnknownAssignment(ticket_id.clone())
                            })?,
                        )
                        .ok_or_else(|| TaskGraphError::UnknownAssignment(ticket_id.clone()))?;
                if assignment.worker_id != report.worker_id {
                    return Err(TaskGraphError::WorkerMismatch(
                        report.worker_id.clone(),
                        ticket_id.clone(),
                    ));
                }
                ticket.status = match report.status {
                    ReportStatus::Progress => TicketStatus::Running,
                    ReportStatus::Waiting => TicketStatus::WaitingInput,
                    ReportStatus::Ready => TicketStatus::Ready,
                    ReportStatus::Blocked => TicketStatus::Blocked,
                    ReportStatus::Failed => TicketStatus::Failed,
                    ReportStatus::Dropped => TicketStatus::Dropped,
                };
                ticket.updated_ms = event.timestamp_unix_ms;
                self.reports
                    .insert(report.report_id.clone(), report.clone());
            }
            TaskGraphEventKind::TicketCancelled {
                ticket_id,
                actor: _,
                reason: _,
            } => {
                let ticket = self
                    .tickets
                    .get_mut(ticket_id)
                    .ok_or_else(|| TaskGraphError::UnknownTicket(ticket_id.clone()))?;
                ticket.status = TicketStatus::Cancelled;
                ticket.updated_ms = event.timestamp_unix_ms;
            }
            TaskGraphEventKind::TicketRetried {
                ticket_id,
                generation,
                ..
            } => {
                let ticket = self
                    .tickets
                    .get_mut(ticket_id)
                    .ok_or_else(|| TaskGraphError::UnknownTicket(ticket_id.clone()))?;
                ticket.status = TicketStatus::Authorized;
                ticket.approval_id = None;
                ticket.assignment_id = None;
                ticket.generation = *generation;
                ticket.human_takeover = false;
                ticket.updated_ms = event.timestamp_unix_ms;
            }
            TaskGraphEventKind::TicketTakenOver { ticket_id, .. } => {
                let ticket = self
                    .tickets
                    .get_mut(ticket_id)
                    .ok_or_else(|| TaskGraphError::UnknownTicket(ticket_id.clone()))?;
                ticket.human_takeover = true;
                ticket.updated_ms = event.timestamp_unix_ms;
            }
            TaskGraphEventKind::TicketTakeoverReleased { ticket_id, .. } => {
                let ticket = self
                    .tickets
                    .get_mut(ticket_id)
                    .ok_or_else(|| TaskGraphError::UnknownTicket(ticket_id.clone()))?;
                ticket.human_takeover = false;
                ticket.updated_ms = event.timestamp_unix_ms;
            }
            TaskGraphEventKind::MandateRevoked {
                mandate_id,
                actor: _,
                reason: _,
            } => {
                let mandate = self
                    .mandates
                    .get_mut(mandate_id)
                    .ok_or_else(|| TaskGraphError::UnknownMandate(mandate_id.clone()))?;
                mandate.revoked_ms = Some(event.timestamp_unix_ms);
            }
            TaskGraphEventKind::SideEffectRecorded {
                ticket_id,
                side_effect,
            } => {
                if !self.tickets.contains_key(ticket_id) {
                    return Err(TaskGraphError::UnknownTicket(ticket_id.clone()));
                }
                if side_effect.ticket_id != *ticket_id {
                    return Err(TaskGraphError::InvalidTicketTransition(
                        ticket_id.clone(),
                        "side effect ticket mismatch".into(),
                    ));
                }
                let key = (ticket_id.clone(), side_effect.idempotency_key.clone());
                if let Some(existing) = self.side_effect_keys.get(&key)
                    && existing != &side_effect.side_effect_id
                {
                    return Err(TaskGraphError::SideEffectAlreadyExists(
                        side_effect.side_effect_id.clone(),
                        ticket_id.clone(),
                    ));
                }
                self.side_effects
                    .insert(side_effect.side_effect_id.clone(), side_effect.clone());
                self.side_effect_keys
                    .insert(key, side_effect.side_effect_id.clone());
            }
            TaskGraphEventKind::SideEffectStatusChanged {
                ticket_id,
                side_effect_id,
                status,
                ..
            } => {
                let effect = self
                    .side_effects
                    .get_mut(side_effect_id)
                    .ok_or_else(|| TaskGraphError::UnknownSideEffect(side_effect_id.clone()))?;
                if effect.ticket_id != *ticket_id {
                    return Err(TaskGraphError::InvalidTicketTransition(
                        ticket_id.clone(),
                        "side effect ticket mismatch".into(),
                    ));
                }
                effect.status = *status;
            }
        }
        self.next_seq = event.seq;
        self.last_sha256 = Some(event.event_sha256.clone());
        Ok(())
    }

    fn seal_event(
        &self,
        seq: u64,
        event_id: String,
        timestamp_unix_ms: u128,
        kind: TaskGraphEventKind,
        previous_sha256: Option<String>,
    ) -> Result<TaskGraphEvent, TaskGraphError> {
        let mut event = TaskGraphEvent {
            schema_version: TASK_GRAPH_SCHEMA_VERSION,
            seq,
            event_id,
            timestamp_unix_ms,
            kind,
            previous_sha256,
            event_sha256: String::new(),
        };
        event.event_sha256 = hash_event(&event)?;
        Ok(event)
    }
}

#[derive(Serialize)]
struct EventHashPayload<'a> {
    schema_version: u32,
    seq: u64,
    event_id: &'a str,
    timestamp_unix_ms: u128,
    kind: &'a TaskGraphEventKind,
    previous_sha256: &'a Option<String>,
}

fn hash_event(event: &TaskGraphEvent) -> Result<String, TaskGraphError> {
    let payload = EventHashPayload {
        schema_version: event.schema_version,
        seq: event.seq,
        event_id: &event.event_id,
        timestamp_unix_ms: event.timestamp_unix_ms,
        kind: &event.kind,
        previous_sha256: &event.previous_sha256,
    };
    let bytes =
        serde_json::to_vec(&payload).map_err(|_| TaskGraphError::EventHashMismatch(event.seq))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trigger(source_event_id: &str) -> Trigger {
        Trigger {
            trigger_id: format!("trigger-{source_event_id}"),
            source: TriggerSource::Web,
            source_event_id: source_event_id.into(),
            summary: "test trigger".into(),
            received_ms: 1,
        }
    }

    #[test]
    fn thread_trigger_eval_flow_replays() {
        let mut graph = TaskGraph::default();
        let thread_id = graph.open_thread("project alpha", 10).unwrap();
        let ingested = graph
            .ingest_trigger(&thread_id, trigger("evt-1"), 11)
            .unwrap();
        assert!(!ingested.duplicate);
        let eval_id = graph
            .begin_eval(&thread_id, &ingested.trigger_id, 12)
            .unwrap();
        graph
            .decide_eval(&eval_id, Decision::Accept, "bounded task", 13)
            .unwrap();

        let replayed = TaskGraph::replay(graph.events()).unwrap();
        let thread = replayed.thread(&thread_id).unwrap();
        assert_eq!(thread.trigger_ids, vec![ingested.trigger_id.clone()]);
        let eval = replayed.eval(&eval_id).unwrap();
        assert_eq!(eval.status, EvalStatus::Decided);
        assert_eq!(eval.decision, Some(Decision::Accept));
    }

    #[test]
    fn duplicate_trigger_is_idempotent_and_does_not_append() {
        let mut graph = TaskGraph::default();
        let thread_id = graph.open_thread("project alpha", 10).unwrap();
        let first = graph
            .ingest_trigger(&thread_id, trigger("evt-1"), 11)
            .unwrap();
        let before = graph.events().len();
        let second = graph
            .ingest_trigger(&thread_id, trigger("evt-1"), 12)
            .unwrap();
        assert!(second.duplicate);
        assert_eq!(second.trigger_id, first.trigger_id);
        assert_eq!(graph.events().len(), before);
    }

    #[test]
    fn trigger_can_belong_to_multiple_threads_but_eval_is_thread_scoped() {
        let mut graph = TaskGraph::default();
        let first = graph.open_thread("first", 10).unwrap();
        let second = graph.open_thread("second", 11).unwrap();
        let first_ingest = graph.ingest_trigger(&first, trigger("evt-1"), 12).unwrap();
        let second_ingest = graph.ingest_trigger(&second, trigger("evt-1"), 13).unwrap();
        assert!(!second_ingest.duplicate);
        assert_eq!(second_ingest.trigger_id, first_ingest.trigger_id);
        assert_eq!(
            graph.thread(&second).unwrap().trigger_ids,
            vec![first_ingest.trigger_id.clone()]
        );
        let first_eval = graph
            .begin_eval(&first, &first_ingest.trigger_id, 14)
            .unwrap();
        let second_eval = graph
            .begin_eval(&second, &second_ingest.trigger_id, 15)
            .unwrap();
        assert_ne!(first_eval, second_eval);
    }

    #[test]
    fn eval_is_idempotent_and_decision_is_terminal() {
        let mut graph = TaskGraph::default();
        let thread_id = graph.open_thread("project alpha", 10).unwrap();
        let trigger_id = graph
            .ingest_trigger(&thread_id, trigger("evt-1"), 11)
            .unwrap()
            .trigger_id;
        let first = graph.begin_eval(&thread_id, &trigger_id, 12).unwrap();
        let second = graph.begin_eval(&thread_id, &trigger_id, 13).unwrap();
        assert_eq!(first, second);
        graph
            .decide_eval(&first, Decision::Ignore, "not actionable", 14)
            .unwrap();
        let error = graph
            .decide_eval(&first, Decision::Accept, "changed mind", 15)
            .unwrap_err();
        assert_eq!(error, TaskGraphError::EvalAlreadyDecided(first));
    }

    #[test]
    fn subscription_routes_context_without_granting_authority() {
        let mut graph = TaskGraph::default();
        let thread_id = graph.open_thread("project alpha", 10).unwrap();
        let subscription_id = graph.add_subscription(&thread_id, "ticket-1", 11).unwrap();
        let subscription = graph.subscription(&subscription_id).unwrap();
        assert_eq!(subscription.status, SubscriptionStatus::Active);
        assert_eq!(subscription.ticket_id, "ticket-1");
        // The subscription struct has no permission field; authority remains in Mandate/Policy.
        graph.remove_subscription(&subscription_id, 12).unwrap();
        assert_eq!(
            graph.subscription(&subscription_id).unwrap().status,
            SubscriptionStatus::Removed
        );
    }

    #[test]
    fn closed_thread_rejects_new_work() {
        let mut graph = TaskGraph::default();
        let thread_id = graph.open_thread("project alpha", 10).unwrap();
        graph.close_thread(&thread_id, 11).unwrap();
        let error = graph
            .ingest_trigger(&thread_id, trigger("evt-1"), 12)
            .unwrap_err();
        assert_eq!(error, TaskGraphError::ThreadClosed(thread_id));
    }

    #[test]
    fn unknown_or_cross_thread_trigger_cannot_start_eval() {
        let mut graph = TaskGraph::default();
        let first = graph.open_thread("first", 10).unwrap();
        let second = graph.open_thread("second", 11).unwrap();
        let trigger_id = graph
            .ingest_trigger(&first, trigger("evt-1"), 12)
            .unwrap()
            .trigger_id;
        assert_eq!(
            graph.begin_eval(&second, &trigger_id, 13).unwrap_err(),
            TaskGraphError::TriggerNotInThread(trigger_id.clone(), second)
        );
        assert_eq!(
            graph.begin_eval(&first, "trigger-missing", 14).unwrap_err(),
            TaskGraphError::UnknownTrigger("trigger-missing".into())
        );
    }

    #[test]
    fn replay_detects_tampering_and_sequence_gaps() {
        let mut graph = TaskGraph::default();
        let thread_id = graph.open_thread("project alpha", 10).unwrap();
        graph
            .ingest_trigger(&thread_id, trigger("evt-1"), 11)
            .unwrap();
        let mut events = graph.events().to_vec();
        events[1].kind = TaskGraphEventKind::ThreadClosed {
            thread_id: thread_id.clone(),
        };
        assert_eq!(
            TaskGraph::replay(&events).unwrap_err(),
            TaskGraphError::EventHashMismatch(2)
        );

        let mut events = graph.events().to_vec();
        events[1].seq = 3;
        assert!(matches!(
            TaskGraph::replay(&events).unwrap_err(),
            TaskGraphError::SequenceMismatch { .. }
        ));
    }

    #[test]
    fn subscription_is_idempotent_per_thread_and_ticket() {
        let mut graph = TaskGraph::default();
        let thread_id = graph.open_thread("project alpha", 10).unwrap();
        let first = graph.add_subscription(&thread_id, "ticket-1", 11).unwrap();
        let second = graph.add_subscription(&thread_id, "ticket-1", 12).unwrap();
        assert_eq!(first, second);
        assert_eq!(graph.thread(&thread_id).unwrap().subscription_ids.len(), 1);
    }

    #[test]
    fn event_log_round_trips_through_json() {
        let mut graph = TaskGraph::default();
        graph.open_thread("project alpha", 10).unwrap();
        let encoded = serde_json::to_string(graph.events()).unwrap();
        let decoded: Vec<TaskGraphEvent> = serde_json::from_str(&encoded).unwrap();
        let replayed = TaskGraph::replay(&decoded).unwrap();
        assert_eq!(replayed.events(), graph.events());
    }

    #[test]
    fn empty_inputs_fail_closed() {
        let mut graph = TaskGraph::default();
        assert_eq!(
            graph.open_thread("   ", 1).unwrap_err(),
            TaskGraphError::EmptyThreadTopic
        );
        let thread_id = graph.open_thread("project alpha", 2).unwrap();
        let mut bad = trigger("");
        bad.source_event_id.clear();
        assert_eq!(
            graph.ingest_trigger(&thread_id, bad, 3).unwrap_err(),
            TaskGraphError::EmptySourceEventId
        );
    }

    #[test]
    fn decisions_cover_all_supported_outcomes() {
        let mut graph = TaskGraph::default();
        let thread_id = graph.open_thread("project alpha", 10).unwrap();
        for (index, decision) in [
            Decision::Accept,
            Decision::Ignore,
            Decision::Stop,
            Decision::Unavailable,
        ]
        .into_iter()
        .enumerate()
        {
            let trigger_id = graph
                .ingest_trigger(&thread_id, trigger(&format!("evt-{index}")), 11)
                .unwrap()
                .trigger_id;
            let eval_id = graph.begin_eval(&thread_id, &trigger_id, 12).unwrap();
            graph.decide_eval(&eval_id, decision, "test", 13).unwrap();
            assert_eq!(graph.eval(&eval_id).unwrap().decision, Some(decision));
        }
    }

    fn accepted_ticket(graph: &mut TaskGraph, requires_approval: bool) -> String {
        let thread_id = graph.open_thread("project alpha", 10).unwrap();
        let trigger_id = graph
            .ingest_trigger(&thread_id, trigger("evt-ticket"), 11)
            .unwrap()
            .trigger_id;
        let eval_id = graph.begin_eval(&thread_id, &trigger_id, 12).unwrap();
        graph
            .decide_eval(&eval_id, Decision::Accept, "accepted", 13)
            .unwrap();
        graph
            .create_ticket_from_eval(
                &eval_id,
                TicketDraft {
                    owner_id: "owner-1".into(),
                    scope: "read project alpha".into(),
                    strength: MandateStrength::ReadOnly,
                    policy_version: "policy-v1".into(),
                    requires_approval,
                },
                14,
            )
            .unwrap()
    }

    #[test]
    fn ticket_flow_requires_approval_assign_start_and_report() {
        let mut graph = TaskGraph::default();
        let ticket_id = accepted_ticket(&mut graph, true);
        assert_eq!(
            graph.ticket(&ticket_id).unwrap().status,
            TicketStatus::Authorized
        );
        assert_eq!(
            graph
                .assign_ticket(&ticket_id, "worker-1", None, 15)
                .unwrap_err(),
            TaskGraphError::ApprovalRequired(ticket_id.clone())
        );
        let approval_id = graph
            .approve_ticket(&ticket_id, "owner-1", ApprovalDecision::AllowOnce, None, 16)
            .unwrap();
        assert_eq!(
            graph.ticket(&ticket_id).unwrap().status,
            TicketStatus::Approved
        );
        assert_eq!(
            graph.approval(&approval_id).unwrap().decision,
            ApprovalDecision::AllowOnce
        );
        let assignment_id = graph
            .assign_ticket(&ticket_id, "worker-1", Some("session-1".into()), 17)
            .unwrap();
        assert_eq!(
            graph.ticket(&ticket_id).unwrap().status,
            TicketStatus::Assigned
        );
        assert_eq!(
            graph.assignment(&assignment_id).unwrap().worker_id,
            "worker-1"
        );
        graph.start_ticket(&ticket_id, "worker-1", 18).unwrap();
        let report_id = graph
            .report_ticket(
                &ticket_id,
                "worker-1",
                ReportStatus::Ready,
                "artifact ready",
                Some("sha256:abc".into()),
                19,
            )
            .unwrap();
        assert_eq!(
            graph.ticket(&ticket_id).unwrap().status,
            TicketStatus::Ready
        );
        assert_eq!(
            graph.report(&report_id).unwrap().status,
            ReportStatus::Ready
        );
        let replayed = TaskGraph::replay(graph.events()).unwrap();
        assert_eq!(
            replayed.ticket(&ticket_id).unwrap().status,
            TicketStatus::Ready
        );
        assert_eq!(
            replayed
                .report(&report_id)
                .unwrap()
                .artifact_sha256
                .as_deref(),
            Some("sha256:abc")
        );
    }

    #[test]
    fn unauthorized_or_expired_approval_cannot_start_ticket() {
        let mut graph = TaskGraph::default();
        let ticket_id = accepted_ticket(&mut graph, true);
        assert_eq!(
            graph
                .approve_ticket(
                    &ticket_id,
                    "other-owner",
                    ApprovalDecision::AllowOnce,
                    None,
                    15
                )
                .unwrap_err(),
            TaskGraphError::UnauthorizedActor("other-owner".into())
        );
        graph
            .approve_ticket(
                &ticket_id,
                "owner-1",
                ApprovalDecision::AllowOnce,
                Some(20),
                16,
            )
            .unwrap();
        graph
            .assign_ticket(&ticket_id, "worker-1", None, 17)
            .unwrap();
        assert_eq!(
            graph.start_ticket(&ticket_id, "worker-1", 21).unwrap_err(),
            TaskGraphError::ApprovalExpired(ticket_id.clone())
        );
    }

    #[test]
    fn revoked_mandate_blocks_assignment() {
        let mut graph = TaskGraph::default();
        let ticket_id = accepted_ticket(&mut graph, false);
        let mandate_id = graph.ticket(&ticket_id).unwrap().mandate_id.clone();
        graph
            .revoke_mandate(&mandate_id, "owner-1", "changed mind", 15)
            .unwrap();
        assert_eq!(
            graph
                .assign_ticket(&ticket_id, "worker-1", None, 16)
                .unwrap_err(),
            TaskGraphError::MandateRevoked(mandate_id)
        );
    }

    #[test]
    fn worker_mismatch_and_side_effect_idempotency_are_enforced() {
        let mut graph = TaskGraph::default();
        let ticket_id = accepted_ticket(&mut graph, false);
        graph
            .assign_ticket(&ticket_id, "worker-1", None, 15)
            .unwrap();
        graph.start_ticket(&ticket_id, "worker-1", 16).unwrap();
        assert_eq!(
            graph
                .report_ticket(
                    &ticket_id,
                    "worker-2",
                    ReportStatus::Progress,
                    "wrong worker",
                    None,
                    17
                )
                .unwrap_err(),
            TaskGraphError::WorkerMismatch("worker-2".into(), ticket_id.clone())
        );
        let draft = SideEffectDraft {
            action: "write".into(),
            target: "/workspace/result.txt".into(),
            idempotency_key: "effect-1".into(),
            reversible: true,
            compensation: Some("delete result.txt".into()),
            status: SideEffectStatus::Planned,
        };
        let first = graph
            .record_side_effect(&ticket_id, draft.clone(), 18)
            .unwrap();
        let second = graph.record_side_effect(&ticket_id, draft, 19).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            graph.side_effect(&first).unwrap().status,
            SideEffectStatus::Planned
        );
    }

    #[test]
    fn report_requires_live_mandate_and_unexpired_approval() {
        let mut graph = TaskGraph::default();
        let ticket_id = accepted_ticket(&mut graph, true);
        graph
            .approve_ticket(&ticket_id, "owner-1", ApprovalDecision::AllowOnce, None, 15)
            .unwrap();
        graph
            .assign_ticket(&ticket_id, "worker-1", None, 16)
            .unwrap();
        graph.start_ticket(&ticket_id, "worker-1", 17).unwrap();
        let mandate_id = graph.ticket(&ticket_id).unwrap().mandate_id.clone();
        graph
            .revoke_mandate(&mandate_id, "owner-1", "changed mind", 18)
            .unwrap();
        assert_eq!(
            graph
                .report_ticket(
                    &ticket_id,
                    "worker-1",
                    ReportStatus::Progress,
                    "still working",
                    None,
                    19,
                )
                .unwrap_err(),
            TaskGraphError::MandateRevoked(mandate_id)
        );

        let mut graph = TaskGraph::default();
        let ticket_id = accepted_ticket(&mut graph, true);
        graph
            .approve_ticket(
                &ticket_id,
                "owner-1",
                ApprovalDecision::AllowOnce,
                Some(30),
                15,
            )
            .unwrap();
        graph
            .assign_ticket(&ticket_id, "worker-1", None, 16)
            .unwrap();
        graph.start_ticket(&ticket_id, "worker-1", 20).unwrap();
        assert_eq!(
            graph
                .report_ticket(
                    &ticket_id,
                    "worker-1",
                    ReportStatus::Ready,
                    "ready",
                    None,
                    30,
                )
                .unwrap_err(),
            TaskGraphError::ApprovalExpired(ticket_id)
        );
    }

    #[test]
    fn side_effect_conflicts_and_completed_tickets_fail_closed() {
        let mut graph = TaskGraph::default();
        let ticket_id = accepted_ticket(&mut graph, false);
        graph
            .assign_ticket(&ticket_id, "worker-1", None, 15)
            .unwrap();
        graph.start_ticket(&ticket_id, "worker-1", 16).unwrap();
        let draft = SideEffectDraft {
            action: "write".into(),
            target: "/workspace/result.txt".into(),
            idempotency_key: "effect-1".into(),
            reversible: true,
            compensation: Some("delete result.txt".into()),
            status: SideEffectStatus::Planned,
        };
        graph
            .record_side_effect(&ticket_id, draft.clone(), 17)
            .unwrap();
        let mut conflicting = draft.clone();
        conflicting.target = "/workspace/other.txt".into();
        assert_eq!(
            graph
                .record_side_effect(&ticket_id, conflicting, 18)
                .unwrap_err(),
            TaskGraphError::SideEffectConflict(ticket_id.clone())
        );
        graph
            .report_ticket(
                &ticket_id,
                "worker-1",
                ReportStatus::Ready,
                "ready",
                None,
                19,
            )
            .unwrap();
        let mut late = draft;
        late.idempotency_key = "effect-2".into();
        assert!(matches!(
            graph.record_side_effect(&ticket_id, late, 20).unwrap_err(),
            TaskGraphError::InvalidTicketTransition(_, _)
        ));
    }

    #[test]
    fn side_effect_requires_unexpired_approval() {
        let mut graph = TaskGraph::default();
        let ticket_id = accepted_ticket(&mut graph, true);
        graph
            .approve_ticket(
                &ticket_id,
                "owner-1",
                ApprovalDecision::AllowOnce,
                Some(20),
                15,
            )
            .unwrap();
        graph
            .assign_ticket(&ticket_id, "worker-1", None, 16)
            .unwrap();
        graph.start_ticket(&ticket_id, "worker-1", 19).unwrap();
        assert_eq!(
            graph
                .record_side_effect(
                    &ticket_id,
                    SideEffectDraft {
                        action: "write".into(),
                        target: "/workspace/result.txt".into(),
                        idempotency_key: "effect-1".into(),
                        reversible: true,
                        compensation: None,
                        status: SideEffectStatus::Applied,
                    },
                    30,
                )
                .unwrap_err(),
            TaskGraphError::ApprovalExpired(ticket_id)
        );
    }

    #[test]
    fn side_effect_lifecycle_and_reconciliation_replay() {
        let mut graph = TaskGraph::default();
        let ticket_id = accepted_ticket(&mut graph, false);
        graph
            .assign_ticket(&ticket_id, "worker-1", None, 15)
            .unwrap();
        graph.start_ticket(&ticket_id, "worker-1", 16).unwrap();
        let effect_id = graph
            .record_side_effect(
                &ticket_id,
                SideEffectDraft {
                    action: "write".into(),
                    target: "/workspace/out".into(),
                    idempotency_key: "effect-1".into(),
                    reversible: true,
                    compensation: Some("delete".into()),
                    status: SideEffectStatus::Planned,
                },
                17,
            )
            .unwrap();
        graph
            .set_side_effect_status(
                &ticket_id,
                &effect_id,
                SideEffectStatus::Applied,
                "applied",
                18,
            )
            .unwrap();
        graph
            .compensate_side_effect(&ticket_id, &effect_id, "rolled back", 19)
            .unwrap();
        let reconciliation = graph.reconcile_side_effects(&ticket_id);
        assert_eq!(reconciliation.compensated, 1);
        assert_eq!(reconciliation.planned, 0);
        assert_eq!(
            TaskGraph::replay(graph.events())
                .unwrap()
                .side_effect(&effect_id)
                .unwrap()
                .status,
            SideEffectStatus::Compensated
        );
        assert!(matches!(
            graph
                .set_side_effect_status(
                    &ticket_id,
                    &effect_id,
                    SideEffectStatus::Applied,
                    "late",
                    20,
                )
                .unwrap_err(),
            TaskGraphError::InvalidSideEffectTransition(_, _, _)
        ));
    }

    #[test]
    fn retry_and_takeover_are_audited_and_replayable() {
        let mut graph = TaskGraph::default();
        let ticket_id = accepted_ticket(&mut graph, false);
        graph
            .assign_ticket(&ticket_id, "worker-1", None, 15)
            .unwrap();
        graph.start_ticket(&ticket_id, "worker-1", 16).unwrap();
        graph
            .report_ticket(
                &ticket_id,
                "worker-1",
                ReportStatus::Failed,
                "failed",
                None,
                17,
            )
            .unwrap();
        graph
            .retry_ticket(&ticket_id, "owner-1", "retry", 18)
            .unwrap();
        let ticket = graph.ticket(&ticket_id).unwrap();
        assert_eq!(ticket.status, TicketStatus::Authorized);
        assert_eq!(ticket.generation, 1);
        assert!(ticket.approval_id.is_none());
        assert!(ticket.assignment_id.is_none());
        let replayed = TaskGraph::replay(graph.events()).unwrap();
        assert_eq!(replayed.ticket(&ticket_id).unwrap().generation, 1);

        let mut graph = TaskGraph::default();
        let ticket_id = accepted_ticket(&mut graph, false);
        graph
            .assign_ticket(&ticket_id, "worker-1", None, 15)
            .unwrap();
        graph
            .takeover_ticket(&ticket_id, "owner-1", "human control", 16)
            .unwrap();
        assert!(graph.ticket(&ticket_id).unwrap().human_takeover);
        assert_eq!(
            graph.start_ticket(&ticket_id, "worker-1", 17).unwrap_err(),
            TaskGraphError::HumanTakeover(ticket_id.clone())
        );
        graph
            .release_takeover(&ticket_id, "owner-1", "hand back", 18)
            .unwrap();
        graph.start_ticket(&ticket_id, "worker-1", 19).unwrap();
        assert_eq!(
            TaskGraph::replay(graph.events())
                .unwrap()
                .ticket(&ticket_id)
                .unwrap()
                .status,
            TicketStatus::Running
        );
    }

    #[test]
    fn failed_ticket_cannot_be_retried_from_ineligible_state() {
        let mut graph = TaskGraph::default();
        let ticket_id = accepted_ticket(&mut graph, false);
        assert_eq!(
            graph
                .retry_ticket(&ticket_id, "owner-1", "too early", 15)
                .unwrap_err(),
            TaskGraphError::InvalidRetryStatus(ticket_id, TicketStatus::Authorized)
        );
    }

    #[test]
    fn ticket_requires_accept_decision() {
        let mut graph = TaskGraph::default();
        let thread_id = graph.open_thread("project alpha", 10).unwrap();
        let trigger_id = graph
            .ingest_trigger(&thread_id, trigger("evt-ignore"), 11)
            .unwrap()
            .trigger_id;
        let eval_id = graph.begin_eval(&thread_id, &trigger_id, 12).unwrap();
        graph
            .decide_eval(&eval_id, Decision::Ignore, "not actionable", 13)
            .unwrap();
        assert_eq!(
            graph
                .create_ticket_from_eval(
                    &eval_id,
                    TicketDraft {
                        owner_id: "owner-1".into(),
                        scope: "scope".into(),
                        strength: MandateStrength::ReadOnly,
                        policy_version: "policy-v1".into(),
                        requires_approval: false,
                    },
                    14,
                )
                .unwrap_err(),
            TaskGraphError::TicketNotAccepted(eval_id)
        );
    }

    #[test]
    fn duplicate_thread_open_is_rejected_by_replay() {
        let mut graph = TaskGraph::default();
        let thread_id = graph.open_thread("project alpha", 10).unwrap();
        let mut events = graph.events().to_vec();
        let duplicate = events[0].clone();
        let mut forged = duplicate.clone();
        forged.seq = 2;
        forged.previous_sha256 = Some(duplicate.event_sha256.clone());
        forged.event_id = "forged".into();
        forged.event_sha256 = hash_event(&forged).unwrap();
        events.push(forged);
        assert_eq!(
            TaskGraph::replay(&events).unwrap_err(),
            TaskGraphError::ThreadAlreadyExists(thread_id)
        );
    }
}
