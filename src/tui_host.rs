use std::env;

use crate::tui_session::valid_session_id;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostKind {
    Standalone,
    WorkshopMaster,
    WorkshopWorker,
}

impl HostKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Standalone => "standalone",
            Self::WorkshopMaster => "workshop-master",
            Self::WorkshopWorker => "workshop-worker",
        }
    }

    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Standalone => "独立 1A",
            Self::WorkshopMaster => "Workshop 总控",
            Self::WorkshopWorker => "Workshop 执行单元",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostBinding {
    kind: HostKind,
    agent_id: Option<String>,
    session_id: Option<String>,
    master_conversation_id: Option<String>,
}

impl HostBinding {
    pub fn standalone() -> Self {
        Self {
            kind: HostKind::Standalone,
            agent_id: None,
            session_id: None,
            master_conversation_id: None,
        }
    }

    pub fn from_env() -> Self {
        Self::from_signals(HostSignals {
            context: env::var("SKYTERM_HOST_CONTEXT").ok(),
            role: env::var("SKYTERM_AGENT_ROLE").ok(),
            context_version: env::var("SKYTERM_HOST_CONTEXT_VERSION").ok(),
            expert_interactive: env::var("SKYTERM_EXPERT_INTERACTIVE").ok(),
            agent_id: env::var("SKYTERM_AGENT_ID").ok(),
            runtime: env::var("SKYTERM_AGENT_RUNTIME").ok(),
            session_id: env::var("SKYTERM_SESSION_ID").ok(),
            master_conversation_id: env::var("SKYTERM_MASTER_CONVERSATION_ID").ok(),
        })
    }

    pub fn kind(&self) -> HostKind {
        self.kind
    }

    pub fn agent_id(&self) -> Option<&str> {
        self.agent_id.as_deref()
    }

    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    pub fn master_conversation_id(&self) -> Option<&str> {
        self.master_conversation_id.as_deref()
    }

    pub fn is_workshop_master(&self) -> bool {
        self.kind == HostKind::WorkshopMaster
    }

    pub fn workshop_context(role: &str, agent_id: &str, runtime: &str, session_id: &str) -> Self {
        Self::workshop_context_with_conversation(role, agent_id, agent_id, runtime, session_id)
    }

    pub fn workshop_context_with_conversation(
        role: &str,
        agent_id: &str,
        master_conversation_id: &str,
        runtime: &str,
        session_id: &str,
    ) -> Self {
        Self::from_signals(HostSignals {
            context: Some("skynet-workshop".into()),
            role: Some(role.into()),
            context_version: Some("1".into()),
            expert_interactive: Some("1".into()),
            agent_id: Some(agent_id.into()),
            runtime: Some(runtime.into()),
            session_id: Some(session_id.into()),
            master_conversation_id: (role == "master").then(|| master_conversation_id.into()),
        })
    }

    fn from_signals(signals: HostSignals) -> Self {
        if let Some(context) = signals.context.as_deref() {
            return Self::from_explicit_context(context, &signals);
        }

        let interactive = signals.expert_interactive.as_deref() == Some("1");
        if !interactive {
            return Self::standalone();
        }

        let agent_id = signals.agent_id.filter(|value| !value.trim().is_empty());
        let session_id = signals.session_id.filter(|value| valid_session_id(value));
        let master = agent_id.as_deref() == Some("main-coordinator")
            && normalize_runtime(signals.runtime.as_deref()) == Some("1agent")
            && session_id.is_some();

        if master {
            Self {
                kind: HostKind::WorkshopMaster,
                master_conversation_id: agent_id.clone(),
                agent_id,
                session_id,
            }
        } else if agent_id.as_deref() == Some("main-coordinator") {
            Self::standalone()
        } else if agent_id.is_some() {
            Self {
                kind: HostKind::WorkshopWorker,
                agent_id,
                session_id,
                master_conversation_id: None,
            }
        } else {
            Self::standalone()
        }
    }

