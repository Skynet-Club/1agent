//! Deterministic attention candidates from goals and commitments.
//!
//! Attention is a read-only projection: it does not mutate goals, send
//! messages or create work. A caller decides which candidates to surface and
//! records that decision elsewhere.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::goal::{CommitmentStatus, GoalGraph, GoalStatus};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AttentionKind {
    CommitmentOverdue,
    CommitmentDue,
    CommitmentMissed,
    GoalDue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AttentionCandidate {
    pub kind: AttentionKind,
    pub id: String,
    pub goal_id: String,
    pub title: String,
    pub due_ms: Option<u128>,
    pub urgency: u8,
    pub why_now: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AttentionPolicy {
    pub lead_ms: u128,
    pub max_candidates: usize,
    pub include_missed: bool,
}

impl Default for AttentionPolicy {
    fn default() -> Self {
        Self {
            lead_ms: 24 * 60 * 60 * 1_000,
            max_candidates: 10,
            include_missed: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct AttentionEntry {
    last_surfaced_ms: Option<u128>,
    snoozed_until_ms: Option<u128>,
}

/// In-memory delivery state for dedupe/silence. Persistence is the caller's
/// responsibility; without a state the projection remains stateless.
#[derive(Debug, Default)]
pub struct AttentionState {
    entries: BTreeMap<String, AttentionEntry>,
    min_interval_ms: u128,
}

impl AttentionState {
    pub fn new(min_interval_ms: u128) -> Self {
        Self {
            entries: BTreeMap::new(),
            min_interval_ms,
        }
    }

    pub fn min_interval_ms(&self) -> u128 {
        self.min_interval_ms
    }

    pub fn snooze(&mut self, candidate_id: &str, until_ms: u128) {
        self.entries
            .entry(candidate_id.into())
            .or_default()
            .snoozed_until_ms = Some(until_ms);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Filter candidates already surfaced inside `min_interval_ms` or snoozed.
    /// Returned candidates are marked as surfaced at `now_ms`.
    pub fn filter_new(
        &mut self,
        candidates: Vec<AttentionCandidate>,
        now_ms: u128,
        min_interval_ms: u128,
    ) -> Vec<AttentionCandidate> {
        let mut delivered = Vec::new();
        for candidate in candidates {
            let entry = self.entries.entry(candidate.id.clone()).or_default();
            if entry.snoozed_until_ms.is_some_and(|until| until > now_ms) {
                continue;
            }
            if entry
                .last_surfaced_ms
                .is_some_and(|last| now_ms.saturating_sub(last) < min_interval_ms)
            {
                continue;
            }
            entry.last_surfaced_ms = Some(now_ms);
            delivered.push(candidate);
        }
        delivered
    }
}

pub fn attention_candidates(
    graph: &GoalGraph,
    now_ms: u128,
    policy: &AttentionPolicy,
) -> Vec<AttentionCandidate> {
    if policy.max_candidates == 0 {
        return Vec::new();
    }
    let snapshot = graph.snapshot();
    let mut candidates = Vec::new();
    for commitment in &snapshot.commitments {
        let Some(goal) = graph.goal(&commitment.goal_id) else {
            continue;
        };
        if goal.status != GoalStatus::Active {
            continue;
        }
        match commitment.status {
            CommitmentStatus::Pending => {
                let Some(due_ms) = commitment.due_ms else {
                    continue;
                };
                if due_ms <= now_ms {
                    candidates.push(candidate(
                        AttentionKind::CommitmentOverdue,
                        &commitment.commitment_id,
                        &commitment.goal_id,
                        &commitment.description,
                        Some(due_ms),
                        100,
                        format!("overdue by {} ms", now_ms.saturating_sub(due_ms)),
                    ));
                } else if due_ms.saturating_sub(now_ms) <= policy.lead_ms {
                    candidates.push(candidate(
                        AttentionKind::CommitmentDue,
                        &commitment.commitment_id,
                        &commitment.goal_id,
                        &commitment.description,
                        Some(due_ms),
                        70,
                        format!("due in {} ms", due_ms.saturating_sub(now_ms)),
                    ));
                }
            }
            CommitmentStatus::Missed if policy.include_missed => {
                candidates.push(candidate(
                    AttentionKind::CommitmentMissed,
                    &commitment.commitment_id,
                    &commitment.goal_id,
                    &commitment.description,
                    commitment.due_ms,
                    90,
                    "commitment was missed".into(),
                ));
            }
            _ => {}
        }
    }
    for goal in &snapshot.goals {
        if goal.status != GoalStatus::Active {
            continue;
        }
        let Some(due_ms) = goal.due_ms else {
            continue;
        };
        if due_ms <= now_ms {
            candidates.push(candidate(
                AttentionKind::GoalDue,
                &goal.goal_id,
                &goal.goal_id,
                &goal.title,
                Some(due_ms),
                80,
                format!("goal overdue by {} ms", now_ms.saturating_sub(due_ms)),
            ));
        } else if due_ms.saturating_sub(now_ms) <= policy.lead_ms {
            candidates.push(candidate(
                AttentionKind::GoalDue,
                &goal.goal_id,
                &goal.goal_id,
                &goal.title,
                Some(due_ms),
                50,
                format!("goal due in {} ms", due_ms.saturating_sub(now_ms)),
            ));
        }
    }
    candidates.sort_by(|left, right| {
        right
            .urgency
            .cmp(&left.urgency)
            .then_with(|| left.due_ms.cmp(&right.due_ms))
            .then_with(|| left.id.cmp(&right.id))
    });
    candidates.truncate(policy.max_candidates);
    candidates
}

fn candidate(
    kind: AttentionKind,
    id: &str,
    goal_id: &str,
    title: &str,
    due_ms: Option<u128>,
    urgency: u8,
    why_now: String,
) -> AttentionCandidate {
    AttentionCandidate {
        kind,
        id: id.into(),
        goal_id: goal_id.into(),
        title: title.into(),
        due_ms,
        urgency,
        why_now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph() -> GoalGraph {
        let mut graph = GoalGraph::default();
        graph
            .create_goal("goal-1", "owner-1", "ship", Some(10_000), 1)
            .unwrap();
        graph
            .add_commitment("commit-due", "goal-1", "due soon", Some(1_100), 2)
            .unwrap();
        graph
            .add_commitment("commit-overdue", "goal-1", "overdue", Some(900), 3)
            .unwrap();
        graph
            .add_commitment("commit-missed", "goal-1", "missed", Some(500), 4)
            .unwrap();
        graph
            .close_commitment("commit-missed", CommitmentStatus::Missed, "missed", 5)
            .unwrap();
        graph
            .add_commitment("commit-far", "goal-1", "far away", Some(9_000), 6)
            .unwrap();
        graph
    }

    #[test]
    fn overdue_due_and_missed_are_ordered_by_urgency() {
        let graph = graph();
        let candidates = attention_candidates(&graph, 1_000, &AttentionPolicy::default());
        let ids: Vec<&str> = candidates.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "commit-overdue",
                "commit-missed",
                "commit-due",
                "commit-far",
                "goal-1"
            ]
        );
        assert_eq!(candidates[0].kind, AttentionKind::CommitmentOverdue);
        assert!(candidates[0].why_now.contains("overdue by 100 ms"));
        assert_eq!(candidates[2].kind, AttentionKind::CommitmentDue);
        assert!(candidates[2].why_now.contains("due in 100 ms"));
        assert_eq!(candidates[4].kind, AttentionKind::GoalDue);
    }

    #[test]
    fn lead_window_and_cap_are_enforced() {
        let graph = graph();
        let policy = AttentionPolicy {
            lead_ms: 50,
            max_candidates: 1,
            include_missed: false,
        };
        let candidates = attention_candidates(&graph, 1_000, &policy);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].kind, AttentionKind::CommitmentOverdue);

        let empty = AttentionPolicy {
            max_candidates: 0,
            ..policy
        };
        assert!(attention_candidates(&graph, 1_000, &empty).is_empty());
    }

    #[test]
    fn delivery_state_dedupes_and_honors_snooze() {
        let graph = graph();
        let candidates = attention_candidates(&graph, 1_000, &AttentionPolicy::default());
        let mut state = AttentionState::default();
        let first = state.filter_new(candidates.clone(), 1_000, 100);
        assert_eq!(first.len(), candidates.len());
        assert!(state.filter_new(candidates.clone(), 1_050, 100).is_empty());
        assert!(!state.filter_new(candidates.clone(), 1_100, 100).is_empty());

        state.snooze("commit-overdue", 2_000);
        let after_snooze = state.filter_new(candidates.clone(), 1_500, 0);
        assert!(
            after_snooze
                .iter()
                .all(|candidate| candidate.id != "commit-overdue")
        );
        let after_expiry = state.filter_new(candidates, 2_000, 0);
        assert!(
            after_expiry
                .iter()
                .any(|candidate| candidate.id == "commit-overdue")
        );
    }

    #[test]
    fn inactive_goals_do_not_produce_candidates() {
        let mut graph = graph();
        graph
            .set_goal_status("goal-1", GoalStatus::Paused, "pause", 2_000)
            .unwrap();
        assert!(attention_candidates(&graph, 2_100, &AttentionPolicy::default()).is_empty());
    }
}
