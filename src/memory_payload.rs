//! Encrypted memory payload store.
//!
//! Each memory body gets a random 256-bit DEK. The DEK is wrapped by a
//! per-scope key derived from the local master key. Hard forget deletes the
//! payload file, destroying the wrapped DEK; the ciphertext alone is useless.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use ring::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
use ring::hmac;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

const PAYLOAD_SCHEMA_VERSION: u32 = 2;
const LEGACY_PAYLOAD_SCHEMA_VERSION: u32 = 1;
const KEK_LEDGER_SCHEMA_VERSION: u32 = 1;
const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 12;

#[derive(Debug, Error)]
pub enum MemoryPayloadError {
    #[error("memory payload io: {0}")]
    Io(#[from] std::io::Error),
    #[error("memory payload json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("memory payload crypto: {0}")]
    Crypto(String),
    #[error("memory payload id is unsafe: {0}")]
    UnsafeId(String),
    #[error("memory payload scope mismatch: expected {expected}, got {actual}")]
    ScopeMismatch { expected: String, actual: String },
    #[error("memory payload is corrupt: {0}")]
    Corrupt(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct KekLedgerFile {
    schema_version: u32,
    entries: Vec<KekLedgerEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct KekLedgerEntry {
    scope_id: String,
    version: u32,
    nonce_hex: String,
    wrapped_key_hex: String,
}

#[derive(Debug, Default)]
struct KekLedger {
    entries: BTreeMap<(String, u32), KekLedgerEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ScopeKeyRotation {
    pub scope_id: String,
    pub new_version: u32,
    pub rotated: usize,
    pub retired_versions: Vec<u32>,
    pub legacy_migrated: usize,
}

#[derive(Clone)]
pub struct MemoryKeyring {
    master: [u8; KEY_LEN],
    ledger_path: PathBuf,
    ledger: Arc<RwLock<KekLedger>>,
}

impl std::fmt::Debug for MemoryKeyring {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MemoryKeyring")
            .field("master", &"<redacted>")
            .field("ledger_path", &self.ledger_path)
            .finish()
    }
}

impl MemoryKeyring {
    pub fn open_or_create(path: impl AsRef<Path>) -> Result<Self, MemoryPayloadError> {
        let path = path.as_ref();
        let ledger_path = ledger_path_for(path);
        let master = if path.exists() {
            let metadata = std::fs::symlink_metadata(path)?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(MemoryPayloadError::Corrupt(
                    "memory key must be a regular file".into(),
                ));
            }
            let mut file = File::open(path)?;
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            if bytes.len() != KEY_LEN {
                return Err(MemoryPayloadError::Corrupt(
                    "memory key must be exactly 32 bytes".into(),
                ));
            }
            check_private_permissions(path)?;
            let mut master = [0u8; KEY_LEN];
            master.copy_from_slice(&bytes);
            master
        } else {
            if let Some(parent) = path.parent()
                && !parent.as_os_str().is_empty()
            {
                let existed = parent.exists();
                std::fs::create_dir_all(parent)?;
                if !existed {
                    set_dir_permissions(parent)?;
                }
            }
            let rng = SystemRandom::new();
            let mut master = [0u8; KEY_LEN];
            rng.fill(&mut master)
                .map_err(|_| MemoryPayloadError::Crypto("cannot generate memory key".into()))?;
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("memory.key");
            let temp = path.with_file_name(format!(".{file_name}.{}.tmp", Uuid::new_v4()));
            let result = (|| -> Result<(), MemoryPayloadError> {
                let mut options = OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(0o600);
                    options.custom_flags(libc::O_NOFOLLOW);
                }
                let mut file = options.open(&temp)?;
                set_private_permissions(&file)?;
                file.write_all(&master)?;
                file.sync_data()?;
                drop(file);
                std::fs::rename(&temp, path)?;
                Ok(())
            })();
            if let Err(error) = result {
                let _ = std::fs::remove_file(&temp);
                return Err(error);
            }
            if let Some(parent) = path.parent()
                && !parent.as_os_str().is_empty()
                && let Ok(dir) = File::open(parent)
            {
                let _ = dir.sync_all();
            }
            master
        };
        let ledger = load_ledger(&ledger_path)?;
        Ok(Self {
            master,
            ledger_path,
            ledger: Arc::new(RwLock::new(ledger)),
        })
    }

    /// Legacy schema-v1 payloads derived their KEK directly from the master.
    /// New payloads use the random per-scope KEK ledger instead.
    fn legacy_scope_key(&self, scope_id: &str) -> [u8; KEY_LEN] {
        let key = hmac::Key::new(hmac::HMAC_SHA256, &self.master);
        let tag = hmac::sign(&key, scope_id.as_bytes());
        let mut output = [0u8; KEY_LEN];
        output.copy_from_slice(tag.as_ref());
        output
    }

    fn ledger_is_empty(&self) -> Result<bool, MemoryPayloadError> {
        let ledger = self
            .ledger
            .read()
            .map_err(|_| MemoryPayloadError::Crypto("memory key ledger lock".into()))?;
        Ok(ledger.entries.is_empty())
    }

    fn wrap_key(&self) -> [u8; KEY_LEN] {
        let key = hmac::Key::new(hmac::HMAC_SHA256, &self.master);
        let tag = hmac::sign(&key, b"memory-kek-wrap-v1");
        let mut output = [0u8; KEY_LEN];
        output.copy_from_slice(tag.as_ref());
        output
    }

    fn ensure_scope_key(&self, scope_id: &str) -> Result<(u32, [u8; KEY_LEN]), MemoryPayloadError> {
        let mut ledger = self
            .ledger
            .write()
            .map_err(|_| MemoryPayloadError::Crypto("memory key ledger lock".into()))?;
        if let Some(version) = ledger
            .entries
            .keys()
            .filter(|(scope, _)| scope == scope_id)
            .map(|(_, version)| *version)
            .max()
        {
            let key = self.unwrap_ledger_key(scope_id, version, &ledger)?;
            return Ok((version, key));
        }
        self.create_scope_key(scope_id, &mut ledger)
    }

    fn new_scope_key(&self, scope_id: &str) -> Result<(u32, [u8; KEY_LEN]), MemoryPayloadError> {
        let mut ledger = self
            .ledger
            .write()
            .map_err(|_| MemoryPayloadError::Crypto("memory key ledger lock".into()))?;
        self.create_scope_key(scope_id, &mut ledger)
    }

    fn scope_key_version(
        &self,
        scope_id: &str,
        version: u32,
    ) -> Result<[u8; KEY_LEN], MemoryPayloadError> {
        let ledger = self
            .ledger
            .read()
            .map_err(|_| MemoryPayloadError::Crypto("memory key ledger lock".into()))?;
        self.unwrap_ledger_key(scope_id, version, &ledger)
    }

    fn create_scope_key(
        &self,
        scope_id: &str,
        ledger: &mut KekLedger,
    ) -> Result<(u32, [u8; KEY_LEN]), MemoryPayloadError> {
        if scope_id.trim().is_empty() {
            return Err(MemoryPayloadError::Corrupt(
                "scope id must not be empty".into(),
            ));
        }
        let version = ledger
            .entries
            .keys()
            .filter(|(scope, _)| scope == scope_id)
            .map(|(_, version)| *version)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let rng = SystemRandom::new();
        let mut key = [0u8; KEY_LEN];
        let mut nonce = [0u8; NONCE_LEN];
        rng.fill(&mut key)
            .map_err(|_| MemoryPayloadError::Crypto("cannot generate scope key".into()))?;
        rng.fill(&mut nonce)
            .map_err(|_| MemoryPayloadError::Crypto("cannot generate nonce".into()))?;
        let aad = kek_aad(scope_id, version);
        let wrapped = seal(&self.wrap_key(), &nonce, &key, &aad)?;
        let key_id = (scope_id.into(), version);
        ledger.entries.insert(
            key_id.clone(),
            KekLedgerEntry {
                scope_id: scope_id.into(),
                version,
                nonce_hex: hex::encode(nonce),
                wrapped_key_hex: hex::encode(wrapped),
            },
        );
        if let Err(error) = self.persist_ledger(ledger) {
            ledger.entries.remove(&key_id);
            return Err(error);
        }
        Ok((version, key))
    }

    fn unwrap_ledger_key(
        &self,
        scope_id: &str,
        version: u32,
        ledger: &KekLedger,
    ) -> Result<[u8; KEY_LEN], MemoryPayloadError> {
        let entry = ledger
            .entries
            .get(&(scope_id.into(), version))
            .ok_or_else(|| {
                MemoryPayloadError::Crypto(format!(
                    "scope key {scope_id} version {version} is retired or missing"
                ))
            })?;
        let nonce = decode_nonce(&entry.nonce_hex)?;
        let wrapped = hex::decode(&entry.wrapped_key_hex)
            .map_err(|_| MemoryPayloadError::Corrupt("wrapped scope key is not hex".into()))?;
        let key = open(
            &self.wrap_key(),
            &nonce,
            &wrapped,
            &kek_aad(scope_id, version),
        )?;
        if key.len() != KEY_LEN {
            return Err(MemoryPayloadError::Corrupt(
                "scope key has wrong length".into(),
            ));
        }
        let mut output = [0u8; KEY_LEN];
        output.copy_from_slice(&key);
        Ok(output)
    }

    fn retire_scope_versions_before(
        &self,
        scope_id: &str,
        version: u32,
    ) -> Result<Vec<u32>, MemoryPayloadError> {
        let mut ledger = self
            .ledger
            .write()
            .map_err(|_| MemoryPayloadError::Crypto("memory key ledger lock".into()))?;
        let retired: Vec<u32> = ledger
            .entries
            .keys()
            .filter(|(scope, existing)| scope == scope_id && *existing < version)
            .map(|(_, existing)| *existing)
            .collect();
        if retired.is_empty() {
            return Ok(retired);
        }
        let removed: Vec<(String, u32, KekLedgerEntry)> = retired
            .iter()
            .filter_map(|old| {
                ledger
                    .entries
                    .remove(&(scope_id.into(), *old))
                    .map(|entry| (scope_id.into(), *old, entry))
            })
            .collect();
        if let Err(error) = self.persist_ledger(&ledger) {
            for (scope, old, entry) in removed {
                ledger.entries.insert((scope, old), entry);
            }
            return Err(error);
        }
        Ok(retired)
    }

    fn persist_ledger(&self, ledger: &KekLedger) -> Result<(), MemoryPayloadError> {
        let file = KekLedgerFile {
            schema_version: KEK_LEDGER_SCHEMA_VERSION,
            entries: ledger.entries.values().cloned().collect(),
        };
        let bytes = serde_json::to_vec_pretty(&file)?;
        write_private_atomic(&self.ledger_path, &bytes)
    }
}

fn ledger_path_for(key_path: &Path) -> PathBuf {
    let name = key_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("memory.key");
    key_path.with_file_name(format!("{name}-keks.json"))
}

fn load_ledger(path: &Path) -> Result<KekLedger, MemoryPayloadError> {
    let mut ledger = KekLedger::default();
    if !path.exists() {
        return Ok(ledger);
    }
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(MemoryPayloadError::Corrupt(
            "scope key ledger must be a regular file".into(),
        ));
    }
    check_private_permissions(path)?;
    let bytes = std::fs::read(path)?;
    let file: KekLedgerFile = serde_json::from_slice(&bytes)?;
    if file.schema_version != KEK_LEDGER_SCHEMA_VERSION {
        return Err(MemoryPayloadError::Corrupt(
            "unsupported scope key ledger schema".into(),
        ));
    }
    for entry in file.entries {
        if entry.scope_id.trim().is_empty() || entry.version == 0 {
            return Err(MemoryPayloadError::Corrupt(
                "invalid scope key ledger entry".into(),
            ));
        }
        let key = (entry.scope_id.clone(), entry.version);
        if ledger.entries.insert(key, entry).is_some() {
            return Err(MemoryPayloadError::Corrupt(
                "duplicate scope key ledger entry".into(),
            ));
        }
    }
    Ok(ledger)
}

fn write_private_atomic(path: &Path, bytes: &[u8]) -> Result<(), MemoryPayloadError> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("memory-keks.json");
    let temp = path.with_file_name(format!(".{file_name}.{}.tmp", Uuid::new_v4()));
    let result = (|| -> Result<(), MemoryPayloadError> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let mut file = options.open(&temp)?;
        set_private_permissions(&file)?;
        file.write_all(bytes)?;
        file.sync_data()?;
        drop(file);
        std::fs::rename(&temp, path)?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temp);
        return Err(error);
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        sync_directory(parent)?;
    }
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), MemoryPayloadError> {
    File::open(path)?.sync_all()?;
    Ok(())
}

