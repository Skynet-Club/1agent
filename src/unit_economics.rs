//! Unit economics for subscription, usage and BYOK pricing.
//!
//! The caller supplies revenue and costs; this module does not invent prices.
//! Gross margin uses contribution (revenue minus variable cost), and break-even
//! uses the per-outcome contribution when an outcome count is supplied.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UnitEconomics {
    pub revenue_micros: u64,
    pub variable_cost_micros: u64,
    #[serde(default)]
    pub fixed_cost_micros: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UnitEconomicsReport {
    pub revenue_micros: u64,
    pub variable_cost_micros: u64,
    pub fixed_cost_micros: u64,
    /// Revenue minus variable cost. Negative means each unit loses money.
    pub contribution_micros: i128,
    /// Contribution minus fixed cost.
    pub net_micros: i128,
    /// Contribution margin in basis points; `None` when revenue is zero.
    pub gross_margin_bps: Option<i64>,
    pub outcomes: Option<u64>,
    pub revenue_per_outcome_micros: Option<u64>,
    pub cost_per_outcome_micros: Option<u64>,
    /// Outcomes needed to cover fixed cost at the supplied per-outcome
    /// contribution; `None` when there is no positive per-outcome contribution.
    pub break_even_outcomes: Option<u64>,
}

pub fn evaluate(economics: UnitEconomics, outcomes: Option<u64>) -> UnitEconomicsReport {
    let contribution_micros =
        i128::from(economics.revenue_micros) - i128::from(economics.variable_cost_micros);
    let net_micros = contribution_micros - i128::from(economics.fixed_cost_micros);
    let gross_margin_bps = (economics.revenue_micros > 0)
        .then(|| (contribution_micros * 10_000 / i128::from(economics.revenue_micros)) as i64);
    let outcomes = outcomes.filter(|count| *count > 0);
    let revenue_per_outcome_micros = outcomes.map(|count| economics.revenue_micros / count);
    let cost_per_outcome_micros = outcomes.map(|count| economics.variable_cost_micros / count);
    let break_even_outcomes = outcomes.and_then(|count| {
        let per_outcome_contribution = contribution_micros / i128::from(count);
        if per_outcome_contribution <= 0 {
            return None;
        }
        let fixed = i128::from(economics.fixed_cost_micros);
        let needed = (fixed + per_outcome_contribution - 1) / per_outcome_contribution;
        u64::try_from(needed).ok()
    });
    UnitEconomicsReport {
        revenue_micros: economics.revenue_micros,
        variable_cost_micros: economics.variable_cost_micros,
        fixed_cost_micros: economics.fixed_cost_micros,
        contribution_micros,
        net_micros,
        gross_margin_bps,
        outcomes,
        revenue_per_outcome_micros,
        cost_per_outcome_micros,
        break_even_outcomes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_margin_and_break_even_round_up() {
        let report = evaluate(
            UnitEconomics {
                revenue_micros: 10_000,
                variable_cost_micros: 4_000,
                fixed_cost_micros: 9_000,
            },
            Some(10),
        );
        assert_eq!(report.contribution_micros, 6_000);
        assert_eq!(report.net_micros, -3_000);
        assert_eq!(report.gross_margin_bps, Some(6_000));
        assert_eq!(report.revenue_per_outcome_micros, Some(1_000));
        assert_eq!(report.cost_per_outcome_micros, Some(400));
        assert_eq!(report.break_even_outcomes, Some(15));
    }

    #[test]
    fn negative_contribution_has_no_break_even() {
        let report = evaluate(
            UnitEconomics {
                revenue_micros: 1_000,
                variable_cost_micros: 2_000,
                fixed_cost_micros: 0,
            },
            Some(5),
        );
        assert_eq!(report.contribution_micros, -1_000);
        assert_eq!(report.gross_margin_bps, Some(-10_000));
        assert_eq!(report.break_even_outcomes, None);
    }

    #[test]
    fn zero_revenue_and_zero_outcomes_are_handled() {
        let report = evaluate(UnitEconomics::default(), None);
        assert_eq!(report.gross_margin_bps, None);
        assert_eq!(report.break_even_outcomes, None);
        let zero = evaluate(UnitEconomics::default(), Some(0));
        assert_eq!(zero.outcomes, None);
    }
}
