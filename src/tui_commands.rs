use crate::tui_host::HostBinding;
use crate::tui_prompt_sources::{PROMPT_SOURCE_GROUPS, PromptSourceGroup};
use crate::tui_prompts::{
    FEATURED_PROMPTS, PROMPT_SUPPLEMENTS, PromptLanguage, PromptSupplement, prompt_score,
    prompt_view,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiCommandScope {
    Core,
    Master,
}

impl TuiCommandScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Master => "master",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandStage {
    Any,
    NoMission,
    Framing,
    Approval,
    Running,
    Verification,
    Delivery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiCommandId {
    Help,
    Status,
    Details,
    Journal,
    Clear,
    Quit,
    Frame,
    Challenge,
    Evidence,
    Audit,
    Decide,
    Report,
    Postmortem,
    Mode,
    Model,
    Permissions,
    Session,
    Workspace,
    Context,
    Preset,
    Tools,
    Export,
    Copy,
    Cost,
    Config,
    Mission,
    Reframe,
    Triage,
    Plan,
    Approve,
    Agents,
    Assign,
    Dispatch,
    Board,
    Watch,
    Attention,
    Respond,
    Control,
    Artifacts,
    Verify,
    Timeline,
}

impl TuiCommandId {
    pub fn uses_command_panel(self) -> bool {
        !matches!(
            self,
            Self::Details
                | Self::Clear
                | Self::Quit
                | Self::Frame
                | Self::Challenge
                | Self::Decide
                | Self::Postmortem
                | Self::Mode
                | Self::Status
                | Self::Journal
                | Self::Evidence
                | Self::Audit
                | Self::Report
                | Self::Tools
                | Self::Cost
                | Self::Copy
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TuiCommandSpec {
    pub id: TuiCommandId,
    pub name: &'static str,
    pub description: &'static str,
    pub scope: TuiCommandScope,
    pub stage: CommandStage,
    pub supports_inline_args: bool,
    pub available_during_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuiInputResolution<'a> {
    Task(&'a str),
    Command { spec: TuiCommandSpec, args: &'a str },
    UnavailableMaster(TuiCommandSpec),
    UnavailableInStage(TuiCommandSpec),
    UnavailableWhileRunning(TuiCommandSpec),
    UnknownCommand(&'a str),
}

pub const COMMANDS: &[TuiCommandSpec] = &[
    command(
        TuiCommandId::Help,
        "/help",
        "显示当前作用域和可用命令",
        TuiCommandScope::Core,
        CommandStage::Any,
        false,
        true,
    ),
    command(
        TuiCommandId::Status,
        "/status",
        "显示运行状态、宿主与预算",
        TuiCommandScope::Core,
        CommandStage::Any,
        false,
        true,
    ),
    command(
        TuiCommandId::Details,
        "/details",
        "打开最近一次运行详情面板",
        TuiCommandScope::Core,
        CommandStage::Any,
        false,
        false,
    ),
    command(
        TuiCommandId::Journal,
        "/journal",
        "显示当前运行日志路径",
        TuiCommandScope::Core,
        CommandStage::Any,
        false,
        true,
    ),
    command(
        TuiCommandId::Clear,
        "/clear",
        "清空显示，不删除会话或日志",
        TuiCommandScope::Core,
        CommandStage::Any,
        false,
        false,
    ),
    command(
        TuiCommandId::Quit,
        "/quit",
        "退出 1-Agent 并保留状态",
        TuiCommandScope::Core,
        CommandStage::Any,
        false,
        true,
    ),
    command(
        TuiCommandId::Frame,
        "/frame",
        "整理事实、假设、约束、未知和验收标准",
        TuiCommandScope::Core,
        CommandStage::Any,
        true,
        false,
    ),
    command(
        TuiCommandId::Challenge,
        "/challenge",
        "红队审查计划、结论、假设和证据",
        TuiCommandScope::Core,
        CommandStage::Any,
        true,
        false,
    ),
    command(
        TuiCommandId::Evidence,
        "/evidence",
        "查看 claim、evidence、unknown 和 conflict",
        TuiCommandScope::Core,
        CommandStage::Any,
        true,
        false,
    ),
    command(
        TuiCommandId::Audit,
        "/audit",
        "审计 run 或 Master journal",
        TuiCommandScope::Core,
        CommandStage::Any,
        true,
        false,
    ),
    command(
        TuiCommandId::Decide,
        "/decide",
        "形成证据绑定的决策记录",
        TuiCommandScope::Core,
        CommandStage::Any,
        true,
        false,
    ),
    command(
        TuiCommandId::Report,
        "/report",
        "输出任务或生产线报告",
        TuiCommandScope::Core,
        CommandStage::Any,
        true,
        false,
    ),
    command(
        TuiCommandId::Postmortem,
        "/postmortem",
        "分析失败根因和预防措施",
        TuiCommandScope::Core,
        CommandStage::Any,
        true,
        false,
    ),
    command(
        TuiCommandId::Mode,
        "/mode",
        "切换自动、直接回答、证据研究、决策分析、执行或诊断模式",
        TuiCommandScope::Core,
        CommandStage::Any,
        false,
        false,
    ),
    command(
        TuiCommandId::Model,
        "/model",
        "查看和选择下一次运行的模型路由",
        TuiCommandScope::Core,
        CommandStage::Any,
        true,
        false,
    ),
    command(
        TuiCommandId::Permissions,
        "/permissions",
        "查看和调整下一次运行的权限",
        TuiCommandScope::Core,
        CommandStage::Any,
        true,
        false,
    ),
    command(
        TuiCommandId::Session,
        "/session",
        "管理会话生命周期",
        TuiCommandScope::Core,
        CommandStage::Any,
        true,
        false,
    ),
    command(
        TuiCommandId::Workspace,
        "/workspace",
        "管理工作目录、文件、diff 和 worktree",
        TuiCommandScope::Core,
        CommandStage::Any,
        true,
        false,
    ),
    command(
        TuiCommandId::Context,
        "/context",
        "压缩上下文或生成摘要",
        TuiCommandScope::Core,
        CommandStage::Any,
        true,
        false,
    ),
    command(
        TuiCommandId::Preset,
        "/preset",
        "管理可复用任务模板",
        TuiCommandScope::Core,
        CommandStage::Any,
        true,
        false,
    ),
    command(
        TuiCommandId::Tools,
        "/tools",
        "查看 ToolRegistry、schema、权限和风险",
        TuiCommandScope::Core,
        CommandStage::Any,
        true,
        false,
    ),
    command(
        TuiCommandId::Export,
        "/export",
        "导出会话、报告或证据",
        TuiCommandScope::Core,
        CommandStage::Any,
        true,
        false,
    ),
    command(
        TuiCommandId::Copy,
        "/copy",
        "复制最近回答",
        TuiCommandScope::Core,
        CommandStage::Any,
        false,
        false,
    ),
    command(
        TuiCommandId::Cost,
        "/cost",
        "汇总 provider token usage 与配置价格成本",
        TuiCommandScope::Core,
        CommandStage::Any,
        false,
        false,
    ),
    command(
        TuiCommandId::Config,
        "/config",
        "查看和修改 TUI 配置",
        TuiCommandScope::Core,
        CommandStage::Any,
        true,
        false,
    ),
    command(
        TuiCommandId::Reframe,
        "/reframe",
        "判断请求是目的还是手段，必要时追问一次",
        TuiCommandScope::Core,
        CommandStage::Any,
        true,
        false,
    ),
    command(
        TuiCommandId::Mission,
        "/mission",
        "管理生产线使命",
        TuiCommandScope::Master,
        CommandStage::Any,
        true,
        false,
    ),
    command(
        TuiCommandId::Triage,
        "/triage",
        "确定任务路线和审批需求",
        TuiCommandScope::Master,
        CommandStage::Framing,
        false,
        false,
    ),
    command(
        TuiCommandId::Plan,
        "/plan",
        "生成和检查 Work Item DAG",
        TuiCommandScope::Master,
        CommandStage::Framing,
        true,
        false,
    ),
    command(
        TuiCommandId::Approve,
        "/approve",
        "人工批准生产线计划",
        TuiCommandScope::Master,
        CommandStage::Approval,
        false,
        false,
    ),
    command(
        TuiCommandId::Agents,
        "/agents",
        "查看 Agent 能力和 readiness",
        TuiCommandScope::Master,
        CommandStage::Any,
        true,
        true,
    ),
    command(
        TuiCommandId::Assign,
        "/assign",
        "查看或应用 Agent 分配",
        TuiCommandScope::Master,
        CommandStage::Approval,
        true,
        false,
    ),
    command(
        TuiCommandId::Dispatch,
        "/dispatch",
        "创建租约并派发任务",
        TuiCommandScope::Master,
        CommandStage::Approval,
        true,
        false,
    ),
    command(
        TuiCommandId::Board,
        "/board",
        "查看 Work Item 生产线总览",
        TuiCommandScope::Master,
        CommandStage::Running,
        true,
        true,
    ),
    command(
        TuiCommandId::Watch,
        "/watch",
        "轮询和推进运行状态",
        TuiCommandScope::Master,
        CommandStage::Running,
        true,
        true,
    ),
    command(
        TuiCommandId::Attention,
        "/attention",
        "只显示需要人工处理的事项",
        TuiCommandScope::Master,
        CommandStage::Running,
        false,
        true,
    ),
    command(
        TuiCommandId::Respond,
        "/respond",
        "向指定任务路由人工回复",
        TuiCommandScope::Master,
        CommandStage::Running,
        true,
        true,
    ),
    command(
        TuiCommandId::Control,
        "/control",
        "暂停、恢复、取消、接管、重试或重规划",
        TuiCommandScope::Master,
        CommandStage::Running,
        true,
        true,
    ),
    command(
        TuiCommandId::Artifacts,
        "/artifacts",
        "检查预期和实际 Artifact",
        TuiCommandScope::Master,
        CommandStage::Running,
        true,
        true,
    ),
    command(
        TuiCommandId::Verify,
        "/verify",
        "执行独立质检验收",
        TuiCommandScope::Master,
        CommandStage::Running,
        true,
        true,
    ),
    command(
        TuiCommandId::Timeline,
        "/timeline",
        "查看 Master 和 Workshop 时间线",
        TuiCommandScope::Master,
        CommandStage::Running,
        false,
        true,
    ),
];

const fn command(
    id: TuiCommandId,
    name: &'static str,
    description: &'static str,
    scope: TuiCommandScope,
    stage: CommandStage,
    supports_inline_args: bool,
    available_during_run: bool,
) -> TuiCommandSpec {
    TuiCommandSpec {
        id,
        name,
        description,
        scope,
        stage,
        supports_inline_args,
        available_during_run,
    }
}

pub fn resolve_input<'a>(
    input: &'a str,
    host: &HostBinding,
    stage: CommandStage,
    running: bool,
) -> TuiInputResolution<'a> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return TuiInputResolution::Task(trimmed);
    }

    let (name, args) = trimmed
        .split_once(char::is_whitespace)
        .map_or((trimmed, ""), |(name, args)| (name, args.trim()));
    let canonical_name = if name == "/exit" { "/quit" } else { name };
    let Some(spec) = COMMANDS
        .iter()
        .copied()
        .find(|spec| spec.name == canonical_name)
    else {
        return TuiInputResolution::UnknownCommand(name);
    };

    if spec.scope == TuiCommandScope::Master && !host.is_workshop_master() {
        return TuiInputResolution::UnavailableMaster(spec);
    }
    if spec.scope == TuiCommandScope::Master
        && spec.stage != CommandStage::Any
        && spec.stage != stage
    {
        return TuiInputResolution::UnavailableInStage(spec);
    }
    if running && !spec.available_during_run {
        return TuiInputResolution::UnavailableWhileRunning(spec);
    }
    TuiInputResolution::Command { spec, args }
}

pub fn commands_for_surface(
    host: &HostBinding,
    stage: CommandStage,
    running: bool,
) -> Vec<TuiCommandSpec> {
    COMMANDS
        .iter()
        .copied()
        .filter(|spec| spec.scope == TuiCommandScope::Core || host.is_workshop_master())
        .filter(|spec| {
            spec.scope == TuiCommandScope::Core
                || spec.stage == CommandStage::Any
                || spec.stage == stage
        })
        .filter(|spec| !running || spec.available_during_run)
        .collect()
}

/// One drawer row: an available command or a command that exists for this
/// scope but is gated by stage or a running task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandEntry {
    pub spec: TuiCommandSpec,
    pub unavailable: Option<&'static str>,
}

/// A drawer row is either a real command, a curated prompt card, or a
/// complete source-table group. Both prompt kinds insert text into the
/// composer instead of executing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawerEntry {
    Command(CommandEntry),
    Prompt(&'static PromptSupplement),
    Source(&'static PromptSourceGroup),
}

impl DrawerEntry {
    pub fn command(self) -> Option<CommandEntry> {
        match self {
            Self::Command(entry) => Some(entry),
            Self::Prompt(_) | Self::Source(_) => None,
        }
    }

    pub fn prompt(self) -> Option<&'static PromptSupplement> {
        match self {
            Self::Command(_) | Self::Source(_) => None,
            Self::Prompt(prompt) => Some(prompt),
        }
    }

    pub fn source(self) -> Option<&'static PromptSourceGroup> {
        match self {
            Self::Command(_) | Self::Prompt(_) => None,
            Self::Source(source) => Some(source),
        }
    }
}

/// Drawer sections, in display order. Recent commands form their own section
/// and are removed from the category they would otherwise appear in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandGroup {
    Session,
    Reasoning,
    Evidence,
    Model,
    Master,
}

impl CommandGroup {
    pub const ORDER: [Self; 5] = [
        Self::Session,
        Self::Reasoning,
        Self::Evidence,
        Self::Model,
        Self::Master,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Session => "会话",
            Self::Reasoning => "思考",
            Self::Evidence => "证据",
            Self::Model => "模型与权限",
            Self::Master => "总控",
        }
    }
}

pub fn command_group(id: TuiCommandId) -> CommandGroup {
    match id {
        TuiCommandId::Help
        | TuiCommandId::Status
        | TuiCommandId::Details
        | TuiCommandId::Journal
        | TuiCommandId::Clear
        | TuiCommandId::Quit
        | TuiCommandId::Session
        | TuiCommandId::Workspace
        | TuiCommandId::Context
        | TuiCommandId::Preset
        | TuiCommandId::Export
        | TuiCommandId::Copy
        | TuiCommandId::Cost
        | TuiCommandId::Config => CommandGroup::Session,
        TuiCommandId::Frame
        | TuiCommandId::Challenge
        | TuiCommandId::Decide
        | TuiCommandId::Postmortem
        | TuiCommandId::Reframe => CommandGroup::Reasoning,
        TuiCommandId::Evidence | TuiCommandId::Audit | TuiCommandId::Report => {
            CommandGroup::Evidence
        }
        TuiCommandId::Mode
        | TuiCommandId::Model
        | TuiCommandId::Permissions
        | TuiCommandId::Tools => CommandGroup::Model,
        TuiCommandId::Mission
        | TuiCommandId::Triage
        | TuiCommandId::Plan
        | TuiCommandId::Approve
        | TuiCommandId::Agents
        | TuiCommandId::Assign
        | TuiCommandId::Dispatch
        | TuiCommandId::Board
        | TuiCommandId::Watch
        | TuiCommandId::Attention
        | TuiCommandId::Respond
        | TuiCommandId::Control
        | TuiCommandId::Artifacts
        | TuiCommandId::Verify
        | TuiCommandId::Timeline => CommandGroup::Master,
    }
}

/// A command row plus the section heading that precedes it, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandDrawerRow {
    pub heading: Option<&'static str>,
    pub entry: DrawerEntry,
}

