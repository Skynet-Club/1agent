use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::approval::{
    ApprovalEnvelope, ApprovalError, ApprovalKeyRegistry, ArtifactKind, RegistrySnapshot,
    TrustAnchorManifest,
};
use crate::provenance::{BuildProvenance, ProvenanceError, verify_provenance};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseArtifactKind {
    LinuxBinary,
    WindowsBinary,
    LinuxProvenance,
    WindowsProvenance,
    Checksums,
    LinuxArchive,
    WindowsArchive,
}

impl ReleaseArtifactKind {
    pub const REQUIRED: [Self; 7] = [
        Self::LinuxBinary,
        Self::WindowsBinary,
        Self::LinuxProvenance,
        Self::WindowsProvenance,
        Self::Checksums,
        Self::LinuxArchive,
        Self::WindowsArchive,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::LinuxBinary => "Linux binary",
            Self::WindowsBinary => "Windows binary",
            Self::LinuxProvenance => "Linux provenance",
            Self::WindowsProvenance => "Windows provenance",
            Self::Checksums => "package checksums",
            Self::LinuxArchive => "Linux archive",
            Self::WindowsArchive => "Windows archive",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReleaseArtifact {
    pub kind: ReleaseArtifactKind,
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReleaseManifest {
    pub schema_version: u32,
    pub package_name: String,
    pub package_version: String,
    pub git_commit: String,
    pub git_dirty: bool,
    pub rustc_version: String,
    pub cargo_version: String,
    pub generated_unix_ms: u128,
    pub artifacts: Vec<ReleaseArtifact>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReleasePolicyMode {
    Development,
    Production,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReleasePolicy {
    pub schema_version: u32,
    pub mode: ReleasePolicyMode,
    pub minimum_registry_version: u64,
    pub allowed_manifest_key_ids: Vec<String>,
    pub allowed_registry_key_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReleaseVerificationReport {
    pub schema_version: u32,
    pub package_version: String,
    pub git_commit: String,
    pub manifest_sha256: String,
    pub policy_mode: ReleasePolicyMode,
    pub signed: bool,
    pub registry_chain_verified: bool,
    pub artifacts_verified: usize,
    pub status: String,
}

impl ReleasePolicy {
    pub fn validate(&self) -> Result<(), ReleaseError> {
        if self.schema_version != 1 {
            return Err(ReleaseError::UnsupportedPolicyVersion);
        }
        if self.mode == ReleasePolicyMode::Production
            && (self.minimum_registry_version == 0
                || self.allowed_manifest_key_ids.is_empty()
                || self.allowed_registry_key_ids.is_empty())
        {
            return Err(ReleaseError::InvalidPolicy);
        }
        Ok(())
    }

    pub fn authorize_manifest_approval(
        &self,
        envelope: &ApprovalEnvelope,
    ) -> Result<(), ReleaseError> {
        if self.mode == ReleasePolicyMode::Production
            && !self.allowed_manifest_key_ids.contains(&envelope.key_id)
        {
            return Err(ReleaseError::PolicyKeyNotAllowed(envelope.key_id.clone()));
        }
        Ok(())
    }

    pub fn authorize_registry_signers(
        &self,
        snapshots: &[RegistrySnapshot],
        anchor: &TrustAnchorManifest,
    ) -> Result<(), ReleaseError> {
        if self.mode != ReleasePolicyMode::Production {
            return Ok(());
        }
        for key_id in snapshots
            .iter()
            .map(|snapshot| &snapshot.key_id)
            .chain(anchor.admin_key_ids.iter())
        {
            if !self.allowed_registry_key_ids.contains(key_id) {
                return Err(ReleaseError::PolicyKeyNotAllowed(key_id.clone()));
            }
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
pub fn collect_release_manifest(
    package_root: impl AsRef<Path>,
    linux_archive: impl AsRef<Path>,
    windows_archive: impl AsRef<Path>,
    manifest_dir: impl AsRef<Path>,
    allow_dirty: bool,
) -> Result<ReleaseManifest, ReleaseError> {
    let package_root = package_root.as_ref();
    let linux_archive = linux_archive.as_ref();
    let windows_archive = windows_archive.as_ref();
    let manifest_dir = manifest_dir.as_ref();

    let linux_binary = package_root.join("linux-x86_64/1agent");
    let windows_binary = package_root.join("windows-x86_64/1agent.exe");
    let linux_provenance = package_root.join("provenance/linux-x86_64.json");
    let windows_provenance = package_root.join("provenance/windows-x86_64.json");
    let checksums = package_root.join("SHA256SUMS");

    let linux_prov: BuildProvenance = read_json(&linux_provenance)?;
    let windows_prov: BuildProvenance = read_json(&windows_provenance)?;
    verify_provenance(&linux_prov, &linux_binary)?;
    verify_provenance(&windows_prov, &windows_binary)?;
    validate_provenance_pair(&linux_prov, &windows_prov, allow_dirty)?;
    verify_checksums_file(&checksums, package_root)?;

    let paths = [
        linux_binary,
        windows_binary,
        linux_provenance,
        windows_provenance,
        checksums,
        linux_archive.to_path_buf(),
        windows_archive.to_path_buf(),
    ];
    let artifacts = ReleaseArtifactKind::REQUIRED
        .into_iter()
        .zip(paths)
        .map(|(kind, path)| release_artifact(kind, &path, manifest_dir))
        .collect::<Result<Vec<_>, _>>()?;
    if linux_prov.package_name != env!("CARGO_PKG_NAME")
        || linux_prov.package_version != env!("CARGO_PKG_VERSION")
    {
        return Err(ReleaseError::PackageMismatch);
    }

    let manifest = ReleaseManifest {
        schema_version: 1,
        package_name: linux_prov.package_name.clone(),
        package_version: linux_prov.package_version.clone(),
        git_commit: linux_prov.git_commit.clone(),
        git_dirty: linux_prov.git_dirty,
        rustc_version: linux_prov.rustc_version.clone(),
        cargo_version: linux_prov.cargo_version.clone(),
        generated_unix_ms: linux_prov.generated_unix_ms,
        artifacts,
    };
    validate_expected_paths(&manifest)?;
    Ok(manifest)
}

pub fn verify_release_manifest(
    manifest: &ReleaseManifest,
    manifest_dir: impl AsRef<Path>,
) -> Result<(), ReleaseError> {
    if manifest.schema_version != 1 {
        return Err(ReleaseError::UnsupportedVersion);
    }
    if [
        manifest.package_name.as_str(),
        manifest.package_version.as_str(),
        manifest.git_commit.as_str(),
        manifest.rustc_version.as_str(),
        manifest.cargo_version.as_str(),
    ]
    .iter()
    .any(|value| value.trim().is_empty())
    {
        return Err(ReleaseError::EmptyField);
    }
    if manifest.git_dirty {
        return Err(ReleaseError::DirtyRelease);
    }

    let manifest_dir = manifest_dir.as_ref();
    let mut seen_kinds = HashSet::new();
    let mut seen_paths = HashSet::new();
    let mut verified = Vec::new();
    for artifact in &manifest.artifacts {
        if !seen_kinds.insert(artifact.kind) {
            return Err(ReleaseError::DuplicateArtifact(artifact.kind));
        }
        let path = safe_join(manifest_dir, &artifact.path)?;
        if !seen_paths.insert(path.clone()) {
            return Err(ReleaseError::DuplicatePath(artifact.path.clone()));
        }
        verify_artifact_bytes(&path, &artifact.sha256, artifact.size_bytes)?;
        verified.push((artifact.kind, path));
    }
    for required in ReleaseArtifactKind::REQUIRED {
        if !seen_kinds.contains(&required) {
            return Err(ReleaseError::MissingArtifact(required));
        }
    }
    validate_expected_paths(manifest)?;

    let path_for = |kind| {
        verified
            .iter()
            .find(|(candidate, _)| *candidate == kind)
            .map(|(_, path)| path.as_path())
            .ok_or(ReleaseError::MissingArtifact(kind))
    };
    let linux_prov: BuildProvenance = read_json(path_for(ReleaseArtifactKind::LinuxProvenance)?)?;
    let windows_prov: BuildProvenance =
        read_json(path_for(ReleaseArtifactKind::WindowsProvenance)?)?;
    validate_provenance_pair(&linux_prov, &windows_prov, false)?;
    verify_provenance(&linux_prov, path_for(ReleaseArtifactKind::LinuxBinary)?)?;
    verify_provenance(&windows_prov, path_for(ReleaseArtifactKind::WindowsBinary)?)?;
    if manifest.package_name != linux_prov.package_name
        || manifest.package_version != linux_prov.package_version
        || manifest.git_commit != linux_prov.git_commit
        || manifest.rustc_version != linux_prov.rustc_version
        || manifest.cargo_version != linux_prov.cargo_version
        || manifest.generated_unix_ms != linux_prov.generated_unix_ms
    {
        return Err(ReleaseError::ProvenanceMismatch);
    }

    let checksums = path_for(ReleaseArtifactKind::Checksums)?;
    let package_root = checksums.parent().ok_or(ReleaseError::PathHasNoParent)?;
    verify_checksums_file(checksums, package_root)?;
    Ok(())
}

pub fn verify_release_approval(
    manifest_path: impl AsRef<Path>,
    envelope: &ApprovalEnvelope,
    key: &[u8],
    registry: &ApprovalKeyRegistry,
) -> Result<(), ApprovalError> {
    envelope.require_kind(ArtifactKind::ReleaseManifest)?;
    registry.verify_approval(envelope, manifest_path, key)
}

pub fn build_release_verification_report(
    manifest: &ReleaseManifest,
    manifest_path: impl AsRef<Path>,
    policy_mode: ReleasePolicyMode,
    signed: bool,
    registry_chain_verified: bool,
) -> Result<ReleaseVerificationReport, ReleaseError> {
    let bytes = std::fs::read(manifest_path)?;
    Ok(ReleaseVerificationReport {
        schema_version: 1,
        package_version: manifest.package_version.clone(),
        git_commit: manifest.git_commit.clone(),
        manifest_sha256: hex::encode(Sha256::digest(bytes)),
        policy_mode,
        signed,
        registry_chain_verified,
        artifacts_verified: manifest.artifacts.len(),
        status: "verified".into(),
    })
}

fn validate_provenance_pair(
    linux: &BuildProvenance,
    windows: &BuildProvenance,
    allow_dirty: bool,
) -> Result<(), ReleaseError> {
    if (linux.git_dirty || windows.git_dirty) && !allow_dirty {
        return Err(ReleaseError::DirtyRelease);
    }
    if linux.package_name != windows.package_name
        || linux.package_version != windows.package_version
        || linux.git_commit != windows.git_commit
        || linux.git_dirty != windows.git_dirty
        || linux.rustc_version != windows.rustc_version
        || linux.cargo_version != windows.cargo_version
        || linux.generated_unix_ms != windows.generated_unix_ms
    {
        return Err(ReleaseError::ProvenanceMismatch);
    }
    if linux.target_triple != "x86_64-unknown-linux-gnu"
        || windows.target_triple != "x86_64-pc-windows-gnu"
    {
        return Err(ReleaseError::UnexpectedTarget);
    }
    Ok(())
}

fn release_artifact(
    kind: ReleaseArtifactKind,
    artifact: &Path,
    manifest_dir: &Path,
) -> Result<ReleaseArtifact, ReleaseError> {
    let root = std::fs::canonicalize(manifest_dir)?;
    let artifact = std::fs::canonicalize(artifact)?;
    let relative = artifact
        .strip_prefix(&root)
        .map_err(|_| ReleaseError::ArtifactOutsideManifestRoot)?;
    let bytes = std::fs::read(&artifact)?;
    Ok(ReleaseArtifact {
        kind,
        path: relative_path(relative)?,
        sha256: hex::encode(Sha256::digest(&bytes)),
        size_bytes: bytes.len() as u64,
    })
}

fn relative_path(path: &Path) -> Result<String, ReleaseError> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => parts.push(value.to_string_lossy()),
            _ => return Err(ReleaseError::UnsafePath(path.to_path_buf())),
        }
    }
    if parts.is_empty() {
        return Err(ReleaseError::UnsafePath(path.to_path_buf()));
    }
    Ok(parts.join("/"))
}

fn safe_join(root: &Path, relative: &str) -> Result<PathBuf, ReleaseError> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ReleaseError::UnsafePath(path.to_path_buf()));
    }
    let root = std::fs::canonicalize(root)?;
    let joined = std::fs::canonicalize(root.join(path))?;
    if !joined.starts_with(&root) {
        return Err(ReleaseError::ArtifactOutsideManifestRoot);
    }
    Ok(joined)
}

fn verify_artifact_bytes(path: &Path, expected_hash: &str, size: u64) -> Result<(), ReleaseError> {
    let bytes = std::fs::read(path)?;
    let actual_hash = hex::encode(Sha256::digest(&bytes));
    if bytes.len() as u64 != size {
        return Err(ReleaseError::SizeMismatch(path.to_path_buf()));
    }
    if actual_hash != expected_hash {
        return Err(ReleaseError::HashMismatch(path.to_path_buf()));
    }
    Ok(())
}

fn verify_checksums_file(path: &Path, package_root: &Path) -> Result<(), ReleaseError> {
    let content = std::fs::read_to_string(path)?;
    let package_root = std::fs::canonicalize(package_root)?;
    let mut seen = HashSet::new();
    for line in content.lines() {
        let (expected_hash, relative) = line
            .split_once("  ")
            .ok_or_else(|| ReleaseError::ChecksumsInvalid(line.into()))?;
        if expected_hash.len() != 64 || !expected_hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(ReleaseError::ChecksumsInvalid(line.into()));
        }
        let normalized = relative.strip_prefix("./").unwrap_or(relative);
        let checked_path = safe_join(&package_root, normalized)?;
        if !seen.insert(checked_path.clone()) {
            return Err(ReleaseError::DuplicatePath(normalized.into()));
        }
        let bytes = std::fs::read(&checked_path)?;
        let actual_hash = hex::encode(Sha256::digest(&bytes));
        if actual_hash != expected_hash {
            return Err(ReleaseError::HashMismatch(checked_path));
        }
    }
    if seen.is_empty() {
        return Err(ReleaseError::ChecksumsInvalid("empty checksum file".into()));
    }
    let mut packaged = HashSet::new();
    collect_package_files(&package_root, &mut packaged)?;
    packaged.remove(&std::fs::canonicalize(path)?);
    if seen != packaged {
        return Err(ReleaseError::ChecksumsIncomplete);
    }
    Ok(())
}

fn collect_package_files(path: &Path, files: &mut HashSet<PathBuf>) -> Result<(), ReleaseError> {
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(ReleaseError::UnsafePath(entry.path()));
        }
        if file_type.is_dir() {
            collect_package_files(&entry.path(), files)?;
        } else if file_type.is_file() {
            files.insert(std::fs::canonicalize(entry.path())?);
        } else {
            return Err(ReleaseError::UnsafePath(entry.path()));
        }
    }
    Ok(())
}

