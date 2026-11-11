//! Computer-use policy: argument provenance and branch-steering guards.
//!
//! The planner may see trusted plan state; screen content is untrusted. A CUA
//! action with an untrusted argument can be approved, but a high-risk action
//! with an untrusted argument is denied. A branch target must exist in the
//! pre-approved plan; an unplanned target is branch steering and is denied.

use serde::{Deserialize, Serialize};

use crate::domain::RiskLevel;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArgumentSource {
    /// A constant from the approved plan.
    Plan,
    /// A value from a trusted source, such as a local config.
    TrustedObservation,
    /// A value read from a web page, screenshot or other untrusted content.
    UntrustedObservation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CuaAction {
    pub action: String,
    pub risk: RiskLevel,
    pub reversible: bool,
    pub argument_sources: Vec<ArgumentSource>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CuaDecision {
    Allow,
    RequireApproval,
    Deny,
}

impl CuaDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::RequireApproval => "require_approval",
            Self::Deny => "deny",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CuaVerdict {
    pub decision: CuaDecision,
    pub reason: String,
}

pub fn evaluate(action: &CuaAction) -> CuaVerdict {
    if action.action.trim().is_empty() {
        return verdict(CuaDecision::Deny, "action name is empty");
    }
    let untrusted = action
        .argument_sources
        .contains(&ArgumentSource::UntrustedObservation);
    if untrusted && action.risk == RiskLevel::High {
        return verdict(
            CuaDecision::Deny,
            "high-risk action has an untrusted argument",
        );
    }
    if untrusted {
        return verdict(
            CuaDecision::RequireApproval,
            "untrusted argument requires explicit approval",
        );
    }
    if !action.reversible && action.risk != RiskLevel::Low {
        return verdict(
            CuaDecision::RequireApproval,
            "irreversible action above low risk requires approval",
        );
    }
    verdict(CuaDecision::Allow, "trusted arguments and acceptable risk")
}

/// The planner must not see untrusted screen content. It may only receive
/// trusted plan/config values; the quarantined perception layer resolves the
/// rest.
pub fn planner_sees_untrusted(content_is_untrusted: bool) -> CuaVerdict {
    if content_is_untrusted {
        verdict(
            CuaDecision::Deny,
            "planner must not see untrusted screen content",
        )
    } else {
        verdict(CuaDecision::Allow, "trusted planner context")
    }
}

/// Branch steering guard: a target must exist in the pre-approved plan.
pub fn check_branch(target: &str, planned_branches: &[String]) -> CuaVerdict {
    if planned_branches.iter().any(|branch| branch == target) {
        verdict(CuaDecision::Allow, "branch is in the approved plan")
    } else {
        verdict(
            CuaDecision::Deny,
            "branch target is not in the approved plan",
        )
    }
}

fn verdict(decision: CuaDecision, reason: &str) -> CuaVerdict {
    CuaVerdict {
        decision,
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(risk: RiskLevel, reversible: bool, sources: Vec<ArgumentSource>) -> CuaAction {
        CuaAction {
            action: "click".into(),
            risk,
            reversible,
            argument_sources: sources,
        }
    }

    #[test]
    fn trusted_low_risk_is_allowed() {
        let verdict = evaluate(&action(
            RiskLevel::Low,
            true,
            vec![ArgumentSource::Plan, ArgumentSource::TrustedObservation],
        ));
        assert_eq!(verdict.decision, CuaDecision::Allow);
    }

    #[test]
    fn untrusted_argument_requires_approval_or_is_denied() {
        let low = evaluate(&action(
            RiskLevel::Low,
            true,
            vec![ArgumentSource::UntrustedObservation],
        ));
        assert_eq!(low.decision, CuaDecision::RequireApproval);

        let high = evaluate(&action(
            RiskLevel::High,
            false,
            vec![ArgumentSource::UntrustedObservation],
        ));
        assert_eq!(high.decision, CuaDecision::Deny);
    }

    #[test]
    fn irreversible_medium_risk_requires_approval() {
        let verdict = evaluate(&action(
            RiskLevel::Medium,
            false,
            vec![ArgumentSource::Plan],
        ));
        assert_eq!(verdict.decision, CuaDecision::RequireApproval);
    }

    #[test]
    fn empty_action_is_denied() {
        let mut empty = action(RiskLevel::Low, true, vec![ArgumentSource::Plan]);
        empty.action.clear();
        assert_eq!(evaluate(&empty).decision, CuaDecision::Deny);
    }

    #[test]
    fn planner_and_branch_guards_fail_closed() {
        assert_eq!(planner_sees_untrusted(true).decision, CuaDecision::Deny);
        assert_eq!(planner_sees_untrusted(false).decision, CuaDecision::Allow);
        let planned = vec!["confirm".into(), "cancel".into()];
        assert_eq!(
            check_branch("confirm", &planned).decision,
            CuaDecision::Allow
        );
        assert_eq!(check_branch("pay", &planned).decision, CuaDecision::Deny);
    }
}
