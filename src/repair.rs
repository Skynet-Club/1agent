//! User-facing repair plans and receipts for side effects.
//!
//! Trust is not only "the action succeeded". It is "the user can see what
//! happened, and a reversible action can be undone". This module builds a
//! deterministic repair plan from the side-effect ledger; it does not execute
//! the compensation itself.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::task_graph::{SideEffect, SideEffectStatus};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RepairError {
    #[error("side effect id or action is empty")]
    EmptyField,
    #[error("reversible side effect has no compensation: {0}")]
    MissingCompensation(String),
    #[error("away digest window is invalid: since {0} > until {1}")]
    InvalidWindow(u128, u128),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RepairAction {
    /// Nothing to undo: the action was never applied or is already compensated.
    None,
    /// Run the recorded compensation.
    Compensate,
    /// The action is irreversible or failed; a human must decide.
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RepairPlan {
    pub side_effect_id: String,
    pub action: RepairAction,
    pub reason: String,
    pub compensation: Option<String>,
    pub user_receipt: String,
}

pub fn plan_repair(effect: &SideEffect) -> Result<RepairPlan, RepairError> {
    if effect.side_effect_id.trim().is_empty() || effect.action.trim().is_empty() {
        return Err(RepairError::EmptyField);
    }
    let compensation = effect
        .compensation
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let (action, reason) = match effect.status {
        SideEffectStatus::Planned => (RepairAction::None, "action was not applied"),
        SideEffectStatus::Compensated => (RepairAction::None, "action was already compensated"),
        SideEffectStatus::Failed => (
            RepairAction::Manual,
            "action failed; verify no partial effect remains",
        ),
        SideEffectStatus::Applied if effect.reversible => {
            if compensation.is_none() {
                return Err(RepairError::MissingCompensation(
                    effect.side_effect_id.clone(),
                ));
            }
            (
                RepairAction::Compensate,
                "action is reversible; compensation is recorded",
            )
        }
        SideEffectStatus::Applied => (
            RepairAction::Manual,
            "action is irreversible; human review required",
        ),
    };

    let user_receipt = match action {
        RepairAction::None => format!("{}: {reason}", effect.action),
        RepairAction::Compensate => format!(
            "{} on {}: {reason}; undo: {}",
            effect.action,
            effect.target,
            compensation.unwrap_or("no compensation recorded")
        ),
        RepairAction::Manual => format!(
            "{} on {}: {reason}; manual rollback required",
            effect.action, effect.target
        ),
    };

    Ok(RepairPlan {
        side_effect_id: effect.side_effect_id.clone(),
        action,
        reason: reason.into(),
        compensation: compensation.map(str::to_string),
        user_receipt,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TrustReceipt {
    pub side_effect_id: String,
    pub action: String,
    pub target: String,
    pub reversible: bool,
    pub status: SideEffectStatus,
    pub compensation: Option<String>,
    pub idempotency_key: String,
}

impl TrustReceipt {
    pub fn summary(&self) -> String {
        let undo = if self.reversible {
            self.compensation
                .as_deref()
                .unwrap_or("no compensation recorded")
        } else {
            "irreversible"
        };
        format!(
            "{} on {} [{:?}] idempotency={} undo={}",
            self.action, self.target, self.status, self.idempotency_key, undo
        )
    }
}

pub fn receipt(effect: &SideEffect) -> Result<TrustReceipt, RepairError> {
    if effect.side_effect_id.trim().is_empty() || effect.action.trim().is_empty() {
        return Err(RepairError::EmptyField);
    }
    Ok(TrustReceipt {
        side_effect_id: effect.side_effect_id.clone(),
        action: effect.action.clone(),
        target: effect.target.clone(),
        reversible: effect.reversible,
        status: effect.status,
        compensation: effect.compensation.clone(),
        idempotency_key: effect.idempotency_key.clone(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AwayDigest {
    pub since_ms: u128,
    pub until_ms: u128,
    pub applied: Vec<String>,
    pub compensated: Vec<String>,
    pub failed: Vec<String>,
    pub pending: Vec<String>,
    /// Applied irreversible actions and failed actions that need a human.
    pub needs_user: Vec<String>,
    pub receipts: Vec<TrustReceipt>,
}

/// Build the "what happened while you were away" view for a bounded window.
pub fn build_away_digest(
    effects: &[SideEffect],
    since_ms: u128,
    until_ms: u128,
) -> Result<AwayDigest, RepairError> {
    if since_ms > until_ms {
        return Err(RepairError::InvalidWindow(since_ms, until_ms));
    }
    let mut digest = AwayDigest {
        since_ms,
        until_ms,
        applied: Vec::new(),
        compensated: Vec::new(),
        failed: Vec::new(),
        pending: Vec::new(),
        needs_user: Vec::new(),
        receipts: Vec::new(),
    };
    for effect in effects
        .iter()
        .filter(|effect| effect.recorded_ms >= since_ms && effect.recorded_ms <= until_ms)
    {
        let receipt = receipt(effect)?;
        digest.receipts.push(receipt);
        match effect.status {
            SideEffectStatus::Planned => digest.pending.push(effect.side_effect_id.clone()),
            SideEffectStatus::Applied => {
                digest.applied.push(effect.side_effect_id.clone());
                if plan_repair(effect)?.action == RepairAction::Manual {
                    digest.needs_user.push(effect.side_effect_id.clone());
                }
            }
            SideEffectStatus::Compensated => {
                digest.compensated.push(effect.side_effect_id.clone());
            }
            SideEffectStatus::Failed => {
                digest.failed.push(effect.side_effect_id.clone());
                digest.needs_user.push(effect.side_effect_id.clone());
            }
        }
    }
    Ok(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn effect(
        id: &str,
        status: SideEffectStatus,
        reversible: bool,
        compensation: Option<&str>,
        at: u128,
    ) -> SideEffect {
        SideEffect {
            side_effect_id: id.into(),
            ticket_id: "ticket-1".into(),
            action: "write".into(),
            target: "/tmp/result.txt".into(),
            idempotency_key: format!("idem-{id}"),
            reversible,
            compensation: compensation.map(str::to_string),
            status,
            recorded_ms: at,
        }
    }

    #[test]
    fn planned_and_compensated_need_no_repair() {
        let planned = plan_repair(&effect(
            "e1",
            SideEffectStatus::Planned,
            true,
            Some("rm result.txt"),
            10,
        ))
        .unwrap();
        assert_eq!(planned.action, RepairAction::None);
        let compensated = plan_repair(&effect(
            "e2",
            SideEffectStatus::Compensated,
            true,
            Some("rm result.txt"),
            10,
        ))
        .unwrap();
        assert_eq!(compensated.action, RepairAction::None);
    }

    #[test]
    fn applied_reversible_effect_offers_compensation() {
        let plan = plan_repair(&effect(
            "e1",
            SideEffectStatus::Applied,
            true,
            Some("rm result.txt"),
            10,
        ))
        .unwrap();
        assert_eq!(plan.action, RepairAction::Compensate);
        assert_eq!(plan.compensation.as_deref(), Some("rm result.txt"));
        assert!(plan.user_receipt.contains("undo: rm result.txt"));
    }

    #[test]
    fn applied_reversible_without_compensation_fails_closed() {
        let error =
            plan_repair(&effect("e1", SideEffectStatus::Applied, true, None, 10)).unwrap_err();
        assert_eq!(error, RepairError::MissingCompensation("e1".into()));
    }

    #[test]
    fn applied_irreversible_and_failed_require_manual_intervention() {
        let irreversible =
            plan_repair(&effect("e1", SideEffectStatus::Applied, false, None, 10)).unwrap();
        assert_eq!(irreversible.action, RepairAction::Manual);
        assert!(
            irreversible
                .user_receipt
                .contains("manual rollback required")
        );

        let failed = plan_repair(&effect(
            "e2",
            SideEffectStatus::Failed,
            true,
            Some("rm result.txt"),
            10,
        ))
        .unwrap();
        assert_eq!(failed.action, RepairAction::Manual);
    }

    #[test]
    fn away_digest_separates_states_and_surfaces_manual_work() {
        let effects = vec![
            effect("planned", SideEffectStatus::Planned, true, None, 10),
            effect(
                "applied-reversible",
                SideEffectStatus::Applied,
                true,
                Some("rm a"),
                20,
            ),
            effect(
                "applied-irreversible",
                SideEffectStatus::Applied,
                false,
                None,
                30,
            ),
            effect(
                "compensated",
                SideEffectStatus::Compensated,
                true,
                Some("rm b"),
                40,
            ),
            effect("failed", SideEffectStatus::Failed, true, Some("rm c"), 50),
            effect("outside", SideEffectStatus::Applied, false, None, 100),
        ];
        let digest = build_away_digest(&effects, 0, 60).unwrap();
        assert_eq!(digest.pending, vec!["planned"]);
        assert_eq!(
            digest.applied,
            vec!["applied-reversible", "applied-irreversible"]
        );
        assert_eq!(digest.compensated, vec!["compensated"]);
        assert_eq!(digest.failed, vec!["failed"]);
        assert_eq!(digest.needs_user, vec!["applied-irreversible", "failed"]);
        assert_eq!(digest.receipts.len(), 5);
    }

    #[test]
    fn invalid_window_and_empty_fields_fail_closed() {
        assert_eq!(
            build_away_digest(&[], 10, 5).unwrap_err(),
            RepairError::InvalidWindow(10, 5)
        );
        let mut empty = effect("", SideEffectStatus::Planned, true, None, 10);
        empty.action.clear();
        assert_eq!(receipt(&empty).unwrap_err(), RepairError::EmptyField);
    }
}
