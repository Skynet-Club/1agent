//! Encrypted key escrow for the runtime home.
//!
//! The escrow bundle contains `memory.key` and, when present, the scope KEK
//! ledger, encrypted with AES-256-GCM under a random 256-bit recovery key. The
//! recovery key is written separately (0600) so losing the home does not lose
//! the key material. A password KDF and organization-level key custody are out
//! of scope; this is a local recovery mechanism, not a claim of HSM-grade
//! escrow.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use ring::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::runtime_home::{HOME_SCHEMA, HOME_VERSION, HomeManifest};

pub const ESCROW_SCHEMA: &str = "1agent.key-escrow.v1";
pub const MATERIAL_SCHEMA: &str = "1agent.key-material.v1";
pub const KEY_FILE: &str = "keys/memory.key";
pub const LEDGER_FILE: &str = "keys/memory.key-keks.json";

const NONCE_LEN: usize = 12;
const RECOVERY_KEY_LEN: usize = 32;

#[derive(Debug, Error)]
pub enum KeyEscrowError {
    #[error("key escrow io: {0}")]
    Io(#[from] std::io::Error),
    #[error("key escrow json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("not a 1Agent runtime home: {0}")]
    NotHome(PathBuf),
    #[error("unsupported runtime home version: found {found}, supported {supported}")]
    UnsupportedVersion { found: u32, supported: u32 },
    #[error("key material is missing: {0}")]
    MissingKey(PathBuf),
    #[error("key escrow layout is unsafe: {0}")]
    UnsafeLayout(String),
    #[error("key escrow output already exists: {0}")]
    OutputExists(PathBuf),
    #[error("recovery key is invalid: {0}")]
    InvalidRecoveryKey(String),
    #[error("key escrow cannot be decrypted with this recovery key")]
    DecryptFailed,
    #[error("key material is invalid: {0}")]
    InvalidMaterial(String),
    #[error("key material hash mismatch: {0}")]
    HashMismatch(String),
    #[error("refusing to overwrite existing key material: {0}")]
    RefuseOverwrite(PathBuf),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EscrowOutcome {
    pub output: PathBuf,
    pub recovery_key: PathBuf,
    pub created_recovery_key: bool,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct KeyRestoreOutcome {
    pub root: PathBuf,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct EscrowEnvelope {
    schema: String,
    created_ms: u128,
    nonce_hex: String,
    ciphertext_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct KeyMaterial {
    schema: String,
    files: Vec<MaterialFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct MaterialFile {
    path: String,
    sha256: String,
    bytes: u64,
    data_hex: String,
}

pub fn escrow(
    root: &Path,
    output: &Path,
    recovery_key_path: &Path,
    now_ms: u128,
) -> Result<EscrowOutcome, KeyEscrowError> {
    let root = std::path::absolute(root)?;
    require_home(&root)?;
    let output = std::path::absolute(output)?;
    let recovery_key_path = std::path::absolute(recovery_key_path)?;
    ensure_outside_home(&root, &output)?;
    ensure_outside_home(&root, &recovery_key_path)?;
    if output == recovery_key_path {
        return Err(KeyEscrowError::UnsafeLayout(
            "escrow output and recovery key must be different files".to_string(),
        ));
    }
    if existing_metadata(&output)?.is_some() {
        return Err(KeyEscrowError::OutputExists(output));
    }

    let mut files = Vec::new();
    for name in [KEY_FILE, LEDGER_FILE] {
        let path = root.join(name);
        let Some(metadata) = existing_metadata(&path)? else {
            if name == KEY_FILE {
                return Err(KeyEscrowError::MissingKey(path));
            }
            continue;
        };
        if !metadata.is_file() {
            return Err(KeyEscrowError::UnsafeLayout(format!(
                "{} is not a regular file",
                path.display()
            )));
        }
        let bytes = fs::read(&path)?;
        files.push(MaterialFile {
            path: name.to_string(),
            sha256: hex_sha256(&bytes),
            bytes: bytes.len() as u64,
            data_hex: hex_encode(&bytes),
        });
    }

    let material = KeyMaterial {
        schema: MATERIAL_SCHEMA.to_string(),
        files,
    };
    let plaintext = serde_json::to_vec(&material)?;
    let (recovery_key, created_recovery_key) = load_or_create_recovery_key(&recovery_key_path)?;
    let ciphertext = seal(&recovery_key, &plaintext)?;
    let envelope = EscrowEnvelope {
        schema: ESCROW_SCHEMA.to_string(),
        created_ms: now_ms,
        nonce_hex: hex_encode(&ciphertext.nonce),
        ciphertext_hex: hex_encode(&ciphertext.bytes),
    };
    write_new_private(&output, &serde_json::to_vec_pretty(&envelope)?)?;
    Ok(EscrowOutcome {
        output,
        recovery_key: recovery_key_path,
        created_recovery_key,
        files: material
            .files
            .iter()
            .map(|file| file.path.clone())
            .collect(),
    })
}

pub fn restore(
    root: &Path,
    escrow_path: &Path,
    recovery_key_path: &Path,
) -> Result<KeyRestoreOutcome, KeyEscrowError> {
    let root = std::path::absolute(root)?;
    require_home(&root)?;
    let escrow_path = std::path::absolute(escrow_path)?;
    let recovery_key_path = std::path::absolute(recovery_key_path)?;
    let metadata = existing_metadata(&escrow_path)?
        .ok_or_else(|| KeyEscrowError::MissingKey(escrow_path.clone()))?;
    if !metadata.is_file() {
        return Err(KeyEscrowError::UnsafeLayout(format!(
            "{} is not a regular file",
            escrow_path.display()
        )));
    }
    let envelope: EscrowEnvelope = serde_json::from_slice(&fs::read(&escrow_path)?)?;
    if envelope.schema != ESCROW_SCHEMA {
        return Err(KeyEscrowError::InvalidMaterial(format!(
            "unexpected escrow schema {}",
            envelope.schema
        )));
    }
    let recovery_key = read_recovery_key(&recovery_key_path)?;
    let nonce = hex_decode(&envelope.nonce_hex)?;
    if nonce.len() != NONCE_LEN {
        return Err(KeyEscrowError::InvalidMaterial(
            "escrow nonce has the wrong length".to_string(),
        ));
    }
    let mut ciphertext = hex_decode(&envelope.ciphertext_hex)?;
    let plaintext = open(&recovery_key, &nonce, &mut ciphertext)?;
    let material: KeyMaterial = serde_json::from_slice(&plaintext)?;
    if material.schema != MATERIAL_SCHEMA {
        return Err(KeyEscrowError::InvalidMaterial(format!(
            "unexpected material schema {}",
            material.schema
        )));
    }
    if material.files.is_empty() {
        return Err(KeyEscrowError::InvalidMaterial(
            "escrow contains no key files".to_string(),
        ));
    }

    let mut pending = Vec::new();
    for file in &material.files {
        if file.path != KEY_FILE && file.path != LEDGER_FILE {
            return Err(KeyEscrowError::InvalidMaterial(format!(
                "unexpected key path {}",
                file.path
            )));
        }
        if pending.iter().any(|(name, _, _)| name == &file.path) {
            return Err(KeyEscrowError::InvalidMaterial(format!(
                "duplicate key path {}",
                file.path
            )));
        }
        let data = hex_decode(&file.data_hex)?;
        if data.len() as u64 != file.bytes || hex_sha256(&data) != file.sha256 {
            return Err(KeyEscrowError::HashMismatch(file.path.clone()));
        }
        let target = root.join(&file.path);
        if existing_metadata(&target)?.is_some() {
            return Err(KeyEscrowError::RefuseOverwrite(target));
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
            }
        }
        pending.push((file.path.clone(), target, data));
    }
    if !pending.iter().any(|(name, _, _)| name == KEY_FILE) {
        return Err(KeyEscrowError::InvalidMaterial(
            "escrow does not contain the master key".to_string(),
        ));
    }
    // Write the ledger first and the master key last, so a partial restore does
    // not look like a usable key set.
    pending.sort_by_key(|(name, _, _)| name != LEDGER_FILE);
    let mut restored = Vec::new();
    for (name, target, data) in pending {
        write_new_private(&target, &data)?;
        restored.push(name);
    }
    sync_dir(&root)?;
    Ok(KeyRestoreOutcome {
        root,
        files: restored,
    })
}

fn require_home(root: &Path) -> Result<(), KeyEscrowError> {
    let marker = root.join("home.json");
    let metadata =
        existing_metadata(&marker)?.ok_or_else(|| KeyEscrowError::NotHome(root.into()))?;
    if !metadata.is_file() {
        return Err(KeyEscrowError::UnsafeLayout(format!(
            "{} is not a regular file",
            marker.display()
        )));
    }
    let manifest: HomeManifest = serde_json::from_slice(&fs::read(&marker)?)?;
    if manifest.schema != HOME_SCHEMA || manifest.version != HOME_VERSION {
        return Err(KeyEscrowError::UnsupportedVersion {
            found: manifest.version,
            supported: HOME_VERSION,
        });
    }
    Ok(())
}

fn ensure_outside_home(root: &Path, path: &Path) -> Result<(), KeyEscrowError> {
    let root = fs::canonicalize(root)?;
    let path = canonicalize_for_compare(path)?;
    if path.starts_with(&root) || root.starts_with(&path) {
        return Err(KeyEscrowError::UnsafeLayout(format!(
            "{} must be outside the runtime home",
            path.display()
        )));
    }
    Ok(())
}

fn canonicalize_for_compare(path: &Path) -> Result<PathBuf, KeyEscrowError> {
    let mut existing = path.to_path_buf();
    let mut suffix = Vec::new();
    loop {
        match fs::symlink_metadata(&existing) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(name) = existing.file_name() else {
                    return Err(KeyEscrowError::UnsafeLayout(format!(
                        "{} has no resolvable parent",
                        path.display()
                    )));
                };
                suffix.push(name.to_os_string());
                existing = existing.parent().map(Path::to_path_buf).ok_or_else(|| {
                    KeyEscrowError::UnsafeLayout(format!(
                        "{} has no resolvable parent",
                        path.display()
                    ))
                })?;
            }
            Err(error) => return Err(KeyEscrowError::Io(error)),
        }
    }
    let mut resolved = fs::canonicalize(&existing)?;
    for name in suffix.iter().rev() {
        resolved.push(name);
    }
    Ok(resolved)
}

fn load_or_create_recovery_key(
    path: &Path,
) -> Result<([u8; RECOVERY_KEY_LEN], bool), KeyEscrowError> {
    ensure_private_parent(path)?;
    if existing_metadata(path)?.is_some() {
        return Ok((read_recovery_key(path)?, false));
    }
    let mut key = [0u8; RECOVERY_KEY_LEN];
    SystemRandom::new()
        .fill(&mut key)
        .map_err(|_| KeyEscrowError::InvalidRecoveryKey("random source failed".to_string()))?;
    let encoded = format!("{}\n", hex_encode(&key));
    write_new_private(path, encoded.as_bytes())?;
    Ok((key, true))
}

fn read_recovery_key(path: &Path) -> Result<[u8; RECOVERY_KEY_LEN], KeyEscrowError> {
    ensure_private_parent(path)?;
    let metadata = existing_metadata(path)?.ok_or_else(|| {
        KeyEscrowError::InvalidRecoveryKey(format!("{} is missing", path.display()))
    })?;
    if !metadata.is_file() {
        return Err(KeyEscrowError::InvalidRecoveryKey(format!(
            "{} is not a regular file",
            path.display()
        )));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            return Err(KeyEscrowError::InvalidRecoveryKey(format!(
                "{} mode {mode:o} is accessible by other users",
                path.display()
            )));
        }
    }
    let text = fs::read_to_string(path)?;
    let bytes = hex_decode(text.trim())?;
    bytes.try_into().map_err(|_| {
        KeyEscrowError::InvalidRecoveryKey(format!(
            "{} must contain {RECOVERY_KEY_LEN} bytes of hex",
            path.display()
        ))
    })
}

struct Sealed {
    nonce: [u8; NONCE_LEN],
    bytes: Vec<u8>,
}

fn seal(key: &[u8; RECOVERY_KEY_LEN], plaintext: &[u8]) -> Result<Sealed, KeyEscrowError> {
    let mut nonce = [0u8; NONCE_LEN];
    SystemRandom::new()
        .fill(&mut nonce)
        .map_err(|_| KeyEscrowError::InvalidRecoveryKey("random source failed".to_string()))?;
    let aead = aead_key(key)?;
    let mut bytes = plaintext.to_vec();
    aead.seal_in_place_append_tag(
        Nonce::assume_unique_for_key(nonce),
        Aad::from(ESCROW_SCHEMA.as_bytes()),
        &mut bytes,
    )
    .map_err(|_| KeyEscrowError::DecryptFailed)?;
    Ok(Sealed { nonce, bytes })
}

fn open(
    key: &[u8; RECOVERY_KEY_LEN],
    nonce: &[u8],
    ciphertext: &mut [u8],
) -> Result<Vec<u8>, KeyEscrowError> {
    let nonce: [u8; NONCE_LEN] = nonce.try_into().map_err(|_| {
        KeyEscrowError::InvalidMaterial("escrow nonce has the wrong length".to_string())
    })?;
    let aead = aead_key(key)?;
    let plaintext = aead
        .open_in_place(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(ESCROW_SCHEMA.as_bytes()),
            ciphertext,
        )
        .map_err(|_| KeyEscrowError::DecryptFailed)?;
    Ok(plaintext.to_vec())
}

fn aead_key(key: &[u8; RECOVERY_KEY_LEN]) -> Result<LessSafeKey, KeyEscrowError> {
    let key = UnboundKey::new(&AES_256_GCM, key)
        .map_err(|_| KeyEscrowError::InvalidRecoveryKey("wrong key length".to_string()))?;
    Ok(LessSafeKey::new(key))
}

fn ensure_private_parent(path: &Path) -> Result<(), KeyEscrowError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let metadata = fs::metadata(parent)?;
        let mode = metadata.permissions().mode() & 0o777;
        if mode & 0o022 != 0 {
            return Err(KeyEscrowError::InvalidRecoveryKey(format!(
                "{} mode {mode:o} is group/world writable",
                parent.display()
            )));
        }
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn existing_metadata(path: &Path) -> Result<Option<fs::Metadata>, KeyEscrowError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(KeyEscrowError::UnsafeLayout(format!(
                    "{} is a symlink",
                    path.display()
                )));
            }
            Ok(Some(metadata))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(KeyEscrowError::Io(error)),
    }
}

fn write_new_private(path: &Path, bytes: &[u8]) -> Result<(), KeyEscrowError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn sync_dir(path: &Path) -> Result<(), KeyEscrowError> {
    fs::File::open(path)?.sync_all()?;
    Ok(())
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hex_decode(value: &str) -> Result<Vec<u8>, KeyEscrowError> {
    if !value.len().is_multiple_of(2) {
        return Err(KeyEscrowError::InvalidMaterial(
            "hex value has an odd length".to_string(),
        ));
    }
    let mut out = Vec::with_capacity(value.len() / 2);
    for chunk in value.as_bytes().chunks(2) {
        let part = std::str::from_utf8(chunk)
            .map_err(|_| KeyEscrowError::InvalidMaterial("hex value is not utf-8".to_string()))?;
        let byte = u8::from_str_radix(part, 16)
            .map_err(|_| KeyEscrowError::InvalidMaterial("hex value is invalid".to_string()))?;
        out.push(byte);
    }
    Ok(out)
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
