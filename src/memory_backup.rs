//! Memory backup export and verification.
//!
//! Backups deliberately exclude the master key. Under the MVP policy, hard
//! forget covers the live payload store only; an existing backup may still hold
//! ciphertext and is not claimed to be unrecoverable.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const BACKUP_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum MemoryBackupError {
    #[error("memory backup io: {0}")]
    Io(#[from] std::io::Error),
    #[error("memory backup json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("memory backup output is not empty: {0}")]
    OutputNotEmpty(PathBuf),
    #[error("memory backup layout is unsafe: {0}")]
    UnsafeLayout(String),
    #[error("memory backup must not contain a key file: {0}")]
    KeyIncluded(String),
    #[error("memory backup hash mismatch: {0}")]
    HashMismatch(String),
    #[error("memory backup size mismatch: {0}")]
    SizeMismatch(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BackupFile {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BackupManifest {
    pub schema_version: u32,
    pub created_ms: u128,
    pub files: Vec<BackupFile>,
    pub key_included: bool,
    pub hard_forget_covers_backup: bool,
}

/// Export a memory journal and encrypted payloads. The key file is never read
/// or copied, even when it sits next to the payload root.
pub fn export_memory_backup(
    journal_path: &Path,
    payload_root: &Path,
    output_dir: &Path,
    now_ms: u128,
) -> Result<BackupManifest, MemoryBackupError> {
    prepare_empty_output(output_dir)?;
    let payload_out = output_dir.join("payloads");
    crate::jsonl::ensure_parent_private(&payload_out.join(".keep"))?;

    let journal_out = output_dir.join("memory-journal.jsonl");
    copy_private(journal_path, &journal_out)?;

    let mut files = Vec::new();
    files.push(record_file(output_dir, &journal_out)?);
    let mut payloads = Vec::new();
    for entry in std::fs::read_dir(payload_root)? {
        let entry = entry?;
        let metadata = std::fs::symlink_metadata(entry.path())?;
        if !metadata.is_file() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name.ends_with(".payload.json") {
            continue;
        }
        payloads.push((name.to_string(), entry.path()));
    }
    payloads.sort_by(|left, right| left.0.cmp(&right.0));
    for (name, source) in payloads {
        let target = payload_out.join(&name);
        copy_private(&source, &target)?;
        files.push(record_file(output_dir, &target)?);
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));

    let manifest = BackupManifest {
        schema_version: BACKUP_SCHEMA_VERSION,
        created_ms: now_ms,
        files,
        key_included: false,
        hard_forget_covers_backup: false,
    };
    let bytes = serde_json::to_vec_pretty(&manifest)?;
    write_private(&output_dir.join("manifest.json"), &bytes)?;
    Ok(manifest)
}

/// Verify the manifest, every listed file and the absence of key material.
pub fn verify_memory_backup(output_dir: &Path) -> Result<BackupManifest, MemoryBackupError> {
    let manifest_path = output_dir.join("manifest.json");
    let bytes = std::fs::read(&manifest_path)?;
    let manifest: BackupManifest = serde_json::from_slice(&bytes)?;
    if manifest.schema_version != BACKUP_SCHEMA_VERSION {
        return Err(MemoryBackupError::UnsafeLayout(format!(
            "unsupported backup schema {}",
            manifest.schema_version
        )));
    }
    if manifest.key_included {
        return Err(MemoryBackupError::KeyIncluded(
            manifest_path.display().to_string(),
        ));
    }
    if manifest.hard_forget_covers_backup {
        return Err(MemoryBackupError::UnsafeLayout(
            "MVP backups must not claim hard-forget coverage".into(),
        ));
    }
    let mut allowed = std::collections::BTreeSet::new();
    allowed.insert("manifest.json".to_string());
    for file in &manifest.files {
        let relative = Path::new(&file.path);
        if relative.is_absolute()
            || relative.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::ParentDir
                        | std::path::Component::RootDir
                        | std::path::Component::Prefix(_)
                )
            })
        {
            return Err(MemoryBackupError::UnsafeLayout(file.path.clone()));
        }
        allowed.insert(file.path.clone());
    }
    for entry in walk_files(output_dir)? {
        let relative = entry
            .strip_prefix(output_dir)
            .map_err(|_| MemoryBackupError::UnsafeLayout(entry.display().to_string()))?;
        let name = relative.to_string_lossy();
        if name.ends_with(".key") || name.ends_with("memory.key") || name.ends_with("-keks.json") {
            return Err(MemoryBackupError::KeyIncluded(name.into_owned()));
        }
        if !allowed.contains(name.as_ref()) {
            return Err(MemoryBackupError::UnsafeLayout(format!(
                "unexpected file in backup: {name}"
            )));
        }
    }
    for file in &manifest.files {
        let path = output_dir.join(&file.path);
        let metadata = std::fs::symlink_metadata(&path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(MemoryBackupError::UnsafeLayout(file.path.clone()));
        }
        let actual = hash_file(&path)?;
        if actual != file.sha256 {
            return Err(MemoryBackupError::HashMismatch(file.path.clone()));
        }
        if metadata.len() != file.bytes {
            return Err(MemoryBackupError::SizeMismatch(file.path.clone()));
        }
    }
    Ok(manifest)
}

fn prepare_empty_output(output_dir: &Path) -> Result<(), MemoryBackupError> {
    if output_dir.exists() {
        let mut entries = std::fs::read_dir(output_dir)?;
        if entries.next().is_some() {
            return Err(MemoryBackupError::OutputNotEmpty(output_dir.to_path_buf()));
        }
    } else {
        std::fs::create_dir_all(output_dir)?;
    }
    crate::jsonl::ensure_parent_private(&output_dir.join(".keep"))?;
    Ok(())
}

