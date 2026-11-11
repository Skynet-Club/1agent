//! Chat-surface adapter contract.
//!
//! This is the pure mapping between a chat message and a task request, and
//! between an `AgentAnswer` and a concise chat reply. It does not implement a
//! Slack/Telegram/WeChat transport.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::domain::{AgentAnswer, AnswerStatus, TaskRequest};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatChannel {
    Slack,
    Telegram,
    WeChat,
    Web,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ChatMessage {
    pub channel: ChatChannel,
    pub sender_id: String,
    pub thread_id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ChatReply {
    pub thread_id: String,
    pub text: String,
    pub needs_approval: bool,
    pub inbox_open: u32,
}

/// Map a chat message to a task request. The caller remains responsible for
/// validating the text and for the chat transport.
pub fn to_task_request(message: &ChatMessage, workspace: PathBuf) -> TaskRequest {
    TaskRequest::from_objective(message.text.trim().to_string(), workspace)
}

/// Build a concise chat reply: conclusion first, one trust line, and an
/// approval flag when the answer is blocked on authorization.
pub fn format_reply(answer: &AgentAnswer, thread_id: &str, inbox_open: u32) -> ChatReply {
    let needs_approval = answer.status == AnswerStatus::NeedsAuthorization;
    let mut text = answer.conclusion.trim().to_string();
    if !text.is_empty() {
        text.push('\n');
    }
    text.push_str(&format!(
        "证据 {} · 未知 {}",
        answer.evidence.len(),
        answer.unknowns.len()
    ));
    if needs_approval {
        text.push_str("\n需要审批后才能继续。");
    }
    ChatReply {
        thread_id: thread_id.into(),
        text,
        needs_approval,
        inbox_open,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AgentAnswer, AnswerStatus, Evidence, UnknownItem};

    fn answer(status: AnswerStatus) -> AgentAnswer {
        AgentAnswer {
            schema_version: crate::domain::ANSWER_SCHEMA_VERSION,
            status,
            conclusion: "先说结论：路由器的方案更便宜。".into(),
            claims: Vec::new(),
            evidence: vec![Evidence {
                source: "local note".into(),
                statement: "two rooms".into(),
                observed_unix_ms: None,
                expires_unix_ms: None,
                claim_ids: Vec::new(),
            }],
            assumptions: Vec::new(),
            constraints: Vec::new(),
            unknowns: vec![UnknownItem {
                id: "u1".into(),
                description: "wall material".into(),
                impact: crate::domain::ImpactLevel::Low,
                why_it_matters: None,
                reduction_method: None,
            }],
            conflicts: Vec::new(),
            flip_conditions: Vec::new(),
            action_receipts: Vec::new(),
        }
    }

    #[test]
    fn message_maps_to_a_task_request() {
        let message = ChatMessage {
            channel: ChatChannel::Slack,
            sender_id: "owner".into(),
            thread_id: "thread-1".into(),
            text: "  compare A and B  ".into(),
        };
        let request = to_task_request(&message, PathBuf::from("/workspace"));
        assert_eq!(request.objective, "compare A and B");
    }

    #[test]
    fn reply_keeps_the_conclusion_and_trust_line_without_schema_leaks() {
        let reply = format_reply(&answer(AnswerStatus::Completed), "thread-1", 2);
        assert!(reply.text.contains("先说结论"));
        assert!(reply.text.contains("证据 1 · 未知 1"));
        assert!(!reply.needs_approval);
        assert_eq!(reply.inbox_open, 2);
        for forbidden in ["schema_version", "claims", "action_receipts"] {
            assert!(!reply.text.contains(forbidden));
        }
    }

    #[test]
    fn authorization_marks_the_reply_as_needing_approval() {
        let reply = format_reply(&answer(AnswerStatus::NeedsAuthorization), "thread-2", 0);
        assert!(reply.needs_approval);
        assert!(reply.text.contains("需要审批"));
    }
}
