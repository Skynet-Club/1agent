//! Deterministic value report over run history and the usage ledger.
//!
//! v0 reports outcomes, cost and pending decisions. It does not invent a
//! "hours saved" number: the user can see completed outcomes and the cost per
//! completed outcome, then decide what that is worth.

use serde::{Deserialize, Serialize};

use crate::domain::StopReason;
use crate::journal::RunHistoryEntry;
use crate::usage_ledger::UsageLedger;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ValueReport {
    pub since_ms: u128,
    pub until_ms: u128,
    pub runs: u32,
    pub completed: u32,
    pub needs_authorization: u32,
    pub needs_evidence: u32,
    pub failed: u32,
    pub cancelled: u32,
    pub other: u32,
    pub journals: usize,
    pub tokens: u64,
    pub cost_micros: u64,
    pub cost_per_completed_micros: u64,
    pub tool_calls: u64,
    pub tool_failures: u64,
    pub alerts: u32,
    /// Caller-supplied count of independently verified outcomes.
    pub verified_outcomes: Option<u32>,
    pub cost_per_verified_outcome_micros: Option<u64>,
    /// Open or due-snoozed inbox items. This is a separate queue size; it is
    /// not deduplicated against alerts.
    pub inbox_open: u32,
    /// Authorization requests plus alerts: things the user may need to decide.
    pub pending_decisions: u32,
}

/// `usage` must already be scoped to the same journals as `runs`; the report
/// does not re-read journals or guess a time window for usage.
pub fn build_value_report(
    runs: &[RunHistoryEntry],
    usage: &UsageLedger,
    alerts: u32,
    inbox_open: u32,
    verified_outcomes: Option<u32>,
    since_ms: u128,
    until_ms: u128,
) -> ValueReport {
    let mut report = ValueReport {
        since_ms,
        until_ms,
        runs: 0,
        completed: 0,
        needs_authorization: 0,
        needs_evidence: 0,
        failed: 0,
        cancelled: 0,
        other: 0,
        journals: usage.journals,
        tokens: usage.totals.total_tokens(),
        cost_micros: usage.totals.cost_micros,
        cost_per_completed_micros: 0,
        tool_calls: 0,
        tool_failures: 0,
        alerts,
        verified_outcomes,
        cost_per_verified_outcome_micros: verified_outcomes.and_then(|verified| {
            crate::budget::cost_per_verified_outcome(usage.totals.cost_micros, u64::from(verified))
        }),
        inbox_open,
        pending_decisions: 0,
    };
    for run in runs
        .iter()
        .filter(|run| run.modified_unix_ms >= since_ms && run.modified_unix_ms <= until_ms)
    {
        report.runs = report.runs.saturating_add(1);
        match run.stop_reason {
            Some(StopReason::Completed) => report.completed = report.completed.saturating_add(1),
            Some(StopReason::NeedsAuthorization) => {
                report.needs_authorization = report.needs_authorization.saturating_add(1);
            }
            Some(StopReason::NeedsEvidence) => {
                report.needs_evidence = report.needs_evidence.saturating_add(1);
            }
            Some(StopReason::Cancelled) => report.cancelled = report.cancelled.saturating_add(1),
            Some(
                StopReason::BudgetExhausted
                | StopReason::ProviderError
                | StopReason::ToolFailure
                | StopReason::InvalidOutput
                | StopReason::InternalError,
            ) => report.failed = report.failed.saturating_add(1),
            None => report.other = report.other.saturating_add(1),
        }
    }
    report.tool_calls = usage.tools.iter().map(|tool| tool.calls).sum();
    report.tool_failures = usage.tools.iter().map(|tool| tool.failures).sum();
    report.cost_per_completed_micros = if report.completed == 0 {
        0
    } else {
        report.cost_micros / u64::from(report.completed)
    };
    report.pending_decisions = report.needs_authorization.saturating_add(report.alerts);
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage_ledger::{ToolUsageGroup, UsageTotals};
    use std::path::PathBuf;

    fn run(id: u128, stop: Option<StopReason>) -> RunHistoryEntry {
        RunHistoryEntry {
            run_id: None,
            path: PathBuf::from(format!("run-{id}.jsonl")),
            size_bytes: 10,
            modified_unix_ms: id,
            stop_reason: stop,
            client_request_id: None,
        }
    }

    fn usage() -> UsageLedger {
        UsageLedger {
            journals: 3,
            totals: UsageTotals {
                calls: 2,
                input_tokens: 100,
                output_tokens: 50,
                cost_micros: 900,
                zero_cost_calls: 0,
            },
            providers: Vec::new(),
            tasks: Vec::new(),
            tools: vec![
                ToolUsageGroup {
                    tool: "read_file".into(),
                    calls: 4,
                    failures: 1,
                },
                ToolUsageGroup {
                    tool: "write_file".into(),
                    calls: 2,
                    failures: 0,
                },
            ],
            runs: Vec::new(),
        }
    }

    #[test]
    fn counts_outcomes_cost_and_pending_decisions() {
        let runs = vec![
            run(10, Some(StopReason::Completed)),
            run(20, Some(StopReason::Completed)),
            run(30, Some(StopReason::NeedsAuthorization)),
            run(40, Some(StopReason::NeedsEvidence)),
            run(50, Some(StopReason::ToolFailure)),
            run(60, Some(StopReason::Cancelled)),
            run(70, None),
        ];
        let report = build_value_report(&runs, &usage(), 1, 2, Some(2), 0, 100);
        assert_eq!(report.runs, 7);
        assert_eq!(report.completed, 2);
        assert_eq!(report.needs_authorization, 1);
        assert_eq!(report.needs_evidence, 1);
        assert_eq!(report.failed, 1);
        assert_eq!(report.cancelled, 1);
        assert_eq!(report.other, 1);
        assert_eq!(report.journals, 3);
        assert_eq!(report.tokens, 150);
        assert_eq!(report.cost_micros, 900);
        assert_eq!(report.cost_per_completed_micros, 450);
        assert_eq!(report.verified_outcomes, Some(2));
        assert_eq!(report.cost_per_verified_outcome_micros, Some(450));
        assert_eq!(report.tool_calls, 6);
        assert_eq!(report.tool_failures, 1);
        assert_eq!(report.inbox_open, 2);
        assert_eq!(report.pending_decisions, 2);
    }

    #[test]
    fn window_filters_and_zero_completed_has_zero_unit_cost() {
        let runs = vec![
            run(10, Some(StopReason::Completed)),
            run(200, Some(StopReason::Completed)),
        ];
        let report = build_value_report(&runs, &usage(), 0, 0, None, 100, 300);
        assert_eq!(report.runs, 1);
        assert_eq!(report.completed, 1);
        assert_eq!(report.cost_per_completed_micros, 900);

        let empty = build_value_report(&runs, &usage(), 0, 0, None, 1000, 2000);
        assert_eq!(empty.runs, 0);
        assert_eq!(empty.cost_per_completed_micros, 0);
    }
}
