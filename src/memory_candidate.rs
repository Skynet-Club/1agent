//! Candidate memories: proposed facts, preferences and decisions that a human
//! must review before they enter the active memory journal.
//!
//! The candidate store is an append-only JSONL file of full snapshots; the last
//! snapshot for a candidate id wins. Active memory remains hash-chained in
//! `MemoryService`; accepting a candidate is the only bridge into it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::journal::{JournalEventKind, read_events, replay_events};
use crate::memory::{
    DataClass, MemoryError, MemoryKind, MemoryLayer, MemoryRecord, MemoryScope, MemorySource,
    MemorySourceKind, MemoryTaint,
};
use crate::memory_service::{MemoryService, MemoryServiceError};

const CANDIDATE_SCHEMA: &str = "1agent.memory-candidate/v1";
const MAX_CANDIDATE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_CANDIDATES: usize = 2_048;
const MAX_CONTENT_CHARS: usize = 2_000;
const MAX_SOURCE_IDS: usize = 16;
const MAX_REASON_CHARS: usize = 500;
const MAX_EXTRACT_PER_RUN: usize = 3;
const MAX_IMPORT_BYTES: u64 = 1024 * 1024;
const MAX_IMPORT_LINES: usize = 200;
const MAX_IMPORT_FILES: usize = 64;
const MAX_IMPORT_CANDIDATES: usize = 200;