fn copy_private(source: &Path, target: &Path) -> Result<u64, MemoryBackupError> {
    let metadata = std::fs::symlink_metadata(source)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(MemoryBackupError::UnsafeLayout(
            source.display().to_string(),
        ));
    }
    let mut input = File::open(source)?;
    let mut bytes = Vec::new();
    input.read_to_end(&mut bytes)?;
    write_private(target, &bytes)?;
    Ok(bytes.len() as u64)
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<(), MemoryBackupError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_data()?;
    Ok(())
}

fn record_file(root: &Path, path: &Path) -> Result<BackupFile, MemoryBackupError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| MemoryBackupError::UnsafeLayout(path.display().to_string()))?;
    Ok(BackupFile {
        path: relative.to_string_lossy().replace('\\', "/"),
        sha256: hash_file(path)?,
        bytes: std::fs::metadata(path)?.len(),
    })
}

fn hash_file(path: &Path) -> Result<String, MemoryBackupError> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(hex::encode(digest.finalize()))
}

fn walk_files(root: &Path) -> Result<Vec<PathBuf>, MemoryBackupError> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(MemoryBackupError::UnsafeLayout(path.display().to_string()));
            }
            if metadata.is_dir() {
                pending.push(path);
            } else if metadata.is_file() {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{
        DataClass, ForgetMode, MemoryKind, MemoryLayer, MemoryRecord, MemoryScope, MemorySource,
        MemorySourceKind, MemoryTaint,
    };
    use crate::memory_service::MemoryService;

    fn record() -> MemoryRecord {
        let mut record = MemoryRecord::new(
            "mem-1",
            MemoryLayer::Semantic,
            MemoryKind::Preference,
            MemoryScope::bot("bot-1"),
            "private preference body",
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
    fn export_excludes_the_key_and_verifies() {
        let dir = tempfile::tempdir().unwrap();
        let mut memory = service(dir.path());
        memory.remember_at(record(), 10).unwrap();
        drop(memory);

        let out = dir.path().join("backup");
        let manifest = export_memory_backup(
            &dir.path().join("memory-journal.jsonl"),
            &dir.path().join("payloads"),
            &out,
            20,
        )
        .unwrap();
        assert!(!manifest.key_included);
        assert!(!manifest.hard_forget_covers_backup);
        assert!(
            manifest
                .files
                .iter()
                .any(|file| file.path.ends_with(".payload.json"))
        );
        assert!(!walk_files(&out).unwrap().iter().any(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains("memory.key"))
        }));
        assert_eq!(verify_memory_backup(&out).unwrap(), manifest);
    }

    #[test]
    fn verify_rejects_tampered_payload_and_smuggled_key() {
        let dir = tempfile::tempdir().unwrap();
        let mut memory = service(dir.path());
        memory.remember_at(record(), 10).unwrap();
        drop(memory);
        let out = dir.path().join("backup");
        export_memory_backup(
            &dir.path().join("memory-journal.jsonl"),
            &dir.path().join("payloads"),
            &out,
            20,
        )
        .unwrap();
        let payload = walk_files(&out)
            .unwrap()
            .into_iter()
            .find(|path| path.to_string_lossy().ends_with(".payload.json"))
            .unwrap();
        let mut bytes = std::fs::read(&payload).unwrap();
        bytes[0] ^= 0xff;
        std::fs::write(&payload, bytes).unwrap();
        assert!(matches!(
            verify_memory_backup(&out).unwrap_err(),
            MemoryBackupError::HashMismatch(_)
        ));

        std::fs::write(out.join("memory.key"), b"stolen").unwrap();
        assert!(matches!(
            verify_memory_backup(&out).unwrap_err(),
            MemoryBackupError::KeyIncluded(_)
        ));
    }

    #[test]
    fn verify_rejects_ledger_and_unlisted_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut memory = service(dir.path());
        memory.remember_at(record(), 10).unwrap();
        drop(memory);
        let out = dir.path().join("backup");
        export_memory_backup(
            &dir.path().join("memory-journal.jsonl"),
            &dir.path().join("payloads"),
            &out,
            20,
        )
        .unwrap();
        let ledger = dir.path().join("memory.key-keks.json");
        assert!(ledger.exists());
        std::fs::copy(&ledger, out.join("memory.key-keks.json")).unwrap();
        assert!(matches!(
            verify_memory_backup(&out).unwrap_err(),
            MemoryBackupError::KeyIncluded(_)
        ));

        std::fs::remove_file(out.join("memory.key-keks.json")).unwrap();
        std::fs::write(out.join("notes.txt"), b"extra").unwrap();
        assert!(matches!(
            verify_memory_backup(&out).unwrap_err(),
            MemoryBackupError::UnsafeLayout(_)
        ));
    }

    #[test]
    fn hard_forget_removes_the_live_payload_but_not_an_existing_backup() {
        let dir = tempfile::tempdir().unwrap();
        let mut memory = service(dir.path());
        memory.remember_at(record(), 10).unwrap();
        drop(memory);
        let out = dir.path().join("backup");
        export_memory_backup(
            &dir.path().join("memory-journal.jsonl"),
            &dir.path().join("payloads"),
            &out,
            20,
        )
        .unwrap();

        let mut memory = service(dir.path());
        memory
            .forget("mem-1", &MemoryScope::bot("bot-1"), ForgetMode::Hard, 30)
            .unwrap();
        assert!(
            !memory
                .payload_contains("mem-1", &MemoryScope::bot("bot-1"))
                .unwrap()
        );
        assert!(
            walk_files(&out)
                .unwrap()
                .iter()
                .any(|path| path.to_string_lossy().ends_with(".payload.json"))
        );
        let manifest = verify_memory_backup(&out).unwrap();
        assert!(!manifest.hard_forget_covers_backup);
    }
}
