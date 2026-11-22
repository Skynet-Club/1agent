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
/// interrupted. Reasoning entries are ignored so a crash after reasoning but
/// before the answer still gets a resume notice.
pub fn needs_resume_notice(path: &Path) -> bool {
    load_entries(path)
        .iter()
        .rev()
        .find(|entry| !matches!(entry, SessionEntry::Reasoning { .. }))
        .is_some_and(|entry| matches!(entry, SessionEntry::Message { role, .. } if role == "user"))
}

/// Prior turns as model messages, bounded by count and characters.
pub fn context_messages(path: &Path) -> Vec<ModelMessage> {
    let entries = load_entries(path);
    let start = entries.len().saturating_sub(MAX_CONTEXT_MESSAGES);
    let mut messages = Vec::new();
    let mut chars = 0usize;
    for entry in entries[start..].iter().rev() {
        let (message, extra) = match entry {
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
        messages.push(message);
    }
    messages.reverse();
    messages
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
