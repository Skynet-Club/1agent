//! Product capability facts for self-introduction questions.
//!
//! This catalog is injected only when the user asks what 1-Agent can do. It is
//! not part of the normal prompt, so ordinary chat does not pay for it.

pub const CAPABILITY_CATALOG: &str = r#"1-AGENT CAPABILITY CATALOG
Use these facts when the user asks what 1-Agent can do or how it differs from other agents. Do not invent capabilities, certifications or platform support beyond this list.

Positioning:
- 1-Agent is an executable, evidence-accountable, long-horizon agent. It is not just a chat wrapper.

Interaction and execution:
- Interactive TUI and headless CLI share one runtime, journal, provider and audit contract.
- Automatically chooses a lightweight chat path or the full tool-enabled agent; ordinary work does not require manual mode switching.
- Streaming answers, provider/model selection, thinking depth, sessions, clarify panels and usage views.
- Tools: read/write/edit files, terminal commands, file search, PDF text, URL fetch, web search, headless browser text rendering, attachments, MCP stdio/HTTP tools.
- Permission, authority, approval and execution gates; kill switch; side-effect ledger; compensation and repair; idempotency records.

Long-horizon operation:
- Goals and commitments; scheduled routines and file triggers; inbox, handoffs and approvals; watchdog pause containment.
- Checkpoint/resume, crash recovery, journal replay, 1M-token context with compaction, parallel read-only tools, bounded subagents.

Evidence and audit:
- Answers carry claims, evidence, assumptions, constraints, unknowns, conflicts, flip conditions and action receipts.
- Append-only hash-chained journal; replay, audit and export; decision receipts; verification store; trust center; local org-audit hash chain; Agent Card.
- Honest statuses: completed, conditional, needs_evidence, needs_authorization, failed. Exit 0 is not automatically "verified".

Memory and learning:
- Four-layer local memory with scope, source, taint, TTL and encryption; user model; experience injection.
- Skill capture, evaluation, canary and rollback; evolution proposals with human approval.

Models and cost:
- Deterministic and OpenAI-compatible providers; quality/balanced/latency/cost routing; health fallback and cooldown.
- Lightweight chat that automatically upgrades to tools when evidence is needed; benchmark preflight, budget, checkpoints and provider-error/crash-free circuit breakers.

Integrations and governance:
- HTTP surface, read-only Python SDK, file chat bridge, OTel export, webhook notifications, team roles, handoffs and org audit.
- Local-first; secrets stay out of transcripts and journals; Linux release with checksums and provenance.

What makes it different:
- It executes and audits, instead of only talking.
- It shows why it believes something and what would change the conclusion.
- It survives long runs and can resume after a crash.
- It controls cost and escalates to tools only when the task needs them.
- It is provider-agnostic, local-first and governed by explicit authority and approvals.
- It learns through memory, experience and reviewed skills instead of hiding a single prompt.

Boundaries:
- No browser clicking/typing or general computer-use.
- No voice, image understanding or OCR.
- No official Windows or macOS support.
- No external certification, SOC2/ISO audit or multi-tenant SaaS.
"#;

/// True when the user is asking about 1-Agent itself, not about a project or
/// another product. Keep this conservative so ordinary project questions do
/// not receive the product catalog.
pub fn asks_about_capabilities(objective: &str) -> bool {
    let lower = objective.to_lowercase();
    let self_reference = contains_any(
        &lower,
        &[
            "你",
            "1-agent",
            "1agent",
            "super-agent",
            "super agent",
            "sagent",
            "s-agent",
        ],
    );
    let topic = contains_any(
        &lower,
        &[
            "功能",
            "能做什么",
            "能干什么",
            "会什么",
            "能力",
            "特点",
            "优势",
            "区别",
            "不同",
            "介绍",
            "是谁",
            "who are you",
            "what can you do",
            "capabilities",
            "features",
            "difference",
        ],
    );
    if self_reference && topic {
        return true;
    }
    contains_any(
        &lower,
        &[
            "介绍一下自己",
            "介绍下自己",
            "介绍你自己",
            "介绍下你自己",
            "你是谁",
            "who are you",
            "what can you do",
        ],
    )
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_questions_are_detected_conservatively() {
        for text in [
            "介绍一下你自己都有哪些功能",
            "你能做什么？",
            "你的特点是什么",
            "你和别的 agent 有什么区别",
            "你是谁",
            "what can you do?",
        ] {
            assert!(asks_about_capabilities(text), "{text}");
        }
        for text in [
            "这个项目有什么功能？",
            "查一下其他 agent 的功能",
            "如何切换模式？",
            "读取 src/main.rs",
        ] {
            assert!(!asks_about_capabilities(text), "{text}");
        }
    }

    #[test]
    fn catalog_names_the_differentiators_and_boundaries() {
        assert!(CAPABILITY_CATALOG.contains("evidence-accountable"));
        assert!(CAPABILITY_CATALOG.contains("executes and audits"));
        assert!(CAPABILITY_CATALOG.contains("crash recovery"));
        assert!(CAPABILITY_CATALOG.contains("automatically upgrades to tools"));
        assert!(CAPABILITY_CATALOG.contains("No browser clicking"));
        assert!(CAPABILITY_CATALOG.len() < 3_200);
    }
}
