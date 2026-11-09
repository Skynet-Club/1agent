use std::path::Path;

use crate::domain::{AgentAnswer, CognitiveMode};
use crate::journal::{JournalAudit, ReplayState};
use crate::tui_commands::TuiCommandId;

const MAX_COMMAND_CONTEXT_CHARS: usize = 4_000;
const MAX_RENDERED_ITEMS: usize = 20;
const MAX_RENDERED_ITEM_CHARS: usize = 600;

pub fn epistemic_command_prompt(
    command: TuiCommandId,
    args: &str,
    last_instruction: &str,
) -> Option<(String, CognitiveMode)> {
    let subject = if args.trim().is_empty() {
        last_instruction.trim()
    } else {
        args.trim()
    };
    if subject.is_empty() {
        return None;
    }
    let subject = subject
        .chars()
        .take(MAX_COMMAND_CONTEXT_CHARS)
        .collect::<String>();
    Some(match command {
        TuiCommandId::Frame => (
            format!(
                "From first principles, frame this objective without executing it: {subject}. \
                 Separate verified facts, user assertions, assumptions, constraints, unknowns, \
                 success criteria and flip conditions. Do not invent evidence."
            ),
            CognitiveMode::EvidenceResearch,
        ),
        TuiCommandId::Challenge => (
            format!(
                "Red-team the following task or conclusion without executing it: {subject}. \
                 Find counterexamples, hidden assumptions, missing evidence, weak acceptance \
                 criteria and likely failure modes. Name what observation would overturn the result."
            ),
            CognitiveMode::Diagnosis,
        ),
        TuiCommandId::Decide => (
            format!(
                "Produce an evidence-bound decision for: {subject}. Compare explicit alternatives, \
                 decision criteria, tradeoffs, unresolved unknowns and flip conditions. Do not \
                 present unsupported certainty."
            ),
            CognitiveMode::DecisionAnalysis,
        ),
        TuiCommandId::Postmortem => (
            format!(
                "Perform an evidence-bound postmortem of this failed or incomplete work: {subject}. \
                 Separate root cause, contributing factors, process defects, evidence gaps and \
                 concrete regression checks. Do not claim facts that the evidence does not support."
            ),
            CognitiveMode::Diagnosis,
        ),
        _ => return None,
    })
}

pub fn evidence_summary(journal: &Path, replay: &ReplayState) -> String {
    let mut output = format!(
        "证据板 · run={} · journal={}",
        replay.run_id,
        journal.display()
    );
    let Some(answer) = &replay.answer else {
        output.push_str("\n当前运行还没有可展示的答案。");
        return output;
    };
    output.push_str(&format!(
        "\nstatus={:?} · mode={} · stop={}",
        answer.status,
        replay.mode.map(CognitiveMode::as_str).unwrap_or("-"),
        replay
            .stop_reason
            .map(|reason| format!("{reason:?}"))
            .unwrap_or_else(|| "-".into())
    ));
    append_claims(&mut output, answer);
    append_evidence(&mut output, answer);
    append_unknowns(&mut output, answer);
    append_conflicts(&mut output, answer);
    append_flip_conditions(&mut output, answer);
    output
}

pub fn run_report(journal: &Path, replay: &ReplayState) -> String {
    let request = replay.request.as_ref();
    let answer = replay.answer.as_ref();
    let provider = match (&replay.provider_id, &replay.model) {
        (Some(provider), Some(model)) => format!("{provider}/{model}"),
        _ => "unknown".into(),
    };
    format!(
        "任务报告\nrun: {}\njournal: {}\nobjective: {}\nmode: {}\nprovider: {}\n\
         stop: {}\nanswer: {}\ntools: {} calls / {} observations\n\
         claims: {} / evidence: {} / assumptions: {} / unknowns: {} / conflicts: {}\n\n{}",
        replay.run_id,
        journal.display(),
        request
            .map(|request| bounded(&request.objective, MAX_RENDERED_ITEM_CHARS))
            .unwrap_or_else(|| "-".into()),
        replay.mode.map(CognitiveMode::as_str).unwrap_or("-"),
        provider,
        replay
            .stop_reason
            .map(|reason| format!("{reason:?}"))
            .unwrap_or_else(|| "-".into()),
        answer
            .map(|answer| format!("{:?}", answer.status))
            .unwrap_or_else(|| "none".into()),
        replay.tool_calls.len(),
        replay.observations.len(),
        answer.map(|answer| answer.claims.len()).unwrap_or(0),
        answer.map(|answer| answer.evidence.len()).unwrap_or(0),
        answer.map(|answer| answer.assumptions.len()).unwrap_or(0),
        answer.map(|answer| answer.unknowns.len()).unwrap_or(0),
        answer.map(|answer| answer.conflicts.len()).unwrap_or(0),
        answer
            .map(|answer| bounded(&answer.conclusion, 2_000))
            .unwrap_or_else(|| "当前运行没有最终答案。".into())
    )
}

