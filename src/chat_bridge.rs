//! File-based chat bridge contract.
//!
//! Converts a `ChatMessage` file into a `TaskRequest` file, and a run result
//! file into a `ChatReply` file. It is a local adapter building block, not a
//! Slack/Telegram/WeChat connection and not a daemon.

use std::path::Path;

use thiserror::Error;

use crate::chat::{self, ChatMessage, ChatReply};
use crate::domain::{RunResult, TaskRequest};

const MAX_INPUT_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Error)]
pub enum ChatBridgeError {
    #[error("chat bridge I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("chat bridge JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("chat bridge input is invalid: {0}")]
    Invalid(String),
}

pub fn read_message(path: &Path) -> Result<ChatMessage, ChatBridgeError> {
    let message: ChatMessage = read_json(path)?;
    if message.sender_id.trim().is_empty() {
        return Err(ChatBridgeError::Invalid("sender_id is empty".into()));
    }
    if message.thread_id.trim().is_empty() {
        return Err(ChatBridgeError::Invalid("thread_id is empty".into()));
    }
    if message.text.trim().is_empty() {
        return Err(ChatBridgeError::Invalid("text is empty".into()));
    }
    Ok(message)
}

pub fn message_to_task(
    message_path: &Path,
    workspace: &Path,
    out_path: &Path,
) -> Result<TaskRequest, ChatBridgeError> {
    let message = read_message(message_path)?;
    let request = chat::to_task_request(&message, workspace.to_path_buf());
    write_json(out_path, &request)?;
    Ok(request)
}

pub fn result_to_reply(
    result_path: &Path,
    thread_id: &str,
    inbox_open: u32,
    out_path: &Path,
) -> Result<ChatReply, ChatBridgeError> {
    if thread_id.trim().is_empty() {
        return Err(ChatBridgeError::Invalid("thread_id is empty".into()));
    }
    let result: RunResult = read_json(result_path)?;
    let reply = chat::format_reply(&result.answer, thread_id, inbox_open);
    write_json(out_path, &reply)?;
    Ok(reply)
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, ChatBridgeError> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(ChatBridgeError::Invalid(
            "input must not be a symlink".into(),
        ));
    }
    if !metadata.is_file() {
        return Err(ChatBridgeError::Invalid(
            "input is not a regular file".into(),
        ));
    }
    if metadata.len() > MAX_INPUT_BYTES {
        return Err(ChatBridgeError::Invalid(format!(
            "input exceeds {MAX_INPUT_BYTES} bytes"
        )));
    }
    let bytes = std::fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), ChatBridgeError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&temporary, serde_json::to_vec_pretty(value)?)?;
    std::fs::rename(&temporary, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_files_become_task_requests() {
        let dir = tempfile::tempdir().unwrap();
        let message = dir.path().join("message.json");
        let out = dir.path().join("out/task.json");
        std::fs::write(
            &message,
            serde_json::to_vec(&serde_json::json!({
                "channel": "slack",
                "sender_id": "owner",
                "thread_id": "thread-1",
                "text": "  compare A and B  "
            }))
            .unwrap(),
        )
        .unwrap();
        let request = message_to_task(&message, dir.path(), &out).unwrap();
        assert_eq!(request.objective, "compare A and B");
        let saved: TaskRequest = serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
        assert_eq!(saved.objective, "compare A and B");
    }

    #[test]
    fn invalid_messages_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let message = dir.path().join("message.json");
        std::fs::write(
            &message,
            serde_json::to_vec(&serde_json::json!({
                "channel": "slack",
                "sender_id": "owner",
                "thread_id": "thread-1",
                "text": "   "
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(read_message(&message).is_err());
    }
}
