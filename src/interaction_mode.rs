//! Conversation interaction mode: answer a question, ask for clarification,
//! or act on a build/change request. The gate runs before capability
//! compilation so a provenance question cannot become a page-build contract.

use serde::{Deserialize, Serialize};

use crate::domain::TaskRequest;
use crate::work_ledger::WorkLedger;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionMode {
    Answer,
    Ask,
    Act,
}

impl InteractionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Answer => "answer",
            Self::Ask => "ask",
            Self::Act => "act",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionModeDecision {
    pub mode: InteractionMode,
    pub confidence: u8,
    pub reasons: Vec<String>,
}

impl InteractionModeDecision {
    pub fn act(reason: impl Into<String>) -> Self {
        Self {
            mode: InteractionMode::Act,
            confidence: 100,
            reasons: vec![reason.into()],
        }
    }
}

/// Decide what the user is doing now, not just what the text mentions.
/// Question-shaped references to earlier work default to Answer; an explicit
/// build/change request defaults to Act. Vague change requests become Ask.
pub fn classify(request: &TaskRequest, ledger: &WorkLedger) -> InteractionModeDecision {
    let objective = request.objective.trim();
    let text = format!(
        "{} {}",
        objective,
        request.deliverable.as_deref().unwrap_or_default()
    )
    .to_lowercase();
    let question = is_question(objective, &text);
    let prior = mentions_prior_work(&text);
    let framing = has_request_framing(&text);
    let action = has_action_verb(&text);
    let new_build = has_new_build(&text);
    let vague = has_vague_change(&text);

    if request.deliverable.is_some() || !request.success_criteria.is_empty() {
        return decision(
            InteractionMode::Act,
            95,
            "explicit deliverable or success criteria",
        );
    }
    if question && !framing && (prior || has_past_marker(&text)) && !has_explicit_new_request(&text)
    {
        return decision(
            InteractionMode::Answer,
            88,
            "question about earlier work; answer from the work ledger",
        );
    }
    if new_build {
        return decision(InteractionMode::Act, 90, "explicit new-build wording");
    }
    if vague && action {
        return decision(
            InteractionMode::Ask,
            70,
            "vague change request; ask what to change",
        );
    }
    if action || framing {
        return decision(InteractionMode::Act, 85, "explicit action request");
    }
    if question {
        return decision(
            InteractionMode::Answer,
            75,
            "question without an action request",
        );
    }
    if prior && !question {
        return decision(
            InteractionMode::Ask,
            65,
            "feedback about earlier work without a concrete change; ask what to change",
        );
    }
    if ledger.has_active_artifact() && objective.chars().count() <= 12 {
        return decision(
            InteractionMode::Ask,
            60,
            "short follow-up without a clear action; ask what to do",
        );
    }
    decision(
        InteractionMode::Act,
        55,
        "no question or prior-work reference; treat it as a task",
    )
}

fn decision(mode: InteractionMode, confidence: u8, reason: &str) -> InteractionModeDecision {
    InteractionModeDecision {
        mode,
        confidence,
        reasons: vec![reason.into()],
    }
}

fn is_question(objective: &str, lower: &str) -> bool {
    objective.ends_with('?')
        || objective.ends_with('？')
        || [
            "吗",
            "呢",
            "还是",
            "是不是",
            "有没有",
            "是否",
            "为什么",
            "怎么",
            "什么",
            "哪个",
            "多少",
            "谁",
            "何时",
        ]
        .iter()
        .any(|marker| lower.contains(marker))
        || [
            "?", "did you", "have you", "is it", "was it", "why ", "how ",
        ]
        .iter()
        .any(|marker| lower.contains(marker))
}