pub fn audit_summary(journal: &Path, audit: &JournalAudit) -> String {
    let mut output = format!(
        "Journal 审计 · {} · {} events · run={} · terminal={}",
        if audit.ok { "通过" } else { "失败" },
        audit.event_count,
        audit
            .run_id
            .map(|run_id| run_id.to_string())
            .unwrap_or_else(|| "-".into()),
        audit
            .terminal_reason
            .map(|reason| format!("{reason:?}"))
            .unwrap_or_else(|| "-".into())
    );
    output.push_str(&format!("\njournal: {}", journal.display()));
    if !audit.unresolved_tool_calls.is_empty() {
        output.push_str("\n未配对工具调用:");
        for call_id in audit.unresolved_tool_calls.iter().take(MAX_RENDERED_ITEMS) {
            output.push_str(&format!("\n- {}", bounded(call_id, 200)));
        }
    }
    if !audit.findings.is_empty() {
        output.push_str("\n发现:");
        for finding in audit.findings.iter().take(MAX_RENDERED_ITEMS) {
            output.push_str(&format!(
                "\n- {:?} · event={} · {}",
                finding.code,
                finding
                    .event_index
                    .map(|index| index.to_string())
                    .unwrap_or_else(|| "-".into()),
                bounded(&finding.message, MAX_RENDERED_ITEM_CHARS)
            ));
        }
    }
    if audit.findings.is_empty() && audit.issues.is_empty() {
        output.push_str("\n未发现结构性问题。");
    }
    output
}

fn append_claims(output: &mut String, answer: &AgentAnswer) {
    if answer.claims.is_empty() {
        return;
    }
    output.push_str("\n主张:");
    for claim in answer.claims.iter().take(MAX_RENDERED_ITEMS) {
        let sources = if claim.sources.is_empty() {
            "-".into()
        } else {
            claim.sources.join(", ")
        };
        output.push_str(&format!(
            "\n- [{}] {}: {} ← {}",
            claim_status(claim.status),
            claim.id,
            bounded(&claim.text, MAX_RENDERED_ITEM_CHARS),
            bounded(&sources, 300)
        ));
    }
}

fn append_evidence(output: &mut String, answer: &AgentAnswer) {
    if answer.evidence.is_empty() {
        return;
    }
    output.push_str("\n证据:");
    for evidence in answer.evidence.iter().take(MAX_RENDERED_ITEMS) {
        output.push_str(&format!(
            "\n- {}: {} [claims: {}]",
            bounded(&evidence.source, 300),
            bounded(&evidence.statement, MAX_RENDERED_ITEM_CHARS),
            evidence.claim_ids.join(", ")
        ));
    }
}

fn append_unknowns(output: &mut String, answer: &AgentAnswer) {
    if answer.unknowns.is_empty() {
        return;
    }
    output.push_str("\n未知:");
    for unknown in answer.unknowns.iter().take(MAX_RENDERED_ITEMS) {
        output.push_str(&format!(
            "\n- [{}] {:?}: {} · 降低方法={}",
            unknown.id,
            unknown.impact,
            bounded(&unknown.description, MAX_RENDERED_ITEM_CHARS),
            unknown
                .reduction_method
                .as_deref()
                .map(|method| bounded(method, 300))
                .unwrap_or_else(|| "未提供".into())
        ));
    }
}

fn append_conflicts(output: &mut String, answer: &AgentAnswer) {
    if answer.conflicts.is_empty() {
        return;
    }
    output.push_str("\n冲突:");
    for conflict in answer.conflicts.iter().take(MAX_RENDERED_ITEMS) {
        output.push_str(&format!(
            "\n- {}: {} ↔ {} · {:?}",
            conflict.id, conflict.claim_ids[0], conflict.claim_ids[1], conflict.status
        ));
    }
}

fn append_flip_conditions(output: &mut String, answer: &AgentAnswer) {
    if answer.flip_conditions.is_empty() {
        return;
    }
    output.push_str("\n翻转条件:");
    for condition in answer.flip_conditions.iter().take(MAX_RENDERED_ITEMS) {
        output.push_str(&format!(
            "\n- 若 {}，则 {} [targets: {}]",
            bounded(&condition.if_observed, MAX_RENDERED_ITEM_CHARS),
            bounded(&condition.then, MAX_RENDERED_ITEM_CHARS),
            condition.target_ids.join(", ")
        ));
    }
}