fn validate_expected_paths(manifest: &ReleaseManifest) -> Result<(), ReleaseError> {
    for artifact in &manifest.artifacts {
        let expected_suffix = match artifact.kind {
            ReleaseArtifactKind::LinuxBinary => "linux-x86_64/1agent",
            ReleaseArtifactKind::WindowsBinary => "windows-x86_64/1agent.exe",
            ReleaseArtifactKind::LinuxProvenance => "provenance/linux-x86_64.json",
            ReleaseArtifactKind::WindowsProvenance => "provenance/windows-x86_64.json",
            ReleaseArtifactKind::Checksums => "SHA256SUMS",
            ReleaseArtifactKind::LinuxArchive => "-linux-x86_64.tar.gz",
            ReleaseArtifactKind::WindowsArchive => "-windows-x86_64.zip",
        };
        if !artifact.path.ends_with(expected_suffix) {
            return Err(ReleaseError::UnexpectedPath(
                artifact.kind,
                artifact.path.clone(),
            ));
        }
    }
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, ReleaseError> {
    let content = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&content)?)
}

#[derive(Debug, thiserror::Error)]
pub enum ReleaseError {
    #[error("release I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("release JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("provenance verification failed: {0}")]
    Provenance(#[from] ProvenanceError),
    #[error("unsupported release manifest version")]
    UnsupportedVersion,
    #[error("unsupported release policy version")]
    UnsupportedPolicyVersion,
    #[error("release policy is incomplete")]
    InvalidPolicy,
    #[error("release policy does not allow key: {0}")]
    PolicyKeyNotAllowed(String),
    #[error("release manifest contains an empty required field")]
    EmptyField,
    #[error("dirty source cannot be promoted as a release")]
    DirtyRelease,
    #[error("release provenance files disagree")]
    ProvenanceMismatch,
    #[error("release provenance contains an unexpected target")]
    UnexpectedTarget,
    #[error("release provenance does not match this package version")]
    PackageMismatch,
    #[error("missing release artifact: {0}")]
    MissingArtifact(ReleaseArtifactKind),
    #[error("duplicate release artifact kind: {0}")]
    DuplicateArtifact(ReleaseArtifactKind),
    #[error("duplicate release artifact path: {0}")]
    DuplicatePath(String),
    #[error("artifact is outside the manifest root")]
    ArtifactOutsideManifestRoot,
    #[error("unsafe release path: {0}")]
    UnsafePath(PathBuf),
    #[error("release path has no parent")]
    PathHasNoParent,
    #[error("artifact size changed: {0}")]
    SizeMismatch(PathBuf),
    #[error("artifact hash changed: {0}")]
    HashMismatch(PathBuf),
    #[error("invalid checksum line: {0}")]
    ChecksumsInvalid(String),
    #[error("SHA256SUMS does not cover the complete package")]
    ChecksumsIncomplete,
    #[error("unexpected path for {0}: {1}")]
    UnexpectedPath(ReleaseArtifactKind, String),
}

impl std::fmt::Display for ReleaseArtifactKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approval::{ApprovalKeyStatus, ArtifactKind};

