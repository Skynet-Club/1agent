//! Durable conversation sessions. The per-run journal stays the audit record;
//! this file is the conversation-level transcript that the next run projects
//! into model context.

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::domain::ModelMessage;

/// Keep the projected context bounded; older turns stay on disk for audit.
const MAX_CONTEXT_MESSAGES: usize = 20;
const MAX_CONTEXT_CHARS: usize = 24_000;
const KEEP_AFTER_COMPACTION: usize = 10;
const SUMMARY_MAX_CHARS: usize = 2_000;
const SUMMARY_LINE_CHARS: usize = 240;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEntry {
    Message {
        role: String,
        content: String,
        run_id: String,
        #[serde(default)]
        status: String,
        timestamp_unix_ms: u64,
    },
    Synthetic {
        text: String,
        timestamp_unix_ms: u64,
    },
    Reasoning {
        run_id: String,
        step: usize,
        text: String,
        timestamp_unix_ms: u64,
    },
    Summary {
        text: String,
        first_kept_index: usize,
        timestamp_unix_ms: u64,
    },
}

pub fn session_root() -> PathBuf {
    std::env::var_os("ONEAGENT_SESSION_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::runtime_home::default_home().join("sessions"))
}

/// One JSONL conversation per workspace and session id. The workspace hash
/// keeps unrelated projects from sharing a transcript.
pub fn session_path(workspace: &Path, session_id: &str) -> PathBuf {
    let canonical = fs::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf());
    let mut hasher = Sha256::new();
    hasher.update(canonical.to_string_lossy().as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    session_root()
        .join(&digest[..16])
        .join(format!("{}.jsonl", sanitize(session_id)))
}

/// Newest session file for a workspace, if any.
pub fn latest_session(workspace: &Path) -> Option<PathBuf> {
    let path = session_path(workspace, "placeholder");
    let directory = path.parent()?.to_path_buf();
    let mut newest: Option<(SystemTime, PathBuf)> = None;
    for entry in fs::read_dir(directory).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }
        let Ok(modified) = entry.metadata().and_then(|metadata| metadata.modified()) else {
            continue;
        };
        if newest.as_ref().is_none_or(|(time, _)| modified > *time) {
            newest = Some((modified, path));
        }
    }
    newest.map(|(_, path)| path)
}

pub fn append(path: &Path, entry: &SessionEntry) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let line = serde_json::to_string(entry)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    writeln!(file, "{line}")
}

pub fn append_user(path: &Path, run_id: &str, text: &str) -> std::io::Result<()> {
    maybe_compact(path)?;
    append(
        path,
        &SessionEntry::Message {
            role: "user".into(),
            content: text.into(),
            run_id: run_id.into(),
            status: String::new(),
            timestamp_unix_ms: now_ms(),
        },
    )
}

pub fn append_assistant(
    path: &Path,
    run_id: &str,
    text: &str,
    status: &str,
) -> std::io::Result<()> {
    append(
        path,
        &SessionEntry::Message {
            role: "assistant".into(),
            content: text.into(),
            run_id: run_id.into(),
            status: status.into(),
            timestamp_unix_ms: now_ms(),
        },
    )
}

pub fn append_synthetic(path: &Path, text: &str) -> std::io::Result<()> {
    append(
        path,
        &SessionEntry::Synthetic {
            text: text.into(),
            timestamp_unix_ms: now_ms(),
        },
    )
}

pub fn append_reasoning(path: &Path, run_id: &str, step: usize, text: &str) -> std::io::Result<()> {
    append(
        path,
        &SessionEntry::Reasoning {
            run_id: run_id.into(),
            step,
            text: text.into(),
            timestamp_unix_ms: now_ms(),
        },
    )
}

/// A session whose last message-like entry is an unanswered user message was
/// interrupted. Reasoning and summary entries are ignored so a crash after
/// reasoning but before the answer still gets a resume notice.
pub fn needs_resume_notice(path: &Path) -> bool {
    load_entries(path)
        .iter()
        .rev()
        .find(|entry| {
            !matches!(
                entry,
                SessionEntry::Reasoning { .. } | SessionEntry::Summary { .. }
            )
        })
        .is_some_and(|entry| matches!(entry, SessionEntry::Message { role, .. } if role == "user"))
}

/// Compact the un-summarized tail before a run loads context, so the same run
/// sees the new summary.
pub fn compact_if_needed(path: &Path) -> std::io::Result<()> {
    maybe_compact(path)
}

