//! Memory service: metadata state + append-only journal + encrypted payloads.

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::memory::{
    ForgetMode, MemoryError, MemoryEvent, MemoryHit, MemoryRecord, MemoryScope, MemoryStore,
};
use crate::memory_journal::{MemoryJournalError, MemoryJournalStore};
use crate::memory_payload::{
    MasterRotation, MemoryPayloadError, MemoryPayloadStore, ScopeKeyRotation,
};

#[derive(Debug, Error)]
pub enum MemoryServiceError {
    #[error("memory: {0}")]
    Memory(#[from] MemoryError),
    #[error("memory journal: {0}")]
    Journal(#[from] MemoryJournalError),
    #[error("memory payload: {0}")]
    Payload(#[from] MemoryPayloadError),
    #[error("memory {0} has metadata but no payload")]
    MissingPayload(String),
    #[error("memory {0} scope mismatch")]
    ScopeMismatch(String),
}

#[derive(Debug)]
pub struct MemoryService {
    store: MemoryStore,
    journal: MemoryJournalStore,
    payload: MemoryPayloadStore,
    journal_path: PathBuf,
    payload_root: PathBuf,
    key_path: PathBuf,
}

impl MemoryService {
    pub fn open(
        journal_path: impl Into<PathBuf>,
        payload_root: impl Into<PathBuf>,
        key_path: impl AsRef<Path>,
    ) -> Result<Self, MemoryServiceError> {
        let journal_path = journal_path.into();
        let payload_root = payload_root.into();
        let key_path = key_path.as_ref().to_path_buf();
        let journal = MemoryJournalStore::open(&journal_path)?;
        let mut store = MemoryStore::from_events(journal.events())?;
        let payload = MemoryPayloadStore::open(&payload_root, &key_path)?;
        for memory_id in store.record_ids() {
            let record = store
                .get(&memory_id)
                .cloned()
                .ok_or_else(|| MemoryError::UnknownMemory(memory_id.clone()))?;
            if record.hard_forgotten_ms.is_some() {
                let _ = payload.forget(&memory_id);
                continue;
            }
            if record.content_sha256.is_empty() {
                continue;
            }
            let content = payload
                .get(&memory_id, &record.scope.scope_id())
                .map_err(|error| match error {
                    MemoryPayloadError::Io(error)
                        if error.kind() == std::io::ErrorKind::NotFound =>
                    {
                        MemoryServiceError::MissingPayload(memory_id.clone())
                    }
                    other => MemoryServiceError::Payload(other),
                })?;
            store.hydrate_content(&memory_id, content)?;
        }
        Ok(Self {
            store,
            journal,
            payload,
            journal_path,
            payload_root,
            key_path,
        })
    }

    pub fn reload(&mut self) -> Result<(), MemoryServiceError> {
        let fresh = Self::open(
            self.journal_path.clone(),
            self.payload_root.clone(),
            self.key_path.clone(),
        )?;
        *self = fresh;
        Ok(())
    }

    fn persist_candidate(&mut self, candidate: &MemoryStore) -> Result<(), MemoryServiceError> {
        match self.journal.persist(candidate) {
            Ok(_) => {
                self.store = candidate.clone();
                Ok(())
            }
            Err(error) => {
                if matches!(error, MemoryJournalError::ConcurrentWrite { .. }) {
                    let _ = self.reload();
                }
                Err(error.into())
            }
        }
    }

    pub fn store(&self) -> &MemoryStore {
        &self.store
    }

    pub fn events(&self) -> &[MemoryEvent] {
        self.store.events()
    }

    pub fn remember_at(
        &mut self,
        record: MemoryRecord,
        now_ms: u128,
    ) -> Result<(), MemoryServiceError> {
        if self.store.get(&record.memory_id).is_some() {
            return Err(MemoryError::AlreadyExists(record.memory_id).into());
        }
        let mut candidate = self.store.clone();
        candidate.remember_at(record.clone(), now_ms)?;
        let scope_id = record.scope.scope_id();
        self.payload
            .put(&record.memory_id, &scope_id, &record.content)?;
        if let Err(error) = self.persist_candidate(&candidate) {
            let _ = self.payload.forget(&record.memory_id);
            return Err(error);
        }
        Ok(())
    }

    pub fn build_user_model(
        &self,
        user_id: &str,
        scope: &MemoryScope,
    ) -> crate::user_model::UserModel {
        let mut model = crate::user_model::UserModel::new(user_id, scope.clone());
        for record in self.store.records() {
            model.apply(record);
        }
        model
    }

    pub fn recall(
        &self,
        query: &str,
        scope: &MemoryScope,
        now_ms: u128,
        limit: usize,
    ) -> Vec<MemoryHit> {
        self.store.recall(query, scope, now_ms, limit)
    }

    pub fn get(&self, memory_id: &str, scope: &MemoryScope) -> Option<&MemoryRecord> {
        self.store
            .get(memory_id)
            .filter(|record| record.scope == *scope)
    }

    pub fn forget(
        &mut self,
        memory_id: &str,
        scope: &MemoryScope,
        mode: ForgetMode,
        now_ms: u128,
    ) -> Result<(), MemoryServiceError> {
        self.ensure_scope(memory_id, scope)?;
        let mut candidate = self.store.clone();
        candidate.forget(memory_id, mode, now_ms)?;
        self.persist_candidate(&candidate)?;
        if mode == ForgetMode::Hard {
            self.payload.forget(memory_id)?;
        }
        Ok(())
    }

    pub fn restore_soft_at(
        &mut self,
        memory_id: &str,
        scope: &MemoryScope,
        now_ms: u128,
    ) -> Result<(), MemoryServiceError> {
        self.ensure_scope(memory_id, scope)?;
        let mut candidate = self.store.clone();
        candidate.restore_soft_at(memory_id, now_ms)?;
        self.persist_candidate(&candidate)?;
        Ok(())
    }

    pub fn record_use(
        &mut self,
        memory_id: &str,
        scope: &MemoryScope,
        accepted: bool,
        now_ms: u128,
    ) -> Result<(), MemoryServiceError> {
        self.ensure_scope(memory_id, scope)?;
        let mut candidate = self.store.clone();
        candidate.record_use(memory_id, accepted, now_ms)?;
        self.persist_candidate(&candidate)?;
        Ok(())
    }

    fn ensure_scope(&self, memory_id: &str, scope: &MemoryScope) -> Result<(), MemoryServiceError> {
        let record = self
            .store
            .get(memory_id)
            .ok_or_else(|| MemoryError::UnknownMemory(memory_id.to_string()))?;
        if record.scope != *scope {
            return Err(MemoryServiceError::ScopeMismatch(memory_id.to_string()));
        }
        Ok(())
    }

    /// Rotate the scope KEK and rewrap all live payloads in that scope. Old
    /// ledger versions are retired; legacy schema-v1 backups remain outside
    /// the crypto-shredding guarantee until master-key rotation.
    pub fn rotate_scope_key(
        &self,
        scope: &MemoryScope,
    ) -> Result<ScopeKeyRotation, MemoryPayloadError> {
        self.payload.rotate_scope_key(&scope.scope_id())
    }

    /// Rotate the master key and rewrap the scope-KEK ledger. All live
    /// payloads must already be schema 2.
    pub fn rotate_master_key(&self) -> Result<MasterRotation, MemoryPayloadError> {
        self.payload.rotate_master_key()
    }

    pub fn payload_contains(
        &self,
        memory_id: &str,
        scope: &MemoryScope,
    ) -> Result<bool, MemoryServiceError> {
        self.ensure_scope(memory_id, scope)?;
        Ok(self.payload.contains(memory_id)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{
        DataClass, MemoryKind, MemoryLayer, MemorySource, MemorySourceKind, MemoryTaint,
    };

    fn record(content: &str) -> MemoryRecord {
        let mut record = MemoryRecord::new(
            "mem-1",
            MemoryLayer::Semantic,
            MemoryKind::Preference,
            MemoryScope::bot("bot-1"),
            content,
            MemorySource::new(MemorySourceKind::UserMessage, "event-1"),
        );
        record.taint = MemoryTaint::Trusted;
        record.data_class = DataClass::Personal;
        record
    }

    fn service(root: &Path) -> MemoryService {
        MemoryService::open(
            root.join("memory-journal.jsonl"),
            root.join("payloads"),
            root.join("memory.key"),
        )
        .unwrap()
    }

    #[test]
    fn remember_survives_reopen_and_recalls_content() {
        let dir = tempfile::tempdir().unwrap();
        let mut memory = service(dir.path());
        memory.remember_at(record("prefers pnpm"), 10).unwrap();
        assert_eq!(
            memory
                .recall("pnpm", &MemoryScope::bot("bot-1"), 11, 5)
                .len(),
            1
        );
        drop(memory);

        let reopened = service(dir.path());
        assert_eq!(
            reopened
                .get("mem-1", &MemoryScope::bot("bot-1"))
                .unwrap()
                .content,
            "prefers pnpm"
        );
        assert_eq!(
            reopened
                .recall("pnpm", &MemoryScope::bot("bot-1"), 12, 5)
                .len(),
            1
        );
        assert!(
            reopened
                .payload_contains("mem-1", &MemoryScope::bot("bot-1"))
                .unwrap()
        );
    }

    #[test]
    fn scope_key_rotation_keeps_memory_readable_after_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let mut memory = service(dir.path());
        memory.remember_at(record("prefers pnpm"), 10).unwrap();
        let rotation = memory.rotate_scope_key(&MemoryScope::bot("bot-1")).unwrap();
        assert_eq!(rotation.rotated, 1);
        assert_eq!(rotation.new_version, 2);
        drop(memory);

        let reopened = service(dir.path());
        assert_eq!(
            reopened
                .get("mem-1", &MemoryScope::bot("bot-1"))
                .unwrap()
                .content,
            "prefers pnpm"
        );
    }

    #[test]
    fn soft_forget_survives_restart_and_can_be_restored() {
        let dir = tempfile::tempdir().unwrap();
        let mut memory = service(dir.path());
        memory.remember_at(record("prefers pnpm"), 10).unwrap();
        memory
            .forget("mem-1", &MemoryScope::bot("bot-1"), ForgetMode::Soft, 11)
            .unwrap();
        drop(memory);

        let mut reopened = service(dir.path());
        assert!(
            reopened
                .recall("pnpm", &MemoryScope::bot("bot-1"), 12, 5)
                .is_empty()
        );
        reopened
            .restore_soft_at("mem-1", &MemoryScope::bot("bot-1"), 13)
            .unwrap();
        assert_eq!(
            reopened
                .recall("pnpm", &MemoryScope::bot("bot-1"), 14, 5)
                .len(),
            1
        );
    }

    #[test]
    fn hard_forget_deletes_payload_and_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let mut memory = service(dir.path());
        memory.remember_at(record("prefers pnpm"), 10).unwrap();
        memory
            .forget("mem-1", &MemoryScope::bot("bot-1"), ForgetMode::Hard, 11)
            .unwrap();
        assert!(
            !memory
                .payload_contains("mem-1", &MemoryScope::bot("bot-1"))
                .unwrap()
        );
        drop(memory);

        let reopened = service(dir.path());
        assert_eq!(
            reopened
                .get("mem-1", &MemoryScope::bot("bot-1"))
                .unwrap()
                .content,
            ""
        );
        assert!(
            reopened
                .recall("pnpm", &MemoryScope::bot("bot-1"), 12, 5)
                .is_empty()
        );
    }

    #[test]
    fn scope_mismatch_cannot_read_or_mutate() {
        let dir = tempfile::tempdir().unwrap();
        let mut memory = service(dir.path());
        memory.remember_at(record("prefers pnpm"), 10).unwrap();
        assert!(
            memory
                .get("mem-1", &MemoryScope::bot("other-bot"))
                .is_none()
        );
        assert!(matches!(
            memory
                .forget(
                    "mem-1",
                    &MemoryScope::bot("other-bot"),
                    ForgetMode::Soft,
                    11
                )
                .unwrap_err(),
            MemoryServiceError::ScopeMismatch(_)
        ));
        assert!(matches!(
            memory
                .payload_contains("mem-1", &MemoryScope::bot("other-bot"))
                .unwrap_err(),
            MemoryServiceError::ScopeMismatch(_)
        ));
    }

    #[test]
    fn hard_forget_retries_payload_cleanup_without_resurrection() {
        let dir = tempfile::tempdir().unwrap();
        let mut memory = service(dir.path());
        memory.remember_at(record("prefers pnpm"), 10).unwrap();
        let payload_path = memory.payload.root().join("mem-1.payload.json");
        std::fs::remove_file(&payload_path).unwrap();
        std::fs::create_dir(&payload_path).unwrap();
        assert!(matches!(
            memory
                .forget("mem-1", &MemoryScope::bot("bot-1"), ForgetMode::Hard, 11)
                .unwrap_err(),
            MemoryServiceError::Payload(_)
        ));
        drop(memory);

        let mut reopened = service(dir.path());
        assert_eq!(
            reopened
                .get("mem-1", &MemoryScope::bot("bot-1"))
                .unwrap()
                .content,
            ""
        );
        std::fs::remove_dir(&payload_path).unwrap();
        reopened
            .forget("mem-1", &MemoryScope::bot("bot-1"), ForgetMode::Hard, 12)
            .unwrap();
        assert!(
            !reopened
                .payload_contains("mem-1", &MemoryScope::bot("bot-1"))
                .unwrap()
        );
    }

    #[test]
    fn stale_writer_reloads_and_retry_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let mut first = service(dir.path());
        first.remember_at(record("first"), 10).unwrap();
        let mut second = service(dir.path());

        let mut external = record("external");
        external.memory_id = "mem-2".into();
        first.remember_at(external, 11).unwrap();

        let mut next = record("second");
        next.memory_id = "mem-3".into();
        let error = second.remember_at(next.clone(), 12).unwrap_err();
        assert!(matches!(
            error,
            MemoryServiceError::Journal(MemoryJournalError::ConcurrentWrite { .. })
        ));
        second.remember_at(next, 13).unwrap();
        let reopened = service(dir.path());
        assert_eq!(
            reopened
                .get("mem-3", &MemoryScope::bot("bot-1"))
                .unwrap()
                .content,
            "second"
        );
        assert!(reopened.get("mem-1", &MemoryScope::bot("bot-1")).is_some());
        assert!(reopened.get("mem-2", &MemoryScope::bot("bot-1")).is_some());
    }

    #[test]
    fn concurrent_first_writes_with_a_shared_key_recover() {
        let dir = tempfile::tempdir().unwrap();
        let mut first = service(dir.path());
        let mut second = service(dir.path());
        first.remember_at(record("first"), 10).unwrap();

        let mut next = record("second");
        next.memory_id = "mem-2".into();
        assert!(second.remember_at(next.clone(), 11).is_err());
        second.remember_at(next, 12).unwrap();

        let reopened = service(dir.path());
        assert!(reopened.get("mem-1", &MemoryScope::bot("bot-1")).is_some());
        assert_eq!(
            reopened
                .get("mem-2", &MemoryScope::bot("bot-1"))
                .unwrap()
                .content,
            "second"
        );
    }

    #[test]
    fn missing_payload_fails_closed_on_open() {
        let dir = tempfile::tempdir().unwrap();
        let mut memory = service(dir.path());
        memory.remember_at(record("prefers pnpm"), 10).unwrap();
        memory.payload.forget("mem-1").unwrap();
        drop(memory);
        assert!(matches!(
            MemoryService::open(
                dir.path().join("memory-journal.jsonl"),
                dir.path().join("payloads"),
                dir.path().join("memory.key"),
            )
            .unwrap_err(),
            MemoryServiceError::MissingPayload(_)
        ));
    }
}