fn stage_label(stage: CommandStage) -> &'static str {
    match stage {
        CommandStage::Any => "任意",
        CommandStage::NoMission => "无使命",
        CommandStage::Framing => "Framing",
        CommandStage::Approval => "Approval",
        CommandStage::Running => "Running",
        CommandStage::Verification => "Verification",
        CommandStage::Delivery => "Delivery",
    }
}

fn command_score(spec: &TuiCommandSpec, query: &str) -> Option<u8> {
    let query = query.trim_start_matches('/').to_ascii_lowercase();
    if query.is_empty() {
        return Some(0);
    }
    let name = spec.name.trim_start_matches('/').to_ascii_lowercase();
    if name == query {
        Some(0)
    } else if name.starts_with(&query) {
        Some(1)
    } else if name.contains(&query) {
        Some(2)
    } else if (query.chars().count() >= 3 || !query.is_ascii())
        && spec.description.to_lowercase().contains(&query)
    {
        Some(3)
    } else {
        None
    }
}

/// Drawer rows including disabled stage/running commands. Master commands stay
/// hidden outside a Workshop master context.
pub fn command_entries(
    prefix: &str,
    host: &HostBinding,
    stage: CommandStage,
    running: bool,
) -> Vec<CommandEntry> {
    if !prefix.starts_with('/') || prefix.chars().any(char::is_whitespace) {
        return Vec::new();
    }
    let mut entries = COMMANDS
        .iter()
        .copied()
        .filter(|spec| spec.scope == TuiCommandScope::Core || host.is_workshop_master())
        .filter_map(|spec| {
            let score = command_score(&spec, prefix)?;
            let unavailable = if spec.scope == TuiCommandScope::Master
                && spec.stage != CommandStage::Any
                && spec.stage != stage
            {
                Some(stage_label(spec.stage))
            } else if running && !spec.available_during_run {
                Some("运行中不可用")
            } else {
                None
            };
            Some((score, CommandEntry { spec, unavailable }))
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|(score, _)| *score);
    entries.into_iter().map(|(_, entry)| entry).collect()
}

fn prompt_entries(prefix: &str, language: PromptLanguage) -> Vec<&'static PromptSupplement> {
    let available = |prompt: &&'static PromptSupplement| prompt_view(prompt, language).is_some();
    if prefix.trim_start_matches('/').is_empty() {
        if language == PromptLanguage::En {
            return PROMPT_SUPPLEMENTS.iter().filter(available).collect();
        }
        return FEATURED_PROMPTS
            .iter()
            .filter_map(|id| PROMPT_SUPPLEMENTS.iter().find(|prompt| prompt.id == *id))
            .filter(available)
            .collect();
    }
    let mut matches = PROMPT_SUPPLEMENTS
        .iter()
        .filter(available)
        .filter_map(|prompt| prompt_score(prompt, prefix).map(|score| (score, prompt)))
        .collect::<Vec<_>>();
    matches.sort_by_key(|(score, _)| *score);
    matches.into_iter().map(|(_, prompt)| prompt).collect()
}

