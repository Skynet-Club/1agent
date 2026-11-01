use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BuildProvenance {
    pub schema_version: u32,
    pub package_name: String,
    pub package_version: String,
    pub git_commit: String,
    pub git_dirty: bool,
    pub rustc_version: String,
    pub cargo_version: String,
    pub target_triple: String,
    pub artifact_path: PathBuf,
    pub artifact_sha256: String,
    pub artifact_size_bytes: u64,
    pub generated_unix_ms: u128,
}

pub fn collect_provenance(
    workspace: impl AsRef<Path>,
    artifact: impl AsRef<Path>,
    target_triple: String,
) -> Result<BuildProvenance, ProvenanceError> {
    let workspace = workspace.as_ref();
    let artifact = artifact.as_ref();
    let bytes = std::fs::read(artifact)?;
    let metadata = std::fs::metadata(artifact)?;
    Ok(BuildProvenance {
        schema_version: 1,
        package_name: env!("CARGO_PKG_NAME").into(),
        package_version: env!("CARGO_PKG_VERSION").into(),
        git_commit: command_output(workspace, "git", &["rev-parse", "HEAD"])?,
        git_dirty: !command_output(workspace, "git", &["status", "--porcelain"])?.is_empty(),
        rustc_version: command_output(workspace, "rustc", &["--version"])?,
        cargo_version: command_output(workspace, "cargo", &["--version"])?,
        target_triple,
        artifact_path: artifact.to_path_buf(),
        artifact_sha256: hex::encode(Sha256::digest(&bytes)),
        artifact_size_bytes: metadata.len(),
        generated_unix_ms: now_unix_ms(),
    })
}

pub fn verify_provenance(
    provenance: &BuildProvenance,
    artifact: impl AsRef<Path>,
) -> Result<(), ProvenanceError> {
    if provenance.schema_version != 1 {
        return Err(ProvenanceError::UnsupportedVersion);
    }
    let bytes = std::fs::read(artifact)?;
    let hash = hex::encode(Sha256::digest(&bytes));
    if hash != provenance.artifact_sha256 || bytes.len() as u64 != provenance.artifact_size_bytes {
        return Err(ProvenanceError::ArtifactMismatch);
    }
    Ok(())
}

fn command_output(
    workspace: &Path,
    program: &str,
    args: &[&str],
) -> Result<String, ProvenanceError> {
    let output = Command::new(program)
        .args(args)
        .current_dir(workspace)
        .output()
        .map_err(|error| ProvenanceError::Command(program.into(), error.to_string()))?;
    if !output.status.success() {
        return Err(ProvenanceError::Command(
            program.into(),
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[derive(Debug, thiserror::Error)]
pub enum ProvenanceError {
    #[error("provenance I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("provenance JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported provenance version")]
    UnsupportedVersion,
    #[error("artifact does not match provenance hash or size")]
    ArtifactMismatch,
    #[error("command {0} failed: {1}")]
    Command(String, String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provenance_detects_artifact_changes() {
        let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let dir = tempfile::tempdir().unwrap();
        let artifact = dir.path().join("artifact.bin");
        std::fs::write(&artifact, b"release").unwrap();
        let provenance =
            collect_provenance(&workspace, &artifact, "x86_64-unknown-linux-gnu".into()).unwrap();
        verify_provenance(&provenance, &artifact).unwrap();
        std::fs::write(&artifact, b"changed").unwrap();
        assert!(matches!(
            verify_provenance(&provenance, &artifact),
            Err(ProvenanceError::ArtifactMismatch)
        ));
    }
}
