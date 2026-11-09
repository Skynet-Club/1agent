use serde::{Deserialize, Serialize};

use crate::domain::{MessageRole, ModelMessage, ToolObservation};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContextCompaction {
    pub before_chars: usize,
    pub after_chars: usize,
    pub compacted_tool_results: usize,
}

pub fn compact_messages(
    messages: &[ModelMessage],
    max_chars: usize,
    preserve_recent: usize,
) -> (Vec<ModelMessage>, Option<ContextCompaction>) {
    compact_messages_at(messages, max_chars, preserve_recent)
}

pub fn compact_messages_at(
    messages: &[ModelMessage],
    trigger_chars: usize,
    preserve_recent: usize,
) -> (Vec<ModelMessage>, Option<ContextCompaction>) {
    let before_chars = total_chars(messages);
    if before_chars <= trigger_chars {
        return (messages.to_vec(), None);
    }

    let mut compacted = messages.to_vec();
    let protected_head = messages.len().min(2);
    let protected_tail_start = messages.len().saturating_sub(preserve_recent);
    let mut compacted_tool_results = 0;

    for (index, message) in messages.iter().enumerate() {
        if index < protected_head || index >= protected_tail_start {
            continue;
        }
        if message.role != MessageRole::Tool {
            continue;
        }
        if let Ok(mut observation) = serde_json::from_str::<ToolObservation>(&message.content)
            && !observation.content.is_empty()
        {
            observation.content.clear();
            compacted[index].content = observation.model_content();
            compacted_tool_results += 1;
        }
    }

    let after_chars = total_chars(&compacted);
    let stats = (compacted_tool_results > 0).then_some(ContextCompaction {
        before_chars,
        after_chars,
        compacted_tool_results,
    });
    (compacted, stats)
}

pub fn total_chars(messages: &[ModelMessage]) -> usize {
    messages
        .iter()
        .map(|message| {
            message.content.chars().count()
                + message
                    .tool_calls
                    .iter()
                    .map(|call| call.arguments.to_string().chars().count())
                    .sum::<usize>()
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use crate::domain::{ToolCall, ToolStatus};

    fn observation(payload: &str) -> ToolObservation {
        ToolObservation {
            call_id: "call-1".into(),
            tool: "read_file".into(),
            status: ToolStatus::Ok,
            summary: "read evidence".into(),
            content: payload.into(),
            error_code: None,
            metadata: BTreeMap::from([("relative_path".into(), "evidence.txt".into())]),
        }
    }

    #[test]
    fn preserves_user_and_recent_messages_while_pruning_old_tool_content() {
        let old = observation(&"x".repeat(10_000));
        let recent = observation(&"y".repeat(10_000));
        let messages = vec![
            ModelMessage::system("system"),
            ModelMessage::user("Do not forget the hard constraint"),
            ModelMessage::assistant(
                "",
                vec![ToolCall {
                    id: "old-call".into(),
                    name: "read_file".into(),
                    arguments: serde_json::json!({}),
                }],
            ),
            ModelMessage::tool("old-call", old.model_content()),
            ModelMessage::assistant("working", Vec::new()),
            ModelMessage::assistant(
                "",
                vec![ToolCall {
                    id: "recent-call".into(),
                    name: "read_file".into(),
                    arguments: serde_json::json!({}),
                }],
            ),
            ModelMessage::tool("recent-call", recent.model_content()),
        ];

        let (compact, stats) = compact_messages(&messages, 5_000, 2);
        let stats = stats.unwrap();
        assert_eq!(stats.compacted_tool_results, 1);
        assert_eq!(compact[1].content, "Do not forget the hard constraint");
        let old_observation: ToolObservation = serde_json::from_str(&compact[3].content).unwrap();
        assert!(old_observation.content.is_empty());
        assert_eq!(old_observation.summary, "read evidence");
        let recent_observation: ToolObservation =
            serde_json::from_str(&compact[6].content).unwrap();
        assert_eq!(recent_observation.content.len(), 10_000);
    }

    #[test]
    fn leaves_messages_unchanged_when_within_budget() {
        let messages = vec![ModelMessage::system("s"), ModelMessage::user("u")];
        let (compact, stats) = compact_messages(&messages, 10, 2);
        assert!(stats.is_none());
        assert_eq!(compact, messages);
    }

    #[test]
    fn auto_compaction_can_trigger_before_the_hard_limit() {
        let messages = vec![
            ModelMessage::system("system"),
            ModelMessage::user("user"),
            ModelMessage::assistant("", Vec::new()),
            ModelMessage::tool("call-1", observation(&"x".repeat(5_000)).model_content()),
            ModelMessage::assistant("recent", Vec::new()),
        ];
        let (_, unchanged) = compact_messages_at(&messages, 4_000, 1);
        assert!(unchanged.is_some());
        let (_, no_trigger) = compact_messages_at(&messages, 6_000, 1);
        assert!(no_trigger.is_none());
    }
}
