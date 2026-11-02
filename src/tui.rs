use crate::domain::{CognitiveMode, RiskLevel};
use crate::safety::is_disallowed_control;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiStatus {
    Idle,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl TuiStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiState {
    pub last_instruction: String,
    pub status: TuiStatus,
    pub mode: CognitiveMode,
    pub risk: RiskLevel,
    pub provider_label: String,
    pub max_steps: usize,
    pub timeout_seconds: u64,
    transcript: Vec<String>,
}

impl TuiState {
    pub fn new(provider_label: impl Into<String>, max_steps: usize, timeout_seconds: u64) -> Self {
        Self {
            last_instruction: String::new(),
            status: TuiStatus::Idle,
            mode: CognitiveMode::DirectAnswer,
            risk: RiskLevel::Low,
            provider_label: provider_label.into(),
            max_steps,
            timeout_seconds,
            transcript: Vec::new(),
        }
    }

    pub fn begin(&mut self, instruction: &str, mode: CognitiveMode, risk: RiskLevel) {
        self.last_instruction = sanitize_inline(instruction);
        self.status = TuiStatus::Running;
        self.mode = mode;
        self.risk = risk;
        self.push_entry(format!("You > {}", self.last_instruction));
    }

    pub fn finish(&mut self, status: TuiStatus, output: impl Into<String>) {
        self.status = status;
        self.push_entry(output.into());
    }

    pub fn info(&mut self, output: impl Into<String>) {
        self.push_entry(output.into());
    }

    pub fn clear(&mut self) {
        self.last_instruction.clear();
        self.status = TuiStatus::Idle;
        self.transcript.clear();
    }

    pub fn render_screen(&self, columns: usize, rows: usize) -> String {
        let columns = columns.max(40);
        let rows = rows.max(8);
        let instruction = truncate_chars(&self.last_instruction, columns.saturating_sub(8));
        let instruction = if instruction.is_empty() {
            "(waiting for a task)"
        } else {
            instruction.as_str()
        };
        let state_line = format!(
            "{} | status={} | mode={} | risk={} | provider={} | budget={} steps/{}s",
            "1·Agent",
            self.status.as_str(),
            snake_case(self.mode),
            snake_case(self.risk),
            self.provider_label,
            self.max_steps,
            self.timeout_seconds
        );
        let state_line = truncate_chars(&sanitize_inline(&state_line), columns);
        let instruction_line = format!(" You > {instruction}");
        let history_lines = self
            .transcript
            .iter()
            .flat_map(|entry| {
                sanitize_multiline(entry)
                    .lines()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let history_budget = rows.saturating_sub(5);
        let history_start = history_lines.len().saturating_sub(history_budget);
        let mut screen = String::new();
        screen.push_str("\x1b[2J\x1b[H");
        screen.push_str("\x1b[1;36m");
        screen.push_str(&state_line);
        screen.push_str("\x1b[0m\n");
        screen.push_str("\x1b[7m");
        screen.push_str(&instruction_line);
        screen.push_str("\x1b[0m\n");
        screen.push_str("\x1b[2m");
        screen.push_str(&"─".repeat(columns.min(120)));
        screen.push_str("\x1b[0m\n");
        for line in history_lines.into_iter().skip(history_start) {
            screen.push_str(&line);
            screen.push('\n');
        }
        screen.push('\n');
        screen.push_str("> ");
        screen
    }

    fn push_entry(&mut self, entry: String) {
        let entry = sanitize_multiline(&entry);
        self.transcript.push(truncate_chars(&entry, 8_000));
        if self.transcript.len() > 100 {
            self.transcript.remove(0);
        }
    }
}

pub fn sanitize_inline(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character == '\n' || character == '\t' || is_disallowed_control(character) {
                ' '
            } else {
                character
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn sanitize_multiline(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character == '\n' || character == '\t' {
                character
            } else if character == '\r' || is_disallowed_control(character) {
                '�'
            } else {
                character
            }
        })
        .collect()
}

fn truncate_chars(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    if limit == 0 {
        return String::new();
    }
    let mut truncated = text
        .chars()
        .take(limit.saturating_sub(1))
        .collect::<String>();
    truncated.push('…');
    truncated
}

fn snake_case(value: impl std::fmt::Debug) -> String {
    let value = format!("{value:?}");
    let mut output = String::with_capacity(value.len() + 4);
    for character in value.chars() {
        if character.is_ascii_uppercase() {
            if !output.is_empty() {
                output.push('_');
            }
            output.push(character.to_ascii_lowercase());
        } else {
            output.push(character);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topbar_keeps_last_instruction_and_updates_status() {
        let mut state = TuiState::new("mock/offline-v1\u{1b}[31m", 6, 120);
        state.begin(
            "Compare option A and B within budget",
            CognitiveMode::DecisionAnalysis,
            RiskLevel::Medium,
        );
        let running = state.render_screen(100, 30);
        assert!(running.contains("You > Compare option A and B within budget"));
        assert!(running.contains("status=running"));
        assert!(running.contains("mode=decision_analysis"));
        assert!(running.contains("risk=medium"));
        state.finish(TuiStatus::Completed, "Conclusion: A");
        let completed = state.render_screen(100, 30);
        assert!(completed.contains("You > Compare option A and B within budget"));
        assert!(completed.contains("status=completed"));
    }

    #[test]
    fn terminal_output_is_control_safe() {
        let mut state = TuiState::new("mock/offline-v1", 6, 120);
        state.begin(
            "safe\u{1b}[31m text\u{202e}",
            CognitiveMode::DirectAnswer,
            RiskLevel::Low,
        );
        state.info("answer\u{0000}\u{202e}tail");
        let screen = state.render_screen(80, 20);
        assert!(!screen.contains("\u{1b}[31m"));
        assert!(!screen.contains('\u{202e}'));
        assert!(screen.contains("safe [31m text"));
        assert!(screen.contains("answer�"));
    }
}
