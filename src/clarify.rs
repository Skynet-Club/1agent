//! Clarify gate: structured questions, option normalization and admission.
//!
//! This module owns policy only. It does not render a panel or read a
//! terminal; a frontend submits questions to the gate and asks the admitted
//! ones. The automatic `Other` option and the recommended-first order are
//! enforced here so every frontend presents the same shape.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MAX_CLARIFY_OPTIONS: usize = 4;
pub const MAX_BLOCKING_QUESTIONS: usize = 5;
pub const DEFAULT_QUESTIONS_PER_TURN: usize = 1;
pub const DEFAULT_TIMEOUT_MS: u64 = 300_000;
pub const OTHER_OPTION_ID: &str = "other";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ClarifyError {
    #[error("clarify question id is invalid: {0}")]
    InvalidQuestionId(String),
    #[error("clarify prompt must be 1..=500 characters without control characters")]
    InvalidPrompt,
    #[error("clarify question needs 2..=4 options")]
    InvalidOptionCount,
    #[error("clarify option id is invalid: {0}")]
    InvalidOptionId(String),
    #[error("clarify option label must be 1..=120 characters without control characters")]
    InvalidOptionLabel,
    #[error("clarify option ids must be unique")]
    DuplicateOptionId,
    #[error("clarify question allows at most one recommended option")]
    MultipleRecommended,
    #[error("clarify batch has more than five blocking questions")]
    TooManyBlocking,
    #[error("clarify answer references an unknown question: {0}")]
    UnknownQuestionId(String),
    #[error("clarify question has more than one answer: {0}")]
    DuplicateAnswer(String),
    #[error("an answered question must carry an option id: {0}")]
    AnsweredWithoutOption(String),
    #[error("clarify answer references an unknown option: {0}")]
    UnknownOptionId(String),
    #[error("a skipped or unanswered question must not carry an option id: {0}")]
    UnexpectedOption(String),
    #[error("submitted round has no answered question")]
    EmptySubmission,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClarifyAnswerStatus {
    Answered,
    Skipped,
    Unanswered,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClarifyOutcome {
    Submitted,
    Cancelled,
    TimedOut,
    Undelivered,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ClarifyAnswer {
    pub question_id: String,
    #[serde(default)]
    pub option_id: Option<String>,
    pub status: ClarifyAnswerStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ClarifyRoundSummary {
    pub outcome: ClarifyOutcome,
    pub answered: usize,
    pub skipped: usize,
    pub unanswered: usize,
    pub blocking_unanswered: usize,
    /// An unanswered question never invalidates the default answer.
    pub default_answer_valid: bool,
}

/// Validate a submitted, cancelled, timed-out or undelivered round against
/// the admitted questions. Unanswered questions leave the default answer
/// valid; only an answer that names an unknown question/option fails closed.
pub fn resolve_round(
    questions: &[ClarifyQuestion],
    answers: &[ClarifyAnswer],
    outcome: ClarifyOutcome,
) -> Result<ClarifyRoundSummary, ClarifyError> {
    let mut by_id = std::collections::BTreeMap::new();
    for question in questions {
        if by_id.insert(question.id.as_str(), question).is_some() {
            return Err(ClarifyError::InvalidQuestionId(question.id.clone()));
        }
    }
    let mut seen = BTreeSet::new();
    let mut answered = 0;
    let mut skipped = 0;
    let mut unanswered = 0;
    for answer in answers {
        let question = by_id
            .get(answer.question_id.as_str())
            .ok_or_else(|| ClarifyError::UnknownQuestionId(answer.question_id.clone()))?;
        if !seen.insert(answer.question_id.as_str()) {
            return Err(ClarifyError::DuplicateAnswer(answer.question_id.clone()));
        }
        match answer.status {
            ClarifyAnswerStatus::Answered => {
                let option_id = answer.option_id.as_ref().ok_or_else(|| {
                    ClarifyError::AnsweredWithoutOption(answer.question_id.clone())
                })?;
                if !question
                    .options
                    .iter()
                    .any(|option| option.id == *option_id)
                {
                    return Err(ClarifyError::UnknownOptionId(option_id.clone()));
                }
                answered += 1;
            }
            ClarifyAnswerStatus::Skipped => {
                if answer.option_id.is_some() {
                    return Err(ClarifyError::UnexpectedOption(answer.question_id.clone()));
                }
                skipped += 1;
            }
            ClarifyAnswerStatus::Unanswered => {
                if answer.option_id.is_some() {
                    return Err(ClarifyError::UnexpectedOption(answer.question_id.clone()));
                }
                unanswered += 1;
            }
        }
    }
    let missing = questions
        .iter()
        .filter(|question| !seen.contains(question.id.as_str()))
        .count();
    unanswered += missing;
    let blocking_unanswered = questions
        .iter()
        .filter(|question| {
            question.blocking
                && !answers.iter().any(|answer| {
                    answer.question_id == question.id
                        && answer.status == ClarifyAnswerStatus::Answered
                })
        })
        .count();
    if outcome == ClarifyOutcome::Submitted && answered == 0 {
        return Err(ClarifyError::EmptySubmission);
    }
    Ok(ClarifyRoundSummary {
        outcome,
        answered,
        skipped,
        unanswered,
        blocking_unanswered,
        default_answer_valid: true,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ClarifyOption {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub recommended: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ClarifyQuestion {
    pub id: String,
    pub prompt: String,
    pub options: Vec<ClarifyOption>,
    #[serde(default)]
    pub blocking: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum ClarifyDecision {
    Ask {
        questions: Vec<ClarifyQuestion>,
        timeout_ms: u64,
    },
    Skip {
        reason: String,
    },
}

impl ClarifyQuestion {
    pub fn validate(&self) -> Result<(), ClarifyError> {
        if !safe_id(&self.id) {
            return Err(ClarifyError::InvalidQuestionId(self.id.clone()));
        }
        if self.prompt.trim().is_empty()
            || self.prompt.chars().count() > 500
            || self
                .prompt
                .chars()
                .any(crate::safety::is_disallowed_control)
        {
            return Err(ClarifyError::InvalidPrompt);
        }
        if !(2..=MAX_CLARIFY_OPTIONS).contains(&self.options.len()) {
            return Err(ClarifyError::InvalidOptionCount);
        }
        let mut ids = BTreeSet::new();
        let mut recommended = 0;
        for option in &self.options {
            if !safe_id(&option.id) {
                return Err(ClarifyError::InvalidOptionId(option.id.clone()));
            }
            if !ids.insert(option.id.as_str()) {
                return Err(ClarifyError::DuplicateOptionId);
            }
            if option.label.trim().is_empty()
                || option.label.chars().count() > 120
                || option
                    .label
                    .chars()
                    .any(crate::safety::is_disallowed_control)
            {
                return Err(ClarifyError::InvalidOptionLabel);
            }
            if option.recommended {
                recommended += 1;
            }
        }
        if recommended > 1 {
            return Err(ClarifyError::MultipleRecommended);
        }
        Ok(())
    }

    /// Add the automatic `Other` option when absent and move the recommended
    /// option first. Validation must run before normalization.
    pub fn normalize(mut self) -> Self {
        if !self
            .options
            .iter()
            .any(|option| option.id == OTHER_OPTION_ID)
        {
            self.options.push(ClarifyOption {
                id: OTHER_OPTION_ID.into(),
                label: "Other".into(),
                recommended: false,
            });
        }
        self.options.sort_by_key(|option| !option.recommended);
        self
    }
}

#[derive(Debug, Clone)]
pub struct ClarifyGate {
    seen_ids: BTreeSet<String>,
    seen_prompts: BTreeSet<String>,
    asked: usize,
    budget: usize,
    per_turn: usize,
}

impl Default for ClarifyGate {
    fn default() -> Self {
        Self::new(0)
    }
}

impl ClarifyGate {
    /// `budget` is the total number of questions the gate may admit; `0` means
    /// unlimited. One question per turn is the default.
    pub fn new(budget: usize) -> Self {
        Self {
            seen_ids: BTreeSet::new(),
            seen_prompts: BTreeSet::new(),
            asked: 0,
            budget,
            per_turn: DEFAULT_QUESTIONS_PER_TURN,
        }
    }

    pub fn with_per_turn(mut self, per_turn: usize) -> Self {
        self.per_turn = per_turn.max(1);
        self
    }

    pub fn asked(&self) -> usize {
        self.asked
    }

    /// Validate the whole batch first, then admit unseen questions up to the
    /// per-turn and total budget. Rejected duplicates do not consume budget.
    pub fn admit(
        &mut self,
        questions: Vec<ClarifyQuestion>,
    ) -> Result<ClarifyDecision, ClarifyError> {
        for question in &questions {
            question.validate()?;
        }
        let blocking = questions
            .iter()
            .filter(|question| question.blocking)
            .count();
        if blocking > MAX_BLOCKING_QUESTIONS {
            return Err(ClarifyError::TooManyBlocking);
        }

        let mut admitted = Vec::new();
        for question in questions {
            if admitted.len() >= self.per_turn {
                break;
            }
            let prompt_key = normalize_prompt(&question.prompt);
            if self.seen_ids.contains(&question.id) || self.seen_prompts.contains(&prompt_key) {
                continue;
            }
            self.seen_ids.insert(question.id.clone());
            self.seen_prompts.insert(prompt_key);
            admitted.push(question.normalize());
        }
        if admitted.is_empty() {
            return Ok(ClarifyDecision::Skip {
                reason: "no new questions to ask".into(),
            });
        }
        if self.budget != 0 {
            let remaining = self.budget.saturating_sub(self.asked);
            if remaining == 0 {
                return Ok(ClarifyDecision::Skip {
                    reason: "question budget exhausted".into(),
                });
            }
            admitted.truncate(remaining);
        }
        self.asked += admitted.len();
        Ok(ClarifyDecision::Ask {
            questions: admitted,
            timeout_ms: DEFAULT_TIMEOUT_MS,
        })
    }
}

fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= 64
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
}

fn normalize_prompt(prompt: &str) -> String {
    prompt
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn option(id: &str, label: &str, recommended: bool) -> ClarifyOption {
        ClarifyOption {
            id: id.into(),
            label: label.into(),
            recommended,
        }
    }

    fn question(id: &str, prompt: &str) -> ClarifyQuestion {
        ClarifyQuestion {
            id: id.into(),
            prompt: prompt.into(),
            options: vec![
                option("a", "Option A", false),
                option("b", "Option B", true),
            ],
            blocking: false,
        }
    }

    #[test]
    fn normalization_adds_other_and_puts_the_recommendation_first() {
        let normalized = question("q1", "Which one?").normalize();
        assert_eq!(normalized.options[0].id, "b");
        assert!(normalized.options[0].recommended);
        assert_eq!(normalized.options.last().unwrap().id, OTHER_OPTION_ID);
    }

    #[test]
    fn gate_admits_one_question_per_turn_and_deduplicates() {
        let mut gate = ClarifyGate::new(0);
        let first = gate
            .admit(vec![
                question("q1", "Which one?"),
                question("q2", "What next?"),
            ])
            .unwrap();
        match first {
            ClarifyDecision::Ask { questions, .. } => {
                assert_eq!(questions.len(), 1);
                assert_eq!(questions[0].id, "q1");
            }
            other => panic!("expected ask, got {other:?}"),
        }
        let repeat = gate.admit(vec![question("q1", "  WHICH   one? ")]).unwrap();
        assert!(matches!(repeat, ClarifyDecision::Skip { .. }));
        let next = gate.admit(vec![question("q2", "What next?")]).unwrap();
        match next {
            ClarifyDecision::Ask { questions, .. } => assert_eq!(questions[0].id, "q2"),
            other => panic!("expected ask, got {other:?}"),
        }
    }

    #[test]
    fn gate_honors_the_total_budget() {
        let mut gate = ClarifyGate::new(1);
        assert!(matches!(
            gate.admit(vec![question("q1", "Which one?")]).unwrap(),
            ClarifyDecision::Ask { .. }
        ));
        assert!(matches!(
            gate.admit(vec![question("q2", "What next?")]).unwrap(),
            ClarifyDecision::Skip { .. }
        ));
        assert_eq!(gate.asked(), 1);
    }

    #[test]
    fn blocking_batches_are_capped_at_five() {
        let mut gate = ClarifyGate::new(0).with_per_turn(5);
        let questions = (0..6)
            .map(|index| {
                let mut question = question(&format!("q{index}"), &format!("Question {index}?"));
                question.blocking = true;
                question
            })
            .collect::<Vec<_>>();
        assert_eq!(gate.admit(questions), Err(ClarifyError::TooManyBlocking));
    }

    #[test]
    fn invalid_questions_fail_closed() {
        let mut gate = ClarifyGate::new(0);
        let mut too_few = question("q1", "Which one?");
        too_few.options.pop();
        assert_eq!(
            gate.admit(vec![too_few]),
            Err(ClarifyError::InvalidOptionCount)
        );

        let mut duplicate = question("q1", "Which one?");
        duplicate.options[1].id = "a".into();
        assert_eq!(
            gate.admit(vec![duplicate]),
            Err(ClarifyError::DuplicateOptionId)
        );

        let mut recommended = question("q1", "Which one?");
        recommended.options[0].recommended = true;
        assert_eq!(
            gate.admit(vec![recommended]),
            Err(ClarifyError::MultipleRecommended)
        );

        let mut control = question("q1", "Which\u{7} one?");
        control.prompt = "Which\u{7} one?".into();
        assert_eq!(gate.admit(vec![control]), Err(ClarifyError::InvalidPrompt));
    }

    fn answer(
        question_id: &str,
        option_id: Option<&str>,
        status: ClarifyAnswerStatus,
    ) -> ClarifyAnswer {
        ClarifyAnswer {
            question_id: question_id.into(),
            option_id: option_id.map(Into::into),
            status,
        }
    }

    #[test]
    fn resolve_round_counts_answers_and_keeps_the_default_valid() {
        let mut first = question("q1", "Which one?");
        first.blocking = true;
        let questions = vec![first.normalize(), question("q2", "What next?")];
        let summary = resolve_round(
            &questions,
            &[
                answer("q1", Some("b"), ClarifyAnswerStatus::Answered),
                answer("q2", None, ClarifyAnswerStatus::Skipped),
            ],
            ClarifyOutcome::Submitted,
        )
        .unwrap();
        assert_eq!(summary.answered, 1);
        assert_eq!(summary.skipped, 1);
        assert_eq!(summary.unanswered, 0);
        assert_eq!(summary.blocking_unanswered, 0);
        assert!(summary.default_answer_valid);

        let missing = resolve_round(
            &questions,
            &[answer("q1", Some("b"), ClarifyAnswerStatus::Answered)],
            ClarifyOutcome::Submitted,
        )
        .unwrap();
        assert_eq!(missing.unanswered, 1);

        let cancelled = resolve_round(&questions, &[], ClarifyOutcome::Cancelled).unwrap();
        assert_eq!(cancelled.answered, 0);
        assert_eq!(cancelled.unanswered, 2);
        assert_eq!(cancelled.blocking_unanswered, 1);
        assert!(cancelled.default_answer_valid);
    }

    #[test]
    fn resolve_round_rejects_bad_answers() {
        let questions = vec![question("q1", "Which one?").normalize()];
        assert_eq!(
            resolve_round(
                &questions,
                &[answer("q9", Some("a"), ClarifyAnswerStatus::Answered)],
                ClarifyOutcome::Submitted,
            ),
            Err(ClarifyError::UnknownQuestionId("q9".into()))
        );
        assert_eq!(
            resolve_round(
                &questions,
                &[answer("q1", None, ClarifyAnswerStatus::Answered)],
                ClarifyOutcome::Submitted,
            ),
            Err(ClarifyError::AnsweredWithoutOption("q1".into()))
        );
        assert_eq!(
            resolve_round(
                &questions,
                &[answer("q1", Some("nope"), ClarifyAnswerStatus::Answered)],
                ClarifyOutcome::Submitted,
            ),
            Err(ClarifyError::UnknownOptionId("nope".into()))
        );
        assert_eq!(
            resolve_round(
                &questions,
                &[answer("q1", Some("a"), ClarifyAnswerStatus::Skipped)],
                ClarifyOutcome::Submitted,
            ),
            Err(ClarifyError::UnexpectedOption("q1".into()))
        );
        assert_eq!(
            resolve_round(
                &questions,
                &[
                    answer("q1", Some("a"), ClarifyAnswerStatus::Answered),
                    answer("q1", Some("b"), ClarifyAnswerStatus::Answered),
                ],
                ClarifyOutcome::Submitted,
            ),
            Err(ClarifyError::DuplicateAnswer("q1".into()))
        );
        assert_eq!(
            resolve_round(
                &questions,
                &[answer("q1", None, ClarifyAnswerStatus::Skipped)],
                ClarifyOutcome::Submitted,
            ),
            Err(ClarifyError::EmptySubmission)
        );
    }
}
