//! Autonomy dial policy.
//!
//! A user preference is not an authorization grant. This maps the preference
//! plus risk, reversibility and any explicit approval into one of four
//! outcomes. `observe` and `suggest` never act; `act` and `auto` differ on
//! medium-risk reversible actions.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::RiskLevel;
use crate::user_model::{AutonomyLevel, UserModel};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QuietHoursError {
    #[error("quiet hours must be HH:MM-HH:MM")]
    Format,
    #[error("quiet hours contain an invalid time")]
    Time,
    #[error("quiet hours start and end must differ")]
    Empty,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct QuietHours {
    pub start_minute: u16,
    pub end_minute: u16,
}

impl QuietHours {
    pub fn parse(value: &str) -> Result<Self, QuietHoursError> {
        let (start, end) = value.split_once('-').ok_or(QuietHoursError::Format)?;
        let start_minute = parse_hhmm(start)?;
        let end_minute = parse_hhmm(end)?;
        if start_minute == end_minute {
            return Err(QuietHoursError::Empty);
        }
        Ok(Self {
            start_minute,
            end_minute,
        })
    }

    /// Start is inclusive, end is exclusive; windows may cross midnight.
    pub fn contains(self, minute: u16) -> bool {
        if self.start_minute < self.end_minute {
            (self.start_minute..self.end_minute).contains(&minute)
        } else {
            minute >= self.start_minute || minute < self.end_minute
        }
    }

    pub fn minutes_until_end(self, minute: u16) -> u16 {
        if self.end_minute > minute {
            self.end_minute - minute
        } else {
            1440 - minute + self.end_minute
        }
    }
}

pub fn parse_hhmm(value: &str) -> Result<u16, QuietHoursError> {
    let (hour, minute) = value
        .trim()
        .split_once(':')
        .ok_or(QuietHoursError::Format)?;
    let hour: u16 = hour.parse().map_err(|_| QuietHoursError::Time)?;
    let minute: u16 = minute.parse().map_err(|_| QuietHoursError::Time)?;
    if hour > 23 || minute > 59 {
        return Err(QuietHoursError::Time);
    }
    Ok(hour * 60 + minute)
}

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
    /// The action would run, but quiet hours postpone proactive work.
    Defer,
}

impl AutonomyDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Observe => "observe",
            Self::Suggest => "suggest",
            Self::Act => "act",
            Self::RequireApproval => "require_approval",
            Self::Defer => "defer",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProactiveContext {
    pub now_minute: u16,
    pub user_initiated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quiet_hours: Option<QuietHours>,
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

/// Applies the same policy, then defers proactive execution during quiet hours.
/// User-initiated work and explicit approval are never deferred.
pub fn decide_proactive(
    level: AutonomyLevel,
    risk: RiskLevel,
    reversible: bool,
    explicit_approval: bool,
    context: ProactiveContext,
) -> AutonomyPlan {
    let plan = decide(level, risk, reversible, explicit_approval);
    let Some(quiet) = context.quiet_hours else {
        return plan;
    };
    if context.user_initiated
        || explicit_approval
        || !quiet.contains(context.now_minute)
        || !matches!(
            plan.decision,
            AutonomyDecision::Act | AutonomyDecision::RequireApproval
        )
    {
        return plan;
    }
    let until = quiet.minutes_until_end(context.now_minute);
    AutonomyPlan {
        decision: AutonomyDecision::Defer,
        reason: format!(
            "quiet hours are active; proactive {} deferred for {until} minutes",
            plan.decision.as_str()
        ),
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
    fn quiet_hours_parse_and_wrap_midnight() {
        let overnight = QuietHours::parse("22:00-07:00").unwrap();
        assert!(overnight.contains(22 * 60));
        assert!(overnight.contains(23 * 60 + 59));
        assert!(overnight.contains(0));
        assert!(!overnight.contains(7 * 60));
        assert!(!overnight.contains(12 * 60));
        assert_eq!(overnight.minutes_until_end(23 * 60), 8 * 60);

        let daytime = QuietHours::parse("09:00-17:00").unwrap();
        assert!(daytime.contains(9 * 60));
        assert!(!daytime.contains(17 * 60));

        assert!(QuietHours::parse("07:00-07:00").is_err());
        assert!(QuietHours::parse("24:00-07:00").is_err());
        assert!(QuietHours::parse("bedtime").is_err());
    }

    #[test]
    fn quiet_hours_defer_proactive_work_only() {
        let context = ProactiveContext {
            now_minute: 23 * 60,
            user_initiated: false,
            quiet_hours: Some(QuietHours::parse("22:00-07:00").unwrap()),
        };
        let deferred = decide_proactive(AutonomyLevel::Auto, RiskLevel::Low, true, false, context);
        assert_eq!(deferred.decision, AutonomyDecision::Defer);
        assert!(deferred.reason.contains("480 minutes"));

        let user_ask = decide_proactive(
            AutonomyLevel::Auto,
            RiskLevel::Low,
            true,
            false,
            ProactiveContext {
                user_initiated: true,
                ..context
            },
        );
        assert_eq!(user_ask.decision, AutonomyDecision::Act);

        let approved = decide_proactive(AutonomyLevel::Auto, RiskLevel::High, false, true, context);
        assert_eq!(approved.decision, AutonomyDecision::Act);

        let observe =
            decide_proactive(AutonomyLevel::Observe, RiskLevel::Low, true, false, context);
        assert_eq!(observe.decision, AutonomyDecision::Observe);

        let daytime = decide_proactive(
            AutonomyLevel::Auto,
            RiskLevel::Low,
            true,
            false,
            ProactiveContext {
                now_minute: 12 * 60,
                ..context
            },
        );
        assert_eq!(daytime.decision, AutonomyDecision::Act);
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
