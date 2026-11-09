//! Deterministic latency and provider health summaries.
//!
//! These are pure functions over samples/reports so they can run in tests and
//! offline evaluation without a metrics server. Percentiles use nearest-rank.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::provider::ProviderProbeReport;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LatencyDigest {
    pub count: u64,
    pub min_ms: u64,
    pub p50_ms: u64,
    pub p95_ms: u64,
    pub max_ms: u64,
}

impl LatencyDigest {
    pub fn from_samples(samples: &[u64]) -> Option<Self> {
        let mut sorted = samples.to_vec();
        if sorted.is_empty() {
            return None;
        }
        sorted.sort_unstable();
        Some(Self {
            count: sorted.len() as u64,
            min_ms: sorted[0],
            p50_ms: percentile_sorted(&sorted, 50),
            p95_ms: percentile_sorted(&sorted, 95),
            max_ms: sorted[sorted.len() - 1],
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderProbeDigest {
    pub provider_id: String,
    pub model: String,
    pub checks: u64,
    pub ok: u64,
    pub failed: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub latency: Option<LatencyDigest>,
}

/// Aggregate probe reports by provider/model, keeping failures visible and
/// averaging latency only over successful probes.
pub fn digest_provider_probes(reports: &[ProviderProbeReport]) -> Vec<ProviderProbeDigest> {
    struct Accumulator {
        checks: u64,
        ok: u64,
        failed: u64,
        input_tokens: u64,
        output_tokens: u64,
        latencies: Vec<u64>,
    }

    let mut grouped: BTreeMap<(String, String), Accumulator> = BTreeMap::new();
    for report in reports {
        for result in &report.results {
            let entry = grouped
                .entry((result.provider_id.clone(), result.model.clone()))
                .or_insert_with(|| Accumulator {
                    checks: 0,
                    ok: 0,
                    failed: 0,
                    input_tokens: 0,
                    output_tokens: 0,
                    latencies: Vec::new(),
                });
            entry.checks = entry.checks.saturating_add(1);
            entry.input_tokens = entry.input_tokens.saturating_add(result.input_tokens);
            entry.output_tokens = entry.output_tokens.saturating_add(result.output_tokens);
            if result.ok {
                entry.ok = entry.ok.saturating_add(1);
                entry.latencies.push(result.latency_ms);
            } else {
                entry.failed = entry.failed.saturating_add(1);
            }
        }
    }

    grouped
        .into_iter()
        .map(|((provider_id, model), accumulator)| ProviderProbeDigest {
            provider_id,
            model,
            checks: accumulator.checks,
            ok: accumulator.ok,
            failed: accumulator.failed,
            input_tokens: accumulator.input_tokens,
            output_tokens: accumulator.output_tokens,
            latency: LatencyDigest::from_samples(&accumulator.latencies),
        })
        .collect()
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderAlertPolicy {
    pub min_checks: u64,
    pub max_failure_bps: u32,
    pub max_p95_latency_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAlertKind {
    FailureRate,
    P95Latency,
    NoSuccessfulProbe,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderAlert {
    pub provider_id: String,
    pub model: String,
    pub kind: ProviderAlertKind,
    pub observed: u64,
    pub threshold: u64,
}

/// Evaluate provider digests against a failure/latency policy. Digests with
/// fewer than `min_checks` probes are ignored so a new provider is not paged
/// on its first sample.
pub fn evaluate_provider_alerts(
    digests: &[ProviderProbeDigest],
    policy: &ProviderAlertPolicy,
) -> Vec<ProviderAlert> {
    let mut alerts = Vec::new();
    for digest in digests {
        if digest.checks < policy.min_checks {
            continue;
        }
        if digest.ok == 0 {
            alerts.push(ProviderAlert {
                provider_id: digest.provider_id.clone(),
                model: digest.model.clone(),
                kind: ProviderAlertKind::NoSuccessfulProbe,
                observed: digest.failed,
                threshold: policy.max_failure_bps.into(),
            });
            continue;
        }
        let failure_bps = digest.failed.saturating_mul(10_000) / digest.checks.max(1);
        if failure_bps > u64::from(policy.max_failure_bps) {
            alerts.push(ProviderAlert {
                provider_id: digest.provider_id.clone(),
                model: digest.model.clone(),
                kind: ProviderAlertKind::FailureRate,
                observed: failure_bps,
                threshold: u64::from(policy.max_failure_bps),
            });
        }
        if let (Some(limit), Some(latency)) = (policy.max_p95_latency_ms, digest.latency)
            && latency.p95_ms > limit
        {
            alerts.push(ProviderAlert {
                provider_id: digest.provider_id.clone(),
                model: digest.model.clone(),
                kind: ProviderAlertKind::P95Latency,
                observed: latency.p95_ms,
                threshold: limit,
            });
        }
    }
    alerts
}

pub fn percentile(samples: &[u64], percentile: u32) -> Option<u64> {
    if samples.is_empty() {
        return None;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    Some(percentile_sorted(&sorted, percentile))
}

fn percentile_sorted(sorted: &[u64], percentile: u32) -> u64 {
    let rank = (usize::try_from(percentile).unwrap_or(100) * sorted.len()).div_ceil(100);
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{ProviderProbeReport, ProviderProbeResult};

    fn result(provider: &str, ok: bool, latency_ms: u64) -> ProviderProbeResult {
        ProviderProbeResult {
            provider_id: provider.into(),
            model: "model".into(),
            ok,
            latency_ms,
            input_tokens: 10,
            output_tokens: 5,
            answer_status: None,
            error_code: None,
        }
    }

    #[test]
    fn latency_digest_uses_nearest_rank() {
        let digest = LatencyDigest::from_samples(&[40, 10, 30, 20]).unwrap();
        assert_eq!(
            digest,
            LatencyDigest {
                count: 4,
                min_ms: 10,
                p50_ms: 20,
                p95_ms: 40,
                max_ms: 40
            }
        );
        assert_eq!(percentile(&[5], 95), Some(5));
        assert_eq!(percentile(&[], 95), None);
        assert!(LatencyDigest::from_samples(&[]).is_none());
    }

    #[test]
    fn provider_alert_policy_pages_failures_and_latency() {
        let reports = vec![ProviderProbeReport {
            schema_version: 1,
            checked: 5,
            failed: 2,
            results: vec![
                result("alpha", false, 10),
                result("alpha", false, 10),
                result("alpha", true, 120),
                result("alpha", true, 130),
                result("alpha", true, 140),
            ],
        }];
        let digests = digest_provider_probes(&reports);
        let policy = ProviderAlertPolicy {
            min_checks: 5,
            max_failure_bps: 3_000,
            max_p95_latency_ms: Some(100),
        };
        let alerts = evaluate_provider_alerts(&digests, &policy);
        assert_eq!(alerts.len(), 2);
        assert_eq!(alerts[0].kind, ProviderAlertKind::FailureRate);
        assert_eq!(alerts[0].observed, 4_000);
        assert_eq!(alerts[1].kind, ProviderAlertKind::P95Latency);
        assert_eq!(alerts[1].observed, 140);

        let quiet = evaluate_provider_alerts(
            &digests,
            &ProviderAlertPolicy {
                min_checks: 6,
                ..policy
            },
        );
        assert!(quiet.is_empty());
    }

    #[test]
    fn provider_without_successes_is_a_no_success_alert() {
        let reports = vec![ProviderProbeReport {
            schema_version: 1,
            checked: 2,
            failed: 2,
            results: vec![result("down", false, 10), result("down", false, 10)],
        }];
        let digests = digest_provider_probes(&reports);
        let alerts = evaluate_provider_alerts(
            &digests,
            &ProviderAlertPolicy {
                min_checks: 1,
                max_failure_bps: 10_000,
                max_p95_latency_ms: None,
            },
        );
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].kind, ProviderAlertKind::NoSuccessfulProbe);
    }

    #[test]
    fn provider_digest_keeps_failures_and_success_latency() {
        let reports = vec![
            ProviderProbeReport {
                schema_version: 1,
                checked: 2,
                failed: 1,
                results: vec![result("alpha", true, 10), result("alpha", false, 500)],
            },
            ProviderProbeReport {
                schema_version: 1,
                checked: 1,
                failed: 0,
                results: vec![result("alpha", true, 30)],
            },
        ];
        let digests = digest_provider_probes(&reports);
        assert_eq!(digests.len(), 1);
        let digest = &digests[0];
        assert_eq!(digest.provider_id, "alpha");
        assert_eq!(digest.checks, 3);
        assert_eq!(digest.ok, 2);
        assert_eq!(digest.failed, 1);
        assert_eq!(digest.latency.unwrap().p95_ms, 30);
        assert_eq!(digest.input_tokens, 30);
    }
}