#[derive(Debug, thiserror::Error)]
pub enum CandidateError {
    #[error("memory candidate I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("memory candidate JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("memory candidate store is corrupt at line {0}")]
    Corrupt(usize),
    #[error("memory candidate {0} not found")]
    NotFound(String),
    #[error("memory candidate {0} is {1:?}, not proposed")]
    NotProposed(String, CandidateStatus),
    #[error("memory candidate contains secret material")]
    Secret,
    #[error("invalid memory candidate: {0}")]
    Invalid(String),
    #[error("memory service: {0}")]
    Memory(#[from] MemoryServiceError),
    #[error("journal: {0}")]
    Journal(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CandidateStatus {
    Proposed,
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MemoryCandidate {
    pub schema: String,
    pub candidate_id: String,
    pub layer: MemoryLayer,
    pub kind: MemoryKind,
    pub scope: MemoryScope,
    pub content: String,
    pub source_kind: MemorySourceKind,
    pub source_event_ids: Vec<String>,
    pub run_id: Option<String>,
    pub confidence: u8,
    pub taint: MemoryTaint,
    pub data_class: DataClass,
    pub valid_from_ms: u128,
    pub expires_at_ms: Option<u128>,
    pub supersedes_hint: Option<String>,
    pub status: CandidateStatus,
    pub created_ms: u128,
    pub decided_ms: Option<u128>,
    pub decided_by: Option<String>,
    pub reason: Option<String>,
    pub accepted_memory_id: Option<String>,
}

impl MemoryCandidate {
    pub fn new(
        kind: MemoryKind,
        scope: MemoryScope,
        content: impl Into<String>,
        source_kind: MemorySourceKind,
        source_event_ids: Vec<String>,
        now_ms: u128,
    ) -> Self {
        let content = normalize_content(&content.into());
        Self {
            schema: CANDIDATE_SCHEMA.into(),
            candidate_id: Uuid::new_v4().to_string(),
            layer: MemoryLayer::Semantic,
            kind,
            scope,
            content,
            source_kind,
            source_event_ids,
            run_id: None,
            confidence: 50,
            taint: MemoryTaint::UserAsserted,
            data_class: DataClass::Internal,
            valid_from_ms: now_ms,
            expires_at_ms: None,
            supersedes_hint: None,
            status: CandidateStatus::Proposed,
            created_ms: now_ms,
            decided_ms: None,
            decided_by: None,
            reason: None,
            accepted_memory_id: None,
        }
    }

    fn validate(&self) -> Result<(), CandidateError> {
        if self.schema != CANDIDATE_SCHEMA {
            return Err(CandidateError::Invalid(
                "unsupported candidate schema".into(),
            ));
        }
        if self.candidate_id.trim().is_empty() {
            return Err(CandidateError::Invalid("candidate_id is empty".into()));
        }
        if self.content.trim().is_empty() {
            return Err(CandidateError::Invalid("content is empty".into()));
        }
        if self.content.chars().count() > MAX_CONTENT_CHARS {
            return Err(CandidateError::Invalid(format!(
                "content exceeds {MAX_CONTENT_CHARS} characters"
            )));
        }
        if self.content.chars().any(char::is_control) {
            return Err(CandidateError::Invalid(
                "content contains control characters".into(),
            ));
        }
        if self.source_event_ids.is_empty() {
            return Err(CandidateError::Invalid("source_event_ids is empty".into()));
        }
        if self.source_event_ids.len() > MAX_SOURCE_IDS {
            return Err(CandidateError::Invalid("too many source_event_ids".into()));
        }
        if self.confidence > 100 {
            return Err(CandidateError::Invalid("confidence must be 0..=100".into()));
        }
        if self
            .expires_at_ms
            .is_some_and(|expires| expires <= self.valid_from_ms)
        {
            return Err(CandidateError::Invalid(
                "expires_at_ms must be after valid_from_ms".into(),
            ));
        }
        if contains_secret(&self.content) {
            return Err(CandidateError::Secret);
        }
        Ok(())
    }
}

pub struct CandidateStore {
    path: PathBuf,
    candidates: BTreeMap<String, MemoryCandidate>,
}

impl CandidateStore {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, CandidateError> {
        let path = path.into();
        let mut candidates = BTreeMap::new();
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    return Err(CandidateError::Invalid(
                        "candidate store must be a regular file".into(),
                    ));
                }
                if metadata.len() > MAX_CANDIDATE_BYTES {
                    return Err(CandidateError::Invalid(
                        "candidate store is too large".into(),
                    ));
                }
                let text = std::fs::read_to_string(&path)?;
                for (index, line) in text.lines().enumerate() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    let candidate: MemoryCandidate = serde_json::from_str(line)
                        .map_err(|_| CandidateError::Corrupt(index + 1))?;
                    if candidate.schema != CANDIDATE_SCHEMA {
                        return Err(CandidateError::Corrupt(index + 1));
                    }
                    candidates.insert(candidate.candidate_id.clone(), candidate);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        if candidates.len() > MAX_CANDIDATES {
            return Err(CandidateError::Invalid("too many candidates".into()));
        }
        Ok(Self { path, candidates })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn candidates(&self) -> impl Iterator<Item = &MemoryCandidate> {
        self.candidates.values()
    }

    pub fn proposed(&self) -> impl Iterator<Item = &MemoryCandidate> {
        self.candidates
            .values()
            .filter(|candidate| candidate.status == CandidateStatus::Proposed)
    }

    pub fn get(&self, candidate_id: &str) -> Option<&MemoryCandidate> {
        self.candidates.get(candidate_id)
    }

    pub fn propose(
        &mut self,
        candidate: MemoryCandidate,
    ) -> Result<MemoryCandidate, CandidateError> {
        candidate.validate()?;
        if self.candidates.contains_key(&candidate.candidate_id) {
            return Err(CandidateError::Invalid(
                "candidate id already exists".into(),
            ));
        }
        if self.candidates.len() >= MAX_CANDIDATES {
            return Err(CandidateError::Invalid("too many candidates".into()));
        }
        self.append(&candidate)?;
        self.candidates
            .insert(candidate.candidate_id.clone(), candidate.clone());
        Ok(candidate)
    }

    pub fn edit(
        &mut self,
        candidate_id: &str,
        content: String,
        now_ms: u128,
    ) -> Result<MemoryCandidate, CandidateError> {
        let candidate = self
            .candidates
            .get(candidate_id)
            .cloned()
            .ok_or_else(|| CandidateError::NotFound(candidate_id.into()))?;
        if candidate.status != CandidateStatus::Proposed {
            return Err(CandidateError::NotProposed(
                candidate_id.into(),
                candidate.status,
            ));
        }
        let mut updated = candidate;
        updated.content = content;
        updated.valid_from_ms = now_ms;
        updated.validate()?;
        self.append(&updated)?;
        self.candidates
            .insert(candidate_id.to_string(), updated.clone());
        Ok(updated)
    }

    pub fn reject(
        &mut self,
        candidate_id: &str,
        actor: &str,
        reason: Option<&str>,
        now_ms: u128,
    ) -> Result<MemoryCandidate, CandidateError> {
        self.decide(
            candidate_id,
            CandidateStatus::Rejected,
            actor,
            reason,
            now_ms,
        )
    }

    fn decide(
        &mut self,
        candidate_id: &str,
        status: CandidateStatus,
        actor: &str,
        reason: Option<&str>,
        now_ms: u128,
    ) -> Result<MemoryCandidate, CandidateError> {
        let candidate = self
            .candidates
            .get(candidate_id)
            .cloned()
            .ok_or_else(|| CandidateError::NotFound(candidate_id.into()))?;
        if candidate.status == status {
            return Ok(candidate);
        }
        if candidate.status != CandidateStatus::Proposed {
            return Err(CandidateError::NotProposed(
                candidate_id.into(),
                candidate.status,
            ));
        }
        let actor = actor.trim();
        if actor.is_empty() || actor.chars().count() > 128 {
            return Err(CandidateError::Invalid(
                "actor must be 1..=128 characters".into(),
            ));
        }
        let mut updated = candidate;
        updated.status = status;
        updated.decided_ms = Some(now_ms);
        updated.decided_by = Some(actor.to_string());
        updated.reason = reason
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.chars().take(MAX_REASON_CHARS).collect());
        self.append(&updated)?;
        self.candidates
            .insert(candidate_id.to_string(), updated.clone());
        Ok(updated)
    }

    fn append(&self, candidate: &MemoryCandidate) -> Result<(), CandidateError> {
        let mut line = serde_json::to_string(candidate)?;
        line.push('\n');
        crate::jsonl::append_batch(&self.path, &line)?;
        Ok(())
    }
}

/// Accept a proposed candidate into the active memory journal.
///
/// Idempotent: if the active record already exists with the same content, the
/// candidate is marked accepted instead of creating a second record.
pub fn accept(
    store: &mut CandidateStore,
    service: &mut MemoryService,
    candidate_id: &str,
    actor: &str,
    now_ms: u128,
) -> Result<MemoryRecord, CandidateError> {
    let candidate = store
        .get(candidate_id)
        .cloned()
        .ok_or_else(|| CandidateError::NotFound(candidate_id.into()))?;
    if candidate.status == CandidateStatus::Accepted
        && let Some(memory_id) = candidate.accepted_memory_id.as_deref()
        && let Some(record) = service.get(memory_id, &candidate.scope)
    {
        return Ok(record.clone());
    }
    if candidate.status != CandidateStatus::Proposed {
        return Err(CandidateError::NotProposed(
            candidate_id.into(),
            candidate.status,
        ));
    }
    candidate.validate()?;

    let mut record = MemoryRecord::new(
        candidate.candidate_id.clone(),
        candidate.layer,
        candidate.kind,
        candidate.scope.clone(),
        candidate.content.clone(),
        MemorySource::new(
            candidate.source_kind,
            candidate
                .source_event_ids
                .first()
                .cloned()
                .unwrap_or_else(|| "candidate".into()),
        ),
    );
    if let Some(run_id) = &candidate.run_id {
        record.source.run_id = Some(run_id.clone());
    }
    record.confidence = candidate.confidence;
    record.taint = candidate.taint;
    record.data_class = candidate.data_class;
    record.valid_from_ms = candidate.valid_from_ms;
    record.valid_to_ms = candidate.expires_at_ms;
    record.supersedes = candidate.supersedes_hint.clone();

    let record = match service.remember_at(record.clone(), now_ms) {
        Ok(()) => record,
        Err(MemoryServiceError::Memory(MemoryError::AlreadyExists(_))) => {
            let existing = service
                .get(&record.memory_id, &record.scope)
                .cloned()
                .ok_or_else(|| {
                    CandidateError::Invalid(
                        "active memory exists but is not readable in this scope".into(),
                    )
                })?;
            if existing.content != record.content {
                return Err(CandidateError::Invalid(
                    "an active memory with this id has different content".into(),
                ));
            }
            existing
        }
        Err(error) => return Err(error.into()),
    };

    let actor = actor.trim();
    if actor.is_empty() || actor.chars().count() > 128 {
        return Err(CandidateError::Invalid(
            "actor must be 1..=128 characters".into(),
        ));
    }
    let mut accepted = candidate;
    accepted.status = CandidateStatus::Accepted;
    accepted.decided_ms = Some(now_ms);
    accepted.decided_by = Some(actor.to_string());
    accepted.accepted_memory_id = Some(record.memory_id.clone());
    store.append(&accepted)?;
    store.candidates.insert(candidate_id.to_string(), accepted);
    Ok(record)
}

/// Reject secret-looking material before it enters a candidate.
pub fn normalize_content(content: &str) -> String {
    let mut content = content.trim().to_string();
    for prefix in ["记住：", "记住:", "请记住：", "请记住:", "remember:"] {
        if let Some(rest) = content.strip_prefix(prefix) {
            content = rest.trim().to_string();
            break;
        }
    }
    content
}

pub fn contains_secret(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    if lower.contains("-----begin")
        || lower.contains("private key")
        || lower.contains("password=")
        || lower.contains("passwd=")
        || lower.contains("api_key=")
        || lower.contains("apikey=")
        || lower.contains("secret=")
        || lower.contains("token=")
        || lower.contains("bearer ")
    {
        return true;
    }
    for token in text.split(|character: char| {
        character.is_whitespace()
            || matches!(
                character,
                '"' | '\'' | '`' | ',' | ';' | '(' | ')' | '[' | ']' | '{' | '}' | '='
            )
    }) {
        let token = token.trim();
        if token.len() >= 12
            && (token.starts_with("sk-")
                || token.starts_with("ghp_")
                || token.starts_with("gho_")
                || token.starts_with("xox")
                || token.starts_with("AKIA"))
        {
            return true;
        }
        if token.len() >= 32 && token.chars().all(|character| character.is_ascii_hexdigit()) {
            return true;
        }
    }
    false
}

/// Extract conservative candidate memories from one run journal.
pub fn extract_from_journal(
    journal_path: &Path,
    scope: &MemoryScope,
    now_ms: u128,
) -> Result<Vec<MemoryCandidate>, CandidateError> {
    let events =
        read_events(journal_path).map_err(|error| CandidateError::Journal(error.to_string()))?;
    let replay = replay_events(&events).map_err(CandidateError::Journal)?;
    let Some(request) = replay.request.as_ref() else {
        return Ok(Vec::new());
    };
    let run_id = replay.run_id.to_string();
    let run_seq = events
        .iter()
        .find(|event| matches!(event.event, JournalEventKind::RunStarted { .. }))
        .map(|event| event.seq)
        .unwrap_or(0);
    let source_event = format!("{run_id}:{run_seq}");
    let mut candidates = Vec::new();
    let mut seen = BTreeSet::new();
    for sentence in sentences(&request.objective) {
        if candidates.len() >= MAX_EXTRACT_PER_RUN {
            break;
        }
        let Some((kind, content, confidence)) = classify_sentence(&sentence) else {
            continue;
        };
        if contains_secret(&content) || !seen.insert(content.clone()) {
            continue;
        }
        let mut candidate = MemoryCandidate::new(
            kind,
            scope.clone(),
            content,
            MemorySourceKind::UserMessage,
            vec![source_event.clone()],
            now_ms,
        );
        candidate.run_id = Some(run_id.clone());
        candidate.confidence = confidence;
        candidates.push(candidate);
    }

    if candidates.len() < MAX_EXTRACT_PER_RUN
        && let Some(answer) = replay.answer.as_ref()
        && answer.status == crate::domain::AnswerStatus::Completed
    {
        let answer_seq = events
            .iter()
            .rev()
            .find(|event| matches!(event.event, JournalEventKind::AnswerProduced { .. }))
            .map(|event| event.seq)
            .unwrap_or(0);
        for sentence in sentences(&answer.conclusion) {
            if candidates.len() >= MAX_EXTRACT_PER_RUN {
                break;
            }
            let Some((kind, content, _)) = classify_sentence(&sentence) else {
                continue;
            };
            if kind != MemoryKind::Decision
                || contains_secret(&content)
                || !seen.insert(content.clone())
            {
                continue;
            }
            let mut candidate = MemoryCandidate::new(
                MemoryKind::Decision,
                scope.clone(),
                content,
                MemorySourceKind::Answer,
                vec![format!("{run_id}:{answer_seq}")],
                now_ms,
            );
            candidate.run_id = Some(run_id.clone());
            candidate.confidence = 40;
            candidate.taint = MemoryTaint::Untrusted;
            candidates.push(candidate);
        }
    }
    Ok(candidates)
}

/// Extract candidates from up to 256 journals in a directory.
pub fn extract_from_dir(
    journal_dir: &Path,
    scope: &MemoryScope,
    now_ms: u128,
) -> Result<Vec<MemoryCandidate>, CandidateError> {
    let mut paths = std::fs::read_dir(journal_dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "jsonl")
        })
        .collect::<Vec<_>>();
    paths.sort();
    let mut candidates = Vec::new();
    let mut seen = BTreeSet::new();
    for path in paths.into_iter().take(256) {
        let Ok(metadata) = std::fs::metadata(&path) else {
            continue;
        };
        if metadata.len() > MAX_CANDIDATE_BYTES {
            continue;
        }
        let Ok(extracted) = extract_from_journal(&path, scope, now_ms) else {
            continue;
        };
        for candidate in extracted {
            let key = format!("{}\0{}", candidate.scope.scope_id(), candidate.content);
            if seen.insert(key) {
                candidates.push(candidate);
            }
            if candidates.len() >= 100 {
                return Ok(candidates);
            }
        }
    }
    Ok(candidates)
}

/// Import a local text document as untrusted candidates. Every non-empty line
/// becomes one candidate with `file:line` provenance; nothing becomes active
/// memory until a human accepts it.
pub fn import_path(
    path: &Path,
    kind: MemoryKind,
    scope: &MemoryScope,
    now_ms: u128,
) -> Result<Vec<MemoryCandidate>, CandidateError> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(CandidateError::Invalid(
            "import path must be a regular file".into(),
        ));
    }
    if metadata.len() > MAX_IMPORT_BYTES {
        return Err(CandidateError::Invalid(format!(
            "import file exceeds {MAX_IMPORT_BYTES} bytes"
        )));
    }
    let text = std::fs::read_to_string(path)
        .map_err(|_| CandidateError::Invalid("import file must be UTF-8 text".into()))?;
    let display = path.display();
    let mut candidates = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let content = line.replace('\t', " ").trim().to_string();
        let length = content.chars().count();
        if !(6..=MAX_CONTENT_CHARS).contains(&length) || contains_secret(&content) {
            continue;
        }
        let mut candidate = MemoryCandidate::new(
            kind,
            scope.clone(),
            content,
            MemorySourceKind::ToolObservation,
            vec![format!("file:{display}:{}", index + 1)],
            now_ms,
        );
        candidate.taint = MemoryTaint::Untrusted;
        candidates.push(candidate);
        if candidates.len() >= MAX_IMPORT_LINES {
            break;
        }
    }
    Ok(candidates)
}

