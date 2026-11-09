use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::master::AssignmentProposal;
use crate::production_line::ProductionPlan;
use crate::safety::{contains_disallowed_control, is_disallowed_control};

const LEGACY_SESSION_VERSION: u8 = 1;
const SESSION_VERSION: u8 = 2;
const SESSION_EVENT_SCHEMA_VERSION: u32 = 1;
const MAX_SESSION_EVENT_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SessionRecord {
    version: u8,
    id: String,
    title: String,
    journals: Vec<PathBuf>,
    #[serde(default)]
    mission: Option<SessionMission>,
    #[serde(default)]
    archived: bool,
    #[serde(default)]
    preferences: Option<SessionPreferences>,
    #[serde(default)]
    context_summary: Option<SessionContextSummary>,
    #[serde(default)]
    workspace: Option<PathBuf>,
    #[serde(default)]
    mentions: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SessionPreferences {
    pub max_steps: usize,
    pub max_tool_calls: usize,
    pub max_provider_attempts: usize,
    pub max_context_chars: usize,
    #[serde(default = "default_auto_compact_percent")]
    pub auto_compact_percent: u8,
    pub timeout_seconds: u64,
}

impl SessionPreferences {
    pub fn validate(&self) -> Result<(), TuiSessionError> {
        if !(1..=64).contains(&self.max_steps)
            || !(1..=64).contains(&self.max_tool_calls)
            || !(1..=16).contains(&self.max_provider_attempts)
            || !(1_000..=1_000_000).contains(&self.max_context_chars)
            || !(50..=95).contains(&self.auto_compact_percent)
            || !(1..=3_600).contains(&self.timeout_seconds)
        {
            return Err(TuiSessionError::InvalidPreferences);
        }
        Ok(())
    }
}

fn default_auto_compact_percent() -> u8 {
    80
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SessionContextSummary {
    pub text: String,
    pub covered_journals: usize,
    pub created_unix_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub archived: bool,
    pub runs: usize,
    pub modified_unix_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SessionMission {
    pub id: String,
    pub goal: String,
    pub workspace: PathBuf,
    pub revision: u32,
    #[serde(default)]
    pub plan: Option<SessionPlan>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SessionPlan {
    pub path: PathBuf,
    pub plan_id: String,
    pub goal_id: String,
    pub sha256: String,
    #[serde(default)]
    pub approved_by: Option<String>,
    #[serde(default)]
    pub approved_at_unix_ms: Option<u128>,
    #[serde(default)]
    pub assignment: Option<AssignmentProposal>,
    #[serde(default)]
    pub master_trace_id: Option<String>,
    #[serde(default)]
    pub master_state_root: Option<PathBuf>,
    #[serde(default)]
    pub task_list_path: Option<PathBuf>,
    #[serde(default)]
    pub task_list_sha256: Option<String>,
    #[serde(default)]
    pub production_plan_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SessionEventRecord {
    pub schema_version: u32,
    pub seq: u64,
    pub timestamp_unix_ms: u128,
    pub event: SessionEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    Opened {
        host_kind: String,
        agent_id: Option<String>,
    },
    CommandAccepted {
        name: String,
        scope: String,
    },
    CommandRejected {
        name: String,
        reason: String,
    },
    TaskSubmitted {
        mode: String,
    },
    MissionStarted {
        mission_id: String,
    },
    MissionRevised {
        mission_id: String,
        revision: u32,
    },
    MissionClosed {
        mission_id: String,
    },
    PlanBound {
        mission_id: String,
        plan_id: String,
        sha256: String,
    },
    PlanApproved {
        mission_id: String,
        plan_id: String,
        sha256: String,
        actor: String,
    },
    TaskListPrepared {
        plan_id: String,
        path: String,
        plan_sha256: String,
        markdown_sha256: String,
    },
    AssignmentBound {
        plan_id: String,
        task_count: usize,
    },
    DispatchPrepared {
        trace_id: String,
    },
    SessionRenamed {
        title: String,
    },
    SessionArchived {
        archived: bool,
    },
    SessionForked {
        source_id: String,
    },
    ContextCompacted {
        covered_journals: usize,
    },
    PreferencesChanged,
    WorkspaceChanged {
        path: String,
    },
    WorkspaceMentioned {
        path: String,
    },
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiSession {
    id: String,
    title: String,
    root: PathBuf,
    journals: Vec<PathBuf>,
    mission: Option<SessionMission>,
    archived: bool,
    preferences: Option<SessionPreferences>,
    context_summary: Option<SessionContextSummary>,
    workspace: Option<PathBuf>,
    mentions: Vec<PathBuf>,
    next_event_seq: u64,
}

impl TuiSession {
    pub fn open(
        journal_dir: impl AsRef<Path>,
        requested_id: Option<&str>,
    ) -> Result<Self, TuiSessionError> {
        let journal_dir = journal_dir.as_ref();
        let id = requested_id
            .map(ToOwned::to_owned)
            .unwrap_or_else(new_session_id);
        validate_id(&id)?;
        let root = journal_dir.join(".sessions").join(&id);
        std::fs::create_dir_all(&root)?;
        let path = root.join("session.json");
        if !path.exists() {
            let session = Self {
                id,
                title: "New session".to_string(),
                root,
                journals: Vec::new(),
                mission: None,
                archived: false,
                preferences: None,
                context_summary: None,
                workspace: None,
                mentions: Vec::new(),
                next_event_seq: 0,
            };
            session.save()?;
            return Ok(session);
        }

        let bytes = std::fs::read(&path)?;
        let record: SessionRecord =
            serde_json::from_slice(&bytes).map_err(TuiSessionError::Json)?;
        if !matches!(record.version, LEGACY_SESSION_VERSION | SESSION_VERSION) {
            return Err(TuiSessionError::UnsupportedVersion(record.version));
        }
        if record.id != id {
            return Err(TuiSessionError::IdentityMismatch);
        }
        let next_event_seq = last_event_seq(&root.join("events.jsonl"))?;
        if let Some(mission) = record.mission.as_ref() {
            validate_mission_binding(&root, mission)?;
        }
        Ok(Self {
            id: record.id,
            title: record.title,
            root,
            journals: record.journals,
            mission: record.mission,
            archived: record.archived,
            preferences: record.preferences,
            context_summary: record.context_summary,
            workspace: record.workspace,
            mentions: record.mentions,
            next_event_seq,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn journals(&self) -> &[PathBuf] {
        &self.journals
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn mission(&self) -> Option<&SessionMission> {
        self.mission.as_ref()
    }

    pub fn archived(&self) -> bool {
        self.archived
    }

    pub fn preferences(&self) -> Option<&SessionPreferences> {
        self.preferences.as_ref()
    }

    pub fn context_summary(&self) -> Option<&SessionContextSummary> {
        self.context_summary.as_ref()
    }

    pub fn workspace(&self) -> Option<&Path> {
        self.workspace.as_deref()
    }

    pub fn mentions(&self) -> &[PathBuf] {
        &self.mentions
    }

    pub fn journal_root(&self) -> PathBuf {
        self.root
            .parent()
            .and_then(Path::parent)
            .unwrap_or(&self.root)
            .to_path_buf()
    }

    pub fn rename(&mut self, title: impl Into<String>) -> Result<(), TuiSessionError> {
        let title = title.into();
        let title = title.trim();
        if title.is_empty() || title.chars().count() > 80 || contains_disallowed_control(title) {
            return Err(TuiSessionError::InvalidTitle);
        }
        self.title = title.to_string();
        self.save()?;
        self.record_event(SessionEvent::SessionRenamed {
            title: self.title.clone(),
        })
    }

    pub fn set_archived(&mut self, archived: bool) -> Result<(), TuiSessionError> {
        self.archived = archived;
        self.save()?;
        self.record_event(SessionEvent::SessionArchived { archived })
    }

    pub fn set_preferences(
        &mut self,
        preferences: Option<SessionPreferences>,
    ) -> Result<(), TuiSessionError> {
        if let Some(preferences) = preferences.as_ref() {
            preferences.validate()?;
        }
        self.preferences = preferences;
        self.save()?;
        self.record_event(SessionEvent::PreferencesChanged)
    }

    pub fn set_context_summary(
        &mut self,
        text: impl Into<String>,
        covered_journals: usize,
        created_unix_ms: u128,
    ) -> Result<(), TuiSessionError> {
        let text = text.into();
        if text.trim().is_empty() || text.chars().count() > 8_000 {
            return Err(TuiSessionError::InvalidContextSummary);
        }
        self.context_summary = Some(SessionContextSummary {
            text,
            covered_journals,
            created_unix_ms,
        });
        self.save()?;
        self.record_event(SessionEvent::ContextCompacted { covered_journals })
    }

    pub fn set_workspace(&mut self, path: PathBuf) -> Result<(), TuiSessionError> {
        let path = std::fs::canonicalize(path)?;
        if !path.is_dir() {
            return Err(TuiSessionError::InvalidWorkspace);
        }
        self.workspace = Some(path.clone());
        self.save()?;
        self.record_event(SessionEvent::WorkspaceChanged {
            path: path.display().to_string(),
        })
    }

    pub fn add_mention(&mut self, path: PathBuf) -> Result<(), TuiSessionError> {
        let path = std::fs::canonicalize(path)?;
        if !self.mentions.contains(&path) {
            self.mentions.push(path.clone());
            self.save()?;
            self.record_event(SessionEvent::WorkspaceMentioned {
                path: path.display().to_string(),
            })?;
        }
        Ok(())
    }

    pub fn fork(&self, title: Option<&str>) -> Result<Self, TuiSessionError> {
        let id = new_session_id();
        let root = self.journal_root().join(".sessions").join(&id);
        std::fs::create_dir_all(&root)?;
        let title = title
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("{} fork", self.title))
            .chars()
            .take(80)
            .collect::<String>();
        if contains_disallowed_control(&title) {
            return Err(TuiSessionError::InvalidTitle);
        }
        let mut forked = Self {
            id,
            title,
            root,
            journals: self.journals.clone(),
            mission: None,
            archived: false,
            preferences: self.preferences.clone(),
            context_summary: self.context_summary.clone(),
            workspace: self.workspace.clone(),
            mentions: self.mentions.clone(),
            next_event_seq: 0,
        };
        forked.record_event(SessionEvent::SessionForked {
            source_id: self.id.clone(),
        })?;
        forked.save()?;
        Ok(forked)
    }

    pub fn trash(&self) -> Result<PathBuf, TuiSessionError> {
        let trash = self.journal_root().join(".trash");
        std::fs::create_dir_all(&trash)?;
        let target = trash.join(format!("{}-{}", self.id, unix_ms()));
        std::fs::rename(&self.root, &target)?;
        Ok(target)
    }

    pub fn list(journal_dir: impl AsRef<Path>) -> Result<Vec<SessionSummary>, TuiSessionError> {
        let directory = journal_dir.as_ref().join(".sessions");
        if !directory.exists() {
            return Ok(Vec::new());
        }
        let mut sessions = Vec::new();
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path().join("session.json");
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            if bytes.len() > 4 * 1024 * 1024 {
                continue;
            }
            let Ok(record) = serde_json::from_slice::<SessionRecord>(&bytes) else {
                continue;
            };
            if !matches!(record.version, LEGACY_SESSION_VERSION | SESSION_VERSION)
                || !valid_session_id(&record.id)
            {
                continue;
            }
            sessions.push(SessionSummary {
                id: record.id,
                title: record.title,
                archived: record.archived,
                runs: record.journals.len(),
                modified_unix_ms: entry
                    .metadata()
                    .ok()
                    .and_then(|metadata| metadata.modified().ok())
                    .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                    .map(|duration| duration.as_millis())
                    .unwrap_or_default(),
            });
        }
        sessions.sort_by(|left, right| {
            right
                .modified_unix_ms
                .cmp(&left.modified_unix_ms)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(sessions)
    }

    pub fn start_mission(
        &mut self,
        goal: impl Into<String>,
        workspace: PathBuf,
    ) -> Result<&SessionMission, TuiSessionError> {
        let goal = goal.into();
        validate_mission_text(&goal)?;
        let workspace = std::fs::canonicalize(workspace)?;
        let mission_id = format!("mission-{}", Uuid::new_v4());
        self.mission = Some(SessionMission {
            id: mission_id.clone(),
            goal,
            workspace,
            revision: 1,
            plan: None,
        });
        self.save()?;
        self.record_event(SessionEvent::MissionStarted { mission_id })?;
        Ok(self.mission.as_ref().expect("mission was just assigned"))
    }

    pub fn revise_mission(&mut self, goal: impl Into<String>) -> Result<(), TuiSessionError> {
        let goal = goal.into();
        validate_mission_text(&goal)?;
        let mission = self
            .mission
            .as_mut()
            .ok_or(TuiSessionError::MissingMission)?;
        if mission
            .plan
            .as_ref()
            .is_some_and(|plan| plan.approved_by.is_some())
        {
            return Err(TuiSessionError::MissionAlreadyApproved);
        }
        mission.goal = goal;
        mission.revision = mission.revision.saturating_add(1);
        mission.plan = None;
        let mission_id = mission.id.clone();
        let revision = mission.revision;
        self.save()?;
        self.record_event(SessionEvent::MissionRevised {
            mission_id,
            revision,
        })?;
        Ok(())
    }

    pub fn close_mission(&mut self) -> Result<(), TuiSessionError> {
        let mission = self.mission.take().ok_or(TuiSessionError::MissingMission)?;
        self.save()?;
        self.record_event(SessionEvent::MissionClosed {
            mission_id: mission.id,
        })?;
        Ok(())
    }

    pub fn bind_plan(
        &mut self,
        path: PathBuf,
        plan_id: impl Into<String>,
        goal_id: impl Into<String>,
        sha256: impl Into<String>,
    ) -> Result<(), TuiSessionError> {
        let sha256 = sha256.into();
        validate_sha256(&sha256)?;
        let plan_id = plan_id.into();
        let path = std::fs::canonicalize(path)?;
        let root = std::fs::canonicalize(&self.root)?;
        if !path.starts_with(&root) {
            return Err(TuiSessionError::PlanOutsideSession);
        }
        let actual_digest = digest_file(&path)?;
        if actual_digest != sha256 {
            return Err(TuiSessionError::PlanDigestMismatch);
        }
        let mission = self
            .mission
            .as_mut()
            .ok_or(TuiSessionError::MissingMission)?;
        mission.plan = Some(SessionPlan {
            path,
            plan_id: plan_id.clone(),
            goal_id: goal_id.into(),
            sha256: sha256.clone(),
            approved_by: None,
            approved_at_unix_ms: None,
            assignment: None,
            master_trace_id: None,
            master_state_root: None,
            task_list_path: None,
            task_list_sha256: None,
            production_plan_sha256: None,
        });
        let mission_id = mission.id.clone();
        self.save()?;
        self.record_event(SessionEvent::PlanBound {
            mission_id,
            plan_id,
            sha256,
        })?;
        Ok(())
    }

    pub fn approve_plan(
        &mut self,
        actor: impl Into<String>,
        expected_sha256: &str,
        approved_at_unix_ms: u128,
    ) -> Result<(), TuiSessionError> {
        let actor = actor.into();
        if actor.trim().is_empty() || contains_disallowed_control(&actor) {
            return Err(TuiSessionError::UnsafeEvent);
        }
        let mission = self
            .mission
            .as_ref()
            .ok_or(TuiSessionError::MissingMission)?;
        let plan = mission.plan.as_ref().ok_or(TuiSessionError::MissingPlan)?;
        if plan.sha256 != expected_sha256 {
            return Err(TuiSessionError::PlanDigestMismatch);
        }
        let mission_id = mission.id.clone();
        let plan_id = plan.plan_id.clone();
        let sha256 = plan.sha256.clone();
        let plan = &mut self
            .mission
            .as_mut()
            .expect("mission was checked above")
            .plan;
        let plan = plan.as_mut().expect("plan was checked above");
        plan.approved_by = Some(actor.clone());
        plan.approved_at_unix_ms = Some(approved_at_unix_ms);
        self.save()?;
        self.record_event(SessionEvent::PlanApproved {
            mission_id,
            plan_id,
            sha256,
            actor,
        })?;
        Ok(())
    }

    pub fn bind_task_list(
        &mut self,
        path: PathBuf,
        production_plan_sha256: impl Into<String>,
        markdown_sha256: impl Into<String>,
    ) -> Result<(), TuiSessionError> {
        let production_plan_sha256 = production_plan_sha256.into();
        let markdown_sha256 = markdown_sha256.into();
        validate_sha256(&production_plan_sha256)?;
        validate_sha256(&markdown_sha256)?;
        let path = std::fs::canonicalize(path)?;
        let root = std::fs::canonicalize(&self.root)?;
        if !path.starts_with(&root) {
            return Err(TuiSessionError::PlanOutsideSession);
        }
        if digest_file(&path)? != markdown_sha256 {
            return Err(TuiSessionError::PlanDigestMismatch);
        }
        let mission = self
            .mission
            .as_mut()
            .ok_or(TuiSessionError::MissingMission)?;
        let plan = mission.plan.as_mut().ok_or(TuiSessionError::MissingPlan)?;
        if plan.approved_by.is_none() {
            return Err(TuiSessionError::DispatchNotReady);
        }
        let plan_id = plan.plan_id.clone();
        let path_text = path.display().to_string();
        plan.task_list_path = Some(path);
        plan.task_list_sha256 = Some(markdown_sha256.clone());
        plan.production_plan_sha256 = Some(production_plan_sha256.clone());
        self.save()?;
        self.record_event(SessionEvent::TaskListPrepared {
            plan_id,
            path: path_text,
            plan_sha256: production_plan_sha256,
            markdown_sha256,
        })?;
        Ok(())
    }

    pub fn bind_assignment(
        &mut self,
        assignment: AssignmentProposal,
    ) -> Result<(), TuiSessionError> {
        let mission = self
            .mission
            .as_mut()
            .ok_or(TuiSessionError::MissingMission)?;
        let plan = mission.plan.as_mut().ok_or(TuiSessionError::MissingPlan)?;
        if assignment.plan_id != plan.plan_id {
            return Err(TuiSessionError::AssignmentPlanMismatch);
        }
        let task_count = assignment.assignments.len();
        let plan_id = plan.plan_id.clone();
        plan.assignment = Some(assignment);
        self.save()?;
        self.record_event(SessionEvent::AssignmentBound {
            plan_id,
            task_count,
        })?;
        Ok(())
    }

    pub fn bind_dispatch(
        &mut self,
        trace_id: impl Into<String>,
        state_root: PathBuf,
    ) -> Result<(), TuiSessionError> {
        let trace_id = trace_id.into();
        if trace_id.trim().is_empty() || contains_disallowed_control(&trace_id) {
            return Err(TuiSessionError::UnsafeEvent);
        }
        let root = std::fs::canonicalize(&self.root)?;
        std::fs::create_dir_all(&state_root)?;
        let state_root = std::fs::canonicalize(state_root)?;
        if !state_root.starts_with(root) {
            return Err(TuiSessionError::PlanOutsideSession);
        }
        let mission = self
            .mission
            .as_mut()
            .ok_or(TuiSessionError::MissingMission)?;
        let plan = mission.plan.as_mut().ok_or(TuiSessionError::MissingPlan)?;
        if plan.approved_by.is_none() || plan.assignment.is_none() {
            return Err(TuiSessionError::DispatchNotReady);
        }
        plan.master_trace_id = Some(trace_id.clone());
        plan.master_state_root = Some(state_root);
        self.save()?;
        self.record_event(SessionEvent::DispatchPrepared { trace_id })?;
        Ok(())
    }

    pub fn set_title_from_prompt(&mut self, prompt: &str) {
        if self.title != "New session" {
            return;
        }
        let title = prompt
            .chars()
            .filter(|character| !is_disallowed_control(*character))
            .take(80)
            .collect::<String>()
            .trim()
            .to_string();
        if !title.is_empty() {
            self.title = title;
        }
    }

    pub fn record_journal(&mut self, journal: impl AsRef<Path>) -> Result<(), TuiSessionError> {
        let journal = journal.as_ref().to_path_buf();
        if !self.journals.contains(&journal) {
            self.journals.push(journal);
            self.save()?;
        }
        Ok(())
    }

    pub fn record_event(&mut self, event: SessionEvent) -> Result<(), TuiSessionError> {
        validate_event(&event)?;
        let record = SessionEventRecord {
            schema_version: SESSION_EVENT_SCHEMA_VERSION,
            seq: self.next_event_seq,
            timestamp_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_millis())
                .unwrap_or_default(),
            event,
        };
        let mut bytes = serde_json::to_vec(&record).map_err(TuiSessionError::Json)?;
        bytes.push(b'\n');
        let path = self.root.join("events.jsonl");
        let mut options = std::fs::OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path)?;
        if file.metadata()?.len().saturating_add(bytes.len() as u64) > MAX_SESSION_EVENT_BYTES {
            return Err(TuiSessionError::EventLogTooLarge);
        }
        std::io::Write::write_all(&mut file, &bytes)?;
        file.sync_data()?;
        self.next_event_seq = self.next_event_seq.saturating_add(1);
        Ok(())
    }

    pub fn continue_command(&self) -> String {
        format!("1agent tui --session {}", self.id)
    }

    fn save(&self) -> Result<(), TuiSessionError> {
        let record = SessionRecord {
            version: SESSION_VERSION,
            id: self.id.clone(),
            title: self.title.clone(),
            journals: self.journals.clone(),
            mission: self.mission.clone(),
            archived: self.archived,
            preferences: self.preferences.clone(),
            context_summary: self.context_summary.clone(),
            workspace: self.workspace.clone(),
            mentions: self.mentions.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&record).map_err(TuiSessionError::Json)?;
        let path = self.root.join("session.json");
        let mut temporary = tempfile::NamedTempFile::new_in(&self.root)?;
        std::io::Write::write_all(&mut temporary, &bytes)?;
        temporary
            .persist(&path)
            .map_err(|error| TuiSessionError::Io(error.error))?;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TuiSessionError {
    #[error("invalid session id")]
    InvalidId,
    #[error("session identity mismatch")]
    IdentityMismatch,
    #[error("unsupported session schema version {0}")]
    UnsupportedVersion(u8),
    #[error("session io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("session json error: {0}")]
    Json(serde_json::Error),
    #[error("session event log exceeds the size limit")]
    EventLogTooLarge,
    #[error("session event log is malformed")]
    InvalidEventLog,
    #[error("session event contains hidden control characters")]
    UnsafeEvent,
    #[error("session has no active mission")]
    MissingMission,
    #[error("session mission has no bound plan")]
    MissingPlan,
    #[error("approved mission cannot be revised")]
    MissionAlreadyApproved,
    #[error("plan digest does not match the approved digest")]
    PlanDigestMismatch,
    #[error("plan path escapes the session directory")]
    PlanOutsideSession,
    #[error("plan file exceeds the size limit")]
    PlanFileTooLarge,
    #[error("production-line plan is invalid: {0}")]
    InvalidProductionPlan(String),
    #[error("assignment plan id does not match the active plan")]
    AssignmentPlanMismatch,
    #[error("dispatch requires an approved plan and assignment")]
    DispatchNotReady,
    #[error("session title is invalid")]
    InvalidTitle,
    #[error("session preferences are outside the supported bounds")]
    InvalidPreferences,
    #[error("context summary is empty or exceeds the size limit")]
    InvalidContextSummary,
    #[error("session workspace must be an existing directory")]
    InvalidWorkspace,
}

fn new_session_id() -> String {
    format!("ses_{}", Uuid::new_v4().simple())
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

pub(crate) fn valid_session_id(id: &str) -> bool {
    let (valid_prefix, max_len) = if id.starts_with("ses_") {
        (true, 80)
    } else if id.starts_with("term-") {
        (true, 120)
    } else {
        (false, 0)
    };
    valid_prefix
        && id.len() >= 12
        && id.len() <= max_len
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
}

fn validate_id(id: &str) -> Result<(), TuiSessionError> {
    if id.starts_with("ses_") && valid_session_id(id) {
        Ok(())
    } else {
        Err(TuiSessionError::InvalidId)
    }
}

fn last_event_seq(path: &Path) -> Result<u64, TuiSessionError> {
    if !path.exists() {
        return Ok(0);
    }
    let file = std::fs::File::open(path)?;
    if file.metadata()?.len() > MAX_SESSION_EVENT_BYTES {
        return Err(TuiSessionError::EventLogTooLarge);
    }
    let mut expected = 0;
    for line in std::io::BufRead::lines(std::io::BufReader::new(file)) {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let record: SessionEventRecord =
            serde_json::from_str(&line).map_err(TuiSessionError::Json)?;
        if record.schema_version != SESSION_EVENT_SCHEMA_VERSION || record.seq != expected {
            return Err(TuiSessionError::InvalidEventLog);
        }
        expected = expected.saturating_add(1);
    }
    Ok(expected)
}

fn validate_event(event: &SessionEvent) -> Result<(), TuiSessionError> {
    let unsafe_text = match event {
        SessionEvent::Opened {
            host_kind,
            agent_id,
        } => {
            contains_disallowed_control(host_kind)
                || agent_id.as_deref().is_some_and(contains_disallowed_control)
        }
        SessionEvent::CommandAccepted { name, scope } => {
            contains_disallowed_control(name) || contains_disallowed_control(scope)
        }
        SessionEvent::CommandRejected { name, reason } => {
            contains_disallowed_control(name) || contains_disallowed_control(reason)
        }
        SessionEvent::TaskSubmitted { mode } => contains_disallowed_control(mode),
        SessionEvent::MissionStarted { mission_id }
        | SessionEvent::MissionClosed { mission_id } => contains_disallowed_control(mission_id),
        SessionEvent::MissionRevised {
            mission_id,
            revision: _,
        } => contains_disallowed_control(mission_id),
        SessionEvent::PlanBound {
            mission_id,
            plan_id,
            sha256,
        } => {
            contains_disallowed_control(mission_id)
                || contains_disallowed_control(plan_id)
                || contains_disallowed_control(sha256)
        }
        SessionEvent::PlanApproved {
            mission_id,
            plan_id,
            sha256,
            actor,
        } => {
            contains_disallowed_control(mission_id)
                || contains_disallowed_control(plan_id)
                || contains_disallowed_control(sha256)
                || contains_disallowed_control(actor)
        }
        SessionEvent::TaskListPrepared {
            plan_id,
            path,
            plan_sha256,
            markdown_sha256,
        } => {
            contains_disallowed_control(plan_id)
                || contains_disallowed_control(path)
                || validate_sha256(plan_sha256).is_err()
                || validate_sha256(markdown_sha256).is_err()
        }
        SessionEvent::AssignmentBound {
            plan_id,
            task_count: _,
        } => contains_disallowed_control(plan_id),
        SessionEvent::DispatchPrepared { trace_id } => contains_disallowed_control(trace_id),
        SessionEvent::SessionRenamed { title } => contains_disallowed_control(title),
        SessionEvent::SessionForked { source_id } => contains_disallowed_control(source_id),
        SessionEvent::WorkspaceChanged { path } | SessionEvent::WorkspaceMentioned { path } => {
            contains_disallowed_control(path)
        }
        SessionEvent::SessionArchived { .. }
        | SessionEvent::ContextCompacted { .. }
        | SessionEvent::PreferencesChanged => false,
        SessionEvent::Closed => false,
    };
    if unsafe_text {
        Err(TuiSessionError::UnsafeEvent)
    } else {
        Ok(())
    }
}

fn validate_mission_text(goal: &str) -> Result<(), TuiSessionError> {
    if goal.trim().is_empty() || contains_disallowed_control(goal) {
        Err(TuiSessionError::UnsafeEvent)
    } else {
        Ok(())
    }
}

fn validate_sha256(value: &str) -> Result<(), TuiSessionError> {
    if value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(TuiSessionError::UnsafeEvent)
    }
}

fn validate_mission_binding(root: &Path, mission: &SessionMission) -> Result<(), TuiSessionError> {
    let root = std::fs::canonicalize(root)?;
    let workspace = std::fs::canonicalize(&mission.workspace)?;
    if !workspace.is_dir() {
        return Err(TuiSessionError::Io(std::io::Error::new(
            std::io::ErrorKind::NotADirectory,
            "mission workspace is not a directory",
        )));
    }
    if let Some(plan) = &mission.plan {
        let path = std::fs::canonicalize(&plan.path)?;
        if !path.starts_with(&root) {
            return Err(TuiSessionError::PlanOutsideSession);
        }
        if digest_file(&path)? != plan.sha256 {
            return Err(TuiSessionError::PlanDigestMismatch);
        }
        if let Some(assignment) = &plan.assignment
            && assignment.plan_id != plan.plan_id
        {
            return Err(TuiSessionError::AssignmentPlanMismatch);
        }
        if let Some(state_root) = &plan.master_state_root {
            let state_root = std::fs::canonicalize(state_root)?;
            if !state_root.starts_with(&root) {
                return Err(TuiSessionError::PlanOutsideSession);
            }
        }
        if let Some(task_list_path) = &plan.task_list_path {
            let task_list_path = std::fs::canonicalize(task_list_path)?;
            if !task_list_path.starts_with(&root) {
                return Err(TuiSessionError::PlanOutsideSession);
            }
            let expected = plan
                .task_list_sha256
                .as_deref()
                .ok_or(TuiSessionError::PlanDigestMismatch)?;
            if digest_file(&task_list_path)? != expected {
                return Err(TuiSessionError::PlanDigestMismatch);
            }
            if plan.production_plan_sha256.as_deref().is_none() {
                return Err(TuiSessionError::PlanDigestMismatch);
            }
            let plan_bytes = std::fs::read(&path)?;
            if plan_bytes.len() > 4 * 1024 * 1024 {
                return Err(TuiSessionError::PlanFileTooLarge);
            }
            let production: ProductionPlan = serde_json::from_slice(&plan_bytes)
                .map_err(|error| TuiSessionError::InvalidProductionPlan(error.to_string()))?;
            let actual_plan_sha = production
                .plan_sha256()
                .map_err(|error| TuiSessionError::InvalidProductionPlan(error.to_string()))?;
            if plan.production_plan_sha256.as_deref() != Some(actual_plan_sha.as_str()) {
                return Err(TuiSessionError::PlanDigestMismatch);
            }
        }
    }
    Ok(())
}

fn digest_file(path: &Path) -> Result<String, TuiSessionError> {
    let bytes = std::fs::read(path)?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err(TuiSessionError::PlanFileTooLarge);
    }
    Ok(hex::encode(Sha256::digest(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{PermissionSet, RiskLevel};
    use crate::master::{
        MASTER_PLAN_SCHEMA_VERSION, PlanProposal, WorkBudget, WorkItem, WorkItemStatus,
    };
    use crate::production_line::{GoalContract, KeyResult, PRODUCTION_LINE_SCHEMA_VERSION};

    fn write_production_plan(path: &Path, workspace: &Path) -> ProductionPlan {
        let production = ProductionPlan {
            schema_version: PRODUCTION_LINE_SCHEMA_VERSION,
            goal: GoalContract {
                goal_id: "goal-1".into(),
                master_conversation_id: "master-1".into(),
                objective: "Deliver verified output".into(),
                non_goals: Vec::new(),
                constraints: Vec::new(),
                key_results: vec![KeyResult {
                    id: "kr-1".into(),
                    statement: "verified output exists".into(),
                    baseline: "missing".into(),
                    target: "verified".into(),
                    measurement_method: "artifact check".into(),
                    work_item_ids: vec!["deliver".into()],
                }],
            },
            plan: PlanProposal {
                schema_version: MASTER_PLAN_SCHEMA_VERSION,
                plan_id: "plan-1".into(),
                goal_id: "goal-1".into(),
                version: 1,
                summary: "Deliver verified output".into(),
                work_items: vec![WorkItem {
                    id: "deliver".into(),
                    title: "Deliver".into(),
                    objective: "Deliver verified output".into(),
                    dependencies: Vec::new(),
                    required_capabilities: vec!["general".into()],
                    required_tools: vec!["shell".into()],
                    permissions: PermissionSet::default(),
                    acceptance_criteria: vec!["output verified".into()],
                    expected_artifacts: vec!["result.txt".into()],
                    workspace: workspace.to_path_buf(),
                    budget: WorkBudget::default(),
                    deadline_unix_ms: None,
                    risk: RiskLevel::Low,
                    status: WorkItemStatus::Pending,
                }],
                max_parallel: 1,
                approved: false,
            },
        };
        std::fs::write(path, serde_json::to_vec_pretty(&production).unwrap()).unwrap();
        production
    }

    #[test]
    fn session_round_trips_title_and_journals() {
        let workspace = tempfile::tempdir().unwrap();
        let mut session = TuiSession::open(workspace.path(), None).unwrap();
        assert!(session.id().starts_with("ses_"));
        session.set_title_from_prompt("Compare A and B");
        let journal = workspace.path().join("run.jsonl");
        session.record_journal(&journal).unwrap();

        let reopened = TuiSession::open(workspace.path(), Some(session.id())).unwrap();
        assert_eq!(reopened.title(), "Compare A and B");
        assert_eq!(reopened.journals(), &[journal]);
        assert_eq!(
            reopened.continue_command(),
            format!("1agent tui --session {}", session.id())
        );
    }

    #[test]
    fn legacy_v1_session_remains_readable() {
        let workspace = tempfile::tempdir().unwrap();
        let id = "ses_legacy000001";
        let root = workspace.path().join(".sessions").join(id);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("session.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "version": 1,
                "id": id,
                "title": "Legacy",
                "journals": []
            }))
            .unwrap(),
        )
        .unwrap();

        let session = TuiSession::open(workspace.path(), Some(id)).unwrap();
        assert_eq!(session.title(), "Legacy");
        assert!(session.journals().is_empty());
    }

    #[test]
    fn version_two_preferences_without_auto_compaction_use_the_safe_default() {
        let workspace = tempfile::tempdir().unwrap();
        let id = "ses_legacycompact01";
        let root = workspace.path().join(".sessions").join(id);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("session.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "version": 2,
                "id": id,
                "title": "Legacy compact",
                "journals": [],
                "preferences": {
                    "max_steps": 4,
                    "max_tool_calls": 5,
                    "max_provider_attempts": 3,
                    "max_context_chars": 20_000,
                    "timeout_seconds": 60
                }
            }))
            .unwrap(),
        )
        .unwrap();
        let session = TuiSession::open(workspace.path(), Some(id)).unwrap();
        assert_eq!(session.preferences().unwrap().auto_compact_percent, 80);
    }

    #[test]
    fn session_events_are_append_only_and_resume_sequence() {
        let workspace = tempfile::tempdir().unwrap();
        let mut session = TuiSession::open(workspace.path(), None).unwrap();
        session
            .record_event(SessionEvent::Opened {
                host_kind: "workshop-master".into(),
                agent_id: Some("main-coordinator".into()),
            })
            .unwrap();
        session
            .record_event(SessionEvent::TaskSubmitted {
                mode: "plan".into(),
            })
            .unwrap();

        let path = session.root.join("events.jsonl");
        let before = std::fs::read_to_string(&path).unwrap();
        assert_eq!(before.lines().count(), 2);

        let reopened = TuiSession::open(workspace.path(), Some(session.id())).unwrap();
        assert_eq!(reopened.next_event_seq, 2);
        drop(reopened);

        let mut reopened = TuiSession::open(workspace.path(), Some(session.id())).unwrap();
        reopened.record_event(SessionEvent::Closed).unwrap();
        let after = std::fs::read_to_string(&path).unwrap();
        assert!(after.starts_with(&before));
        assert!(after.lines().last().unwrap().contains("\"seq\":2"));
    }

    #[test]
    fn session_event_log_rejects_noncontiguous_sequences() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join(".sessions").join("ses_badseq00001");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("session.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "version": 2,
                "id": "ses_badseq00001",
                "title": "Bad",
                "journals": []
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            root.join("events.jsonl"),
            "{\"schema_version\":1,\"seq\":1,\"timestamp_unix_ms\":1,\"event\":{\"type\":\"closed\"}}\n",
        )
        .unwrap();

        assert!(matches!(
            TuiSession::open(workspace.path(), Some("ses_badseq00001")),
            Err(TuiSessionError::InvalidEventLog)
        ));
    }

    #[test]
    fn session_event_log_rejects_oversize_files() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().join(".sessions").join("ses_oversize0001");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("session.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "version": 2,
                "id": "ses_oversize0001",
                "title": "Oversize",
                "journals": []
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            root.join("events.jsonl"),
            vec![b' '; MAX_SESSION_EVENT_BYTES as usize + 1],
        )
        .unwrap();

        assert!(matches!(
            TuiSession::open(workspace.path(), Some("ses_oversize0001")),
            Err(TuiSessionError::EventLogTooLarge)
        ));
    }

    #[test]
    fn mission_plan_binding_and_approval_are_persistent_and_digest_bound() {
        let workspace = tempfile::tempdir().unwrap();
        let mut session = TuiSession::open(workspace.path(), None).unwrap();
        let mission = session
            .start_mission("Deliver verified output", workspace.path().to_path_buf())
            .unwrap();
        assert_eq!(mission.revision, 1);
        let plan_path = session.root().join("plan.json");
        let production = write_production_plan(&plan_path, workspace.path());
        let plan_bytes = std::fs::read(&plan_path).unwrap();
        let digest = hex::encode(Sha256::digest(&plan_bytes));
        let production_plan_sha = production.plan_sha256().unwrap();
        session
            .bind_plan(plan_path, "plan-1", "goal-1", &digest)
            .unwrap();
        assert!(matches!(
            session.approve_plan("tester", &"b".repeat(64), 10),
            Err(TuiSessionError::PlanDigestMismatch)
        ));
        session.approve_plan("tester", &digest, 10).unwrap();
        let task_list = session.root().join("master-state/goal-1/v1/任务清单.md");
        std::fs::create_dir_all(task_list.parent().unwrap()).unwrap();
        std::fs::write(&task_list, "approved task list\n").unwrap();
        let task_list_digest = hex::encode(Sha256::digest(b"approved task list\n"));
        session
            .bind_task_list(
                task_list.clone(),
                production_plan_sha.clone(),
                task_list_digest,
            )
            .unwrap();

        let reopened = TuiSession::open(workspace.path(), Some(session.id())).unwrap();
        let mission = reopened.mission().unwrap();
        assert_eq!(mission.goal, "Deliver verified output");
        assert_eq!(
            mission
                .plan
                .as_ref()
                .and_then(|plan| plan.approved_by.as_deref()),
            Some("tester")
        );
        assert_eq!(
            mission
                .plan
                .as_ref()
                .and_then(|plan| plan.task_list_path.as_deref()),
            Some(task_list.as_path())
        );
        assert_eq!(
            mission
                .plan
                .as_ref()
                .and_then(|plan| plan.production_plan_sha256.as_deref()),
            Some(production_plan_sha.as_str())
        );
    }

    #[test]
    fn approved_mission_cannot_be_revised_without_closing() {
        let workspace = tempfile::tempdir().unwrap();
        let mut session = TuiSession::open(workspace.path(), None).unwrap();
        session
            .start_mission("First goal", workspace.path().to_path_buf())
            .unwrap();
        let plan_path = session.root().join("plan.json");
        std::fs::write(&plan_path, b"plan-c").unwrap();
        let digest = hex::encode(Sha256::digest(b"plan-c"));
        session
            .bind_plan(plan_path, "plan-1", "goal-1", &digest)
            .unwrap();
        session.approve_plan("tester", &digest, 10).unwrap();
        assert!(matches!(
            session.revise_mission("Second goal"),
            Err(TuiSessionError::MissionAlreadyApproved)
        ));
    }

    #[test]
    fn phase_six_session_state_round_trips_and_forks_safely() {
        let workspace = tempfile::tempdir().unwrap();
        let project = workspace.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        let mut session = TuiSession::open(workspace.path(), None).unwrap();
        session.rename("Release review").unwrap();
        session.set_workspace(project.clone()).unwrap();
        session.add_mention(project.join(".")).unwrap();
        session
            .set_preferences(Some(SessionPreferences {
                max_steps: 4,
                max_tool_calls: 5,
                max_provider_attempts: 3,
                max_context_chars: 20_000,
                auto_compact_percent: 75,
                timeout_seconds: 60,
            }))
            .unwrap();
        session
            .set_context_summary("Earlier work produced a reviewed release.", 0, 10)
            .unwrap();

        let reopened = TuiSession::open(workspace.path(), Some(session.id())).unwrap();
        assert_eq!(reopened.title(), "Release review");
        assert_eq!(reopened.workspace(), Some(project.as_path()));
        assert_eq!(reopened.mentions().len(), 1);
        assert_eq!(reopened.preferences().unwrap().max_context_chars, 20_000);
        assert_eq!(
            reopened.context_summary().unwrap().text,
            "Earlier work produced a reviewed release."
        );

        let mut forked = reopened.fork(Some("Release review fork")).unwrap();
        assert_eq!(forked.title(), "Release review fork");
        assert!(forked.mission().is_none());
        assert_eq!(forked.mentions(), reopened.mentions());
        let listed = TuiSession::list(workspace.path()).unwrap();
        assert!(listed.iter().any(|summary| summary.id == forked.id()));
        forked.set_archived(true).unwrap();
        assert!(
            TuiSession::list(workspace.path())
                .unwrap()
                .into_iter()
                .find(|summary| summary.id == forked.id())
                .unwrap()
                .archived
        );
    }

    #[test]
    fn invalid_preferences_and_workspace_are_rejected_and_delete_is_recoverable() {
        let workspace = tempfile::tempdir().unwrap();
        let session = TuiSession::open(workspace.path(), None).unwrap();
        let mut invalid = session.clone();
        assert!(matches!(
            invalid.set_preferences(Some(SessionPreferences {
                max_steps: 0,
                max_tool_calls: 1,
                max_provider_attempts: 1,
                max_context_chars: 1_000,
                auto_compact_percent: 80,
                timeout_seconds: 1,
            })),
            Err(TuiSessionError::InvalidPreferences)
        ));
        let mut invalid = session.clone();
        assert!(matches!(
            invalid.set_workspace(workspace.path().join("missing")),
            Err(TuiSessionError::Io(_))
        ));
        let trashed = session.trash().unwrap();
        assert!(trashed.is_dir());
        assert!(!session.root().exists());
        assert!(trashed.starts_with(workspace.path().join(".trash")));
    }

    #[test]
    fn rejects_unsafe_session_ids() {
        let workspace = tempfile::tempdir().unwrap();
        for id in ["../../escape", "ses_bad/id", "short", "ses_with space"] {
            assert!(matches!(
                TuiSession::open(workspace.path(), Some(id)),
                Err(TuiSessionError::InvalidId)
            ));
        }
    }
}
