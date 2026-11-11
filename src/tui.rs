use crate::domain::{CognitiveMode, RiskLevel};
use crate::safety::is_disallowed_control;
use crate::tui_commands::{
    CommandDrawerRow, CommandStage, DrawerEntry, TuiCommandId, TuiCommandSpec, command_usage,
    drawer_rows, prompt_drawer_rows, visible_commands,
};
use crate::tui_host::HostBinding;
use crate::tui_prompts::{PromptLanguage, prompt_view};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
};
use std::ops::Range;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Whole-terminal backdrop, 80% of the input panel background.
const APP_BG: Color = Color::Rgb(0x06, 0x10, 0x0a);
const PANEL_BG: Color = Color::Rgb(0x08, 0x14, 0x0d);
const PANEL_GREEN: Color = Color::Rgb(0x3f, 0x64, 0x4b);
const PANEL_TEXT_GREEN: Color = Color::Rgb(0x48, 0x77, 0x48);
const INPUT_BG: Color = PANEL_BG;
const CODE_BG: Color = Color::Indexed(236);
const CODE_ADDED_BG: Color = Color::Indexed(22);
const CODE_REMOVED_BG: Color = Color::Indexed(52);
const MUTED_TEXT: Color = Color::Indexed(245);
const MARK_GREEN: Color = Color::Rgb(0x3f, 0xb9, 0x50);
const LINK: Color = Color::LightGreen;
const LINK_HOVER: Color = Color::Rgb(0xb7, 0xff, 0xc8);
const MAX_INPUT_BYTES: usize = 64 * 1024;
const COMMAND_DESCRIPTION: Color = PANEL_TEXT_GREEN;
const DRAWER_BORDER: Color = PANEL_GREEN;
const MENU_BG: Color = Color::Rgb(0x0a, 0x1f, 0x12);
const MENU_SELECTED_BG: Color = Color::Rgb(0x14, 0x3d, 0x24);
const MENU_LABEL: Color = Color::LightGreen;
const MENU_HINT: Color = PANEL_TEXT_GREEN;
const DEEP_GREEN: Color = Color::Rgb(0x1f, 0x4e, 0x28);
const CONCLUSION_BORDER: Color = DEEP_GREEN;
const LOCAL_FILE_GREEN: Color = DEEP_GREEN;

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
    Body,
    White,
    Link,
    LocalFile,
    WorkText,
    WorkLink,
    SectionText,
    Border,
    ConclusionBorder,
    ConclusionTitle,
    Strong,
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
    Work,
    WorkResult,
    ConclusionStart,
    ConclusionEnd,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiActivityKind {
    Understanding,
    Planning,
    Routing,
    Thinking,
    Reading,
    Writing,
    Creating,
    Updating,
    Searching,
    Fetching,
    Querying,
    Bash,
    Spawning,
    Skill,
    Delegating,
    CallingTool,
    ToolResult,
    Retrying,
    Compacting,
    Verifying,
    Answering,
    MasterUnderstanding,
    MasterPlanning,
    MasterApproval,
    MasterAssignment,
    MasterStarting,
    WorkerRunning,
    MasterCollecting,
    MasterVerifying,
    MasterReporting,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiActivity {
    pub kind: TuiActivityKind,
    pub title: String,
    pub detail: String,
    pub step: usize,
    pub footer: Option<String>,
}

impl TuiActivity {
    pub fn new(
        kind: TuiActivityKind,
        title: impl Into<String>,
        detail: impl Into<String>,
        step: usize,
    ) -> Self {
        Self {
            kind,
            title: sanitize_inline(&title.into()),
            detail: sanitize_inline(&detail.into()),
            step,
            footer: None,
        }
    }

    pub fn bilingual(kind: TuiActivityKind, detail: impl Into<String>, step: usize) -> Self {
        let (cn, en) = kind.label();
        Self {
            kind,
            title: format!("{cn} {en}"),
            detail: sanitize_inline(&detail.into()),
            step,
            footer: None,
        }
    }

    pub fn with_footer(mut self, footer: impl Into<String>) -> Self {
        self.footer = Some(sanitize_inline(&footer.into()));
        self
    }
}

impl TuiActivityKind {
    pub fn label(self) -> (&'static str, &'static str) {
        match self {
            Self::Understanding => ("任务理解", "Task Understanding"),
            Self::Planning => ("规划", "Planning"),
            Self::Routing => ("选择模型", "Model Routing"),
            Self::Thinking => ("模型思考", "Thinking"),
            Self::Reading => ("读取", "Reading"),
            Self::Writing => ("写入", "Writing"),
            Self::Creating => ("创建", "Creating"),
            Self::Updating => ("修改", "Updating"),
            Self::Searching => ("搜索", "Searching"),
            Self::Fetching => ("抓取", "Fetching"),
            Self::Querying => ("查询", "Querying"),
            Self::Bash => ("执行命令", "Bash"),
            Self::Spawning => ("子 Agent", "Subagent"),
            Self::Skill => ("技能", "Skill"),
            Self::Delegating => ("委托", "Delegation"),
            Self::CallingTool => ("调用工具", "Tool Call"),
            Self::ToolResult => ("工具结果", "Tool Result"),
            Self::Retrying => ("重试", "Retrying"),
            Self::Compacting => ("压缩上下文", "Compacting"),
            Self::Verifying => ("验收", "Verifying"),
            Self::Answering => ("整理答案", "Answering"),
            Self::MasterUnderstanding => ("任务理解", "Mission Understanding"),
            Self::MasterPlanning => ("任务规划", "Mission Planning"),
            Self::MasterApproval => ("等待批准", "Waiting Approval"),
            Self::MasterAssignment => ("任务分配", "Task Assignment"),
            Self::MasterStarting => ("任务启动", "Task Starting"),
            Self::WorkerRunning => ("Worker 运行", "Worker Running"),
            Self::MasterCollecting => ("收集结果", "Collecting Results"),
            Self::MasterVerifying => ("任务验收", "Mission Verification"),
            Self::MasterReporting => ("汇报", "Reporting"),
            Self::Blocked => ("阻塞", "Blocked"),
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
    pub prompt_language: PromptLanguage,
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
    context_tokens: usize,
    context_budget_tokens: usize,
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
    activity: Option<TuiActivity>,
    activity_persistent: bool,
    input_history: Vec<String>,
    history_cursor: Option<usize>,
    input: String,
    input_cursor: usize,
    input_scroll: usize,
    input_geometry: Option<InputGeometry>,
    input_box: Option<Rect>,
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
            prompt_language: PromptLanguage::Zh,
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
            context_tokens: 0,
            context_budget_tokens: crate::domain::DEFAULT_MAX_CONTEXT_TOKENS,
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
            activity: None,
            activity_persistent: false,
            input_history: Vec::new(),
            history_cursor: None,
            input: String::new(),
            input_cursor: 0,
            input_scroll: 0,
            input_geometry: None,
            input_box: None,
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
        self.activity = Some(TuiActivity::bilingual(
            TuiActivityKind::Understanding,
            "",
            0,
        ));
        self.activity_persistent = false;
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
        self.clear_activity();
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
            TuiTone::Body,
            None,
            TuiTone::Body,
            text.into(),
        );
    }

    pub fn assistant_conclusion_start(&mut self, label: impl Into<String>) {
        self.push_entry(
            TranscriptKind::ConclusionStart,
            TuiTone::ConclusionBorder,
            None,
            TuiTone::ConclusionBorder,
            label.into(),
        );
    }

    pub fn assistant_conclusion_end(&mut self) {
        self.push_entry(
            TranscriptKind::ConclusionEnd,
            TuiTone::ConclusionBorder,
            None,
            TuiTone::White,
            String::new(),
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

            if let Some(heading) = markdown_heading(trimmed) {
                self.flush_markdown_prose(&mut prose);
                self.push_entry(
                    TranscriptKind::Heading,
                    TuiTone::Strong,
                    None,
                    TuiTone::Strong,
                    heading.to_string(),
                );
                index += 1;
                continue;
            }

            if let Some((table, next)) = markdown_table(&lines, index) {
                self.flush_markdown_prose(&mut prose);
                self.assistant_body(table.join("\n"));
                index = next;
                continue;
            }

            if markdown_rule(trimmed) {
                self.flush_markdown_prose(&mut prose);
                self.assistant_body("─".repeat(40));
                index += 1;
                continue;
            }

            if let Some((number, text)) = markdown_numbered(trimmed) {
                self.flush_markdown_prose(&mut prose);
                let indent = lines[index].len().saturating_sub(trimmed.len());
                let text = strip_bold_markers(text);
                let text = if indent >= 2 {
                    format!("  {text}")
                } else {
                    text
                };
                self.assistant_body(format!("{number}. {text}"));
                index += 1;
                continue;
            }

            if let Some(item) = markdown_bullet(trimmed) {
                self.flush_markdown_prose(&mut prose);
                let indent = lines[index].len().saturating_sub(trimmed.len());
                let item = strip_bold_markers(item);
                if let Some((checked, text)) = task_checkbox(&item) {
                    let mark = if checked { "☑" } else { "☐" };
                    let text = if indent >= 2 {
                        format!("  {text}")
                    } else {
                        text.to_string()
                    };
                    self.assistant_item(mark, TuiTone::Strong, text);
                    index += 1;
                    continue;
                }
                let item = if indent >= 2 {
                    format!("  {item}")
                } else {
                    item
                };
                if let Some((label, rest)) = split_bold_label(&item) {
                    self.assistant_item(label, TuiTone::Strong, strip_bold_markers(rest));
                } else {
                    self.assistant_item("", TuiTone::White, item);
                }
                index += 1;
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
            label_tone,
            Some(label.into()),
            label_tone,
            text.into(),
        );
    }

    pub fn assistant_evidence(&mut self, source: impl Into<String>, text: impl Into<String>) {
        self.push_entry(
            TranscriptKind::Evidence,
            TuiTone::SectionText,
            Some(format!("[{}]", source.into())),
            TuiTone::SectionText,
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
        self.context_tokens = 0;
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

    /// The token budget the runtime compacts against.
    pub fn set_context_budget(&mut self, budget: usize) {
        self.context_budget_tokens = budget.max(1);
    }

    pub fn set_prompt_language(&mut self, language: PromptLanguage) {
        self.prompt_language = language;
    }

    /// How many tokens the most recent model request carried.
    pub fn set_context_tokens(&mut self, used: usize) {
        self.context_tokens = used;
    }

    pub fn context_usage_tokens(&self) -> usize {
        self.context_tokens
    }

    fn context_percent(&self) -> usize {
        self.context_tokens
            .saturating_mul(100)
            .checked_div(self.context_budget_tokens.max(1))
            .unwrap_or_default()
            .min(100)
    }

    fn context_bar_counts_for(&self, cells: usize) -> (usize, usize) {
        let percent = self.context_percent();
        let filled = (percent * cells + 50) / 100;
        (filled, cells - filled)
    }

    fn context_bar(&self) -> String {
        self.context_bar_with_cells(20)
    }

    fn context_bar_with_cells(&self, cells: usize) -> String {
        let percent = self.context_percent();
        // The styled renderer colours the first `filled` cells green and the
        // rest black; the plain text form keeps one glyph for both.
        format!(
            "上下文 {} {}/{} {}%",
            "─".repeat(cells),
            format_token_count(self.context_tokens as u64),
            format_token_count(self.context_budget_tokens as u64),
            percent
        )
    }

    fn styled_context_line(&self, text: String, base_style: Style) -> Vec<Span<'static>> {
        let Some(start) = text.find('─') else {
            return vec![Span::styled(text, base_style)];
        };
        let end = text[start..]
            .char_indices()
            .find(|(_, character)| *character != '─')
            .map(|(offset, _)| start + offset)
            .unwrap_or(text.len());
        let run = &text[start..end];
        let cells = run.chars().count();
        let filled = self.context_bar_counts_for(cells).0.min(cells);
        let green = run.chars().take(filled).collect::<String>();
        let gray = run.chars().skip(filled).collect::<String>();
        let mut spans = Vec::new();
        if start > 0 {
            spans.push(Span::styled(text[..start].to_string(), base_style));
        }
        if !green.is_empty() {
            spans.push(Span::styled(green, base_style.fg(PANEL_TEXT_GREEN)));
        }
        if !gray.is_empty() {
            spans.push(Span::styled(gray, base_style.fg(Color::Black)));
        }
        if end < text.len() {
            spans.push(Span::styled(text[end..].to_string(), base_style));
        }
        spans
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
            let entries = self.filtered_drawer_entries();
            let exact_command = entries.iter().any(|entry| {
                entry
                    .command()
                    .is_some_and(|command| command.spec.name == self.input)
            });
            if !exact_command {
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
        self.normalize_input_state();
        if let Some((start, end)) = self.selection_range() {
            self.input.replace_range(start..end, "");
            self.input_cursor = start;
        }
        self.input.insert_str(self.input_cursor, &text);
        self.input_cursor += text.len();
        if self.input.len() > MAX_INPUT_BYTES {
            let cut = floor_char_boundary(&self.input, MAX_INPUT_BYTES);
            self.input.truncate(cut);
            self.input_cursor = self.input_cursor.min(cut);
            self.notice = Some("输入过长，已截断到 64 KiB".into());
        }
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
        self.normalize_input_state();
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
        self.normalize_input_state();
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
        self.normalize_input_state();
        self.selection_anchor = None;
        self.clear_transcript_selection();
        self.input_cursor = previous_grapheme_boundary(&self.input, self.input_cursor);
    }

    pub fn move_right(&mut self) {
        self.normalize_input_state();
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
        self.normalize_input_state();
        self.input_cursor = self.input.len();
    }

    fn normalized_input_cursor(&self) -> usize {
        let mut cursor = self.input_cursor.min(self.input.len());
        while cursor > 0 && !self.input.is_char_boundary(cursor) {
            cursor -= 1;
        }
        cursor
    }

    fn normalize_input_state(&mut self) {
        self.input_cursor = self.normalized_input_cursor();
        self.input_scroll = self.input_scroll.min(self.input_cursor);
        while self.input_scroll > 0 && !self.input.is_char_boundary(self.input_scroll) {
            self.input_scroll -= 1;
        }
        if let Some(anchor) = self.selection_anchor
            && (anchor > self.input.len() || !self.input.is_char_boundary(anchor))
        {
            self.selection_anchor = None;
        }
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
        if let Some((_, query)) = self.prompt_drawer_parts() {
            return prompt_drawer_rows(query, self.prompt_language);
        }
        drawer_rows(
            &self.input,
            &self.host_binding,
            self.command_stage,
            self.status == TuiStatus::Running,
            &self.recent_commands,
            self.prompt_language,
        )
    }

    pub fn filtered_drawer_entries(&self) -> Vec<DrawerEntry> {
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

    pub fn set_activity(&mut self, activity: TuiActivity) {
        self.activity = Some(activity);
        self.activity_persistent = false;
    }

    pub fn set_master_activity(&mut self, activity: TuiActivity) {
        self.activity = Some(activity);
        self.activity_persistent = true;
    }

    pub fn push_work(&mut self, kind: TuiActivityKind, detail: impl Into<String>) {
        self.push_work_with_tone(kind, detail, activity_kind_tone(kind));
    }

    pub fn push_work_with_tone(
        &mut self,
        kind: TuiActivityKind,
        detail: impl Into<String>,
        tone: TuiTone,
    ) {
        let (cn, en) = kind.label();
        self.push_entry(
            TranscriptKind::Work,
            tone,
            Some(format!("{cn} {en}")),
            tone,
            detail.into(),
        );
    }

    pub fn push_work_result(&mut self, detail: impl Into<String>, tone: TuiTone) {
        self.push_entry(TranscriptKind::WorkResult, tone, None, tone, detail.into());
    }

    pub fn clear_activity(&mut self) {
        self.activity = None;
        self.activity_persistent = false;
    }

    pub fn activity(&self) -> Option<&TuiActivity> {
        self.activity.as_ref()
    }

    fn activity_visible(&self) -> bool {
        self.activity.is_some() && (self.status == TuiStatus::Running || self.activity_persistent)
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

    fn prompt_drawer_parts(&self) -> Option<(&str, &str)> {
        if let Some(query) = self.input.strip_prefix("//")
            && !query.chars().any(char::is_whitespace)
        {
            return Some(("", query));
        }
        let separator = self.input.rfind(" //")?;
        let base = self.input[..separator].trim_end();
        let query = &self.input[separator + 1..];
        if base.is_empty() || query.chars().any(char::is_whitespace) {
            return None;
        }
        Some((base, query))
    }

    pub fn slash_drawer_open(&self) -> bool {
        if self.slash_drawer_dismissed {
            return false;
        }
        if self.prompt_drawer_parts().is_some() {
            return true;
        }
        let input = self.input.trim_start();
        input.starts_with('/') && !input.chars().any(char::is_whitespace)
    }

    pub fn dismiss_slash_drawer(&mut self) {
        let base = self.prompt_drawer_parts().map(|(base, _)| base.to_string());
        if let Some(base) = base {
            self.input = base;
            self.input_cursor = self.input.len();
            self.input_scroll = 0;
        }
        self.slash_drawer_dismissed = true;
    }

    pub fn move_command_selection(&mut self, delta: isize) {
        let count = self.filtered_drawer_entries().len();
        if count == 0 {
            return;
        }
        let current = self.selected_command.min(count - 1) as isize;
        self.selected_command = (current + delta).rem_euclid(count as isize) as usize;
    }

    pub fn complete_selected_command(&mut self) -> bool {
        let entries = self.filtered_drawer_entries();
        let Some(entry) = entries.get(self.selected_command.min(entries.len().saturating_sub(1)))
        else {
            return false;
        };
        let prompt_base = self
            .prompt_drawer_parts()
            .map(|(base, _)| base.to_string())
            .filter(|base| !base.is_empty());
        match entry {
            DrawerEntry::Command(entry) => {
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
            DrawerEntry::Prompt(prompt) => {
                let Some(view) = prompt_view(prompt, self.prompt_language)
                    .or_else(|| prompt_view(prompt, PromptLanguage::Zh))
                else {
                    return false;
                };
                self.input = match prompt_base {
                    Some(base) if prompt.insertion == "作为前置指令" => {
                        format!("{}\n{base}", view.body)
                    }
                    Some(base) => format!("{base}\n{}", view.body),
                    None => view.body.to_string(),
                };
                self.input_cursor = self.input.len();
                self.input_scroll = 0;
                self.selection_anchor = None;
                self.slash_drawer_dismissed = true;
                if view.variables.is_empty() {
                    self.set_notice(format!("已插入提示词：{}", view.name));
                } else {
                    self.set_notice(format!(
                        "已插入提示词：{} · 变量：{}",
                        view.name, view.variables
                    ));
                }
                true
            }
            DrawerEntry::Source(source) => {
                let text = match self.prompt_language {
                    PromptLanguage::Zh => source.translation,
                    PromptLanguage::En => source.original,
                };
                if self.input.contains(text) {
                    self.slash_drawer_dismissed = true;
                    self.set_notice(format!(
                        "原文已在输入框中：{} · {}",
                        source.author, source.topic
                    ));
                    return true;
                }
                self.input = match prompt_base {
                    Some(base) => format!("{base}\n{text}"),
                    None => text.to_string(),
                };
                self.input_cursor = self.input.len();
                self.input_scroll = 0;
                self.selection_anchor = None;
                self.slash_drawer_dismissed = true;
                self.set_notice(format!("已插入原文：{} · {}", source.author, source.topic));
                true
            }
        }
    }

    pub fn input_box_contains(&self, column: u16, row: u16) -> bool {
        self.input_box
            .is_some_and(|area| area.contains((column, row).into()))
    }

    pub fn select_at(&mut self, column: u16, row: u16, extend: bool) {
        if let Some(geometry) = self.input_geometry
            && self
                .input_box
                .map(|area| area.contains((column, row).into()))
                .unwrap_or(row == geometry.row)
        {
            if extend && self.selection_anchor.is_none() {
                return;
            }
            let position = if row < geometry.row {
                0
            } else if row > geometry.row {
                self.input.len()
            } else {
                input_index_for_column(
                    &self.input,
                    geometry.visible_start,
                    geometry.visible_end,
                    geometry.text_x,
                    geometry.text_width,
                    column,
                )
            };
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
        if anchor > self.input.len() || !self.input.is_char_boundary(anchor) {
            return None;
        }
        let cursor = self.normalized_input_cursor();
        let range = anchor.min(cursor)..anchor.max(cursor);
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
        self.selection_autoscroll = 0;
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        frame.render_widget(
            Block::default().style(Style::default().bg(APP_BG)),
            frame.area(),
        );
        let folded = self.input_panel_folds(frame.area().width);
        let panel_height: u16 = if folded { 6 } else { 5 };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),
                Constraint::Length(1),
                Constraint::Length(panel_height),
            ])
            .split(frame.area());

        if self.command_panel.is_some() {
            self.draw_command_panel(frame, chunks[0]);
        } else if self.welcome_visible && self.transcript.is_empty() {
            self.draw_welcome(frame, chunks[0]);
        } else {
            self.draw_transcript(frame, chunks[0]);
        }
        frame.render_widget(
            Block::default().style(Style::default().bg(APP_BG)),
            chunks[1],
        );
        let panel_area = chunks[2];
        let composer_area = Rect {
            x: panel_area.x,
            y: panel_area.y.saturating_add(if folded { 2 } else { 1 }),
            width: panel_area.width,
            height: 3,
        };
        self.draw_input_panel(frame, panel_area, composer_area, folded);

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
                .style(Style::default().bg(PANEL_BG).fg(Color::White)),
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
                TranscriptKind::User => Some(PANEL_BG),
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
        frame.render_widget(Block::default().style(Style::default().bg(APP_BG)), area);
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
                Line::styled("超级个体俱乐部", Style::default().fg(PANEL_TEXT_GREEN))
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
                    Span::styled(prefix, Style::default().fg(PANEL_TEXT_GREEN)),
                    Span::styled(name, Style::default().fg(link_colour)),
                    Span::styled(tld, Style::default().fg(link_colour)),
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
        let mut in_conclusion = false;
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
                    | TranscriptKind::ConclusionStart
            ) && lines.last().is_some_and(|line| !line.text.is_empty())
            {
                lines.push(blank_transcript_line(
                    TranscriptKind::Assistant,
                    TuiTone::Default,
                ));
            }
            let (prefix, prefix_tone) = match entry.kind {
                TranscriptKind::User => ("› ", TuiTone::Success),
                TranscriptKind::Assistant => {
                    (if in_conclusion { "   " } else { "  " }, TuiTone::Default)
                }
                TranscriptKind::Heading => ("▌ ", entry.tone),
                TranscriptKind::Item => ("  • ", entry.tone),
                TranscriptKind::Quote => ("  ▌ ", TuiTone::Supplementary),
                TranscriptKind::QueryResult => ("  ◇ ", TuiTone::Muted),
                TranscriptKind::Evidence => ("  ↳ ", entry.tone),
                TranscriptKind::Code => ("  ", TuiTone::Code),
                TranscriptKind::Metadata => ("  ", TuiTone::Muted),
                TranscriptKind::Work => ("• ", entry.tone),
                TranscriptKind::WorkResult => ("  ⎿ ", TuiTone::Body),
                TranscriptKind::ConclusionStart | TranscriptKind::ConclusionEnd => {
                    ("", TuiTone::ConclusionBorder)
                }
                TranscriptKind::System => ("Sys ", TuiTone::Muted),
                TranscriptKind::Error => ("Err ", TuiTone::Danger),
            };
            let label = entry.label.as_deref().unwrap_or_default();
            if entry.kind == TranscriptKind::ConclusionStart {
                let label = if entry.text.trim().is_empty() {
                    "结论 Conclusion"
                } else {
                    entry.text.trim()
                };
                let top_prefix = "╭─ ";
                let right = "╮";
                let label = truncate_to_width(label, usize::from(width.saturating_sub(10)));
                let fill = usize::from(width)
                    .saturating_sub(top_prefix.width() + label.width() + 1 + right.width());
                let text = format!("{top_prefix}{label} {}╮", "─".repeat(fill));
                lines.push(TranscriptVisualLine {
                    kind: TranscriptKind::ConclusionStart,
                    tone: TuiTone::ConclusionBorder,
                    text,
                    prompt_text: None,
                    prefix_len: top_prefix.len(),
                    prefix_tone: TuiTone::ConclusionBorder,
                    label_range: Some(top_prefix.len()..top_prefix.len() + label.len()),
                    label_tone: TuiTone::ConclusionTitle,
                    syntax_ranges: Vec::new(),
                    background: None,
                });
                lines.push(blank_transcript_line(
                    TranscriptKind::Assistant,
                    TuiTone::Body,
                ));
                in_conclusion = true;
                continue;
            }
            if entry.kind == TranscriptKind::ConclusionEnd {
                in_conclusion = false;
                lines.push(blank_transcript_line(
                    TranscriptKind::Assistant,
                    TuiTone::Body,
                ));
                lines.push(TranscriptVisualLine {
                    kind: TranscriptKind::ConclusionEnd,
                    tone: TuiTone::ConclusionBorder,
                    text: format!("╰{}╯", "─".repeat(usize::from(width).saturating_sub(2))),
                    prompt_text: None,
                    prefix_len: 0,
                    prefix_tone: TuiTone::ConclusionBorder,
                    label_range: None,
                    label_tone: TuiTone::White,
                    syntax_ranges: Vec::new(),
                    background: None,
                });
                continue;
            }
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
            if entry.kind == TranscriptKind::Work {
                let first_prefix = if entry.text.trim().is_empty() {
                    format!("• {label}")
                } else {
                    format!("• {label} · ")
                };
                let continuation_prefix = "  ";
                let first_width = usize::from(width)
                    .saturating_sub(first_prefix.width())
                    .max(1);
                let continuation_width = usize::from(width)
                    .saturating_sub(continuation_prefix.width())
                    .max(1);
                let mut first = true;
                for text_line in entry.text.lines() {
                    let wrapped = if first {
                        wrap_first_then_rest(text_line, first_width, continuation_width)
                    } else {
                        wrap_display(text_line, continuation_width)
                    };
                    for wrapped in wrapped {
                        let line_prefix = if first {
                            &first_prefix
                        } else {
                            continuation_prefix
                        };
                        let syntax_ranges = link_ranges(
                            &wrapped,
                            line_prefix.len(),
                            TuiTone::WorkLink,
                            TuiTone::WorkLink,
                        );
                        lines.push(TranscriptVisualLine {
                            kind: TranscriptKind::Work,
                            tone: TuiTone::WorkText,
                            text: format!("{line_prefix}{wrapped}"),
                            prompt_text: None,
                            // Action names, the bullet and links are the
                            // bright-green accents; details stay deep green.
                            prefix_len: if first { "•".len() } else { 0 },
                            prefix_tone: TuiTone::WorkLink,
                            label_range: (first && !label.is_empty())
                                .then_some("• ".len().."• ".len() + label.len()),
                            label_tone: TuiTone::WorkLink,
                            syntax_ranges,
                            background: None,
                        });
                        first = false;
                    }
                }
                continue;
            }
            if entry.kind == TranscriptKind::WorkResult {
                let first_prefix = "  ⎿ ";
                let continuation_prefix = "    ";
                let mut first = true;
                for text_line in entry.text.lines() {
                    let line_prefix = if first {
                        first_prefix
                    } else {
                        continuation_prefix
                    };
                    let line_width = usize::from(width)
                        .saturating_sub(line_prefix.width())
                        .max(1);
                    for wrapped in wrap_display(text_line, line_width) {
                        let line_prefix = if first {
                            first_prefix
                        } else {
                            continuation_prefix
                        };
                        let syntax_ranges = link_ranges(
                            &wrapped,
                            line_prefix.len(),
                            TuiTone::WorkLink,
                            TuiTone::WorkLink,
                        );
                        lines.push(TranscriptVisualLine {
                            kind: TranscriptKind::WorkResult,
                            tone: TuiTone::WorkText,
                            text: format!("{line_prefix}{wrapped}"),
                            prompt_text: None,
                            prefix_len: line_prefix.width(),
                            prefix_tone: TuiTone::WorkText,
                            label_range: None,
                            label_tone: TuiTone::WorkText,
                            syntax_ranges,
                            background: None,
                        });
                        first = false;
                    }
                }
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
                    let in_answer = in_conclusion && entry.kind == TranscriptKind::Assistant;
                    let syntax_ranges = if entry.kind == TranscriptKind::Assistant {
                        link_ranges(
                            &wrapped,
                            line_prefix.len(),
                            TuiTone::Link,
                            TuiTone::LocalFile,
                        )
                    } else {
                        Vec::new()
                    };
                    lines.push(TranscriptVisualLine {
                        kind: entry.kind,
                        tone: if in_answer {
                            TuiTone::White
                        } else {
                            entry.tone
                        },
                        text: format!("{line_prefix}{wrapped}"),
                        prompt_text: (first && entry.kind == TranscriptKind::User)
                            .then(|| entry.text.clone()),
                        prefix_len: prefix.len(),
                        prefix_tone,
                        label_range,
                        label_tone: if in_answer {
                            TuiTone::White
                        } else {
                            entry.label_tone
                        },
                        syntax_ranges,
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

    fn input_panel_caption(&self, width: u16, bar_cells: usize) -> (String, String, bool) {
        let (title, detail, step) = if self.activity_visible() {
            self.activity_summary()
        } else {
            ("就绪".to_string(), "Ready".to_string(), 0)
        };
        let step = if step > 0 {
            format!(" · 第 {step} 步")
        } else {
            String::new()
        };
        let heading = if detail.is_empty() {
            format!("{title}{step}")
        } else {
            format!("{title} · {detail}{step}")
        };
        let (right_text, follow) = if let Some(notice) = &self.notice {
            (format!("✓ {notice}"), false)
        } else if let Some(label) = self.follow_control_label(width) {
            (label.to_string(), true)
        } else if let Some(footer) = self
            .activity
            .as_ref()
            .and_then(|activity| activity.footer.clone())
        {
            (footer, false)
        } else {
            (
                format!(
                    "{} · {} · {}",
                    self.run_metrics_line(),
                    self.context_bar_with_cells(bar_cells),
                    self.elapsed_label()
                ),
                false,
            )
        };
        (heading, right_text, follow)
    }

    /// The heading and metrics share the top border only while both fit. On a
    /// narrow terminal the metrics move to one row inside the same fence
    /// instead of being truncated away.
    fn input_panel_folds(&self, width: u16) -> bool {
        const MIN_FILL: usize = 3;
        let (heading, right_text, _) = self.input_panel_caption(width, 20);
        let heading = truncate_to_width(&heading, usize::from(width.saturating_sub(8)));
        // 15 fixed glyph columns plus the heading, right text and fill.
        let needed = 15 + MIN_FILL + heading.width() + right_text.width();
        needed > usize::from(width)
    }

    fn draw_input_panel(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        composer_area: Rect,
        folded: bool,
    ) {
        self.follow_control_area = None;
        if area.is_empty() {
            return;
        }

        let bg = Style::default().bg(PANEL_BG);
        let border_style = Style::default().fg(PANEL_GREEN).bg(PANEL_BG);
        let text_style = Style::default().fg(PANEL_TEXT_GREEN).bg(PANEL_BG);
        let bar_cells = if folded { 10 } else { 20 };
        let (heading, right_text, follow) = self.input_panel_caption(area.width, bar_cells);
        let right_text = if folded && area.width > 4 {
            truncate_to_width(&right_text, usize::from(area.width - 4))
        } else {
            right_text
        };

        let mut right_style = text_style;
        if follow && self.follow_control_hovered {
            right_style = right_style.add_modifier(Modifier::REVERSED);
        }

        let (top_spans, follow_area) = if folded {
            let heading = truncate_to_width(&heading, usize::from(area.width.saturating_sub(8)));
            let left_width = 3 + 2 + heading.width() + 1;
            let fill = usize::from(area.width)
                .saturating_sub(left_width + 1)
                .max(1);
            let top_spans = vec![
                Span::styled("╭─ ", border_style),
                Span::styled("• ", Style::default().fg(PANEL_TEXT_GREEN).bg(PANEL_BG)),
                Span::styled(heading, text_style),
                Span::styled(" ", border_style),
                Span::styled("─".repeat(fill), border_style),
                Span::styled("╮", border_style),
            ];
            let follow_area = follow.then(|| {
                Rect::new(
                    area.x.saturating_add(2),
                    area.y.saturating_add(1),
                    right_text.width() as u16,
                    1,
                )
            });
            (top_spans, follow_area)
        } else {
            let heading = truncate_to_width(&heading, usize::from(area.width.saturating_sub(12)));
            let left_width = 3 + 2 + heading.width() + 1;
            let reserved_right = 3 + 1 + 4 + 1;
            let max_right = usize::from(area.width)
                .saturating_sub(left_width + reserved_right + 1)
                .max(1);
            let right_text = truncate_to_width(&right_text, max_right);
            let right_width = 3 + right_text.width() + 1 + 4 + 1;
            let fill = usize::from(area.width)
                .saturating_sub(left_width + right_width)
                .max(1);
            let mut top_spans = vec![
                Span::styled("╭─ ", border_style),
                Span::styled("• ", Style::default().fg(PANEL_TEXT_GREEN).bg(PANEL_BG)),
                Span::styled(heading, text_style),
                Span::styled(" ", border_style),
                Span::styled("─".repeat(fill), border_style),
                Span::styled(" • ", text_style),
            ];
            top_spans.extend(self.styled_context_line(right_text.clone(), right_style));
            top_spans.push(Span::styled(" ", border_style));
            top_spans.push(Span::styled("─".repeat(4), border_style));
            top_spans.push(Span::styled("╮", border_style));
            let follow_area = follow.then(|| {
                let x = area.x.saturating_add((left_width + fill + 3) as u16);
                Rect::new(x, area.y, right_text.width() as u16, 1)
            });
            (top_spans, follow_area)
        };

        let status = self.panel_status_line();
        let status = truncate_to_width(&status, usize::from(area.width.saturating_sub(4)));
        let mut bottom_spans = vec![Span::styled("─ ", border_style)];
        bottom_spans.extend(shortcut_key_spans(status, text_style));
        bottom_spans.push(Span::styled(" ", border_style));
        let bottom_title = Line::from(bottom_spans);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style)
            .style(bg)
            .title_bottom(bottom_title);
        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new(Line::from(top_spans)).style(bg),
            Rect::new(area.x, area.y, area.width, 1),
        );

        if folded && area.width > 2 {
            let inner_width = usize::from(area.width - 2);
            let padding = inner_width.saturating_sub(1 + right_text.width());
            let mut metric_spans = vec![Span::styled(" ", text_style)];
            metric_spans.extend(self.styled_context_line(right_text, right_style));
            if padding > 0 {
                metric_spans.push(Span::styled(" ".repeat(padding), bg));
            }
            frame.render_widget(
                Paragraph::new(Line::from(metric_spans)).style(bg),
                Rect::new(
                    area.x.saturating_add(1),
                    area.y.saturating_add(1),
                    area.width.saturating_sub(2),
                    1,
                ),
            );
        }

        self.follow_control_area = follow_area;

        let inner = area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        });
        if inner.is_empty() {
            return;
        }
        let composer = Rect::new(
            composer_area.x,
            composer_area.y,
            composer_area.width,
            composer_area.height.min(inner.height),
        );
        self.draw_composer(frame, composer);
    }

    fn panel_status_line(&self) -> String {
        let model = self
            .provider_label
            .rsplit_once('/')
            .map(|(_, model)| model)
            .unwrap_or(&self.provider_label);
        let status = match self.status {
            TuiStatus::Running => "● 运行中",
            TuiStatus::Completed => "✓ 已完成",
            TuiStatus::Failed => "失败",
            TuiStatus::Cancelled => "已取消",
            TuiStatus::Idle => "就绪",
        };
        format!(
            "{status} · {model} · {} · Ctrl+T 模式 ",
            self.displayed_mode().label_zh()
        )
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
            self.input_box = None;
            return;
        }
        self.input_box = Some(area);

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
            Style::default().fg(PANEL_TEXT_GREEN).bg(PANEL_BG),
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

    fn input_spans(&self, visible_end: usize) -> Vec<Span<'static>> {
        let visible_end = visible_end.min(self.input.len());
        let selection = self
            .selection_range()
            .map(|(start, end)| start.max(self.input_scroll).min(visible_end)..end.min(visible_end))
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
            .style(Style::default().bg(MENU_BG)),
            area,
        );
        frame.render_widget(
            Paragraph::new(text)
                .style(Style::default().fg(Color::White).bg(MENU_BG))
                .wrap(Wrap { trim: false })
                .scroll((u16::try_from(top).unwrap_or(u16::MAX), 0))
                .block(
                    Block::bordered()
                        .border_style(
                            Style::default()
                                .fg(Color::LightGreen)
                                .bg(MENU_BG)
                                .add_modifier(Modifier::BOLD),
                        )
                        .style(Style::default().bg(MENU_BG)),
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
                    Style::default().fg(MENU_LABEL).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(MENU_LABEL)
                };
                ListItem::new(Line::from(vec![
                    Span::styled(bar, Style::default().fg(Color::LightGreen)),
                    Span::styled(
                        format!("{marker} "),
                        Style::default().fg(if is_active {
                            Color::LightGreen
                        } else {
                            MENU_HINT
                        }),
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
        let block = menu_block(&breadcrumb, hint, MENU_BG);
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
                        Span::styled(bar, Style::default().fg(Color::LightGreen)),
                        Span::styled(
                            fit_row(&item.label, 18),
                            Style::default()
                                .fg(Color::LightGreen)
                                .add_modifier(Modifier::BOLD),
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
                    Style::default()
                        .fg(PANEL_TEXT_GREEN)
                        .add_modifier(Modifier::BOLD),
                )));
            }
            let is_selected = selected == Some(index);
            let bar = if is_selected { "▌ " } else { "  " };
            let (name, detail, name_style, detail_style) = match row.entry {
                DrawerEntry::Command(entry) => {
                    let detail = match entry.unavailable {
                        Some(reason) => format!("{}  [{reason}]", entry.spec.description),
                        None => entry.spec.description.to_string(),
                    };
                    let (name_style, detail_style) = if entry.unavailable.is_some() {
                        (
                            Style::default().fg(PANEL_TEXT_GREEN),
                            Style::default().fg(PANEL_TEXT_GREEN),
                        )
                    } else {
                        (
                            Style::default()
                                .fg(Color::LightGreen)
                                .add_modifier(Modifier::BOLD),
                            Style::default().fg(Color::LightGreen),
                        )
                    };
                    (entry.spec.name, detail, name_style, detail_style)
                }
                DrawerEntry::Prompt(prompt) => {
                    let view = prompt_view(prompt, self.prompt_language)
                        .or_else(|| prompt_view(prompt, PromptLanguage::Zh))
                        .expect("Chinese prompt view is always available");
                    (
                        view.name,
                        format!("{} · {} · {}", view.value, view.category, view.purpose),
                        Style::default()
                            .fg(Color::LightGreen)
                            .add_modifier(Modifier::BOLD),
                        Style::default().fg(Color::LightGreen),
                    )
                }
                DrawerEntry::Source(source) => (
                    source.topic,
                    format!(
                        "{} · {} · {} 条",
                        source.author, source.category, source.fragments
                    ),
                    Style::default()
                        .fg(Color::LightGreen)
                        .add_modifier(Modifier::BOLD),
                    Style::default().fg(Color::LightGreen),
                ),
            };
            items.push(ListItem::new(Line::from(vec![
                Span::styled(bar, Style::default().fg(Color::LightGreen)),
                Span::styled(fit_row(name, 22), name_style),
                Span::styled(detail, detail_style),
            ])));
            if is_selected {
                selected_item = Some(items.len() - 1);
            }
        }
        if items.is_empty() {
            items.push(ListItem::new(Line::styled(
                " 没有匹配的命令或提示词",
                Style::default().fg(PANEL_TEXT_GREEN),
            )));
            selected_item = Some(0);
        }
        let mut state = ListState::default();
        state.select(selected_item);
        frame.render_widget(Clear, area);
        let hint = if self.prompt_drawer_parts().is_some() {
            "↑↓ 选择 · Enter 插入 · Esc 取消"
        } else {
            "↑↓ 选择 · Tab 补全 · 输入关键词搜提示词 · Esc 关闭"
        };
        let block = menu_block(self.command_drawer_title(), hint, MENU_BG);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let selected_source = selected
            .and_then(|index| rows.get(index))
            .and_then(|row| row.entry.source());
        let (list_area, preview) = match selected_source.filter(|_| inner.width >= 72) {
            Some(source) => {
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
                    .split(inner);
                (chunks[0], Some((chunks[1], source)))
            }
            None => (inner, None),
        };
        frame.render_stateful_widget(
            List::new(items)
                .style(Style::default().bg(MENU_BG))
                .highlight_style(Style::default().bg(MENU_SELECTED_BG)),
            list_area,
            &mut state,
        );
        if let Some((preview_area, source)) = preview {
            let body = match self.prompt_language {
                PromptLanguage::Zh => source.translation,
                PromptLanguage::En => source.original,
            };
            let header = format!(
                "{} · {}\n{} · {} 条 · {} 字\n\n",
                source.author,
                source.topic,
                source.category,
                source.fragments,
                body.chars().count()
            );
            let block = Block::default()
                .borders(Borders::LEFT)
                .border_style(Style::default().fg(DRAWER_BORDER));
            let preview_inner = block.inner(preview_area);
            frame.render_widget(block, preview_area);
            frame.render_widget(
                Paragraph::new(format!("{header}{body}"))
                    .wrap(Wrap { trim: false })
                    .style(Style::default().fg(Color::White).bg(MENU_BG)),
                preview_inner,
            );
        }
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
        self.normalize_input_state();
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
        let steps = if self.max_steps == 0 {
            "步数不限".to_string()
        } else {
            format!("最多 {} 步", self.max_steps)
        };
        let time = if self.timeout_seconds == 0 {
            "不限时".to_string()
        } else {
            format!("{} 秒", self.timeout_seconds)
        };
        format!(
            " 1A · {} · {} · {}风险 · {} · {} / {} · {}",
            self.status.label_zh(),
            self.mode.label_zh(),
            self.risk.label_zh(),
            model,
            steps,
            time,
            self.host_binding.kind().label_zh()
        )
    }

    fn command_drawer_title(&self) -> &'static str {
        if self.prompt_drawer_parts().is_some() {
            " 原文库 "
        } else if self.host_binding.is_workshop_master() {
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
        let context = self.context_bar();
        match self.status {
            TuiStatus::Running => {
                let (title, detail, step) = self.activity_summary();
                let step = if step > 0 {
                    format!(" · 第 {step} 步")
                } else {
                    String::new()
                };
                let detail = if detail.is_empty() {
                    String::new()
                } else {
                    format!(" · {detail}")
                };
                format!(
                    " {title}{detail}{step} · {metrics} · {context} · {} · Ctrl+C 取消",
                    self.elapsed_label()
                )
            }
            TuiStatus::Idle => format!(" 就绪 · {context}"),
            TuiStatus::Completed => {
                format!(
                    " ✓ 已完成 · {} · {metrics} · {context}",
                    self.elapsed_label()
                )
            }
            TuiStatus::Failed => {
                format!(" 失败 · {} · {metrics} · {context}", self.elapsed_label())
            }
            TuiStatus::Cancelled => {
                format!(" 已取消 · {} · {metrics} · {context}", self.elapsed_label())
            }
        }
    }

    fn activity_summary(&self) -> (String, String, usize) {
        match &self.activity {
            Some(activity) => (
                activity.title.clone(),
                activity.detail.clone(),
                activity.step,
            ),
            None => ("准备请求模型 Prepare Request".into(), String::new(), 0),
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
            return "↑ — / ↓ — · 缓存 —".to_string();
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
            .map(|percent| format!("缓存 {percent}%"))
            .unwrap_or_else(|| "缓存 —".to_string());
        format!(
            "↑ {} / ↓ {} · {cache}",
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
            self.assistant_body(strip_bold_markers(&text));
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

fn markdown_heading(line: &str) -> Option<&str> {
    for level in 1..=3 {
        let prefix = "#".repeat(level);
        let Some(rest) = line.strip_prefix(&prefix) else {
            continue;
        };
        let Some(text) = rest.strip_prefix(' ') else {
            continue;
        };
        let text = text.trim();
        if !text.is_empty() {
            return Some(text);
        }
    }
    None
}

fn markdown_bullet(line: &str) -> Option<&str> {
    ["- ", "* ", "+ "]
        .into_iter()
        .find_map(|marker| line.strip_prefix(marker))
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

fn markdown_numbered(line: &str) -> Option<(&str, &str)> {
    let digits = line
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    if digits.is_empty() || digits.len() > 3 {
        return None;
    }
    let number = &line[..digits.len()];
    let rest = line[digits.len()..].strip_prefix(". ")?;
    let rest = rest.trim();
    (!rest.is_empty()).then_some((number, rest))
}

fn task_checkbox(text: &str) -> Option<(bool, &str)> {
    let rest = text.strip_prefix('[')?;
    let close = rest.find(']')?;
    let mark = rest[..close].trim();
    let tail = rest[close + 1..].trim();
    match mark {
        "" => Some((false, tail)),
        "x" | "X" => Some((true, tail)),
        _ => None,
    }
}

fn markdown_rule(line: &str) -> bool {
    let line = line.trim();
    line.len() >= 3
        && (line.chars().all(|character| character == '-')
            || line.chars().all(|character| character == '*')
            || line.chars().all(|character| character == '_'))
}

fn split_bold_label(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix("**")?;
    let end = rest.find("**")?;
    let label = rest[..end].trim();
    if label.is_empty() {
        return None;
    }
    let tail = rest[end + 2..].trim_start_matches([' ', ':', '：']);
    Some((label, tail))
}

fn strip_bold_markers(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("**") {
        output.push_str(&rest[..start]);
        rest = &rest[start + 2..];
        if let Some(end) = rest.find("**") {
            output.push_str(&rest[..end]);
            rest = &rest[end + 2..];
        } else {
            output.push_str(rest);
            return output;
        }
    }
    output.push_str(rest);
    output
}

fn is_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.matches('|').count() >= 2
}

fn is_table_separator(line: &str) -> bool {
    if !is_table_row(line) {
        return false;
    }
    let cells = line
        .trim()
        .trim_matches('|')
        .split('|')
        .map(str::trim)
        .collect::<Vec<_>>();
    !cells.is_empty()
        && cells.iter().all(|cell| {
            let cell = cell.trim_matches(':');
            !cell.is_empty() && cell.chars().all(|character| character == '-')
        })
}

fn parse_table_row(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

fn pad_to_width(text: &str, width: usize) -> String {
    let text = truncate_to_width(text, width);
    let actual = text.width();
    if actual < width {
        format!("{text}{}", " ".repeat(width - actual))
    } else {
        text
    }
}

fn markdown_table(lines: &[&str], start: usize) -> Option<(Vec<String>, usize)> {
    let header = *lines.get(start)?;
    if !is_table_row(header) || !is_table_separator(lines.get(start + 1)?) {
        return None;
    }
    let mut rows = vec![parse_table_row(header)];
    let mut index = start + 2;
    while let Some(line) = lines.get(index) {
        if !is_table_row(line) {
            break;
        }
        rows.push(parse_table_row(line));
        index += 1;
    }
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    if columns < 2 {
        return None;
    }
    let mut widths = vec![1usize; columns];
    for row in &rows {
        for (column, cell) in row.iter().enumerate() {
            widths[column] = widths[column].max(cell.width()).min(24);
        }
    }
    while widths.iter().sum::<usize>() + columns * 3 + 1 > 100 {
        let Some(max) = widths.iter().copied().max() else {
            break;
        };
        if max <= 6 {
            break;
        }
        for width in &mut widths {
            *width = (*width).min(max - 1).max(6);
        }
    }
    let mut output = Vec::new();
    for (row_index, row) in rows.iter().enumerate() {
        let cells = (0..columns)
            .map(|column| {
                let cell = row.get(column).map(String::as_str).unwrap_or("");
                pad_to_width(&strip_bold_markers(cell), widths[column])
            })
            .collect::<Vec<_>>();
        output.push(format!("| {} |", cells.join(" | ")));
        if row_index == 0 {
            let separator = widths
                .iter()
                .map(|width| "-".repeat(width + 2))
                .collect::<Vec<_>>()
                .join("|");
            output.push(format!("|{separator}|"));
        }
    }
    Some((output, index))
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

fn activity_kind_tone(kind: TuiActivityKind) -> TuiTone {
    // One accent for work in progress; colour is reserved for outcomes.
    match kind {
        TuiActivityKind::Verifying
        | TuiActivityKind::Answering
        | TuiActivityKind::MasterVerifying
        | TuiActivityKind::MasterReporting => TuiTone::Success,
        TuiActivityKind::Retrying | TuiActivityKind::MasterApproval => TuiTone::Warning,
        TuiActivityKind::Blocked => TuiTone::Danger,
        TuiActivityKind::Understanding
        | TuiActivityKind::Planning
        | TuiActivityKind::Routing
        | TuiActivityKind::Thinking
        | TuiActivityKind::Reading
        | TuiActivityKind::Writing
        | TuiActivityKind::Creating
        | TuiActivityKind::Updating
        | TuiActivityKind::Searching
        | TuiActivityKind::Fetching
        | TuiActivityKind::Querying
        | TuiActivityKind::Bash
        | TuiActivityKind::Spawning
        | TuiActivityKind::Skill
        | TuiActivityKind::Delegating
        | TuiActivityKind::CallingTool
        | TuiActivityKind::ToolResult
        | TuiActivityKind::Compacting
        | TuiActivityKind::MasterUnderstanding
        | TuiActivityKind::MasterPlanning
        | TuiActivityKind::MasterAssignment
        | TuiActivityKind::MasterStarting
        | TuiActivityKind::WorkerRunning
        | TuiActivityKind::MasterCollecting => TuiTone::Evidence,
    }
}

fn floor_char_boundary(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
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

const BARE_FILE_EXTENSIONS: &[&str] = &[
    "md", "markdown", "html", "htm", "pdf", "txt", "rst", "adoc", "org", "csv", "tsv", "json",
    "jsonl", "yaml", "yml", "toml", "xml", "ini", "cfg", "conf", "env", "log", "jpg", "jpeg",
    "png", "gif", "webp", "svg", "bmp", "tiff", "tif", "ico", "heic", "doc", "docx", "xls", "xlsx",
    "ppt", "pptx", "odt", "ods", "odp", "rtf", "rs", "py", "js", "mjs", "cjs", "ts", "tsx", "jsx",
    "go", "java", "kt", "kts", "c", "cc", "cpp", "cxx", "h", "hpp", "cs", "rb", "php", "swift",
    "scala", "sh", "bash", "zsh", "fish", "ps1", "sql", "graphql", "gql", "proto", "ipynb", "lock",
    "zip", "tar", "gz", "bz2", "xz", "7z", "rar", "mp3", "wav", "flac", "ogg", "mp4", "mov", "avi",
    "mkv", "webm", "woff", "woff2", "ttf", "otf", "wasm", "bin", "exe",
];

fn link_ranges(
    text: &str,
    offset: usize,
    link_tone: TuiTone,
    file_tone: TuiTone,
) -> Vec<(Range<usize>, TuiTone)> {
    let mut ranges = Vec::new();
    let mut token_start = 0usize;
    for (index, character) in text
        .char_indices()
        .chain(std::iter::once((text.len(), '\0')))
    {
        if index == text.len() || character.is_whitespace() {
            if index > token_start {
                push_link_range(
                    &text[token_start..index],
                    offset + token_start,
                    &mut ranges,
                    link_tone,
                    file_tone,
                );
            }
            token_start = index.saturating_add(character.len_utf8());
        }
    }
    ranges
}

fn push_link_range(
    token: &str,
    start: usize,
    ranges: &mut Vec<(Range<usize>, TuiTone)>,
    link_tone: TuiTone,
    file_tone: TuiTone,
) {
    let trimmed = token.trim_matches(|character: char| {
        matches!(
            character,
            ',' | ';'
                | ':'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '"'
                | '\''
                | '`'
                | '。'
                | '，'
                | '；'
                | '：'
                | '？'
                | '！'
                | '、'
        )
    });
    if trimmed.is_empty() {
        return;
    }
    let leading = token.len().saturating_sub(
        token
            .trim_start_matches(|character: char| {
                matches!(
                    character,
                    ',' | ';'
                        | ':'
                        | '('
                        | ')'
                        | '['
                        | ']'
                        | '{'
                        | '}'
                        | '"'
                        | '\''
                        | '`'
                        | '。'
                        | '，'
                        | '；'
                        | '：'
                        | '？'
                        | '！'
                        | '、'
                )
            })
            .len(),
    );
    let range_start = start + leading;
    let range_end = range_start + trimmed.len();

    if let Some(position) = trimmed.find("http://").or_else(|| trimmed.find("https://")) {
        let start = range_start + position;
        let url = &trimmed[position..];
        ranges.push((start..start + url.len(), link_tone));
        return;
    }

    let normalized = trimmed.replace('\\', "/");
    let path = normalized
        .split_once(':')
        .filter(|(_, rest)| {
            !rest.is_empty()
                && rest
                    .chars()
                    .all(|character| character.is_ascii_digit() || character == ':')
        })
        .map(|(path, _)| path)
        .unwrap_or(normalized.as_str());
    if path.starts_with("http://") || path.starts_with("https://") || path.contains("://") {
        return;
    }
    let Some((stem, extension)) = path
        .rsplit('/')
        .next()
        .and_then(|name| name.rsplit_once('.'))
    else {
        return;
    };
    if stem.is_empty()
        || extension.is_empty()
        || extension.len() > 8
        || !extension
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
    {
        return;
    }
    let extension = extension.to_ascii_lowercase();
    let has_slash = path.contains('/');
    let absolute = path.starts_with('/')
        || path.starts_with("./")
        || path.starts_with("../")
        || path.starts_with("~/");
    if has_slash || absolute || BARE_FILE_EXTENSIONS.contains(&extension.as_str()) {
        ranges.push((range_start..range_end, file_tone));
    }
}

/// Render the `T` in `Ctrl+T` as a bright-green, bold shortcut key while the
/// rest of the status keeps the panel text style.
fn shortcut_key_spans(text: String, base: Style) -> Vec<Span<'static>> {
    let Some(start) = text.find("Ctrl+T") else {
        return vec![Span::styled(text, base)];
    };
    let key = start + "Ctrl+".len();
    let mut spans = Vec::new();
    if start > 0 {
        spans.push(Span::styled(text[..start].to_string(), base));
    }
    spans.push(Span::styled("Ctrl+".to_string(), base));
    spans.push(Span::styled(
        "T".to_string(),
        base.fg(Color::LightGreen).add_modifier(Modifier::BOLD),
    ));
    if key + 1 < text.len() {
        spans.push(Span::styled(text[key + 1..].to_string(), base));
    }
    spans
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
        TuiTone::Body => Style::default().fg(Color::Indexed(250)),
        TuiTone::White => Style::default().fg(Color::White),
        TuiTone::Link => Style::default().fg(Color::LightBlue),
        TuiTone::LocalFile => Style::default().fg(LOCAL_FILE_GREEN),
        TuiTone::WorkText => Style::default().fg(PANEL_TEXT_GREEN),
        TuiTone::WorkLink => Style::default().fg(Color::LightGreen),
        TuiTone::SectionText => Style::default().fg(PANEL_TEXT_GREEN),
        TuiTone::Border => Style::default().fg(Color::DarkGray),
        TuiTone::ConclusionBorder => Style::default().fg(CONCLUSION_BORDER),
        TuiTone::ConclusionTitle => Style::default()
            .fg(Color::LightGreen)
            .add_modifier(Modifier::BOLD),
        TuiTone::Strong => Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
        TuiTone::User => Style::default().fg(Color::White).bg(PANEL_BG),
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
                            Style::default().fg(PANEL_TEXT_GREEN)
                        };
                        Span::styled(c.to_string(), style)
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

pub fn welcome_logo_green_range(index: usize) -> Range<usize> {
    let Some(line) = WELCOME_LOGO.get(index) else {
        return 0..0;
    };
    let mut start = None;
    let mut end = 0;
    for (column, character) in line.chars().enumerate() {
        if column < 4 && character != ' ' {
            start.get_or_insert(column);
            end = column + 1;
        }
    }
    start.unwrap_or(0)..end
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
                .fg(Color::LightGreen)
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
    fn editing_fuzz_keeps_the_cursor_on_a_character_boundary() {
        let backend = TestBackend::new(74, 26);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        let samples = [
            "",
            "plain",
            "  spaces  ",
            "中文😀",
            "e\u{301}",
            "line\nnext\ttab",
            "/help",
            "\u{1b}[31mred",
            "a😀b 中 c",
            "\u{0}\u{202e}hidden",
        ];
        let chars = [
            'a', ' ', '中', '😀', '\u{301}', '/', '\n', '\t', '\u{1b}', '\u{202e}',
        ];
        let mut seed = 0x9e37_79b9_u64;
        for _ in 0..2500 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            match (seed >> 33) % 10 {
                0 => state.insert_text(samples[(seed as usize) % samples.len()]),
                1 => state.insert_char(chars[(seed as usize) % chars.len()]),
                2 => state.backspace(),
                3 => state.delete(),
                4 => state.move_left(),
                5 => state.move_right(),
                6 => state.move_home(),
                7 => state.move_end(),
                8 => {
                    terminal.draw(|frame| state.draw(frame)).unwrap();
                    let column = ((seed >> 8) % 90) as u16;
                    let row = ((seed >> 16) % 30) as u16;
                    state.select_at(column, row, false);
                }
                _ => {
                    state.select_at(40, 20, false);
                    state.update_selection_drag(((seed >> 8) % 90) as u16, 20);
                    state.end_selection_drag();
                }
            }
            assert!(state.input_cursor <= state.input.len());
            assert!(state.input.is_char_boundary(state.input_cursor));
            if let Some(anchor) = state.selection_anchor {
                assert!(anchor <= state.input.len());
                assert!(state.input.is_char_boundary(anchor));
            }
            if state.input.len() > 4096 {
                state.clear_input();
            }
        }
        terminal.draw(|frame| state.draw(frame)).unwrap();
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
        let entries = state.filtered_drawer_entries();
        let approve = entries
            .iter()
            .filter_map(|entry| entry.command())
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
        let commands = rows
            .iter()
            .filter(|row| row.entry.command().is_some())
            .collect::<Vec<_>>();
        assert_eq!(commands[0].heading, Some("最近"));
        assert_eq!(
            commands[0].entry.command().unwrap().spec.id,
            TuiCommandId::Status
        );
        assert_eq!(
            commands[1].entry.command().unwrap().spec.id,
            TuiCommandId::Mode
        );
        assert_eq!(
            rows.iter()
                .filter(|row| {
                    row.entry
                        .command()
                        .is_some_and(|entry| entry.spec.id == TuiCommandId::Status)
                })
                .count(),
            1
        );
    }

    #[test]
    fn prompt_supplement_completion_fills_the_composer() {
        let mut state = TuiState::new("mock", 6, 120);
        state.set_input("/验收");
        let rows = state.filtered_drawer_rows();
        let index = rows
            .iter()
            .position(|row| {
                row.entry
                    .prompt()
                    .is_some_and(|prompt| prompt.name == "先说验收标准")
            })
            .expect("prompt row");
        state.selected_command = index;
        assert!(state.complete_selected_command());
        assert!(state.input().contains("先别动手"));
        assert!(state.input().contains("验收标准"));
        assert_eq!(state.input_cursor, state.input().len());
        assert!(!state.slash_drawer_open());
    }

    #[test]
    fn double_slash_opens_the_whole_source_library() {
        let mut state = TuiState::new("mock", 6, 120);
        state.set_input("//");
        assert!(state.slash_drawer_open());
        assert_eq!(state.command_drawer_title(), " 原文库 ");
        let rows = state.filtered_drawer_rows();
        assert_eq!(rows.len(), 32);
        assert!(rows.iter().all(|row| row.entry.source().is_some()));
        let source = rows[0].entry.source().expect("source row");
        state.selected_command = 0;
        assert!(state.complete_selected_command());
        assert!(state.input().contains("先想再写"));
        assert!(state.input().contains(source.translation));
        assert!(!state.input().contains('【'));
        assert!(!state.input().contains("参考："));
        assert!(!state.input().contains("https://x.com"));
        assert!(!state.input().starts_with('\n'));
        assert!(!state.input().ends_with('\n'));
        assert!(!state.slash_drawer_open());
    }

    #[test]
    fn source_drawer_previews_the_complete_body_before_insert() {
        let mut state = TuiState::new("mock", 6, 120);
        state.set_input("//");
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| state.draw(frame)).unwrap();
        let screen = (0..40)
            .map(|y| {
                (0..120)
                    .map(|x| terminal.backend().buffer().cell((x, y)).unwrap().symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        let compact = screen.replace(' ', "");
        assert!(compact.contains("ThinkBeforeCoding"), "{screen}");
        assert!(compact.contains("先想再写"), "{screen}");
        assert!(compact.contains("Enter插入"), "{screen}");
    }

    #[test]
    fn prompt_drawer_opens_after_a_task_and_merges_the_source_text() {
        let mut state = TuiState::new("mock", 6, 120);
        state.set_input("做跨境电商 //");
        assert!(state.slash_drawer_open());
        assert_eq!(state.command_drawer_title(), " 原文库 ");
        let rows = state.filtered_drawer_rows();
        assert!(rows.iter().all(|row| row.entry.source().is_some()));
        let source = rows[0].entry.source().expect("source row");
        state.selected_command = 0;
        assert!(state.complete_selected_command());
        assert!(state.input().starts_with("做跨境电商\n"));
        assert!(state.input().contains(source.translation));
        assert!(!state.input().contains('【'));
        assert!(!state.slash_drawer_open());
    }

    #[test]
    fn english_source_mode_inserts_the_complete_english_original() {
        let mut state = TuiState::new("mock", 6, 120);
        state.set_prompt_language(PromptLanguage::En);
        state.set_input("做跨境电商 //Altitude");
        let rows = state.filtered_drawer_rows();
        assert!(rows.iter().all(|row| row.entry.source().is_some()));
        let index = rows
            .iter()
            .position(|row| {
                row.entry
                    .source()
                    .is_some_and(|source| source.topic == "Altitude")
            })
            .expect("english source row");
        let source = rows[index].entry.source().expect("source row");
        state.selected_command = index;
        assert!(state.complete_selected_command());
        assert!(state.input().starts_with("做跨境电商"));
        assert!(
            state.input().contains("Altitude: Going up in abstraction"),
            "{}",
            state.input()
        );
        assert!(state.input().contains(source.original));
        assert!(!state.input().contains("高度：往上提高抽象层级"));
        assert!(!state.input().contains("[Felix Rieseberg"));
        assert!(!state.input().contains("Reference:"));
    }

    #[test]
    fn inserting_the_same_source_twice_does_not_duplicate_it() {
        let mut state = TuiState::new("mock", 6, 120);
        state.set_input("//");
        let source = state.filtered_drawer_rows()[0]
            .entry
            .source()
            .expect("source row");
        state.selected_command = 0;
        assert!(state.complete_selected_command());
        let first = state.input().to_string();
        state.set_input(&format!("{first} //"));
        state.selected_command = 0;
        assert!(state.complete_selected_command());
        assert_eq!(state.input().matches(source.translation).count(), 1);
    }

    #[test]
    fn prompt_drawer_searches_inside_a_task_and_escape_restores_it() {
        let mut state = TuiState::new("mock", 6, 120);
        state.set_input("做跨境电商 //Think");
        assert!(state.slash_drawer_open());
        let rows = state.filtered_drawer_rows();
        assert!(rows.iter().all(|row| row.entry.source().is_some()));
        assert!(rows.iter().any(|row| {
            row.entry
                .source()
                .is_some_and(|source| source.topic == "Think Before Coding")
        }));
        state.dismiss_slash_drawer();
        assert_eq!(state.input(), "做跨境电商");
        assert!(!state.slash_drawer_open());
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
        assert_eq!(prompt_cell.fg, PANEL_TEXT_GREEN);
        assert!(!prompt_cell.modifier.contains(Modifier::BOLD));
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
        terminal.draw(|frame| state.draw(frame)).unwrap();
        assert!(
            terminal
                .backend()
                .buffer()
                .cell((geometry.text_x, geometry.row))
                .unwrap()
                .modifier
                .contains(Modifier::REVERSED)
        );

        // The whole composer is a hit target, so a drag that starts on the
        // block padding still selects the input line.
        state.begin_selection_drag(geometry.text_x, geometry.row + 1);
        state.update_selection_drag(geometry.text_x, geometry.row);
        assert_eq!(state.selected_input_text().as_deref(), Some("hello"));
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
        let user_row = state
            .transcript_lines
            .iter()
            .position(|line| line.text.contains("用户问题"))
            .unwrap();
        let user_y = area.y + u16::try_from(user_row - state.transcript_scroll_top).unwrap();
        assert_eq!(
            terminal
                .backend()
                .buffer()
                .cell((area.x, user_y))
                .unwrap()
                .bg,
            PANEL_BG
        );
        let gap_y = area.y + u16::try_from(answer_row - 1 - state.transcript_scroll_top).unwrap();
        assert_ne!(
            terminal
                .backend()
                .buffer()
                .cell((area.x, gap_y))
                .unwrap()
                .bg,
            PANEL_BG
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
        assert_eq!(tone_style(TuiTone::Body).fg, Some(Color::Indexed(250)));
        assert!(
            !tone_style(TuiTone::Body)
                .add_modifier
                .contains(Modifier::BOLD)
        );
        assert_eq!(tone_style(TuiTone::White).fg, Some(Color::White));
        assert!(
            !tone_style(TuiTone::White)
                .add_modifier
                .contains(Modifier::BOLD)
        );
        assert_eq!(tone_style(TuiTone::Link).fg, Some(Color::LightBlue));
        assert_eq!(tone_style(TuiTone::LocalFile).fg, Some(LOCAL_FILE_GREEN));
        assert_eq!(tone_style(TuiTone::WorkText).fg, Some(PANEL_TEXT_GREEN));
        assert_eq!(tone_style(TuiTone::WorkLink).fg, Some(Color::LightGreen));
        assert_eq!(tone_style(TuiTone::SectionText).fg, Some(PANEL_TEXT_GREEN));
        assert_eq!(tone_style(TuiTone::Border).fg, Some(Color::DarkGray));
        assert_eq!(
            tone_style(TuiTone::ConclusionBorder).fg,
            Some(CONCLUSION_BORDER)
        );
        assert_eq!(
            tone_style(TuiTone::ConclusionTitle).fg,
            Some(Color::LightGreen)
        );
        assert!(
            tone_style(TuiTone::ConclusionTitle)
                .add_modifier
                .contains(Modifier::BOLD)
        );
        assert_eq!(tone_style(TuiTone::User).bg, Some(PANEL_BG));
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
        state.set_context_budget(1_000);
        state.set_context_tokens(250);
        terminal.draw(|frame| state.draw(frame)).unwrap();

        let rows = (0..18)
            .map(|y| {
                (0..120)
                    .map(|x| terminal.backend().buffer().cell((x, y)).unwrap().symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();
        let top = rows
            .iter()
            .find(|row| row.contains('╭') && row.replace(' ', "").contains("任务理解"))
            .expect("input panel top");
        let metrics = rows
            .iter()
            .find(|row| row.contains("↑ 1.2k / ↓ 340"))
            .expect("metrics card");
        let top_compact = top.replace(' ', "");
        let metrics_compact = metrics.replace(' ', "");
        assert!(top_compact.contains("任务理解"), "top: {top}");
        assert!(top_compact.contains("TaskUnderstanding"), "top: {top}");
        assert!(top.contains('╭') && top.contains('╮'), "top: {top}");
        assert!(metrics.contains("↑ 1.2k / ↓ 340"), "metrics: {metrics}");
        assert!(metrics_compact.contains("缓存40%"), "metrics: {metrics}");
        assert!(metrics.contains("250/1.0k"), "metrics: {metrics}");
        assert!(state.context_bar().contains("25%"));
        assert!(state.context_bar().contains("────────────────────"));
    }

    #[test]
    fn work_entries_use_a_bilingual_label_in_the_transcript_log() {
        let mut state = TuiState::new("mock", 6, 120);
        state.push_work(TuiActivityKind::Reading, "read_file · src/main.rs");
        let entry = state.transcript.last().unwrap();
        assert_eq!(entry.kind, TranscriptKind::Work);
        assert_eq!(entry.label.as_deref(), Some("读取 Reading"));
        assert!(entry.text.contains("read_file · src/main.rs"));
        assert_eq!(entry.label_tone, TuiTone::Evidence);

        state.push_work_result("read README.md (999 bytes)", TuiTone::Success);
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| state.draw(frame)).unwrap();
        let screen = (0..20)
            .map(|y| {
                (0..80)
                    .map(|x| terminal.backend().buffer().cell((x, y)).unwrap().symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        let compact = screen.replace(' ', "");
        assert!(compact.contains("•读取Reading·read_file"), "{screen}");
        assert!(compact.contains("⎿readREADME.md"), "{screen}");
    }

    #[test]
    fn work_log_uses_bright_green_labels_and_deep_green_details() {
        let mut state = TuiState::new("mock", 6, 120);
        state.push_work(TuiActivityKind::Reading, "read_file · src/main.rs");
        state.push_work_result("read README.md (999 bytes)", TuiTone::Success);

        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| state.draw(frame)).unwrap();
        let buffer = terminal.backend().buffer();
        let mut marker = None;
        let mut label = None;
        let mut result = None;
        for y in 0..20 {
            let work_row = (0..80).any(|x| buffer.cell((x, y)).unwrap().symbol() == "读");
            for x in 0..80 {
                let cell = buffer.cell((x, y)).unwrap();
                match cell.symbol() {
                    "•" if work_row => marker = Some(cell.clone()),
                    "读" => label = Some(cell.clone()),
                    "⎿" => result = Some(cell.clone()),
                    _ => {}
                }
            }
        }
        let marker = marker.expect("• marker");
        let label = label.expect("work label");
        let result = result.expect("result marker");

        // Action names and the bullet are bright-green accents; process text
        // and results stay deep green.
        assert_eq!(marker.fg, Color::LightGreen);
        assert_eq!(label.fg, Color::LightGreen);
        assert!(!label.modifier.contains(Modifier::BOLD));
        assert_eq!(result.fg, PANEL_TEXT_GREEN);
        assert!(!result.modifier.contains(Modifier::BOLD));
        let detail = state
            .transcript_lines
            .iter()
            .find(|line| line.text.contains("read_file"))
            .expect("work detail line");
        assert_eq!(detail.tone, TuiTone::WorkText);
        assert_eq!(detail.label_tone, TuiTone::WorkLink);
    }

    #[test]
    fn work_log_colors_links_bright_green() {
        let mut state = TuiState::new("mock", 6, 120);
        state.push_work(
            TuiActivityKind::Reading,
            "read_file · /home/skynet/1-agent/README.md",
        );
        state.push_work(
            TuiActivityKind::Fetching,
            "fetch_url · https://www.ruby-lang.org/en/documentation/",
        );
        state.transcript_lines = state.build_transcript_lines(120);

        let file = state
            .transcript_lines
            .iter()
            .find(|line| line.text.contains("README.md"))
            .expect("file line");
        assert!(file.syntax_ranges.iter().any(|(range, tone)| {
            *tone == TuiTone::WorkLink && file.text[range.clone()].contains("README.md")
        }));

        let url = state
            .transcript_lines
            .iter()
            .find(|line| line.text.contains("ruby-lang.org"))
            .expect("url line");
        assert!(url.syntax_ranges.iter().any(|(range, tone)| {
            *tone == TuiTone::WorkLink && url.text[range.clone()].contains("https://")
        }));
    }

    #[test]
    fn conclusion_markdown_renders_headings_bullets_and_bold_labels() {
        let mut state = TuiState::new("mock", 6, 120);
        state.assistant_conclusion_start("结论 Conclusion");
        state.assistant_markdown(
            "**结论**：可以。\n\n## 能力\n- **文件**：读写\n- 联网：搜索\n1. 第一步\n2. 第二步\n- [x] 已完成\n- [ ] 未完成\n---\n参考：https://example.com\n",
        );
        state.assistant_conclusion_end();
        state.transcript_lines = state.build_transcript_lines(100);
        assert!(
            state
                .transcript_lines
                .iter()
                .any(|line| line.kind == TranscriptKind::Heading && line.text.contains("能力"))
        );
        assert!(state.transcript_lines.iter().any(|line| {
            line.kind == TranscriptKind::Item
                && line.text.contains('•')
                && line.text.contains("文件")
        }));
        assert!(
            state
                .transcript_lines
                .iter()
                .any(|line| line.label_tone == TuiTone::Strong)
        );
        assert!(
            state
                .transcript_lines
                .iter()
                .any(|line| line.text.contains("1. 第一步"))
        );
        assert!(
            state
                .transcript_lines
                .iter()
                .any(|line| line.text.contains('☑') && line.text.contains("已完成"))
        );
        assert!(
            state
                .transcript_lines
                .iter()
                .any(|line| line.text.contains('☐') && line.text.contains("未完成"))
        );
        assert!(
            state
                .transcript_lines
                .iter()
                .any(|line| line.text.contains("────"))
        );
        let link = state
            .transcript_lines
            .iter()
            .find(|line| line.text.contains("example.com"))
            .expect("link line");
        assert!(link.syntax_ranges.iter().any(|(range, tone)| {
            *tone == TuiTone::Link && link.text[range.clone()].contains("example.com")
        }));
        assert!(
            !state
                .transcript_lines
                .iter()
                .any(|line| line.text.contains("**"))
        );
    }

    #[test]
    fn conclusion_markdown_renders_an_aligned_table() {
        let mut state = TuiState::new("mock", 6, 120);
        state.assistant_conclusion_start("结论 Conclusion");
        state.assistant_markdown(
            "| 维度 | A | B |\n|---|---|---|\n| 速度 | 快 | 慢 |\n| 成本 | 高 | 低 |\n",
        );
        state.assistant_conclusion_end();
        state.transcript_lines = state.build_transcript_lines(100);
        let rows = state
            .transcript_lines
            .iter()
            .filter(|line| line.text.contains('|'))
            .collect::<Vec<_>>();
        assert!(rows.len() >= 4, "{rows:?}");
        let widths = rows
            .iter()
            .map(|line| line.text.width())
            .collect::<Vec<_>>();
        assert!(
            widths.windows(2).all(|pair| pair[0] == pair[1]),
            "{widths:?}"
        );
    }

    #[test]
    fn final_answer_is_wrapped_in_a_conclusion_fence() {
        let mut state = TuiState::new("mock", 6, 120);
        state.assistant_conclusion_start("结论 Conclusion");
        state.assistant_markdown("不用启动，GridCraft 已经在运行。");
        state.assistant_conclusion_end();

        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| state.draw(frame)).unwrap();
        let screen = (0..20)
            .map(|y| {
                (0..80)
                    .map(|x| terminal.backend().buffer().cell((x, y)).unwrap().symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        let compact = screen.replace(' ', "");
        assert!(compact.contains("╭─结论Conclusion"), "{screen}");
        assert!(compact.contains("不用启动"), "{screen}");
        assert!(compact.contains("╰─"), "{screen}");

        let title = state
            .transcript_lines
            .iter()
            .find(|line| line.text.contains("结论"))
            .expect("conclusion title");
        assert_eq!(title.label_tone, TuiTone::ConclusionTitle);
        let title_row = state
            .transcript_lines
            .iter()
            .position(|line| line.text.contains("结论"))
            .expect("conclusion title row");
        let area = state.transcript_area.unwrap();
        let title_y = area.y + u16::try_from(title_row - state.transcript_scroll_top).unwrap();
        let title_cell = (0..80)
            .map(|x| terminal.backend().buffer().cell((x, title_y)).unwrap())
            .find(|cell| cell.symbol() == "结")
            .expect("conclusion title glyph");
        assert_eq!(title_cell.fg, Color::LightGreen);
        assert!(title_cell.modifier.contains(Modifier::BOLD));

        let body = state
            .transcript_lines
            .iter()
            .position(|line| line.text.contains("不用启动"))
            .expect("answer body");
        assert!(state.transcript_lines[body].text.starts_with("   不用启动"));
        assert_eq!(state.transcript_lines[body].tone, TuiTone::White);
        assert!(state.transcript_lines[body - 1].text.is_empty());
        assert!(state.transcript_lines[body + 1].text.is_empty());

        let border = (0..20)
            .flat_map(|y| (0..80).map(move |x| (x, y)))
            .map(|(x, y)| terminal.backend().buffer().cell((x, y)).unwrap())
            .find(|cell| cell.symbol() == "╭")
            .expect("top border");
        assert_eq!(border.fg, CONCLUSION_BORDER);
    }

    #[test]
    fn structured_sections_are_deep_green_while_the_answer_stays_white() {
        let mut state = TuiState::new("mock", 6, 120);
        state.assistant_conclusion_start("结论 Conclusion");
        state.assistant_markdown("答案正文");
        state.assistant_conclusion_end();
        state.assistant_heading("主张", TuiTone::SectionText);
        state.assistant_item("[已证实]", TuiTone::SectionText, "该文档是字幕翻译");
        state.assistant_evidence("source", "证据正文");

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| state.draw(frame)).unwrap();
        let buffer = terminal.backend().buffer();
        let cell_at = |needle: &str| {
            (0..24)
                .flat_map(|y| (0..80).map(move |x| (x, y)))
                .map(|(x, y)| buffer.cell((x, y)).unwrap())
                .find(|cell| cell.symbol() == needle)
                .unwrap_or_else(|| panic!("cell {needle}"))
        };
        let answer = cell_at("答");
        assert_eq!(answer.fg, Color::White);
        let heading = cell_at("主");
        assert_eq!(heading.fg, PANEL_TEXT_GREEN);
        let item = cell_at("该");
        assert_eq!(item.fg, PANEL_TEXT_GREEN);
        let item_row = (0..24)
            .find(|y| (0..80).any(|x| buffer.cell((x, *y)).unwrap().symbol() == "该"))
            .expect("item row");
        let bullet = (0..80)
            .map(|x| buffer.cell((x, item_row)).unwrap())
            .find(|cell| cell.symbol() == "•")
            .map(|cell| cell.fg)
            .expect("item bullet");
        assert_eq!(bullet, PANEL_TEXT_GREEN);
        let evidence = cell_at("据");
        assert_eq!(evidence.fg, PANEL_TEXT_GREEN);
        let marker = cell_at("↳");
        assert_eq!(marker.fg, PANEL_TEXT_GREEN);
    }

    #[test]
    fn activity_card_survives_narrow_and_short_terminals() {
        for (width, height) in [(20, 8), (24, 9), (40, 12), (80, 18), (120, 34)] {
            let backend = TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).unwrap();
            let mut state = TuiState::new("mock", 6, 120);
            state.begin("窄终端测试", CognitiveMode::DirectAnswer, RiskLevel::Low);
            state.set_activity(TuiActivity::bilingual(
                TuiActivityKind::CallingTool,
                "read_file · 一个很长的路径/with/many/segments/README.md",
                3,
            ));
            terminal.draw(|frame| state.draw(frame)).unwrap();
        }
    }

    #[test]
    fn activity_taxonomy_is_bilingual_and_color_coded() {
        let cases = [
            (TuiActivityKind::Reading, "读取", "Reading", Color::Cyan),
            (TuiActivityKind::Writing, "写入", "Writing", Color::Cyan),
            (TuiActivityKind::Updating, "修改", "Updating", Color::Cyan),
            (TuiActivityKind::Bash, "执行命令", "Bash", Color::Cyan),
            (TuiActivityKind::Searching, "搜索", "Searching", Color::Cyan),
            (TuiActivityKind::Fetching, "抓取", "Fetching", Color::Cyan),
            (TuiActivityKind::Querying, "查询", "Querying", Color::Cyan),
            (
                TuiActivityKind::Spawning,
                "子Agent",
                "Subagent",
                Color::Cyan,
            ),
            (
                TuiActivityKind::MasterUnderstanding,
                "任务理解",
                "MissionUnderstanding",
                Color::Cyan,
            ),
            (
                TuiActivityKind::MasterPlanning,
                "任务规划",
                "MissionPlanning",
                Color::Cyan,
            ),
            (
                TuiActivityKind::MasterAssignment,
                "任务分配",
                "TaskAssignment",
                Color::Cyan,
            ),
            (
                TuiActivityKind::MasterStarting,
                "任务启动",
                "TaskStarting",
                Color::Cyan,
            ),
            (
                TuiActivityKind::WorkerRunning,
                "Worker运行",
                "WorkerRunning",
                Color::Cyan,
            ),
            (
                TuiActivityKind::MasterVerifying,
                "任务验收",
                "MissionVerification",
                Color::LightGreen,
            ),
            (TuiActivityKind::Blocked, "阻塞", "Blocked", Color::LightRed),
        ];
        for (kind, title_cn, title_en, _color) in cases {
            let backend = TestBackend::new(100, 20);
            let mut terminal = Terminal::new(backend).unwrap();
            let mut state = TuiState::new("mock", 6, 120);
            state.set_master_activity(
                TuiActivity::bilingual(kind, "detail", 0).with_footer("footer-state"),
            );
            terminal.draw(|frame| state.draw(frame)).unwrap();
            let rows = (0..20)
                .map(|y| {
                    (0..100)
                        .map(|x| terminal.backend().buffer().cell((x, y)).unwrap().symbol())
                        .collect::<String>()
                })
                .collect::<Vec<_>>();
            let top_y = rows
                .iter()
                .position(|row| row.contains('╭') && row.replace(' ', "").contains(title_en))
                .unwrap_or_else(|| panic!("{kind:?}: panel top missing: {rows:?}"));
            let top = &rows[top_y];
            let compact = top.replace(' ', "");
            assert!(compact.contains(title_cn), "{kind:?}: {top}");
            assert!(compact.contains(title_en), "{kind:?}: {top}");
            assert!(top.contains('╭') && top.contains('╮'), "{kind:?}: {top}");
            assert!(top.contains('•'), "{kind:?}: {top}");
            assert_eq!(
                terminal
                    .backend()
                    .buffer()
                    .cell((1, top_y as u16))
                    .unwrap()
                    .fg,
                PANEL_GREEN,
                "{kind:?}"
            );
            assert!(
                (0..100).any(|x| terminal
                    .backend()
                    .buffer()
                    .cell((x, top_y as u16))
                    .unwrap()
                    .fg
                    == PANEL_TEXT_GREEN),
                "{kind:?}: panel text color missing"
            );
            assert!(
                (0..100).all(|x| !terminal
                    .backend()
                    .buffer()
                    .cell((x, top_y as u16))
                    .unwrap()
                    .modifier
                    .contains(Modifier::BOLD)),
                "{kind:?}: title must not be bold"
            );
            let footer_y = rows
                .iter()
                .position(|row| row.contains("footer-state"))
                .unwrap_or_else(|| panic!("{kind:?}: footer missing: {rows:?}"));
            assert!(
                rows.iter()
                    .any(|row| row.contains('╰') && row.contains('╯')),
                "{kind:?}: panel bottom missing: {rows:?}"
            );
            assert!(
                (0..100).all(|x| !terminal
                    .backend()
                    .buffer()
                    .cell((x, footer_y as u16))
                    .unwrap()
                    .modifier
                    .contains(Modifier::BOLD)),
                "{kind:?}: work content must not be bold"
            );
        }
    }

    #[test]
    fn tool_activity_shows_step_and_context_with_minimal_fence() {
        let backend = TestBackend::new(110, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        state.begin("检查文件", CognitiveMode::Execute, RiskLevel::Low);
        state.set_activity(TuiActivity::bilingual(
            TuiActivityKind::CallingTool,
            "read_file · src/main.rs",
            3,
        ));
        state.set_context_budget(10_000);
        state.set_context_tokens(2_500);
        terminal.draw(|frame| state.draw(frame)).unwrap();
        let rows = (0..20)
            .map(|y| {
                (0..110)
                    .map(|x| terminal.backend().buffer().cell((x, y)).unwrap().symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();
        let y = rows
            .iter()
            .position(|row| row.contains('╭') && row.replace(' ', "").contains("调用工具ToolCall"))
            .expect("input panel top");
        let top = &rows[y];
        let compact = top.replace(' ', "");
        assert!(compact.contains("调用工具ToolCall"), "top: {top}");
        assert!(compact.contains("read_file"), "top: {top}");
        assert!(compact.contains("src/main.rs"), "top: {top}");
        assert!(compact.contains("第3步"), "top: {top}");
        assert!(top.contains('╭') && top.contains('╮'), "top: {top}");
        assert!(state.context_bar().contains("────────────────────"));

        let buffer = terminal.backend().buffer();
        let bulb = (0..110)
            .map(|x| buffer.cell((x, y as u16)).unwrap())
            .find(|cell| cell.symbol() == "•")
            .expect("bulb");
        let heading = (0..110)
            .map(|x| buffer.cell((x, y as u16)).unwrap())
            .find(|cell| cell.symbol() == "调")
            .expect("heading");
        assert_eq!(bulb.fg, PANEL_TEXT_GREEN);
        assert_eq!(heading.fg, PANEL_TEXT_GREEN);
        assert!(!heading.modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn default_context_budget_is_one_million_tokens() {
        let mut state = TuiState::new("mock", 6, 120);
        assert!(state.context_bar().contains("/1.0M"));
        state.set_context_tokens(1_234);
        state.clear();
        assert_eq!(state.context_usage_tokens(), 0);
    }

    #[test]
    fn context_meter_colors_used_and_unused_cells_separately() {
        let backend = TestBackend::new(90, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        state.set_activity(TuiActivity::bilingual(
            TuiActivityKind::Thinking,
            "working",
            1,
        ));
        state.set_context_budget(10_000);
        state.set_context_tokens(2_500);
        terminal.draw(|frame| state.draw(frame)).unwrap();

        let buffer = terminal.backend().buffer();
        let metrics_y = (0..20)
            .find(|y| (0..90).any(|x| buffer.cell((x, *y)).unwrap().symbol() == "↑"))
            .expect("folded metrics row");
        let bars: Vec<Color> = (0..90)
            .map(|x| buffer.cell((x, metrics_y)).unwrap())
            .filter(|cell| cell.symbol() == "─")
            .map(|cell| cell.fg)
            .collect();
        assert_eq!(bars.len(), 10, "narrow context meter halves its cells");
        assert_eq!(
            bars.iter()
                .filter(|color| **color == PANEL_TEXT_GREEN)
                .count(),
            3
        );
        assert_eq!(
            bars.iter().filter(|color| **color == Color::Black).count(),
            7
        );
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
    fn interaction_overlays_use_green_surfaces() {
        for color in [MENU_BG, MENU_SELECTED_BG] {
            let Color::Rgb(r, g, b) = color else {
                panic!("overlay colors must be RGB");
            };
            assert!(g > r && g > b, "overlay backgrounds must be green");
        }

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        state.set_last_details("details body");
        state.open_details();
        terminal.draw(|frame| state.draw(frame)).unwrap();
        let buffer = terminal.backend().buffer();
        assert!(
            buffer
                .content
                .iter()
                .all(|cell| cell.symbol() == " " || cell.bg == MENU_BG),
            "details text sits on the green surface"
        );
        for (x, y) in [(0, 0), (79, 23), (40, 10)] {
            assert_eq!(
                terminal.backend().buffer().cell((x, y)).unwrap().bg,
                MENU_BG,
                "details surface at ({x}, {y})"
            );
        }
        let border = buffer
            .content
            .iter()
            .find(|cell| cell.symbol() == "┌")
            .expect("details border");
        assert_eq!(border.fg, Color::LightGreen);

        state.close_details();
        state.toggle_mode_menu();
        terminal.draw(|frame| state.draw(frame)).unwrap();
        let buffer = terminal.backend().buffer();
        assert!(
            buffer
                .content
                .iter()
                .any(|cell| cell.symbol() != " " && cell.bg == MENU_BG),
            "mode menu surface is green"
        );
        assert!(
            buffer
                .content
                .iter()
                .any(|cell| cell.symbol() != " " && cell.bg == MENU_SELECTED_BG),
            "selected mode row uses a green highlight"
        );
    }

    #[test]
    fn status_bar_shows_the_model_mode_and_bold_shortcut_key() {
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("openai-compatible/deepseek-flash", 6, 120);
        state.set_context_budget(1000);
        state.set_context_tokens(250);
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
        assert!(
            !tight.contains("上下文"),
            "context stays on the top border: {row}"
        );
        let key = (0..80)
            .map(|x| terminal.backend().buffer().cell((x, 19)).unwrap())
            .find(|cell| cell.symbol() == "T")
            .expect("T shortcut key");
        assert_eq!(key.fg, Color::LightGreen);
        assert!(key.modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn terminal_backdrop_is_darker_green_than_the_input_panel() {
        let Color::Rgb(r, g, b) = APP_BG else {
            panic!("APP_BG must be RGB");
        };
        let Color::Rgb(pr, pg, pb) = PANEL_BG else {
            panic!("PANEL_BG must be RGB");
        };
        assert!(r < pr && g < pg && b < pb, "backdrop must be darker");
        assert_eq!(
            (r, g, b),
            (pr * 4 / 5, pg * 4 / 5, pb * 4 / 5),
            "backdrop must be 80% of the panel background"
        );

        let backend = TestBackend::new(80, 18);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        state.info("Welcome");
        terminal.draw(|frame| state.draw(frame)).unwrap();
        let buffer = terminal.backend().buffer();
        let panel_top = (0..18)
            .find(|y| (0..80).any(|x| buffer.cell((x, *y)).unwrap().symbol() == "╭"))
            .expect("panel top");
        assert_eq!(
            buffer.cell((0, panel_top - 1)).unwrap().bg,
            APP_BG,
            "the gap above the panel keeps the terminal backdrop"
        );
        assert_eq!(
            buffer.cell((79, 0)).unwrap().bg,
            APP_BG,
            "the transcript area keeps the terminal backdrop"
        );
    }

    #[test]
    fn composer_and_panel_text_share_one_fence() {
        let backend = TestBackend::new(100, 18);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("openai-compatible/deepseek-flash", 6, 120);
        state.info("Welcome");
        terminal.draw(|frame| state.draw(frame)).unwrap();

        let rows = (0..18)
            .map(|y| {
                (0..100)
                    .map(|x| terminal.backend().buffer().cell((x, y)).unwrap().symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();
        let top = rows
            .iter()
            .position(|row| row.contains('╭') && row.contains('•'))
            .expect("top");
        let bottom = rows
            .iter()
            .rposition(|row| row.contains('╰'))
            .expect("bottom");
        assert!(rows[top].contains('•'), "top: {}", rows[top]);
        assert!(rows[top].contains('↑'), "top: {}", rows[top]);
        assert!(
            rows[bottom].contains("deepseek-flash"),
            "bottom: {}",
            rows[bottom]
        );
        assert!(rows[bottom].contains("Ctrl+T"), "bottom: {}", rows[bottom]);

        let metrics_y = rows
            .iter()
            .position(|row| row.contains('╭') && row.contains('↑'))
            .expect("metrics fence");
        let metrics_cell = (0..100)
            .map(|x| {
                terminal
                    .backend()
                    .buffer()
                    .cell((x, metrics_y as u16))
                    .unwrap()
            })
            .find(|cell| cell.symbol() == "↑")
            .expect("metrics text");
        assert_eq!(metrics_cell.fg, PANEL_TEXT_GREEN);
        assert_eq!(
            (0..100)
                .map(|x| {
                    terminal
                        .backend()
                        .buffer()
                        .cell((x, metrics_y as u16))
                        .unwrap()
                })
                .filter(|cell| cell.symbol() == "─" && cell.fg == Color::Black)
                .count(),
            20,
            "wide context meter keeps 20 cells"
        );
        assert_eq!(
            terminal
                .backend()
                .buffer()
                .cell((1, top as u16))
                .unwrap()
                .fg,
            PANEL_GREEN
        );

        let composer = rows
            .iter()
            .position(|row| row.contains('›'))
            .expect("composer");
        assert!(composer > top && composer < bottom, "{rows:?}");
        assert!(rows[composer].starts_with('│'), "{}", rows[composer]);
        assert!(rows[composer].ends_with('│'), "{}", rows[composer]);
        assert_eq!(
            terminal
                .backend()
                .buffer()
                .cell((5, composer as u16))
                .unwrap()
                .bg,
            PANEL_BG
        );
    }

    #[test]
    fn right_click_paste_is_limited_to_the_composer() {
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        state.info("Welcome");
        terminal.draw(|frame| state.draw(frame)).unwrap();

        let composer = state.input_box.expect("composer box");
        assert!(state.input_box_contains(composer.x, composer.y));
        assert!(state.input_box_contains(
            composer.right().saturating_sub(1),
            composer.bottom().saturating_sub(1)
        ));
        assert!(!state.input_box_contains(0, 0));
        assert!(!state.input_box_contains(composer.x, composer.y.saturating_sub(3)));
    }

    #[test]
    fn narrow_terminal_folds_metrics_inside_the_same_fence() {
        let backend = TestBackend::new(72, 18);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("openai-compatible/deepseek-flash", 6, 120);
        state.info("Welcome");
        terminal.draw(|frame| state.draw(frame)).unwrap();

        let rows = (0..18)
            .map(|y| {
                (0..72)
                    .map(|x| terminal.backend().buffer().cell((x, y)).unwrap().symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();
        let top = rows
            .iter()
            .position(|row| row.contains('╭') && row.contains('•'))
            .expect("top");
        let metrics = rows
            .iter()
            .position(|row| row.contains('↑'))
            .expect("folded metrics");
        assert!(!rows[top].contains('↑'), "top: {}", rows[top]);
        assert_eq!(metrics, top + 1, "{rows:?}");
        assert!(rows[metrics].starts_with('│'), "{}", rows[metrics]);
        assert!(rows[metrics].ends_with('│'), "{}", rows[metrics]);
        assert!(
            rows[metrics].replace(' ', "").contains("上下文"),
            "{}",
            rows[metrics]
        );
        assert!(rows[metrics].contains("0s"), "{}", rows[metrics]);
        assert_eq!(
            (0..72)
                .map(|x| terminal
                    .backend()
                    .buffer()
                    .cell((x, metrics as u16))
                    .unwrap())
                .filter(|cell| cell.symbol() == "─")
                .count(),
            10,
            "narrow context meter halves its cells"
        );

        let bottom = rows
            .iter()
            .rposition(|row| row.contains('╰'))
            .expect("bottom");
        let composer = rows
            .iter()
            .position(|row| row.contains('›'))
            .expect("composer");
        assert!(composer > metrics && composer < bottom, "{rows:?}");
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
        let notice = (0..18)
            .flat_map(|y| (0..60).map(move |x| (x, y)))
            .map(|(x, y)| terminal.backend().buffer().cell((x, y)).unwrap())
            .find(|cell| cell.symbol() == "已")
            .expect("notice cell");
        assert_eq!(notice.fg, PANEL_TEXT_GREEN);
        assert!(!notice.modifier.contains(Modifier::BOLD));
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
            colours.contains(&MARK_GREEN),
            "the leading 1 keeps its green"
        );
        assert!(
            colours.contains(&PANEL_TEXT_GREEN),
            "Agent and the captions are deep green"
        );
        assert!(
            colours.contains(&LINK),
            "the domain is the bright-green highlight"
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
    fn welcome_logo_keeps_one_green_and_paints_agent_deep_green() {
        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = TuiState::new("mock", 6, 120);
        terminal.draw(|frame| state.draw(frame)).unwrap();
        let buffer = terminal.backend().buffer();
        let mut colours = Vec::new();
        for y in 0..buffer.area.height {
            let row: String = (0..buffer.area.width)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect();
            if !['╔', '╚', '╝', '╗']
                .iter()
                .any(|glyph| row.contains(*glyph))
            {
                continue;
            }
            for x in 0..buffer.area.width {
                let cell = buffer.cell((x, y)).unwrap();
                if cell.symbol() != " " {
                    colours.push(cell.fg);
                }
            }
        }
        assert!(colours.contains(&MARK_GREEN), "the 1 stays bright green");
        assert!(colours.contains(&PANEL_TEXT_GREEN), "Agent uses deep green");
        assert!(!colours.contains(&LINK), "Agent is no longer blue");
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
            APP_BG
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
    fn command_drawer_uses_bright_green_entries() {
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
            buffer.content.iter().any(|cell| {
                cell.symbol() != " "
                    && cell.fg == Color::LightGreen
                    && cell.modifier.contains(Modifier::BOLD)
            }),
            "command names are bright green and bold"
        );
        assert!(
            buffer.content.iter().any(|cell| {
                cell.symbol() != " "
                    && cell.fg == Color::LightGreen
                    && !cell.modifier.contains(Modifier::BOLD)
            }),
            "command descriptions are bright green"
        );
        assert!(
            buffer
                .content
                .iter()
                .any(|cell| { cell.symbol() != " " && cell.bg == MENU_SELECTED_BG })
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
