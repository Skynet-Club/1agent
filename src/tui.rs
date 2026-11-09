use crate::domain::{CognitiveMode, RiskLevel};
use crate::safety::is_disallowed_control;
use crate::tui_commands::{
    CommandDrawerRow, CommandEntry, CommandStage, TuiCommandId, TuiCommandSpec, command_usage,
    drawer_rows, visible_commands,
};
use crate::tui_host::HostBinding;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, List, ListItem, ListState, Paragraph, Wrap};
use std::ops::Range;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const INPUT_BG: Color = Color::Indexed(236);
const UI_ACCENT: Color = Color::Indexed(75);
const CODE_BG: Color = Color::Indexed(236);
const CODE_ADDED_BG: Color = Color::Indexed(22);
const CODE_REMOVED_BG: Color = Color::Indexed(52);
const MUTED_TEXT: Color = Color::Indexed(245);
const CAPTION_GREY: Color = Color::Indexed(245);
const MARK_GREEN: Color = Color::Rgb(0x3f, 0xb9, 0x50);
const TLD_GREEN: Color = Color::Rgb(0x2e, 0xa0, 0x43);
const LINK: Color = Color::Indexed(75);
const LINK_HOVER: Color = Color::Rgb(0x7f, 0xe6, 0xff);
const BANNER_BG: Color = Color::Indexed(236);
const COMMAND_DESCRIPTION: Color = Color::Indexed(252);
const DRAWER_BORDER: Color = Color::Indexed(240);
const MENU_BG: Color = Color::Indexed(235);
const MENU_SELECTED_BG: Color = Color::Indexed(238);
const MENU_LABEL: Color = Color::Indexed(252);
const MENU_HINT: Color = Color::Indexed(245);

/// One row of the Ctrl+Tab mode menu. `None` means automatic inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeMenuItem {
    pub mode: Option<CognitiveMode>,
    pub label: &'static str,
    pub description: &'static str,
}

/// Escalate is deliberately absent: nothing produces it and it has no
/// behaviour, so offering it would be a promise the runtime cannot keep.
pub const MODE_MENU: &[ModeMenuItem] = &[
    ModeMenuItem {
        mode: None,
        label: "自动",
        description: "跟随任务内容推断",
    },
    ModeMenuItem {
        mode: Some(CognitiveMode::DirectAnswer),
        label: "直接回答",
        description: "只给结论，不展开结构化推理",
    },
    ModeMenuItem {
        mode: Some(CognitiveMode::EvidenceResearch),
        label: "证据研究",
        description: "事实、断言、假设与未知分开",
    },
    ModeMenuItem {
        mode: Some(CognitiveMode::DecisionAnalysis),
        label: "决策分析",
        description: "备选、标准、权衡与翻转条件",
    },
    ModeMenuItem {
        mode: Some(CognitiveMode::Execute),
        label: "执行任务",
        description: "有副作用，需要写权限",
    },
    ModeMenuItem {
        mode: Some(CognitiveMode::Diagnosis),
        label: "诊断分析",
        description: "根因、失败模式与回归检查",
    },
];

