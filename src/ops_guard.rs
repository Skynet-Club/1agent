//! Runtime cost/call/time limits and a manual kill switch.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OpsGuardError {
    #[error("ops limits must all be non-zero")]
    InvalidLimits,
    #[error("ops guard is tripped: {0}")]
    Tripped(String),
    #[error("ops limit exceeded: {0}")]
    LimitExceeded(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OpsLimits {
    pub max_cost_micros: u64,
    pub max_model_calls: u32,
    pub max_tool_calls: u32,
    pub max_wall_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OpsMetrics {
    pub cost_micros: u64,
    pub model_calls: u32,
    pub tool_calls: u32,
    pub elapsed_ms: u128,
    pub tripped: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OpsGuard {
    limits: OpsLimits,
    started_ms: u128,
    cost_micros: u64,
    model_calls: u32,
    tool_calls: u32,
    tripped: Option<String>,
}

impl OpsGuard {
    pub fn new(limits: OpsLimits, now_ms: u128) -> Result<Self, OpsGuardError> {
        if limits.max_cost_micros == 0
            || limits.max_model_calls == 0
            || limits.max_tool_calls == 0
            || limits.max_wall_ms == 0
        {
            return Err(OpsGuardError::InvalidLimits);
        }
        Ok(Self {
            limits,
            started_ms: now_ms,
            cost_micros: 0,
            model_calls: 0,
            tool_calls: 0,
            tripped: None,
        })
    }

    pub fn check(&self, now_ms: u128) -> Result<(), OpsGuardError> {
        if let Some(reason) = &self.tripped {
            return Err(OpsGuardError::Tripped(reason.clone()));
        }
        let elapsed = now_ms.saturating_sub(self.started_ms);
        if elapsed > self.limits.max_wall_ms {
            return Err(OpsGuardError::LimitExceeded("wall time".into()));
        }
        if self.cost_micros > self.limits.max_cost_micros {
            return Err(OpsGuardError::LimitExceeded("cost".into()));
        }
        if self.model_calls > self.limits.max_model_calls {
            return Err(OpsGuardError::LimitExceeded("model calls".into()));
        }
        if self.tool_calls > self.limits.max_tool_calls {
            return Err(OpsGuardError::LimitExceeded("tool calls".into()));
        }
        Ok(())
    }

    pub fn record_model(&mut self, cost_micros: u64, now_ms: u128) -> Result<(), OpsGuardError> {
        self.check(now_ms)?;
        self.cost_micros = self.cost_micros.saturating_add(cost_micros);
        self.model_calls = self.model_calls.saturating_add(1);
        self.check(now_ms)
    }

    pub fn record_tool(&mut self, now_ms: u128) -> Result<(), OpsGuardError> {
        self.check(now_ms)?;
        self.tool_calls = self.tool_calls.saturating_add(1);
        self.check(now_ms)
    }

    pub fn trip(&mut self, reason: impl Into<String>) {
        self.tripped = Some(reason.into());
    }

    pub fn metrics(&self, now_ms: u128) -> OpsMetrics {
        OpsMetrics {
            cost_micros: self.cost_micros,
            model_calls: self.model_calls,
            tool_calls: self.tool_calls,
            elapsed_ms: now_ms.saturating_sub(self.started_ms),
            tripped: self.tripped.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> OpsLimits {
        OpsLimits {
            max_cost_micros: 100,
            max_model_calls: 2,
            max_tool_calls: 1,
            max_wall_ms: 1_000,
        }
    }

    #[test]
    fn cost_and_call_limits_trip_after_the_budget() {
        let mut guard = OpsGuard::new(limits(), 0).unwrap();
        guard.record_model(60, 1).unwrap();
        assert!(matches!(
            guard.record_model(60, 2),
            Err(OpsGuardError::LimitExceeded(_))
        ));
        let mut guard = OpsGuard::new(limits(), 0).unwrap();
        guard.record_tool(1).unwrap();
        assert!(matches!(
            guard.record_tool(2),
            Err(OpsGuardError::LimitExceeded(_))
        ));
    }

    #[test]
    fn wall_clock_and_manual_kill_switch_are_explicit() {
        let guard = OpsGuard::new(limits(), 0).unwrap();
        assert!(matches!(
            guard.check(1_001),
            Err(OpsGuardError::LimitExceeded(_))
        ));
        let mut guard = OpsGuard::new(limits(), 0).unwrap();
        guard.trip("operator stop");
        assert_eq!(
            guard.check(1).unwrap_err(),
            OpsGuardError::Tripped("operator stop".into())
        );
        assert_eq!(guard.metrics(1).tripped.as_deref(), Some("operator stop"));
    }

    #[test]
    fn zero_limits_are_rejected() {
        let mut limits = limits();
        limits.max_tool_calls = 0;
        assert_eq!(
            OpsGuard::new(limits, 0).unwrap_err(),
            OpsGuardError::InvalidLimits
        );
    }
}
