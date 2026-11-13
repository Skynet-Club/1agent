//! Human-readable answer rendering for the CLI.
//!
//! `render_answer` is the full audit view; `render_answer_default` is the
//! layered summary that hides structured fields unless the answer is
//! high-risk. TUI hosts have their own renderer.

use crate::domain::{AnswerStatus, ClaimStatus, ImpactLevel, RunResult};

pub fn render_answer(result: &RunResult) -> String {
    let answer = &result.answer;
    let mut output = format!(
        "状态：{}\n结论：{}\n终止原因：{:?}",
        answer_status_label(answer.status),
        answer.conclusion,
        result.stop_reason
    );
    if !answer.claims.is_empty() {
        output.push_str("\n我的判断：");
        for claim in &answer.claims {
            output.push_str(&format!(
                "\n- [{}] {}",
                claim_status_label(claim.status),
                claim.text
            ));
        }
    }
    if !answer.evidence.is_empty() {
        output.push_str("\n依据：");
        for evidence in &answer.evidence {
            output.push_str(&format!("\n- [{}] {}", evidence.source, evidence.statement));
        }
    }
    if !answer.assumptions.is_empty() {
        output.push_str("\n我默认的前提：");
        for assumption in &answer.assumptions {
            output.push_str(&format!(
                "\n- [{}] {}",
                answer_impact_label(assumption.impact),
                assumption.description
            ));
        }
    }
    if !answer.constraints.is_empty() {
        output.push_str("\n限制：");
        for constraint in &answer.constraints {
            output.push_str(&format!("\n- {constraint}"));
        }
    }
    if !answer.unknowns.is_empty() {
        output.push_str("\n还没确定：");
        for unknown in &answer.unknowns {
            output.push_str(&format!("\n- {}", unknown.description));
        }
    }
    if !answer.flip_conditions.is_empty() {
        output.push_str("\n什么情况会改变结论：");
        for condition in &answer.flip_conditions {
            output.push_str(&format!(
                "\n- 若 {}，则 {}",
                condition.if_observed, condition.then
            ));
        }
    }
    output
}

/// Default human output: the conclusion, one trust-summary line and only the
/// high-risk details that change a decision. Full fields and the route trace
/// require `--details`.
pub fn render_answer_default(result: &RunResult) -> String {
    let answer = &result.answer;
    let mut output = format!(
        "状态：{}\n结论：{}\n",
        answer_status_label(answer.status),
        answer.conclusion
    );
    output.push_str(&format!(
        "信任摘要：我的判断 {} · 依据 {} · 假设 {} · 还没确定 {} · 改判 {}\n",
        answer.claims.len(),
        answer.evidence.len(),
        answer.assumptions.len(),
        answer.unknowns.len(),
        answer.flip_conditions.len()
    ));
    let high_risk = matches!(
        answer.status,
        AnswerStatus::Conditional | AnswerStatus::NeedsEvidence
    ) || answer
        .unknowns
        .iter()
        .any(|unknown| unknown.impact == ImpactLevel::High);
    if high_risk {
        let evidence = answer.evidence.iter().take(3).collect::<Vec<_>>();
        if !evidence.is_empty() {
            output.push_str("依据：\n");
            for evidence in evidence {
                output.push_str(&format!("- [{}] {}\n", evidence.source, evidence.statement));
            }
        }
        if !answer.flip_conditions.is_empty() {
            output.push_str("什么情况会改变结论：\n");
            for condition in &answer.flip_conditions {
                output.push_str(&format!(
                    "- 若 {}，则 {}\n",
                    condition.if_observed, condition.then
                ));
            }
        }
    }
    output.push_str("使用 --details 查看完整字段与路由轨迹\n");
    output
}

pub fn answer_impact_label(impact: ImpactLevel) -> &'static str {
    match impact {
        ImpactLevel::Low => "低",
        ImpactLevel::Medium => "中",
        ImpactLevel::High => "高",
    }
}

pub fn answer_status_label(status: AnswerStatus) -> &'static str {
    match status {
        AnswerStatus::Completed => "完成",
        AnswerStatus::Conditional => "有条件完成",
        AnswerStatus::NeedsEvidence => "需证据",
        AnswerStatus::NeedsAuthorization => "需授权",
        AnswerStatus::Failed => "无法完成",
    }
}

pub fn claim_status_label(status: ClaimStatus) -> &'static str {
    match status {
        ClaimStatus::Verified => "实测",
        ClaimStatus::UserAsserted => "你说的",
        ClaimStatus::Derived => "推断",
        ClaimStatus::Assumed => "假设",
        ClaimStatus::ModelPrior => "经验",
        ClaimStatus::Conflicting => "有冲突",
        ClaimStatus::Unknown => "不确定",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        AgentAnswer, Claim, Evidence, FlipCondition, ImpactLevel, RunResult, StopReason,
        UnknownItem,
    };
    use std::path::PathBuf;
    use uuid::Uuid;

    fn run(
        status: AnswerStatus,
        claims: Vec<Claim>,
        evidence: Vec<Evidence>,
        unknowns: Vec<UnknownItem>,
        flips: Vec<FlipCondition>,
    ) -> RunResult {
        RunResult {
            run_id: Uuid::new_v4(),
            journal_path: PathBuf::from("journal.jsonl"),
            answer: AgentAnswer {
                task_need: None,
                schema_version: crate::domain::ANSWER_SCHEMA_VERSION,
                status,
                conclusion: "conclusion".into(),
                claims,
                evidence,
                assumptions: Vec::new(),
                constraints: Vec::new(),
                unknowns,
                conflicts: Vec::new(),
                flip_conditions: flips,
                action_receipts: Vec::new(),
            },
            stop_reason: StopReason::Completed,
        }
    }

    fn claim() -> Claim {
        Claim {
            id: "c1".into(),
            text: "claim text".into(),
            status: ClaimStatus::Verified,
            sources: vec!["fixture".into()],
            scope: None,
        }
    }

    #[test]
    fn low_risk_default_hides_structured_fields() {
        let result = run(
            AnswerStatus::Completed,
            vec![claim()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let concise = render_answer_default(&result);
        assert!(concise.contains("信任摘要："));
        assert!(concise.contains("使用 --details"));
        assert!(!concise.contains("我的判断："));
        let full = render_answer(&result);
        assert!(full.contains("我的判断："));
        assert!(full.contains("终止原因："));
    }

    #[test]
    fn high_risk_default_inlines_evidence_and_flips() {
        let result = run(
            AnswerStatus::NeedsEvidence,
            Vec::new(),
            vec![Evidence {
                source: "fixture".into(),
                statement: "observed".into(),
                observed_unix_ms: None,
                expires_unix_ms: None,
                claim_ids: Vec::new(),
            }],
            vec![UnknownItem {
                id: "u1".into(),
                description: "missing provider".into(),
                impact: ImpactLevel::High,
                why_it_matters: None,
                reduction_method: None,
            }],
            vec![FlipCondition {
                if_observed: "provider configured".into(),
                then: "rerun".into(),
                target_ids: Vec::new(),
            }],
        );
        let concise = render_answer_default(&result);
        assert!(concise.contains("依据："));
        assert!(concise.contains("什么情况会改变结论："));
    }

    #[test]
    fn labels_are_stable() {
        assert_eq!(answer_status_label(AnswerStatus::NeedsEvidence), "需证据");
        assert_eq!(claim_status_label(ClaimStatus::Verified), "实测");
        assert_eq!(answer_impact_label(ImpactLevel::High), "高");
    }
}