pub const WELCOME_LOGO: &[&str] = &[
    " ╔╗ ╔═══╗             ╔╗ ",
    "╔╝║ ║╔═╗║            ╔╝╚╗",
    "╚╗║ ║║ ║║╔══╗╔══╗╔═╗ ╚╗╔╝",
    " ║║ ║╚═╝║║╔╗║║╔╗║║╔╗╗ ║║ ",
    "╔╝╚╗║╔═╗║║╚╝║║║═╣║║║║ ║╚╗",
    "╚══╝╚╝ ╚╝╚═╗║╚══╝╚╝╚╝ ╚═╝",
    "         ╔═╝║            ",
    "         ╚══╝            ",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiTone {
    Default,
    Accent,
    Success,
    Warning,
    Danger,
    Evidence,
    Code,
    Keyword,
    Function,
    Type,
    String,
    Number,
    Comment,
    Reference,
    Supplementary,
    Muted,
    User,
}

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

    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Idle => "空闲",
            Self::Running => "运行中",
            Self::Completed => "已完成",
            Self::Failed => "失败",
            Self::Cancelled => "已取消",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TranscriptKind {
    User,
    Assistant,
    Heading,
    Item,
    Quote,
    QueryResult,
    Evidence,
    Code,
    Metadata,
    System,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TranscriptEntry {
    kind: TranscriptKind,
    tone: TuiTone,
    label: Option<String>,
    label_tone: TuiTone,
    text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommandPanel {
    title: String,
    body: String,
    menu: Vec<CommandMenuItem>,
    selected_menu_item: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandMenuItem {
    pub id: String,
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TranscriptVisualLine {
    kind: TranscriptKind,
    tone: TuiTone,
    text: String,
    prompt_text: Option<String>,
    prefix_len: usize,
    prefix_tone: TuiTone,
    label_range: Option<Range<usize>>,
    label_tone: TuiTone,
    syntax_ranges: Vec<(Range<usize>, TuiTone)>,
    background: Option<Color>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TranscriptPosition {
    line: usize,
    index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InputGeometry {
    row: u16,
    text_x: u16,
    text_width: u16,
    visible_start: usize,
    visible_end: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TuiRunMetrics {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_hit_tokens: u64,
    pub cache_miss_tokens: u64,
    pub elapsed_ms: u128,
    pub usage_seen: bool,
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
    host_binding: HostBinding,
    command_stage: CommandStage,
    session_id: Option<String>,
    last_journal_path: Option<PathBuf>,
    last_details: Option<String>,
    pending_reframe: Option<crate::reframe::ReframeProposal>,
    pointer: Option<(u16, u16)>,
    mode_menu: Option<usize>,
    mode_override: Option<CognitiveMode>,
    last_inferred: Option<CognitiveMode>,
    context_chars: usize,
    context_budget: usize,
    details_open: bool,
    details_scroll: usize,
    command_panel: Option<CommandPanel>,
    command_panel_history: Vec<CommandPanel>,
    command_panel_scroll: usize,
    welcome_visible: bool,
    transcript: Vec<TranscriptEntry>,
    slash_drawer_dismissed: bool,
    recent_commands: Vec<TuiCommandId>,
    pending_confirmation: Option<String>,
    quit_requested: bool,
    run_started_unix_ms: Option<u128>,
    run_metrics: TuiRunMetrics,
    input_history: Vec<String>,
    history_cursor: Option<usize>,
    input: String,
    input_cursor: usize,
    input_scroll: usize,
    input_geometry: Option<InputGeometry>,
    selection_anchor: Option<usize>,
    transcript_area: Option<Rect>,
    transcript_lines: Vec<TranscriptVisualLine>,
    transcript_scroll_top: usize,
    transcript_anchor: Option<TranscriptPosition>,
    transcript_focus: Option<TranscriptPosition>,
    selection_dragging: bool,
    selection_autoscroll: i8,
    selection_drag_column: u16,
    notice: Option<String>,
    unseen_activity: bool,
    follow_control_area: Option<Rect>,
    follow_control_hovered: bool,
    scroll_from_bottom: usize,
    selected_command: usize,
    activity_frame: usize,
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
            host_binding: HostBinding::standalone(),
            command_stage: CommandStage::NoMission,
            session_id: None,
            last_journal_path: None,
            last_details: None,
            pending_reframe: None,
            pointer: None,
            mode_menu: None,
            mode_override: None,
            last_inferred: None,
            context_chars: 0,
            context_budget: 100_000,
            details_open: false,
            details_scroll: 0,
            command_panel: None,
            command_panel_history: Vec::new(),
            command_panel_scroll: 0,
            welcome_visible: true,
            transcript: Vec::new(),
            slash_drawer_dismissed: false,
            recent_commands: Vec::new(),
            pending_confirmation: None,
            quit_requested: false,
            run_started_unix_ms: None,
            run_metrics: TuiRunMetrics::default(),
            input_history: Vec::new(),
            history_cursor: None,
            input: String::new(),
            input_cursor: 0,
            input_scroll: 0,
            input_geometry: None,
            selection_anchor: None,
            transcript_area: None,
            transcript_lines: Vec::new(),
            transcript_scroll_top: 0,
            transcript_anchor: None,
            transcript_focus: None,
            selection_dragging: false,
            selection_autoscroll: 0,
            selection_drag_column: 0,
            notice: None,
            unseen_activity: false,
            follow_control_area: None,
            follow_control_hovered: false,
            scroll_from_bottom: 0,
            selected_command: 0,
            activity_frame: 0,
        }
    }

    pub fn begin(&mut self, instruction: &str, mode: CognitiveMode, risk: RiskLevel) {
        self.close_command_panel();
        self.quit_requested = false;
        self.run_started_unix_ms = Some(now_unix_ms());
        self.run_metrics = TuiRunMetrics::default();
        self.last_instruction = sanitize_inline(instruction);
        self.status = TuiStatus::Running;
        self.welcome_visible = false;
        self.mode = mode;
        self.risk = risk;
        self.clear_input();
        self.scroll_from_bottom = 0;
        self.push_entry(
            TranscriptKind::User,
            TuiTone::User,
            None,
            TuiTone::Default,
            self.last_instruction.clone(),
        );
    }

    pub fn finish(&mut self, status: TuiStatus, output: impl Into<String>) {
        self.status = status;
        self.finish_run_clock();
        let (kind, tone) = if status == TuiStatus::Failed {
            (TranscriptKind::Error, TuiTone::Danger)
        } else {
            (TranscriptKind::Assistant, TuiTone::Default)
        };
        self.push_entry(kind, tone, None, TuiTone::Default, output.into());
    }

    pub fn info(&mut self, output: impl Into<String>) {
        if let Some(panel) = self.command_panel.as_mut() {
            let output = sanitize_multiline(&output.into());
            if !output.is_empty() {
                if !panel.body.is_empty() {
                    panel.body.push('\n');
                }
                panel.body.push_str(&output);
                self.command_panel_scroll = 0;
            }
            return;
        }
        self.push_entry(
            TranscriptKind::System,
            TuiTone::Default,
            None,
            TuiTone::Muted,
            output.into(),
        );
    }

    pub fn set_status(&mut self, status: TuiStatus) {
        self.status = status;
    }

    pub fn assistant_heading(&mut self, title: impl Into<String>, tone: TuiTone) {
        self.push_entry(
            TranscriptKind::Heading,
            tone,
            None,
            TuiTone::Default,
            title.into(),
        );
    }

    pub fn assistant_body(&mut self, text: impl Into<String>) {
        self.push_entry(
            TranscriptKind::Assistant,
            TuiTone::Default,
            None,
            TuiTone::Default,
            text.into(),
        );
    }

    pub fn assistant_markdown(&mut self, text: &str) {
        let lines = text.lines().collect::<Vec<_>>();
        let mut prose = Vec::new();
        let mut index = 0;
        while index < lines.len() {
            let trimmed = lines[index].trim_start();
            if let Some(language) = trimmed.strip_prefix("```") {
                self.flush_markdown_prose(&mut prose);
                let language = language.trim().to_string();
                index += 1;
                let code_start = index;
                while index < lines.len() && !lines[index].trim_start().starts_with("```") {
                    index += 1;
                }
                self.push_code_block(&language, &lines[code_start..index]);
                if index < lines.len() {
                    index += 1;
                }
                continue;
            }

            if let Some(quote) = trimmed.strip_prefix('>') {
                self.flush_markdown_prose(&mut prose);
                let mut quote_lines = vec![quote.strip_prefix(' ').unwrap_or(quote).to_string()];
                index += 1;
                while index < lines.len() {
                    let Some(quote) = lines[index].trim_start().strip_prefix('>') else {
                        break;
                    };
                    quote_lines.push(quote.strip_prefix(' ').unwrap_or(quote).to_string());
                    index += 1;
                }
                while quote_lines
                    .last()
                    .is_some_and(|line| line.trim().is_empty())
                {
                    quote_lines.pop();
                }
                if !quote_lines.is_empty() {
                    self.assistant_quote(quote_lines.join("\n"));
                }
                continue;
            }

            if let Some((language, code_start)) = unfenced_code_start(&lines, index) {
                self.flush_markdown_prose(&mut prose);
                let mut code_end = code_start;
                index = code_start;
                while index < lines.len() {
                    if lines[index].trim().is_empty() {
                        let next = lines[index..]
                            .iter()
                            .position(|line| !line.trim().is_empty())
                            .map(|offset| index + offset);
                        if next.is_some_and(|next| looks_like_code_line(lines[next])) {
                            index += 1;
                            code_end = index;
                            continue;
                        }
                        break;
                    }
                    if !looks_like_code_line(lines[index]) {
                        break;
                    }
                    index += 1;
                    code_end = index;
                }
                self.push_code_block(language, &lines[code_start..code_end]);
                continue;
            }

            prose.push(lines[index].to_string());
            index += 1;
        }
        self.flush_markdown_prose(&mut prose);
    }

    pub fn assistant_item(
        &mut self,
        label: impl Into<String>,
        label_tone: TuiTone,
        text: impl Into<String>,
    ) {
        self.push_entry(
            TranscriptKind::Item,
            TuiTone::Default,
            Some(label.into()),
            label_tone,
            text.into(),
        );
    }

    pub fn assistant_evidence(&mut self, source: impl Into<String>, text: impl Into<String>) {
        self.push_entry(
            TranscriptKind::Evidence,
            TuiTone::Default,
            Some(format!("[{}]", source.into())),
            TuiTone::Evidence,
            text.into(),
        );
    }

    pub fn assistant_quote(&mut self, text: impl Into<String>) {
        self.push_entry(
            TranscriptKind::Quote,
            TuiTone::Supplementary,
            Some("引用".into()),
            TuiTone::Supplementary,
            text.into(),
        );
    }

    pub fn assistant_query_result(&mut self, source: impl Into<String>, text: impl Into<String>) {
        let source = sanitize_inline(&source.into());
        let label = if source.is_empty() {
            "来源".to_string()
        } else {
            format!("来源 · {source}")
        };
        self.push_entry(
            TranscriptKind::QueryResult,
            TuiTone::Supplementary,
            Some(label),
            TuiTone::Muted,
            text.into(),
        );
    }

    pub fn assistant_metadata(&mut self, text: impl Into<String>) {
        self.push_entry(
            TranscriptKind::Metadata,
            TuiTone::Muted,
            None,
            TuiTone::Muted,
            text.into(),
        );
    }

    pub fn clear(&mut self) {
        self.close_command_panel();
        self.last_instruction.clear();
        self.status = TuiStatus::Idle;
        self.transcript.clear();
        self.clear_input();
        self.last_details = None;
        self.details_open = false;
        self.details_scroll = 0;
        self.welcome_visible = true;
        self.unseen_activity = false;
        self.scroll_from_bottom = 0;
    }

    pub fn begin_command_panel(&mut self, title: impl Into<String>) {
        self.command_panel = Some(CommandPanel {
            title: sanitize_inline(&title.into()),
            body: String::new(),
            menu: Vec::new(),
            selected_menu_item: 0,
        });
        self.command_panel_history.clear();
        self.command_panel_scroll = 0;
        self.welcome_visible = false;
    }

    pub fn begin_command_subpanel(&mut self, title: impl Into<String>) {
        if let Some(current) = self.command_panel.take() {
            self.command_panel_history.push(current);
        }
        self.command_panel = Some(CommandPanel {
            title: sanitize_inline(&title.into()),
            body: String::new(),
            menu: Vec::new(),
            selected_menu_item: 0,
        });
        self.command_panel_scroll = 0;
        self.welcome_visible = false;
    }

    pub fn back_command_panel(&mut self) -> bool {
        let Some(previous) = self.command_panel_history.pop() else {
            return false;
        };
        self.command_panel = Some(previous);
        self.command_panel_scroll = 0;
        true
    }

    pub fn close_command_panel(&mut self) {
        self.command_panel = None;
        self.command_panel_history.clear();
        self.command_panel_scroll = 0;
    }

    pub fn mode_menu_open(&self) -> bool {
        self.mode_menu.is_some()
    }

    /// Opens the mode menu, or closes it when already open. The cursor starts
    /// on the applied mode so opening twice never hides where you are.
    pub fn toggle_mode_menu(&mut self) {
        if self.mode_menu.is_some() {
            self.mode_menu = None;
            return;
        }
        let selected = MODE_MENU
            .iter()
            .position(|item| item.mode == self.mode_override)
            .unwrap_or(0);
        self.mode_menu = Some(selected);
    }

    pub fn close_mode_menu(&mut self) {
        self.mode_menu = None;
    }

    pub fn move_mode_menu_selection(&mut self, delta: isize) {
        let Some(current) = self.mode_menu else {
            return;
        };
        let len = MODE_MENU.len() as isize;
        self.mode_menu = Some((current as isize + delta).rem_euclid(len) as usize);
    }

    pub fn mode_menu_selected(&self) -> Option<ModeMenuItem> {
        let index = self.mode_menu?;
        MODE_MENU.get(index).copied()
    }

    /// Applies the highlighted row. `自动` clears the override and lets the
    /// next task be classified from its content.
    pub fn apply_mode_menu(&mut self) {
        let Some(item) = self.mode_menu_selected() else {
            return;
        };
        self.mode_override = item.mode;
        self.mode_menu = None;
    }

    pub fn mode_override(&self) -> Option<CognitiveMode> {
        self.mode_override
    }

    /// What automatic inference produced for the most recent plain task.
    pub fn set_last_inferred(&mut self, mode: CognitiveMode) {
        self.last_inferred = Some(mode);
    }

    /// What the status bar shows: a manual choice, otherwise what inference
    /// would pick next. Falling back to the last run keeps it defined before
    /// any task has been submitted.
    pub fn displayed_mode(&self) -> CognitiveMode {
        self.mode_override
            .or(self.last_inferred)
            .unwrap_or(self.mode)
    }

    /// The mode a task will actually run in: a manual choice wins over
    /// inference.
    pub fn effective_mode(&self, inferred: CognitiveMode) -> CognitiveMode {
        self.mode_override.unwrap_or(inferred)
    }

    /// The character budget the runtime compacts against. The status bar
    /// reports usage as a share of this, not of a model token window, because
    /// no token window metadata exists.
    pub fn set_context_budget(&mut self, budget: usize) {
        self.context_budget = budget.max(1);
    }

    /// How many characters the most recent model request carried.
    pub fn set_context_chars(&mut self, used: usize) {
        self.context_chars = used;
    }

    fn context_percent(&self) -> usize {
        self.context_chars
            .saturating_mul(100)
            .checked_div(self.context_budget.max(1))
            .unwrap_or_default()
            .min(100)
    }

    pub fn set_pointer(&mut self, column: u16, row: u16) {
        self.pointer = Some((column, row));
    }

    pub fn clear_pointer(&mut self) {
        self.pointer = None;
    }

    /// The proposal shown by `/reframe`, waiting for the user's choice.
    pub fn set_pending_reframe(&mut self, proposal: crate::reframe::ReframeProposal) {
        self.pending_reframe = Some(proposal);
    }

    pub fn pending_reframe(&self) -> Option<&crate::reframe::ReframeProposal> {
        self.pending_reframe.as_ref()
    }

    pub fn take_pending_reframe(&mut self) -> Option<crate::reframe::ReframeProposal> {
        self.pending_reframe.take()
    }

    pub fn command_panel_open(&self) -> bool {
        self.command_panel.is_some()
    }

    pub fn set_command_panel_menu(&mut self, items: Vec<CommandMenuItem>) {
        if let Some(panel) = self.command_panel.as_mut() {
            panel.menu = items;
            panel.selected_menu_item = 0;
        }
    }

    pub fn command_panel_menu_open(&self) -> bool {
        self.command_panel
            .as_ref()
            .is_some_and(|panel| !panel.menu.is_empty())
    }

    pub fn move_command_panel_selection(&mut self, delta: isize) {
        let Some(panel) = self.command_panel.as_mut() else {
            return;
        };
        let count = panel.menu.len();
        if count == 0 {
            return;
        }
        panel.selected_menu_item = (panel.selected_menu_item.min(count - 1) as isize + delta)
            .rem_euclid(count as isize) as usize;
    }

    pub fn selected_command_panel_item(&self) -> Option<CommandMenuItem> {
        let panel = self.command_panel.as_ref()?;
        panel
            .menu
            .get(
                panel
                    .selected_menu_item
                    .min(panel.menu.len().saturating_sub(1)),
            )
            .cloned()
    }

    pub fn scroll_command_panel_up(&mut self, lines: usize) {
        self.command_panel_scroll = self.command_panel_scroll.saturating_sub(lines);
    }

    pub fn scroll_command_panel_down(&mut self, lines: usize) {
        self.command_panel_scroll = self.command_panel_scroll.saturating_add(lines);
    }

    pub fn set_input(&mut self, text: &str) {
        self.input = sanitize_input(text);
        self.input_cursor = self.input.len();
        self.input_scroll = 0;
        self.selection_anchor = None;
        self.clear_transcript_selection();
        self.selected_command = 0;
        self.slash_drawer_dismissed = false;
        self.pending_confirmation = None;
        self.history_cursor = None;
    }

    pub fn input(&self) -> String {
        self.input.clone()
    }

    pub fn clear_input(&mut self) {
        self.input.clear();
        self.input_cursor = 0;
        self.input_scroll = 0;
        self.selection_anchor = None;
        self.clear_transcript_selection();
        self.selected_command = 0;
        self.slash_drawer_dismissed = false;
        self.pending_confirmation = None;
        self.history_cursor = None;
    }

    pub fn take_input(&mut self) -> String {
        let input = self.input.clone();
        self.clear_input();
        input
    }

    pub fn record_input(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        if self.input_history.last().map(String::as_str) != Some(text) {
            self.input_history.push(text.to_string());
            if self.input_history.len() > 100 {
                self.input_history.remove(0);
            }
        }
        self.history_cursor = None;
    }

    pub fn history_active(&self) -> bool {
        self.history_cursor.is_some()
    }

    pub fn history_previous(&mut self) -> bool {
        if self.input_history.is_empty() {
            return false;
        }
        let index = match self.history_cursor {
            Some(index) => index.saturating_sub(1),
            None => self.input_history.len() - 1,
        };
        self.input = self.input_history[index].clone();
        self.input_cursor = self.input.len();
        self.input_scroll = 0;
        self.selected_command = 0;
        self.slash_drawer_dismissed = false;
        self.pending_confirmation = None;
        self.history_cursor = Some(index);
        true
    }

    pub fn history_next(&mut self) -> bool {
        let Some(index) = self.history_cursor else {
            return false;
        };
        if index + 1 < self.input_history.len() {
            self.history_cursor = Some(index + 1);
            self.input = self.input_history[index + 1].clone();
            self.input_cursor = self.input.len();
            self.input_scroll = 0;
            self.selected_command = 0;
            self.slash_drawer_dismissed = false;
        } else {
            self.history_cursor = None;
            self.clear_input();
        }
        true
    }

    pub fn take_submission(&mut self) -> String {
        if self.slash_drawer_open() {
            let entries = self.filtered_command_entries();
            if !entries.iter().any(|entry| entry.spec.name == self.input) {
                if entries.is_empty() {
                    self.dismiss_slash_drawer();
                } else {
                    self.complete_selected_command();
                }
                return String::new();
            }
        }
        self.take_input()
    }

    pub fn insert_char(&mut self, character: char) {
        if is_disallowed_control(character) || character == '\r' || character == '\n' {
            return;
        }
        self.insert_text(&character.to_string());
    }

    pub fn insert_text(&mut self, text: &str) {
        let text = sanitize_input(text);
        if text.is_empty() {
            return;
        }
        if let Some((start, end)) = self.selection_range() {
            self.input.replace_range(start..end, "");
            self.input_cursor = start;
        }
        self.input.insert_str(self.input_cursor, &text);
        self.input_cursor += text.len();
        self.selection_anchor = None;
        self.clear_transcript_selection();
        self.input_scroll = self.input_scroll.min(self.input_cursor);
        self.selected_command = 0;
        self.slash_drawer_dismissed = false;
        self.pending_confirmation = None;
        self.history_cursor = None;
    }

    pub fn backspace(&mut self) {
        self.pending_confirmation = None;
        if let Some((start, end)) = self.selection_range() {
            self.input.replace_range(start..end, "");
            self.input_cursor = start;
            self.selection_anchor = None;
            return;
        }
        let previous = previous_grapheme_boundary(&self.input, self.input_cursor);
        if previous < self.input_cursor {
            self.input.replace_range(previous..self.input_cursor, "");
            self.input_cursor = previous;
        }
    }

    pub fn delete(&mut self) {
        self.pending_confirmation = None;
        if let Some((start, end)) = self.selection_range() {
            self.input.replace_range(start..end, "");
            self.input_cursor = start;
            self.selection_anchor = None;
            return;
        }
        let next = next_grapheme_boundary(&self.input, self.input_cursor);
        if next > self.input_cursor {
            self.input.replace_range(self.input_cursor..next, "");
        }
    }

    pub fn move_left(&mut self) {
        self.selection_anchor = None;
        self.clear_transcript_selection();
        self.input_cursor = previous_grapheme_boundary(&self.input, self.input_cursor);
    }

    pub fn move_right(&mut self) {
        self.selection_anchor = None;
        self.clear_transcript_selection();
        self.input_cursor = next_grapheme_boundary(&self.input, self.input_cursor);
    }

    pub fn move_home(&mut self) {
        self.selection_anchor = None;
        self.clear_transcript_selection();
        self.input_cursor = 0;
    }

    pub fn move_end(&mut self) {
        self.selection_anchor = None;
        self.clear_transcript_selection();
        self.input_cursor = self.input.len();
    }

    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll_from_bottom = self.scroll_from_bottom.saturating_add(lines);
    }

    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_from_bottom = self.scroll_from_bottom.saturating_sub(lines);
        if self.scroll_from_bottom == 0 {
            self.unseen_activity = false;
        }
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_from_bottom = 0;
        self.unseen_activity = false;
    }

    pub fn tick(&mut self) {
        self.activity_frame = self.activity_frame.wrapping_add(1);
    }

    pub fn set_notice(&mut self, notice: impl Into<String>) {
        self.notice = Some(sanitize_inline(&notice.into()));
    }

    pub fn clear_notice(&mut self) {
        self.notice = None;
    }

    pub fn handle_follow_control(&mut self, column: u16, row: u16) -> bool {
        if self
            .follow_control_area
            .is_some_and(|area| area.contains((column, row).into()))
        {
            self.scroll_to_bottom();
            true
        } else {
            false
        }
    }

    pub fn handle_follow_control_hover(&mut self, column: u16, row: u16) -> bool {
        let hovered = self
            .follow_control_area
            .is_some_and(|area| area.contains((column, row).into()));
        if hovered != self.follow_control_hovered {
            self.follow_control_hovered = hovered;
            true
        } else {
            false
        }
    }

    pub fn status_summary(&self) -> String {
        let mut status = format!(
            "1A · {} · {} · {}风险",
            self.status.label_zh(),
            self.mode.label_zh(),
            self.risk.label_zh()
        );
        if let Some(session_id) = &self.session_id {
            let short = if session_id.chars().count() > 16 {
                format!(
                    "{}…{}",
                    session_id.chars().take(10).collect::<String>(),
                    session_id
                        .chars()
                        .rev()
                        .take(6)
                        .collect::<String>()
                        .chars()
                        .rev()
                        .collect::<String>()
                )
            } else {
                session_id.clone()
            };
            status.push_str(&format!(" · 会话 {short}"));
        }
        status
    }

    pub fn set_session_info(&mut self, session_id: impl Into<String>) {
        self.session_id = Some(session_id.into());
    }

    pub fn set_host_binding(&mut self, binding: HostBinding) {
        self.host_binding = binding;
    }

    pub fn host_binding(&self) -> &HostBinding {
        &self.host_binding
    }

    pub fn set_command_stage(&mut self, stage: CommandStage) {
        self.command_stage = stage;
    }

    pub fn command_stage(&self) -> CommandStage {
        self.command_stage
    }

    pub fn set_last_journal(&mut self, journal: impl Into<PathBuf>) {
        self.last_journal_path = Some(journal.into());
    }

    pub fn last_journal(&self) -> Option<&PathBuf> {
        self.last_journal_path.as_ref()
    }

    pub fn set_last_details(&mut self, details: impl Into<String>) {
        self.last_details = Some(sanitize_multiline(&details.into()));
    }

    pub fn last_details(&self) -> Option<&str> {
        self.last_details.as_deref()
    }

    pub fn open_details(&mut self) {
        self.details_open = true;
        self.details_scroll = 0;
        self.clear_selection();
    }

    pub fn close_details(&mut self) {
        self.details_open = false;
        self.details_scroll = 0;
    }

    pub fn details_open(&self) -> bool {
        self.details_open
    }

    pub fn scroll_details_up(&mut self, lines: usize) {
        self.details_scroll = self.details_scroll.saturating_add(lines);
    }

    pub fn scroll_details_down(&mut self, lines: usize) {
        self.details_scroll = self.details_scroll.saturating_sub(lines);
    }

    pub fn scroll_details_to_bottom(&mut self) {
        self.details_scroll = 0;
    }

    pub fn filtered_drawer_rows(&self) -> Vec<CommandDrawerRow> {
        drawer_rows(
            &self.input,
            &self.host_binding,
            self.command_stage,
            self.status == TuiStatus::Running,
            &self.recent_commands,
        )
    }

    pub fn filtered_command_entries(&self) -> Vec<CommandEntry> {
        self.filtered_drawer_rows()
            .into_iter()
            .map(|row| row.entry)
            .collect()
    }

    pub fn record_command_use(&mut self, id: TuiCommandId) {
        self.recent_commands.retain(|recent| *recent != id);
        self.recent_commands.insert(0, id);
        self.recent_commands.truncate(5);
    }

    pub fn arm_confirmation(&mut self, command: &str) {
        self.pending_confirmation = Some(command.to_string());
        self.set_notice(format!("危险操作：再次按 Enter 执行 {command}，Esc 取消"));
    }

    pub fn pending_confirmation(&self) -> Option<&str> {
        self.pending_confirmation.as_deref()
    }

    pub fn take_confirmation(&mut self) -> Option<String> {
        self.pending_confirmation.take()
    }

    pub fn clear_confirmation(&mut self) {
        self.pending_confirmation = None;
    }

    pub fn request_quit(&mut self) {
        self.quit_requested = true;
        self.set_notice("再次按 Enter 退出，Esc 取消");
    }

    pub fn quit_requested(&self) -> bool {
        self.quit_requested
    }

    pub fn cancel_quit(&mut self) {
        self.quit_requested = false;
    }

    pub fn take_quit_request(&mut self) -> bool {
        let requested = self.quit_requested;
        self.quit_requested = false;
        requested
    }

    pub fn set_run_metrics(&mut self, metrics: TuiRunMetrics) {
        self.run_metrics = metrics;
    }

    pub fn run_metrics(&self) -> TuiRunMetrics {
        self.run_metrics
    }

    pub fn finish_run_clock(&mut self) {
        if let Some(started) = self.run_started_unix_ms {
            self.run_metrics.elapsed_ms = now_unix_ms().saturating_sub(started);
        }
    }

    pub fn filtered_commands(&self) -> Vec<TuiCommandSpec> {
        visible_commands(
            &self.input,
            &self.host_binding,
            self.command_stage,
            self.status == TuiStatus::Running,
        )
    }

    pub fn slash_drawer_open(&self) -> bool {
        if self.slash_drawer_dismissed {
            return false;
        }
        let input = self.input.trim_start();
        input.starts_with('/') && !input.chars().any(char::is_whitespace)
    }

    pub fn dismiss_slash_drawer(&mut self) {
        self.slash_drawer_dismissed = true;
    }

    pub fn move_command_selection(&mut self, delta: isize) {
        let count = self.filtered_command_entries().len();
        if count == 0 {
            return;
        }
        let current = self.selected_command.min(count - 1) as isize;
        self.selected_command = (current + delta).rem_euclid(count as isize) as usize;
    }

    pub fn complete_selected_command(&mut self) -> bool {
        let entries = self.filtered_command_entries();
        let Some(entry) = entries.get(self.selected_command.min(entries.len().saturating_sub(1)))
        else {
            return false;
        };
        if let Some(reason) = entry.unavailable {
            self.set_notice(format!("{}  {}", entry.spec.name, reason));
            return false;
        }
        self.input = entry.spec.name.to_string();
        if entry.spec.supports_inline_args {
            self.input.push(' ');
        }
        self.input_cursor = self.input.len();
        self.input_scroll = 0;
        self.selection_anchor = None;
        self.slash_drawer_dismissed = false;
        if let Some(usage) = command_usage(entry.spec.id) {
            self.set_notice(format!("参数：{usage}"));
        }
        true
    }

    pub fn select_at(&mut self, column: u16, row: u16, extend: bool) {
        if let Some(geometry) = self.input_geometry
            && row == geometry.row
        {
            if extend && self.selection_anchor.is_none() {
                return;
            }
            let position = input_index_for_column(
                &self.input,
                geometry.visible_start,
                geometry.visible_end,
                geometry.text_x,
                geometry.text_width,
                column,
            );
            if !extend {
                self.selection_anchor = Some(position);
                self.clear_transcript_selection();
            }
            self.input_cursor = position;
            return;
        }

        if let Some(area) = self.transcript_area
            && area.contains((column, row).into())
        {
            if extend && self.transcript_anchor.is_none() {
                return;
            }
            let line_index = self
                .transcript_scroll_top
                .saturating_add(usize::from(row.saturating_sub(area.y)))
                .min(self.transcript_lines.len().saturating_sub(1));
            let Some(line) = self.transcript_lines.get(line_index) else {
                return;
            };
            let position = TranscriptPosition {
                line: line_index,
                index: text_index_for_column(&line.text, area.x, column),
            };
            if !extend || self.transcript_anchor.is_none() {
                self.transcript_anchor = Some(position);
                self.selection_anchor = None;
            }
            self.transcript_focus = Some(position);
        } else if !extend {
            self.clear_selection();
        }
    }

    pub fn begin_selection_drag(&mut self, column: u16, row: u16) {
        self.selection_dragging = true;
        self.selection_drag_column = column;
        self.selection_autoscroll = 0;
        self.select_at(column, row, false);
    }

    pub fn update_selection_drag(&mut self, column: u16, row: u16) {
        if !self.selection_dragging {
            return;
        }
        self.selection_drag_column = column;
        let Some(area) = self.transcript_area else {
            self.select_at(column, row, true);
            return;
        };
        if self.transcript_anchor.is_none() {
            self.select_at(column, row, true);
            return;
        }
        if row <= area.y {
            self.selection_autoscroll = -1;
            self.autoscroll_selection_once();
        } else if row >= area.bottom().saturating_sub(1) {
            self.selection_autoscroll = 1;
            self.autoscroll_selection_once();
        } else {
            self.selection_autoscroll = 0;
            let column = column.clamp(area.x, area.right().saturating_sub(1));
            self.select_at(column, row, true);
        }
    }

    pub fn end_selection_drag(&mut self) {
        self.selection_dragging = false;
        self.selection_autoscroll = 0;
    }

    pub fn selection_autoscroll_active(&self) -> bool {
        self.selection_dragging && self.selection_autoscroll != 0
    }

    pub fn autoscroll_selection_once(&mut self) -> bool {
        if !self.selection_autoscroll_active() || self.transcript_anchor.is_none() {
            return false;
        }
        let Some(area) = self.transcript_area else {
            self.selection_autoscroll = 0;
            return false;
        };
        let height = usize::from(area.height);
        if height == 0 || self.transcript_lines.is_empty() {
            self.selection_autoscroll = 0;
            return false;
        }
        let max_top = self.transcript_lines.len().saturating_sub(height);
        let current_top = self.transcript_scroll_top.min(max_top);
        let target_line = if self.selection_autoscroll < 0 {
            if current_top == 0 {
                self.selection_autoscroll = 0;
                return false;
            }
            self.scroll_up(1);
            current_top - 1
        } else {
            if current_top >= max_top {
                self.selection_autoscroll = 0;
                return false;
            }
            self.scroll_down(1);
            current_top
                .saturating_add(height)
                .min(self.transcript_lines.len().saturating_sub(1))
        };
        let line = &self.transcript_lines[target_line];
        let column = self
            .selection_drag_column
            .clamp(area.x, area.right().saturating_sub(1));
        self.transcript_focus = Some(TranscriptPosition {
            line: target_line,
            index: text_index_for_column(&line.text, area.x, column),
        });
        true
    }

    pub fn selected_input_text(&self) -> Option<String> {
        self.selection_range()
            .map(|(start, end)| self.input[start..end].to_string())
            .filter(|text| !text.is_empty())
    }

    pub fn selected_text(&self) -> Option<String> {
        self.selected_input_text()
            .or_else(|| self.selected_transcript_text())
    }

    pub fn clear_selection(&mut self) {
        self.selection_anchor = None;
        self.selection_dragging = false;
        self.selection_autoscroll = 0;
        self.clear_transcript_selection();
    }

    fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.selection_anchor?;
        let range = anchor.min(self.input_cursor)..anchor.max(self.input_cursor);
        (!range.is_empty()).then_some((range.start, range.end))
    }

    fn transcript_selection_range(&self) -> Option<(TranscriptPosition, TranscriptPosition)> {
        let anchor = self.transcript_anchor?;
        let focus = self.transcript_focus?;
        (anchor != focus).then(|| (anchor.min(focus), anchor.max(focus)))
    }

    fn selected_transcript_text(&self) -> Option<String> {
        let (start, end) = self.transcript_selection_range()?;
        let mut output = String::new();
        for line_index in start.line..=end.line {
            let line = &self.transcript_lines.get(line_index)?.text;
            let from = if line_index == start.line {
                start.index.min(line.len())
            } else {
                0
            };
            let to = if line_index == end.line {
                end.index.min(line.len())
            } else {
                line.len()
            };
            if line_index > start.line {
                output.push('\n');
            }
            output.push_str(&line[from..to]);
        }
        (!output.is_empty()).then_some(output)
    }

    fn clear_transcript_selection(&mut self) {
        self.transcript_anchor = None;
        self.transcript_focus = None;
        self.selection_dragging = false;
        self.selection_autoscroll = 0;
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(frame.area());

        if self.command_panel.is_some() {
            self.draw_command_panel(frame, chunks[0]);
        } else if self.welcome_visible && self.transcript.is_empty() {
            self.draw_welcome(frame, chunks[0]);
        } else {
            self.draw_transcript(frame, chunks[0]);
        }
        self.draw_separator(frame, chunks[1]);
        self.draw_activity(frame, chunks[2]);
        frame.render_widget(Clear, chunks[3]);
        self.draw_composer(frame, chunks[4]);
        self.draw_status_bar(frame, chunks[5]);

        if self.slash_drawer_open() {
            let rows = self.filtered_drawer_rows();
            let display_rows = rows
                .iter()
                .map(|row| 1 + usize::from(row.heading.is_some()))
                .sum::<usize>()
                .clamp(1, 12);
            let popup_height = (display_rows + 2) as u16;
            let popup_height = popup_height.min(chunks[0].height);
            let popup_width = chunks[0].width.min(86);
            let popup = Rect {
                x: chunks[0].x + chunks[0].width.saturating_sub(popup_width) / 2,
                y: chunks[0].bottom().saturating_sub(popup_height),
                width: popup_width,
                height: popup_height,
            };
            self.draw_command_drawer(frame, popup);
        }
        if self.mode_menu_open() {
            self.draw_mode_menu(frame, chunks[0]);
        }
        if self.details_open {
            self.draw_details(frame, frame.area());
        }
    }

    pub fn render_screen(&mut self, columns: usize, _rows: usize) -> String {
        let columns = columns.max(40);
        let instruction = format!(" You > {}", self.last_instruction);
        let mut output = format!(
            "{}\n{}\n{}\n",
            fit_row(&self.status_line(), columns),
            truncate_to_width(&instruction, columns),
            "─".repeat(columns)
        );
        if let Some(panel) = self.command_panel.as_ref() {
            output.push_str(&fit_row(
                &format!(" ▌ {}", panel.title),
                columns.saturating_sub(2),
            ));
            output.push('\n');
            for line in wrap_display(&panel.body, columns.saturating_sub(4)) {
                output.push_str("   ");
                output.push_str(&line);
                output.push('\n');
            }
            for (index, item) in panel.menu.iter().enumerate() {
                let selected = index == panel.selected_menu_item;
                let marker = if selected { "› " } else { "  " };
                output.push_str(&fit_row(
                    &format!(
                        "   {} {} {}",
                        marker,
                        fit_row(&item.label, 18),
                        item.description
                    ),
                    columns.saturating_sub(4),
                ));
                output.push('\n');
            }
        } else {
            for entry in &self.transcript {
                for line in wrap_display(&entry.text, columns.saturating_sub(2)) {
                    output.push(' ');
                    output.push_str(&line);
                    output.push('\n');
                }
            }
        }
        output.push_str(&format!(
            "{}\n› {}\n",
            fit_row(&self.activity_line(), columns),
            self.input
        ));
        output
    }

    fn draw_separator(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(
            Paragraph::new("─".repeat(usize::from(area.width)))
                .style(Style::default().fg(Color::DarkGray)),
            area,
        );
    }

    fn draw_transcript(&mut self, frame: &mut Frame, mut area: Rect) {
        if area.width > 2 {
            area = area.inner(Margin {
                horizontal: 1,
                vertical: 0,
            });
        }
        if area.is_empty() {
            return;
        }
        self.transcript_lines = self.build_transcript_lines(area.width);
        let total = self.transcript_lines.len();
        let full_height = usize::from(area.height);
        let max_scroll = total.saturating_sub(full_height);
        self.scroll_from_bottom = self.scroll_from_bottom.min(max_scroll);
        if self.scroll_from_bottom == 0 {
            self.unseen_activity = false;
        }
        let mut top = max_scroll.saturating_sub(self.scroll_from_bottom);
        let mut body = area;
        let mut header = (area.height >= 4)
            .then(|| self.prompt_header_for_line(top, area.width))
            .flatten();
        if header.is_some() {
            body = Rect::new(area.x, area.y + 1, area.width, area.height - 1);
            let body_height = usize::from(body.height);
            let body_max_scroll = total.saturating_sub(body_height);
            self.scroll_from_bottom = self.scroll_from_bottom.min(body_max_scroll);
            top = body_max_scroll.saturating_sub(self.scroll_from_bottom);
            header = self.prompt_header_for_line(top, area.width);
            if header.is_none() {
                body = area;
                top = max_scroll.saturating_sub(self.scroll_from_bottom.min(max_scroll));
            }
        }
        if let Some(header) = header {
            let header_area = Rect::new(area.x, area.y, area.width, 1);
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("› ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled(header, Style::default().add_modifier(Modifier::BOLD)),
                ]))
                .style(Style::default().bg(Color::Indexed(238)).fg(Color::White)),
                header_area,
            );
        }
        self.transcript_area = Some(body);
        self.transcript_scroll_top = top;
        for (offset, line) in self.transcript_lines[top..]
            .iter()
            .take(usize::from(body.height))
            .enumerate()
        {
            let line_area = Rect::new(
                body.x,
                body.y + u16::try_from(offset).unwrap_or(u16::MAX),
                body.width,
                1,
            );
            let line_background = line.background.or(match line.kind {
                TranscriptKind::Code => Some(CODE_BG),
                TranscriptKind::User => Some(Color::Indexed(238)),
                _ => None,
            });
            if let Some(background) = line_background {
                frame
                    .buffer_mut()
                    .set_style(line_area, Style::default().bg(background));
            }
            frame.render_widget(
                Paragraph::new(self.render_transcript_line(top + offset, line)),
                line_area,
            );
        }
    }

    fn draw_welcome(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }
        frame.render_widget(Clear, area);
        let mark_width = WELCOME_LOGO
            .iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or_default() as u16;
        let mark_height = WELCOME_LOGO.len() as u16;

        // The English caption shares one line with the domain, so it is wider
        // than the mark; the block takes whichever is wider and the mark is
        // centred inside it. No scheme: many terminals underline anything that
        // looks like a URL, and the domain is already highlighted.
        let prefix = "Super Individual Club - ";
        let name = "SKYNET";
        let tld = ".CLUB";
        let text_width =
            (prefix.chars().count() + name.chars().count() + tld.chars().count()) as u16;
        let block_width = mark_width.max(text_width);

        // The block needs its own room; below that the compact wordmark is used.
        if area.width >= block_width + 4 && area.height >= mark_height + 5 {
            let block_height = mark_height.saturating_add(3);
            let rect = Rect::new(
                area.x + area.width.saturating_sub(block_width) / 2,
                area.y + area.height.saturating_sub(block_height) / 2,
                block_width.min(area.width),
                block_height.min(area.height),
            );
            let mut lines = welcome_mark_lines()
                .into_iter()
                .map(|line| line.alignment(Alignment::Center))
                .collect::<Vec<_>>();
            lines.push(Line::raw(""));
            lines.push(
                Line::styled("超级个体俱乐部", Style::default().fg(CAPTION_GREY))
                    .alignment(Alignment::Center),
            );

            // Only the domain is highlighted, and the pointer brightens it. The
            // hit range is derived from the rendered text width rather than
            // hard-coded.
            let left = block_width.saturating_sub(text_width) / 2;
            let link_start = rect.x + left + prefix.chars().count() as u16;
            let link_width = (name.chars().count() + tld.chars().count()) as u16;
            let hovered = self.pointer.is_some_and(|(column, row)| {
                row == rect.y + mark_height + 2
                    && column >= link_start
                    && column < link_start + link_width
            });
            let link_colour = if hovered { LINK_HOVER } else { LINK };
            lines.push(
                Line::from(vec![
                    Span::styled(prefix, Style::default().fg(CAPTION_GREY)),
                    Span::styled(name, Style::default().fg(link_colour)),
                    Span::styled(tld, Style::default().fg(TLD_GREEN)),
                ])
                .alignment(Alignment::Center),
            );
            frame.render_widget(Paragraph::new(lines), rect);
        } else {
            let content_height = 1u16;
            let start_y = area
                .y
                .saturating_add(area.height.saturating_sub(content_height) / 2);
            let compact = Rect::new(area.x, start_y, area.width, area.height.min(content_height));
            let title = vec![Line::styled(
                "1-Agent",
                Style::default()
                    .fg(Color::Indexed(75))
                    .add_modifier(Modifier::BOLD),
            )];
            frame.render_widget(Paragraph::new(title).alignment(Alignment::Center), compact);
        }
    }

    fn build_transcript_lines(&self, width: u16) -> Vec<TranscriptVisualLine> {
        let mut lines: Vec<TranscriptVisualLine> = Vec::new();
        for entry in &self.transcript {
            if entry.kind == TranscriptKind::User {
                lines.push(blank_transcript_line(TranscriptKind::User, TuiTone::User));
            }
            if matches!(
                entry.kind,
                TranscriptKind::Heading
                    | TranscriptKind::Code
                    | TranscriptKind::Quote
                    | TranscriptKind::QueryResult
            ) && lines.last().is_some_and(|line| !line.text.is_empty())
            {
                lines.push(blank_transcript_line(
                    TranscriptKind::Assistant,
                    TuiTone::Default,
                ));
            }
            let (prefix, prefix_tone) = match entry.kind {
                TranscriptKind::User => ("› ", TuiTone::Success),
                TranscriptKind::Assistant => ("  ", TuiTone::Default),
                TranscriptKind::Heading => ("▌ ", entry.tone),
                TranscriptKind::Item => ("  • ", TuiTone::Muted),
                TranscriptKind::Quote => ("  ▌ ", TuiTone::Supplementary),
                TranscriptKind::QueryResult => ("  ◇ ", TuiTone::Muted),
                TranscriptKind::Evidence => ("  ↳ ", TuiTone::Evidence),
                TranscriptKind::Code => ("  ", TuiTone::Code),
                TranscriptKind::Metadata => ("  ", TuiTone::Muted),
                TranscriptKind::System => ("Sys ", TuiTone::Muted),
                TranscriptKind::Error => ("Err ", TuiTone::Danger),
            };
            let label = entry.label.as_deref().unwrap_or_default();
            if entry.kind == TranscriptKind::Code {
                let language = if label.is_empty() { "code" } else { label };
                let header = format!("  ╭─ {language}");
                let header_prefix = "  ╭─ ";
                lines.push(TranscriptVisualLine {
                    kind: TranscriptKind::Code,
                    tone: TuiTone::Success,
                    text: header.clone(),
                    prompt_text: None,
                    prefix_len: header_prefix.len(),
                    prefix_tone: TuiTone::Success,
                    label_range: Some(header_prefix.len()..header.len()),
                    label_tone: TuiTone::Accent,
                    syntax_ranges: Vec::new(),
                    background: Some(CODE_BG),
                });
                let code_prefix = "  │ ";
                let code_width = usize::from(width)
                    .saturating_sub(code_prefix.width())
                    .max(1);
                let diff_block = matches!(language, "diff" | "patch")
                    || entry.text.lines().any(|line| {
                        let line = line.trim_start();
                        line.starts_with("@@") || line.starts_with("+++") || line.starts_with("---")
                    });
                for text_line in entry.text.lines() {
                    for wrapped in wrap_display(text_line, code_width) {
                        let mut syntax_ranges = highlight_code_line(&wrapped, language);
                        for (range, _) in &mut syntax_ranges {
                            range.start += code_prefix.len();
                            range.end += code_prefix.len();
                        }
                        lines.push(TranscriptVisualLine {
                            kind: TranscriptKind::Code,
                            tone: TuiTone::Code,
                            text: format!("{code_prefix}{wrapped}"),
                            prompt_text: None,
                            prefix_len: code_prefix.len(),
                            prefix_tone: TuiTone::Success,
                            label_range: None,
                            label_tone: TuiTone::Default,
                            syntax_ranges,
                            background: Some(code_line_background(text_line, diff_block)),
                        });
                    }
                }
                lines.push(TranscriptVisualLine {
                    kind: TranscriptKind::Code,
                    tone: TuiTone::Success,
                    text: "  ╰────".to_string(),
                    prompt_text: None,
                    prefix_len: 0,
                    prefix_tone: TuiTone::Default,
                    label_range: None,
                    label_tone: TuiTone::Default,
                    syntax_ranges: Vec::new(),
                    background: Some(CODE_BG),
                });
                lines.push(blank_transcript_line(
                    TranscriptKind::Assistant,
                    TuiTone::Default,
                ));
                continue;
            }
            if entry.kind == TranscriptKind::QueryResult {
                let label = if label.is_empty() { "来源" } else { label };
                let header_prefix = "  ◇ ";
                let header = format!("{header_prefix}{label}");
                lines.push(TranscriptVisualLine {
                    kind: TranscriptKind::QueryResult,
                    tone: TuiTone::Muted,
                    text: header.clone(),
                    prompt_text: None,
                    prefix_len: header_prefix.len(),
                    prefix_tone: TuiTone::Muted,
                    label_range: Some(header_prefix.len()..header.len()),
                    label_tone: TuiTone::Muted,
                    syntax_ranges: Vec::new(),
                    background: None,
                });
                let body_prefix = "  │ ";
                let body_width = usize::from(width)
                    .saturating_sub(body_prefix.width())
                    .max(1);
                for text_line in entry.text.lines() {
                    for wrapped in wrap_display(text_line, body_width) {
                        lines.push(TranscriptVisualLine {
                            kind: TranscriptKind::QueryResult,
                            tone: TuiTone::Muted,
                            text: format!("{body_prefix}{wrapped}"),
                            prompt_text: None,
                            prefix_len: body_prefix.len(),
                            prefix_tone: TuiTone::Muted,
                            label_range: None,
                            label_tone: TuiTone::Muted,
                            syntax_ranges: Vec::new(),
                            background: None,
                        });
                    }
                }
                lines.push(blank_transcript_line(
                    TranscriptKind::Assistant,
                    TuiTone::Default,
                ));
                continue;
            }
            let first_prefix = if label.is_empty() {
                prefix.to_string()
            } else {
                format!("{prefix}{label} ")
            };
            let first_text_width = usize::from(width)
                .saturating_sub(first_prefix.width())
                .max(1);
            let continuation_width = usize::from(width).saturating_sub(prefix.width()).max(1);
            let mut first = true;
            for text_line in entry.text.lines() {
                let wrapped = if first {
                    wrap_first_then_rest(text_line, first_text_width, continuation_width)
                } else {
                    wrap_display(text_line, continuation_width)
                };
                for wrapped in wrapped {
                    let line_prefix = if first { &first_prefix } else { prefix };
                    let label_range =
                        (first && !label.is_empty()).then_some(prefix.len()..line_prefix.len());
                    lines.push(TranscriptVisualLine {
                        kind: entry.kind,
                        tone: entry.tone,
                        text: format!("{line_prefix}{wrapped}"),
                        prompt_text: (first && entry.kind == TranscriptKind::User)
                            .then(|| entry.text.clone()),
                        prefix_len: prefix.len(),
                        prefix_tone,
                        label_range,
                        label_tone: entry.label_tone,
                        syntax_ranges: Vec::new(),
                        background: None,
                    });
                    first = false;
                }
            }
            if matches!(
                entry.kind,
                TranscriptKind::User
                    | TranscriptKind::System
                    | TranscriptKind::Error
                    | TranscriptKind::Code
                    | TranscriptKind::Quote
                    | TranscriptKind::QueryResult
            ) {
                let (kind, tone) = if entry.kind == TranscriptKind::User {
                    (TranscriptKind::User, TuiTone::User)
                } else {
                    (TranscriptKind::Assistant, TuiTone::Default)
                };
                lines.push(blank_transcript_line(kind, tone));
                if entry.kind == TranscriptKind::User {
                    lines.push(blank_transcript_line(
                        TranscriptKind::Assistant,
                        TuiTone::Default,
                    ));
                }
            }
        }
        lines
    }

    fn prompt_header_for_line(&self, first_visible: usize, width: u16) -> Option<String> {
        if width < 16 {
            return None;
        }
        let first = self.transcript_lines.get(first_visible)?;
        if first.kind == TranscriptKind::User
            && first
                .text
                .strip_prefix("› ")
                .is_some_and(|text| !text.trim().is_empty())
        {
            return None;
        }
        let prompt = self
            .transcript_lines
            .iter()
            .take(first_visible.saturating_add(1))
            .rev()
            .find_map(|line| {
                (line.kind == TranscriptKind::User)
                    .then_some(line.prompt_text.as_deref())
                    .flatten()
            })?;
        let message = sanitize_inline(prompt);
        if message.is_empty() {
            return None;
        }
        Some(truncate_to_width(
            &message,
            usize::from(width.saturating_sub(2)),
        ))
    }

    fn render_transcript_line(
        &self,
        line_index: usize,
        line: &TranscriptVisualLine,
    ) -> Line<'static> {
        let selection = self
            .transcript_selection_range()
            .and_then(|(start, end)| {
                (line_index >= start.line && line_index <= end.line).then_some((
                    if line_index == start.line {
                        start.index
                    } else {
                        0
                    },
                    if line_index == end.line {
                        end.index
                    } else {
                        line.text.len()
                    },
                ))
            })
            .map(|(start, end)| start.min(line.text.len())..end.min(line.text.len()))
            .filter(|range| !range.is_empty());
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut run_start = 0;
        let mut run_tone = None;
        let mut run_selected = false;
        for (index, _) in line
            .text
            .char_indices()
            .chain(std::iter::once((line.text.len(), '\0')))
        {
            let tone = transcript_tone_at(line, index);
            let selected = selection
                .as_ref()
                .is_some_and(|range| range.contains(&index));
            if let Some(previous_tone) = run_tone
                && (previous_tone != tone || run_selected != selected || index == line.text.len())
            {
                let mut style = tone_style(previous_tone);
                if run_selected {
                    style = style.add_modifier(Modifier::REVERSED);
                }
                spans.push(Span::styled(line.text[run_start..index].to_string(), style));
                run_start = index;
            }
            if index < line.text.len() {
                run_tone = Some(tone);
                run_selected = selected;
            }
        }
        if spans.is_empty() && line.text.is_empty() {
            Line::raw("")
        } else {
            Line::from(spans)
        }
    }

    fn draw_activity(&mut self, frame: &mut Frame, mut area: Rect) {
        if area.width > 2 {
            area = area.inner(Margin {
                horizontal: 1,
                vertical: 0,
            });
        }
        self.follow_control_area = None;
        if self.notice.is_none()
            && let Some(label) = self.follow_control_label(area.width)
        {
            let width = label.width() as u16;
            let target = Rect::new(
                area.x + area.width.saturating_sub(width) / 2,
                area.y,
                width,
                1,
            );
            let mut style = Style::default()
                .bg(Color::Indexed(238))
                .fg(Color::Indexed(75))
                .add_modifier(Modifier::BOLD);
            if self.follow_control_hovered {
                style = style.add_modifier(Modifier::REVERSED);
            }
            frame.render_widget(Paragraph::new(label).style(style), target);
            self.follow_control_area = Some(target);
            return;
        }
        self.follow_control_hovered = false;
        if self.notice.is_none() && self.status == TuiStatus::Running {
            frame.render_widget(
                Paragraph::new(self.activity_line()).style(
                    Style::default()
                        .fg(UI_ACCENT)
                        .bg(BANNER_BG)
                        .add_modifier(Modifier::BOLD),
                ),
                area,
            );
            return;
        }
        let style = if self.notice.is_some() {
            Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(MUTED_TEXT)
        };
        frame.render_widget(Paragraph::new(self.activity_line()).style(style), area);
    }

    fn draw_composer(&mut self, frame: &mut Frame, mut area: Rect) {
        if area.width > 2 {
            area = area.inner(Margin {
                horizontal: 1,
                vertical: 0,
            });
        }
        let background = Style::default().bg(INPUT_BG).fg(Color::Indexed(255));
        frame.render_widget(Block::default().style(background), area);
        if area.width < 4 || area.height < 3 {
            self.input_geometry = None;
            return;
        }

        let input_area = Rect {
            x: area.x,
            y: area.y + 1,
            width: area.width,
            height: 1,
        };
        let text_x = input_area.x + 2;
        let text_width = input_area.width.saturating_sub(2);
        self.ensure_input_cursor_visible(text_width);
        let visible_end = visible_input_end(&self.input, self.input_scroll, text_width);
        self.input_geometry = Some(InputGeometry {
            row: input_area.y,
            text_x,
            text_width,
            visible_start: self.input_scroll,
            visible_end,
        });

        let mut spans = vec![Span::styled(
            "› ",
            Style::default()
                .fg(UI_ACCENT)
                .bg(BANNER_BG)
                .add_modifier(Modifier::BOLD),
        )];
        spans.extend(self.input_spans(visible_end));
        frame.render_widget(
            Paragraph::new(Line::from(spans)).style(background),
            input_area,
        );

        let cursor_column =
            text_x + self.input[self.input_scroll..self.input_cursor].width() as u16;
        frame.set_cursor_position((
            cursor_column.min(area.right().saturating_sub(1)),
            input_area.y,
        ));
    }

    fn draw_status_bar(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }
        let model = self
            .provider_label
            .rsplit_once('/')
            .map(|(_, model)| model)
            .unwrap_or(&self.provider_label);
        let left = format!(
            " {model} · {} · / 命令 · Ctrl+T 模式",
            self.displayed_mode().label_zh()
        );
        let right = format!("上下文 {}% ", self.context_percent());
        let style = Style::default().fg(MUTED_TEXT);
        let right_width = right.width() as u16;
        if area.width > right_width {
            let right_area = Rect {
                x: area.right() - right_width,
                y: area.y,
                width: right_width,
                height: 1,
            };
            frame.render_widget(Paragraph::new(Line::styled(right, style)), right_area);
            let left_area = Rect {
                width: area.width - right_width,
                ..area
            };
            let text = truncate_to_width(&left, left_area.width as usize);
            frame.render_widget(Paragraph::new(Line::styled(text, style)), left_area);
        } else {
            let text = truncate_to_width(&left, area.width as usize);
            frame.render_widget(Paragraph::new(Line::styled(text, style)), area);
        }
    }

    fn input_spans(&self, visible_end: usize) -> Vec<Span<'static>> {
        let selection = self
            .selection_range()
            .map(|(start, end)| start.max(self.input_scroll)..end.min(visible_end))
            .filter(|range| !range.is_empty());
        let Some(selection) = selection else {
            return vec![Span::raw(
                self.input[self.input_scroll..visible_end].to_string(),
            )];
        };
        let mut spans = Vec::new();
        if self.input_scroll < selection.start {
            spans.push(Span::raw(
                self.input[self.input_scroll..selection.start].to_string(),
            ));
        }
        spans.push(Span::styled(
            self.input[selection.clone()].to_string(),
            Style::default().add_modifier(Modifier::REVERSED),
        ));
        if selection.end < visible_end {
            spans.push(Span::raw(
                self.input[selection.end..visible_end].to_string(),
            ));
        }
        spans
    }

    fn draw_details(&mut self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }
        let details = self
            .last_details
            .as_deref()
            .unwrap_or("当前会话还没有可显示的结构化运行详情。");
        let text = format!("运行详情 · Esc 关闭 · ↑/↓ 滚动\n\n{details}");
        let content_width = usize::from(area.width.saturating_sub(4).max(1));
        let content_height = usize::from(area.height.saturating_sub(4).max(1));
        let wrapped_rows = text
            .lines()
            .map(|line| wrap_display(line, content_width).len())
            .sum::<usize>();
        let max_scroll = wrapped_rows.saturating_sub(content_height);
        self.details_scroll = self.details_scroll.min(max_scroll);
        let top = max_scroll.saturating_sub(self.details_scroll);

        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(vec![
                Line::raw(" ".repeat(usize::from(area.width)));
                usize::from(area.height)
            ])
            .style(Style::default().bg(Color::Indexed(235))),
            area,
        );
        frame.render_widget(
            Paragraph::new(text)
                .style(Style::default().fg(Color::White).bg(Color::Indexed(235)))
                .wrap(Wrap { trim: false })
                .scroll((u16::try_from(top).unwrap_or(u16::MAX), 0))
                .block(
                    Block::bordered()
                        .border_style(
                            Style::default()
                                .fg(UI_ACCENT)
                                .bg(Color::Indexed(235))
                                .add_modifier(Modifier::BOLD),
                        )
                        .style(Style::default().bg(Color::Indexed(235))),
                ),
            area,
        );
    }

    fn draw_mode_menu(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() || area.height < 3 {
            return;
        }
        let width = 52.min(area.width.saturating_sub(2)).max(20);
        let height = (u16::try_from(MODE_MENU.len()).unwrap_or(u16::MAX) + 2).min(area.height);
        let rect = Rect {
            x: area.x + area.width.saturating_sub(width) / 2,
            y: area.bottom().saturating_sub(height),
            width,
            height,
        };
        let selected = self
            .mode_menu
            .unwrap_or(0)
            .min(MODE_MENU.len().saturating_sub(1));
        let items = MODE_MENU
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let is_selected = index == selected;
                let is_active = item.mode == self.mode_override;
                let bar = if is_selected { "▌ " } else { "  " };
                let marker = if is_active { "●" } else { " " };
                let label_style = if is_selected {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(MENU_LABEL)
                };
                ListItem::new(Line::from(vec![
                    Span::styled(bar, Style::default().fg(UI_ACCENT)),
                    Span::styled(
                        format!("{marker} "),
                        Style::default().fg(if is_active { UI_ACCENT } else { MENU_HINT }),
                    ),
                    Span::styled(fit_row(item.label, 12), label_style),
                    Span::styled(item.description, Style::default().fg(MENU_HINT)),
                ]))
            })
            .collect::<Vec<_>>();
        let mut state = ListState::default();
        state.select(Some(selected));
        frame.render_widget(Clear, rect);
        frame.render_stateful_widget(
            List::new(items)
                .style(Style::default().bg(MENU_BG))
                .block(menu_block(
                    "切换模式",
                    "↑↓ 选择 · Enter 确认 · Esc 返回",
                    MENU_BG,
                ))
                .highlight_style(Style::default().bg(MENU_SELECTED_BG)),
            rect,
            &mut state,
        );
    }

    pub fn command_panel_breadcrumb(&self) -> Option<String> {
        let panel = self.command_panel.as_ref()?;
        let mut parts = self
            .command_panel_history
            .iter()
            .map(|entry| entry.title.clone())
            .collect::<Vec<_>>();
        parts.push(panel.title.clone());
        Some(parts.join(" › "))
    }

    fn draw_command_panel(&mut self, frame: &mut Frame, area: Rect) {
        let breadcrumb = self
            .command_panel_breadcrumb()
            .unwrap_or_else(|| " 命令 ".to_string());
        let Some(panel) = self.command_panel.clone() else {
            return;
        };
        frame.render_widget(Clear, area);
        let hint = if panel.menu.is_empty() {
            ""
        } else {
            "↑↓ 选择 · Enter 确认 · Esc 返回"
        };
        let block = menu_block(&breadcrumb, hint, Color::Reset);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        if inner.is_empty() {
            return;
        }
        let menu_height = panel
            .menu
            .len()
            .min(8)
            .min(usize::from(inner.height.saturating_sub(2)));
        let content_width = usize::from(inner.width.saturating_sub(2).max(1));
        let wrapped_rows = panel
            .body
            .lines()
            .map(|line| wrap_display(line, content_width).len())
            .sum::<usize>();
        // The body takes only the rows it needs, so a short panel keeps its
        // menu next to the text instead of pinning it to the far edge.
        let reserved = menu_height + usize::from(menu_height > 0) * 2;
        let max_body = usize::from(inner.height).saturating_sub(reserved).max(1);
        let body_height = u16::try_from(wrapped_rows)
            .unwrap_or(u16::MAX)
            .min(u16::try_from(max_body).unwrap_or(u16::MAX));
        let body_area = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: body_height,
        };
        let max_scroll = wrapped_rows.saturating_sub(usize::from(body_height));
        self.command_panel_scroll = self.command_panel_scroll.min(max_scroll);
        let top = self.command_panel_scroll;
        if !body_area.is_empty() {
            frame.render_widget(
                Paragraph::new(panel.body)
                    .style(Style::default().fg(Color::Reset))
                    .wrap(Wrap { trim: false })
                    .scroll((u16::try_from(top).unwrap_or(u16::MAX), 0)),
                Rect {
                    x: body_area.x.saturating_add(1),
                    y: body_area.y,
                    width: body_area.width.saturating_sub(2),
                    height: body_area.height,
                },
            );
        }
        if menu_height > 0 {
            let divider = Line::styled(
                "─".repeat(usize::from(inner.width)),
                Style::default().fg(DRAWER_BORDER),
            );
            frame.render_widget(
                Paragraph::new(divider),
                Rect {
                    x: inner.x,
                    y: body_area.bottom(),
                    width: inner.width,
                    height: 1,
                },
            );
            let menu_area = Rect {
                x: inner.x.saturating_add(1),
                y: body_area.bottom().saturating_add(1),
                width: inner.width.saturating_sub(2),
                height: u16::try_from(menu_height).unwrap_or(u16::MAX),
            };
            let selected = panel
                .selected_menu_item
                .min(panel.menu.len().saturating_sub(1));
            let items = panel
                .menu
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    let bar = if index == selected { "▌ " } else { "  " };
                    ListItem::new(Line::from(vec![
                        Span::styled(bar, Style::default().fg(UI_ACCENT)),
                        Span::styled(
                            fit_row(&item.label, 18),
                            Style::default().fg(UI_ACCENT).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            item.description.clone(),
                            Style::default().fg(COMMAND_DESCRIPTION),
                        ),
                    ]))
                })
                .collect::<Vec<_>>();
            let mut menu_state = ListState::default();
            menu_state.select(Some(
                panel.selected_menu_item.min(items.len().saturating_sub(1)),
            ));
            frame.render_stateful_widget(
                List::new(items).highlight_style(Style::default().bg(MENU_SELECTED_BG)),
                menu_area,
                &mut menu_state,
            );
        }
    }

    fn draw_command_drawer(&self, frame: &mut Frame, area: Rect) {
        let rows = self.filtered_drawer_rows();
        let selected = rows
            .len()
            .checked_sub(1)
            .map(|last| self.selected_command.min(last));
        let mut items = Vec::new();
        let mut selected_item = None;
        for (index, row) in rows.iter().enumerate() {
            if let Some(heading) = row.heading {
                items.push(ListItem::new(Line::styled(
                    format!("  {heading}"),
                    Style::default().fg(MUTED_TEXT).add_modifier(Modifier::BOLD),
                )));
            }
            let entry = row.entry;
            let is_selected = selected == Some(index);
            let bar = if is_selected { "▌ " } else { "  " };
            let (name_style, description_style) = if entry.unavailable.is_some() {
                (
                    Style::default().fg(MUTED_TEXT),
                    Style::default().fg(MUTED_TEXT),
                )
            } else {
                (
                    Style::default().fg(UI_ACCENT).add_modifier(Modifier::BOLD),
                    Style::default().fg(COMMAND_DESCRIPTION),
                )
            };
            let detail = match entry.unavailable {
                Some(reason) => format!("{}  [{reason}]", entry.spec.description),
                None => entry.spec.description.to_string(),
            };
            items.push(ListItem::new(Line::from(vec![
                Span::styled(bar, Style::default().fg(UI_ACCENT)),
                Span::styled(fit_row(entry.spec.name, 16), name_style),
                Span::styled(detail, description_style),
            ])));
            if is_selected {
                selected_item = Some(items.len() - 1);
            }
        }
        if items.is_empty() {
            items.push(ListItem::new(Line::styled(
                " 没有匹配的命令",
                Style::default().fg(MUTED_TEXT),
            )));
            selected_item = Some(0);
        }
        let mut state = ListState::default();
        state.select(selected_item);
        frame.render_widget(Clear, area);
        let block = menu_block(
            self.command_drawer_title(),
            "↑↓ 选择 · Tab 补全 · Esc 关闭",
            MENU_BG,
        );
        frame.render_stateful_widget(
            List::new(items)
                .style(Style::default().bg(MENU_BG))
                .block(block)
                .highlight_style(Style::default().bg(MENU_SELECTED_BG)),
            area,
            &mut state,
        );
    }

    fn follow_control_label(&self, width: u16) -> Option<&'static str> {
        if self.scroll_from_bottom == 0
            || self.selection_anchor.is_some()
            || self.transcript_anchor.is_some()
        {
            return None;
        }
        let labels = if self.unseen_activity {
            [
                " 有新活动 · ↓ 返回底部 · Esc ",
                " 有新活动 · ↓ 底部 · Esc ",
                " New · ↓ Bottom · esc ",
                " ↓ Bottom · esc ",
                " ↓ ",
            ]
        } else {
            [
                " ↓ 返回底部 · Esc ",
                " ↓ 返回底部 ",
                " ↓ 底部 · Esc ",
                " ↓ 底部 ",
                " ↓ ",
            ]
        };
        labels
            .into_iter()
            .find(|label| label.width() <= usize::from(width))
    }

    fn ensure_input_cursor_visible(&mut self, width: u16) {
        let width = usize::from(width.saturating_sub(1).max(1));
        if self.input_cursor < self.input_scroll {
            self.input_scroll = self.input_cursor;
        }
        while self.input[self.input_scroll..self.input_cursor].width() > width {
            self.input_scroll = next_grapheme_boundary(&self.input, self.input_scroll);
        }
    }

    fn status_line(&self) -> String {
        let model = self
            .provider_label
            .rsplit_once('/')
            .map(|(_, model)| model)
            .unwrap_or(&self.provider_label);
        format!(
            " 1A · {} · {} · {}风险 · {} · 最多 {} 步 / {} 秒 · {}",
            self.status.label_zh(),
            self.mode.label_zh(),
            self.risk.label_zh(),
            model,
            self.max_steps,
            self.timeout_seconds,
            self.host_binding.kind().label_zh()
        )
    }

    fn command_drawer_title(&self) -> &'static str {
        if self.host_binding.is_workshop_master() {
            " 总控命令 "
        } else {
            " 命令 "
        }
    }

    fn activity_line(&self) -> String {
        if let Some(notice) = &self.notice {
            return format!(" ✓ {notice}");
        }
        let metrics = self.run_metrics_line();
        match self.status {
            TuiStatus::Running => {
                let frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
                format!(
                    " {} Working... {} · {metrics} · Ctrl+C 取消",
                    frames[self.activity_frame % frames.len()],
                    self.elapsed_label()
                )
            }
            TuiStatus::Idle => " 就绪".to_string(),
            TuiStatus::Completed => format!(" ✓ 已完成 · {} · {metrics}", self.elapsed_label()),
            TuiStatus::Failed => format!(" 失败 · {} · {metrics}", self.elapsed_label()),
            TuiStatus::Cancelled => format!(" 已取消 · {} · {metrics}", self.elapsed_label()),
        }
    }

    fn elapsed_label(&self) -> String {
        let elapsed = if self.status == TuiStatus::Running {
            self.run_started_unix_ms
                .map(|started| now_unix_ms().saturating_sub(started))
                .unwrap_or_default()
        } else {
            self.run_metrics.elapsed_ms
        };
        let seconds = elapsed / 1000;
        if seconds < 60 {
            format!("{seconds}s")
        } else {
            format!("{}m{:02}s", seconds / 60, seconds % 60)
        }
    }

    fn run_metrics_line(&self) -> String {
        if !self.run_metrics.usage_seen {
            return "↑— ↓— · cache —".to_string();
        }
        let total_cache = self
            .run_metrics
            .cache_hit_tokens
            .saturating_add(self.run_metrics.cache_miss_tokens);
        let cache = self
            .run_metrics
            .cache_hit_tokens
            .saturating_mul(100)
            .saturating_add(total_cache / 2)
            .checked_div(total_cache)
            .map(|percent| format!("cache {percent}%"))
            .unwrap_or_else(|| "cache —".to_string());
        format!(
            "↑{} ↓{} · {cache}",
            format_token_count(self.run_metrics.input_tokens),
            format_token_count(self.run_metrics.output_tokens)
        )
    }

    fn push_entry(
        &mut self,
        kind: TranscriptKind,
        tone: TuiTone,
        label: Option<String>,
        label_tone: TuiTone,
        entry: String,
    ) {
        if self.scroll_from_bottom > 0 {
            self.unseen_activity = true;
        }
        self.transcript.push(TranscriptEntry {
            kind,
            tone,
            label: label.map(|label| sanitize_inline(&label)),
            label_tone,
            text: sanitize_multiline(&entry),
        });
        if self.transcript.len() > 200 {
            self.transcript.remove(0);
        }
    }

    fn push_code_block(&mut self, language: &str, lines: &[&str]) {
        let mut code = lines.join("\n");
        while code.ends_with('\n') {
            code.pop();
        }
        self.push_entry(
            TranscriptKind::Code,
            TuiTone::Code,
            (!language.is_empty()).then(|| language.to_string()),
            TuiTone::Muted,
            code,
        );
    }

    fn flush_markdown_prose(&mut self, prose: &mut Vec<String>) {
        if let Some(text) = compact_markdown_prose(prose) {
            self.assistant_body(text);
        }
        prose.clear();
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

fn compact_markdown_prose(lines: &[String]) -> Option<String> {
    let mut output = Vec::new();
    for line in lines {
        let line = line.trim_end();
        if line.trim().is_empty() {
            if output.is_empty() || output.last().is_some_and(|line: &&str| line.is_empty()) {
                continue;
            }
            output.push("");
        } else {
            output.push(line);
        }
    }
    while output.last().is_some_and(|line| line.is_empty()) {
        output.pop();
    }
    (!output.is_empty()).then(|| output.join("\n"))
}

pub fn sanitize_multiline(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '\n' => output.push('\n'),
            '\t' => output.push_str("    "),
            '\r' => output.push('�'),
            character if is_disallowed_control(character) => output.push('�'),
            character => output.push(character),
        }
    }
    output
}