fn source_score(source: &PromptSourceGroup, query: &str) -> Option<u8> {
    let query = query.trim_start_matches('/').to_lowercase();
    if query.is_empty() {
        return Some(0);
    }
    let topic = source.topic.to_lowercase();
    if topic == query {
        return Some(0);
    }
    if topic.starts_with(&query) {
        return Some(1);
    }
    if topic.contains(&query) {
        return Some(2);
    }
    let metadata_search = !query.is_ascii() || query.len() >= 3;
    if metadata_search
        && (source.author.to_lowercase().contains(&query)
            || source.category.to_lowercase().contains(&query)
            || source.original.to_lowercase().contains(&query)
            || source.translation.to_lowercase().contains(&query)
            || source.source_doc.to_lowercase().contains(&query))
    {
        Some(3)
    } else {
        None
    }
}

/// Source-table rows used by `//` and `//keyword`. Each row is one complete
/// (author, topic) group; labels and categories come from the table.
pub fn prompt_drawer_rows(prefix: &str, _language: PromptLanguage) -> Vec<CommandDrawerRow> {
    let query = prefix.trim();
    if query.is_empty() {
        let mut rows = Vec::new();
        let mut last_category = None;
        for source in PROMPT_SOURCE_GROUPS {
            let heading = if rows.is_empty() {
                Some("原文库 · 完整原文")
            } else if last_category != Some(source.category) {
                Some(source.category)
            } else {
                None
            };
            last_category = Some(source.category);
            rows.push(CommandDrawerRow {
                heading,
                entry: DrawerEntry::Source(source),
            });
        }
        return rows;
    }
    let mut matches = PROMPT_SOURCE_GROUPS
        .iter()
        .filter_map(|source| source_score(source, query).map(|score| (score, source)))
        .collect::<Vec<_>>();
    matches.sort_by_key(|(score, _)| *score);
    matches
        .into_iter()
        .map(|(_, source)| CommandDrawerRow {
            heading: None,
            entry: DrawerEntry::Source(source),
        })
        .collect()
}

