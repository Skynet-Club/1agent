//! Project inbox items through the escalation ladder and optionally deliver
//! newly reached `notify`/`urgent` rungs to a webhook.
//!
//! Projection is read-only. Delivery advances a per-item watermark only after
//! a successful send, so a crash in between can duplicate one batch.

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::escalation::{
    self, EscalationAction, EscalationLadder, EscalationState, EscalationStateError,
};
use crate::inbox::InboxItem;
use crate::notify::{InboxNotifier, NotifyError};

const MAX_BATCH: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EscalationItem {
    pub item_id: String,
    pub kind: String,
    pub status: String,
    pub title: String,
    pub created_ms: u128,
    pub elapsed_ms: u128,
    pub level: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EscalationNotification {
    pub sent: bool,
    pub notified: usize,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EscalationReport {
    pub items: Vec<EscalationItem>,
    pub notification: Option<EscalationNotification>,
}

#[derive(Debug, Error)]
pub enum EscalationRunError {
    #[error(transparent)]
    State(#[from] EscalationStateError),
    #[error(transparent)]
    Notify(#[from] NotifyError),
    #[error("escalation delivery needs both a webhook and a state path")]
    MissingState,
}

/// Computes levels for `items`; when notifiers and a state path are supplied,
/// also delivers items whose level is newly `notify` or `urgent`. Delivery
/// runs in target order; a later failure leaves the watermark unadvanced, so a
/// retry can duplicate one batch to earlier targets.
pub async fn run(
    items: &[InboxItem],
    ladder: &EscalationLadder,
    now_ms: u128,
    notifiers: &[InboxNotifier],
    state_path: Option<&Path>,
    dry_run: bool,
) -> Result<EscalationReport, EscalationRunError> {
    if !notifiers.is_empty() != state_path.is_some() {
        return Err(EscalationRunError::MissingState);
    }
    let mut state = state_path.map(EscalationState::open).transpose()?;
    let rows = items
        .iter()
        .map(|item| {
            let acknowledged = state
                .as_ref()
                .is_some_and(|state| state.is_acknowledged(&item.item_id));
            let plan = if acknowledged {
                escalation::EscalationPlan {
                    level: None,
                    elapsed_ms: now_ms.saturating_sub(item.created_ms),
                    reason: "item is acknowledged".into(),
                }
            } else {
                escalation::plan(ladder, item.created_ms, now_ms, false)
            };
            (item, plan)
        })
        .collect::<Vec<_>>();

    let mut to_notify = Vec::new();
    if !notifiers.is_empty() {
        let delivered = state.as_ref().map(|state| &state.delivered);
        to_notify = rows
            .iter()
            .filter(|(item, plan)| {
                let Some(level) = plan.level else {
                    return false;
                };
                level >= EscalationAction::Notify
                    && delivered.is_some_and(|delivered| {
                        delivered
                            .get(&item.item_id)
                            .is_none_or(|sent| *sent < level)
                    })
            })
            .take(MAX_BATCH)
            .collect();
    }

    let mut sent = false;
    if !notifiers.is_empty() && !to_notify.is_empty() && !dry_run {
        let mut text = format!("1Agent Escalation: {} item(s)\n", to_notify.len());
        for (item, plan) in &to_notify {
            text.push_str(&format!(
                "[{}] {} ({})\n",
                plan.level.map_or("none", |level| level.as_str()),
                item.title,
                item.item_id
            ));
        }
        for notifier in notifiers {
            notifier.send_text(text.trim_end()).await?;
        }
        sent = true;
    }
    if let (Some(state), Some(path)) = (state.as_mut(), state_path)
        && !dry_run
    {
        let open_items = rows
            .iter()
            .map(|(item, _)| item.item_id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let before = state.delivered.len();
        state
            .delivered
            .retain(|item_id, _| open_items.contains(item_id.as_str()));
        let pruned = state.delivered.len() != before;
        if sent {
            for (item, plan) in &to_notify {
                if let Some(level) = plan.level {
                    state.delivered.insert(item.item_id.clone(), level);
                }
            }
        }
        if sent || pruned {
            state.save(path)?;
        }
    }

    let items = rows
        .iter()
        .map(|(item, plan)| EscalationItem {
            item_id: item.item_id.clone(),
            kind: item.kind.as_str().into(),
            status: item.status.as_str().into(),
            title: item.title.clone(),
            created_ms: item.created_ms,
            elapsed_ms: plan.elapsed_ms,
            level: plan.level.map(|level| level.as_str().into()),
            reason: plan.reason.clone(),
        })
        .collect();
    Ok(EscalationReport {
        items,
        notification: (!notifiers.is_empty()).then_some(EscalationNotification {
            sent,
            notified: to_notify.len(),
            dry_run,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::escalation::{EscalationAction, EscalationLadder, EscalationState, EscalationStep};
    use crate::inbox::{InboxItem, InboxKind};
    use crate::notify::InboxNotifier;

    #[tokio::test]
    async fn acknowledged_items_stay_visible_but_do_not_escalate() {
        let dir = tempfile::tempdir().unwrap();
        let state_path = dir.path().join("escalation.json");
        let mut state = EscalationState::default();
        state.acknowledge("i1");
        state.save(&state_path).unwrap();
        let item = InboxItem::new("i1", InboxKind::Notify, "title", "why", "test", 1_000);
        let ladder = EscalationLadder::new(vec![
            EscalationStep {
                after_ms: 0,
                action: EscalationAction::Inbox,
            },
            EscalationStep {
                after_ms: 60_000,
                action: EscalationAction::Notify,
            },
        ])
        .unwrap();
        let notifier = InboxNotifier::new("http://127.0.0.1:9/").unwrap();
        let report = run(
            &[item],
            &ladder,
            100_000,
            &[notifier],
            Some(&state_path),
            false,
        )
        .await
        .unwrap();
        assert_eq!(report.items.len(), 1);
        assert_eq!(report.items[0].level, None);
        assert!(report.items[0].reason.contains("acknowledged"));
    }
}
