//! Skill service: in-memory registry backed by an append-only journal.

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::skill::{SkillError, SkillEvent, SkillManifest, SkillStore, SkillSummary};
use crate::skill_eval::{SkillEvalError, SkillEvalReport, SkillEvalSuite, evaluate_skill};
use crate::skill_journal::{SkillJournalError, SkillJournalStore};

#[derive(Debug, Error)]
pub enum SkillServiceError {
    #[error("skill: {0}")]
    Skill(#[from] SkillError),
    #[error("skill journal: {0}")]
    Journal(#[from] SkillJournalError),
    #[error("skill eval: {0}")]
    Eval(#[from] SkillEvalError),
    #[error("skill eval failed: {0}")]
    EvalFailed(String),
    #[error("routine: {0}")]
    Routine(String),
}

#[derive(Debug)]
pub struct SkillService {
    store: SkillStore,
    journal: SkillJournalStore,
}

impl SkillService {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, SkillServiceError> {
        let journal = SkillJournalStore::open(path)?;
        let store = SkillStore::replay(journal.events())?;
        Ok(Self { store, journal })
    }

    pub fn store(&self) -> &SkillStore {
        &self.store
    }

    pub fn events(&self) -> &[SkillEvent] {
        self.store.events()
    }

    pub fn journal_path(&self) -> &Path {
        self.journal.path()
    }

    pub fn reload(&mut self) -> Result<(), SkillServiceError> {
        let path = self.journal.path().to_path_buf();
        *self = Self::open(path)?;
        Ok(())
    }

    fn persist_candidate(&mut self, candidate: &SkillStore) -> Result<(), SkillServiceError> {
        match self.journal.persist(candidate) {
            Ok(_) => {
                self.store = candidate.clone();
                Ok(())
            }
            Err(error) => {
                if matches!(error, SkillJournalError::ConcurrentWrite { .. }) {
                    let _ = self.reload();
                }
                Err(error.into())
            }
        }
    }

    pub fn register(
        &mut self,
        manifest: SkillManifest,
        now_ms: u128,
    ) -> Result<(), SkillServiceError> {
        let mut candidate = self.store.clone();
        candidate.register(manifest, now_ms)?;
        self.persist_candidate(&candidate)?;
        Ok(())
    }

    pub fn activate(
        &mut self,
        name: &str,
        version: &str,
        now_ms: u128,
    ) -> Result<(), SkillServiceError> {
        let mut candidate = self.store.clone();
        candidate.activate(name, version, now_ms)?;
        self.persist_candidate(&candidate)?;
        Ok(())
    }

    pub fn disable(&mut self, name: &str, now_ms: u128) -> Result<(), SkillServiceError> {
        let mut candidate = self.store.clone();
        candidate.disable_at(name, now_ms)?;
        self.persist_candidate(&candidate)?;
        Ok(())
    }

    pub fn rollback(
        &mut self,
        name: &str,
        version: &str,
        now_ms: u128,
    ) -> Result<(), SkillServiceError> {
        self.activate(name, version, now_ms)
    }

    pub fn evaluate(
        &self,
        name: &str,
        version: &str,
        suite: &SkillEvalSuite,
    ) -> Result<SkillEvalReport, SkillServiceError> {
        let manifest = self
            .store
            .view(name, version)
            .ok_or_else(|| SkillError::UnknownVersion(name.into(), version.into()))?;
        Ok(evaluate_skill(manifest, suite)?)
    }

    pub fn activate_if_passed(
        &mut self,
        name: &str,
        version: &str,
        suite: &SkillEvalSuite,
        now_ms: u128,
    ) -> Result<SkillEvalReport, SkillServiceError> {
        let report = self.evaluate(name, version, suite)?;
        if !report.passed {
            return Err(SkillServiceError::EvalFailed(format!(
                "{} {} scored {} bps, below {}",
                name, version, report.score_bps, suite.min_score_bps
            )));
        }
        self.activate(name, version, now_ms)?;
        Ok(report)
    }

    /// Compose a routine task from an active skill version. The routine only
    /// reuses an already-approved skill; it does not grant new permissions.
    pub fn routine_task(&self, spec: &str, task: &str) -> Result<String, SkillServiceError> {
        let (name, version) = spec.split_once('@').ok_or_else(|| {
            SkillServiceError::Routine("routine skill must be name@version".into())
        })?;
        let name = name.trim();
        let version = version.trim();
        if name.is_empty() || version.is_empty() || name.contains('@') || version.contains('@') {
            return Err(SkillServiceError::Routine(
                "routine skill must be name@version".into(),
            ));
        }
        let active = self.active(name).ok_or_else(|| {
            SkillServiceError::Routine(format!("skill {name} has no active version"))
        })?;
        if active.version != version {
            return Err(SkillServiceError::Routine(format!(
                "skill {name} active version is {}, not {version}",
                active.version
            )));
        }
        let manifest = self.view(name, version).ok_or_else(|| {
            SkillServiceError::Routine(format!("skill {name} {version} does not exist"))
        })?;
        if task.trim().is_empty() {
            return Err(SkillServiceError::Routine("routine task is empty".into()));
        }
        Ok(format!(
            "## Routine: {} {}\n\n{}\n\n## Task\n{}",
            manifest.name, manifest.version, manifest.body, task
        ))
    }

    pub fn active(&self, name: &str) -> Option<&SkillManifest> {
        self.store.active(name)
    }

    pub fn view(&self, name: &str, version: &str) -> Option<&SkillManifest> {
        self.store.view(name, version)
    }

    pub fn list(&self) -> Vec<SkillSummary> {
        self.store.list()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::SkillRisk;
    use crate::skill_eval::SkillEvalCase;

    fn manifest(version: &str) -> SkillManifest {
        SkillManifest {
            name: "release".into(),
            description: "release process".into(),
            version: version.into(),
            platforms: vec!["linux".into()],
            scope: "repo:1-agent".into(),
            requires_tools: vec!["shell".into()],
            risk: SkillRisk::Medium,
            body: "## Procedure\n1. verify".into(),
            content_sha256: String::new(),
        }
    }

    #[test]
    fn registry_survives_restart_and_rollback() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("skill-journal.jsonl");
        let mut service = SkillService::open(&path).unwrap();
        service.register(manifest("1.0.0"), 10).unwrap();
        service.register(manifest("1.1.0"), 11).unwrap();
        service.activate("release", "1.1.0", 12).unwrap();
        service.rollback("release", "1.0.0", 13).unwrap();
        drop(service);

        let reopened = SkillService::open(&path).unwrap();
        assert_eq!(reopened.active("release").unwrap().version, "1.0.0");
        assert_eq!(reopened.list().len(), 2);
        assert_eq!(reopened.events().len(), 4);
    }

    #[test]
    fn failed_registration_leaves_the_journal_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("skill-journal.jsonl");
        let mut service = SkillService::open(&path).unwrap();
        service.register(manifest("1.0.0"), 10).unwrap();
        assert!(service.register(manifest("1.0.0"), 11).is_err());
        assert_eq!(service.events().len(), 1);
        let reopened = SkillService::open(&path).unwrap();
        assert_eq!(reopened.events().len(), 1);
    }

    #[test]
    fn disable_is_persisted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("skill-journal.jsonl");
        let mut service = SkillService::open(&path).unwrap();
        service.register(manifest("1.0.0"), 10).unwrap();
        service.activate("release", "1.0.0", 11).unwrap();
        service.disable("release", 12).unwrap();
        drop(service);

        let reopened = SkillService::open(&path).unwrap();
        assert!(reopened.active("release").is_none());
    }

    #[test]
    fn activation_gate_requires_the_held_out_score() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("skill-journal.jsonl");
        let mut service = SkillService::open(&path).unwrap();
        service.register(manifest("1.0.0"), 10).unwrap();
        let suite = SkillEvalSuite {
            suite_id: "release-suite".into(),
            min_score_bps: 10_000,
            cases: vec![SkillEvalCase {
                case_id: "verify".into(),
                required_markers: vec!["verify".into()],
                forbidden_markers: vec!["rm -rf".into()],
            }],
        };
        let report = service
            .activate_if_passed("release", "1.0.0", &suite, 11)
            .unwrap();
        assert!(report.passed);
        assert_eq!(service.active("release").unwrap().version, "1.0.0");

        let mut bad = manifest("1.1.0");
        bad.body = "rm -rf /".into();
        service.register(bad, 12).unwrap();
        assert!(matches!(
            service
                .activate_if_passed("release", "1.1.0", &suite, 13)
                .unwrap_err(),
            SkillServiceError::EvalFailed(_)
        ));
        assert_eq!(service.active("release").unwrap().version, "1.0.0");
    }