pub fn sanitize_input(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character == '\n'
                || character == '\r'
                || character == '\t'
                || is_disallowed_control(character)
            {
                ' '
            } else {
                character
            }
        })
        .collect()
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn format_token_count(value: u64) -> String {
    if value < 1_000 {
        value.to_string()
    } else if value < 1_000_000 {
        format!("{:.1}k", value as f64 / 1_000.0)
    } else {
        format!("{:.1}M", value as f64 / 1_000_000.0)
    }
}

fn previous_grapheme_boundary(text: &str, cursor: usize) -> usize {
    text[..cursor]
        .grapheme_indices(true)
        .next_back()
        .map_or(0, |(index, _)| index)
}

fn next_grapheme_boundary(text: &str, cursor: usize) -> usize {
    text[cursor..]
        .grapheme_indices(true)
        .nth(1)
        .map_or(text.len(), |(index, _)| cursor + index)
}

fn input_index_for_column(
    text: &str,
    visible_start: usize,
    visible_end: usize,
    text_x: u16,
    text_width: u16,
    column: u16,
) -> usize {
    let relative = usize::from(column.saturating_sub(text_x)).min(usize::from(text_width));
    let visible = &text[visible_start..visible_end];
    let mut width = 0;
    for (index, grapheme) in visible.grapheme_indices(true) {
        let next = width + grapheme.width();
        if relative < next {
            return visible_start + index;
        }
        width = next;
    }
    visible_end
}

