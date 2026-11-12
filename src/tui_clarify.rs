//! TUI adapter for `ClarifyPanel`: focus, selection, Other text and expiry.
//!
//! The domain panel owns validation and round resolution. This module only
//! turns key events into panel transitions and returns a finished result.

use crate::clarify::{
    ClarifyOutcome, ClarifyPanel, ClarifyPanelAnswer, ClarifyQuestion, ClarifyRoundSummary,
    OTHER_OPTION_ID,
};
use crate::tui_terminal::TuiInput;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiClarifyState {
    panel: ClarifyPanel,
    question_index: usize,
    option_index: usize,
    editing_other: bool,
    other_text: String,
    error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiClarifyResult {
    pub summary: ClarifyRoundSummary,
    pub answers: Vec<ClarifyPanelAnswer>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuiClarifyStep {
    None,
    Redraw,
    Finished(TuiClarifyResult),
}

impl TuiClarifyState {
    pub fn new(panel: ClarifyPanel) -> Self {
        Self {
            panel,
            question_index: 0,
            option_index: 0,
            editing_other: false,
            other_text: String::new(),
            error: None,
        }
    }

    pub fn panel(&self) -> &ClarifyPanel {
        &self.panel
    }

    pub fn question_index(&self) -> usize {
        self.question_index
    }

    pub fn option_index(&self) -> usize {
        self.option_index
    }

    pub fn editing_other(&self) -> bool {
        self.editing_other
    }

    pub fn other_text(&self) -> &str {
        &self.other_text
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn deadline_ms(&self) -> u128 {
        self.panel.deadline_ms()
    }

    pub fn remaining_seconds(&self, now_ms: u128) -> u64 {
        self.deadline_ms()
            .saturating_sub(now_ms)
            .div_ceil(1_000)
            .min(u128::from(u64::MAX)) as u64
    }

    pub fn current_question(&self) -> Option<&ClarifyQuestion> {
        self.panel.questions.get(self.question_index)
    }

    pub fn is_resolved(&self, question_id: &str) -> bool {
        self.panel
            .answers
            .iter()
            .any(|answer| answer.question_id == question_id)
    }

    pub fn all_resolved(&self) -> bool {
        self.panel.answers.len() == self.panel.questions.len()
    }

    pub fn handle_input(&mut self, input: TuiInput, now_ms: u128) -> TuiClarifyStep {
        if let Some(step) = self.expire(now_ms) {
            return step;
        }
        if self.editing_other {
            return self.handle_other_input(input, now_ms);
        }
        match input {
            TuiInput::Up => self.move_option(-1),
            TuiInput::Down => self.move_option(1),
            TuiInput::Left => self.move_question(-1),
            TuiInput::Right | TuiInput::Tab => self.move_question(1),
            TuiInput::Enter => self.select_current(now_ms),
            TuiInput::Char('s' | 'S' | ' ') => self.skip_current(now_ms),
            TuiInput::Escape | TuiInput::CtrlC => self.cancel(),
            TuiInput::Backspace | TuiInput::Delete => TuiClarifyStep::Redraw,
            TuiInput::Paste(text) => {
                // Paste only edits while the Other field is focused.
                let _ = text;
                TuiClarifyStep::None
            }
            _ => TuiClarifyStep::None,
        }
    }

    pub fn expire(&mut self, now_ms: u128) -> Option<TuiClarifyStep> {
        match self.panel.expire(now_ms) {
            Ok(Some(summary)) => Some(self.finish(summary)),
            Ok(None) => None,
            Err(error) => {
                self.error = Some(error.to_string());
                Some(TuiClarifyStep::Redraw)
            }
        }
    }

    fn handle_other_input(&mut self, input: TuiInput, now_ms: u128) -> TuiClarifyStep {
        match input {
            TuiInput::Char(character) if !character.is_control() => {
                if self.other_text.chars().count() < 500 {
                    self.other_text.push(character);
                }
                self.error = None;
                TuiClarifyStep::Redraw
            }
            TuiInput::Backspace => {
                self.other_text.pop();
                self.error = None;
                TuiClarifyStep::Redraw
            }
            TuiInput::Paste(text) => {
                for character in text.chars() {
                    if character.is_control() {
                        if character == '\n' || character == '\r' || character == '\t' {
                            self.push_other_char(' ');
                        }
                        continue;
                    }
                    self.push_other_char(character);
                }
                self.error = None;
                TuiClarifyStep::Redraw
            }
            TuiInput::Enter => {
                if self.other_text.trim().is_empty() {
                    self.error = Some("Other 需要输入文本".into());
                    return TuiClarifyStep::Redraw;
                }
                let Some(question) = self.current_question() else {
                    return TuiClarifyStep::Redraw;
                };
                let question_id = question.id.clone();
                let text = self.other_text.clone();
                match self
                    .panel
                    .select(&question_id, OTHER_OPTION_ID, Some(&text), now_ms)
                {
                    Ok(()) => {
                        self.editing_other = false;
                        self.other_text.clear();
                        self.error = None;
                        self.advance_to_unanswered();
                        TuiClarifyStep::Redraw
                    }
                    Err(error) => {
                        self.error = Some(error.to_string());
                        TuiClarifyStep::Redraw
                    }
                }
            }
            TuiInput::Escape => {
                self.editing_other = false;
                self.other_text.clear();
                self.error = None;
                TuiClarifyStep::Redraw
            }
            TuiInput::CtrlC => self.cancel(),
            _ => TuiClarifyStep::None,
        }
    }

    fn push_other_char(&mut self, character: char) {
        if self.other_text.chars().count() < 500 {
            self.other_text.push(character);
        }
    }

    fn move_option(&mut self, delta: isize) -> TuiClarifyStep {
        let Some(question) = self.current_question() else {
            return TuiClarifyStep::None;
        };
        let count = question.options.len();
        if count == 0 {
            return TuiClarifyStep::None;
        }
        self.option_index =
            (self.option_index as isize + delta).rem_euclid(count as isize) as usize;
        self.error = None;
        TuiClarifyStep::Redraw
    }

    fn move_question(&mut self, delta: isize) -> TuiClarifyStep {
        let count = self.panel.questions.len();
        if count == 0 {
            return TuiClarifyStep::None;
        }
        self.question_index =
            (self.question_index as isize + delta).rem_euclid(count as isize) as usize;
        self.option_index = 0;
        self.editing_other = false;
        self.other_text.clear();
        self.error = None;
        TuiClarifyStep::Redraw
    }

    fn select_current(&mut self, now_ms: u128) -> TuiClarifyStep {
        if self.all_resolved() {
            return self.confirm(now_ms);
        }
        let Some(question) = self.current_question() else {
            return TuiClarifyStep::None;
        };
        if self.is_resolved(&question.id) {
            self.advance_to_unanswered();
            return TuiClarifyStep::Redraw;
        }
        let Some(option) = question.options.get(self.option_index) else {
            return TuiClarifyStep::None;
        };
        if option.id == OTHER_OPTION_ID {
            self.editing_other = true;
            self.other_text.clear();
            self.error = None;
            return TuiClarifyStep::Redraw;
        }
        let question_id = question.id.clone();
        let option_id = option.id.clone();
        match self.panel.select(&question_id, &option_id, None, now_ms) {
            Ok(()) => {
                self.error = None;
                self.advance_to_unanswered();
                TuiClarifyStep::Redraw
            }
            Err(error) => {
                self.error = Some(error.to_string());
                TuiClarifyStep::Redraw
            }
        }
    }

    fn skip_current(&mut self, now_ms: u128) -> TuiClarifyStep {
        let Some(question) = self.current_question() else {
            return TuiClarifyStep::None;
        };
        if self.is_resolved(&question.id) {
            self.advance_to_unanswered();
            return TuiClarifyStep::Redraw;
        }
        let question_id = question.id.clone();
        match self.panel.skip(&question_id, now_ms) {
            Ok(()) => {
                self.error = None;
                self.advance_to_unanswered();
                TuiClarifyStep::Redraw
            }
            Err(error) => {
                self.error = Some(error.to_string());
                TuiClarifyStep::Redraw
            }
        }
    }

    fn advance_to_unanswered(&mut self) {
        let count = self.panel.questions.len();
        for offset in 1..=count {
            let index = (self.question_index + offset) % count;
            let question = &self.panel.questions[index];
            if !self.is_resolved(&question.id) {
                self.question_index = index;
                self.option_index = 0;
                self.editing_other = false;
                self.other_text.clear();
                return;
            }
        }
        self.option_index = self.option_index.min(
            self.current_question()
                .map(|question| question.options.len().saturating_sub(1))
                .unwrap_or(0),
        );
    }

    fn confirm(&mut self, now_ms: u128) -> TuiClarifyStep {
        match self.panel.confirm(now_ms) {
            Ok(summary) => self.finish(summary),
            Err(error) => {
                self.error = Some(error.to_string());
                TuiClarifyStep::Redraw
            }
        }
    }

    fn cancel(&mut self) -> TuiClarifyStep {
        match self.panel.cancel() {
            Ok(summary) => self.finish(summary),
            Err(error) => {
                self.error = Some(error.to_string());
                TuiClarifyStep::Redraw
            }
        }
    }

    fn finish(&self, summary: ClarifyRoundSummary) -> TuiClarifyStep {
        TuiClarifyStep::Finished(TuiClarifyResult {
            summary,
            answers: self.panel.answers.clone(),
        })
    }
}

/// The four outcome labels shared by the panel footer and the result line.
pub fn outcome_label(outcome: ClarifyOutcome) -> &'static str {
    match outcome {
        ClarifyOutcome::Submitted => "已提交",
        ClarifyOutcome::Cancelled => "已取消",
        ClarifyOutcome::TimedOut => "已超时",
        ClarifyOutcome::Undelivered => "未送达",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clarify::{ClarifyAnswerStatus, ClarifyOption};

    fn question(id: &str, blocking: bool) -> ClarifyQuestion {
        ClarifyQuestion {
            id: id.into(),
            prompt: format!("Question {id}?"),
            options: vec![
                ClarifyOption {
                    id: "a".into(),
                    label: "Alpha".into(),
                    recommended: true,
                },
                ClarifyOption {
                    id: "b".into(),
                    label: "Beta".into(),
                    recommended: false,
                },
            ],
            blocking,
        }
    }

    fn panel() -> ClarifyPanel {
        ClarifyPanel::new(vec![question("q1", true)], 0).unwrap()
    }

    #[test]
    fn enter_locks_the_recommended_option_and_enter_confirms() {
        let mut state = TuiClarifyState::new(panel());
        assert_eq!(
            state.handle_input(TuiInput::Enter, 1),
            TuiClarifyStep::Redraw
        );
        assert!(state.all_resolved());
        let TuiClarifyStep::Finished(result) = state.handle_input(TuiInput::Enter, 2) else {
            panic!("expected a finished round");
        };
        assert_eq!(result.summary.outcome, ClarifyOutcome::Submitted);
        assert_eq!(result.summary.answered, 1);
        assert_eq!(result.answers[0].status, ClarifyAnswerStatus::Answered);
        assert_eq!(result.answers[0].option_id.as_deref(), Some("a"));
    }

    #[test]
    fn down_selects_the_second_option() {
        let mut state = TuiClarifyState::new(panel());
        state.handle_input(TuiInput::Down, 1);
        assert_eq!(
            state.handle_input(TuiInput::Enter, 2),
            TuiClarifyStep::Redraw
        );
        let TuiClarifyStep::Finished(result) = state.handle_input(TuiInput::Enter, 3) else {
            panic!("expected a finished round");
        };
        assert_eq!(result.answers[0].option_id.as_deref(), Some("b"));
    }

    #[test]
    fn skip_then_confirm_is_a_cancellation() {
        let mut state = TuiClarifyState::new(panel());
        assert_eq!(
            state.handle_input(TuiInput::Char('s'), 1),
            TuiClarifyStep::Redraw
        );
        let TuiClarifyStep::Finished(result) = state.handle_input(TuiInput::Enter, 2) else {
            panic!("expected a finished round");
        };
        assert_eq!(result.summary.outcome, ClarifyOutcome::Cancelled);
        assert_eq!(result.summary.skipped, 1);
    }

    #[test]
    fn other_requires_text_and_keeps_it() {
        let mut state = TuiClarifyState::new(panel());
        state.handle_input(TuiInput::Up, 1);
        state.handle_input(TuiInput::Enter, 2);
        assert!(state.editing_other());
        let step = state.handle_input(TuiInput::Enter, 3);
        assert_eq!(step, TuiClarifyStep::Redraw);
        assert!(state.error().is_some());
        for character in "custom".chars() {
            state.handle_input(TuiInput::Char(character), 4);
        }
        state.handle_input(TuiInput::Enter, 5);
        let TuiClarifyStep::Finished(result) = state.handle_input(TuiInput::Enter, 6) else {
            panic!("expected a finished round");
        };
        assert_eq!(result.answers[0].other_text.as_deref(), Some("custom"));
        assert_eq!(result.answers[0].status, ClarifyAnswerStatus::Answered);
    }

    #[test]
    fn escape_cancels_the_panel() {
        let mut state = TuiClarifyState::new(panel());
        let TuiClarifyStep::Finished(result) = state.handle_input(TuiInput::Escape, 1) else {
            panic!("expected a finished round");
        };
        assert_eq!(result.summary.outcome, ClarifyOutcome::Cancelled);
    }

    #[test]
    fn deadline_expires_and_keeps_the_default_valid() {
        let mut state = TuiClarifyState::new(panel());
        let deadline = state.deadline_ms();
        let TuiClarifyStep::Finished(result) = state.handle_input(TuiInput::Enter, deadline + 1)
        else {
            panic!("expected a finished round");
        };
        assert_eq!(result.summary.outcome, ClarifyOutcome::TimedOut);
        assert!(result.summary.blocking_unanswered >= 1);
        assert!(result.summary.default_answer_valid);
    }

    #[test]
    fn left_and_right_move_between_questions() {
        let panel =
            ClarifyPanel::new(vec![question("q1", true), question("q2", false)], 0).unwrap();
        let mut state = TuiClarifyState::new(panel);
        assert_eq!(state.question_index(), 0);
        state.handle_input(TuiInput::Right, 1);
        assert_eq!(state.question_index(), 1);
        state.handle_input(TuiInput::Left, 2);
        assert_eq!(state.question_index(), 0);
    }

    #[test]
    fn paste_in_other_is_single_line_and_bounded() {
        let mut state = TuiClarifyState::new(panel());
        state.handle_input(TuiInput::Up, 1);
        state.handle_input(TuiInput::Enter, 2);
        state.handle_input(TuiInput::Paste("a\nb\tc".into()), 3);
        assert_eq!(state.other_text(), "a b c");
        let long = "x".repeat(700);
        state.handle_input(TuiInput::Paste(long), 4);
        assert_eq!(state.other_text().chars().count(), 500);
    }
}