    fn fixture() -> (tempfile::TempDir, ReleaseManifest) {
        let root = tempfile::tempdir().unwrap();
        let package_name = format!("1agent-{}", env!("CARGO_PKG_VERSION"));
        let package_root = root.path().join(&package_name);
        std::fs::create_dir_all(package_root.join("linux-x86_64")).unwrap();
        std::fs::create_dir_all(package_root.join("windows-x86_64")).unwrap();
        std::fs::create_dir_all(package_root.join("provenance")).unwrap();
        let linux_binary = package_root.join("linux-x86_64/1agent");
        let windows_binary = package_root.join("windows-x86_64/1agent.exe");
        std::fs::write(&linux_binary, b"linux-binary").unwrap();
        std::fs::write(&windows_binary, b"windows-binary").unwrap();

        let linux_prov = provenance(&linux_binary, "x86_64-unknown-linux-gnu", b"linux-binary");
        let windows_prov = provenance(&windows_binary, "x86_64-pc-windows-gnu", b"windows-binary");
        let linux_prov_path = package_root.join("provenance/linux-x86_64.json");
        let windows_prov_path = package_root.join("provenance/windows-x86_64.json");
        std::fs::write(
            &linux_prov_path,
            serde_json::to_vec_pretty(&linux_prov).unwrap(),
        )
        .unwrap();
        std::fs::write(
            &windows_prov_path,
            serde_json::to_vec_pretty(&windows_prov).unwrap(),
        )
        .unwrap();

        let mut checksum_lines = [
            linux_binary,
            windows_binary,
            linux_prov_path,
            windows_prov_path,
        ]
        .into_iter()
        .map(|path| {
            let bytes = std::fs::read(&path).unwrap();
            let relative = path.strip_prefix(&package_root).unwrap();
            format!(
                "{}  ./{}",
                hex::encode(Sha256::digest(bytes)),
                relative.to_string_lossy()
            )
        })
        .collect::<Vec<_>>();
        checksum_lines.sort();
        std::fs::write(
            package_root.join("SHA256SUMS"),
            format!("{}\n", checksum_lines.join("\n")),
        )
        .unwrap();

        let linux_archive = root
            .path()
            .join(format!("{package_name}-linux-x86_64.tar.gz"));
        let windows_archive = root
            .path()
            .join(format!("{package_name}-windows-x86_64.zip"));
        std::fs::write(&linux_archive, b"linux-archive").unwrap();
        std::fs::write(&windows_archive, b"windows-archive").unwrap();
        let manifest = collect_release_manifest(
            &package_root,
            &linux_archive,
            &windows_archive,
            root.path(),
            false,
        )
        .unwrap();
        (root, manifest)
    }

