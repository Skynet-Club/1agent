//! TUI adapter for `ClarifyPanel`: focus, selection, Other text and expiry.
//!
//! The domain panel owns validation and round resolution. This module only
//! turns key events into panel transitions and returns a finished result.

use std::collections::BTreeMap;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::clarify::{
    ClarifyOutcome, ClarifyPanel, ClarifyPanelAnswer, ClarifyQuestion, ClarifyRoundSummary,
    OTHER_OPTION_ID,
};
use crate::domain::{PermissionSet, RiskLevel, ToolSpec};
use crate::tools::{AgentTool, ToolError, ToolOutcome};
use crate::tui_terminal::TuiInput;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiClarifyState {
    panel: ClarifyPanel,
    question_index: usize,
    option_index: usize,
    editing_text: bool,
    editing_option_id: Option<String>,
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

/// A model-initiated clarify request waiting for the TUI panel.
pub struct ClarifyToolRequest {
    pub questions: Vec<ClarifyQuestion>,
    pub response: oneshot::Sender<Result<TuiClarifyResult, String>>,
}

/// Runtime tool that forwards a structured question batch to the TUI panel.
#[derive(Clone)]
pub struct ClarifyTool {
    requests: mpsc::UnboundedSender<ClarifyToolRequest>,
}

impl ClarifyTool {
    pub fn new(requests: mpsc::UnboundedSender<ClarifyToolRequest>) -> Self {
        Self { requests }
    }
}

#[async_trait]
impl AgentTool for ClarifyTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "clarify".into(),
            description: "Ask the user a blocking structured clarification (1-5 questions, 2-4 options each, an automatic Other and at most one recommended option). Use only when the answer cannot be inferred from the conversation, the journal or memory. Set `requires_text: true` on any option that asks the user to supply a name, copy, URL, number or other concrete input; selecting it then requires text and an empty promise is not an answer. Returns answered/skipped/unanswered questions and keeps the default answer valid.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "questions": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": 5,
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": {"type": "string", "description": "Stable question id"},
                                "prompt": {"type": "string", "description": "Question text"},
                                "options": {
                                    "type": "array",
                                    "minItems": 2,
                                    "maxItems": 4,
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "id": {"type": "string"},
                                            "label": {"type": "string"},
                                            "recommended": {"type": "boolean"},
                                            "requires_text": {
                                                "type": "boolean",
                                                "description": "Selecting this option opens a text field; empty text is not an answer."
                                            }
                                        },
                                        "required": ["id", "label"],
                                        "additionalProperties": false
                                    }
                                },
                                "blocking": {"type": "boolean", "description": "At least one question must be blocking"}
                            },
                            "required": ["id", "prompt", "options"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["questions"],
                "additionalProperties": false
            }),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::Low
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(310)
    }

    async fn execute(
        &self,
        arguments: Value,
        _permissions: &PermissionSet,
        cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        let questions_value = arguments
            .get("questions")
            .cloned()
            .ok_or_else(|| ToolError::InvalidArguments("missing questions".into()))?;
        let questions: Vec<ClarifyQuestion> = serde_json::from_value(questions_value)
            .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
        if questions.is_empty() || questions.len() > 5 {
            return Err(ToolError::InvalidArguments(
                "questions must contain 1..=5 items".into(),
            ));
        }
        for question in &questions {
            question
                .validate()
                .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
        }
        let (response, receiver) = oneshot::channel();
        self.requests
            .send(ClarifyToolRequest {
                questions,
                response,
            })
            .map_err(|_| ToolError::Failed("clarify UI is unavailable".into()))?;
        let result = tokio::select! {
            _ = cancellation.cancelled() => return Err(ToolError::Cancelled),
            result = receiver => result.map_err(|_| {
                ToolError::Failed("clarify UI closed without a response".into())
            })?,
        };
        let result = result.map_err(ToolError::Failed)?;
        let content = serde_json::to_string_pretty(&json!({
            "summary": &result.summary,
            "answers": &result.answers,
        }))
        .map_err(|error| ToolError::Failed(error.to_string()))?;
        Ok(ToolOutcome {
            summary: format!(
                "clarify {}: answered {} skipped {} unanswered {}",
                outcome_label(result.summary.outcome),
                result.summary.answered,
                result.summary.skipped,
                result.summary.unanswered
            ),
            content,
            metadata: BTreeMap::new(),
        })
    }
}