fn text_index_for_column(text: &str, text_x: u16, column: u16) -> usize {
    let relative = usize::from(column.saturating_sub(text_x));
    let mut width = 0;
    for (index, grapheme) in text.grapheme_indices(true) {
        let next = width + grapheme.width();
        if relative < next {
            return index;
        }
        width = next;
    }
    text.len()
}

fn transcript_tone_at(line: &TranscriptVisualLine, index: usize) -> TuiTone {
    if index < line.prefix_len {
        line.prefix_tone
    } else if line
        .label_range
        .as_ref()
        .is_some_and(|range| range.contains(&index))
    {
        line.label_tone
    } else if let Some((_, tone)) = line
        .syntax_ranges
        .iter()
        .find(|(range, _)| range.contains(&index))
    {
        *tone
    } else {
        line.tone
    }
}

fn tone_style(tone: TuiTone) -> Style {
    match tone {
        TuiTone::Default => Style::default().fg(Color::Reset),
        TuiTone::Accent => Style::default()
            .fg(Color::Indexed(75))
            .add_modifier(Modifier::BOLD),
        TuiTone::Success => Style::default()
            .fg(Color::LightGreen)
            .add_modifier(Modifier::BOLD),
        TuiTone::Warning => Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
        TuiTone::Danger => Style::default()
            .fg(Color::LightRed)
            .add_modifier(Modifier::BOLD),
        TuiTone::Evidence => Style::default().fg(Color::Cyan),
        TuiTone::Code => Style::default().fg(Color::Indexed(252)),
        TuiTone::Keyword => Style::default()
            .fg(Color::LightMagenta)
            .add_modifier(Modifier::BOLD),
        TuiTone::Function => Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
        TuiTone::Type => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        TuiTone::String => Style::default().fg(Color::LightGreen),
        TuiTone::Number => Style::default().fg(Color::Magenta),
        TuiTone::Comment => Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
        TuiTone::Reference => Style::default().fg(Color::LightCyan),
        TuiTone::Supplementary => Style::default()
            .fg(Color::Indexed(245))
            .add_modifier(Modifier::DIM),
        TuiTone::Muted => Style::default().fg(MUTED_TEXT),
        TuiTone::User => Style::default().fg(Color::White).bg(Color::Indexed(238)),
    }
}

