//! Read-only cross-session conversation recall.

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use super::{AgentTool, ToolError, ToolOutcome};
use crate::domain::{PermissionSet, RiskLevel, ToolSpec};
use crate::runtime_home::home_or_legacy;
use crate::session_recall::{SessionRecallHit, age_label, recall_sessions};

const DEFAULT_LIMIT: usize = 5;
const MAX_QUERY_CHARS: usize = 500;

pub struct SessionSearchTool {
    journal_dir: PathBuf,
    current_session_id: Option<String>,
}

impl SessionSearchTool {
    pub fn new(journal_dir: impl Into<PathBuf>) -> Self {
        Self {
            journal_dir: journal_dir.into(),
            current_session_id: None,
        }
    }

    pub fn from_env() -> Self {
        Self::new(home_or_legacy("journals", ".journal"))
    }

    pub fn with_current_session(mut self, session_id: impl Into<String>) -> Self {
        self.current_session_id = Some(session_id.into());
        self
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionSearchArgs {
    query: String,
    #[serde(default)]
    limit: Option<usize>,
}

#[async_trait]
impl AgentTool for SessionSearchTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "session_search".into(),
            description: "Search local cross-session conversation records: earlier session titles, user requests and answer conclusions from the 1Agent session store. Read-only. Use when the user asks what you discussed, says they talked about something before, or asks whether you remember. Returns grounded snippets with session ids. An empty result means there is no local record: say so plainly and do not invent a memory. An empty query lists recent sessions.".into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["query"],
                "properties": {
                    "query": {
                        "type": "string",
                        "maxLength": MAX_QUERY_CHARS,
                        "description": "Topic, words or phrase to find in earlier local sessions. Pass an empty string to list recent sessions."
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 10,
                        "default": DEFAULT_LIMIT
                    }
                }
            }),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::Low
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(5)
    }

    async fn execute(
        &self,
        arguments: Value,
        _permissions: &PermissionSet,
        _cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        let args: SessionSearchArgs = serde_json::from_value(arguments)
            .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
        let query = args.query.trim().to_string();
        if query.chars().count() > MAX_QUERY_CHARS {
            return Err(ToolError::InvalidArguments(format!(
                "query exceeds {MAX_QUERY_CHARS} characters"
            )));
        }
        let limit = args.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, 10);
        let hits = recall_sessions(
            &self.journal_dir,
            self.current_session_id.as_deref(),
            &query,
            limit,
        )
        .map_err(|error| ToolError::Failed(error.to_string()))?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default();
        let matches = hits
            .iter()
            .map(|hit| hit_json(hit, now))
            .collect::<Vec<_>>();
        let note = if matches.is_empty() {
            "No local session record matched. Tell the user you have no record of it and ask for a topic or time; do not invent a memory."
        } else {
            "These are retrieved local session records, not model memory. Cite the session title and earlier request or conclusion. When the user asks whether you remember, give the concrete content from the evidence (the words, decision or next step), not only the topic. Do not add details that are not in the evidence."
        };
        Ok(ToolOutcome {
            summary: format!("session search: {} match(es)", matches.len()),
            content: json!({
                "query": query,
                "matches": matches,
                "note": note,
            })
            .to_string(),
            metadata: Default::default(),
        })
    }
}

fn hit_json(hit: &SessionRecallHit, now_unix_ms: u128) -> Value {
    json!({
        "session_id": hit.session_id,
        "title": hit.title,
        "last_used": age_label(hit.last_used_unix_ms, now_unix_ms),
        "runs": hit.runs,
        "score": hit.score,
        "matched_terms": hit.matched_terms,
        "evidence": hit.evidence.iter().map(|evidence| json!({
            "run_id": evidence.run_id,
            "timestamp_unix_ms": evidence.timestamp_unix_ms,
            "user_request": evidence.request,
            "answer_conclusion": evidence.conclusion,
            "status": evidence.status,
        })).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::domain::{AgentAnswer, AnswerStatus, CognitiveMode, TaskRequest};
    use crate::journal::{Journal, JournalEventKind};
    use uuid::Uuid;

    fn write_session(root: &Path) {
        let session_root = root.join(".sessions").join("ses_fg");
        std::fs::create_dir_all(&session_root).unwrap();
        let journal_path = root.join("run.jsonl");
        let mut journal = Journal::create(&journal_path, Uuid::new_v4()).unwrap();
        journal
            .append(JournalEventKind::RunStarted {
                request: TaskRequest::from_objective("要F开头的，G再中间的", root.to_path_buf()),
                mode: CognitiveMode::DirectAnswer,
            })
            .unwrap();
        journal
            .append(JournalEventKind::AnswerProduced {
                answer: AgentAnswer {
                    schema_version: crate::domain::ANSWER_SCHEMA_VERSION,
                    status: AnswerStatus::Completed,
                    conclusion: "flagship、foreground、forging、fulgent、foregleam".into(),
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
        let record = serde_json::json!({
            "version": 2,
            "id": "ses_fg",
            "title": "F 和 G 开头的英文单词",
            "journals": [journal_path],
            "archived": false,
            "mentions": [],
        });
        std::fs::write(
            session_root.join("session.json"),
            serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn searches_previous_sessions_and_excludes_the_current_one() {
        let directory = tempfile::tempdir().unwrap();
        write_session(directory.path());
        let tool = SessionSearchTool::new(directory.path());
        let outcome = tool
            .execute(
                json!({"query": "F开头和G开头"}),
                &PermissionSet::default(),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(outcome.content.contains("ses_fg"), "{}", outcome.content);
        assert!(outcome.content.contains("flagship"), "{}", outcome.content);
        assert!(
            outcome.content.contains("retrieved local session records"),
            "{}",
            outcome.content
        );

        let current = SessionSearchTool::new(directory.path()).with_current_session("ses_fg");
        let outcome = current
            .execute(
                json!({"query": "F开头和G开头"}),
                &PermissionSet::default(),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(!outcome.content.contains("ses_fg"), "{}", outcome.content);
        assert!(outcome.content.contains("no record"), "{}", outcome.content);
    }
}