    fn provenance(path: &Path, target: &str, bytes: &[u8]) -> BuildProvenance {
        BuildProvenance {
            schema_version: 1,
            package_name: env!("CARGO_PKG_NAME").into(),
            package_version: env!("CARGO_PKG_VERSION").into(),
            git_commit: "0123456789abcdef".into(),
            git_dirty: false,
            rustc_version: "rustc test".into(),
            cargo_version: "cargo test".into(),
            target_triple: target.into(),
            artifact_path: path.to_path_buf(),
            artifact_sha256: hex::encode(Sha256::digest(bytes)),
            artifact_size_bytes: bytes.len() as u64,
            generated_unix_ms: 0,
        }
    }

    #[test]
    fn release_manifest_verifies_all_required_artifacts() {
        let (root, manifest) = fixture();
        verify_release_manifest(&manifest, root.path()).unwrap();
        assert_eq!(
            manifest.artifacts.len(),
            ReleaseArtifactKind::REQUIRED.len()
        );
    }

    #[test]
    fn release_manifest_rejects_changed_archive() {
        let (root, manifest) = fixture();
        std::fs::write(
            root.path().join(format!(
                "1agent-{}-windows-x86_64.zip",
                env!("CARGO_PKG_VERSION")
            )),
            b"changed",
        )
        .unwrap();
        assert!(matches!(
            verify_release_manifest(&manifest, root.path()),
            Err(ReleaseError::HashMismatch(_)) | Err(ReleaseError::SizeMismatch(_))
        ));
    }