    fn from_explicit_context(context: &str, signals: &HostSignals) -> Self {
        if context != "skynet-workshop" || signals.context_version.as_deref() != Some("1") {
            return Self::standalone();
        }

        let agent_id = signals
            .agent_id
            .as_ref()
            .filter(|value| !value.trim().is_empty())
            .cloned();
        let session_id = signals
            .session_id
            .as_ref()
            .filter(|value| valid_session_id(value))
            .cloned();
        let master_conversation_id = signals
            .master_conversation_id
            .as_ref()
            .filter(|value| safe_identity(value))
            .cloned();

        match signals.role.as_deref() {
            Some("master")
                if agent_id.is_some()
                    && normalize_runtime(signals.runtime.as_deref()) == Some("1agent")
                    && session_id.is_some()
                    && master_conversation_id.is_some() =>
            {
                Self {
                    kind: HostKind::WorkshopMaster,
                    agent_id,
                    session_id,
                    master_conversation_id,
                }
            }
            Some("worker")
                if agent_id.is_some() && agent_id.as_deref() != Some("main-coordinator") =>
            {
                Self {
                    kind: HostKind::WorkshopWorker,
                    agent_id,
                    session_id,
                    master_conversation_id: None,
                }
            }
            _ => Self::standalone(),
        }
    }
}

#[derive(Debug, Default)]
struct HostSignals {
    context: Option<String>,
    role: Option<String>,
    context_version: Option<String>,
    expert_interactive: Option<String>,
    agent_id: Option<String>,
    runtime: Option<String>,
    session_id: Option<String>,
    master_conversation_id: Option<String>,
}

fn safe_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn normalize_runtime(runtime: Option<&str>) -> Option<&str> {
    runtime
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            if value.eq_ignore_ascii_case("1agent") || value.eq_ignore_ascii_case("1-agent") {
                "1agent"
            } else {
                value
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_master() -> HostSignals {
        HostSignals {
            expert_interactive: Some("1".into()),
            agent_id: Some("main-coordinator".into()),
            runtime: Some("1agent".into()),
            session_id: Some("term-1790000000000-1234-1".into()),
            ..HostSignals::default()
        }
    }

    #[test]
    fn empty_environment_is_standalone() {
        assert_eq!(
            HostBinding::from_signals(HostSignals::default()).kind(),
            HostKind::Standalone
        );
    }

    #[test]
    fn legacy_workshop_master_requires_master_identity() {
        assert_eq!(
            HostBinding::from_signals(legacy_master()).kind(),
            HostKind::WorkshopMaster
        );

        let mut signals = legacy_master();
        signals.runtime = Some("codex".into());
        assert_eq!(
            HostBinding::from_signals(signals).kind(),
            HostKind::Standalone
        );
    }

    #[test]
    fn other_interactive_agent_is_workshop_worker() {
        let signals = HostSignals {
            expert_interactive: Some("1".into()),
            agent_id: Some("release-verifier".into()),
            session_id: Some("term-1790000000000-1234-2".into()),
            ..HostSignals::default()
        };
        assert_eq!(
            HostBinding::from_signals(signals).kind(),
            HostKind::WorkshopWorker
        );
    }

    #[test]
    fn explicit_context_requires_version_and_consistent_role() {
        let mut signals = legacy_master();
        signals.context = Some("skynet-workshop".into());
        signals.role = Some("master".into());
        signals.context_version = Some("1".into());
        signals.master_conversation_id = Some("main-coordinator".into());
        assert_eq!(
            HostBinding::from_signals(signals).kind(),
            HostKind::WorkshopMaster
        );

        let mut conflict = legacy_master();
        conflict.context = Some("skynet-workshop".into());
        conflict.role = Some("worker".into());
        conflict.context_version = Some("1".into());
        conflict.master_conversation_id = Some("main-coordinator".into());
        assert_eq!(
            HostBinding::from_signals(conflict).kind(),
            HostKind::Standalone
        );

        let mut version_mismatch = legacy_master();
        version_mismatch.context = Some("skynet-workshop".into());
        version_mismatch.role = Some("master".into());
        version_mismatch.context_version = Some("2".into());
        version_mismatch.master_conversation_id = Some("main-coordinator".into());
        assert_eq!(
            HostBinding::from_signals(version_mismatch).kind(),
            HostKind::Standalone
        );
    }

    #[test]
    fn explicit_master_conversation_supports_distinct_ids() {
        let binding = HostBinding::workshop_context_with_conversation(
            "master",
            "master-2",
            "conversation-goal-2",
            "1agent",
            "term-1790000000000-1234-2",
        );
        assert_eq!(binding.kind(), HostKind::WorkshopMaster);
        assert_eq!(
            binding.master_conversation_id(),
            Some("conversation-goal-2")
        );
    }
}
