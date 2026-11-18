//! File-based chat bridge contract.
//!
//! Converts a `ChatMessage` file into a `TaskRequest` file, and a run result
//! file into a `ChatReply` file. It is a local adapter building block, not a
//! Slack/Telegram/WeChat connection and not a daemon.

use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ChatBridgeState {
    #[serde(default)]
    pub seen_hashes: Vec<String>,
}

pub fn open_state(path: &Path) -> Result<ChatBridgeState, ChatBridgeError> {
    let state: ChatBridgeState = match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => ChatBridgeState::default(),
        Err(error) => return Err(error.into()),
    };
    if state.seen_hashes.len() > 10_000 {
        return Err(ChatBridgeError::Invalid(
            "chat bridge state has more than 10000 hashes".into(),
        ));
    }
    if state
        .seen_hashes
        .iter()
        .any(|hash| hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return Err(ChatBridgeError::Invalid(
            "chat bridge state contains a non-SHA-256 hash".into(),
        ));
    }
    Ok(state)
}

pub fn read_message(path: &Path) -> Result<ChatMessage, ChatBridgeError> {
    Ok(read_message_with_hash(path)?.0)
}

fn read_message_with_hash(path: &Path) -> Result<(ChatMessage, String), ChatBridgeError> {
    let bytes = read_bounded(path)?;
    let hash = hex::encode(Sha256::digest(&bytes));
    let message: ChatMessage = serde_json::from_slice(&bytes)?;
    if message.sender_id.trim().is_empty() {
        return Err(ChatBridgeError::Invalid("sender_id is empty".into()));
    }
    if message.thread_id.trim().is_empty() {
        return Err(ChatBridgeError::Invalid("thread_id is empty".into()));
    }
    if message.text.trim().is_empty() {
        return Err(ChatBridgeError::Invalid("text is empty".into()));
    }
    Ok((message, hash))
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ChatBridgeIngestReport {
    pub scanned: usize,
    pub converted: usize,
    pub skipped: usize,
    pub duplicates: usize,
    pub failed: usize,
    pub task_files: Vec<String>,
    pub errors: Vec<String>,
}

pub struct IngestLock {
    path: std::path::PathBuf,
}

impl IngestLock {
    pub fn acquire(path: impl Into<std::path::PathBuf>) -> Result<Self, ChatBridgeError> {
        use std::io::Write;
        let path = path.into();
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| {
                ChatBridgeError::Invalid(format!(
                    "cannot acquire ingest lock {}: {error}",
                    path.display()
                ))
            })?;
        writeln!(file, "{}", std::process::id())?;
        Ok(Self { path })
    }
}

impl Drop for IngestLock {
    fn drop(&mut self) {
        std::fs::remove_file(&self.path).ok();
    }
}

#[cfg(test)]
pub fn ingest_dir(
    inbox: &Path,
    workspace: &Path,
    out_dir: &Path,
    processed_dir: &Path,
    failed_dir: &Path,
) -> Result<ChatBridgeIngestReport, ChatBridgeError> {
    ingest_dir_with_state(inbox, workspace, out_dir, processed_dir, failed_dir, None)
}

pub fn ingest_dir_with_state(
    inbox: &Path,
    workspace: &Path,
    out_dir: &Path,
    processed_dir: &Path,
    failed_dir: &Path,
    state_path: Option<&Path>,
) -> Result<ChatBridgeIngestReport, ChatBridgeError> {
    const MAX_FILES: usize = 500;
    std::fs::create_dir_all(out_dir)?;
    std::fs::create_dir_all(processed_dir)?;
    std::fs::create_dir_all(failed_dir)?;
    let mut state = state_path.map(open_state).transpose()?;
    let mut state_changed = false;
    let mut files = Vec::new();
    for entry in std::fs::read_dir(inbox)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_file() && path.extension().is_some_and(|ext| ext == "json") {
            files.push(path);
        }
    }
    files.sort();
    if files.len() > MAX_FILES {
        return Err(ChatBridgeError::Invalid(format!(
            "inbox has more than {MAX_FILES} message files"
        )));
    }
    let mut report = ChatBridgeIngestReport {
        scanned: files.len(),
        converted: 0,
        skipped: 0,
        duplicates: 0,
        failed: 0,
        task_files: Vec::new(),
        errors: Vec::new(),
    };
    for path in files {
        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("message");
        match read_message_with_hash(&path) {
            Ok((message, hash)) => {
                if state
                    .as_ref()
                    .is_some_and(|state| state.seen_hashes.iter().any(|seen| seen == &hash))
                {
                    move_to(&path, &processed_dir.join(format!("{stem}.json")))?;
                    report.duplicates += 1;
                    continue;
                }
                let task_path = out_dir.join(format!("{stem}.task.json"));
                if task_path.exists() {
                    move_to(&path, &processed_dir.join(format!("{stem}.json")))?;
                    report.skipped += 1;
                    if let Some(state) = &mut state
                        && !state.seen_hashes.iter().any(|seen| seen == &hash)
                    {
                        state.seen_hashes.push(hash);
                        state_changed = true;
                    }
                    continue;
                }
                let request = chat::to_task_request(&message, workspace.to_path_buf());
                write_json(&task_path, &request)?;
                move_to(&path, &processed_dir.join(format!("{stem}.json")))?;
                report.converted += 1;
                report.task_files.push(task_path.display().to_string());
                if let Some(state) = &mut state
                    && !state.seen_hashes.iter().any(|seen| seen == &hash)
                {
                    state.seen_hashes.push(hash);
                    state_changed = true;
                }
            }
            Err(error) => {
                let reason = error.to_string();
                std::fs::write(
                    failed_dir.join(format!("{stem}.error.txt")),
                    format!("{reason}\n"),
                )?;
                let failed_path = failed_dir.join(format!("{stem}.json"));
                if let Err(move_error) = move_to(&path, &failed_path) {
                    report
                        .errors
                        .push(format!("{stem}: {reason}; move failed: {move_error}"));
                } else {
                    report.failed += 1;
                    report.errors.push(format!("{stem}: {reason}"));
                }
            }
        }
    }
    if state_changed
        && let Some(path) = state_path
        && let Some(state) = state
    {
        write_json(path, &state)?;
    }
    Ok(report)
}