    #[test]
    fn release_manifest_rejects_path_escape() {
        let (root, mut manifest) = fixture();
        manifest.artifacts[0].path = "../linux".into();
        assert!(matches!(
            verify_release_manifest(&manifest, root.path()),
            Err(ReleaseError::UnsafePath(_))
        ));
    }

    #[test]
    fn release_manifest_rejects_incomplete_checksum_coverage() {
        let (root, manifest) = fixture();
        let package_root = root
            .path()
            .join(format!("1agent-{}", env!("CARGO_PKG_VERSION")));
        std::fs::write(package_root.join("unlisted.txt"), b"extra").unwrap();
        assert!(matches!(
            verify_release_manifest(&manifest, root.path()),
            Err(ReleaseError::ChecksumsIncomplete)
        ));
    }

    #[test]
    fn release_approval_requires_the_release_kind_and_respects_key_state() {
        let (root, manifest) = fixture();
        let manifest_path = root.path().join("release.json");
        std::fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        let key = b"test-only-release-approval-key".to_vec();
        let mut registry = ApprovalKeyRegistry::default();
        registry.add_active("release-key".into()).unwrap();
        let envelope = ApprovalEnvelope::sign(
            &manifest_path,
            ArtifactKind::ReleaseManifest,
            "release-owner".into(),
            "approve release".into(),
            "release-key".into(),
            &key,
        )
        .unwrap();
        verify_release_approval(&manifest_path, &envelope, &key, &registry).unwrap();

        registry
            .transition("release-key", ApprovalKeyStatus::Retired, "rotated".into())
            .unwrap();
        verify_release_approval(&manifest_path, &envelope, &key, &registry).unwrap();
        assert!(matches!(
            registry.require_active("release-key"),
            Err(ApprovalError::KeyNotActive)
        ));

        registry
            .transition(
                "release-key",
                ApprovalKeyStatus::Revoked,
                "compromised".into(),
            )
            .unwrap();
        assert!(matches!(
            verify_release_approval(&manifest_path, &envelope, &key, &registry),
            Err(ApprovalError::KeyRevoked)
        ));

        let wrong_kind = ApprovalEnvelope::sign(
            &manifest_path,
            ArtifactKind::Provenance,
            "release-owner".into(),
            "approve release".into(),
            "release-key".into(),
            &key,
        )
        .unwrap();
        assert!(matches!(
            verify_release_approval(&manifest_path, &wrong_kind, &key, &registry),
            Err(ApprovalError::UnexpectedArtifactKind)
        ));
    }