impl TuiClarifyState {
    pub fn new(panel: ClarifyPanel) -> Self {
        Self {
            panel,
            question_index: 0,
            option_index: 0,
            editing_text: false,
            editing_option_id: None,
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

    pub fn editing_text(&self) -> bool {
        self.editing_text
    }

    /// The option currently collecting text, if any. `Other` and options
    /// marked `requires_text` use this path.
    pub fn editing_option_id(&self) -> Option<&str> {
        self.editing_option_id.as_deref()
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
        if self.editing_text {
            return self.handle_text_input(input, now_ms);
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

    fn handle_text_input(&mut self, input: TuiInput, now_ms: u128) -> TuiClarifyStep {
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
                    self.error = Some("此项需要输入文本".into());
                    return TuiClarifyStep::Redraw;
                }
                let Some(question) = self.current_question() else {
                    return TuiClarifyStep::Redraw;
                };
                let Some(option_id) = self.editing_option_id.clone() else {
                    self.editing_text = false;
                    return TuiClarifyStep::Redraw;
                };
                let question_id = question.id.clone();
                let text = self.other_text.clone();
                match self
                    .panel
                    .select(&question_id, &option_id, Some(&text), now_ms)
                {
                    Ok(()) => {
                        self.editing_text = false;
                        self.editing_option_id = None;
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
                self.editing_text = false;
                self.editing_option_id = None;
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
        self.editing_text = false;
        self.editing_option_id = None;
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
        let question_id = question.id.clone();
        let Some(option) = question.options.get(self.option_index) else {
            return TuiClarifyStep::None;
        };
        let option_id = option.id.clone();
        let requires_text = option.requires_text;
        if self.is_resolved(&question_id) {
            self.advance_to_unanswered();
            return TuiClarifyStep::Redraw;
        }
        if option_id == OTHER_OPTION_ID || requires_text {
            self.editing_text = true;
            self.editing_option_id = Some(option_id);
            self.other_text.clear();
            self.error = None;
            return TuiClarifyStep::Redraw;
        }
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

    /// Select an option by its zero-based index. Used by the bottom bar's
    /// number-key shortcuts; selection policy stays in `select_current`.
    pub fn select_option(&mut self, index: usize, now_ms: u128) -> TuiClarifyStep {
        if self.editing_text || self.all_resolved() {
            return TuiClarifyStep::None;
        }
        let Some(question) = self.current_question() else {
            return TuiClarifyStep::None;
        };
        if index >= question.options.len() {
            return TuiClarifyStep::None;
        }
        self.option_index = index;
        self.error = None;
        self.select_current(now_ms)
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
                self.editing_text = false;
                self.editing_option_id = None;
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
                    requires_text: false,
                },
                ClarifyOption {
                    id: "b".into(),
                    label: "Beta".into(),
                    recommended: false,
                    requires_text: false,
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
        assert!(state.editing_text());
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
    fn requires_text_option_opens_the_text_field() {
        let mut question = question("q1", true);
        question.options[0].requires_text = true;
        let mut state = TuiClarifyState::new(ClarifyPanel::new(vec![question], 0).unwrap());
        assert_eq!(
            state.handle_input(TuiInput::Enter, 1),
            TuiClarifyStep::Redraw
        );
        assert!(state.editing_text());
        assert_eq!(state.editing_option_id(), Some("a"));
        for character in "云笺".chars() {
            state.handle_input(TuiInput::Char(character), 2);
        }
        state.handle_input(TuiInput::Enter, 3);
        let TuiClarifyStep::Finished(result) = state.handle_input(TuiInput::Enter, 4) else {
            panic!("expected a finished round");
        };
        assert_eq!(result.answers[0].option_id.as_deref(), Some("a"));
        assert_eq!(result.answers[0].other_text.as_deref(), Some("云笺"));
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
    fn number_selection_uses_the_option_index() {
        let mut state = TuiClarifyState::new(panel());
        assert_eq!(state.select_option(1, 1), TuiClarifyStep::Redraw);
        assert_eq!(state.panel().answers.len(), 1);
        assert_eq!(state.panel().answers[0].option_id.as_deref(), Some("b"));
        assert_eq!(state.select_option(9, 2), TuiClarifyStep::None);
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

    #[tokio::test]
    async fn clarify_tool_forwards_questions_and_returns_the_round() {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let tool = ClarifyTool::new(sender);
        let arguments = json!({
            "questions": [{
                "id": "q1",
                "prompt": "Which?",
                "options": [
                    {"id": "a", "label": "Alpha", "recommended": true},
                    {"id": "b", "label": "Beta"}
                ],
                "blocking": true
            }]
        });
        let permissions = PermissionSet::default();
        let cancellation = CancellationToken::new();
        let future = tool.execute(arguments, &permissions, &cancellation);
        tokio::pin!(future);
        let request = tokio::select! {
            request = receiver.recv() => request.expect("clarify request"),
            result = &mut future => panic!("tool finished early: {result:?}"),
        };
        request
            .response
            .send(Ok(TuiClarifyResult {
                summary: ClarifyRoundSummary {
                    outcome: ClarifyOutcome::Submitted,
                    answered: 1,
                    skipped: 0,
                    unanswered: 0,
                    blocking_unanswered: 0,
                    default_answer_valid: true,
                },
                answers: vec![ClarifyPanelAnswer {
                    question_id: "q1".into(),
                    option_id: Some("a".into()),
                    status: ClarifyAnswerStatus::Answered,
                    other_text: None,
                }],
            }))
            .unwrap();
        let outcome = future.await.unwrap();
        assert!(outcome.content.contains("\"outcome\": \"submitted\""));
        assert!(outcome.summary.contains("已回答") || outcome.summary.contains("已提交"));
    }
}
