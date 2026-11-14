//! M19–M23 answer-quality board over caller-supplied blind-review samples.
//!
//! This module computes rates from judgments; it does not run a blind review
//! or judge answers. A sample is one fixed prompt/answer pair.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AnswerQualityError {
    #[error("answer-quality line {line} is invalid: {source}")]
    Json {
        line: usize,
        #[source]
        source: serde_json::Error,
    },
    #[error("answer-quality line {line}: a follow-up cannot be adopted when none occurred")]
    AdoptedWithoutFollowup { line: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AnswerQualitySample {
    pub first_answer_usable: bool,
    /// `Some(true)` is a win, `Some(false)` a loss, `None` a tie or no
    /// pairwise comparison.
    #[serde(default)]
    pub pairwise_win: Option<bool>,
    #[serde(default)]
    pub over_structured: bool,
    #[serde(default)]
    pub unnecessary_followup: bool,
    #[serde(default)]
    pub followup: bool,
    #[serde(default)]
    pub followup_adopted: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AnswerQualityReport {
    pub samples: u64,
    /// M19: first answer usable.
    pub first_answer_usable_bps: Option<u16>,
    pub pairwise_compared: u64,
    pub pairwise_wins: u64,
    /// M20: pairwise win rate, ties excluded.
    pub pairwise_win_bps: Option<u16>,
    /// M21: over-structured rate.
    pub over_structured_bps: Option<u16>,
    /// M22: unnecessary follow-up rate.
    pub unnecessary_followup_bps: Option<u16>,
    pub followups: u64,
    pub followup_adopted: u64,
    /// M23: follow-up adoption rate.
    pub followup_adoption_bps: Option<u16>,
}

pub fn summarize_answer_quality(samples: &[AnswerQualitySample]) -> AnswerQualityReport {
    let mut report = AnswerQualityReport {
        samples: samples.len() as u64,
        ..AnswerQualityReport::default()
    };
    if samples.is_empty() {
        return report;
    }
    let mut usable = 0u64;
    let mut over_structured = 0u64;
    let mut unnecessary = 0u64;
    let mut pairwise_wins = 0u64;
    let mut pairwise_losses = 0u64;
    let mut followups = 0u64;
    let mut adopted = 0u64;
    for sample in samples {
        if sample.first_answer_usable {
            usable += 1;
        }
        if sample.over_structured {
            over_structured += 1;
        }
        if sample.unnecessary_followup {
            unnecessary += 1;
        }
        match sample.pairwise_win {
            Some(true) => pairwise_wins += 1,
            Some(false) => pairwise_losses += 1,
            None => {}
        }
        if sample.followup {
            followups += 1;
            if sample.followup_adopted {
                adopted += 1;
            }
        }
    }
    report.first_answer_usable_bps = ratio_bps(usable, report.samples);
    report.pairwise_compared = pairwise_wins + pairwise_losses;
    report.pairwise_wins = pairwise_wins;
    report.pairwise_win_bps = ratio_bps(pairwise_wins, report.pairwise_compared);
    report.over_structured_bps = ratio_bps(over_structured, report.samples);
    report.unnecessary_followup_bps = ratio_bps(unnecessary, report.samples);
    report.followups = followups;
    report.followup_adopted = adopted;
    report.followup_adoption_bps = ratio_bps(adopted, followups);
    report
}

/// Parse JSONL samples and reject a follow-up adoption with no follow-up.
pub fn parse_answer_quality_samples(
    text: &str,
) -> Result<Vec<AnswerQualitySample>, AnswerQualityError> {
    let mut samples = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        if line.trim().is_empty() {
            continue;
        }
        let sample: AnswerQualitySample =
            serde_json::from_str(line).map_err(|source| AnswerQualityError::Json {
                line: line_number,
                source,
            })?;
        if sample.followup_adopted && !sample.followup {
            return Err(AnswerQualityError::AdoptedWithoutFollowup { line: line_number });
        }
        samples.push(sample);
    }
    Ok(samples)
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

    fn sample(
        usable: bool,
        win: Option<bool>,
        over: bool,
        unnecessary: bool,
        followup: bool,
        adopted: bool,
    ) -> AnswerQualitySample {
        AnswerQualitySample {
            first_answer_usable: usable,
            pairwise_win: win,
            over_structured: over,
            unnecessary_followup: unnecessary,
            followup,
            followup_adopted: adopted,
        }
    }

    #[test]
    fn rates_compute_m19_to_m23() {
        let report = summarize_answer_quality(&[
            sample(true, Some(true), false, false, false, false),
            sample(true, Some(false), true, true, true, true),
            sample(false, None, false, false, true, false),
            sample(true, None, false, false, false, false),
        ]);
        assert_eq!(report.samples, 4);
        assert_eq!(report.first_answer_usable_bps, Some(7_500));
        assert_eq!(report.pairwise_compared, 2);
        assert_eq!(report.pairwise_wins, 1);
        assert_eq!(report.pairwise_win_bps, Some(5_000));
        assert_eq!(report.over_structured_bps, Some(2_500));
        assert_eq!(report.unnecessary_followup_bps, Some(2_500));
        assert_eq!(report.followups, 2);
        assert_eq!(report.followup_adopted, 1);
        assert_eq!(report.followup_adoption_bps, Some(5_000));
    }

    #[test]
    fn ties_are_excluded_from_the_pairwise_win_rate() {
        let report = summarize_answer_quality(&[
            sample(true, None, false, false, false, false),
            sample(true, None, false, false, false, false),
        ]);
        assert_eq!(report.pairwise_compared, 0);
        assert_eq!(report.pairwise_win_bps, None);
    }

    #[test]
    fn empty_samples_have_null_rates() {
        let report = summarize_answer_quality(&[]);
        assert_eq!(report.samples, 0);
        assert_eq!(report.first_answer_usable_bps, None);
        assert_eq!(report.over_structured_bps, None);
        assert_eq!(report.followup_adoption_bps, None);
    }

    #[test]
    fn parse_reports_line_numbers() {
        let text = "{\"first_answer_usable\":true}\n{\"first_answer_usable\":\"yes\"}\n";
        let error = parse_answer_quality_samples(text).unwrap_err();
        match error {
            AnswerQualityError::Json { line, .. } => assert_eq!(line, 2),
            other => panic!("expected a JSON error, got {other:?}"),
        }
    }

    #[test]
    fn parse_rejects_adoption_without_a_followup() {
        let text = "{\"first_answer_usable\":true,\"followup_adopted\":true}\n";
        assert!(matches!(
            parse_answer_quality_samples(text),
            Err(AnswerQualityError::AdoptedWithoutFollowup { line: 1 })
        ));
    }
}