fn blank_transcript_line(kind: TranscriptKind, tone: TuiTone) -> TranscriptVisualLine {
    TranscriptVisualLine {
        kind,
        tone,
        text: String::new(),
        prompt_text: None,
        prefix_len: 0,
        prefix_tone: TuiTone::Default,
        label_range: None,
        label_tone: TuiTone::Default,
        syntax_ranges: Vec::new(),
        background: None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodeLanguageFamily {
    C,
    Python,
    Rust,
    JavaScript,
    Java,
    Go,
    Shell,
    Sql,
    Generic,
}

fn code_language_family(language: &str) -> CodeLanguageFamily {
    if matches!(
        language,
        "c" | "cpp" | "c++" | "csharp" | "c#" | "cs" | "objc" | "objective-c" | "swift" | "dart"
    ) {
        CodeLanguageFamily::C
    } else if matches!(language, "python" | "py" | "ruby" | "rb") {
        CodeLanguageFamily::Python
    } else if matches!(language, "rust" | "rs") {
        CodeLanguageFamily::Rust
    } else if matches!(
        language,
        "javascript" | "js" | "jsx" | "typescript" | "ts" | "tsx" | "node"
    ) {
        CodeLanguageFamily::JavaScript
    } else if matches!(language, "java" | "kotlin" | "kts" | "scala" | "groovy") {
        CodeLanguageFamily::Java
    } else if matches!(language, "go" | "golang") {
        CodeLanguageFamily::Go
    } else if matches!(
        language,
        "bash" | "sh" | "shell" | "zsh" | "fish" | "powershell" | "pwsh" | "ps1"
    ) {
        CodeLanguageFamily::Shell
    } else if matches!(language, "sql" | "plsql" | "tsql") {
        CodeLanguageFamily::Sql
    } else {
        CodeLanguageFamily::Generic
    }
}

fn code_keywords(language: CodeLanguageFamily) -> &'static [&'static str] {
    const C: &[&str] = &[
        "auto",
        "break",
        "case",
        "const",
        "continue",
        "default",
        "do",
        "else",
        "extern",
        "for",
        "goto",
        "if",
        "inline",
        "register",
        "restrict",
        "return",
        "sizeof",
        "static",
        "switch",
        "typedef",
        "volatile",
        "while",
        "class",
        "namespace",
        "new",
        "delete",
        "public",
        "private",
        "protected",
        "virtual",
        "override",
        "final",
        "template",
        "typename",
        "constexpr",
        "noexcept",
        "struct",
        "enum",
        "union",
        "protocol",
        "func",
        "let",
        "var",
        "guard",
        "defer",
    ];
    const PYTHON: &[&str] = &[
        "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del",
        "elif", "else", "except", "False", "finally", "for", "from", "global", "if", "import",
        "in", "is", "lambda", "None", "nonlocal", "not", "or", "pass", "raise", "return", "True",
        "try", "while", "with", "yield", "match", "case",
    ];
    const RUST: &[&str] = &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
        "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait",
        "true", "type", "unsafe", "use", "where", "while",
    ];
    const JAVASCRIPT: &[&str] = &[
        "async",
        "await",
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "debugger",
        "default",
        "delete",
        "do",
        "else",
        "export",
        "extends",
        "false",
        "finally",
        "for",
        "from",
        "function",
        "if",
        "import",
        "in",
        "instanceof",
        "let",
        "new",
        "null",
        "of",
        "return",
        "static",
        "super",
        "switch",
        "this",
        "throw",
        "true",
        "try",
        "typeof",
        "undefined",
        "var",
        "void",
        "while",
        "with",
        "yield",
        "interface",
        "type",
        "enum",
        "implements",
        "private",
        "protected",
        "public",
        "readonly",
        "declare",
        "namespace",
    ];
    const JAVA: &[&str] = &[
        "abstract",
        "assert",
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "default",
        "do",
        "else",
        "enum",
        "extends",
        "final",
        "finally",
        "for",
        "goto",
        "if",
        "implements",
        "import",
        "instanceof",
        "interface",
        "native",
        "new",
        "package",
        "private",
        "protected",
        "public",
        "return",
        "static",
        "strictfp",
        "super",
        "switch",
        "synchronized",
        "this",
        "throw",
        "throws",
        "transient",
        "try",
        "volatile",
        "while",
        "var",
        "record",
        "sealed",
        "permits",
        "fun",
        "object",
        "companion",
        "data",
        "when",
        "val",
        "override",
        "open",
        "internal",
        "suspend",
    ];
    const GO: &[&str] = &[
        "break",
        "case",
        "chan",
        "const",
        "continue",
        "default",
        "defer",
        "else",
        "fallthrough",
        "for",
        "func",
        "go",
        "goto",
        "if",
        "import",
        "interface",
        "map",
        "package",
        "range",
        "return",
        "select",
        "struct",
        "switch",
        "type",
        "var",
        "nil",
        "true",
        "false",
    ];
    const SHELL: &[&str] = &[
        "if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac",
        "function", "in", "select", "until", "time",
    ];
    const SQL: &[&str] = &[
        "select", "from", "where", "join", "inner", "left", "right", "outer", "on", "group", "by",
        "order", "having", "insert", "update", "delete", "into", "values", "create", "alter",
        "drop", "table", "index", "and", "or", "not", "null", "as", "limit", "distinct",
    ];
    const GENERIC: &[&str] = &[
        "break",
        "case",
        "class",
        "const",
        "continue",
        "default",
        "else",
        "enum",
        "for",
        "function",
        "if",
        "import",
        "in",
        "interface",
        "let",
        "match",
        "namespace",
        "new",
        "return",
        "static",
        "struct",
        "switch",
        "try",
        "type",
        "use",
        "var",
        "while",
    ];
    match language {
        CodeLanguageFamily::C => C,
        CodeLanguageFamily::Python => PYTHON,
        CodeLanguageFamily::Rust => RUST,
        CodeLanguageFamily::JavaScript => JAVASCRIPT,
        CodeLanguageFamily::Java => JAVA,
        CodeLanguageFamily::Go => GO,
        CodeLanguageFamily::Shell => SHELL,
        CodeLanguageFamily::Sql => SQL,
        CodeLanguageFamily::Generic => GENERIC,
    }
}

fn is_builtin_type(word: &str, language: CodeLanguageFamily) -> bool {
    let common = matches!(
        word,
        "bool" | "boolean" | "byte" | "char" | "double" | "float"
    );
    if common {
        return true;
    }
    match language {
        CodeLanguageFamily::C => matches!(
            word,
            "int"
                | "long"
                | "short"
                | "signed"
                | "size_t"
                | "ssize_t"
                | "unsigned"
                | "void"
                | "uint8_t"
                | "uint16_t"
                | "uint32_t"
                | "uint64_t"
                | "int8_t"
                | "int16_t"
                | "int32_t"
                | "int64_t"
        ),
        CodeLanguageFamily::Python => matches!(
            word,
            "bytes" | "dict" | "int" | "list" | "object" | "set" | "str" | "tuple"
        ),
        CodeLanguageFamily::Rust => matches!(
            word,
            "f32"
                | "f64"
                | "i8"
                | "i16"
                | "i32"
                | "i64"
                | "i128"
                | "isize"
                | "str"
                | "u8"
                | "u16"
                | "u32"
                | "u64"
                | "u128"
                | "usize"
        ),
        CodeLanguageFamily::Java => matches!(
            word,
            "int" | "long" | "short" | "void" | "String" | "Object" | "Integer" | "List" | "Map"
        ),
        CodeLanguageFamily::JavaScript => matches!(
            word,
            "number" | "object" | "string" | "Promise" | "Array" | "Object"
        ),
        CodeLanguageFamily::Go => matches!(
            word,
            "error" | "float32" | "float64" | "int64" | "rune" | "string" | "uint" | "uint64"
        ),
        CodeLanguageFamily::Shell | CodeLanguageFamily::Sql | CodeLanguageFamily::Generic => false,
    }
}

fn is_type_declaration(word: &str) -> bool {
    matches!(
        word,
        "class"
            | "struct"
            | "enum"
            | "interface"
            | "trait"
            | "record"
            | "union"
            | "typedef"
            | "type"
    )
}

fn is_function_declaration(word: &str) -> bool {
    matches!(word, "def" | "fn" | "func" | "function" | "fun")
}

fn is_reference_keyword(word: &str) -> bool {
    matches!(
        word,
        "include" | "import" | "from" | "use" | "using" | "require" | "package" | "namespace"
    )
}

fn highlight_code_line(line: &str, language: &str) -> Vec<(Range<usize>, TuiTone)> {
    let family = code_language_family(language);
    let leading = line.len().saturating_sub(line.trim_start().len());
    let trimmed = &line[leading..];
    if trimmed.starts_with('#')
        && matches!(family, CodeLanguageFamily::C | CodeLanguageFamily::Rust)
    {
        let directive_end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
        let mut ranges = vec![(leading..leading + directive_end, TuiTone::Reference)];
        if let Some(open) = trimmed[directive_end..].find(['<', '"'])
            && let Some(close) = trimmed[directive_end + open + 1..].find(['>', '"'])
        {
            let start = leading + directive_end + open + 1;
            let end = leading + directive_end + open + 1 + close;
            ranges.push((start..end, TuiTone::Reference));
        }
        return ranges;
    }

    let hash_comments = matches!(
        family,
        CodeLanguageFamily::Python | CodeLanguageFamily::Shell
    ) || matches!(
        language,
        "perl" | "yaml" | "yml" | "toml" | "ini" | "dockerfile" | "makefile" | "nix"
    );
    let dash_comments =
        matches!(family, CodeLanguageFamily::Sql) || matches!(language, "lua" | "haskell" | "hs");
    let slash_comments = !hash_comments && !dash_comments;
    let bytes = line.as_bytes();
    let mut ranges = Vec::new();
    let mut index = 0;
    let mut expect_type = false;
    let mut expect_function = false;
    let mut reference_active = false;

    while index < bytes.len() {
        let character = line[index..].chars().next().expect("valid character");
        if character.is_whitespace() {
            index += character.len_utf8();
            continue;
        }

        let comment_end = if slash_comments && line[index..].starts_with("//") {
            Some(bytes.len())
        } else if slash_comments && line[index..].starts_with("/*") {
            Some(
                line[index + 2..]
                    .find("*/")
                    .map_or(bytes.len(), |end| index + 2 + end + 2),
            )
        } else if (hash_comments && character == '#')
            || (dash_comments && line[index..].starts_with("--"))
        {
            Some(bytes.len())
        } else {
            None
        };
        if let Some(end) = comment_end {
            ranges.push((index..end, TuiTone::Comment));
            break;
        }

        if matches!(character, '"' | '\'' | '`') {
            let quote_len =
                if line[index..].starts_with("\"\"\"") || line[index..].starts_with("'''") {
                    3
                } else {
                    character.len_utf8()
                };
            let mut end = index + quote_len;
            let mut escaped = false;
            while end < bytes.len() {
                if !escaped && line[end..].starts_with(&line[index..index + quote_len]) {
                    end += quote_len;
                    break;
                }
                let next = line[end..].chars().next().expect("valid character");
                escaped = !escaped && next == '\\';
                if next != '\\' {
                    escaped = false;
                }
                end += next.len_utf8();
            }
            ranges.push((index..end, TuiTone::String));
            index = end;
            continue;
        }

        if character.is_ascii_digit()
            || (character == '.'
                && line[index + 1..]
                    .chars()
                    .next()
                    .is_some_and(|next| next.is_ascii_digit()))
        {
            let mut end = index + character.len_utf8();
            while end < bytes.len() {
                let next = line[end..].chars().next().expect("valid character");
                if !(next.is_ascii_alphanumeric() || matches!(next, '_' | '.')) {
                    break;
                }
                end += next.len_utf8();
            }
            ranges.push((index..end, TuiTone::Number));
            index = end;
            continue;
        }

        if character.is_alphabetic() || character == '_' {
            let start = index;
            index += character.len_utf8();
            while index < bytes.len() {
                let next = line[index..].chars().next().expect("valid character");
                if !(next.is_alphanumeric() || next == '_') {
                    break;
                }
                index += next.len_utf8();
            }
            let word = &line[start..index];
            let is_call = line[index..].trim_start().starts_with('(')
                || (line[index..].trim_start().starts_with('!')
                    && line[index..]
                        .trim_start()
                        .trim_start_matches('!')
                        .trim_start()
                        .starts_with('('));
            let tone = if reference_active {
                TuiTone::Reference
            } else if is_builtin_type(word, family) {
                TuiTone::Type
            } else if code_keywords(family).contains(&word) {
                if is_reference_keyword(word) {
                    reference_active = true;
                    TuiTone::Reference
                } else {
                    expect_type |= is_type_declaration(word);
                    expect_function |= is_function_declaration(word);
                    TuiTone::Keyword
                }
            } else if expect_type {
                TuiTone::Type
            } else if expect_function {
                TuiTone::Function
            } else if is_call || word.chars().next().is_some_and(char::is_uppercase) {
                if is_call {
                    TuiTone::Function
                } else {
                    TuiTone::Type
                }
            } else {
                TuiTone::Code
            };
            if tone != TuiTone::Code {
                ranges.push((start..index, tone));
            }
            if !code_keywords(family).contains(&word) {
                expect_type = false;
                expect_function = false;
            }
            continue;
        }

        if character == ';' {
            reference_active = false;
        }
        index += character.len_utf8();
    }
    ranges
}

