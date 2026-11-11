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

/// Computes levels for `items`; when a notifier and state path are supplied,
/// also delivers items whose level is newly `notify` or `urgent`.
pub async fn run(
    items: &[InboxItem],
    ladder: &EscalationLadder,
    now_ms: u128,
    notifier: Option<&InboxNotifier>,
    state_path: Option<&Path>,
    dry_run: bool,
) -> Result<EscalationReport, EscalationRunError> {
    if notifier.is_some() != state_path.is_some() {
        return Err(EscalationRunError::MissingState);
    }
    let mut state = state_path.map(EscalationState::open).transpose()?;
    let rows = items
        .iter()
        .map(|item| {
            (
                item,
                escalation::plan(ladder, item.created_ms, now_ms, false),
            )
        })
        .collect::<Vec<_>>();

    let mut to_notify = Vec::new();
    if notifier.is_some() {
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
    if let Some(notifier) = notifier
        && !to_notify.is_empty()
        && !dry_run
    {
        let mut text = format!("1Agent Escalation: {} item(s)\n", to_notify.len());
        for (item, plan) in &to_notify {
            text.push_str(&format!(
                "[{}] {} ({})\n",
                plan.level.map_or("none", |level| level.as_str()),
                item.title,
                item.item_id
            ));
        }
        notifier.send_text(text.trim_end()).await?;
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
        notification: notifier.map(|_| EscalationNotification {
            sent,
            notified: to_notify.len(),
            dry_run,
        }),
    })
}