    #[test]
    fn release_policy_requires_configured_production_signers() {
        let mut policy = ReleasePolicy {
            schema_version: 1,
            mode: ReleasePolicyMode::Production,
            minimum_registry_version: 7,
            allowed_manifest_key_ids: vec!["release-key".into()],
            allowed_registry_key_ids: vec!["admin-key".into()],
        };
        policy.validate().unwrap();
        let envelope = ApprovalEnvelope {
            envelope_version: 1,
            artifact_kind: ArtifactKind::ReleaseManifest,
            artifact_sha256: "0".repeat(64),
            approver: "owner".into(),
            approved_unix_ms: 0,
            purpose: "release".into(),
            key_id: "release-key".into(),
            mac_sha256: "0".repeat(64),
        };
        policy.authorize_manifest_approval(&envelope).unwrap();
        let mut unauthorized = envelope;
        unauthorized.key_id = "other-key".into();
        assert!(matches!(
            policy.authorize_manifest_approval(&unauthorized),
            Err(ReleaseError::PolicyKeyNotAllowed(_))
        ));

        let snapshot = RegistrySnapshot {
            snapshot_version: 1,
            registry_version: 7,
            registry_sha256: "0".repeat(64),
            approved_unix_ms: 0,
            approver: "admin".into(),
            key_id: "admin-key".into(),
            mac_sha256: "0".repeat(64),
            previous_snapshot_sha256: None,
            snapshot_sha256: "0".repeat(64),
        };
        let anchor = TrustAnchorManifest {
            anchor_version: 1,
            first_snapshot_sha256: snapshot.snapshot_sha256.clone(),
            operator: "owner".into(),
            admin_key_ids: vec!["admin-key".into()],
            created_unix_ms: 0,
        };
        policy
            .authorize_registry_signers(std::slice::from_ref(&snapshot), &anchor)
            .unwrap();

        policy.allowed_registry_key_ids.clear();
        assert!(matches!(
            policy.validate(),
            Err(ReleaseError::InvalidPolicy)
        ));
    }
}