/// Prior turns as model messages, bounded by count and characters. When a
/// summary entry exists, it replaces the turns before it.
pub fn context_messages(path: &Path) -> Vec<ModelMessage> {
    let entries = load_entries(path);
    let summary = entries
        .iter()
        .rposition(|entry| matches!(entry, SessionEntry::Summary { .. }))
        .and_then(|index| match &entries[index] {
            SessionEntry::Summary {
                text,
                first_kept_index,
                ..
            } => Some((index, *first_kept_index, text.clone())),
            _ => None,
        });
    let mut messages = Vec::new();
    let mut chars = 0usize;
    if let Some((_, _, text)) = &summary {
        let text: String = text.chars().take(SUMMARY_MAX_CHARS).collect();
        chars += text.chars().count();
        messages.push(ModelMessage::user(format!("[Session summary]\n{text}")));
    }
    let begin = summary.as_ref().map(|(_, kept, _)| *kept).unwrap_or(0);
    let summary_index = summary.as_ref().map(|(index, _, _)| *index);
    let mut tail_indices: Vec<usize> = (begin..entries.len())
        .filter(|index| Some(*index) != summary_index)
        .filter(|index| {
            matches!(
                entries[*index],
                SessionEntry::Message { .. } | SessionEntry::Synthetic { .. }
            )
        })
        .collect();
    if tail_indices.len() > MAX_CONTEXT_MESSAGES {
        tail_indices = tail_indices.split_off(tail_indices.len() - MAX_CONTEXT_MESSAGES);
    }
    let mut kept = Vec::new();
    for index in tail_indices.iter().rev() {
        let (message, extra) = match &entries[*index] {
            SessionEntry::Message { role, content, .. } if role == "user" => {
                (ModelMessage::user(content), content.chars().count())
            }
            SessionEntry::Message { role, content, .. } if role == "assistant" => (
                ModelMessage::assistant(content, Vec::new()),
                content.chars().count(),
            ),
            SessionEntry::Synthetic { text, .. } => (
                ModelMessage::user(format!("[Session notice] {text}")),
                text.chars().count() + 17,
            ),
            _ => continue,
        };
        if chars + extra > MAX_CONTEXT_CHARS {
            break;
        }
        chars += extra;
        kept.push(message);
    }
    kept.reverse();
    messages.extend(kept);
    messages
}

/// Keep the un-summarized tail bounded. The summary is deterministic and
/// extractive; it never calls a model and the original entries stay on disk.
fn maybe_compact(path: &Path) -> std::io::Result<()> {
    let entries = load_entries(path);
    let start = entries
        .iter()
        .rev()
        .find_map(|entry| match entry {
            SessionEntry::Summary {
                first_kept_index, ..
            } => Some(*first_kept_index),
            _ => None,
        })
        .unwrap_or(0);
    let context_indices: Vec<usize> = (start..entries.len())
        .filter(|index| {
            matches!(
                entries[*index],
                SessionEntry::Message { .. } | SessionEntry::Synthetic { .. }
            )
        })
        .collect();
    let chars: usize = entries[start..].iter().map(entry_context_chars).sum();
    if context_indices.len() <= MAX_CONTEXT_MESSAGES && chars <= MAX_CONTEXT_CHARS {
        return Ok(());
    }
    if context_indices.len() <= KEEP_AFTER_COMPACTION {
        return Ok(());
    }
    let kept_index = context_indices[context_indices.len() - KEEP_AFTER_COMPACTION];
    if kept_index == 0 {
        return Ok(());
    }
    append(
        path,
        &SessionEntry::Summary {
            text: summarize_entries(&entries[..kept_index]),
            first_kept_index: kept_index,
            timestamp_unix_ms: now_ms(),
        },
    )
}

fn entry_context_chars(entry: &SessionEntry) -> usize {
    match entry {
        SessionEntry::Message { content, .. } => content.chars().count(),
        SessionEntry::Synthetic { text, .. } => text.chars().count() + 17,
        _ => 0,
    }
}