/// Drawer rows for the empty `/` query: recent commands first, then category
/// sections, then a small prompt-supplement set. A non-empty query keeps the
/// flat command order and adds matching prompts under their own heading.
pub fn drawer_rows(
    prefix: &str,
    host: &HostBinding,
    stage: CommandStage,
    running: bool,
    recent: &[TuiCommandId],
    language: PromptLanguage,
) -> Vec<CommandDrawerRow> {
    let entries = command_entries(prefix, host, stage, running);
    let prompts = prompt_entries(prefix, language);
    if prefix.trim() != "/" {
        let best_command = entries
            .iter()
            .filter_map(|entry| command_score(&entry.spec, prefix))
            .min();
        let best_prompt = prompts
            .iter()
            .filter_map(|prompt| prompt_score(prompt, prefix))
            .min();
        let prompts_first = match (best_command, best_prompt) {
            (None, Some(_)) => true,
            (Some(command), Some(prompt)) => prompt < command,
            _ => false,
        };
        let mut rows = Vec::new();
        if prompts_first {
            for (index, prompt) in prompts.iter().copied().enumerate() {
                rows.push(CommandDrawerRow {
                    heading: (index == 0).then_some("提示词补充"),
                    entry: DrawerEntry::Prompt(prompt),
                });
            }
            for entry in entries {
                rows.push(CommandDrawerRow {
                    heading: None,
                    entry: DrawerEntry::Command(entry),
                });
            }
        } else {
            for entry in entries {
                rows.push(CommandDrawerRow {
                    heading: None,
                    entry: DrawerEntry::Command(entry),
                });
            }
            for (index, prompt) in prompts.iter().copied().enumerate() {
                rows.push(CommandDrawerRow {
                    heading: (index == 0).then_some("提示词补充"),
                    entry: DrawerEntry::Prompt(prompt),
                });
            }
        }
        return rows;
    }

    let mut used = vec![false; entries.len()];
    let mut rows = Vec::new();
    let prompt_limit = if language == PromptLanguage::En {
        prompts.len()
    } else {
        3
    };
    for (index, prompt) in prompts.iter().take(prompt_limit).copied().enumerate() {
        rows.push(CommandDrawerRow {
            heading: (index == 0).then_some("提示词补充 · 输入关键词 · // 原文库"),
            entry: DrawerEntry::Prompt(prompt),
        });
    }
    let mut recent_entries = Vec::new();
    for id in recent.iter().take(5) {
        let Some((index, entry)) = entries
            .iter()
            .enumerate()
            .find(|(index, entry)| !used[*index] && entry.spec.id == *id)
        else {
            continue;
        };
        used[index] = true;
        recent_entries.push(*entry);
        if recent_entries.len() == 3 {
            break;
        }
    }
    for (index, entry) in recent_entries.into_iter().enumerate() {
        rows.push(CommandDrawerRow {
            heading: (index == 0).then_some("最近"),
            entry: DrawerEntry::Command(entry),
        });
    }

    for group in CommandGroup::ORDER {
        let mut first = true;
        for (index, entry) in entries.iter().enumerate() {
            if used[index] || command_group(entry.spec.id) != group {
                continue;
            }
            rows.push(CommandDrawerRow {
                heading: first.then_some(group.label()),
                entry: DrawerEntry::Command(*entry),
            });
            first = false;
        }
    }

    rows
}