fn kek_aad(scope_id: &str, version: u32) -> Vec<u8> {
    format!("memory-kek:{scope_id}:{version}").into_bytes()
}

fn root_has_schema_two_payload(root: &Path) -> Result<bool, MemoryPayloadError> {
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if !entry
            .file_name()
            .to_string_lossy()
            .ends_with(".payload.json")
        {
            continue;
        }
        let Ok(bytes) = std::fs::read(entry.path()) else {
            continue;
        };
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            continue;
        };
        if value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|version| version >= u64::from(PAYLOAD_SCHEMA_VERSION))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn ensure_regular_payload(path: &Path) -> Result<(), MemoryPayloadError> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(MemoryPayloadError::Corrupt(
            "payload must be a regular file".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct MemoryPayloadStore {
    root: PathBuf,
    keyring: MemoryKeyring,
    rotation: Arc<RwLock<()>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct EncryptedPayload {
    schema_version: u32,
    #[serde(default)]
    kek_version: u32,
    memory_id: String,
    scope_id: String,
    nonce_hex: String,
    ciphertext_hex: String,
    wrapped_dek_nonce_hex: String,
    wrapped_dek_hex: String,
    content_sha256: String,
}

impl MemoryPayloadStore {
    pub fn open(
        root: impl Into<PathBuf>,
        key_path: impl AsRef<Path>,
    ) -> Result<Self, MemoryPayloadError> {
        let root = root.into();
        std::fs::create_dir_all(&root)?;
        set_dir_permissions(&root)?;
        let keyring = MemoryKeyring::open_or_create(key_path)?;
        if keyring.ledger_is_empty()? && root_has_schema_two_payload(&root)? {
            return Err(MemoryPayloadError::Corrupt(
                "scope key ledger is missing for existing schema-2 payloads".into(),
            ));
        }
        Ok(Self {
            root,
            keyring,
            rotation: Arc::new(RwLock::new(())),
        })
    }

    fn read_rotation(&self) -> Result<std::sync::RwLockReadGuard<'_, ()>, MemoryPayloadError> {
        self.rotation
            .read()
            .map_err(|_| MemoryPayloadError::Crypto("memory rotation lock".into()))
    }

    fn write_rotation(&self) -> Result<std::sync::RwLockWriteGuard<'_, ()>, MemoryPayloadError> {
        self.rotation
            .write()
            .map_err(|_| MemoryPayloadError::Crypto("memory rotation lock".into()))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn put(
        &self,
        memory_id: &str,
        scope_id: &str,
        content: &str,
    ) -> Result<(), MemoryPayloadError> {
        if scope_id.trim().is_empty() {
            return Err(MemoryPayloadError::Corrupt(
                "scope id must not be empty".into(),
            ));
        }
        let _guard = self.read_rotation()?;
        let (kek_version, scope_key) = self.keyring.ensure_scope_key(scope_id)?;
        let rng = SystemRandom::new();
        let mut dek = [0u8; KEY_LEN];
        let mut content_nonce = [0u8; NONCE_LEN];
        let mut wrap_nonce = [0u8; NONCE_LEN];
        rng.fill(&mut dek)
            .map_err(|_| MemoryPayloadError::Crypto("cannot generate DEK".into()))?;
        rng.fill(&mut content_nonce)
            .map_err(|_| MemoryPayloadError::Crypto("cannot generate nonce".into()))?;
        rng.fill(&mut wrap_nonce)
            .map_err(|_| MemoryPayloadError::Crypto("cannot generate wrap nonce".into()))?;
        let aad = payload_aad(memory_id, scope_id);
        let ciphertext = seal(&dek, &content_nonce, content.as_bytes(), &aad)?;
        let wrapped_dek = seal(&scope_key, &wrap_nonce, &dek, &aad)?;
        let payload = EncryptedPayload {
            schema_version: PAYLOAD_SCHEMA_VERSION,
            kek_version,
            memory_id: memory_id.to_string(),
            scope_id: scope_id.to_string(),
            nonce_hex: hex::encode(content_nonce),
            ciphertext_hex: hex::encode(ciphertext),
            wrapped_dek_nonce_hex: hex::encode(wrap_nonce),
            wrapped_dek_hex: hex::encode(wrapped_dek),
            content_sha256: sha256_hex(content.as_bytes()),
        };
        self.write_payload(&payload)
    }

    pub fn get(&self, memory_id: &str, scope_id: &str) -> Result<String, MemoryPayloadError> {
        let _guard = self.read_rotation()?;
        let path = self.path_for(memory_id)?;
        ensure_regular_payload(&path)?;
        let bytes = std::fs::read(&path)?;
        let payload: EncryptedPayload = serde_json::from_slice(&bytes)?;
        let scope_key = self.scope_key_for(&payload)?;
        if payload.memory_id != memory_id {
            return Err(MemoryPayloadError::Corrupt("payload id mismatch".into()));
        }
        if payload.scope_id != scope_id {
            return Err(MemoryPayloadError::ScopeMismatch {
                expected: scope_id.to_string(),
                actual: payload.scope_id,
            });
        }
        let aad = payload_aad(memory_id, scope_id);
        let content_nonce = decode_nonce(&payload.nonce_hex)?;
        let wrap_nonce = decode_nonce(&payload.wrapped_dek_nonce_hex)?;
        let wrapped_dek = hex::decode(&payload.wrapped_dek_hex)
            .map_err(|_| MemoryPayloadError::Corrupt("wrapped DEK is not hex".into()))?;
        let ciphertext = hex::decode(&payload.ciphertext_hex)
            .map_err(|_| MemoryPayloadError::Corrupt("ciphertext is not hex".into()))?;
        let dek = open(&scope_key, &wrap_nonce, &wrapped_dek, &aad)?;
        if dek.len() != KEY_LEN {
            return Err(MemoryPayloadError::Corrupt("DEK has wrong length".into()));
        }
        let mut dek_bytes = [0u8; KEY_LEN];
        dek_bytes.copy_from_slice(&dek);
        let plaintext = open(&dek_bytes, &content_nonce, &ciphertext, &aad)?;
        if sha256_hex(&plaintext) != payload.content_sha256 {
            return Err(MemoryPayloadError::Corrupt(
                "payload content hash mismatch".into(),
            ));
        }
        String::from_utf8(plaintext)
            .map_err(|_| MemoryPayloadError::Corrupt("payload is not UTF-8".into()))
    }

    /// Rewrap every payload in one scope to a fresh random KEK, then retire
    /// older ledger versions. Legacy schema-v1 payloads are migrated too, but
    /// their master-derived key cannot be retired; that requires master-key
    /// rotation in a later step.
    pub fn rotate_scope_key(&self, scope_id: &str) -> Result<ScopeKeyRotation, MemoryPayloadError> {
        if scope_id.trim().is_empty() {
            return Err(MemoryPayloadError::Corrupt(
                "scope id must not be empty".into(),
            ));
        }
        let _guard = self.write_rotation()?;
        let (new_version, new_key) = self.keyring.new_scope_key(scope_id)?;
        let mut paths = Vec::new();
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry
                .file_name()
                .to_string_lossy()
                .ends_with(".payload.json")
            {
                continue;
            }
            let metadata = std::fs::symlink_metadata(entry.path())?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(MemoryPayloadError::Corrupt(
                    "payload must be a regular file".into(),
                ));
            }
            paths.push(entry.path());
        }
        paths.sort();
        let rng = SystemRandom::new();
        let mut rotated = 0;
        let mut legacy_migrated = 0;
        for path in paths {
            let bytes = std::fs::read(&path)?;
            let mut payload: EncryptedPayload = serde_json::from_slice(&bytes)?;
            if payload.scope_id != scope_id {
                continue;
            }
            if payload.memory_id.trim().is_empty() {
                return Err(MemoryPayloadError::Corrupt("payload id is empty".into()));
            }
            let old_key = if payload.schema_version == LEGACY_PAYLOAD_SCHEMA_VERSION {
                legacy_migrated += 1;
                self.keyring.legacy_scope_key(scope_id)
            } else if payload.schema_version == PAYLOAD_SCHEMA_VERSION {
                if payload.kek_version == 0 {
                    return Err(MemoryPayloadError::Corrupt(
                        "payload schema 2 has no key version".into(),
                    ));
                }
                self.keyring
                    .scope_key_version(scope_id, payload.kek_version)?
            } else {
                return Err(MemoryPayloadError::Corrupt(
                    "unsupported payload schema version".into(),
                ));
            };
            let aad = payload_aad(&payload.memory_id, scope_id);
            let old_wrap_nonce = decode_nonce(&payload.wrapped_dek_nonce_hex)?;
            let wrapped_dek = hex::decode(&payload.wrapped_dek_hex)
                .map_err(|_| MemoryPayloadError::Corrupt("wrapped DEK is not hex".into()))?;
            let dek = open(&old_key, &old_wrap_nonce, &wrapped_dek, &aad)?;
            if dek.len() != KEY_LEN {
                return Err(MemoryPayloadError::Corrupt("DEK has wrong length".into()));
            }
            let mut dek_bytes = [0u8; KEY_LEN];
            dek_bytes.copy_from_slice(&dek);
            let mut new_nonce = [0u8; NONCE_LEN];
            rng.fill(&mut new_nonce)
                .map_err(|_| MemoryPayloadError::Crypto("cannot generate nonce".into()))?;
            payload.schema_version = PAYLOAD_SCHEMA_VERSION;
            payload.kek_version = new_version;
            payload.wrapped_dek_nonce_hex = hex::encode(new_nonce);
            payload.wrapped_dek_hex = hex::encode(seal(&new_key, &new_nonce, &dek_bytes, &aad)?);
            self.write_payload(&payload)?;
            rotated += 1;
        }
        sync_directory(&self.root)?;
        let retired_versions = self
            .keyring
            .retire_scope_versions_before(scope_id, new_version)?;
        Ok(ScopeKeyRotation {
            scope_id: scope_id.into(),
            new_version,
            rotated,
            retired_versions,
            legacy_migrated,
        })
    }

    fn scope_key_for(
        &self,
        payload: &EncryptedPayload,
    ) -> Result<[u8; KEY_LEN], MemoryPayloadError> {
        match payload.schema_version {
            LEGACY_PAYLOAD_SCHEMA_VERSION => Ok(self.keyring.legacy_scope_key(&payload.scope_id)),
            PAYLOAD_SCHEMA_VERSION => {
                if payload.kek_version == 0 {
                    return Err(MemoryPayloadError::Corrupt(
                        "payload schema 2 has no key version".into(),
                    ));
                }
                self.keyring
                    .scope_key_version(&payload.scope_id, payload.kek_version)
            }
            _ => Err(MemoryPayloadError::Corrupt(
                "unsupported payload schema version".into(),
            )),
        }
    }

    fn write_payload(&self, payload: &EncryptedPayload) -> Result<(), MemoryPayloadError> {
        let path = self.path_for(&payload.memory_id)?;
        let bytes = serde_json::to_vec(payload)?;
        let temp = self
            .root
            .join(format!(".{}.{}.tmp", payload.memory_id, Uuid::new_v4()));
        let result = (|| -> Result<(), MemoryPayloadError> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            set_private_permissions(&file)?;
            file.write_all(&bytes)?;
            file.sync_data()?;
            std::fs::rename(&temp, &path)?;
            sync_directory(&self.root)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        result
    }

    pub fn forget(&self, memory_id: &str) -> Result<(), MemoryPayloadError> {
        let _guard = self.read_rotation()?;
        let path = self.path_for(memory_id)?;
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let temp_prefix = format!(".{memory_id}.");
        if let Ok(entries) = std::fs::read_dir(&self.root) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with(&temp_prefix) && name.ends_with(".tmp") {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        Ok(())
    }

    pub fn contains(&self, memory_id: &str) -> Result<bool, MemoryPayloadError> {
        let _guard = self.read_rotation()?;
        let path = self.path_for(memory_id)?;
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => Ok(true),
            Ok(_) => Err(MemoryPayloadError::Corrupt(
                "payload must be a regular file".into(),
            )),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    fn path_for(&self, memory_id: &str) -> Result<PathBuf, MemoryPayloadError> {
        if !safe_id(memory_id) {
            return Err(MemoryPayloadError::UnsafeId(memory_id.to_string()));
        }
        Ok(self.root.join(format!("{memory_id}.payload.json")))
    }
}

fn payload_aad(memory_id: &str, scope_id: &str) -> Vec<u8> {
    format!("{memory_id}\0{scope_id}").into_bytes()
}

fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        && !value.starts_with('.')
}

fn decode_nonce(value: &str) -> Result<[u8; NONCE_LEN], MemoryPayloadError> {
    let bytes =
        hex::decode(value).map_err(|_| MemoryPayloadError::Corrupt("nonce is not hex".into()))?;
    if bytes.len() != NONCE_LEN {
        return Err(MemoryPayloadError::Corrupt("nonce has wrong length".into()));
    }
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&bytes);
    Ok(nonce)
}

fn seal(
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, MemoryPayloadError> {
    let unbound = UnboundKey::new(&AES_256_GCM, key)
        .map_err(|_| MemoryPayloadError::Crypto("cannot build AEAD key".into()))?;
    let key = LessSafeKey::new(unbound);
    let nonce = Nonce::assume_unique_for_key(*nonce);
    let mut output = plaintext.to_vec();
    key.seal_in_place_append_tag(nonce, Aad::from(aad), &mut output)
        .map_err(|_| MemoryPayloadError::Crypto("cannot encrypt payload".into()))?;
    Ok(output)
}

fn open(
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    ciphertext: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, MemoryPayloadError> {
    let unbound = UnboundKey::new(&AES_256_GCM, key)
        .map_err(|_| MemoryPayloadError::Crypto("cannot build AEAD key".into()))?;
    let key = LessSafeKey::new(unbound);
    let nonce = Nonce::assume_unique_for_key(*nonce);
    let mut output = ciphertext.to_vec();
    let plaintext = key
        .open_in_place(nonce, Aad::from(aad), &mut output)
        .map_err(|_| MemoryPayloadError::Crypto("cannot decrypt payload".into()))?;
    Ok(plaintext.to_vec())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(unix)]
fn set_private_permissions(file: &File) -> Result<(), MemoryPayloadError> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = file.metadata()?.permissions();
    permissions.set_mode(0o600);
    file.set_permissions(permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_permissions(_file: &File) -> Result<(), MemoryPayloadError> {
    Ok(())
}

#[cfg(unix)]
fn set_dir_permissions(path: &Path) -> Result<(), MemoryPayloadError> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_dir_permissions(_path: &Path) -> Result<(), MemoryPayloadError> {
    Ok(())
}

#[cfg(unix)]
fn check_private_permissions(path: &Path) -> Result<(), MemoryPayloadError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let metadata = std::fs::metadata(path)?;
    if !metadata.file_type().is_file() {
        return Err(MemoryPayloadError::Corrupt(
            "memory key must be a regular file".into(),
        ));
    }
    if metadata.uid() != unsafe { libc::geteuid() } {
        return Err(MemoryPayloadError::Corrupt(
            "memory key must be owned by the current user".into(),
        ));
    }
    let mode = metadata.permissions().mode();
    if mode & 0o077 != 0 {
        return Err(MemoryPayloadError::Corrupt(
            "memory key permissions must be 0600".into(),
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_private_permissions(_path: &Path) -> Result<(), MemoryPayloadError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_round_trip_does_not_store_plaintext() {
        let dir = tempfile::tempdir().unwrap();
        let store =
            MemoryPayloadStore::open(dir.path().join("payloads"), dir.path().join("memory.key"))
                .unwrap();
        store
            .put("mem-1", "bot-1:ws-1", "private body text")
            .unwrap();
        assert_eq!(
            store.get("mem-1", "bot-1:ws-1").unwrap(),
            "private body text"
        );
        let raw = std::fs::read_to_string(dir.path().join("payloads/mem-1.payload.json")).unwrap();
        assert!(!raw.contains("private body text"));
        assert!(store.contains("mem-1").unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn key_creation_is_complete_private_and_leaves_no_temp_file() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("memory.key");
        MemoryKeyring::open_or_create(&key_path).unwrap();
        let bytes = std::fs::read(&key_path).unwrap();
        assert_eq!(bytes.len(), 32);
        let mode = std::fs::metadata(&key_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let leftovers = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp"))
            .count();
        assert_eq!(leftovers, 0);
    }

    #[test]
    fn keyring_debug_redacts_material() {
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("memory.key");
        let keyring = MemoryKeyring::open_or_create(&key_path).unwrap();
        let encoded = hex::encode(std::fs::read(&key_path).unwrap());
        let debug = format!("{keyring:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains(&encoded));
    }

    #[test]
    fn scope_mismatch_and_wrong_key_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("payloads");
        let key = dir.path().join("memory.key");
        let store = MemoryPayloadStore::open(&root, &key).unwrap();
        store.put("mem-1", "scope-a", "secret body").unwrap();
        assert!(matches!(
            store.get("mem-1", "scope-b").unwrap_err(),
            MemoryPayloadError::ScopeMismatch { .. }
        ));

        let other_key = dir.path().join("other.key");
        assert!(matches!(
            MemoryPayloadStore::open(&root, &other_key).unwrap_err(),
            MemoryPayloadError::Corrupt(_)
        ));
    }

    #[test]
    fn hard_forget_deletes_payload() {
        let dir = tempfile::tempdir().unwrap();
        let store =
            MemoryPayloadStore::open(dir.path().join("payloads"), dir.path().join("memory.key"))
                .unwrap();
        store.put("mem-1", "scope-a", "body").unwrap();
        assert!(store.contains("mem-1").unwrap());
        store.forget("mem-1").unwrap();
        assert!(!store.contains("mem-1").unwrap());
        store.forget("mem-1").unwrap();
    }

    #[test]
    fn tampered_ciphertext_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("payloads");
        let store = MemoryPayloadStore::open(&root, dir.path().join("memory.key")).unwrap();
        store.put("mem-1", "scope-a", "body").unwrap();
        let path = root.join("mem-1.payload.json");
        let mut payload: EncryptedPayload =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let mut bytes = hex::decode(&payload.ciphertext_hex).unwrap();
        bytes[0] ^= 0xff;
        payload.ciphertext_hex = hex::encode(bytes);
        std::fs::write(&path, serde_json::to_vec(&payload).unwrap()).unwrap();
        assert!(matches!(
            store.get("mem-1", "scope-a").unwrap_err(),
            MemoryPayloadError::Crypto(_)
        ));
    }

    #[test]
    fn scope_key_rotation_rewraps_and_retires_old_versions() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("payloads");
        let store = MemoryPayloadStore::open(&root, dir.path().join("memory.key")).unwrap();
        store.put("mem-1", "scope-a", "one").unwrap();
        store.put("mem-2", "scope-a", "two").unwrap();
        store.put("mem-3", "scope-b", "three").unwrap();
        let old_payload = std::fs::read(root.join("mem-1.payload.json")).unwrap();

        let rotation = store.rotate_scope_key("scope-a").unwrap();
        assert_eq!(rotation.new_version, 2);
        assert_eq!(rotation.rotated, 2);
        assert_eq!(rotation.retired_versions, vec![1]);
        assert_eq!(rotation.legacy_migrated, 0);
        assert_eq!(store.get("mem-1", "scope-a").unwrap(), "one");
        assert_eq!(store.get("mem-2", "scope-a").unwrap(), "two");
        assert_eq!(store.get("mem-3", "scope-b").unwrap(), "three");

        // A backup taken before rotation references the retired version.
        std::fs::write(root.join("mem-1.payload.json"), old_payload).unwrap();
        assert!(matches!(
            store.get("mem-1", "scope-a").unwrap_err(),
            MemoryPayloadError::Crypto(_)
        ));
    }

    #[test]
    fn missing_or_tampered_ledger_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("payloads");
        let key_path = dir.path().join("memory.key");
        let store = MemoryPayloadStore::open(&root, &key_path).unwrap();
        store.put("mem-1", "scope-a", "secret").unwrap();
        drop(store);

        let ledger_path = dir.path().join("memory.key-keks.json");
        let mut ledger: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&ledger_path).unwrap()).unwrap();
        ledger["entries"][0]["wrapped_key_hex"] = "00".into();
        std::fs::write(&ledger_path, serde_json::to_vec(&ledger).unwrap()).unwrap();
        let tampered = MemoryPayloadStore::open(&root, &key_path).unwrap();
        assert!(matches!(
            tampered.get("mem-1", "scope-a").unwrap_err(),
            MemoryPayloadError::Crypto(_) | MemoryPayloadError::Corrupt(_)
        ));

        std::fs::remove_file(&ledger_path).unwrap();
        assert!(matches!(
            MemoryPayloadStore::open(&root, &key_path).unwrap_err(),
            MemoryPayloadError::Corrupt(_)
        ));
    }

    #[test]
    fn legacy_schema_one_payload_still_reads_and_migrates() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("payloads");
        let store = MemoryPayloadStore::open(&root, dir.path().join("memory.key")).unwrap();
        let scope = "scope-legacy";
        let legacy_key = store.keyring.legacy_scope_key(scope);
        let rng = SystemRandom::new();
        let mut dek = [0u8; KEY_LEN];
        let mut content_nonce = [0u8; NONCE_LEN];
        let mut wrap_nonce = [0u8; NONCE_LEN];
        rng.fill(&mut dek).unwrap();
        rng.fill(&mut content_nonce).unwrap();
        rng.fill(&mut wrap_nonce).unwrap();
        let aad = payload_aad("mem-legacy", scope);
        let ciphertext = seal(&dek, &content_nonce, b"legacy body", &aad).unwrap();
        let wrapped = seal(&legacy_key, &wrap_nonce, &dek, &aad).unwrap();
        let payload = EncryptedPayload {
            schema_version: LEGACY_PAYLOAD_SCHEMA_VERSION,
            kek_version: 0,
            memory_id: "mem-legacy".into(),
            scope_id: scope.into(),
            nonce_hex: hex::encode(content_nonce),
            ciphertext_hex: hex::encode(ciphertext),
            wrapped_dek_nonce_hex: hex::encode(wrap_nonce),
            wrapped_dek_hex: hex::encode(wrapped),
            content_sha256: sha256_hex(b"legacy body"),
        };
        store.write_payload(&payload).unwrap();
        assert_eq!(store.get("mem-legacy", scope).unwrap(), "legacy body");

        let rotation = store.rotate_scope_key(scope).unwrap();
        assert_eq!(rotation.legacy_migrated, 1);
        assert_eq!(rotation.new_version, 1);
        assert_eq!(store.get("mem-legacy", scope).unwrap(), "legacy body");
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_payload_is_rejected_by_read_and_rotation() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("payloads");
        let store = MemoryPayloadStore::open(&root, dir.path().join("memory.key")).unwrap();
        store.put("mem-1", "scope-a", "secret").unwrap();
        let path = root.join("mem-1.payload.json");
        let target = dir.path().join("outside.json");
        std::fs::rename(&path, &target).unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        assert!(matches!(
            store.get("mem-1", "scope-a").unwrap_err(),
            MemoryPayloadError::Corrupt(_)
        ));
        assert!(matches!(
            store.contains("mem-1").unwrap_err(),
            MemoryPayloadError::Corrupt(_)
        ));
        assert!(matches!(
            store.rotate_scope_key("scope-a").unwrap_err(),
            MemoryPayloadError::Corrupt(_)
        ));
    }

    #[test]
    fn unsafe_ids_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let store =
            MemoryPayloadStore::open(dir.path().join("payloads"), dir.path().join("memory.key"))
                .unwrap();
        assert!(matches!(
            store.put("../escape", "scope", "body").unwrap_err(),
            MemoryPayloadError::UnsafeId(_)
        ));
    }
}
