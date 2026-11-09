//! Bounded supervision loop for governed tickets.
//!
//! The supervisor inspects ticket state on each tick, retries failed tickets
//! while the generation budget allows, and escalates stalled or exhausted
//! tickets. It never invents new work and never bypasses `GovernedTaskGraph`:
//! retries go through the same authority checks as a human action.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::governed_task_graph::GovernedTaskGraph;
use crate::task_graph::{Ticket, TicketStatus};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SupervisionError {
    #[error("supervisor actor must not be empty")]
    EmptySupervisor,
    #[error("stall threshold must be greater than zero")]
    InvalidStall,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SupervisionPolicy {
    pub supervisor_actor: String,
    pub stall_after_ms: u128,
    pub max_generation: u32,
    pub retry_failed: bool,
}

impl SupervisionPolicy {
    pub fn validate(&self) -> Result<(), SupervisionError> {
        if self.supervisor_actor.trim().is_empty() {
            return Err(SupervisionError::EmptySupervisor);
        }
        if self.stall_after_ms == 0 {
            return Err(SupervisionError::InvalidStall);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SupervisionAction {
    RetryTicket {
        ticket_id: String,
        generation: u32,
        reason: String,
        at_ms: u128,
    },
    Escalate {
        ticket_id: String,
        status: TicketStatus,
        generation: u32,
        reason: String,
        at_ms: u128,
    },
}

impl SupervisionAction {
    pub fn ticket_id(&self) -> &str {
        match self {
            Self::RetryTicket { ticket_id, .. } | Self::Escalate { ticket_id, .. } => ticket_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SupervisionFailure {
    pub ticket_id: String,
    pub operation: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SupervisionReport {
    pub considered: usize,
    pub actions: Vec<SupervisionAction>,
    pub failures: Vec<SupervisionFailure>,
}

impl SupervisionReport {
    pub fn retries(&self) -> usize {
        self.actions
            .iter()
            .filter(|action| matches!(action, SupervisionAction::RetryTicket { .. }))
            .count()
    }

    pub fn escalations(&self) -> usize {
        self.actions
            .iter()
            .filter(|action| matches!(action, SupervisionAction::Escalate { .. }))
            .count()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EscalationStamp {
    generation: u32,
    status: TicketStatus,
    at_ms: u128,
}

pub struct Supervisor {
    policy: SupervisionPolicy,
    escalations: BTreeMap<String, EscalationStamp>,
}

impl Supervisor {
    pub fn new(policy: SupervisionPolicy) -> Result<Self, SupervisionError> {
        policy.validate()?;
        Ok(Self {
            policy,
            escalations: BTreeMap::new(),
        })
    }

    pub fn policy(&self) -> &SupervisionPolicy {
        &self.policy
    }

    /// Inspect all tickets and apply at most one action per ticket. Repeated
    /// escalations for the same ticket state are suppressed for one stall
    /// window, so a long-running ticket produces a heartbeat rather than a
    /// flood of alerts.
    pub fn tick(
        &mut self,
        graph: &mut GovernedTaskGraph,
        now_ms: u128,
    ) -> Result<SupervisionReport, SupervisionError> {
        self.policy.validate()?;
        let tickets = graph.snapshot().tickets;
        let live: BTreeSet<&str> = tickets
            .iter()
            .filter(|ticket| is_live(ticket.status))
            .map(|ticket| ticket.ticket_id.as_str())
            .collect();
        self.escalations
            .retain(|ticket_id, _| live.contains(ticket_id.as_str()));

        let mut report = SupervisionReport {
            considered: tickets.len(),
            ..SupervisionReport::default()
        };
        for ticket in &tickets {
            if ticket.human_takeover {
                continue;
            }
            if ticket.status == TicketStatus::Failed
                && self.policy.retry_failed
                && ticket.generation < self.policy.max_generation
            {
                match graph.retry_ticket(
                    &ticket.ticket_id,
                    &self.policy.supervisor_actor,
                    format!(
                        "supervisor retry after failure at generation {}",
                        ticket.generation
                    ),
                    now_ms,
                ) {
                    Ok(()) => {
                        self.escalations.remove(&ticket.ticket_id);
                        report.actions.push(SupervisionAction::RetryTicket {
                            ticket_id: ticket.ticket_id.clone(),
                            generation: ticket.generation.saturating_add(1),
                            reason: "failed ticket retried by supervisor".into(),
                            at_ms: now_ms,
                        });
                    }
                    Err(error) => report.failures.push(SupervisionFailure {
                        ticket_id: ticket.ticket_id.clone(),
                        operation: "retry".into(),
                        message: error.to_string(),
                    }),
                }
                continue;
            }

            let reason = self.escalation_reason(ticket, now_ms);
            let Some(reason) = reason else {
                continue;
            };
            let stamp = EscalationStamp {
                generation: ticket.generation,
                status: ticket.status,
                at_ms: now_ms,
            };
            if self
                .escalations
                .get(&ticket.ticket_id)
                .is_some_and(|previous| {
                    previous.generation == stamp.generation
                        && previous.status == stamp.status
                        && now_ms.saturating_sub(previous.at_ms) < self.policy.stall_after_ms
                })
            {
                continue;
            }
            self.escalations.insert(ticket.ticket_id.clone(), stamp);
            report.actions.push(SupervisionAction::Escalate {
                ticket_id: ticket.ticket_id.clone(),
                status: ticket.status,
                generation: ticket.generation,
                reason,
                at_ms: now_ms,
            });
        }
        Ok(report)
    }

    fn escalation_reason(&self, ticket: &Ticket, now_ms: u128) -> Option<String> {
        if ticket.status == TicketStatus::Failed {
            return Some(if self.policy.retry_failed {
                format!(
                    "retry budget exhausted at generation {} (max {})",
                    ticket.generation, self.policy.max_generation
                )
            } else {
                "failed ticket requires attention".into()
            });
        }
        if ticket.status == TicketStatus::Blocked {
            return Some("ticket is blocked and requires attention".into());
        }
        if !stall_eligible(ticket.status) {
            return None;
        }
        let idle_ms = now_ms.saturating_sub(ticket.updated_ms);
        (idle_ms >= self.policy.stall_after_ms).then(|| {
            format!(
                "ticket has been {:?} for {} ms without progress (threshold {} ms)",
                ticket.status, idle_ms, self.policy.stall_after_ms
            )
        })
    }
}

fn stall_eligible(status: TicketStatus) -> bool {
    matches!(
        status,
        TicketStatus::Authorized
            | TicketStatus::Approved
            | TicketStatus::Assigned
            | TicketStatus::Running
            | TicketStatus::WaitingInput
    )
}

fn is_live(status: TicketStatus) -> bool {
    !matches!(
        status,
        TicketStatus::Ready
            | TicketStatus::Dropped
            | TicketStatus::Cancelled
            | TicketStatus::Denied
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authority::AuthorityProfile;
    use crate::domain::RiskLevel;
    use crate::task_graph::{
        ApprovalDecision, Decision, MandateStrength, ReportStatus, TicketDraft, Trigger,
        TriggerSource,
    };

    fn governed(dir: &std::path::Path) -> GovernedTaskGraph {
        let mut service = GovernedTaskGraph::open(dir.join("task-graph.jsonl")).unwrap();
        for actor in ["owner-1", "worker-1"] {
            service
                .register_profile(AuthorityProfile {
                    profile_id: format!("{actor}-profile"),
                    actor_id: actor.into(),
                    scopes: vec!["*".into()],
                    max_strength: MandateStrength::ReadOnly,
                    allowed_tools: Vec::new(),
                    denied_tools: Vec::new(),
                    max_risk: RiskLevel::Medium,
                    expires_ms: None,
                })
                .unwrap();
            service
                .bind_actor(actor, &format!("{actor}-profile"))
                .unwrap();
        }
        service
    }

    fn ticket(service: &mut GovernedTaskGraph, key: &str, requires_approval: bool) -> String {
        let thread_id = service.open_thread(key, 10).unwrap();
        let trigger_id = service
            .ingest_trigger(
                &thread_id,
                Trigger {
                    trigger_id: format!("trigger-{key}"),
                    source: TriggerSource::Web,
                    source_event_id: format!("event-{key}"),
                    summary: key.into(),
                    received_ms: 1,
                },
                11,
            )
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
                    scope: "read supervision".into(),
                    strength: MandateStrength::ReadOnly,
                    policy_version: "v1".into(),
                    requires_approval,
                },
                14,
            )
            .unwrap()
    }

    fn policy(retry_failed: bool, max_generation: u32) -> SupervisionPolicy {
        SupervisionPolicy {
            supervisor_actor: "owner-1".into(),
            stall_after_ms: 1_000,
            max_generation,
            retry_failed,
        }
    }

    fn fail(service: &mut GovernedTaskGraph, ticket_id: &str, at: u128) {
        service
            .report_ticket(
                ticket_id,
                "worker-1",
                ReportStatus::Failed,
                "failed",
                None,
                at,
            )
            .unwrap();
    }

    #[test]
    fn retries_failed_tickets_until_the_budget_is_exhausted() {
        let dir = tempfile::tempdir().unwrap();
        let mut service = governed(dir.path());
        let ticket_id = ticket(&mut service, "retry", true);
        service
            .approve_ticket(&ticket_id, "owner-1", ApprovalDecision::AllowOnce, None, 15)
            .unwrap();
        service
            .assign_ticket(&ticket_id, "owner-1", "worker-1", None, 16)
            .unwrap();
        service.start_ticket(&ticket_id, "worker-1", 17).unwrap();
        fail(&mut service, &ticket_id, 18);

        let mut supervisor = Supervisor::new(policy(true, 2)).unwrap();
        let first = supervisor.tick(&mut service, 19).unwrap();
        assert_eq!(first.retries(), 1);
        assert_eq!(first.escalations(), 0);
        assert_eq!(service.graph().ticket(&ticket_id).unwrap().generation, 1);

        service
            .approve_ticket(&ticket_id, "owner-1", ApprovalDecision::AllowOnce, None, 20)
            .unwrap();
        service
            .assign_ticket(&ticket_id, "owner-1", "worker-1", None, 21)
            .unwrap();
        service.start_ticket(&ticket_id, "worker-1", 22).unwrap();
        fail(&mut service, &ticket_id, 23);
        let second = supervisor.tick(&mut service, 24).unwrap();
        assert_eq!(second.retries(), 1);
        assert_eq!(service.graph().ticket(&ticket_id).unwrap().generation, 2);

        service
            .approve_ticket(&ticket_id, "owner-1", ApprovalDecision::AllowOnce, None, 25)
            .unwrap();
        service
            .assign_ticket(&ticket_id, "owner-1", "worker-1", None, 26)
            .unwrap();
        service.start_ticket(&ticket_id, "worker-1", 27).unwrap();
        fail(&mut service, &ticket_id, 28);
        let exhausted = supervisor.tick(&mut service, 29).unwrap();
        assert_eq!(exhausted.retries(), 0);
        assert_eq!(exhausted.escalations(), 1);
        let again = supervisor.tick(&mut service, 30).unwrap();
        assert_eq!(again.escalations(), 0);
        assert_eq!(service.graph().ticket(&ticket_id).unwrap().generation, 2);
    }

    #[test]
    fn escalates_stalls_once_per_window_and_skips_human_takeover() {
        let dir = tempfile::tempdir().unwrap();
        let mut service = governed(dir.path());
        let stalled = ticket(&mut service, "stalled", true);
        let taken = ticket(&mut service, "taken", true);
        for (ticket_id, at) in [(&stalled, 15), (&taken, 15)] {
            service
                .approve_ticket(ticket_id, "owner-1", ApprovalDecision::AllowOnce, None, at)
                .unwrap();
            service
                .assign_ticket(ticket_id, "owner-1", "worker-1", None, at + 1)
                .unwrap();
            service.start_ticket(ticket_id, "worker-1", at + 2).unwrap();
        }
        service
            .takeover_ticket(&taken, "owner-1", "human owns this", 18)
            .unwrap();

        let mut supervisor = Supervisor::new(policy(false, 0)).unwrap();
        let first = supervisor.tick(&mut service, 1_100).unwrap();
        assert_eq!(first.escalations(), 1);
        assert_eq!(first.actions[0].ticket_id(), stalled);

        let quiet = supervisor.tick(&mut service, 1_500).unwrap();
        assert_eq!(quiet.escalations(), 0);

        let repeated = supervisor.tick(&mut service, 2_200).unwrap();
        assert_eq!(repeated.escalations(), 1);
    }

    #[test]
    fn policy_denial_is_reported_without_mutating_the_ticket() {
        let dir = tempfile::tempdir().unwrap();
        let mut service = governed(dir.path());
        let ticket_id = ticket(&mut service, "denied", true);
        service
            .approve_ticket(&ticket_id, "owner-1", ApprovalDecision::AllowOnce, None, 15)
            .unwrap();
        service
            .assign_ticket(&ticket_id, "owner-1", "worker-1", None, 16)
            .unwrap();
        service.start_ticket(&ticket_id, "worker-1", 17).unwrap();
        fail(&mut service, &ticket_id, 18);

        let mut supervisor = Supervisor::new(SupervisionPolicy {
            supervisor_actor: "attacker".into(),
            stall_after_ms: 1_000,
            max_generation: 2,
            retry_failed: true,
        })
        .unwrap();
        let report = supervisor.tick(&mut service, 19).unwrap();
        assert_eq!(report.retries(), 0);
        assert_eq!(report.failures.len(), 1);
        assert_eq!(service.graph().ticket(&ticket_id).unwrap().generation, 0);
        assert!(
            service
                .actions()
                .query(None, Some(crate::action_log::ActionKind::Retry), None, 10)
                .is_empty()
        );
    }
}