/// Commands that need a second Enter because they discard work or change a
/// running master's task state.
pub fn is_destructive_input(input: &str) -> bool {
    let (name, args) = input
        .trim()
        .split_once(char::is_whitespace)
        .map_or((input.trim(), ""), |(name, args)| (name, args.trim()));
    match name {
        "/clear" => true,
        "/dispatch" => args.split_whitespace().next() != Some("dry-run"),
        "/control" => matches!(
            args.split_whitespace().next(),
            Some("cancel") | Some("takeover")
        ),
        _ => false,
    }
}

/// Available commands only, used by completion and existing callers.
pub fn visible_commands(
    prefix: &str,
    host: &HostBinding,
    stage: CommandStage,
    running: bool,
) -> Vec<TuiCommandSpec> {
    command_entries(prefix, host, stage, running)
        .into_iter()
        .filter(|entry| entry.unavailable.is_none())
        .map(|entry| entry.spec)
        .collect()
}

pub fn command_usage(id: TuiCommandId) -> Option<&'static str> {
    match id {
        TuiCommandId::Frame => Some("/frame <请求或当前任务>"),
        TuiCommandId::Challenge => Some("/challenge [计划或结论]"),
        TuiCommandId::Evidence => Some("/evidence [run|master]"),
        TuiCommandId::Audit => Some("/audit [run|master]"),
        TuiCommandId::Decide => Some("/decide <决策主题>"),
        TuiCommandId::Report => Some("/report [task|production]"),
        TuiCommandId::Postmortem => Some("/postmortem <失败任务或 run>"),
        TuiCommandId::Model => Some("/model [show|<provider>/<model>]"),
        TuiCommandId::Permissions => Some("/permissions [show|set …]"),
        TuiCommandId::Session => Some("/session [list|new|resume|…]"),
        TuiCommandId::Workspace => Some("/workspace [set|diff|files|worktree]"),
        TuiCommandId::Context => Some("/context [show|compact|summarize]"),
        TuiCommandId::Preset => Some("/preset [list|save|load|delete]"),
        TuiCommandId::Tools => Some("/tools [show|…]"),
        TuiCommandId::Export => Some("/export [session|report|evidence]"),
        TuiCommandId::Config => Some("/config [show|set …]"),
        TuiCommandId::Reframe => Some("/reframe <请求>"),
        TuiCommandId::Mission => Some("/mission [show|set|…]"),
        TuiCommandId::Plan => Some("/plan [show|revise]"),
        TuiCommandId::Agents => Some("/agents [list|refresh]"),
        TuiCommandId::Assign => Some("/assign [show|apply]"),
        TuiCommandId::Dispatch => Some("/dispatch [dry-run|start]"),
        TuiCommandId::Board => Some("/board [list|detail]"),
        TuiCommandId::Watch => Some("/watch [once|loop]"),
        TuiCommandId::Respond => Some("/respond <task> <回复>"),
        TuiCommandId::Control => Some("/control [pause|resume|cancel|takeover|retry|replan]"),
        TuiCommandId::Artifacts => Some("/artifacts <task>"),
        TuiCommandId::Verify => Some("/verify <task> <验收文件>"),
        _ => None,
    }
}

/// One selectable next step shown at the end of a turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NextAction {
    pub id: TuiCommandId,
    pub name: &'static str,
    pub label: &'static str,
    pub default: bool,
}

/// A menu longer than this stops being a choice and becomes a list.
pub const MAX_NEXT_ACTIONS: usize = 4;

/// Which commands are worth offering at the end of a turn, by stage.
///
/// This is navigation, not advice: the order is the preference order and the
/// first entry becomes the default. Host and stage filtering still happen in
/// [`next_actions`], so a standalone session never sees a Master command.
pub fn stage_next_actions(stage: CommandStage) -> &'static [TuiCommandId] {
    match stage {
        CommandStage::Any => &[
            TuiCommandId::Status,
            TuiCommandId::Context,
            TuiCommandId::Tools,
        ],
        CommandStage::NoMission => &[
            TuiCommandId::Status,
            TuiCommandId::Evidence,
            TuiCommandId::Frame,
        ],
        CommandStage::Framing => &[
            TuiCommandId::Decide,
            TuiCommandId::Evidence,
            TuiCommandId::Report,
        ],
        CommandStage::Approval => &[
            TuiCommandId::Approve,
            TuiCommandId::Plan,
            TuiCommandId::Board,
        ],
        CommandStage::Running => &[
            TuiCommandId::Board,
            TuiCommandId::Watch,
            TuiCommandId::Attention,
        ],
        CommandStage::Verification => &[
            TuiCommandId::Artifacts,
            TuiCommandId::Verify,
            TuiCommandId::Report,
        ],
        CommandStage::Delivery => &[
            TuiCommandId::Export,
            TuiCommandId::Report,
            TuiCommandId::Cost,
        ],
    }
}