fn move_to(source: &Path, destination: &Path) -> Result<(), ChatBridgeError> {
    if destination.exists() {
        return Err(ChatBridgeError::Invalid(format!(
            "destination already exists: {}",
            destination.display()
        )));
    }
    std::fs::rename(source, destination)?;
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, ChatBridgeError> {
    Ok(serde_json::from_slice(&read_bounded(path)?)?)
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, ChatBridgeError> {
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
    Ok(std::fs::read(path)?)
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
    fn ingest_dir_moves_valid_and_failed_messages() {
        let dir = tempfile::tempdir().unwrap();
        let inbox = dir.path().join("inbox");
        let out = dir.path().join("tasks");
        let processed = dir.path().join("processed");
        let failed = dir.path().join("failed");
        std::fs::create_dir_all(&inbox).unwrap();
        for (name, text) in [("a.json", "first task"), ("b.json", "second task")] {
            std::fs::write(
                inbox.join(name),
                serde_json::to_vec(&serde_json::json!({
                    "channel": "web",
                    "sender_id": "owner",
                    "thread_id": "thread-1",
                    "text": text
                }))
                .unwrap(),
            )
            .unwrap();
        }
        std::fs::write(
            inbox.join("c.json"),
            serde_json::to_vec(&serde_json::json!({
                "channel": "web",
                "sender_id": "owner",
                "thread_id": "thread-1",
                "text": " "
            }))
            .unwrap(),
        )
        .unwrap();

        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join("a.task.json"), b"already converted").unwrap();

        let report = ingest_dir(&inbox, dir.path(), &out, &processed, &failed).unwrap();
        assert_eq!(report.scanned, 3);
        assert_eq!(report.converted, 1);
        assert_eq!(report.skipped, 1);
        assert_eq!(report.failed, 1);
        assert_eq!(report.task_files.len(), 1);
        assert_eq!(std::fs::read_dir(&processed).unwrap().count(), 2);
        assert_eq!(std::fs::read_dir(&failed).unwrap().count(), 2);
        assert!(!inbox.join("c.json").exists());
        assert_eq!(
            std::fs::read(out.join("a.task.json")).unwrap(),
            b"already converted"
        );

        let second = ingest_dir(&inbox, dir.path(), &out, &processed, &failed).unwrap();
        assert_eq!(second.scanned, 0);
    }

    #[test]
    fn state_deduplicates_the_same_content_under_a_new_name() {
        let dir = tempfile::tempdir().unwrap();
        let inbox = dir.path().join("inbox");
        let out = dir.path().join("tasks");
        let processed = dir.path().join("processed");
        let failed = dir.path().join("failed");
        let state = dir.path().join("state.json");
        std::fs::create_dir_all(&inbox).unwrap();
        let message = serde_json::to_vec(&serde_json::json!({
            "channel": "web",
            "sender_id": "owner",
            "thread_id": "thread-1",
            "text": "same task"
        }))
        .unwrap();
        std::fs::write(inbox.join("a.json"), &message).unwrap();

        let first =
            ingest_dir_with_state(&inbox, dir.path(), &out, &processed, &failed, Some(&state))
                .unwrap();
        assert_eq!(first.converted, 1);
        assert!(state.exists());

        std::fs::write(inbox.join("d.json"), &message).unwrap();
        let second =
            ingest_dir_with_state(&inbox, dir.path(), &out, &processed, &failed, Some(&state))
                .unwrap();
        assert_eq!(second.duplicates, 1);
        assert_eq!(second.converted, 0);
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
