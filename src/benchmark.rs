//! Deterministic benchmark gate for long-running agent windows.
//!
//! The gate summarizes caller-supplied samples; it does not run the benchmark
//! itself and does not infer a task's quality. A sample marked `crashed` is
//! never counted as completed or replay-verified.

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MAX_BENCHMARK_SAMPLES: usize = 1_000_000;
pub const MAX_BENCHMARK_INPUT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BenchmarkError {
    #[error("benchmark requires at least one sample")]
    Empty,
    #[error("benchmark has {0} samples (limit {MAX_BENCHMARK_SAMPLES})")]
    TooManySamples(usize),
    #[error("benchmark threshold must be 0..=10000 bps")]
    InvalidThreshold(u16),
    #[error("benchmark sample line {line} is invalid: {message}")]
    InvalidSample { line: usize, message: String },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkSample {
    pub completed: bool,
    pub crashed: bool,
    pub replay_verified: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkPolicy {
    pub min_completion_bps: u16,
    pub min_crash_free_bps: u16,
    pub min_replay_bps: u16,
}

impl Default for BenchmarkPolicy {
    fn default() -> Self {
        Self {
            min_completion_bps: 7_000,
            min_crash_free_bps: 9_950,
            min_replay_bps: 10_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkReport {
    pub iterations: u64,
    pub completed: u64,
    pub crashed: u64,
    pub replay_verified: u64,
    pub completion_bps: u16,
    pub crash_free_bps: u16,
    pub replay_bps: u16,
    pub passed: bool,
}

pub fn summarize(
    samples: &[BenchmarkSample],
    policy: BenchmarkPolicy,
) -> Result<BenchmarkReport, BenchmarkError> {
    if samples.is_empty() {
        return Err(BenchmarkError::Empty);
    }
    if samples.len() > MAX_BENCHMARK_SAMPLES {
        return Err(BenchmarkError::TooManySamples(samples.len()));
    }
    for threshold in [
        policy.min_completion_bps,
        policy.min_crash_free_bps,
        policy.min_replay_bps,
    ] {
        if threshold > 10_000 {
            return Err(BenchmarkError::InvalidThreshold(threshold));
        }
    }
    let iterations = samples.len() as u64;
    let completed = samples
        .iter()
        .filter(|sample| sample.completed && !sample.crashed)
        .count() as u64;
    let crashed = samples.iter().filter(|sample| sample.crashed).count() as u64;
    let replay_verified = samples
        .iter()
        .filter(|sample| sample.replay_verified && !sample.crashed)
        .count() as u64;
    let completion_bps = bps(completed, iterations);
    let crash_free_bps = bps(iterations - crashed, iterations);
    let replay_bps = bps(replay_verified, iterations);
    Ok(BenchmarkReport {
        iterations,
        completed,
        crashed,
        replay_verified,
        completion_bps,
        crash_free_bps,
        replay_bps,
        passed: completion_bps >= policy.min_completion_bps
            && crash_free_bps >= policy.min_crash_free_bps
            && replay_bps >= policy.min_replay_bps,
    })
}

/// Parse one JSON object per non-empty line. Unknown fields and malformed
/// lines fail closed with the source line number.
pub fn parse_samples(raw: &str) -> Result<Vec<BenchmarkSample>, BenchmarkError> {
    let mut samples = Vec::new();
    for (index, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let sample = serde_json::from_str::<BenchmarkSample>(line).map_err(|error| {
            BenchmarkError::InvalidSample {
                line: index + 1,
                message: error.to_string(),
            }
        })?;
        samples.push(sample);
        if samples.len() > MAX_BENCHMARK_SAMPLES {
            return Err(BenchmarkError::TooManySamples(samples.len()));
        }
    }
    if samples.is_empty() {
        return Err(BenchmarkError::Empty);
    }
    Ok(samples)
}

fn bps(part: u64, total: u64) -> u16 {
    ((part * 10_000) / total) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(completed: bool, crashed: bool, replay_verified: bool) -> BenchmarkSample {
        BenchmarkSample {
            completed,
            crashed,
            replay_verified,
        }
    }

    #[test]
    fn a_clean_window_passes_all_thresholds() {
        let report = summarize(
            &[
                sample(true, false, true),
                sample(true, false, true),
                sample(true, false, true),
            ],
            BenchmarkPolicy::default(),
        )
        .unwrap();
        assert_eq!(report.iterations, 3);
        assert_eq!(report.completed, 3);
        assert_eq!(report.crashed, 0);
        assert_eq!(report.replay_verified, 3);
        assert_eq!(report.completion_bps, 10_000);
        assert_eq!(report.crash_free_bps, 10_000);
        assert_eq!(report.replay_bps, 10_000);
        assert!(report.passed);
    }

    #[test]
    fn a_crash_is_never_counted_as_completed_or_replayed() {
        let report = summarize(
            &[
                sample(true, true, true),
                sample(true, false, true),
                sample(false, false, false),
            ],
            BenchmarkPolicy::default(),
        )
        .unwrap();
        assert_eq!(report.iterations, 3);
        assert_eq!(report.completed, 1);
        assert_eq!(report.crashed, 1);
        assert_eq!(report.replay_verified, 1);
        assert_eq!(report.completion_bps, 3_333);
        assert_eq!(report.crash_free_bps, 6_666);
        assert_eq!(report.replay_bps, 3_333);
        assert!(!report.passed);
    }

    #[test]
    fn thresholds_are_checked_independently() {
        let samples = [
            sample(true, false, true),
            sample(true, false, false),
            sample(false, false, false),
        ];
        let completion_only = summarize(
            &samples,
            BenchmarkPolicy {
                min_completion_bps: 6_666,
                min_crash_free_bps: 10_000,
                min_replay_bps: 0,
            },
        )
        .unwrap();
        assert!(completion_only.passed);
        let replay_fails = summarize(
            &samples,
            BenchmarkPolicy {
                min_completion_bps: 0,
                min_crash_free_bps: 0,
                min_replay_bps: 6_667,
            },
        )
        .unwrap();
        assert!(!replay_fails.passed);
    }

    #[test]
    fn jsonl_samples_parse_and_fail_with_line_numbers() {
        let raw = concat!(
            "{\"completed\":true,\"crashed\":false,\"replay_verified\":true}\n",
            "\n",
            "{\"completed\":false,\"crashed\":false,\"replay_verified\":false}\n"
        );
        let samples = parse_samples(raw).unwrap();
        assert_eq!(samples.len(), 2);
        assert!(samples[0].completed);
        assert!(!samples[1].replay_verified);

        let malformed = parse_samples("{\"completed\":true}\n");
        assert!(matches!(
            malformed,
            Err(BenchmarkError::InvalidSample { line: 1, .. })
        ));
        assert_eq!(parse_samples("\n\n").unwrap_err(), BenchmarkError::Empty);
    }

    #[test]
    fn empty_oversized_and_invalid_thresholds_fail_closed() {
        assert_eq!(
            summarize(&[], BenchmarkPolicy::default()).unwrap_err(),
            BenchmarkError::Empty
        );

        let oversized = vec![sample(true, false, true); MAX_BENCHMARK_SAMPLES + 1];
        assert_eq!(
            summarize(&oversized, BenchmarkPolicy::default()).unwrap_err(),
            BenchmarkError::TooManySamples(MAX_BENCHMARK_SAMPLES + 1)
        );

        let invalid = BenchmarkPolicy {
            min_completion_bps: 10_001,
            ..BenchmarkPolicy::default()
        };
        assert_eq!(
            summarize(&[sample(true, false, true)], invalid).unwrap_err(),
            BenchmarkError::InvalidThreshold(10_001)
        );
    }
}