/// Builds the end-of-turn menu.
///
/// `candidates` names the actions the turn actually leaves open. Anything not
/// available for this host and stage, or not implemented, is dropped rather
/// than shown, so a menu can never promise a command that would fail. An empty
/// result means "no fork": the caller must not render a menu at all.
///
/// The model may only choose from this list; it never authors an entry. That is
/// what keeps "next step" options honest.
pub fn next_actions(
    host: &HostBinding,
    stage: CommandStage,
    candidates: &[TuiCommandId],
    default: Option<TuiCommandId>,
) -> Vec<NextAction> {
    let available = commands_for_surface(host, stage, false);

    let mut chosen: Vec<TuiCommandSpec> = Vec::new();
    for id in candidates {
        if chosen.iter().any(|spec| spec.id == *id) {
            continue;
        }
        if let Some(spec) = available.iter().find(|spec| spec.id == *id) {
            chosen.push(*spec);
        }
        if chosen.len() == MAX_NEXT_ACTIONS {
            break;
        }
    }
    if chosen.is_empty() {
        return Vec::new();
    }

    let default_id = default
        .filter(|id| chosen.iter().any(|spec| spec.id == *id))
        .unwrap_or(chosen[0].id);

    let mut actions = Vec::with_capacity(chosen.len());
    if let Some(spec) = chosen.iter().find(|spec| spec.id == default_id) {
        actions.push(NextAction {
            id: spec.id,
            name: spec.name,
            label: spec.description,
            default: true,
        });
    }
    for spec in &chosen {
        if spec.id != default_id {
            actions.push(NextAction {
                id: spec.id,
                name: spec.name,
                label: spec.description,
                default: false,
            });
        }
    }
    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui_host::{HostBinding, HostKind};

    fn host(kind: HostKind) -> HostBinding {
        match kind {
            HostKind::Standalone => HostBinding::standalone(),
            HostKind::WorkshopMaster => HostBinding::workshop_context(
                "master",
                "main-coordinator",
                "1agent",
                "term-1790000000000-1234-1",
            ),
            HostKind::WorkshopWorker => HostBinding::workshop_context(
                "worker",
                "release-verifier",
                "codex",
                "term-1790000000000-1234-2",
            ),
        }
    }

    #[test]
    fn standalone_surface_never_contains_master_commands() {
        let commands =
            commands_for_surface(&host(HostKind::Standalone), CommandStage::Running, false);
        assert!(
            commands
                .iter()
                .all(|command| command.scope == TuiCommandScope::Core)
        );
    }

    #[test]
    fn master_surface_is_stage_aware() {
        let commands = commands_for_surface(
            &host(HostKind::WorkshopMaster),
            CommandStage::Framing,
            false,
        );
        assert!(
            commands
                .iter()
                .any(|command| command.id == TuiCommandId::Plan)
        );
        assert!(
            !commands
                .iter()
                .any(|command| command.id == TuiCommandId::Dispatch)
        );
    }

    #[test]
    fn worker_surface_never_contains_master_commands() {
        let commands = commands_for_surface(
            &host(HostKind::WorkshopWorker),
            CommandStage::Approval,
            false,
        );
        assert!(
            commands
                .iter()
                .all(|command| command.scope == TuiCommandScope::Core)
        );
    }

    #[test]
    fn hidden_master_command_is_rejected_even_when_typed() {
        assert!(matches!(
            resolve_input(
                "/dispatch",
                &host(HostKind::Standalone),
                CommandStage::Any,
                false
            ),
            TuiInputResolution::UnavailableMaster(_)
        ));
        assert!(matches!(
            resolve_input(
                "/dispatch",
                &host(HostKind::WorkshopWorker),
                CommandStage::Any,
                false
            ),
            TuiInputResolution::UnavailableMaster(_)
        ));
    }

    #[test]
    fn phase_four_supervision_commands_are_master_only_and_implemented() {
        let ids = [
            TuiCommandId::Board,
            TuiCommandId::Watch,
            TuiCommandId::Attention,
            TuiCommandId::Respond,
            TuiCommandId::Control,
            TuiCommandId::Timeline,
        ];
        let master = commands_for_surface(
            &host(HostKind::WorkshopMaster),
            CommandStage::Running,
            false,
        );
        for id in ids {
            let command = master
                .iter()
                .find(|command| command.id == id)
                .expect("supervision command should be visible in Master mode");
            assert_eq!(command.scope, TuiCommandScope::Master);
            assert!(matches!(
                resolve_input(
                    command.name,
                    &host(HostKind::WorkshopMaster),
                    CommandStage::Running,
                    true,
                ),
                TuiInputResolution::Command { spec, .. } if spec.id == id
            ));
            assert!(matches!(
                resolve_input(
                    command.name,
                    &host(HostKind::Standalone),
                    CommandStage::Running,
                    false,
                ),
                TuiInputResolution::UnavailableMaster(spec) if spec.id == id
            ));
        }
    }

    #[test]
    fn phase_five_verification_commands_are_master_only_and_implemented() {
        let master = commands_for_surface(
            &host(HostKind::WorkshopMaster),
            CommandStage::Running,
            false,
        );
        for id in [TuiCommandId::Artifacts, TuiCommandId::Verify] {
            let command = master
                .iter()
                .find(|command| command.id == id)
                .expect("verification command should be visible in Master mode");
            assert_eq!(command.stage, CommandStage::Running);
            assert!(matches!(
                resolve_input(
                    command.name,
                    &host(HostKind::Standalone),
                    CommandStage::Running,
                    false,
                ),
                TuiInputResolution::UnavailableMaster(spec) if spec.id == id
            ));
        }
    }

    #[test]
    fn read_only_output_commands_stay_in_the_transcript() {
        for id in [
            TuiCommandId::Status,
            TuiCommandId::Journal,
            TuiCommandId::Evidence,
            TuiCommandId::Audit,
            TuiCommandId::Report,
            TuiCommandId::Tools,
            TuiCommandId::Cost,
            TuiCommandId::Copy,
        ] {
            assert!(!id.uses_command_panel(), "{id:?} should be inline");
        }
        assert!(TuiCommandId::Workspace.uses_command_panel());
        assert!(TuiCommandId::Model.uses_command_panel());
    }

    #[test]
    fn phase_six_general_commands_are_implemented_core_commands() {
        for id in [
            TuiCommandId::Session,
            TuiCommandId::Workspace,
            TuiCommandId::Context,
            TuiCommandId::Preset,
            TuiCommandId::Cost,
            TuiCommandId::Config,
            TuiCommandId::Export,
            TuiCommandId::Copy,
        ] {
            let command = COMMANDS
                .iter()
                .find(|command| command.id == id)
                .expect("phase six command should be registered");
            assert_eq!(command.scope, TuiCommandScope::Core);
        }
    }

    #[test]
    fn command_counts_follow_scope_and_lifecycle_stage() {
        let standalone =
            commands_for_surface(&host(HostKind::Standalone), CommandStage::NoMission, false);
        assert_eq!(standalone.len(), 26);
        let master = host(HostKind::WorkshopMaster);
        assert_eq!(
            commands_for_surface(&master, CommandStage::NoMission, false).len(),
            28
        );
        assert_eq!(
            commands_for_surface(&master, CommandStage::Framing, false).len(),
            30
        );
        assert_eq!(
            commands_for_surface(&master, CommandStage::Approval, false).len(),
            31
        );
        assert_eq!(
            commands_for_surface(&master, CommandStage::Running, false).len(),
            36
        );
    }

    #[test]
    fn master_drawer_contains_stage_command_and_standalone_does_not() {
        let commands = visible_commands(
            "/",
            &host(HostKind::WorkshopMaster),
            CommandStage::NoMission,
            false,
        );
        assert_eq!(
            commands
                .iter()
                .map(|command| command.name)
                .collect::<Vec<_>>(),
            vec![
                "/help",
                "/status",
                "/details",
                "/journal",
                "/clear",
                "/quit",
                "/frame",
                "/challenge",
                "/evidence",
                "/audit",
                "/decide",
                "/report",
                "/postmortem",
                "/mode",
                "/model",
                "/permissions",
                "/session",
                "/workspace",
                "/context",
                "/preset",
                "/tools",
                "/export",
                "/copy",
                "/cost",
                "/config",
                "/reframe",
                "/mission",
                "/agents"
            ]
        );
        let standalone = visible_commands(
            "/",
            &host(HostKind::Standalone),
            CommandStage::NoMission,
            false,
        );
        assert!(
            standalone
                .iter()
                .all(|command| command.scope == TuiCommandScope::Core)
        );
    }
}

