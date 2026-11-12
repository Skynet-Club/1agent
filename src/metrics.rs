//! Deterministic latency and provider health summaries.
//!
//! These are pure functions over samples/reports so they can run in tests and
//! offline evaluation without a metrics server. Percentiles use nearest-rank.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::domain::{StopReason, ToolStatus};
use crate::journal::{JournalEvent, JournalEventKind, read_events, replay_events};
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

/// Local 0.97 gate metrics derived from run journals.
///
/// These are structural measurements: replay is a journal replay, not a new
/// model run, and audit completeness uses `AgentAnswer::validate`. They are
/// suitable for a local dashboard and regression checks, not for external
/// performance claims.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct JournalMetricsReport {
    pub journals: u64,
    pub runs: u64,
    pub completed: u64,
    pub terminal_stops: u64,
    pub interrupted: u64,
    pub resumes: u64,
    pub resumed_completed: u64,
    pub recovery_bps: Option<u16>,
    pub replay_total: u64,
    pub replay_ok: u64,
    pub replay_bps: Option<u16>,
    pub compacted_runs: u64,
    pub compacted_replay_ok: u64,
    pub compacted_replay_bps: Option<u16>,
    pub tool_calls: u64,
    pub tool_ok: u64,
    pub tool_success_bps: Option<u16>,
    pub denied_observations: u64,
    pub audit_total: u64,
    pub audit_complete: u64,
    pub audit_complete_bps: Option<u16>,
    pub context_compactions: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_micros: u64,
    pub run_latency: Option<LatencyDigest>,
}

/// Summarize already-parsed journals. Each slice is one run attempt.
pub fn summarize_journal_events<'a>(
    journals: impl IntoIterator<Item = &'a [JournalEvent]>,
) -> JournalMetricsReport {
    let mut report = JournalMetricsReport::default();
    let mut latencies = Vec::new();
    for events in journals {
        if events.is_empty() {
            continue;
        }
        report.journals += 1;
        report.runs += 1;
        let request = events.iter().find_map(|event| match &event.event {
            JournalEventKind::RunStarted { request, .. }
            | JournalEventKind::ResumeStarted { request, .. } => Some(request),
            _ => None,
        });
        let resumed = events
            .iter()
            .any(|event| matches!(event.event, JournalEventKind::ResumeStarted { .. }));
        if resumed {
            report.resumes += 1;
        }
        let mut stop_reason = None;
        let mut answer = None;
        let mut compacted = false;
        for event in events {
            match &event.event {
                JournalEventKind::ToolCalled { .. } => report.tool_calls += 1,
                JournalEventKind::ToolObserved { observation } => {
                    if observation.status == ToolStatus::Ok {
                        report.tool_ok += 1;
                    }
                    if observation.status == ToolStatus::PermissionDenied
                        || matches!(
                            observation.error_code.as_deref(),
                            Some("authority_denied" | "write_not_allowed")
                        )
                    {
                        report.denied_observations += 1;
                    }
                }
                JournalEventKind::ModelResponded { response } => {
                    report.input_tokens = report
                        .input_tokens
                        .saturating_add(response.usage.input_tokens);
                    report.output_tokens = report
                        .output_tokens
                        .saturating_add(response.usage.output_tokens);
                    report.cost_micros = report
                        .cost_micros
                        .saturating_add(response.usage.cost_micros);
                }
                JournalEventKind::ContextCompacted { .. } => {
                    compacted = true;
                    report.context_compactions += 1;
                }
                JournalEventKind::AnswerProduced { answer: produced } => {
                    answer = Some(produced);
                }
                JournalEventKind::RunStopped { reason } => {
                    stop_reason = Some(*reason);
                    report.terminal_stops += 1;
                }
                _ => {}
            }
        }
        let completed = stop_reason == Some(StopReason::Completed);
        if completed {
            report.completed += 1;
            if resumed {
                report.resumed_completed += 1;
            }
            report.replay_total += 1;
            let replay_ok = replay_events(events).is_ok();
            if replay_ok {
                report.replay_ok += 1;
            }
            if compacted {
                report.compacted_runs += 1;
                if replay_ok {
                    report.compacted_replay_ok += 1;
                }
            }
        }
        if let (Some(answer), Some(request)) = (answer, request) {
            report.audit_total += 1;
            if answer.validate(request).is_ok() {
                report.audit_complete += 1;
            }
        }
        if let (Some(first), Some(last)) = (events.first(), events.last())
            && last.timestamp_unix_ms >= first.timestamp_unix_ms
        {
            latencies.push(
                last.timestamp_unix_ms
                    .saturating_sub(first.timestamp_unix_ms)
                    .min(u128::from(u64::MAX)) as u64,
            );
        }
    }
    report.interrupted = report.runs.saturating_sub(report.terminal_stops);
    report.recovery_bps = ratio_bps(report.resumed_completed, report.resumes);
    report.replay_bps = ratio_bps(report.replay_ok, report.replay_total);
    report.compacted_replay_bps = ratio_bps(report.compacted_replay_ok, report.compacted_runs);
    report.tool_success_bps = ratio_bps(report.tool_ok, report.tool_calls);
    report.audit_complete_bps = ratio_bps(report.audit_complete, report.audit_total);
    report.run_latency = LatencyDigest::from_samples(&latencies);
    report
}

