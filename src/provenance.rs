use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReproducibleBuildReport {
    pub target_triple: String,
    pub original_sha256: String,
    pub rebuilt_sha256: String,
    pub original_size_bytes: u64,
    pub rebuilt_size_bytes: u64,
    pub hashes_equal: bool,
    pub sizes_equal: bool,
    pub duration_ms: u64,
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

pub fn rebuild_and_compare(
    workspace: impl AsRef<Path>,
    artifact: impl AsRef<Path>,
    target_triple: &str,
) -> Result<ReproducibleBuildReport, ProvenanceError> {
    let workspace = workspace.as_ref();
    let artifact = artifact.as_ref();
    let start = Instant::now();
    let build_dir = tempfile::Builder::new().prefix("1agent-repro-").tempdir()?;
    let status = Command::new("cargo")
        .args([
            "build",
            "--offline",
            "--release",
            "--target",
            target_triple,
            "--target-dir",
        ])
        .arg(build_dir.path())
        .current_dir(workspace)
        .status()
        .map_err(|error| ProvenanceError::Command("cargo build".into(), error.to_string()))?;
    if !status.success() {
        return Err(ProvenanceError::Command(
            "cargo build".into(),
            format!("exit status {status}"),
        ));
    }
    let file_name = artifact
        .file_name()
        .ok_or(ProvenanceError::InvalidArtifactPath)?;
    let rebuilt = build_dir
        .path()
        .join(target_triple)
        .join("release")
        .join(file_name);
    let report = compare_artifacts(workspace, artifact, &rebuilt, target_triple)?;
    let duration_ms = start.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    Ok(ReproducibleBuildReport {
        duration_ms,
        ..report
    })
}

fn compare_artifacts(
    _workspace: &Path,
    original: &Path,
    rebuilt: &Path,
    target_triple: &str,
) -> Result<ReproducibleBuildReport, ProvenanceError> {
    let original_bytes = std::fs::read(original)?;
    let rebuilt_bytes = std::fs::read(rebuilt)?;
    let original_sha256 = hex::encode(Sha256::digest(&original_bytes));
    let rebuilt_sha256 = hex::encode(Sha256::digest(&rebuilt_bytes));
    Ok(ReproducibleBuildReport {
        target_triple: target_triple.into(),
        original_sha256,
        rebuilt_sha256,
        original_size_bytes: original_bytes.len() as u64,
        rebuilt_size_bytes: rebuilt_bytes.len() as u64,
        hashes_equal: original_bytes == rebuilt_bytes,
        sizes_equal: original_bytes.len() == rebuilt_bytes.len(),
        duration_ms: 0,
    })
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
    if let Some(ms) =
        timestamp_from_source_date_epoch(std::env::var("SOURCE_DATE_EPOCH").ok().as_deref())
    {
        return ms;
    }
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn timestamp_from_source_date_epoch(value: Option<&str>) -> Option<u128> {
    value
        .and_then(|raw| raw.parse::<u64>().ok())
        .map(|seconds| u128::from(seconds) * 1_000)
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
    #[error("artifact path has no file name")]
    InvalidArtifactPath,
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

    #[test]
    fn artifact_comparison_detects_equal_and_different_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let left = dir.path().join("left.bin");
        let right = dir.path().join("right.bin");
        std::fs::write(&left, b"same").unwrap();
        std::fs::write(&right, b"same").unwrap();
        let report = compare_artifacts(dir.path(), &left, &right, "target").unwrap();
        assert!(report.hashes_equal);
        std::fs::write(&right, b"different").unwrap();
        let report = compare_artifacts(dir.path(), &left, &right, "target").unwrap();
        assert!(!report.hashes_equal);
    }

    #[test]
    fn source_date_epoch_accepts_only_valid_seconds() {
        assert_eq!(timestamp_from_source_date_epoch(None), None);
        assert_eq!(timestamp_from_source_date_epoch(Some("")), None);
        assert_eq!(timestamp_from_source_date_epoch(Some("not-a-time")), None);
        assert_eq!(timestamp_from_source_date_epoch(Some("0")), Some(0));
        assert_eq!(
            timestamp_from_source_date_epoch(Some("1735689600")),
            Some(1_735_689_600_000)
        );
    }
}
