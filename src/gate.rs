//! Deterministic two-level event gate.
//!
//! Level 1 rejects duplicates and untrusted high-risk input. Level 2 decides
//! between observe-only and automatic handling based on confidence and risk.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::RiskLevel;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GateError {
    #[error("gate confidence must be 0..=10000")]
    InvalidConfidence,
    #[error("gate thresholds must be ordered observe<=auto")]
    InvalidThresholds,
    #[error("gate source event id is empty")]
    EmptySource,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceTrust {
    Trusted,
    Unknown,
    Untrusted,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GateOutcome {
    Ignore,
    Observe,
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GateInput {
    pub source_event_id: String,
    pub trust: SourceTrust,
    pub risk: RiskLevel,
    pub confidence_bps: u16,
    pub explicit_mention: bool,
    #[serde(default)]
    pub duplicate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GatePolicy {
    pub observe_at_bps: u16,
    pub auto_at_bps: u16,
    pub max_auto_risk: RiskLevel,
    pub allow_untrusted_auto: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GateDecision {
    pub outcome: GateOutcome,
    pub reason: String,
}

impl GatePolicy {
    pub fn validate(&self) -> Result<(), GateError> {
        if self.observe_at_bps > 10_000 || self.auto_at_bps > 10_000 {
            return Err(GateError::InvalidConfidence);
        }
        if self.observe_at_bps > self.auto_at_bps {
            return Err(GateError::InvalidThresholds);
        }
        Ok(())
    }
}

pub fn evaluate(input: &GateInput, policy: &GatePolicy) -> Result<GateDecision, GateError> {
    policy.validate()?;
    if input.confidence_bps > 10_000 {
        return Err(GateError::InvalidConfidence);
    }
    if input.source_event_id.trim().is_empty() {
        return Err(GateError::EmptySource);
    }
    if input.duplicate {
        return Ok(decision(GateOutcome::Ignore, "duplicate event"));
    }
    if input.trust == SourceTrust::Untrusted && input.risk == RiskLevel::High {
        return Ok(decision(GateOutcome::Ignore, "untrusted high-risk event"));
    }
    if input.risk == RiskLevel::High && !input.explicit_mention {
        return Ok(decision(
            GateOutcome::Observe,
            "high-risk event without explicit mention",
        ));
    }
    if input.trust == SourceTrust::Unknown && !input.explicit_mention {
        return Ok(decision(
            GateOutcome::Observe,
            "unknown source without explicit mention",
        ));
    }
    if input.confidence_bps >= policy.auto_at_bps
        && risk_rank(input.risk) <= risk_rank(policy.max_auto_risk)
        && (input.trust != SourceTrust::Untrusted || policy.allow_untrusted_auto)
    {
        return Ok(decision(
            GateOutcome::Auto,
            "confidence clears auto threshold",
        ));
    }
    if input.confidence_bps >= policy.observe_at_bps {
        return Ok(decision(
            GateOutcome::Observe,
            "confidence clears observe threshold",
        ));
    }
    Ok(decision(GateOutcome::Ignore, "confidence below gate"))
}

fn decision(outcome: GateOutcome, reason: &str) -> GateDecision {
    GateDecision {
        outcome,
        reason: reason.into(),
    }
}

fn risk_rank(risk: RiskLevel) -> u8 {
    match risk {
        RiskLevel::Low => 0,
        RiskLevel::Medium => 1,
        RiskLevel::High => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> GatePolicy {
        GatePolicy {
            observe_at_bps: 4_000,
            auto_at_bps: 8_000,
            max_auto_risk: RiskLevel::Medium,
            allow_untrusted_auto: false,
        }
    }

    fn input() -> GateInput {
        GateInput {
            source_event_id: "event-1".into(),
            trust: SourceTrust::Trusted,
            risk: RiskLevel::Low,
            confidence_bps: 9_000,
            explicit_mention: false,
            duplicate: false,
        }
    }

    #[test]
    fn duplicates_and_untrusted_high_risk_are_ignored() {
        let mut duplicate = input();
        duplicate.duplicate = true;
        assert_eq!(
            evaluate(&duplicate, &policy()).unwrap().outcome,
            GateOutcome::Ignore
        );
        let mut untrusted = input();
        untrusted.trust = SourceTrust::Untrusted;
        untrusted.risk = RiskLevel::High;
        untrusted.explicit_mention = true;
        assert_eq!(
            evaluate(&untrusted, &policy()).unwrap().outcome,
            GateOutcome::Ignore
        );
    }

    #[test]
    fn observe_and_auto_thresholds_are_deterministic() {
        assert_eq!(
            evaluate(&input(), &policy()).unwrap().outcome,
            GateOutcome::Auto
        );
        let mut observe = input();
        observe.confidence_bps = 5_000;
        assert_eq!(
            evaluate(&observe, &policy()).unwrap().outcome,
            GateOutcome::Observe
        );
        let mut ignore = input();
        ignore.confidence_bps = 1_000;
        assert_eq!(
            evaluate(&ignore, &policy()).unwrap().outcome,
            GateOutcome::Ignore
        );
        let mut unknown = input();
        unknown.trust = SourceTrust::Unknown;
        assert_eq!(
            evaluate(&unknown, &policy()).unwrap().outcome,
            GateOutcome::Observe
        );
    }

    #[test]
    fn high_risk_or_untrusted_never_auto_without_policy() {
        let mut high = input();
        high.risk = RiskLevel::High;
        high.explicit_mention = true;
        assert_eq!(
            evaluate(&high, &policy()).unwrap().outcome,
            GateOutcome::Observe
        );
        let mut untrusted = input();
        untrusted.trust = SourceTrust::Untrusted;
        assert_eq!(
            evaluate(&untrusted, &policy()).unwrap().outcome,
            GateOutcome::Observe
        );
    }

    #[test]
    fn invalid_policy_and_inputs_fail_closed() {
        let mut invalid = policy();
        invalid.auto_at_bps = 3_000;
        assert_eq!(
            evaluate(&input(), &invalid).unwrap_err(),
            GateError::InvalidThresholds
        );
        let mut bad = input();
        bad.source_event_id.clear();
        assert_eq!(
            evaluate(&bad, &policy()).unwrap_err(),
            GateError::EmptySource
        );
    }
}
