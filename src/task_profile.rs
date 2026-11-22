use serde::{Deserialize, Serialize};

use crate::domain::TaskRequest;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskProfile {
    Fast,
    Standard,
    Deep,
}

impl TaskProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Standard => "standard",
            Self::Deep => "deep",
        }
    }

    pub fn recommended_max_steps(self) -> usize {
        match self {
            Self::Fast => 24,
            Self::Standard => 48,
            Self::Deep => 80,
        }
    }

    pub fn recommended_context_tokens(self) -> usize {
        match self {
            Self::Fast => 12_000,
            Self::Standard => 32_000,
            Self::Deep => 64_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TaskProfileDecision {
    pub profile: TaskProfile,
    pub confidence: u8,
    pub reasons: Vec<String>,
    pub recommended_max_steps: usize,
    pub recommended_context_tokens: usize,
    pub ambiguous: bool,
}

impl TaskProfileDecision {
    pub fn direct_chat() -> Self {
        Self {
            profile: TaskProfile::Fast,
            confidence: 100,
            reasons: vec!["direct chat does not need the tool loop".into()],
            recommended_max_steps: 1,
            recommended_context_tokens: 4_000,
            ambiguous: false,
        }
    }

    /// Stable prefix guidance. It is injected once at run start so prompt
    /// caching is not invalidated mid-run.
    pub fn guidance(&self) -> String {
        let profile = self.profile.as_str();
        let action = match self.profile {
            TaskProfile::Fast => {
                "Small delivery: probe once, fetch only needed inputs, write once, verify once, answer. No codebase audit or repeated read/audit. If blocked, return the blocker."
            }
            TaskProfile::Standard => {
                "Short plan, only the evidence the deliverable needs, verify once, answer; expand only if verification fails."
            }
            TaskProfile::Deep => {
                "This is a complex or high-risk task. Plan before acting, keep decisions and evidence in the working notes, verify each artifact, and re-plan after a failed tool or test. Do not stop early without a concrete blocker. Maintain a short NOTES.md in the workspace with the goal, constraints, decisions, artifact paths/SHA, failed attempts, open questions and next step; update it at phase boundaries and re-read it after compaction."
            }
        };
        format!(
            "TASK PROFILE: {profile} (confidence {}).\n{action}\nBudget: about {} steps / {} tokens (guidance, not a hard stop).",
            self.confidence, self.recommended_max_steps, self.recommended_context_tokens
        )
    }
}

pub fn classify(request: &TaskRequest) -> TaskProfileDecision {
    let mut text = String::new();
    text.push_str(&request.objective);
    if let Some(deliverable) = &request.deliverable {
        text.push('\n');
        text.push_str(deliverable);
    }
    for item in &request.constraints {
        text.push('\n');
        text.push_str(item);
    }
    for item in &request.success_criteria {
        text.push('\n');
        text.push_str(item);
    }
    let lower = text.to_lowercase();
    let chars = text.chars().count();
    let mut reasons = Vec::new();

    let deep_markers = [
        "refactor",
        "migrate",
        "migration",
        "architecture",
        "security",
        "audit",
        "multi-file",
        "debug",
        "diagnose",
        "investigate",
        "research",
        "compare",
        "design",
        "plan",
        "reproduce",
        "root cause",
        "重构",
        "迁移",
        "架构",
        "安全",
        "审计",
        "多文件",
        "排查",
        "诊断",
        "调研",
        "研究",
        "对比",
        "设计",
        "规划",
        "复现",
        "根因",
        "深度思考",
    ];
    let deep_hits: Vec<&str> = deep_markers
        .into_iter()
        .filter(|marker| lower.contains(marker))
        .collect();
    if !deep_hits.is_empty() || chars > 1_200 {
        reasons.push(if chars > 1_200 {
            "long, multi-part objective".into()
        } else {
            format!("complexity markers: {}", deep_hits.join(", "))
        });
        return decision(TaskProfile::Deep, 85, reasons, false);
    }

    let ambiguous_markers = [
        "随便",
        "看着办",
        "你决定",
        "合适",
        "好看",
        "尽量",
        "maybe",
        "whatever",
        "as you like",
        "you decide",
        "look good",
        "not sure",
        "unsure",
        "ambiguous",
    ];
    let ambiguous = ambiguous_markers
        .into_iter()
        .any(|marker| lower.contains(marker));
    if ambiguous {
        reasons.push("ambiguous success criteria".into());
        return decision(TaskProfile::Standard, 60, reasons, true);
    }

    // A delivery verb plus an external source, installed skill, or formal
    // acceptance is not a small one-shot task even when it ends in a page.
    let multi_step_markers = [
        "lark-cli", "https://", "http://", "skill", "browser", "audit", "验收", "质检", "文档",
        "wiki",
    ];
    let multi_step_hits: Vec<&str> = multi_step_markers
        .into_iter()
        .filter(|marker| lower.contains(marker))
        .collect();
    let has_skill = lower.contains("skill");
    if !multi_step_hits.is_empty() && (chars > 200 || has_skill) {
        let formal_acceptance = ["验收", "质检"]
            .into_iter()
            .any(|marker| lower.contains(marker));
        let strong = lower.contains("lark-cli")
            || (formal_acceptance && lower.contains("skill"))
            || lower.contains("深度思考");
        reasons.push(format!(
            "multi-step/tool signals: {}",
            multi_step_hits.join(", ")
        ));
        return decision(
            if strong {
                TaskProfile::Deep
            } else {
                TaskProfile::Standard
            },
            if strong { 80 } else { 65 },
            reasons,
            false,
        );
    }

    let fast_markers = [
        "html", "webpage", "page", "document", "doc", "report", "summary", "write", "create",
        "build", "网页", "页面", "文档", "报告", "总结", "写", "生成", "制作",
    ];
    let fast_hits: Vec<&str> = fast_markers
        .into_iter()
        .filter(|marker| lower.contains(marker))
        .collect();
    if !fast_hits.is_empty()
        && chars <= 600
        && request.success_criteria.len() <= 2
        && request.constraints.len() <= 2
    {
        reasons.push(format!("small delivery markers: {}", fast_hits.join(", ")));
        return decision(TaskProfile::Fast, 75, reasons, false);
    }

    reasons.push("no strong fast or deep signal; use the standard loop".into());
    decision(TaskProfile::Standard, 60, reasons, false)
}

fn decision(
    profile: TaskProfile,
    confidence: u8,
    reasons: Vec<String>,
    ambiguous: bool,
) -> TaskProfileDecision {
    TaskProfileDecision {
        profile,
        confidence,
        reasons,
        recommended_max_steps: profile.recommended_max_steps(),
        recommended_context_tokens: profile.recommended_context_tokens(),
        ambiguous,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn request(objective: &str) -> TaskRequest {
        TaskRequest::from_objective(objective, PathBuf::from("."))
    }

    #[test]
    fn small_artifact_is_fast() {
        let decision = classify(&request("Make a small HTML page from this document"));
        assert_eq!(decision.profile, TaskProfile::Fast);
        assert!(!decision.ambiguous);
    }

    #[test]
    fn complex_change_is_deep() {
        let decision = classify(&request(
            "Refactor the authentication module and migrate the database schema",
        ));
        assert_eq!(decision.profile, TaskProfile::Deep);
    }

    #[test]
    fn lark_skill_page_is_deep_not_fast() {
        let decision = classify(&request(
            "使用lark-cli 阅读理解，https://example.feishu.cn/wiki/abc 并且深度思考后 使用ui-ux-pro-max skill 制作成手机网页，并得到unslop skill 的质检验收",
        ));
        assert_eq!(decision.profile, TaskProfile::Deep);
    }

    #[test]
    fn deep_guidance_names_notes_pointers_and_compaction() {
        let decision = classify(&request("Refactor the authentication module"));
        let guidance = decision.guidance();
        assert!(guidance.contains("NOTES.md"));
        assert!(guidance.contains("SHA"));
        assert!(guidance.contains("compaction"));
    }

    #[test]
    fn short_skill_task_is_at_least_standard() {
        let decision = classify(&request("Use the ui-ux skill to make this page"));
        assert_eq!(decision.profile, TaskProfile::Standard);
    }

    #[test]
    fn ambiguous_style_request_stays_standard() {
        let decision = classify(&request("Make the page look good; you decide the style"));
        assert_eq!(decision.profile, TaskProfile::Standard);
        assert!(decision.ambiguous);
    }

    #[test]
    fn short_plain_request_is_standard() {
        let decision = classify(&request("What is the current time?"));
        assert_eq!(decision.profile, TaskProfile::Standard);
    }
}
