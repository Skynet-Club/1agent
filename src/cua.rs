//! Computer-use policy: argument provenance and branch-steering guards.
//!
//! The planner may see trusted plan state; screen content is untrusted. A CUA
//! action with an untrusted argument can be approved, but a high-risk action
//! with an untrusted argument is denied. A branch target must exist in the
//! pre-approved plan; an unplanned target is branch steering and is denied.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

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

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CuaPlanError {
    #[error("CUA plan id must be 1..=64 safe characters")]
    InvalidPlanId,
    #[error("CUA plan needs at least one branch")]
    EmptyBranches,
    #[error("CUA plan branch is invalid: {0}")]
    InvalidBranch(String),
    #[error("CUA plan has a duplicate branch: {0}")]
    DuplicateBranch(String),
}

/// A single-shot CUA plan: branches are approved and frozen before any action.
/// Once frozen, replanning is denied so untrusted screen content cannot steer
/// execution into a new branch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CuaPlan {
    plan_id: String,
    branches: Vec<String>,
    frozen: bool,
}

impl CuaPlan {
    pub fn new(plan_id: &str, branches: Vec<String>) -> Result<Self, CuaPlanError> {
        if plan_id.is_empty()
            || plan_id.chars().count() > 64
            || !plan_id.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
            })
        {
            return Err(CuaPlanError::InvalidPlanId);
        }
        validate_branches(&branches)?;
        Ok(Self {
            plan_id: plan_id.into(),
            branches,
            frozen: false,
        })
    }

    pub fn plan_id(&self) -> &str {
        &self.plan_id
    }

    pub fn branches(&self) -> &[String] {
        &self.branches
    }

    #[cfg(test)]
    pub fn is_frozen(&self) -> bool {
        self.frozen
    }

    pub fn freeze(&mut self) {
        self.frozen = true;
    }

    pub fn replan(&mut self, branches: Vec<String>) -> CuaVerdict {
        if self.frozen {
            return verdict(
                CuaDecision::Deny,
                "plan is frozen; replanning after execution is branch steering",
            );
        }
        if let Err(error) = validate_branches(&branches) {
            return verdict(CuaDecision::Deny, &error.to_string());
        }
        self.branches = branches;
        verdict(CuaDecision::Allow, "plan replaced before freeze")
    }

    pub fn authorize(&self, action: &CuaAction, target: &str) -> CuaVerdict {
        if !self.frozen {
            return verdict(
                CuaDecision::Deny,
                "plan must be frozen before an action is authorized",
            );
        }
        let branch = check_branch(target, &self.branches);
        if branch.decision != CuaDecision::Allow {
            return branch;
        }
        evaluate(action)
    }
}

fn validate_branches(branches: &[String]) -> Result<(), CuaPlanError> {
    if branches.is_empty() {
        return Err(CuaPlanError::EmptyBranches);
    }
    let mut seen = BTreeSet::new();
    for branch in branches {
        if branch.is_empty()
            || branch.chars().count() > 64
            || !branch.chars().all(|character| {
                character.is_ascii_alphanumeric()
                    || matches!(character, '.' | '_' | '-' | ':' | '/' | '@')
            })
        {
            return Err(CuaPlanError::InvalidBranch(branch.clone()));
        }
        if !seen.insert(branch) {
            return Err(CuaPlanError::DuplicateBranch(branch.clone()));
        }
    }
    Ok(())
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
#[cfg(test)]
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
    fn single_shot_plan_must_be_frozen_and_rejects_replanning() {
        let mut plan = CuaPlan::new("plan-1", vec!["confirm".into()]).unwrap();
        let click = action(RiskLevel::Low, true, vec![ArgumentSource::Plan]);
        assert_eq!(
            plan.authorize(&click, "confirm").decision,
            CuaDecision::Deny,
            "an unfrozen plan cannot authorize actions"
        );
        plan.freeze();
        assert!(plan.is_frozen());
        assert_eq!(
            plan.authorize(&click, "confirm").decision,
            CuaDecision::Allow
        );
        assert_eq!(
            plan.authorize(&click, "pay").decision,
            CuaDecision::Deny,
            "branch steering must stay inside the frozen plan"
        );
        let replan = plan.replan(vec!["pay".into()]);
        assert_eq!(replan.decision, CuaDecision::Deny);
        assert_eq!(plan.branches(), ["confirm"]);
    }

    #[test]
    fn invalid_plans_are_rejected() {
        assert_eq!(
            CuaPlan::new("", vec!["confirm".into()]),
            Err(CuaPlanError::InvalidPlanId)
        );
        assert_eq!(
            CuaPlan::new("plan", vec![]),
            Err(CuaPlanError::EmptyBranches)
        );
        assert_eq!(
            CuaPlan::new("plan", vec!["confirm".into(), "confirm".into()]),
            Err(CuaPlanError::DuplicateBranch("confirm".into()))
        );
        assert!(matches!(
            CuaPlan::new("plan", vec!["bad branch".into()]),
            Err(CuaPlanError::InvalidBranch(_))
        ));
    }

    #[test]
    fn red_team_cases_fail_closed() {
        let cases = [
            (
                "screen text changes the payee",
                action(
                    RiskLevel::High,
                    false,
                    vec![ArgumentSource::Plan, ArgumentSource::UntrustedObservation],
                ),
                CuaDecision::Deny,
            ),
            (
                "untrusted address book value",
                action(
                    RiskLevel::Medium,
                    true,
                    vec![ArgumentSource::UntrustedObservation],
                ),
                CuaDecision::RequireApproval,
            ),
            (
                "irreversible publish from the plan",
                action(RiskLevel::Medium, false, vec![ArgumentSource::Plan]),
                CuaDecision::RequireApproval,
            ),
            (
                "trusted reversible click",
                action(
                    RiskLevel::Low,
                    true,
                    vec![ArgumentSource::Plan, ArgumentSource::TrustedObservation],
                ),
                CuaDecision::Allow,
            ),
        ];
        for (name, case, expected) in cases {
            assert_eq!(evaluate(&case).decision, expected, "{name}");
        }
        let planned = vec!["confirm".into(), "cancel".into()];
        assert_eq!(
            check_branch("transfer-all", &planned).decision,
            CuaDecision::Deny,
            "branch steering must not leave the approved plan"
        );
        assert_eq!(
            planner_sees_untrusted(true).decision,
            CuaDecision::Deny,
            "planner quarantine must hide untrusted observations"
        );
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
