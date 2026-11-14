//! In-memory provider health used to avoid repeatedly selecting a failing
//! provider. Health is deliberately separate from routing policy: it only
//! demotes a provider for a cooldown and never grants capabilities.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderHealthPolicy {
    pub failure_threshold: u32,
    pub cooldown_ms: u128,
}

impl Default for ProviderHealthPolicy {
    fn default() -> Self {
        Self {
            failure_threshold: 2,
            cooldown_ms: 30_000,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderHealthEntry {
    pub provider_id: String,
    pub model: String,
    pub consecutive_failures: u32,
    pub total_failures: u64,
    pub total_successes: u64,
    pub last_failure_ms: Option<u128>,
    pub last_success_ms: Option<u128>,
}

impl ProviderHealthEntry {
    fn available(&self, now_ms: u128, policy: &ProviderHealthPolicy) -> bool {
        self.consecutive_failures < policy.failure_threshold
            || self
                .last_failure_ms
                .is_some_and(|last| now_ms.saturating_sub(last) >= policy.cooldown_ms)
    }
}

#[derive(Debug, Clone)]
pub struct ProviderHealth {
    policy: ProviderHealthPolicy,
    entries: BTreeMap<(String, String), ProviderHealthEntry>,
}

impl ProviderHealth {
    pub fn new(policy: ProviderHealthPolicy) -> Result<Self, String> {
        if policy.failure_threshold == 0 || policy.cooldown_ms == 0 {
            return Err("provider health limits must be non-zero".into());
        }
        Ok(Self {
            policy,
            entries: BTreeMap::new(),
        })
    }

    pub fn policy(&self) -> ProviderHealthPolicy {
        self.policy
    }

    /// Record a success and reset the failure streak.
    pub fn record_success(&mut self, provider_id: &str, model: &str, now_ms: u128) {
        let entry = self.entry(provider_id, model);
        entry.consecutive_failures = 0;
        entry.total_successes = entry.total_successes.saturating_add(1);
        entry.last_success_ms = Some(now_ms);
    }

    /// Record a failure; returns true when the threshold was just reached.
    pub fn record_failure(&mut self, provider_id: &str, model: &str, now_ms: u128) -> bool {
        let threshold = self.policy.failure_threshold;
        let entry = self.entry(provider_id, model);
        entry.consecutive_failures = entry.consecutive_failures.saturating_add(1);
        entry.total_failures = entry.total_failures.saturating_add(1);
        entry.last_failure_ms = Some(now_ms);
        entry.consecutive_failures == threshold
    }

    pub fn available(&self, provider_id: &str, model: &str, now_ms: u128) -> bool {
        self.entries
            .get(&(provider_id.into(), model.into()))
            .is_none_or(|entry| entry.available(now_ms, &self.policy))
    }

    pub fn snapshot(&self) -> Vec<ProviderHealthEntry> {
        self.entries.values().cloned().collect()
    }

    fn entry(&mut self, provider_id: &str, model: &str) -> &mut ProviderHealthEntry {
        self.entries
            .entry((provider_id.into(), model.into()))
            .or_insert_with(|| ProviderHealthEntry {
                provider_id: provider_id.into(),
                model: model.into(),
                ..ProviderHealthEntry::default()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_demote_until_cooldown_then_recover() {
        let mut health = ProviderHealth::new(ProviderHealthPolicy {
            failure_threshold: 2,
            cooldown_ms: 100,
        })
        .unwrap();
        assert!(health.available("p", "m", 0));
        assert!(!health.record_failure("p", "m", 10));
        assert!(health.available("p", "m", 10));
        assert!(health.record_failure("p", "m", 20));
        assert!(!health.available("p", "m", 20));
        assert!(!health.available("p", "m", 119));
        assert!(health.available("p", "m", 120));
        health.record_success("p", "m", 121);
        assert!(health.available("p", "m", 121));
        assert_eq!(health.snapshot()[0].consecutive_failures, 0);
    }

    #[test]
    fn providers_are_independent_and_invalid_policy_is_rejected() {
        let mut health = ProviderHealth::new(ProviderHealthPolicy {
            failure_threshold: 1,
            cooldown_ms: 10,
        })
        .unwrap();
        health.record_failure("a", "m", 1);
        assert!(!health.available("a", "m", 2));
        assert!(health.available("b", "m", 2));
        assert!(
            ProviderHealth::new(ProviderHealthPolicy {
                failure_threshold: 0,
                cooldown_ms: 10
            })
            .is_err()
        );
    }
}
