//! Escalation ladder for unresolved attention items.
//!
//! The ladder is a pure policy: the caller supplies the item age and the
//! ordered steps. It does not send messages or change an item's status.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EscalationError {
    #[error("an escalation ladder needs at least one step")]
    Empty,
    #[error("escalation step thresholds must be strictly increasing")]
    NotIncreasing,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum EscalationAction {
    Inbox,
    Digest,
    Notify,
    Urgent,
}

impl EscalationAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inbox => "inbox",
            Self::Digest => "digest",
            Self::Notify => "notify",
            Self::Urgent => "urgent",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "inbox" => Some(Self::Inbox),
            "digest" => Some(Self::Digest),
            "notify" => Some(Self::Notify),
            "urgent" => Some(Self::Urgent),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EscalationStep {
    pub after_ms: u128,
    pub action: EscalationAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EscalationLadder {
    pub steps: Vec<EscalationStep>,
}

impl EscalationLadder {
    pub fn new(steps: Vec<EscalationStep>) -> Result<Self, EscalationError> {
        if steps.is_empty() {
            return Err(EscalationError::Empty);
        }
        if steps
            .windows(2)
            .any(|pair| pair[0].after_ms >= pair[1].after_ms)
        {
            return Err(EscalationError::NotIncreasing);
        }
        Ok(Self { steps })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EscalationPlan {
    pub level: Option<EscalationAction>,
    pub elapsed_ms: u128,
    pub reason: String,
}

pub fn plan(
    ladder: &EscalationLadder,
    created_ms: u128,
    now_ms: u128,
    resolved: bool,
) -> EscalationPlan {
    let elapsed_ms = now_ms.saturating_sub(created_ms);
    if resolved {
        return EscalationPlan {
            level: None,
            elapsed_ms,
            reason: "item is resolved".into(),
        };
    }
    let Some(step) = ladder
        .steps
        .iter()
        .rev()
        .find(|step| step.after_ms <= elapsed_ms)
    else {
        return EscalationPlan {
            level: None,
            elapsed_ms,
            reason: "no escalation threshold reached".into(),
        };
    };
    EscalationPlan {
        level: Some(step.action),
        elapsed_ms,
        reason: format!(
            "threshold {} ms reached; action {}",
            step.after_ms,
            step.action.as_str()
        ),
    }
}

#[derive(Debug, Error)]
pub enum EscalationStateError {
    #[error("escalation state io: {0}")]
    Io(#[from] std::io::Error),
    #[error("escalation state json: {0}")]
    Json(#[from] serde_json::Error),
}

/// Highest delivered rung per inbox item. A missing item has never been
/// delivered; a resolved item may be pruned by the caller.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EscalationState {
    #[serde(default)]
    pub delivered: BTreeMap<String, EscalationAction>,
    /// Items the operator acknowledged; they stay visible but stop escalating.
    #[serde(default)]
    pub acknowledged: BTreeSet<String>,
}

impl EscalationState {
    pub fn acknowledge(&mut self, item_id: &str) -> bool {
        self.acknowledged.insert(item_id.into())
    }

    pub fn is_acknowledged(&self, item_id: &str) -> bool {
        self.acknowledged.contains(item_id)
    }

    pub fn open(path: impl Into<PathBuf>) -> Result<Self, EscalationStateError> {
        match std::fs::read(path.into()) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.into()),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), EscalationStateError> {
        use std::io::Write;
        let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
        let bytes = serde_json::to_vec_pretty(self)?;
        {
            let mut file = std::fs::File::create(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        std::fs::rename(&temporary, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ladder() -> EscalationLadder {
        EscalationLadder::new(vec![
            EscalationStep {
                after_ms: 0,
                action: EscalationAction::Inbox,
            },
            EscalationStep {
                after_ms: 15 * 60 * 1_000,
                action: EscalationAction::Notify,
            },
            EscalationStep {
                after_ms: 60 * 60 * 1_000,
                action: EscalationAction::Urgent,
            },
        ])
        .unwrap()
    }

    #[test]
    fn picks_the_highest_reached_step() {
        assert_eq!(
            plan(&ladder(), 1_000, 1_000, false).level,
            Some(EscalationAction::Inbox)
        );
        assert_eq!(
            plan(&ladder(), 1_000, 1_000 + 15 * 60 * 1_000, false).level,
            Some(EscalationAction::Notify)
        );
        assert_eq!(
            plan(&ladder(), 1_000, 1_000 + 60 * 60 * 1_000, false).level,
            Some(EscalationAction::Urgent)
        );
    }

    #[test]
    fn resolved_items_do_not_escalate() {
        let plan = plan(&ladder(), 1_000, 60 * 60 * 1_000, true);
        assert_eq!(plan.level, None);
        assert!(plan.reason.contains("resolved"));
    }

    #[test]
    fn a_grace_period_keeps_the_item_quiet() {
        let ladder = EscalationLadder::new(vec![EscalationStep {
            after_ms: 5_000,
            action: EscalationAction::Notify,
        }])
        .unwrap();
        assert_eq!(plan(&ladder, 1_000, 2_000, false).level, None);
        assert_eq!(
            plan(&ladder, 1_000, 6_000, false).level,
            Some(EscalationAction::Notify)
        );
    }

    #[test]
    fn acknowledgement_round_trips_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("escalation.json");
        let mut state = EscalationState::default();
        assert!(state.acknowledge("i1"));
        assert!(!state.acknowledge("i1"));
        state.save(&path).unwrap();
        let reopened = EscalationState::open(&path).unwrap();
        assert!(reopened.is_acknowledged("i1"));
        assert!(!reopened.is_acknowledged("i2"));
    }

    #[test]
    fn invalid_ladders_are_rejected() {
        assert_eq!(EscalationLadder::new(vec![]), Err(EscalationError::Empty));
        assert_eq!(
            EscalationLadder::new(vec![
                EscalationStep {
                    after_ms: 10,
                    action: EscalationAction::Inbox,
                },
                EscalationStep {
                    after_ms: 10,
                    action: EscalationAction::Notify,
                },
            ]),
            Err(EscalationError::NotIncreasing)
        );
    }
}
