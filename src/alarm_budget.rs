//! False-alarm budget for proactive alerts.
//!
//! The caller supplies the delivery window counts. Critical alerts bypass the
//! budget; ordinary alerts are moved to a digest once the delivery budget is
//! exhausted or the observed false-alarm rate is too high. This module does
//! not send, persist or acknowledge alerts.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AlarmBudget {
    pub max_delivered: u32,
    /// False-alarm rate ceiling in basis points (10000 = 100%).
    pub max_false_alarm_bps: u32,
    /// Alerts at or above this urgency bypass the budget.
    pub critical_urgency: u8,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AlarmWindow {
    pub delivered: u32,
    pub false_alarms: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlarmDecision {
    /// Deliver immediately.
    Deliver,
    /// Hold for the next digest instead of interrupting.
    Digest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AlarmVerdict {
    pub decision: AlarmDecision,
    pub reason: String,
    pub false_alarm_rate_bps: Option<u32>,
}

pub fn false_alarm_rate_bps(window: AlarmWindow) -> Option<u32> {
    if window.delivered == 0 {
        return None;
    }
    Some(
        (u64::from(window.false_alarms.min(window.delivered)) * 10_000
            / u64::from(window.delivered)) as u32,
    )
}

pub fn evaluate(urgency: u8, window: AlarmWindow, budget: AlarmBudget) -> AlarmVerdict {
    let rate = false_alarm_rate_bps(window);
    if urgency >= budget.critical_urgency {
        return verdict(
            AlarmDecision::Deliver,
            "critical alert bypasses the alarm budget",
            rate,
        );
    }
    if window.delivered >= budget.max_delivered {
        return verdict(
            AlarmDecision::Digest,
            "delivery budget is exhausted; alert moved to the digest",
            rate,
        );
    }
    if let Some(rate) = rate
        && rate > budget.max_false_alarm_bps
    {
        return verdict(
            AlarmDecision::Digest,
            &format!(
                "false-alarm rate {rate} bps exceeds {} bps; alert moved to the digest",
                budget.max_false_alarm_bps
            ),
            Some(rate),
        );
    }
    verdict(AlarmDecision::Deliver, "within the alarm budget", rate)
}

fn verdict(
    decision: AlarmDecision,
    reason: &str,
    false_alarm_rate_bps: Option<u32>,
) -> AlarmVerdict {
    AlarmVerdict {
        decision,
        reason: reason.into(),
        false_alarm_rate_bps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budget() -> AlarmBudget {
        AlarmBudget {
            max_delivered: 3,
            max_false_alarm_bps: 2_000,
            critical_urgency: 90,
        }
    }

    #[test]
    fn routine_alerts_deliver_within_budget() {
        let verdict = evaluate(
            50,
            AlarmWindow {
                delivered: 1,
                false_alarms: 0,
            },
            budget(),
        );
        assert_eq!(verdict.decision, AlarmDecision::Deliver);
        assert_eq!(verdict.false_alarm_rate_bps, Some(0));
    }

    #[test]
    fn exhausted_budget_moves_non_critical_alerts_to_digest() {
        let verdict = evaluate(
            50,
            AlarmWindow {
                delivered: 3,
                false_alarms: 0,
            },
            budget(),
        );
        assert_eq!(verdict.decision, AlarmDecision::Digest);
    }

    #[test]
    fn high_false_alarm_rate_moves_alerts_to_digest() {
        let verdict = evaluate(
            50,
            AlarmWindow {
                delivered: 10,
                false_alarms: 3,
            },
            budget(),
        );
        assert_eq!(verdict.decision, AlarmDecision::Digest);
        assert_eq!(verdict.false_alarm_rate_bps, Some(3_000));
    }

    #[test]
    fn critical_alerts_bypass_budget_and_rate() {
        let verdict = evaluate(
            95,
            AlarmWindow {
                delivered: 100,
                false_alarms: 100,
            },
            budget(),
        );
        assert_eq!(verdict.decision, AlarmDecision::Deliver);
    }

    #[test]
    fn no_deliveries_has_no_rate() {
        assert_eq!(
            false_alarm_rate_bps(AlarmWindow {
                delivered: 0,
                false_alarms: 0,
            }),
            None
        );
        let verdict = evaluate(10, AlarmWindow::default(), budget());
        assert_eq!(verdict.false_alarm_rate_bps, None);
    }
}