/// Import supported text files from one directory, deduplicated by content.
pub fn import_dir(
    dir: &Path,
    kind: MemoryKind,
    scope: &MemoryScope,
    now_ms: u128,
) -> Result<Vec<MemoryCandidate>, CandidateError> {
    let mut paths = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|extension| {
                matches!(
                    extension.to_str().unwrap_or_default(),
                    "md" | "markdown" | "txt" | "text" | "log"
                )
            })
        })
        .collect::<Vec<_>>();
    paths.sort();
    let mut candidates = Vec::new();
    let mut seen = BTreeSet::new();
    for path in paths.into_iter().take(MAX_IMPORT_FILES) {
        let Ok(imported) = import_path(&path, kind, scope, now_ms) else {
            continue;
        };
        for candidate in imported {
            let key = format!("{}\0{}", candidate.scope.scope_id(), candidate.content);
            if seen.insert(key) {
                candidates.push(candidate);
            }
            if candidates.len() >= MAX_IMPORT_CANDIDATES {
                return Ok(candidates);
            }
        }
    }
    Ok(candidates)
}

fn sentences(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|character: char| {
        matches!(
            character,
            '。' | '！' | '？' | '!' | '?' | '；' | ';' | '\n'
        )
    })
    .map(str::trim)
    .filter(|sentence| {
        let length = sentence.chars().count();
        (6..=500).contains(&length)
    })
    .map(str::to_string)
}