#[cfg(test)]
mod next_action_tests {
    use super::*;
    use crate::tui_host::HostKind;

    fn host(kind: HostKind) -> HostBinding {
        match kind {
            HostKind::Standalone => HostBinding::standalone(),
            HostKind::WorkshopMaster => HostBinding::workshop_context(
                "master",
                "main-coordinator",
                "1agent",
                "term-1790000000000-1234-1",
            ),
            HostKind::WorkshopWorker => HostBinding::workshop_context(
                "worker",
                "release-verifier",
                "codex",
                "term-1790000000000-1234-2",
            ),
        }
    }

    #[test]
    fn no_candidates_means_no_menu() {
        let actions = next_actions(&host(HostKind::Standalone), CommandStage::Any, &[], None);
        assert!(actions.is_empty());
    }

    #[test]
    fn master_commands_are_dropped_for_standalone() {
        let actions = next_actions(
            &host(HostKind::Standalone),
            CommandStage::Running,
            &[TuiCommandId::Board],
            None,
        );
        assert!(actions.is_empty());

        let actions = next_actions(
            &host(HostKind::WorkshopMaster),
            CommandStage::Running,
            &[TuiCommandId::Board],
            None,
        );
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].id, TuiCommandId::Board);
    }

    #[test]
    fn stage_filter_applies() {
        // /board only exists while work is running.
        let actions = next_actions(
            &host(HostKind::WorkshopMaster),
            CommandStage::Framing,
            &[TuiCommandId::Board],
            None,
        );
        assert!(actions.is_empty());
    }

    #[test]
    fn default_is_first_and_marked() {
        let actions = next_actions(
            &host(HostKind::Standalone),
            CommandStage::Any,
            &[TuiCommandId::Status, TuiCommandId::Journal],
            Some(TuiCommandId::Journal),
        );
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].id, TuiCommandId::Journal);
        assert!(actions[0].default);
        assert!(!actions[1].default);
    }

    #[test]
    fn default_outside_the_candidates_falls_back_to_the_first() {
        let actions = next_actions(
            &host(HostKind::Standalone),
            CommandStage::Any,
            &[TuiCommandId::Status, TuiCommandId::Journal],
            Some(TuiCommandId::Quit),
        );
        assert_eq!(actions[0].id, TuiCommandId::Status);
        assert!(actions[0].default);
    }

    #[test]
    fn menu_is_capped_and_deduplicated() {
        let actions = next_actions(
            &host(HostKind::Standalone),
            CommandStage::Any,
            &[
                TuiCommandId::Help,
                TuiCommandId::Status,
                TuiCommandId::Journal,
                TuiCommandId::Clear,
                TuiCommandId::Quit,
                TuiCommandId::Status,
            ],
            None,
        );
        assert_eq!(actions.len(), MAX_NEXT_ACTIONS);
        let mut ids = actions.iter().map(|action| action.id).collect::<Vec<_>>();
        let before = ids.len();
        ids.sort_by_key(|id| *id as u8 as u32);
        ids.dedup();
        assert_eq!(before, ids.len());
    }

    #[test]
    fn every_entry_carries_a_real_command_name() {
        let actions = next_actions(
            &host(HostKind::Standalone),
            CommandStage::Any,
            &[TuiCommandId::Status, TuiCommandId::Journal],
            None,
        );
        for action in actions {
            assert!(action.name.starts_with('/'));
            assert!(
                COMMANDS.iter().any(|spec| spec.id == action.id),
                "menu entry must exist in the registry"
            );
        }
    }
}

#[cfg(test)]
mod next_action_stage_tests {
    use super::*;
    use crate::tui_host::HostKind;

    const STAGES: [CommandStage; 7] = [
        CommandStage::Any,
        CommandStage::NoMission,
        CommandStage::Framing,
        CommandStage::Approval,
        CommandStage::Running,
        CommandStage::Verification,
        CommandStage::Delivery,
    ];

    fn host(kind: HostKind) -> HostBinding {
        match kind {
            HostKind::Standalone => HostBinding::standalone(),
            _ => HostBinding::workshop_context(
                "master",
                "main-coordinator",
                "1agent",
                "term-1790000000000-1234-1",
            ),
        }
    }

    #[test]
    fn every_stage_list_names_registered_commands() {
        for stage in STAGES {
            let ids = stage_next_actions(stage);
            assert!(!ids.is_empty(), "{stage:?} has no candidates");
            for id in ids {
                assert!(
                    COMMANDS.iter().any(|spec| spec.id == *id),
                    "{stage:?} references an unregistered command"
                );
            }
        }
    }

    #[test]
    fn a_standalone_session_gets_a_menu_where_it_has_commands() {
        let standalone = host(HostKind::Standalone);
        for stage in [CommandStage::Any, CommandStage::NoMission] {
            let actions = next_actions(&standalone, stage, stage_next_actions(stage), None);
            assert!(!actions.is_empty(), "{stage:?} produced no menu");
            assert!(actions[0].default, "the first entry is the default");
        }
    }