    #[test]
    fn routine_requires_an_active_exact_version_and_composes_the_task() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("skill-journal.jsonl");
        let mut service = SkillService::open(&path).unwrap();
        service.register(manifest("1.0.0"), 10).unwrap();
        service.register(manifest("2.0.0"), 11).unwrap();
        service.activate("release", "1.0.0", 12).unwrap();

        let composed = service
            .routine_task("release@1.0.0", "run the checks")
            .unwrap();
        assert!(composed.starts_with("## Routine: release 1.0.0"));
        assert!(composed.contains("## Task\nrun the checks"));

        assert!(matches!(
            service
                .routine_task("release@2.0.0", "run the checks")
                .unwrap_err(),
            SkillServiceError::Routine(_)
        ));
        assert!(matches!(
            service
                .routine_task("release", "run the checks")
                .unwrap_err(),
            SkillServiceError::Routine(_)
        ));
        assert!(matches!(
            service.routine_task("missing@1.0.0", "run").unwrap_err(),
            SkillServiceError::Routine(_)
        ));
        assert!(matches!(
            service.routine_task("release@1.0.0", "  ").unwrap_err(),
            SkillServiceError::Routine(_)
        ));
    }

    #[test]
    fn stale_writer_reloads_and_retry_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("skill-journal.jsonl");
        let mut first = SkillService::open(&path).unwrap();
        let mut second = SkillService::open(&path).unwrap();
        first.register(manifest("1.0.0"), 10).unwrap();
        let error = second.register(manifest("2.0.0"), 11).unwrap_err();
        assert!(matches!(
            error,
            SkillServiceError::Journal(SkillJournalError::ConcurrentWrite { .. })
        ));
        second.register(manifest("2.0.0"), 12).unwrap();
        let reopened = SkillService::open(&path).unwrap();
        assert_eq!(reopened.events().len(), 2);
    }
}