fn classify_sentence(sentence: &str) -> Option<(MemoryKind, String, u8)> {
    let lower = sentence.to_lowercase();
    let explicit = sentence.contains("记住")
        || lower.contains("remember")
        || sentence.contains("请记")
        || sentence.contains("记一下");
    let preference = lower.contains("prefer")
        || sentence.contains("喜欢")
        || sentence.contains("偏好")
        || sentence.contains("习惯");
    let boundary = lower.contains("never") || sentence.contains("不要") || sentence.contains("别 ");
    let decision =
        lower.contains("decide") || sentence.contains("决定") || sentence.contains("采用");
    let project =
        lower.contains("project") || sentence.contains("项目") || sentence.contains("仓库");
    let entity = lower.contains("customer")
        || lower.contains("company")
        || lower.contains("vendor")
        || sentence.contains("客户")
        || sentence.contains("公司")
        || sentence.contains("供应商")
        || sentence.contains("联系人");
    let process = lower.contains("workflow")
        || lower.contains("sop")
        || sentence.contains("流程")
        || sentence.contains("步骤");
    let assignment = sentence.contains('=');
    if !(explicit || boundary || preference || decision) {
        return None;
    }
    let kind = if boundary {
        MemoryKind::Feedback
    } else if preference || (explicit && assignment) {
        MemoryKind::Preference
    } else if decision {
        MemoryKind::Decision
    } else if process && explicit {
        MemoryKind::Process
    } else if entity && explicit {
        MemoryKind::Entity
    } else if project && explicit {
        MemoryKind::Project
    } else {
        MemoryKind::Fact
    };
    let content = normalize_content(sentence);
    let content = if boundary && !content.starts_with("boundary:") {
        format!("boundary: {content}")
    } else {
        content
    };
    let confidence = if explicit || boundary {
        70
    } else if preference {
        60
    } else {
        50
    };
    Some((kind, content, confidence))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AgentAnswer, AnswerStatus, CognitiveMode, TaskRequest};
    use crate::journal::Journal;
    use crate::memory::MemoryScope;

    fn paths(dir: &Path) -> (PathBuf, PathBuf, PathBuf) {
        (
            dir.join("memory.jsonl"),
            dir.join("payloads"),
            dir.join("memory.key"),
        )
    }

    fn journal_with_objective(dir: &Path, objective: &str) -> PathBuf {
        let path = dir.join("run.jsonl");
        let mut journal = Journal::create(&path, Uuid::new_v4()).unwrap();
        journal
            .append(JournalEventKind::RunStarted {
                request: TaskRequest::from_objective(objective, dir.to_path_buf()),
                mode: CognitiveMode::DirectAnswer,
            })
            .unwrap();
        journal
            .append(JournalEventKind::AnswerProduced {
                answer: AgentAnswer {
                    schema_version: crate::domain::ANSWER_SCHEMA_VERSION,
                    status: AnswerStatus::Completed,
                    conclusion: "ok".into(),
                    claims: Vec::new(),
                    evidence: Vec::new(),
                    assumptions: Vec::new(),
                    constraints: Vec::new(),
                    unknowns: Vec::new(),
                    conflicts: Vec::new(),
                    flip_conditions: Vec::new(),
                    action_receipts: Vec::new(),
                    task_need: None,
                },
            })
            .unwrap();
        journal
            .append(JournalEventKind::RunStopped {
                reason: crate::domain::StopReason::Completed,
            })
            .unwrap();
        path
    }

    #[test]
    fn candidate_status_is_append_only_and_latest_wins() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("candidates.jsonl");
        let mut store = CandidateStore::open(&path).unwrap();
        let mut candidate = MemoryCandidate::new(
            MemoryKind::Preference,
            MemoryScope::bot("default"),
            "editor=pnpm",
            MemorySourceKind::UserMessage,
            vec!["run:1".into()],
            10,
        );
        candidate.confidence = 80;
        let candidate = store.propose(candidate).unwrap();
        let rejected = store
            .reject(&candidate.candidate_id, "alice", Some("wrong"), 20)
            .unwrap();
        assert_eq!(rejected.status, CandidateStatus::Rejected);
        drop(store);
        let reopened = CandidateStore::open(&path).unwrap();
        assert_eq!(
            reopened.get(&candidate.candidate_id).unwrap().status,
            CandidateStatus::Rejected
        );
        assert_eq!(reopened.proposed().count(), 0);
    }

    #[test]
    fn recall_prefix_is_stripped_from_candidate_content() {
        assert_eq!(normalize_content("记住：editor=pnpm"), "editor=pnpm");
        assert_eq!(normalize_content("  editor=pnpm  "), "editor=pnpm");
    }

    #[test]
    fn secret_candidates_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = CandidateStore::open(dir.path().join("candidates.jsonl")).unwrap();
        let candidate = MemoryCandidate::new(
            MemoryKind::Fact,
            MemoryScope::bot("default"),
            "password=hunter2",
            MemorySourceKind::UserMessage,
            vec!["run:1".into()],
            10,
        );
        assert!(matches!(
            store.propose(candidate),
            Err(CandidateError::Secret)
        ));
        assert!(contains_secret("sk-1234567890abcdef"));
        assert!(!contains_secret("editor=pnpm"));
    }

    #[test]
    fn imports_text_lines_as_untrusted_candidates_with_provenance() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.md");
        std::fs::write(
            &path,
            "客户=ACME\nshort\npassword=hunter2\n2026-11-01 会议：决定采用 release/1.0\n",
        )
        .unwrap();
        let candidates =
            import_path(&path, MemoryKind::Entity, &MemoryScope::bot("default"), 10).unwrap();
        assert_eq!(candidates.len(), 2);
        assert!(candidates.iter().all(|candidate| {
            candidate.kind == MemoryKind::Entity
                && candidate.taint == MemoryTaint::Untrusted
                && candidate.source_kind == MemorySourceKind::ToolObservation
        }));
        assert!(candidates[0].source_event_ids[0].contains("notes.md:1"));
        assert!(candidates[1].source_event_ids[0].contains("notes.md:4"));

        let imported = import_dir(
            dir.path(),
            MemoryKind::Fact,
            &MemoryScope::bot("default"),
            20,
        )
        .unwrap();
        assert_eq!(imported.len(), 2);
    }

    #[test]
    fn accepting_a_candidate_writes_active_memory_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let (journal, payloads, key) = paths(dir.path());
        let service = MemoryService::open(journal, payloads, &key).unwrap();
        let mut service = service;
        let mut store = CandidateStore::open(dir.path().join("candidates.jsonl")).unwrap();
        let candidate = store
            .propose(MemoryCandidate::new(
                MemoryKind::Preference,
                MemoryScope::bot("default"),
                "editor=pnpm",
                MemorySourceKind::UserMessage,
                vec!["run:1".into()],
                10,
            ))
            .unwrap();
        let record = accept(
            &mut store,
            &mut service,
            &candidate.candidate_id,
            "alice",
            20,
        )
        .unwrap();
        assert_eq!(record.content, "editor=pnpm");
        let again = accept(
            &mut store,
            &mut service,
            &candidate.candidate_id,
            "alice",
            30,
        )
        .unwrap();
        assert_eq!(again.memory_id, record.memory_id);
        assert_eq!(
            service
                .recall("editor", &MemoryScope::bot("default"), 30, 10)
                .len(),
            1
        );
    }

    #[test]
    fn extracts_explicit_user_preference_from_journal() {
        let dir = tempfile::tempdir().unwrap();
        let path = journal_with_objective(dir.path(), "记住：editor=pnpm");
        let candidates = extract_from_journal(&path, &MemoryScope::bot("default"), 20).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].kind, MemoryKind::Preference);
        assert!(candidates[0].content.contains("editor=pnpm"));
        assert!(!candidates[0].source_event_ids.is_empty());
        assert_eq!(candidates[0].status, CandidateStatus::Proposed);
    }
}