/// Read every `*.jsonl` file directly under `dir` and summarize it.
pub fn summarize_journal_dir(dir: &Path) -> Result<JournalMetricsReport, String> {
    let mut paths = std::fs::read_dir(dir)
        .map_err(|error| format!("{}: {error}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension == "jsonl")
        })
        .collect::<Vec<_>>();
    paths.sort();
    let mut journals = Vec::with_capacity(paths.len());
    for path in paths {
        let events = read_events(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        journals.push(events);
    }
    Ok(summarize_journal_events(
        journals.iter().map(|events| events.as_slice()),
    ))
}

fn ratio_bps(part: u64, total: u64) -> Option<u16> {
    if total == 0 {
        return None;
    }
    Some((part.saturating_mul(10_000) / total).min(10_000) as u16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        AgentAnswer, AnswerStatus, Claim, ClaimStatus, CognitiveMode, ModelResponse, ModelTurn,
        RuntimeCheckpoint, TaskRequest, ToolObservation,
    };
    use crate::journal::Journal;
    use crate::provider::{ProviderProbeReport, ProviderProbeResult};
    use std::path::PathBuf;
    use uuid::Uuid;

    fn task() -> TaskRequest {
        TaskRequest::from_objective("metrics", PathBuf::from("."))
    }

    fn valid_answer() -> AgentAnswer {
        AgentAnswer {
            schema_version: crate::domain::ANSWER_SCHEMA_VERSION,
            status: AnswerStatus::Completed,
            conclusion: "done".into(),
            claims: vec![Claim {
                id: "claim-1".into(),
                text: "verified".into(),
                status: ClaimStatus::Verified,
                sources: vec!["fixture".into()],
                scope: None,
            }],
            evidence: Vec::new(),
            assumptions: Vec::new(),
            constraints: Vec::new(),
            unknowns: Vec::new(),
            conflicts: Vec::new(),
            flip_conditions: Vec::new(),
            action_receipts: Vec::new(),
        }
    }

    fn invalid_answer() -> AgentAnswer {
        let mut answer = valid_answer();
        answer.claims[0].sources.clear();
        answer
    }

    fn observation(status: ToolStatus) -> ToolObservation {
        ToolObservation {
            call_id: "call-1".into(),
            tool: "echo".into(),
            status,
            summary: "summary".into(),
            content: "content".into(),
            error_code: None,
            metadata: Default::default(),
        }
    }

    fn journal_with(extras: Vec<JournalEventKind>) -> Vec<JournalEvent> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let mut journal = Journal::create(&path, Uuid::new_v4()).unwrap();
        journal
            .append(JournalEventKind::RunStarted {
                request: task(),
                mode: CognitiveMode::DirectAnswer,
            })
            .unwrap();
        for extra in extras {
            journal.append(extra).unwrap();
        }
        drop(journal);
        read_events(&path).unwrap()
    }

    fn resumed_journal_with(extras: Vec<JournalEventKind>) -> Vec<JournalEvent> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let mut journal = Journal::create(&path, Uuid::new_v4()).unwrap();
        journal
            .append(JournalEventKind::ResumeStarted {
                original_run_id: Uuid::new_v4(),
                checkpoint_seq: 0,
                request: task(),
                checkpoint: RuntimeCheckpoint {
                    mode: CognitiveMode::DirectAnswer,
                    messages: Vec::new(),
                    tool_calls_used: 0,
                    provider_attempts_used: 0,
                    unavailable_routes: Vec::new(),
                    next_step: 0,
                    elapsed_ms: 0,
                    route_plan_sha256: None,
                    route_policy_sources: None,
                    route_objective: None,
                    tool_capabilities_sha256: None,
                },
            })
            .unwrap();
        for extra in extras {
            journal.append(extra).unwrap();
        }
        drop(journal);
        read_events(&path).unwrap()
    }

    fn response() -> ModelResponse {
        ModelResponse {
            provider_id: "p".into(),
            model: "m".into(),
            turn: ModelTurn::Final {
                answer: valid_answer(),
            },
            usage: crate::domain::Usage {
                input_tokens: 10,
                output_tokens: 5,
                cost_micros: 100,
                ..Default::default()
            },
        }
    }

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

    #[test]
    fn journal_metrics_count_completion_tools_and_audit() {
        let events = journal_with(vec![
            JournalEventKind::ToolCalled {
                call: crate::domain::ToolCall {
                    id: "call-1".into(),
                    name: "echo".into(),
                    arguments: serde_json::json!({"text": "x"}),
                },
            },
            JournalEventKind::ToolObserved {
                observation: observation(ToolStatus::Ok),
            },
            JournalEventKind::ModelResponded {
                response: response(),
            },
            JournalEventKind::AnswerProduced {
                answer: valid_answer(),
            },
            JournalEventKind::RunStopped {
                reason: StopReason::Completed,
            },
        ]);
        let report = summarize_journal_events([events.as_slice()]);
        assert_eq!(report.journals, 1);
        assert_eq!(report.runs, 1);
        assert_eq!(report.completed, 1);
        assert_eq!(report.terminal_stops, 1);
        assert_eq!(report.interrupted, 0);
        assert_eq!(report.tool_calls, 1);
        assert_eq!(report.tool_ok, 1);
        assert_eq!(report.tool_success_bps, Some(10_000));
        assert_eq!(report.audit_total, 1);
        assert_eq!(report.audit_complete, 1);
        assert_eq!(report.audit_complete_bps, Some(10_000));
        assert_eq!(report.replay_total, 1);
        assert_eq!(report.replay_bps, Some(10_000));
        assert_eq!(report.input_tokens, 10);
        assert_eq!(report.output_tokens, 5);
        assert_eq!(report.cost_micros, 100);
        assert!(report.run_latency.is_some());
    }

    #[test]
    fn journal_metrics_count_resumes_and_compacted_replays() {
        let events = resumed_journal_with(vec![
            JournalEventKind::ContextCompacted {
                before_tokens: 100,
                after_tokens: 50,
                before_chars: 400,
                after_chars: 200,
                compacted_tool_results: 1,
            },
            JournalEventKind::AnswerProduced {
                answer: valid_answer(),
            },
            JournalEventKind::RunStopped {
                reason: StopReason::Completed,
            },
        ]);
        let report = summarize_journal_events([events.as_slice()]);
        assert_eq!(report.resumes, 1);
        assert_eq!(report.resumed_completed, 1);
        assert_eq!(report.recovery_bps, Some(10_000));
        assert_eq!(report.context_compactions, 1);
        assert_eq!(report.compacted_runs, 1);
        assert_eq!(report.compacted_replay_ok, 1);
        assert_eq!(report.compacted_replay_bps, Some(10_000));
    }

    #[test]
    fn journal_metrics_keep_failures_and_invalid_answers_visible() {
        let events = journal_with(vec![
            JournalEventKind::ToolCalled {
                call: crate::domain::ToolCall {
                    id: "call-1".into(),
                    name: "write_file".into(),
                    arguments: serde_json::json!({}),
                },
            },
            JournalEventKind::ToolObserved {
                observation: observation(ToolStatus::PermissionDenied),
            },
            JournalEventKind::AnswerProduced {
                answer: invalid_answer(),
            },
        ]);
        let report = summarize_journal_events([events.as_slice()]);
        assert_eq!(report.completed, 0);
        assert_eq!(report.interrupted, 1);
        assert_eq!(report.tool_success_bps, Some(0));
        assert_eq!(report.denied_observations, 1);
        assert_eq!(report.audit_total, 1);
        assert_eq!(report.audit_complete_bps, Some(0));
        assert_eq!(report.replay_bps, None);
    }

    #[test]
    fn empty_journal_metrics_are_zero() {
        let report = summarize_journal_events(std::iter::empty());
        assert_eq!(report, JournalMetricsReport::default());
    }

    #[test]
    fn journal_dir_reads_only_jsonl_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut first = Journal::create(dir.path().join("a.jsonl"), Uuid::new_v4()).unwrap();
        first
            .append(JournalEventKind::RunStarted {
                request: task(),
                mode: CognitiveMode::DirectAnswer,
            })
            .unwrap();
        first
            .append(JournalEventKind::RunStopped {
                reason: StopReason::Completed,
            })
            .unwrap();
        drop(first);
        let mut second = Journal::create(dir.path().join("b.jsonl"), Uuid::new_v4()).unwrap();
        second
            .append(JournalEventKind::RunStarted {
                request: task(),
                mode: CognitiveMode::DirectAnswer,
            })
            .unwrap();
        drop(second);
        std::fs::write(dir.path().join("notes.txt"), "ignored").unwrap();
        let report = summarize_journal_dir(dir.path()).unwrap();
        assert_eq!(report.journals, 2);
        assert_eq!(report.runs, 2);
        assert_eq!(report.completed, 1);
        assert_eq!(report.interrupted, 1);
    }
}
