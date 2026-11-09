//! Runtime canary for skill versions.
//!
//! A candidate version is observed on live tasks while the active version
//! remains the baseline. Promotion requires a minimum sample count, a failure
//! budget and an optional p95 latency ceiling. Applying a decision goes through
//! `SkillService`, so activation and rollback remain journaled.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::skill_service::{SkillService, SkillServiceError};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SkillCanaryError {
    #[error("canary skill name must not be empty")]
    EmptyName,
    #[error("canary requires at least one sample")]
    InvalidMinSamples,
    #[error("failure budget must be 0..=10000 bps")]
    InvalidFailureBudget,
    #[error("skill version {0} {1} is not registered")]
    UnknownVersion(String, String),
    #[error("canary is already finished")]
    Finished,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CanaryPolicy {
    pub min_samples: u32,
    pub max_failure_bps: u32,
    pub max_p95_latency_ms: Option<u64>,
}

impl CanaryPolicy {
    fn validate(&self) -> Result<(), SkillCanaryError> {
        if self.min_samples == 0 {
            return Err(SkillCanaryError::InvalidMinSamples);
        }
        if self.max_failure_bps > 10_000 {
            return Err(SkillCanaryError::InvalidFailureBudget);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CanaryDecision {
    Pending {
        samples: u32,
        needed: u32,
    },
    Promote {
        samples: u32,
        failure_bps: u32,
        p95_latency_ms: u64,
    },
    Rollback {
        samples: u32,
        failure_bps: u32,
        p95_latency_ms: u64,
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CanaryState {
    Running,
    Promoted,
    RolledBack,
}

#[derive(Debug, Clone)]
pub struct SkillCanary {
    name: String,
    version: String,
    baseline_version: Option<String>,
    policy: CanaryPolicy,
    samples: u32,
    failures: u32,
    latencies: Vec<u64>,
    state: CanaryState,
}

impl SkillCanary {
    pub fn start(
        skills: &SkillService,
        name: &str,
        version: &str,
        policy: CanaryPolicy,
        _now_ms: u128,
    ) -> Result<Self, SkillCanaryError> {
        if name.trim().is_empty() {
            return Err(SkillCanaryError::EmptyName);
        }
        policy.validate()?;
        if skills.view(name, version).is_none() {
            return Err(SkillCanaryError::UnknownVersion(
                name.into(),
                version.into(),
            ));
        }
        Ok(Self {
            name: name.into(),
            version: version.into(),
            baseline_version: skills.active(name).map(|skill| skill.version.clone()),
            policy,
            samples: 0,
            failures: 0,
            latencies: Vec::new(),
            state: CanaryState::Running,
        })
    }

    pub fn state(&self) -> CanaryState {
        self.state
    }

    pub fn samples(&self) -> u32 {
        self.samples
    }

    pub fn record(&mut self, success: bool, latency_ms: u64) -> Result<(), SkillCanaryError> {
        if self.state != CanaryState::Running {
            return Err(SkillCanaryError::Finished);
        }
        self.samples = self.samples.saturating_add(1);
        if !success {
            self.failures = self.failures.saturating_add(1);
        }
        self.latencies.push(latency_ms);
        Ok(())
    }

    pub fn decision(&self) -> CanaryDecision {
        if self.samples < self.policy.min_samples {
            return CanaryDecision::Pending {
                samples: self.samples,
                needed: self.policy.min_samples,
            };
        }
        let failure_bps = self.failures.saturating_mul(10_000) / self.samples.max(1);
        let p95_latency_ms = crate::metrics::percentile(&self.latencies, 95).unwrap_or(0);
        if failure_bps > self.policy.max_failure_bps {
            return CanaryDecision::Rollback {
                samples: self.samples,
                failure_bps,
                p95_latency_ms,
                reason: format!(
                    "failure rate {failure_bps} bps exceeds {} bps",
                    self.policy.max_failure_bps
                ),
            };
        }
        if self
            .policy
            .max_p95_latency_ms
            .is_some_and(|limit| p95_latency_ms > limit)
        {
            return CanaryDecision::Rollback {
                samples: self.samples,
                failure_bps,
                p95_latency_ms,
                reason: format!(
                    "p95 latency {p95_latency_ms} ms exceeds {} ms",
                    self.policy.max_p95_latency_ms.unwrap_or_default()
                ),
            };
        }
        CanaryDecision::Promote {
            samples: self.samples,
            failure_bps,
            p95_latency_ms,
        }
    }

    /// Apply a final decision to the registry. Pending decisions are a no-op.
    pub fn apply(
        &mut self,
        skills: &mut SkillService,
        now_ms: u128,
    ) -> Result<CanaryDecision, SkillServiceError> {
        let decision = self.decision();
        match decision {
            CanaryDecision::Pending { .. } => Ok(decision),
            CanaryDecision::Promote { .. } if self.state == CanaryState::Running => {
                skills.activate(&self.name, &self.version, now_ms)?;
                self.state = CanaryState::Promoted;
                Ok(decision)
            }
            CanaryDecision::Rollback { .. } if self.state == CanaryState::Running => {
                if let Some(baseline) = &self.baseline_version {
                    skills.rollback(&self.name, baseline, now_ms)?;
                }
                self.state = CanaryState::RolledBack;
                Ok(decision)
            }
            _ => Ok(decision),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::{SkillManifest, SkillRisk};

    fn manifest(version: &str) -> SkillManifest {
        SkillManifest {
            name: "release".into(),
            description: "release".into(),
            version: version.into(),
            platforms: vec!["linux".into()],
            scope: "repo".into(),
            requires_tools: Vec::new(),
            risk: SkillRisk::Low,
            body: "1. verify".into(),
            content_sha256: String::new(),
        }
    }

    fn skills(dir: &std::path::Path) -> SkillService {
        let mut skills = SkillService::open(dir.join("skills.jsonl")).unwrap();
        skills.register(manifest("1.0.0"), 1).unwrap();
        skills.register(manifest("1.1.0"), 2).unwrap();
        skills.activate("release", "1.0.0", 3).unwrap();
        skills
    }

    fn policy() -> CanaryPolicy {
        CanaryPolicy {
            min_samples: 4,
            max_failure_bps: 2_500,
            max_p95_latency_ms: Some(100),
        }
    }

    #[test]
    fn promotes_after_enough_healthy_samples() {
        let dir = tempfile::tempdir().unwrap();
        let mut skills = skills(dir.path());
        let mut canary = SkillCanary::start(&skills, "release", "1.1.0", policy(), 10).unwrap();
        for latency in [10, 20, 30, 40] {
            canary.record(true, latency).unwrap();
        }
        assert!(matches!(
            canary.decision(),
            CanaryDecision::Promote {
                samples: 4,
                failure_bps: 0,
                p95_latency_ms: 40
            }
        ));
        canary.apply(&mut skills, 20).unwrap();
        assert_eq!(canary.state(), CanaryState::Promoted);
        assert_eq!(skills.active("release").unwrap().version, "1.1.0");
        assert_eq!(
            canary.record(true, 10).unwrap_err(),
            SkillCanaryError::Finished
        );
    }

    #[test]
    fn rolls_back_on_failures_and_restores_baseline() {
        let dir = tempfile::tempdir().unwrap();
        let mut skills = skills(dir.path());
        let mut canary = SkillCanary::start(&skills, "release", "1.1.0", policy(), 10).unwrap();
        canary.record(true, 10).unwrap();
        canary.record(false, 20).unwrap();
        canary.record(false, 30).unwrap();
        assert_eq!(
            canary.decision(),
            CanaryDecision::Pending {
                samples: 3,
                needed: 4
            }
        );
        canary.record(true, 40).unwrap();
        assert!(matches!(
            canary.decision(),
            CanaryDecision::Rollback {
                failure_bps: 5_000,
                ..
            }
        ));
        canary.apply(&mut skills, 20).unwrap();
        assert_eq!(canary.state(), CanaryState::RolledBack);
        assert_eq!(skills.active("release").unwrap().version, "1.0.0");
    }

    #[test]
    fn rolls_back_on_latency_and_disables_without_baseline() {
        let dir = tempfile::tempdir().unwrap();
        let mut skills = SkillService::open(dir.path().join("skills.jsonl")).unwrap();
        skills.register(manifest("1.1.0"), 1).unwrap();
        let mut canary = SkillCanary::start(&skills, "release", "1.1.0", policy(), 10).unwrap();
        for latency in [10, 20, 30, 200] {
            canary.record(true, latency).unwrap();
        }
        assert!(matches!(
            canary.decision(),
            CanaryDecision::Rollback {
                p95_latency_ms: 200,
                ..
            }
        ));
        canary.apply(&mut skills, 20).unwrap();
        assert!(skills.active("release").is_none());
    }

    #[test]
    fn rejects_invalid_policy_and_unknown_version() {
        let dir = tempfile::tempdir().unwrap();
        let skills = skills(dir.path());
        assert_eq!(
            SkillCanary::start(
                &skills,
                "release",
                "1.1.0",
                CanaryPolicy {
                    min_samples: 0,
                    max_failure_bps: 100,
                    max_p95_latency_ms: None,
                },
                10
            )
            .unwrap_err(),
            SkillCanaryError::InvalidMinSamples
        );
        assert!(matches!(
            SkillCanary::start(&skills, "release", "9.9.9", policy(), 10).unwrap_err(),
            SkillCanaryError::UnknownVersion(_, _)
        ));
    }
}