fn claim_status(status: crate::domain::ClaimStatus) -> &'static str {
    match status {
        crate::domain::ClaimStatus::Verified => "verified",
        crate::domain::ClaimStatus::UserAsserted => "user_asserted",
        crate::domain::ClaimStatus::Derived => "derived",
        crate::domain::ClaimStatus::Assumed => "assumed",
        crate::domain::ClaimStatus::ModelPrior => "model_prior",
        crate::domain::ClaimStatus::Conflicting => "conflicting",
        crate::domain::ClaimStatus::Unknown => "unknown",
    }
}

fn bounded(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let mut output = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        output.push('…');
    }
    output.replace(['\r', '\n'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        AgentAnswer, Claim, ClaimStatus, Evidence, FlipCondition, ImpactLevel,
        LEGACY_ANSWER_SCHEMA_VERSION, UnknownItem,
    };
    use crate::journal::{JournalAudit, ReplayState};
    use uuid::Uuid;

    #[test]
    fn epistemic_commands_use_explicit_modes_and_require_context() {
        assert_eq!(epistemic_command_prompt(TuiCommandId::Frame, "", ""), None);
        assert_eq!(
            epistemic_command_prompt(TuiCommandId::Frame, "problem", "")
                .unwrap()
                .1,
            CognitiveMode::EvidenceResearch
        );
        assert_eq!(
            epistemic_command_prompt(TuiCommandId::Challenge, "", "plan")
                .unwrap()
                .1,
            CognitiveMode::Diagnosis
        );
        assert_eq!(
            epistemic_command_prompt(TuiCommandId::Decide, "A or B", "")
                .unwrap()
                .1,
            CognitiveMode::DecisionAnalysis
        );
        assert_eq!(
            epistemic_command_prompt(TuiCommandId::Postmortem, "failure", "")
                .unwrap()
                .1,
            CognitiveMode::Diagnosis
        );
    }

    #[test]
    fn evidence_and_report_include_provenance_without_raw_journal_dump() {
        let mut replay = ReplayState {
            run_id: Uuid::nil(),
            request: None,
            mode: Some(CognitiveMode::EvidenceResearch),
            provider_id: Some("provider".into()),
            model: Some("model".into()),
            provider_failures: Vec::new(),
            retry_delays_ms: Vec::new(),
            retry_timings: Vec::new(),
            route_plan: None,
            route_policy_sources: None,
            tool_capabilities: None,
            answer: None,
            stop_reason: Some(crate::domain::StopReason::Completed),
            tool_calls: Vec::new(),
            observations: Vec::new(),
            checkpoint: None,
            client_request_id: None,
            reframe_proposal: None,
            reframe_choice: None,
            reframe_actor: None,
            reframe_decided_at_unix_ms: None,
        };
        replay.answer = Some(AgentAnswer {
            schema_version: LEGACY_ANSWER_SCHEMA_VERSION,
            status: crate::domain::AnswerStatus::Conditional,
            conclusion: "A is preferred.".into(),
            claims: vec![Claim {
                id: "c1".into(),
                text: "A costs less.".into(),
                status: ClaimStatus::Verified,
                sources: vec!["file:a".into()],
                scope: None,
            }],
            evidence: vec![Evidence {
                source: "file:a".into(),
                statement: "A has cost 1.".into(),
                observed_unix_ms: None,
                expires_unix_ms: None,
                claim_ids: vec!["c1".into()],
            }],
            assumptions: Vec::new(),
            constraints: Vec::new(),
            unknowns: vec![UnknownItem {
                id: "u1".into(),
                description: "Load is unknown.".into(),
                impact: ImpactLevel::High,
                why_it_matters: None,
                reduction_method: Some("Run load test.".into()),
            }],
            conflicts: Vec::new(),
            flip_conditions: vec![FlipCondition {
                if_observed: "load doubles".into(),
                then: "choice changes".into(),
                target_ids: vec!["c1".into()],
            }],
            action_receipts: Vec::new(),
        });

        let evidence = evidence_summary(std::path::Path::new("run.jsonl"), &replay);
        assert!(evidence.contains("[verified] c1"));
        assert!(evidence.contains("file:a"));
        assert!(evidence.contains("Run load test."));
        let report = run_report(std::path::Path::new("run.jsonl"), &replay);
        assert!(report.contains("A is preferred."));
        assert!(report.contains("claims: 1 / evidence: 1"));
    }

    #[test]
    fn audit_summary_is_explicit_about_findings() {
        let audit = JournalAudit {
            ok: false,
            event_count: 2,
            run_id: Some(Uuid::nil()),
            terminal_reason: None,
            answer_schema_version: None,
            unresolved_tool_calls: vec!["call-1".into()],
            issues: Vec::new(),
            findings: Vec::new(),
        };
        let summary = audit_summary(std::path::Path::new("run.jsonl"), &audit);
        assert!(summary.contains("失败"));
        assert!(summary.contains("call-1"));
    }
}
