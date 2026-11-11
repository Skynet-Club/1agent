//! Autonomy dial policy.
//!
//! A user preference is not an authorization grant. This maps the preference
//! plus risk, reversibility and any explicit approval into one of four
//! outcomes. `observe` and `suggest` never act; `act` and `auto` differ on
//! medium-risk reversible actions.

use serde::{Deserialize, Serialize};

use crate::domain::RiskLevel;
use crate::user_model::{AutonomyLevel, UserModel};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyDecision {
    /// Report only; do not propose an action.
    Observe,
    /// Propose an action and wait for the user.
    Suggest,
    /// Execute and report.
    Act,
    /// The action is ready, but needs an explicit yes.
    RequireApproval,
}

impl AutonomyDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Observe => "observe",
            Self::Suggest => "suggest",
            Self::Act => "act",
            Self::RequireApproval => "require_approval",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AutonomyPlan {
    pub decision: AutonomyDecision,
    pub reason: String,
}

pub fn decide(
    level: AutonomyLevel,
    risk: RiskLevel,
    reversible: bool,
    explicit_approval: bool,
) -> AutonomyPlan {
    if level == AutonomyLevel::Observe {
        return plan(AutonomyDecision::Observe, "user set observe");
    }
    if level == AutonomyLevel::Suggest {
        return plan(AutonomyDecision::Suggest, "user set suggest");
    }
    if explicit_approval {
        return plan(
            AutonomyDecision::Act,
            "explicit approval covers this action",
        );
    }
    if !reversible {
        return plan(AutonomyDecision::RequireApproval, "action is irreversible");
    }
    match (level, risk) {
        (AutonomyLevel::Act, RiskLevel::Low) => plan(
            AutonomyDecision::Act,
            "act allows low-risk reversible actions",
        ),
        (AutonomyLevel::Act, RiskLevel::Medium | RiskLevel::High) => plan(
            AutonomyDecision::RequireApproval,
            "act requires approval above low risk",
        ),
        (AutonomyLevel::Auto, RiskLevel::Low | RiskLevel::Medium) => plan(
            AutonomyDecision::Act,
            "auto allows low/medium-risk reversible actions",
        ),
        (AutonomyLevel::Auto, RiskLevel::High) => plan(
            AutonomyDecision::RequireApproval,
            "high risk requires approval even in auto",
        ),
        (AutonomyLevel::Observe | AutonomyLevel::Suggest, _) => {
            plan(AutonomyDecision::Suggest, "non-acting level")
        }
    }
}

/// Resolve the user's per-domain preference and apply the same policy.
pub fn decide_for(
    model: &UserModel,
    domain: &str,
    risk: RiskLevel,
    reversible: bool,
    explicit_approval: bool,
) -> AutonomyPlan {
    let level = model.autonomy_for(domain);
    let mut plan = decide(level, risk, reversible, explicit_approval);
    plan.reason = format!("domain {domain}: {}", plan.reason);
    plan
}

fn plan(decision: AutonomyDecision, reason: &str) -> AutonomyPlan {
    AutonomyPlan {
        decision,
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemoryScope;
    use crate::user_model::AutonomyEntry;

    #[test]
    fn observe_and_suggest_never_act() {
        for level in [AutonomyLevel::Observe, AutonomyLevel::Suggest] {
            let plan = decide(level, RiskLevel::Low, true, true);
            assert_ne!(plan.decision, AutonomyDecision::Act);
        }
    }

    #[test]
    fn act_allows_only_low_risk_reversible() {
        assert_eq!(
            decide(AutonomyLevel::Act, RiskLevel::Low, true, false).decision,
            AutonomyDecision::Act
        );
        assert_eq!(
            decide(AutonomyLevel::Act, RiskLevel::Medium, true, false).decision,
            AutonomyDecision::RequireApproval
        );
        assert_eq!(
            decide(AutonomyLevel::Act, RiskLevel::Low, false, false).decision,
            AutonomyDecision::RequireApproval
        );
    }

    #[test]
    fn auto_allows_low_and_medium_reversible() {
        assert_eq!(
            decide(AutonomyLevel::Auto, RiskLevel::Medium, true, false).decision,
            AutonomyDecision::Act
        );
        assert_eq!(
            decide(AutonomyLevel::Auto, RiskLevel::High, true, false).decision,
            AutonomyDecision::RequireApproval
        );
        assert_eq!(
            decide(AutonomyLevel::Auto, RiskLevel::Low, false, false).decision,
            AutonomyDecision::RequireApproval
        );
    }

    #[test]
    fn explicit_approval_covers_act_and_auto() {
        assert_eq!(
            decide(AutonomyLevel::Act, RiskLevel::High, false, true).decision,
            AutonomyDecision::Act
        );
        assert_eq!(
            decide(AutonomyLevel::Auto, RiskLevel::High, false, true).decision,
            AutonomyDecision::Act
        );
    }

    #[test]
    fn user_model_domain_level_is_used() {
        let mut model = UserModel::new("user-1", MemoryScope::bot("bot-1"));
        model.autonomy.insert(
            "email".into(),
            AutonomyEntry {
                level: AutonomyLevel::Observe,
                source_memory_id: "m1".into(),
                updated_ms: 1,
            },
        );
        let plan = decide_for(&model, "email", RiskLevel::Low, true, false);
        assert_eq!(plan.decision, AutonomyDecision::Observe);
        assert!(plan.reason.contains("domain email"));
        let default = decide_for(&model, "browser", RiskLevel::Low, true, false);
        assert_eq!(default.decision, AutonomyDecision::Suggest);
    }
}
