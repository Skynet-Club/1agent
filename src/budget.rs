//! Cost and usage budget policy.
//!
//! A budget is a hard stop, not a suggestion: exceeding any configured limit
//! fails the check. Reaching 80% of a limit is a warning, not a stop.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    #[serde(default)]
    pub max_cost_micros: Option<u64>,
    #[serde(default)]
    pub max_tokens: Option<u64>,
    #[serde(default)]
    pub max_tool_calls: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Usage {
    pub cost_micros: u64,
    pub tokens: u64,
    pub tool_calls: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BudgetStatus {
    Ok,
    NearLimit,
    Exceeded,
}

impl BudgetStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::NearLimit => "near_limit",
            Self::Exceeded => "exceeded",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BudgetVerdict {
    pub status: BudgetStatus,
    pub allowed: bool,
    pub failures: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn evaluate(usage: &Usage, budget: &Budget) -> BudgetVerdict {
    let mut failures = Vec::new();
    let mut warnings = Vec::new();
    check(
        "cost_micros",
        usage.cost_micros,
        budget.max_cost_micros,
        &mut failures,
        &mut warnings,
    );
    check(
        "tokens",
        usage.tokens,
        budget.max_tokens,
        &mut failures,
        &mut warnings,
    );
    check(
        "tool_calls",
        usage.tool_calls,
        budget.max_tool_calls,
        &mut failures,
        &mut warnings,
    );
    let status = if !failures.is_empty() {
        BudgetStatus::Exceeded
    } else if !warnings.is_empty() {
        BudgetStatus::NearLimit
    } else {
        BudgetStatus::Ok
    };
    BudgetVerdict {
        status,
        allowed: failures.is_empty(),
        failures,
        warnings,
    }
}

fn check(
    label: &str,
    observed: u64,
    limit: Option<u64>,
    failures: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    let Some(limit) = limit else {
        return;
    };
    if observed > limit {
        failures.push(format!("{label} {observed} exceeds limit {limit}"));
    } else if limit > 0 && u128::from(observed) * 100 >= u128::from(limit) * 80 {
        warnings.push(format!("{label} {observed} is at or above 80% of {limit}"));
    }
}

pub fn cost_per_verified_outcome(cost_micros: u64, verified_outcomes: u64) -> Option<u64> {
    (verified_outcomes > 0).then(|| cost_micros / verified_outcomes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_limits_are_unbounded() {
        let verdict = evaluate(&Usage::default(), &Budget::default());
        assert_eq!(verdict.status, BudgetStatus::Ok);
        assert!(verdict.allowed);
        assert!(verdict.failures.is_empty());
    }

    #[test]
    fn under_limit_is_ok_and_eighty_percent_warns() {
        let budget = Budget {
            max_cost_micros: Some(1_000),
            max_tokens: Some(1_000),
            max_tool_calls: Some(10),
        };
        let under = evaluate(
            &Usage {
                cost_micros: 500,
                tokens: 500,
                tool_calls: 5,
            },
            &budget,
        );
        assert_eq!(under.status, BudgetStatus::Ok);
        assert!(under.allowed);
        let near = evaluate(
            &Usage {
                cost_micros: 800,
                tokens: 800,
                tool_calls: 8,
            },
            &budget,
        );
        assert_eq!(near.status, BudgetStatus::NearLimit);
        assert!(near.allowed);
        assert_eq!(near.warnings.len(), 3);
    }

    #[test]
    fn exceeding_any_limit_fails_the_check() {
        let budget = Budget {
            max_cost_micros: Some(100),
            max_tokens: None,
            max_tool_calls: None,
        };
        let verdict = evaluate(
            &Usage {
                cost_micros: 101,
                ..Usage::default()
            },
            &budget,
        );
        assert_eq!(verdict.status, BudgetStatus::Exceeded);
        assert!(!verdict.allowed);
        assert_eq!(verdict.failures.len(), 1);
    }

    #[test]
    fn cost_per_verified_outcome_handles_zero() {
        assert_eq!(cost_per_verified_outcome(100, 0), None);
        assert_eq!(cost_per_verified_outcome(100, 4), Some(25));
    }
}