    #[test]
    fn a_master_session_gets_a_menu_at_every_stage() {
        let master = host(HostKind::WorkshopMaster);
        for stage in STAGES {
            let actions = next_actions(&master, stage, stage_next_actions(stage), None);
            assert!(!actions.is_empty(), "{stage:?} produced no menu");
            assert!(actions[0].default);
        }
    }

    #[test]
    fn a_standalone_session_never_sees_a_master_command_in_the_menu() {
        let standalone = host(HostKind::Standalone);
        for stage in STAGES {
            for action in next_actions(&standalone, stage, stage_next_actions(stage), None) {
                assert!(action.name.starts_with('/'));
                assert!(
                    COMMANDS
                        .iter()
                        .any(|spec| spec.id == action.id && spec.scope == TuiCommandScope::Core),
                    "{stage:?} leaked a master command into standalone"
                );
            }
        }
    }

    #[test]
    fn drawer_groups_standalone_commands_and_keeps_recent_first() {
        let rows = drawer_rows(
            "/",
            &host(HostKind::Standalone),
            CommandStage::NoMission,
            false,
            &[TuiCommandId::Mode, TuiCommandId::Status],
            PromptLanguage::Zh,
        );
        let commands = rows
            .iter()
            .filter(|row| row.entry.command().is_some())
            .collect::<Vec<_>>();
        assert_eq!(commands[0].heading, Some("最近"));
        assert_eq!(
            commands[0].entry.command().unwrap().spec.id,
            TuiCommandId::Mode
        );
        assert_eq!(
            commands[1].entry.command().unwrap().spec.id,
            TuiCommandId::Status
        );
        assert!(
            rows.iter()
                .filter_map(|row| row.entry.command())
                .all(|entry| entry.spec.scope == TuiCommandScope::Core)
        );
        assert!(
            rows.iter()
                .any(|row| row.heading == Some(CommandGroup::Session.label()))
        );
        assert!(
            rows.iter()
                .any(|row| row.heading == Some(CommandGroup::Evidence.label()))
        );
        assert_eq!(
            rows.iter()
                .filter(|row| {
                    row.entry
                        .command()
                        .is_some_and(|entry| entry.spec.id == TuiCommandId::Mode)
                })
                .count(),
            1
        );
    }

    #[test]
    fn drawer_keeps_a_flat_order_when_the_query_has_content() {
        let rows = drawer_rows(
            "/st",
            &host(HostKind::Standalone),
            CommandStage::NoMission,
            false,
            &[],
            PromptLanguage::Zh,
        );
        assert!(rows.iter().all(|row| row.heading.is_none()));
        assert!(rows.iter().any(|row| {
            row.entry
                .command()
                .is_some_and(|entry| entry.spec.id == TuiCommandId::Status)
        }));
    }

    #[test]
    fn prompt_supplements_are_featured_and_searchable() {
        let host = host(HostKind::Standalone);
        let rows = drawer_rows(
            "/",
            &host,
            CommandStage::NoMission,
            false,
            &[],
            PromptLanguage::Zh,
        );
        assert!(
            rows.iter()
                .any(|row| row.heading == Some("提示词补充 · 输入关键词 · // 原文库"))
        );
        assert!(rows.iter().any(|row| {
            row.entry
                .prompt()
                .is_some_and(|prompt| prompt.name == "真正的目的是")
        }));

        let rows = drawer_rows(
            "/验收",
            &host,
            CommandStage::NoMission,
            false,
            &[],
            PromptLanguage::Zh,
        );
        assert!(rows.iter().any(|row| row.heading == Some("提示词补充")));
        assert!(rows.iter().any(|row| {
            row.entry
                .prompt()
                .is_some_and(|prompt| prompt.name == "先说验收标准")
        }));
    }

    #[test]
    fn prompt_drawer_without_a_task_lists_the_whole_source_library_by_category() {
        let rows = prompt_drawer_rows("", PromptLanguage::Zh);
        assert_eq!(rows.len(), 32);
        assert_eq!(rows[0].heading, Some("原文库 · 完整原文"));
        assert!(rows.iter().any(|row| row.heading == Some("先验证再算完")));
        let source = rows[0].entry.source().expect("source row");
        assert_eq!(source.topic, "Think Before Coding");
        assert_eq!(source.fragments, 6);
        assert!(source.translation.contains("先想再写"));
        assert!(source.original.contains("Think Before Coding"));
        let rows = prompt_drawer_rows("", PromptLanguage::En);
        assert_eq!(rows.len(), 32);
    }

    #[test]
    fn english_drawer_only_lists_cards_with_english_source() {
        let host = host(HostKind::Standalone);
        let empty = drawer_rows(
            "/",
            &host,
            CommandStage::NoMission,
            false,
            &[],
            PromptLanguage::En,
        );
        assert_eq!(
            empty
                .iter()
                .filter(|row| row.entry.prompt().is_some())
                .count(),
            14
        );
        let rows = drawer_rows(
            "/real goal",
            &host,
            CommandStage::NoMission,
            false,
            &[],
            PromptLanguage::En,
        );
        assert!(rows.iter().any(|row| {
            row.entry
                .prompt()
                .is_some_and(|prompt| prompt.id == "prompt-01")
        }));
        let rows = drawer_rows(
            "/验收",
            &host,
            CommandStage::NoMission,
            false,
            &[],
            PromptLanguage::En,
        );
        assert!(rows.iter().all(|row| row.entry.prompt().is_none()));
    }

    #[test]
    fn destructive_inputs_require_confirmation() {
        assert!(is_destructive_input("/clear"));
        assert!(is_destructive_input("/dispatch"));
        assert!(is_destructive_input("/dispatch start task-1"));
        assert!(!is_destructive_input("/dispatch dry-run"));
        assert!(is_destructive_input("/control cancel task-1"));
        assert!(is_destructive_input("/control takeover task-1"));
        assert!(!is_destructive_input("/control pause task-1"));
        assert!(!is_destructive_input("build a report"));
    }
}