fn code_line_background(line: &str, diff_block: bool) -> Color {
    if !diff_block {
        return CODE_BG;
    }
    let line = line.trim_start();
    if line.starts_with("+++") || line.starts_with("---") || line.starts_with("@@") {
        CODE_BG
    } else if line.starts_with('+') {
        CODE_ADDED_BG
    } else if line.starts_with('-') {
        CODE_REMOVED_BG
    } else {
        CODE_BG
    }
}

const CODE_LANGUAGE_LABELS: &[&str] = &[
    "abap",
    "ada",
    "apex",
    "applescript",
    "asm",
    "assembly",
    "astro",
    "awk",
    "bash",
    "bat",
    "batch",
    "bazel",
    "bicep",
    "c",
    "c#",
    "cpp",
    "c++",
    "csharp",
    "cs",
    "clojure",
    "clj",
    "cmake",
    "cmd",
    "cobol",
    "crystal",
    "css",
    "csv",
    "cypher",
    "dart",
    "delphi",
    "diff",
    "dockerfile",
    "elixir",
    "elm",
    "erl",
    "erlang",
    "ex",
    "fish",
    "fortran",
    "f90",
    "fs",
    "fsharp",
    "f#",
    "fennel",
    "go",
    "golang",
    "gleam",
    "glsl",
    "gradle",
    "graphql",
    "gql",
    "groovy",
    "haskell",
    "handlebars",
    "haxe",
    "hbs",
    "hcl",
    "hlsl",
    "hs",
    "htm",
    "html",
    "ini",
    "java",
    "javascript",
    "js",
    "json",
    "jsonc",
    "json5",
    "jsonl",
    "jsx",
    "julia",
    "kotlin",
    "kql",
    "kts",
    "latex",
    "lean",
    "less",
    "liquid",
    "lisp",
    "lua",
    "makefile",
    "markdown",
    "matlab",
    "md",
    "meson",
    "mermaid",
    "moonscript",
    "mustache",
    "nim",
    "nix",
    "node",
    "objc",
    "objective-c",
    "ocaml",
    "octave",
    "odin",
    "pascal",
    "patch",
    "perl",
    "php",
    "plsql",
    "powershell",
    "proto",
    "protobuf",
    "ps1",
    "purescript",
    "pwsh",
    "py",
    "python",
    "q",
    "r",
    "racket",
    "raku",
    "rb",
    "regex",
    "reason",
    "reasonml",
    "rescript",
    "rexx",
    "rs",
    "ruby",
    "rust",
    "sas",
    "sass",
    "scala",
    "scheme",
    "scss",
    "sh",
    "shell",
    "sol",
    "solidity",
    "sparql",
    "sql",
    "starlark",
    "stata",
    "svg",
    "svelte",
    "swift",
    "systemverilog",
    "tcl",
    "terraform",
    "tex",
    "thrift",
    "toml",
    "ts",
    "tsql",
    "tsx",
    "typescript",
    "v",
    "vala",
    "vb",
    "vbnet",
    "verilog",
    "vhdl",
    "vue",
    "wgsl",
    "xml",
    "xsl",
    "xslt",
    "yaml",
    "yml",
    "zig",
    "zsh",
];

fn unfenced_code_start(lines: &[&str], index: usize) -> Option<(&'static str, usize)> {
    let line = lines.get(index)?.trim();
    if let Some(language) = CODE_LANGUAGE_LABELS
        .iter()
        .find(|language| line.eq_ignore_ascii_case(language))
        .copied()
    {
        let code_start = lines
            .iter()
            .enumerate()
            .skip(index + 1)
            .find_map(|(index, line)| (!line.trim().is_empty()).then_some(index))?;
        if looks_like_code_line(lines[code_start]) {
            return Some((language, code_start));
        }
    }

    inferred_code_language(line).map(|language| (language, index))
}

fn inferred_code_language(line: &str) -> Option<&'static str> {
    let line = line.trim_start();
    if line.starts_with("#include")
        || line.starts_with("#define")
        || line.starts_with("#pragma")
        || line.starts_with("#if")
    {
        return Some("c");
    }
    if line.starts_with("def ") || line.starts_with("from ") || line.starts_with("import ") {
        return Some("python");
    }
    if line.starts_with("fn ")
        || line.starts_with("pub fn ")
        || line.starts_with("impl ")
        || line.starts_with("use ")
    {
        return Some("rust");
    }
    if line.starts_with("function ")
        || line.starts_with("const ")
        || line.starts_with("let ")
        || line.starts_with("var ")
    {
        return Some("javascript");
    }
    if line.starts_with("package ") {
        return Some("go");
    }
    if line.starts_with("<!DOCTYPE")
        || line.starts_with("<html")
        || line.starts_with("<div")
        || line.starts_with("<?xml")
    {
        return Some("html");
    }
    if line.starts_with("SELECT ") || line.starts_with("INSERT ") || line.starts_with("CREATE ") {
        return Some("sql");
    }
    None
}

fn looks_like_code_line(line: &str) -> bool {
    let line = line.trim();
    if line.is_empty() {
        return false;
    }

    const CODE_STARTS: &[&str] = &[
        "#include",
        "#define",
        "#pragma",
        "#if",
        "#endif",
        "import ",
        "from ",
        "def ",
        "class ",
        "fn ",
        "pub fn ",
        "fun ",
        "impl ",
        "trait ",
        "struct ",
        "enum ",
        "interface ",
        "namespace ",
        "module ",
        "record ",
        "object ",
        "type ",
        "typedef ",
        "template ",
        "union ",
        "using ",
        "use ",
        "let ",
        "const ",
        "var ",
        "function ",
        "static ",
        "return",
        "if ",
        "else",
        "for ",
        "while ",
        "switch ",
        "case ",
        "break;",
        "continue;",
        "printf(",
        "fprintf(",
        "print(",
        "puts ",
        "println!",
        "println(",
        "fmt.",
        "System.out",
        "Console.",
        "console.",
        "console.log(",
        "package ",
        "func ",
        "defmodule ",
        "defp ",
        "echo ",
        "export ",
        "set ",
        "with ",
        "match ",
        "begin",
        "data ",
        "newtype ",
        "macro ",
        "public ",
        "private ",
        "protected ",
        "FROM ",
        "RUN ",
        "COPY ",
        "WORKDIR ",
        "ENTRYPOINT ",
        "CMD ",
        "ENV ",
        "resource ",
        "provider ",
        "variable ",
        "syntax =",
        "message ",
        "service ",
        "query ",
        "mutation ",
        "subscription ",
        "SELECT ",
        "INSERT ",
        "UPDATE ",
        "DELETE ",
        "CREATE ",
        "select ",
        "insert ",
        "update ",
        "delete ",
        "create ",
        "<?php",
        "#!/",
        "\\documentclass",
        "\\begin",
        "\\end",
        "//",
        "/*",
        "*/",
        "* ",
        "@",
        "#[",
        "<!DOCTYPE",
        "<?xml",
        "</",
        "<html",
        "<div",
        "{",
        "};",
        "]);",
    ];
    if CODE_STARTS.iter().any(|prefix| line.starts_with(prefix)) {
        return true;
    }

    if line.starts_with('}') || line == "]" {
        return true;
    }
    if line.starts_with('[') && line.ends_with(']') {
        return true;
    }
    if line.starts_with('<') && line.ends_with('>') {
        return true;
    }
    if line.ends_with(';')
        || line.ends_with('{')
        || line.ends_with('}')
        || line.ends_with(',')
        || line.ends_with('\\')
    {
        return true;
    }

    let Some((callee, arguments)) = line.split_once('(') else {
        return false;
    };
    !callee.is_empty()
        && arguments.contains(')')
        && callee
            .chars()
            .all(|character| character.is_alphanumeric() || "_$.:".contains(character))
}

