//! Model-blind credential broker for browser actions.
//!
//! The model only sees a `secret://` reference. The runtime resolves it at the
//! action point and binds it to one exact origin. Secret values never enter the
//! prompt, journal, metadata or screenshots.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("secret store io: {0}")]
    Io(#[from] std::io::Error),
    #[error("secret store json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("secret store must be a private regular file: {0}")]
    InsecureStore(PathBuf),
    #[error("unsupported secret store schema {0}")]
    UnsupportedSchema(u32),
    #[error("invalid secret reference: {0}")]
    InvalidReference(String),
    #[error("secret '{0}' is not configured")]
    Unknown(String),
    #[error("secret '{label}' is bound to {expected}, not {actual}")]
    OriginMismatch {
        label: String,
        expected: String,
        actual: String,
    },
    #[error("invalid origin: {0}")]
    InvalidOrigin(String),
    #[error("secret value must not be empty")]
    EmptyValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct SecretEntry {
    origin: String,
    value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct SecretFile {
    schema_version: u32,
    secrets: BTreeMap<String, SecretEntry>,
}

impl Default for SecretFile {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            secrets: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SecretSummary {
    pub label: String,
    pub origin: String,
}

pub struct SecretStore {
    path: PathBuf,
    file: SecretFile,
}

impl SecretStore {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, SecretError> {
        let path = path.into();
        let file = match std::fs::read(&path) {
            Ok(bytes) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let metadata = std::fs::symlink_metadata(&path)?;
                    if metadata.file_type().is_symlink()
                        || !metadata.is_file()
                        || metadata.permissions().mode() & 0o077 != 0
                    {
                        return Err(SecretError::InsecureStore(path));
                    }
                }
                let file: SecretFile = serde_json::from_slice(&bytes)?;
                if file.schema_version != SCHEMA_VERSION {
                    return Err(SecretError::UnsupportedSchema(file.schema_version));
                }
                file
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => SecretFile::default(),
            Err(error) => return Err(error.into()),
        };
        Ok(Self { path, file })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn set(&mut self, label: &str, origin: &str, value: &str) -> Result<(), SecretError> {
        let label = validate_label(label)?;
        let origin = normalize_origin(origin)?;
        if value.is_empty() {
            return Err(SecretError::EmptyValue);
        }
        self.file.secrets.insert(
            label.to_string(),
            SecretEntry {
                origin,
                value: value.to_string(),
            },
        );
        self.save()
    }

    pub fn remove(&mut self, label: &str) -> Result<bool, SecretError> {
        let label = validate_label(label)?;
        let removed = self.file.secrets.remove(label).is_some();
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    pub fn list(&self) -> Vec<SecretSummary> {
        self.file
            .secrets
            .iter()
            .map(|(label, entry)| SecretSummary {
                label: label.clone(),
                origin: entry.origin.clone(),
            })
            .collect()
    }

    /// Resolve a reference for an exact origin. The returned value is for the
    /// action executor only; callers must not log it.
    pub fn resolve(&self, reference: &str, origin: &str) -> Result<String, SecretError> {
        let label = parse_reference(reference)?;
        let origin = normalize_origin(origin)?;
        let entry = self
            .file
            .secrets
            .get(&label)
            .ok_or_else(|| SecretError::Unknown(label.clone()))?;
        if entry.origin != origin {
            return Err(SecretError::OriginMismatch {
                label,
                expected: entry.origin.clone(),
                actual: origin,
            });
        }
        Ok(entry.value.clone())
    }

    fn save(&self) -> Result<(), SecretError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
            }
        }
        let bytes = serde_json::to_vec_pretty(&self.file)?;
        let temporary = self
            .path
            .with_extension(format!("tmp-{}", std::process::id()));
        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW)
                .open(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        #[cfg(not(unix))]
        {
            std::fs::write(&temporary, &bytes)?;
        }
        std::fs::rename(&temporary, &self.path)?;
        Ok(())
    }
}

fn validate_label(label: &str) -> Result<&str, SecretError> {
    let label = label.trim();
    if label.is_empty()
        || label.len() > 128
        || !label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'/'))
        || label.starts_with('/')
    {
        return Err(SecretError::InvalidReference(label.to_string()));
    }
    Ok(label)
}

fn parse_reference(reference: &str) -> Result<String, SecretError> {
    let reference = reference.trim();
    let Some(rest) = reference.strip_prefix("secret://") else {
        return Err(SecretError::InvalidReference(reference.to_string()));
    };
    let label = validate_label(rest.trim_matches('/'))?;
    if label
        .split('/')
        .any(|segment| segment == "." || segment == "..")
    {
        return Err(SecretError::InvalidReference(reference.to_string()));
    }
    Ok(label.to_string())
}

pub fn normalize_origin(value: &str) -> Result<String, SecretError> {
    let url = reqwest::Url::parse(value.trim())
        .map_err(|_| SecretError::InvalidOrigin(value.to_string()))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(SecretError::InvalidOrigin(value.to_string()));
    }
    Ok(url.origin().ascii_serialization())
}

pub fn origin_of(url: &str) -> Option<String> {
    let url = reqwest::Url::parse(url).ok()?;
    Some(url.origin().ascii_serialization())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn secrets_are_origin_bound_private_and_listed_without_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secrets.json");
        let mut store = SecretStore::open(&path).unwrap();
        store
            .set("vendor/password", "https://vendor.example", "s3cret")
            .unwrap();
        let listed = store.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].label, "vendor/password");
        assert_eq!(listed[0].origin, "https://vendor.example");
        assert_eq!(
            store
                .resolve("secret://vendor/password", "https://vendor.example/login")
                .unwrap(),
            "s3cret"
        );
        assert!(matches!(
            store.resolve("secret://vendor/password", "https://evil.example"),
            Err(SecretError::OriginMismatch { .. })
        ));
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        store.remove("vendor/password").unwrap();
        assert!(store.list().is_empty());
    }

    #[test]
    fn malformed_references_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let store = SecretStore::open(dir.path().join("secrets.json")).unwrap();
        for reference in [
            "vendor/password",
            "secret://",
            "secret:///",
            "secret://../x",
        ] {
            assert!(store.resolve(reference, "https://example.com").is_err());
        }
    }
}