fn mentions_prior_work(lower: &str) -> bool {
    [
        "昨晚",
        "昨天",
        "上次",
        "之前",
        "刚才",
        "上一轮",
        "上一版",
        "原来",
        "旧的",
        "这版",
        "那版",
        "这个页面",
        "那个页面",
        "这是",
        "那是",
        "这份",
        "那份",
        "同一条",
        "同一个",
        "沿用",
        "复用",
        "还是",
        "last night",
        "last time",
        "previous",
        "same file",
        "reuse",
        "reused",
        "old version",
        "this page",
        "that page",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn has_request_framing(lower: &str) -> bool {
    [
        "帮我",
        "请你",
        "帮忙",
        "给我",
        "麻烦",
        "我要",
        "我想要",
        "需要你",
        "替我",
        "为我",
        "please",
        "can you",
        "could you",
        "i want",
        "i need",
        "help me",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn has_action_verb(lower: &str) -> bool {
    let chinese = [
        "做",
        "制作",
        "生成",
        "写",
        "编写",
        "改",
        "修改",
        "重做",
        "重新做",
        "重新设计",
        "修复",
        "部署",
        "实现",
        "构建",
        "创建",
        "新建",
        "新增",
        "删除",
        "替换",
        "优化",
        "改进",
        "升级",
        "转换",
        "整理",
        "分析",
        "检查",
        "验证",
        "测试",
        "读",
        "阅读",
        "查看",
        "打开",
        "运行",
        "执行",
        "安装",
        "下载",
        "上传",
        "继续",
    ];
    if chinese.iter().any(|marker| lower.contains(marker)) {
        return true;
    }
    contains_any_word(
        lower,
        &[
            "build",
            "create",
            "make",
            "write",
            "generate",
            "implement",
            "fix",
            "change",
            "update",
            "modify",
            "redesign",
            "rewrite",
            "refactor",
            "deploy",
            "add",
            "remove",
            "delete",
            "replace",
            "optimize",
            "improve",
            "convert",
            "read",
            "analyze",
            "check",
            "verify",
            "test",
            "run",
            "install",
            "download",
            "upload",
            "continue",
        ],
    )
}

fn contains_any_word(lower: &str, words: &[&str]) -> bool {
    lower
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .any(|token| words.contains(&token))
}

fn has_new_build(lower: &str) -> bool {
    [
        "重新做",
        "重做",
        "再做一版",
        "再做一",
        "新做",
        "新建",
        "新版本",
        "另一个版本",
        "从头做",
        "从零做",
        "重写",
        "重新生成",
        "重新设计一个",
        "重新设计一版",
        "重新设计新的",
        "new version",
        "another version",
        "start over",
        "from scratch",
        "rebuild",
        "redesign a",
        "make a new",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn has_explicit_new_request(lower: &str) -> bool {
    [
        "重新做一个",
        "再做一版",
        "做一个新的",
        "做一版新的",
        "重新生成一版",
        "做新的",
        "new version",
        "start over",
        "from scratch",
        "rebuild it",
        "make a new one",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn has_vague_change(lower: &str) -> bool {
    [
        "优化一下",
        "改进一下",
        "提升一下",
        "更好",
        "好看",
        "美化",
        "完善",
        "调整一下",
        "弄好",
        "弄得更好",
        "polish",
        "make it better",
        "improve it",
        "better looking",
        "beautify",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn has_past_marker(lower: &str) -> bool {
    [
        "了", "过", "已经", "刚", "完成", "did you", "have you", "was it", "is it",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

/// Mode-specific system guidance. Architecture still masks tools; this text
/// explains the boundary to the model.
pub fn guidance(mode: InteractionMode) -> Option<&'static str> {
    match mode {
        InteractionMode::Act => None,
        InteractionMode::Answer => Some(
            "ANSWER MODE (internal runtime decision, not a user request): the user is asking about existing work, not requesting a new build. Use the WORK LEDGER facts first; it records which earlier run wrote each artifact and whether the current file still matches. Do not call session_search, search_files or other tools merely to confirm the ledger. Do not write files, run acceptance_gate, preview_file, verify_artifact or browser audit. If a needed fact is missing, say what is missing instead of starting a delivery workflow.",
        ),
        InteractionMode::Ask => Some(
            "ASK MODE (internal runtime decision, not a user request): the change request is vague. Ask one concise clarification question about what to change; do not call tools and do not start building.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::work_ledger::{LedgerArtifact, LedgerEntry};

    fn request(objective: &str) -> TaskRequest {
        TaskRequest::from_objective(objective, std::env::temp_dir())
    }

    fn ledger_with_artifact() -> WorkLedger {
        WorkLedger {
            entries: vec![LedgerEntry {
                run_id: "run-1".into(),
                session_id: "session-1".into(),
                timestamp_unix_ms: 1,
                objective: "做一个产品网站页面".into(),
                mode: "act".into(),
                status: "conditional".into(),
                stop_reason: "completed".into(),
                artifacts: vec![LedgerArtifact {
                    path: "/tmp/index.html".into(),
                    sha256: "abc".into(),
                    bytes: 10,
                    fresh: true,
                }],
                reused: Vec::new(),
                blockers: Vec::new(),
            }],
        }
    }

    #[test]
    fn a_question_about_last_nights_page_is_answer_not_build() {
        let decision = classify(
            &request("你这个是重新设计的，html ，还是用昨晚的来对付我呢？"),
            &ledger_with_artifact(),
        );
        assert_eq!(decision.mode, InteractionMode::Answer);
    }

    #[test]
    fn the_original_build_request_stays_act() {
        let decision = classify(
            &request("使用lark-cli 阅读理解，深度思考后 制作成可以在手机端浏览交互的网页"),
            &WorkLedger::default(),
        );
        assert_eq!(decision.mode, InteractionMode::Act);
    }

    #[test]
    fn modify_and_new_version_are_act() {
        assert_eq!(
            classify(
                &request("把首页的主按钮改成投递简历"),
                &ledger_with_artifact()
            )
            .mode,
            InteractionMode::Act
        );
        assert_eq!(
            classify(&request("重新做一个版本，换版式"), &ledger_with_artifact()).mode,
            InteractionMode::Act
        );
    }

    #[test]
    fn past_tense_check_is_answer() {
        assert_eq!(
            classify(&request("你测了吗？"), &ledger_with_artifact()).mode,
            InteractionMode::Answer
        );
    }

    #[test]
    fn vague_change_asks_one_question() {
        assert_eq!(
            classify(&request("优化一下"), &ledger_with_artifact()).mode,
            InteractionMode::Ask
        );
    }

    #[test]
    fn english_question_without_action_is_answer() {
        assert_eq!(
            classify(&request("Is this already done?"), &ledger_with_artifact()).mode,
            InteractionMode::Answer
        );
        assert_eq!(
            classify(&request("read the notes"), &ledger_with_artifact()).mode,
            InteractionMode::Act
        );
    }

    #[test]
    fn english_question_is_answer() {
        assert_eq!(
            classify(
                &request("Is this a redesign or last night's html page?"),
                &ledger_with_artifact()
            )
            .mode,
            InteractionMode::Answer
        );
    }
}
