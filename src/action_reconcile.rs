//! Read-only reconciliation between a run journal and the action log.
//!
//! A gated tool call writes an `action_id` and policy metadata into the
//! journal observation and an `ActionRecord` into the action log. Both sides
//! are written from the same decision, so a mismatch means the audit trail is
//! incomplete. This module reports those gaps; it never rewrites either side.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::action_log::ActionRecord;
use crate::journal::{JournalEvent, JournalEventKind};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionReconcileIssueKind {
    MissingActionRecord,
    MissingJournalObservation,
    ToolMismatch,
    StatusMismatch,
    PolicyMismatch,
    ArtifactMismatch,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionReconcileIssue {
    pub action_id: String,
    pub kind: ActionReconcileIssueKind,
    pub detail: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionReconciliation {
    pub ok: bool,
    pub checked: usize,
    pub issues: Vec<ActionReconcileIssue>,
}

/// Compare the gated observations of one run journal with its action records.
/// Records from other runs are ignored; observations without an `action_id`
/// (non-gated calls) are ignored.
pub fn reconcile_actions(
    events: &[JournalEvent],
    actions: &[ActionRecord],
) -> ActionReconciliation {
    let Some(run_id) = events.first().map(|event| event.run_id) else {
        return ActionReconciliation {
            ok: true,
            checked: 0,
            issues: Vec::new(),
        };
    };
    let run_id = run_id.to_string();
    let mut journal_actions: BTreeMap<String, &crate::domain::ToolObservation> = BTreeMap::new();
    for event in events {
        if let JournalEventKind::ToolObserved { observation } = &event.event
            && let Some(action_id) = observation.metadata.get("action_id")
        {
            journal_actions.insert(action_id.clone(), observation);
        }
    }
    let mut latest: BTreeMap<String, &ActionRecord> = BTreeMap::new();
    for record in actions {
        if record.run_id == run_id {
            latest.insert(record.action_id.clone(), record);
        }
    }

    let mut issues = Vec::new();
    let mut checked = 0;
    for (action_id, observation) in &journal_actions {
        checked += 1;
        let Some(record) = latest.get(action_id) else {
            issues.push(issue(
                action_id,
                ActionReconcileIssueKind::MissingActionRecord,
                "journal has a gated observation but the action log has no record",
            ));
            continue;
        };
        if record.action_type != observation.tool {
            issues.push(issue(
                action_id,
                ActionReconcileIssueKind::ToolMismatch,
                format!(
                    "journal tool {} does not match action type {}",
                    observation.tool, record.action_type
                ),
            ));
        }
        let expected = crate::runtime::action_status_for(observation);
        if record.status != expected {
            issues.push(issue(
                action_id,
                ActionReconcileIssueKind::StatusMismatch,
                format!(
                    "journal implies {} but the action log says {}",
                    expected.as_str(),
                    record.status.as_str()
                ),
            ));
        }
        if let Some(policy) = observation.metadata.get("policy_outcome")
            && policy != crate::runtime::policy_outcome_str(record.policy_outcome)
        {
            issues.push(issue(
                action_id,
                ActionReconcileIssueKind::PolicyMismatch,
                format!(
                    "journal policy {policy} does not match action log policy {}",
                    crate::runtime::policy_outcome_str(record.policy_outcome)
                ),
            ));
        }
        let journal_sha = observation.metadata.get("sha256");
        if journal_sha != record.artifact_sha256.as_ref()
            && (journal_sha.is_some() || record.artifact_sha256.is_some())
        {
            issues.push(issue(
                action_id,
                ActionReconcileIssueKind::ArtifactMismatch,
                "journal artifact SHA does not match the action record".to_string(),
            ));
        }
    }
    for action_id in latest.keys() {
        if !journal_actions.contains_key(action_id) {
            checked += 1;
            issues.push(issue(
                action_id,
                ActionReconcileIssueKind::MissingJournalObservation,
                "action log has a record but the journal has no gated observation",
            ));
        }
    }
    ActionReconciliation {
        ok: issues.is_empty(),
        checked,
        issues,
    }
}

fn issue(
    action_id: &str,
    kind: ActionReconcileIssueKind,
    detail: impl Into<String>,
) -> ActionReconcileIssue {
    ActionReconcileIssue {
        action_id: action_id.into(),
        kind,
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action_log::ActionStatus;
    use crate::authority::PolicyOutcome;
    use crate::domain::{RiskLevel, ToolObservation, ToolStatus};
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn observation(
        status: ToolStatus,
        error_code: Option<&str>,
        action_id: &str,
        policy: &str,
        sha: Option<&str>,
    ) -> ToolObservation {
        let mut metadata = BTreeMap::from([
            ("action_id".into(), action_id.into()),
            ("policy_outcome".into(), policy.into()),
        ]);
        if let Some(sha) = sha {
            metadata.insert("sha256".into(), sha.into());
        }
        ToolObservation {
            call_id: "call-1".into(),
            tool: "write_file".into(),
            status,
            summary: "test".into(),
            content: String::new(),
            error_code: error_code.map(str::to_string),
            metadata,
        }
    }

    fn event(run_id: Uuid, seq: u64, observation: ToolObservation) -> JournalEvent {
        JournalEvent {
            schema_version: crate::domain::PROTOCOL_VERSION,
            run_id,
            seq,
            timestamp_unix_ms: u128::from(seq),
            event: JournalEventKind::ToolObserved { observation },
        }
    }

    fn record(
        run_id: &str,
        status: ActionStatus,
        policy: PolicyOutcome,
        sha: Option<&str>,
    ) -> ActionRecord {
        ActionRecord {
            action_id: format!("{run_id}:call-1"),
            run_id: run_id.into(),
            session_id: Some("ses-1".into()),
            action_type: "write_file".into(),
            risk: RiskLevel::Medium,
            reversible: false,
            policy_outcome: policy,
            policy_reason: Some("test".into()),
            authority_profile_id: Some("profile-1".into()),
            approval_card_id: None,
            status,
            artifact_sha256: sha.map(str::to_string),
        }
    }

    #[test]
    fn matching_journal_and_action_records_reconcile() {
        let run_id = Uuid::new_v4();
        let sha = "a".repeat(64);
        let events = vec![event(
            run_id,
            0,
            observation(
                ToolStatus::Ok,
                None,
                &format!("{run_id}:call-1"),
                "allow",
                Some(&sha),
            ),
        )];
        let actions = vec![record(
            &run_id.to_string(),
            ActionStatus::Executed,
            PolicyOutcome::Allow,
            Some(&sha),
        )];
        let report = reconcile_actions(&events, &actions);
        assert!(report.ok, "{report:?}");
        assert_eq!(report.checked, 1);
    }

    #[test]
    fn a_missing_action_record_is_reported() {
        let run_id = Uuid::new_v4();
        let events = vec![event(
            run_id,
            0,
            observation(
                ToolStatus::Ok,
                None,
                &format!("{run_id}:call-1"),
                "allow",
                None,
            ),
        )];
        let report = reconcile_actions(&events, &[]);
        assert!(!report.ok);
        assert_eq!(report.issues.len(), 1);
        assert_eq!(
            report.issues[0].kind,
            ActionReconcileIssueKind::MissingActionRecord
        );
    }

    #[test]
    fn a_missing_journal_observation_is_reported() {
        let run_id = Uuid::new_v4();
        let actions = vec![record(
            &run_id.to_string(),
            ActionStatus::Executed,
            PolicyOutcome::Allow,
            None,
        )];
        let report = reconcile_actions(&[], &actions);
        assert!(report.ok, "an empty journal has no run to attribute");

        let events = vec![event(
            run_id,
            0,
            observation(
                ToolStatus::Ok,
                None,
                &format!("{run_id}:call-other"),
                "allow",
                None,
            ),
        )];
        let report = reconcile_actions(&events, &actions);
        assert!(!report.ok);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.kind == ActionReconcileIssueKind::MissingJournalObservation)
        );
    }

    #[test]
    fn status_policy_and_artifact_mismatches_are_reported() {
        let run_id = Uuid::new_v4();
        let events = vec![event(
            run_id,
            0,
            observation(
                ToolStatus::Ok,
                None,
                &format!("{run_id}:call-1"),
                "allow",
                Some("b"),
            ),
        )];
        let actions = vec![record(
            &run_id.to_string(),
            ActionStatus::Denied,
            PolicyOutcome::Deny,
            None,
        )];
        let report = reconcile_actions(&events, &actions);
        assert!(!report.ok);
        let kinds = report
            .issues
            .iter()
            .map(|issue| issue.kind)
            .collect::<Vec<_>>();
        assert!(kinds.contains(&ActionReconcileIssueKind::StatusMismatch));
        assert!(kinds.contains(&ActionReconcileIssueKind::PolicyMismatch));
        assert!(kinds.contains(&ActionReconcileIssueKind::ArtifactMismatch));
    }

    #[test]
    fn records_from_other_runs_are_ignored() {
        let run_id = Uuid::new_v4();
        let other = Uuid::new_v4();
        let events = vec![event(
            run_id,
            0,
            observation(
                ToolStatus::Ok,
                None,
                &format!("{run_id}:call-1"),
                "allow",
                None,
            ),
        )];
        let actions = vec![
            record(
                &run_id.to_string(),
                ActionStatus::Executed,
                PolicyOutcome::Allow,
                None,
            ),
            record(
                &other.to_string(),
                ActionStatus::Denied,
                PolicyOutcome::Deny,
                None,
            ),
        ];
        let report = reconcile_actions(&events, &actions);
        assert!(report.ok, "{report:?}");
        assert_eq!(report.checked, 1);
    }

    #[test]
    fn non_gated_journals_reconcile_without_actions() {
        let run_id = Uuid::new_v4();
        let mut observation = observation(ToolStatus::Ok, None, "unused", "allow", None);
        observation.metadata.clear();
        let events = vec![event(run_id, 0, observation)];
        let report = reconcile_actions(&events, &[]);
        assert!(report.ok);
        assert_eq!(report.checked, 0);
    }
}