/// The mark is drawn with two colours: a grey field of `╋` cells and the strokes
/// in accent blue.
fn welcome_mark_lines() -> Vec<Line<'static>> {
    WELCOME_LOGO
        .iter()
        .map(|row| {
            Line::from(
                row.chars()
                    .enumerate()
                    .map(|(column, c)| {
                        // The mark opens with a four-column "1"; the rest is the
                        // word. Spaces stay empty so the terminal shows through.
                        let style = if c == ' ' {
                            Style::default()
                        } else if column < 4 {
                            Style::default().fg(MARK_GREEN)
                        } else {
                            Style::default().fg(LINK)
                        };
                        Span::styled(c.to_string(), style)
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

pub fn welcome_logo_green_range(_index: usize) -> Range<usize> {
    // The plain-text banner printed after a session uses one colour; the coloured
    // mark only exists on the welcome screen, where it has no terminal colours.
    0..0
}

fn wrap_first_then_rest(text: &str, first_width: usize, rest_width: usize) -> Vec<String> {
    let (first, rest) = split_display_width(text, first_width.max(1));
    let mut lines = vec![first.to_string()];
    if !rest.is_empty() {
        lines.extend(wrap_display(rest, rest_width.max(1)));
    }
    lines
}

fn split_display_width(text: &str, limit: usize) -> (&str, &str) {
    let mut width = 0;
    for (index, grapheme) in text.grapheme_indices(true) {
        let next = width + grapheme.width();
        if next > limit {
            return (&text[..index], &text[index..]);
        }
        width = next;
    }
    (text, "")
}

fn visible_input_end(text: &str, start: usize, width: u16) -> usize {
    let limit = usize::from(width);
    let mut used = 0;
    for (index, grapheme) in text[start..].grapheme_indices(true) {
        let next = used + grapheme.width();
        if next > limit {
            return start + index;
        }
        used = next;
    }
    text.len()
}

fn truncate_to_width(text: &str, limit: usize) -> String {
    if text.width() <= limit {
        return text.to_string();
    }
    if limit == 0 {
        return String::new();
    }
    let mut output = String::new();
    let mut width = 0;
    for character in text.chars() {
        let character_width = character.width().unwrap_or(0);
        if width + character_width >= limit {
            break;
        }
        output.push(character);
        width += character_width;
    }
    output.push('…');
    output
}

fn menu_block(title: &str, hint: &str, background: Color) -> Block<'static> {
    let mut block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title_top(Line::from(Span::styled(
            format!(" {} ", title.trim()),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )))
        .border_style(Style::default().fg(DRAWER_BORDER))
        .style(Style::default().bg(background));
    if !hint.is_empty() {
        block = block.title_bottom(
            Line::from(Span::styled(
                format!(" {hint} "),
                Style::default().fg(MENU_HINT),
            ))
            .right_aligned(),
        );
    }
    block
}

fn fit_row(text: &str, columns: usize) -> String {
    let text = truncate_to_width(text, columns);
    let padding = columns.saturating_sub(text.width());
    format!("{text}{}", " ".repeat(padding))
}

fn wrap_display(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut output = Vec::new();
    for logical_line in text.split('\n') {
        if logical_line.is_empty() {
            output.push(String::new());
            continue;
        }
        let mut line = String::new();
        let mut line_width = 0;
        for character in logical_line.chars() {
            let character_width = character.width().unwrap_or(0);
            if line_width > 0 && line_width + character_width > width {
                output.push(line);
                line = String::new();
                line_width = 0;
            }
            line.push(character);
            line_width += character_width;
        }
        output.push(line);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn code_tone_at_word(line: &str, language: &str, word: &str) -> TuiTone {
        let ranges = highlight_code_line(line, language);
        let start = line.find(word).expect("word should exist");
        ranges
            .iter()
            .find(|(range, _)| range.contains(&start))
            .map_or(TuiTone::Code, |(_, tone)| *tone)
    }

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
        assert!(running.contains("运行中"));
        assert!(running.contains("决策分析"));
        assert!(running.contains("中等风险"));
        state.finish(TuiStatus::Completed, "Conclusion: A");
        let completed = state.render_screen(100, 30);
        assert!(completed.contains("已完成"));
    }

    #[test]
    fn terminal_output_is_control_safe() {
        let mut state = TuiState::new("mock/offline-v1", 6, 120);
        state.begin(
            "safe\u{1b}[31m text\u{202e}",
            CognitiveMode::DirectAnswer,
            RiskLevel::Low,
        );
        state.info("answer\u{0000}\u{202e}tail\tend");
        let screen = state.render_screen(80, 20);
        assert!(!screen.contains("\u{1b}[31m"));
        assert!(!screen.contains('\u{202e}'));
        assert!(screen.contains("safe [31m text"));
        assert!(screen.contains("answer�"));
        assert!(screen.contains("tail    end"));
    }

    #[test]
    fn editing_is_grapheme_aware_and_replaces_selection() {
        let mut state = TuiState::new("mock", 6, 120);
        state.insert_char(' ');
        assert_eq!(state.input(), " ");
        state.clear_input();
        state.set_input("  two spaces  ");
        assert_eq!(state.input(), "  two spaces  ");
        state.clear_input();
        state.insert_text("Ae\u{301}B");
        state.move_left();
        state.insert_char('X');
        assert_eq!(state.input(), "Ae\u{301}XB");
        state.move_left();
        state.backspace();
        assert_eq!(state.input(), "AXB");

        state.set_input("hello");
        state.selection_anchor = Some(1);
        state.input_cursor = 4;
        state.insert_text("i");
        assert_eq!(state.input(), "hio");
    }

    #[test]
    fn columns_map_to_input_indices_and_command_drawer_filters() {
        let text = "ab界cd";
        assert_eq!(input_index_for_column(text, 0, text.len(), 10, 20, 9), 0);
        assert_eq!(input_index_for_column(text, 0, text.len(), 10, 20, 10), 0);
        assert_eq!(input_index_for_column(text, 0, text.len(), 10, 20, 12), 2);
        assert_eq!(
            input_index_for_column(text, 0, text.len(), 10, 20, 100),
            text.len()
        );

        let mut state = TuiState::new("mock", 6, 120);
        state.set_input("/he");
        assert_eq!(
            state
                .filtered_commands()
                .iter()
                .map(|command| command.name)
                .collect::<Vec<_>>(),
            vec!["/help"]
        );
        assert!(state.slash_drawer_open());
        assert!(state.complete_selected_command());
        assert_eq!(state.input(), "/help");
    }

    #[test]
    fn incomplete_drawer_input_completes_before_it_submits() {
        let mut state = TuiState::new("mock", 6, 120);
        state.set_input("/he");
        assert_eq!(state.take_submission(), "");
        assert_eq!(state.input(), "/help");
        assert_eq!(state.take_submission(), "/help");
    }

    #[test]
    fn escape_dismisses_the_drawer_without_losing_input() {
        let mut state = TuiState::new("mock", 6, 120);
        state.set_input("/he");
        assert!(state.slash_drawer_open());
        state.dismiss_slash_drawer();
        assert!(!state.slash_drawer_open());
        assert_eq!(state.input(), "/he");
        state.insert_char('l');
        assert!(state.slash_drawer_open());
    }

    #[test]
    fn disabled_stage_commands_are_shown_with_a_reason() {
        let mut state = TuiState::new("mock", 6, 120);
        state.set_host_binding(HostBinding::workshop_context(
            "master",
            "main-coordinator",
            "1agent",
            "term-1790000000000-1234-1",
        ));
        state.set_input("/app");
        let entries = state.filtered_command_entries();
        let approve = entries
            .iter()
            .find(|entry| entry.spec.name == "/approve")
            .expect("approve should be listed");
        assert_eq!(approve.unavailable, Some("Approval"));
        assert!(
            state
                .filtered_commands()
                .iter()
                .all(|command| command.name != "/approve")
        );
    }

    #[test]
    fn argument_commands_complete_with_a_trailing_space() {
        let mut state = TuiState::new("mock", 6, 120);
        state.set_input("/fra");
        assert!(state.complete_selected_command());
        assert_eq!(state.input(), "/frame ");
    }

    #[test]
    fn recent_commands_are_deduplicated_and_lead_the_drawer() {
        let mut state = TuiState::new("mock", 6, 120);
        state.record_command_use(TuiCommandId::Status);
        state.record_command_use(TuiCommandId::Mode);
        state.record_command_use(TuiCommandId::Status);
        state.set_input("/");
        let rows = state.filtered_drawer_rows();
        assert_eq!(rows[0].heading, Some("最近"));
        assert_eq!(rows[0].entry.spec.id, TuiCommandId::Status);
        assert_eq!(rows[1].entry.spec.id, TuiCommandId::Mode);
        assert_eq!(
            rows.iter()
                .filter(|row| row.entry.spec.id == TuiCommandId::Status)
                .count(),
            1
        );
    }

    #[test]
    fn quit_requires_an_explicit_confirmation() {
        let mut state = TuiState::new("mock", 6, 120);
        state.request_quit();
        assert!(state.quit_requested());
        state.cancel_quit();
        assert!(!state.quit_requested());
        state.request_quit();
        assert!(state.take_quit_request());
        assert!(!state.quit_requested());
    }

    #[test]
    fn typing_cancels_an_armed_confirmation() {
        let mut state = TuiState::new("mock", 6, 120);
        state.arm_confirmation("/clear");
        assert_eq!(state.pending_confirmation(), Some("/clear"));
        state.insert_char('x');
        assert_eq!(state.pending_confirmation(), None);
        state.arm_confirmation("/dispatch");
        assert_eq!(state.take_confirmation().as_deref(), Some("/dispatch"));
        assert_eq!(state.pending_confirmation(), None);
    }

    #[test]
    fn input_history_recalls_previous_submissions() {
        let mut state = TuiState::new("mock", 6, 120);
        state.record_input("first task");
        state.record_input("second task");
        assert!(state.history_previous());
        assert_eq!(state.input(), "second task");
        assert!(state.history_previous());
        assert_eq!(state.input(), "first task");
        assert!(state.history_next());
        assert_eq!(state.input(), "second task");
        assert!(state.history_next());
        assert_eq!(state.input(), "");
    }

    #[test]
    fn workshop_master_drawer_is_scoped_and_labelled() {
        let mut state = TuiState::new("mock", 6, 120);
        state.set_host_binding(HostBinding::workshop_context(
            "master",
            "main-coordinator",
            "1agent",
            "term-1790000000000-1234-1",
        ));
        state.set_input("/");
        assert_eq!(state.command_drawer_title(), " 总控命令 ");
        assert!(
            state
                .filtered_commands()
                .iter()
                .any(|command| command.name == "/mission")
        );

        let mut standalone = TuiState::new("mock", 6, 120);
        standalone.set_input("/");
        assert_eq!(standalone.command_drawer_title(), " 命令 ");
        assert!(
            standalone
                .filtered_commands()
                .iter()
                .all(|command| command.scope == crate::tui_commands::TuiCommandScope::Core)
        );
    }

    #[test]
    fn ratatui_composer_centers_input_and_supports_mouse_selection() {
        let backend = TestBackend::new(60, 18);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        state.set_input("hello");
        terminal.draw(|frame| state.draw(frame)).unwrap();

        let geometry = state.input_geometry.unwrap();
        let prompt_cell = terminal
            .backend()
            .buffer()
            .cell((geometry.text_x - 2, geometry.row))
            .unwrap();
        assert_eq!(prompt_cell.symbol(), "›");
        assert_eq!(prompt_cell.fg, UI_ACCENT);
        assert!(prompt_cell.modifier.contains(Modifier::BOLD));
        assert_eq!(
            terminal
                .backend()
                .buffer()
                .cell((1, geometry.row - 1))
                .unwrap()
                .bg,
            INPUT_BG
        );
        assert_eq!(
            terminal
                .backend()
                .buffer()
                .cell((1, geometry.row + 1))
                .unwrap()
                .bg,
            INPUT_BG
        );
        state.select_at(geometry.text_x, geometry.row, false);
        state.select_at(geometry.text_x + 3, geometry.row, true);
        assert_eq!(state.selected_input_text().as_deref(), Some("hel"));
    }

    #[test]
    fn user_message_and_answer_are_separated_by_an_empty_gap_row() {
        let backend = TestBackend::new(70, 22);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        state.begin("用户问题", CognitiveMode::DirectAnswer, RiskLevel::Low);
        state.assistant_body("助手回答");
        terminal.draw(|frame| state.draw(frame)).unwrap();

        let answer_row = state
            .transcript_lines
            .iter()
            .position(|line| line.text.contains("助手回答"))
            .unwrap();
        assert!(answer_row >= 2);
        assert_eq!(
            state.transcript_lines[answer_row - 1].kind,
            TranscriptKind::Assistant
        );
        assert!(state.transcript_lines[answer_row - 1].text.is_empty());
        assert_eq!(
            state.transcript_lines[answer_row - 2].kind,
            TranscriptKind::User
        );
        assert!(state.transcript_lines[answer_row - 2].text.is_empty());

        let area = state.transcript_area.unwrap();
        let gap_y = area.y + u16::try_from(answer_row - 1 - state.transcript_scroll_top).unwrap();
        assert_ne!(
            terminal
                .backend()
                .buffer()
                .cell((area.x, gap_y))
                .unwrap()
                .bg,
            Color::Indexed(238)
        );
    }

    #[test]
    fn transcript_mouse_selection_copies_rendered_text() {
        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        state.info("alpha\nbeta");
        terminal.draw(|frame| state.draw(frame)).unwrap();

        let area = state.transcript_area.unwrap();
        let first_line = &state.transcript_lines[0].text;
        let alpha_start = first_line.find("alpha").unwrap();
        let alpha_column = first_line[..alpha_start].width() as u16;
        state.select_at(area.x + alpha_column, area.y, false);
        state.select_at(area.x + alpha_column + 5, area.y, true);
        assert_eq!(state.selected_text().as_deref(), Some("alpha"));
    }

    #[test]
    fn mouse_drag_at_viewport_edges_autoscrolls_and_extends_selection() {
        let backend = TestBackend::new(70, 18);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        for index in 0..30 {
            state.info(format!("history line {index}"));
        }
        terminal.draw(|frame| state.draw(frame)).unwrap();
        let area = state.transcript_area.unwrap();

        state.begin_selection_drag(area.x, area.bottom().saturating_sub(2));
        let upward_anchor = state.transcript_anchor.unwrap();
        state.update_selection_drag(area.x + 2, area.y);
        assert!(state.selection_autoscroll_active());
        assert!(state.scroll_from_bottom > 0);
        assert!(
            state
                .transcript_focus
                .is_some_and(|focus| focus.line < upward_anchor.line)
        );
        let first_focus = state.transcript_focus.unwrap().line;
        assert!(state.autoscroll_selection_once());
        assert!(state.transcript_focus.unwrap().line <= first_focus);
        state.end_selection_drag();
        assert!(!state.selection_autoscroll_active());

        state.scroll_to_bottom();
        state.scroll_up(10);
        terminal.draw(|frame| state.draw(frame)).unwrap();
        let area = state.transcript_area.unwrap();
        state.begin_selection_drag(area.x, area.y + 1);
        let downward_anchor = state.transcript_anchor.unwrap();
        state.update_selection_drag(area.x + 2, area.bottom().saturating_sub(1));
        assert!(state.selection_autoscroll_active());
        assert!(
            state
                .transcript_focus
                .is_some_and(|focus| focus.line > downward_anchor.line)
        );
        assert!(state.scroll_from_bottom < 10);
        state.end_selection_drag();
    }

    #[test]
    fn sticky_prompt_header_tracks_visible_answer_and_yields_to_prompt() {
        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        state.begin("First topic", CognitiveMode::DirectAnswer, RiskLevel::Low);
        for index in 0..20 {
            state.info(format!("answer line {index}"));
        }
        terminal.draw(|frame| state.draw(frame)).unwrap();
        let body = state.transcript_area.unwrap();
        let header_y = body.y.saturating_sub(1);
        let header = terminal
            .backend()
            .buffer()
            .cell((1, header_y))
            .unwrap()
            .symbol();
        assert_eq!(header, "›");

        state.begin("Second topic", CognitiveMode::DirectAnswer, RiskLevel::Low);
        state.transcript_lines = state.build_transcript_lines(60);
        let second_prompt = state
            .transcript_lines
            .iter()
            .position(|line| line.prompt_text.as_deref() == Some("Second topic"))
            .expect("second prompt");
        assert!(state.prompt_header_for_line(second_prompt, 60).is_none());
    }

    #[test]
    fn transcript_semantic_tones_match_the_palette() {
        assert_eq!(tone_style(TuiTone::Accent).fg, Some(Color::Indexed(75)));
        assert_eq!(tone_style(TuiTone::Success).fg, Some(Color::LightGreen));
        assert_eq!(tone_style(TuiTone::Warning).fg, Some(Color::Yellow));
        assert_eq!(tone_style(TuiTone::Danger).fg, Some(Color::LightRed));
        assert_eq!(tone_style(TuiTone::Evidence).fg, Some(Color::Cyan));
        assert_eq!(
            tone_style(TuiTone::Supplementary).fg,
            Some(Color::Indexed(245))
        );
        assert!(
            tone_style(TuiTone::Supplementary)
                .add_modifier
                .contains(Modifier::DIM)
        );
        assert_eq!(tone_style(TuiTone::Muted).fg, Some(MUTED_TEXT));
        assert!(
            !tone_style(TuiTone::Muted)
                .add_modifier
                .contains(Modifier::DIM)
        );
        assert_eq!(tone_style(TuiTone::User).bg, Some(Color::Indexed(238)));
    }

    #[test]
    fn running_activity_uses_working_label_and_run_metrics() {
        let backend = TestBackend::new(120, 18);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        state.begin(
            "正在执行的任务",
            CognitiveMode::DirectAnswer,
            RiskLevel::Low,
        );
        state.set_run_metrics(TuiRunMetrics {
            input_tokens: 1_200,
            output_tokens: 340,
            cache_hit_tokens: 40,
            cache_miss_tokens: 60,
            elapsed_ms: 0,
            usage_seen: true,
        });
        terminal.draw(|frame| state.draw(frame)).unwrap();

        let activity_y = 18 - 3 - 1 - 1 - 1;
        let row = (0..120)
            .map(|x| {
                terminal
                    .backend()
                    .buffer()
                    .cell((x, activity_y))
                    .unwrap()
                    .symbol()
            })
            .collect::<String>();
        assert!(row.contains("Working..."), "row: {row}");
        assert!(row.contains("↑1.2k"), "row: {row}");
        assert!(row.contains("↓340"), "row: {row}");
        assert!(row.contains("cache 40%"), "row: {row}");
        let spinner = terminal.backend().buffer().cell((2, activity_y)).unwrap();
        assert_eq!(spinner.fg, Color::Indexed(75));
        assert!(spinner.modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn mode_menu_switches_the_effective_mode_and_returns_to_auto() {
        let mut state = TuiState::new("mock", 6, 120);
        assert_eq!(
            state.effective_mode(CognitiveMode::EvidenceResearch),
            CognitiveMode::EvidenceResearch
        );

        state.toggle_mode_menu();
        assert!(state.mode_menu_open());
        state.move_mode_menu_selection(3);
        assert_eq!(state.mode_menu_selected().unwrap().label, "决策分析");
        state.apply_mode_menu();
        assert!(!state.mode_menu_open());
        assert_eq!(state.displayed_mode(), CognitiveMode::DecisionAnalysis);
        assert_eq!(
            state.effective_mode(CognitiveMode::DirectAnswer),
            CognitiveMode::DecisionAnalysis
        );

        // Reopening starts on the applied row, not on the first row.
        state.toggle_mode_menu();
        assert_eq!(state.mode_menu_selected().unwrap().label, "决策分析");
        state.move_mode_menu_selection(-3);
        state.apply_mode_menu();
        assert_eq!(state.mode_override(), None);
        assert_eq!(
            state.effective_mode(CognitiveMode::EvidenceResearch),
            CognitiveMode::EvidenceResearch
        );
    }

    #[test]
    fn mode_menu_is_rounded_highlights_the_selection_and_marks_the_active_mode() {
        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        state.toggle_mode_menu();
        state.move_mode_menu_selection(3);
        terminal.draw(|frame| state.draw(frame)).unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
        let tight: String = content.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(content.contains('╭'), "rounded border");
        assert!(tight.contains("切换模式"));
        assert!(tight.contains("决策分析"));
        assert!(tight.contains("↑↓选择"));
        assert!(
            buffer
                .content
                .iter()
                .any(|cell| cell.symbol() != " " && cell.bg == MENU_SELECTED_BG),
            "the highlighted row has its own background"
        );

        state.close_mode_menu();
        terminal.draw(|frame| state.draw(frame)).unwrap();
        let content: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(!content.contains("切换模式"));
    }

    #[test]
    fn status_bar_shows_the_model_the_mode_and_context_usage() {
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("openai-compatible/deepseek-flash", 6, 120);
        state.set_context_budget(1000);
        state.set_context_chars(250);
        state.begin("任务", CognitiveMode::EvidenceResearch, RiskLevel::Low);
        terminal.draw(|frame| state.draw(frame)).unwrap();

        let row: String = (0..80)
            .map(|x| {
                terminal
                    .backend()
                    .buffer()
                    .cell((x, 19))
                    .map(|cell| cell.symbol())
                    .unwrap_or("")
            })
            .collect();
        // Wide characters leave a filler cell, so compare without whitespace.
        let tight: String = row.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(tight.contains("deepseek-flash"), "model: {row}");
        assert!(tight.contains("证据研究"), "mode: {row}");
        assert!(tight.contains("Ctrl+T模式"), "hint: {row}");
        assert!(tight.contains("上下文25%"), "usage: {row}");
    }

    #[test]
    fn interactive_surface_has_no_global_status_bar_and_uses_green_copy_notice() {
        let backend = TestBackend::new(60, 18);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        state.info("Welcome");
        terminal.draw(|frame| state.draw(frame)).unwrap();
        let content = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(!content.contains("1-Agent | status="));

        state.set_notice("已复制选择内容");
        terminal.draw(|frame| state.draw(frame)).unwrap();
        let notice_y = 18 - 3 - 1 - 1 - 1;
        let notice = terminal.backend().buffer().cell((1, notice_y)).unwrap();
        assert_eq!(notice.fg, Color::LightGreen);
        assert!(notice.modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn follow_control_reports_new_activity_and_returns_to_bottom() {
        let backend = TestBackend::new(70, 18);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        for index in 0..24 {
            state.info(format!("history {index}"));
        }
        state.scroll_up(3);
        terminal.draw(|frame| state.draw(frame)).unwrap();
        assert!(state.follow_control_area.is_some());

        state.info("new live output");
        terminal.draw(|frame| state.draw(frame)).unwrap();
        let area = state.follow_control_area.unwrap();
        let row = (area.y..area.bottom())
            .flat_map(|y| (area.x..area.right()).map(move |x| (x, y)))
            .filter_map(|(x, y)| terminal.backend().buffer().cell((x, y)))
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(row.contains('新'));
        assert!(row.contains('返'));

        assert!(state.handle_follow_control_hover(area.x + 1, area.y));
        terminal.draw(|frame| state.draw(frame)).unwrap();
        let hovered = terminal.backend().buffer().cell((area.x, area.y)).unwrap();
        assert!(hovered.modifier.contains(Modifier::REVERSED));

        assert!(state.handle_follow_control(area.x, area.y));
        assert_eq!(state.scroll_from_bottom, 0);
        assert!(!state.unseen_activity);
    }

    #[test]
    fn welcome_surface_uses_logo_session_and_no_global_status() {
        // Tall enough for the raster mark; the fallback is covered separately.
        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        state.set_session_info("ses_welcome_test");
        terminal.draw(|frame| state.draw(frame)).unwrap();
        let content = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        let colours = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.fg)
            .collect::<Vec<_>>();
        assert!(
            colours.contains(&CAPTION_GREY),
            "the mark and the captions are grey"
        );
        assert!(
            colours.contains(&LINK),
            "the domain is the only highlighted element"
        );
        assert!(
            !colours.contains(&LINK_HOVER),
            "nothing is hovered in a fresh render"
        );
        // The caption ships with the product; pin the exact casing. Wide
        // characters occupy two cells, so the buffer holds a filler between them.
        let tight = content
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>();
        assert!(tight.contains("超级个体俱乐部"));
        assert!(tight.contains("SuperIndividualClub"));
        assert!(tight.contains("SKYNET.CLUB"));
        // The session block and the key hints were removed on request; assert
        // their absence so they cannot creep back.
        assert!(!content.contains("ses_welcome_test"));
        assert!(!content.contains("输入任务开始"));
        assert!(!content.contains("Ctrl+Q"));
        assert!(!content.contains("1-Agent | status="));
    }

    #[test]
    fn hovering_the_domain_brightens_only_it() {
        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        terminal.draw(|frame| state.draw(frame)).unwrap();

        let buffer = terminal.backend().buffer().clone();
        let mut target = None;
        for y in 0..buffer.area.height {
            let row: String = (0..buffer.area.width)
                .map(|x| buffer.cell((x, y)).map(|c| c.symbol()).unwrap_or(""))
                .collect();
            if let Some(offset) = row.find("SKYNET") {
                target = Some((y, offset as u16));
                break;
            }
        }
        let (row, offset) = target.expect("domain is rendered");

        state.set_pointer(offset + 2, row);
        terminal.draw(|frame| state.draw(frame)).unwrap();
        assert!(
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .any(|cell| cell.fg == LINK_HOVER),
            "hovering the domain brightens it"
        );
    }

    #[test]
    fn a_short_terminal_falls_back_to_the_wordmark_instead_of_cropping_the_mark() {
        let backend = TestBackend::new(80, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        terminal.draw(|frame| state.draw(frame)).unwrap();
        let content = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(content.contains("1-Agent"));
        assert!(!content.contains('╔'), "the mark is not drawn at this size");
    }

    #[test]
    fn fenced_code_uses_label_and_full_width_dark_background() {
        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        state.begin("show code", CognitiveMode::DirectAnswer, RiskLevel::Low);
        state.assistant_markdown("示例：\n```python\nprint('hello')\n```\n代码结束。");
        terminal.draw(|frame| state.draw(frame)).unwrap();

        assert!(
            state
                .transcript_lines
                .iter()
                .any(|line| { line.kind == TranscriptKind::Code && line.text.contains("python") })
        );
        let code_row = state
            .transcript_lines
            .iter()
            .position(|line| line.text.contains("print('hello')"))
            .unwrap();
        let area = state.transcript_area.unwrap();
        let y = area
            .y
            .saturating_add(u16::try_from(code_row - state.transcript_scroll_top).unwrap());
        assert_eq!(
            terminal.backend().buffer().cell((area.x, y)).unwrap().bg,
            CODE_BG
        );
        let content = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(!content.contains("```"));
        assert!(
            state.transcript_lines.iter().any(
                |line| line.kind == TranscriptKind::Assistant && line.text.contains("代码结束")
            )
        );

        let header_row = state
            .transcript_lines
            .iter()
            .position(|line| line.text == "  ╭─ python")
            .unwrap();
        let body_row = state
            .transcript_lines
            .iter()
            .position(|line| line.text.contains("print('hello')"))
            .unwrap();
        let footer_row = state
            .transcript_lines
            .iter()
            .position(|line| line.text == "  ╰────")
            .unwrap();
        let row_y = |line: usize| {
            area.y
                .saturating_add(u16::try_from(line - state.transcript_scroll_top).unwrap())
        };
        let border = terminal
            .backend()
            .buffer()
            .cell((area.x + 2, row_y(header_row)))
            .unwrap();
        assert_eq!(border.fg, Color::LightGreen);
        assert!(border.modifier.contains(Modifier::BOLD));
        assert_eq!(border.bg, CODE_BG);
        let language_column =
            state.transcript_lines[header_row].text[.."  ╭─ ".len()].width() as u16;
        assert_eq!(
            terminal
                .backend()
                .buffer()
                .cell((area.x + language_column, row_y(header_row)))
                .unwrap()
                .fg,
            Color::Indexed(75)
        );
        assert_eq!(
            terminal
                .backend()
                .buffer()
                .cell((area.x + 2, row_y(body_row)))
                .unwrap()
                .fg,
            Color::LightGreen
        );
        assert_eq!(
            terminal
                .backend()
                .buffer()
                .cell((area.x + 2, row_y(footer_row)))
                .unwrap()
                .fg,
            Color::LightGreen
        );
        assert_eq!(
            terminal
                .backend()
                .buffer()
                .cell((area.x + 2, row_y(footer_row)))
                .unwrap()
                .bg,
            CODE_BG
        );
    }

    #[test]
    fn markdown_paragraphs_and_code_blocks_use_compact_spacing() {
        let mut state = TuiState::new("mock", 6, 120);
        state.assistant_markdown("第一段。\n\n\n\n```python\nprint(\"one\")\n```\n\n\n\n第二段。");
        let lines = state.build_transcript_lines(80);
        let header = lines
            .iter()
            .position(|line| line.text == "  ╭─ python")
            .unwrap();
        assert!(lines[header - 1].text.is_empty());
        assert!(lines[header - 2].text.contains("第一段"));

        let footer = lines
            .iter()
            .position(|line| line.text == "  ╰────")
            .unwrap();
        assert!(lines[footer + 1].text.is_empty());
        assert!(lines[footer + 2].text.contains("第二段"));
    }

    #[test]
    fn diff_code_uses_green_and_red_line_backgrounds() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        state.assistant_markdown("```diff\n@@ -1 +1 @@\n-print(\"old\")\n+print(\"new\")\n```");
        terminal.draw(|frame| state.draw(frame)).unwrap();

        let area = state.transcript_area.unwrap();
        let row_background = |needle: &str| {
            let row = state
                .transcript_lines
                .iter()
                .position(|line| line.text.contains(needle))
                .unwrap();
            let y = area.y + u16::try_from(row - state.transcript_scroll_top).unwrap();
            terminal.backend().buffer().cell((area.x, y)).unwrap().bg
        };
        assert_eq!(row_background("-print(\"old\")"), CODE_REMOVED_BG);
        assert_eq!(row_background("+print(\"new\")"), CODE_ADDED_BG);
    }

    #[test]
    fn bare_language_label_and_c_code_use_a_visible_code_fence() {
        let backend = TestBackend::new(100, 45);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        state.begin("show code", CognitiveMode::DirectAnswer, RiskLevel::Low);
        state.assistant_markdown(
            r#"下面是约 30 行的 C 代码示例，用于展示代码围栏样式：

c
#include <stdio.h>
#include <stdlib.h>

/* 计算斐波那契数列前 n 项并打印 */
static int fib(int n) {
    if (n < 2) return n;
    int a = 0, b = 1;
    for (int i = 2; i <= n; i++) {
        int t = a + b;
        a = b;
        b = t;
    }
    return b;
}

int main(int argc, char **argv) {
    int n = (argc > 1) ? atoi(argv[1]) : 10;
    if (n < 0) {
        fprintf(stderr, "n must be >= 0\n");
        return 1;
    }
    for (int i = 0; i < n; i++) {
        printf("fib(%d) = %d\n", i, fib(i));
    }
    return 0;
}

代码结束。"#,
        );
        terminal.draw(|frame| state.draw(frame)).unwrap();

        let code = state
            .transcript
            .iter()
            .find(|entry| entry.kind == TranscriptKind::Code)
            .expect("C block should be recognized");
        assert_eq!(code.label.as_deref(), Some("c"));
        assert!(code.text.starts_with("#include <stdio.h>"));
        assert!(
            code.text
                .contains("printf(\"fib(%d) = %d\\n\", i, fib(i));")
        );
        assert!(code.text.ends_with('}'));
        assert_eq!(
            state
                .transcript
                .iter()
                .filter(|entry| entry.kind == TranscriptKind::Code)
                .count(),
            1
        );
        assert!(
            state
                .transcript
                .iter()
                .all(|entry| !(entry.kind == TranscriptKind::Assistant && entry.text.trim() == "c"))
        );

        assert!(
            state
                .transcript_lines
                .iter()
                .any(|line| line.text == "  ╭─ c")
        );
        assert!(
            state
                .transcript_lines
                .iter()
                .any(|line| line.text == "  │ #include <stdio.h>")
        );
        assert!(
            state
                .transcript_lines
                .iter()
                .any(|line| line.text == "  ╰────")
        );
        let code_row = state
            .transcript_lines
            .iter()
            .position(|line| line.text.contains("#include <stdio.h>"))
            .unwrap();
        let area = state.transcript_area.unwrap();
        let y = area
            .y
            .saturating_add(u16::try_from(code_row - state.transcript_scroll_top).unwrap());
        assert_eq!(
            terminal.backend().buffer().cell((area.x, y)).unwrap().bg,
            CODE_BG
        );
        assert!(state.transcript_lines.iter().any(|line| {
            line.kind == TranscriptKind::Assistant && line.text.contains("代码结束")
        }));
    }

    #[test]
    fn obvious_unlabelled_code_is_detected_but_prose_is_not() {
        assert!(unfenced_code_start(&["c", "这是普通说明。"], 0).is_none());

        let mut state = TuiState::new("mock", 6, 120);
        state.assistant_markdown(
            "普通说明不会变成代码。\n\
             \n\
             #include <stdio.h>\n\
             int main(void) {\n\
                 return 0;\n\
             }\n\
             \n\
             这段仍然是普通说明。",
        );

        let code = state
            .transcript
            .iter()
            .find(|entry| entry.kind == TranscriptKind::Code)
            .expect("obvious C code should be recognized");
        assert_eq!(code.label.as_deref(), Some("c"));
        assert!(code.text.starts_with("#include <stdio.h>"));
        assert!(code.text.contains("int main(void) {"));
        assert!(
            state
                .transcript
                .iter()
                .any(|entry| entry.kind == TranscriptKind::Assistant
                    && entry.text.contains("这段仍然是普通说明"))
        );
    }

    #[test]
    fn expanded_language_labels_accept_common_language_starts() {
        let mut labels = CODE_LANGUAGE_LABELS.to_vec();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), CODE_LANGUAGE_LABELS.len());
        assert!(CODE_LANGUAGE_LABELS.len() >= 100);

        for (label, first_line) in [
            ("kotlin", "fun main() {"),
            ("csharp", "using System;"),
            ("ruby", "puts \"hello\""),
            ("php", "<?php"),
            ("swift", "import Foundation"),
            ("haskell", "module Main where"),
            ("dockerfile", "FROM rust:latest"),
            ("solidity", "pragma solidity ^0.8.0;"),
            ("nix", "{ pkgs }:"),
            ("tex", "\\documentclass{article}"),
            ("vue", "<template>"),
            ("protobuf", "syntax = \"proto3\";"),
            ("graphql", "query {"),
            ("julia", "function greet(name)"),
            ("ada", "with Ada.Text_IO;"),
            ("gleam", "pub fn main() {"),
            ("wgsl", "@vertex"),
            ("vhdl", "library ieee;"),
            ("raku", "my $value = 1;"),
            ("cypher", "MATCH (n) RETURN n;"),
            ("starlark", "def main():"),
            ("json5", "{"),
            ("astro", "<div>"),
        ] {
            assert_eq!(
                unfenced_code_start(&[label, first_line], 0),
                Some((label, 1)),
                "language label {label}"
            );
        }
    }

    #[test]
    fn syntax_highlighter_classifies_ide_basics_for_c_python_java_and_rust() {
        assert_eq!(
            code_tone_at_word("#include <stdio.h>", "c", "#include"),
            TuiTone::Reference
        );
        assert_eq!(
            code_tone_at_word("#include <stdio.h>", "c", "stdio.h"),
            TuiTone::Reference
        );
        let c = "int main(void) { return 42; } // entry";
        assert_eq!(code_tone_at_word(c, "c", "int"), TuiTone::Type);
        assert_eq!(code_tone_at_word(c, "c", "main"), TuiTone::Function);
        assert_eq!(code_tone_at_word(c, "c", "return"), TuiTone::Keyword);
        assert_eq!(code_tone_at_word(c, "c", "42"), TuiTone::Number);
        assert_eq!(code_tone_at_word(c, "c", "// entry"), TuiTone::Comment);

        assert_eq!(
            code_tone_at_word("import os", "python", "import"),
            TuiTone::Reference
        );
        assert_eq!(
            code_tone_at_word("import os", "python", "os"),
            TuiTone::Reference
        );
        assert_eq!(
            code_tone_at_word("def greet(name):", "python", "def"),
            TuiTone::Keyword
        );
        assert_eq!(
            code_tone_at_word("def greet(name):", "python", "greet"),
            TuiTone::Function
        );
        assert_eq!(
            code_tone_at_word("print(\"Hello\")", "python", "print"),
            TuiTone::Function
        );
        assert_eq!(
            code_tone_at_word("print(\"Hello\")", "python", "\"Hello\""),
            TuiTone::String
        );

        assert_eq!(
            code_tone_at_word("import java.util.List;", "java", "java"),
            TuiTone::Reference
        );
        assert_eq!(
            code_tone_at_word("import java.util.List;", "java", "List"),
            TuiTone::Reference
        );
        let java = "public class Main { void run() {} }";
        assert_eq!(code_tone_at_word(java, "java", "class"), TuiTone::Keyword);
        assert_eq!(code_tone_at_word(java, "java", "Main"), TuiTone::Type);
        assert_eq!(code_tone_at_word(java, "java", "run"), TuiTone::Function);

        assert_eq!(
            code_tone_at_word("use std::io;", "rust", "use"),
            TuiTone::Reference
        );
        assert_eq!(
            code_tone_at_word("use std::io;", "rust", "std"),
            TuiTone::Reference
        );
        let rust = "fn main() { println!(\"Hello\"); }";
        assert_eq!(code_tone_at_word(rust, "rust", "fn"), TuiTone::Keyword);
        assert_eq!(code_tone_at_word(rust, "rust", "main"), TuiTone::Function);
        assert_eq!(
            code_tone_at_word(rust, "rust", "println"),
            TuiTone::Function
        );
        assert_eq!(
            code_tone_at_word(rust, "rust", "\"Hello\""),
            TuiTone::String
        );
    }

    #[test]
    fn code_transcript_applies_syntax_tones_after_the_fence_gutter() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        state.assistant_markdown("```c\nint main(void) {\n    return 42;\n}\n```");
        terminal.draw(|frame| state.draw(frame)).unwrap();

        let line_index = state
            .transcript_lines
            .iter()
            .position(|line| line.text.contains("int main(void)"))
            .unwrap();
        let line = &state.transcript_lines[line_index];
        let area = state.transcript_area.unwrap();
        let y = area.y + u16::try_from(line_index - state.transcript_scroll_top).unwrap();
        let word_x = |word: &str| {
            area.x + u16::try_from(line.text[..line.text.find(word).unwrap()].width()).unwrap()
        };
        assert_eq!(
            terminal
                .backend()
                .buffer()
                .cell((word_x("int"), y))
                .unwrap()
                .fg,
            Color::Cyan
        );
        assert_eq!(
            terminal
                .backend()
                .buffer()
                .cell((word_x("main"), y))
                .unwrap()
                .fg,
            Color::Yellow
        );
    }

    #[test]
    fn markdown_quote_uses_a_dedicated_reference_surface() {
        let backend = TestBackend::new(70, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        state.assistant_markdown(
            "说明如下：\n> 对酒当歌，人生几何！\n> 譬如朝露，去日苦多。\n\n后文继续。",
        );
        terminal.draw(|frame| state.draw(frame)).unwrap();

        let quote = state
            .transcript
            .iter()
            .find(|entry| entry.kind == TranscriptKind::Quote)
            .expect("Markdown quote should be recognized");
        assert_eq!(quote.label.as_deref(), Some("引用"));
        assert_eq!(quote.tone, TuiTone::Supplementary);
        assert_eq!(quote.text, "对酒当歌，人生几何！\n譬如朝露，去日苦多。");
        assert!(
            state
                .transcript_lines
                .iter()
                .any(|line| line.text.contains("▌ 引用 对酒当歌"))
        );
        assert!(
            state
                .transcript_lines
                .iter()
                .any(|line| line.text.contains("▌ 譬如朝露"))
        );
    }

    #[test]
    fn query_results_are_marked_non_original_and_use_their_own_background() {
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        state.assistant_query_result(
            "本地文件检索 · 成功",
            "src/tui.rs:1600: fn unfenced_code_start(...)",
        );
        terminal.draw(|frame| state.draw(frame)).unwrap();

        let result_line = state
            .transcript_lines
            .iter()
            .position(|line| line.text.contains("◇ 来源 · 本地文件检索 · 成功"))
            .expect("query result label");
        let area = state.transcript_area.unwrap();
        let y = area
            .y
            .saturating_add(u16::try_from(result_line - state.transcript_scroll_top).unwrap());
        assert_eq!(
            terminal.backend().buffer().cell((area.x, y)).unwrap().bg,
            Color::Reset
        );
        assert!(
            state
                .transcript
                .iter()
                .any(|entry| entry.kind == TranscriptKind::QueryResult
                    && entry.text.contains("fn unfenced_code_start"))
        );
        assert!(
            state
                .transcript_lines
                .iter()
                .any(|line| line.text.contains("│ src/tui.rs:1600"))
        );
        let body_row = state
            .transcript_lines
            .iter()
            .position(|line| line.text.contains("│ src/tui.rs:1600"))
            .unwrap();
        let area = state.transcript_area.unwrap();
        let y = area.y + u16::try_from(body_row - state.transcript_scroll_top).unwrap();
        let body = terminal.backend().buffer().cell((area.x + 2, y)).unwrap();
        assert_eq!(body.fg, MUTED_TEXT);
    }

    #[test]
    fn command_drawer_uses_readable_codex_like_hierarchy() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        state.set_input("/");
        terminal.draw(|frame| state.draw(frame)).unwrap();
        let buffer = terminal.backend().buffer();
        let rendered = buffer
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("/help"));
        assert!(rendered.contains('命'));
        assert!(
            buffer
                .content
                .iter()
                .any(|cell| { cell.symbol() != " " && cell.fg == UI_ACCENT })
        );
        assert!(
            buffer
                .content
                .iter()
                .any(|cell| { cell.symbol() != " " && cell.fg == COMMAND_DESCRIPTION })
        );
        assert!(
            buffer
                .content
                .iter()
                .any(|cell| { cell.symbol() != " " && cell.bg == Color::Indexed(238) })
        );

        state.set_input("/no-such-command");
        terminal.draw(|frame| state.draw(frame)).unwrap();
        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains('没'));
        assert!(rendered.contains('匹'));
    }

    #[test]
    fn command_panels_replace_each_other_and_escape_restores_conversation() {
        let mut state = TuiState::new("mock", 6, 120);
        state.info("existing conversation");
        state.begin_command_panel("/status");
        state.info("status panel content");
        let status = state.render_screen(100, 30);
        assert!(status.contains("status panel content"));
        assert!(!status.contains("existing conversation"));

        state.begin_command_panel("/cost");
        state.info("cost panel content");
        let cost = state.render_screen(100, 30);
        assert!(cost.contains("cost panel content"));
        assert!(!cost.contains("status panel content"));

        state.close_command_panel();
        let conversation = state.render_screen(100, 30);
        assert!(conversation.contains("existing conversation"));
    }

    #[test]
    fn command_panel_breadcrumb_tracks_subpanels() {
        let mut state = TuiState::new("mock", 6, 120);
        state.begin_command_panel("/workspace");
        assert_eq!(
            state.command_panel_breadcrumb().as_deref(),
            Some("/workspace")
        );
        state.begin_command_subpanel("设置工作区");
        assert_eq!(
            state.command_panel_breadcrumb().as_deref(),
            Some("/workspace › 设置工作区")
        );
    }

    #[test]
    fn command_panel_menus_are_selectable_and_return_to_their_parent() {
        let mut state = TuiState::new("mock", 6, 120);
        state.begin_command_panel("/workspace");
        state.info("workspace overview");
        state.set_command_panel_menu(vec![
            CommandMenuItem {
                id: "workspace.set".into(),
                label: "设置工作区".into(),
                description: "选择目录".into(),
            },
            CommandMenuItem {
                id: "workspace.diff".into(),
                label: "Git diff".into(),
                description: "查看变更".into(),
            },
        ]);
        assert!(state.command_panel_menu_open());
        assert_eq!(
            state.selected_command_panel_item().unwrap().id,
            "workspace.set"
        );
        state.move_command_panel_selection(1);
        assert_eq!(
            state.selected_command_panel_item().unwrap().id,
            "workspace.diff"
        );
        let parent = state.render_screen(100, 30);
        assert!(parent.contains("›"));
        assert!(parent.contains("Git diff"));

        state.begin_command_subpanel("/workspace › 设置工作区");
        state.info("choose an authorized root");
        state.set_command_panel_menu(vec![CommandMenuItem {
            id: "workspace.set.root.0".into(),
            label: "根目录 1".into(),
            description: "/workspace".into(),
        }]);
        let child = state.render_screen(100, 30);
        assert!(child.contains("choose an authorized root"));
        assert!(!child.contains("workspace overview"));

        assert!(state.back_command_panel());
        let parent = state.render_screen(100, 30);
        assert!(parent.contains("workspace overview"));
        assert!(!parent.contains("choose an authorized root"));
        assert_eq!(
            state.selected_command_panel_item().unwrap().id,
            "workspace.diff"
        );
    }

    #[test]
    fn long_command_panels_open_at_the_top_and_scroll_toward_the_end() {
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        state.begin_command_panel("/status");
        state.info(
            (1..=20)
                .map(|index| format!("panel-line-{index:02}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        terminal.draw(|frame| state.draw(frame)).unwrap();
        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("panel-line-01"));
        assert!(!rendered.contains("panel-line-20"));

        state.scroll_command_panel_down(usize::MAX / 2);
        terminal.draw(|frame| state.draw(frame)).unwrap();
        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(!rendered.contains("panel-line-01"));
        assert!(rendered.contains("panel-line-20"));
    }
}