fn summarize_entries(entries: &[SessionEntry]) -> String {
    let mut sections = Vec::new();
    if let Some(SessionEntry::Summary { text, .. }) = entries
        .iter()
        .rev()
        .find(|entry| matches!(entry, SessionEntry::Summary { .. }))
    {
        let text: String = text.chars().take(SUMMARY_LINE_CHARS).collect();
        sections.push(format!("Earlier summary: {text}"));
    }
    let mut lines = Vec::new();
    for entry in entries {
        match entry {
            SessionEntry::Message { role, content, .. } => {
                let text: String = content.chars().take(SUMMARY_LINE_CHARS).collect();
                lines.push(format!("- {role}: {text}"));
            }
            SessionEntry::Synthetic { text, .. } => {
                let text: String = text.chars().take(SUMMARY_LINE_CHARS).collect();
                lines.push(format!("- notice: {text}"));
            }
            _ => {}
        }
    }
    let mut body = lines.join("\n");
    if body.chars().count() > SUMMARY_MAX_CHARS {
        let keep: String = body
            .chars()
            .skip(body.chars().count() - SUMMARY_MAX_CHARS)
            .collect();
        body = format!("…{keep}");
    }
    if body.is_empty() {
        return sections.join("\n\n");
    }
    sections.push(format!("Earlier conversation (extractive):\n{body}"));
    sections.join("\n\n")
}

pub fn load_entries(path: &Path) -> Vec<SessionEntry> {
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };
    BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<SessionEntry>(&line).ok())
        .collect()
}

fn sanitize(id: &str) -> String {
    id.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_round_trips_and_flags_interrupted_turns() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("session.jsonl");

        assert!(!needs_resume_notice(&path));
        append_user(&path, "run-1", "写一个页面").unwrap();
        assert!(needs_resume_notice(&path));
        append_assistant(&path, "run-1", "页面写好了", "completed").unwrap();
        assert!(!needs_resume_notice(&path));

        append_user(&path, "run-2", "这是你刚写的吗？").unwrap();
        let context = context_messages(&path);
        assert_eq!(context.len(), 3);
        assert_eq!(context[0].role, crate::domain::MessageRole::User);
        assert!(context[0].content.contains("写一个页面"));
        assert!(context[1].content.contains("页面写好了"));
        assert_eq!(context[2].content, "这是你刚写的吗？");
    }

    #[test]
    fn reasoning_is_stored_but_not_projected_into_context() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("session.jsonl");
        append_user(&path, "run-1", "9.11 和 9.9 哪个大？").unwrap();
        append_reasoning(&path, "run-1", 0, "9.9 = 9.90 > 9.11").unwrap();
        assert!(needs_resume_notice(&path));
        append_assistant(&path, "run-1", "9.9 更大", "completed").unwrap();
        assert!(!needs_resume_notice(&path));

        let entries = load_entries(&path);
        assert!(matches!(
            entries.get(1),
            Some(SessionEntry::Reasoning { step: 0, text, .. }) if text.contains("9.90")
        ));
        let context = context_messages(&path);
        assert_eq!(context.len(), 2);
        assert!(
            context
                .iter()
                .all(|message| !message.content.contains("9.90"))
        );
    }

    #[test]
    fn long_sessions_compact_into_a_summary_and_keep_the_tail() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("session.jsonl");
        for i in 0..14 {
            append_user(&path, "run", &format!("user turn {i} with some detail")).unwrap();
            append_assistant(&path, "run", &format!("assistant turn {i}"), "completed").unwrap();
        }
        let entries = load_entries(&path);
        assert_eq!(
            entries
                .iter()
                .filter(|entry| matches!(entry, SessionEntry::Summary { .. }))
                .count(),
            1
        );
        let context = context_messages(&path);
        assert!(context[0].content.contains("[Session summary]"));
        assert!(context.len() <= MAX_CONTEXT_MESSAGES + 1);
        assert!(
            context
                .iter()
                .any(|message| message.content.contains("user turn 13"))
        );
        assert!(
            context
                .iter()
                .any(|message| message.content == "assistant turn 13")
        );

        for i in 14..30 {
            append_user(&path, "run", &format!("user turn {i} with some detail")).unwrap();
            append_assistant(&path, "run", &format!("assistant turn {i}"), "completed").unwrap();
        }
        let entries = load_entries(&path);
        let summaries: Vec<&str> = entries
            .iter()
            .filter_map(|entry| match entry {
                SessionEntry::Summary { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(summaries.len() >= 2);
        assert!(summaries.last().unwrap().contains("Earlier summary"));
        assert!(
            context_messages(&path)[0]
                .content
                .contains("[Session summary]")
        );
    }

    #[test]
    fn synthetic_notice_enters_context_as_user_text() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("session.jsonl");
        append_user(&path, "run-1", "做一个页面").unwrap();
        append_synthetic(&path, "The previous run was interrupted.").unwrap();
        let context = context_messages(&path);
        assert_eq!(context.len(), 2);
        assert!(context[1].content.contains("Session notice"));
    }
}
