//! Product runtime home (`~/.1a`): layout, init, doctor, export/restore,
//! upgrade and uninstall.
//!
//! `home.json` marks a directory as a 1Agent home. Commands refuse to touch a
//! non-empty directory without that marker, so a mistyped `--root` cannot
//! overwrite or delete unrelated data. Exports exclude key material and
//! credential-looking files by default; key escrow is a separate decision.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::audit::{AuditKind, audit_store};

pub const HOME_SCHEMA: &str = "1agent.home.v1";
pub const HOME_VERSION: u32 = 2;
pub const HOME_DIRS: [&str; 4] = ["journals", "payloads", "backups", "keys"];
const MEMORY_KEY: &str = "memory.key";
const MEMORY_LEDGER: &str = "memory.key-keks.json";
const KEYS_DIR: &str = "keys";

const MANIFEST_NAME: &str = "home.json";
const EXPORT_MANIFEST_NAME: &str = "export.json";
const EXPORT_SCHEMA: &str = "1agent.home.export.v1";

#[derive(Debug, Error)]
pub enum HomeError {
    #[error("runtime home io: {0}")]
    Io(#[from] std::io::Error),
    #[error("runtime home json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("runtime home layout is unsafe: {0}")]
    UnsafeLayout(String),
    #[error("not a 1Agent runtime home: {0}")]
    MissingHome(PathBuf),
    #[error("unsupported runtime home version: found {found}, supported {supported}")]
    UnsupportedVersion { found: u32, supported: u32 },
    #[error("export output is not empty: {0}")]
    OutputNotEmpty(PathBuf),
    #[error("export bundle is invalid: {0}")]
    InvalidBundle(String),
    #[error("export bundle hash mismatch: {0}")]
    HashMismatch(String),
    #[error("purge requires --yes")]
    ConfirmationRequired,
    #[error("refusing to delete an unsafe path: {0}")]
    Refused(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HomeManifest {
    pub schema: String,
    pub version: u32,
    pub created_ms: u128,
    pub updated_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InitOutcome {
    pub root: PathBuf,
    pub created: bool,
    pub manifest: HomeManifest,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DoctorStatus {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DoctorCheck {
    pub id: String,
    pub status: DoctorStatus,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DoctorReport {
    pub root: PathBuf,
    pub status: DoctorStatus,
    pub checks: Vec<DoctorCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExportFile {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExportManifest {
    pub schema: String,
    pub created_ms: u128,
    pub home_version: u32,
    pub files: Vec<ExportFile>,
    pub excluded: Vec<String>,
    pub key_included: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExportOutcome {
    pub output: PathBuf,
    pub manifest: ExportManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RestoreOutcome {
    pub root: PathBuf,
    pub files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UpgradeOutcome {
    pub root: PathBuf,
    pub check_only: bool,
    pub from_version: u32,
    pub to_version: u32,
    pub rolled_back: bool,
    pub repaired: Vec<String>,
    pub changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UninstallOutcome {
    pub root: PathBuf,
    pub removed: bool,
    pub message: String,
}

pub fn default_home() -> PathBuf {
    if let Some(home) = std::env::var_os("ONEAGENT_HOME") {
        return PathBuf::from(home);
    }
    let base = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join(".1a")
}

pub fn is_initialized(root: &Path) -> bool {
    let path = root.join(MANIFEST_NAME);
    let Ok(metadata) = fs::symlink_metadata(&path) else {
        return false;
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return false;
    }
    let Ok(bytes) = fs::read(&path) else {
        return false;
    };
    let Ok(manifest) = serde_json::from_slice::<HomeManifest>(&bytes) else {
        return false;
    };
    manifest.schema == HOME_SCHEMA && manifest.version == HOME_VERSION
}

/// Resolve a command default against the initialized home, falling back to the
/// historical relative path when no home exists. This keeps existing worktrees
/// working while making an initialized home the single state root.
pub fn home_or_legacy(relative: &str, legacy: &str) -> PathBuf {
    let home = default_home();
    if is_initialized(&home) {
        home.join(relative)
    } else {
        PathBuf::from(legacy)
    }
}

pub fn init(root: &Path, now_ms: u128) -> Result<InitOutcome, HomeError> {
    let root = absolute(root)?;
    if let Some(metadata) = existing_metadata(&root)? {
        if !metadata.is_dir() {
            return Err(HomeError::UnsafeLayout(format!(
                "{} is not a directory",
                root.display()
            )));
        }
        if has_manifest(&root)? {
            let manifest = read_manifest(&root)?;
            ensure_supported(&manifest)?;
            ensure_layout(&root)?;
            return Ok(InitOutcome {
                root,
                created: false,
                manifest,
            });
        }
        if fs::read_dir(&root)?.next().is_some() {
            return Err(HomeError::UnsafeLayout(format!(
                "{} is not empty and has no {}",
                root.display(),
                MANIFEST_NAME
            )));
        }
    } else {
        create_private_dirs(&root)?;
    }
    set_mode(&root, 0o700)?;
    ensure_layout(&root)?;
    let manifest = HomeManifest {
        schema: HOME_SCHEMA.to_string(),
        version: HOME_VERSION,
        created_ms: now_ms,
        updated_ms: now_ms,
    };
    write_manifest(&root, &manifest)?;
    Ok(InitOutcome {
        root,
        created: true,
        manifest,
    })
}

pub fn doctor(root: &Path) -> Result<DoctorReport, HomeError> {
    let root = absolute(root)?;
    let mut checks = Vec::new();
    let metadata = existing_metadata(&root)?;
    let Some(metadata) = metadata else {
        checks.push(check(
            "root",
            DoctorStatus::Fail,
            format!("{} does not exist; run `1agent init`", root.display()),
        ));
        return Ok(finish_report(root, checks));
    };
    if !metadata.is_dir() {
        checks.push(check(
            "root",
            DoctorStatus::Fail,
            format!("{} is not a directory", root.display()),
        ));
        return Ok(finish_report(root, checks));
    }
    checks.push(check("root", DoctorStatus::Ok, "home directory exists"));

    match read_manifest(&root) {
        Ok(manifest) => {
            if manifest.version > HOME_VERSION {
                checks.push(check(
                    "home_marker",
                    DoctorStatus::Fail,
                    format!(
                        "home version {} is newer than this binary ({HOME_VERSION})",
                        manifest.version
                    ),
                ));
            } else if manifest.version < HOME_VERSION {
                let (status, detail) = if manifest.version == 1 {
                    (
                        DoctorStatus::Warn,
                        format!(
                            "home version 1 is older than {HOME_VERSION}; run `1agent upgrade`"
                        ),
                    )
                } else {
                    (
                        DoctorStatus::Fail,
                        format!(
                            "home version {} is older than {HOME_VERSION}; this build has no migration",
                            manifest.version
                        ),
                    )
                };
                checks.push(check("home_marker", status, detail));
            } else {
                checks.push(check(
                    "home_marker",
                    DoctorStatus::Ok,
                    format!(
                        "{MANIFEST_NAME} schema is valid (version {})",
                        manifest.version
                    ),
                ));
            }
        }
        Err(error) => checks.push(check("home_marker", DoctorStatus::Fail, error.to_string())),
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode() & 0o777;
        if mode & 0o022 != 0 {
            checks.push(check(
                "root_private",
                DoctorStatus::Fail,
                format!("home mode {mode:o} is group/world writable"),
            ));
        } else if mode & 0o077 != 0 {
            checks.push(check(
                "root_private",
                DoctorStatus::Warn,
                format!("home mode {mode:o} is accessible by other users"),
            ));
        } else {
            checks.push(check(
                "root_private",
                DoctorStatus::Ok,
                format!("home mode {mode:o} is private"),
            ));
        }
    }
    #[cfg(not(unix))]
    checks.push(check(
        "root_private",
        DoctorStatus::Warn,
        "permission checks are not available on this platform",
    ));

    for name in HOME_DIRS {
        let path = root.join(name);
        match fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() => checks.push(check(
                name,
                DoctorStatus::Fail,
                format!("{} is a symlink", path.display()),
            )),
            Ok(meta) if meta.is_dir() => checks.push(check(name, DoctorStatus::Ok, "present")),
            Ok(_) => checks.push(check(
                name,
                DoctorStatus::Fail,
                format!("{} is not a directory", path.display()),
            )),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => checks.push(check(
                name,
                DoctorStatus::Warn,
                format!("{} is missing; run `1agent upgrade`", path.display()),
            )),
            Err(error) => checks.push(check(name, DoctorStatus::Fail, error.to_string())),
        }
    }

    checks.push(check_private_key_file(
        &root,
        "master_key",
        "keys/memory.key",
    ));
    checks.push(check_private_key_file(
        &root,
        "kek_ledger",
        "keys/memory.key-keks.json",
    ));
    checks.push(check_journal(
        &root,
        "task_graph_journal",
        "task-graph.jsonl",
        AuditKind::TaskGraph,
    ));
    checks.push(check_journal(
        &root,
        "memory_journal",
        "memory-journal.jsonl",
        AuditKind::Memory,
    ));
    checks.push(check_journal(
        &root,
        "skill_journal",
        "skill-journal.jsonl",
        AuditKind::Skill,
    ));

    let probe = root.join(format!(
        ".doctor-probe-{}-{now}",
        std::process::id(),
        now = now_ms()
    ));
    let write_result = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .and_then(|mut file| file.write_all(b"ok"));
    let _ = fs::remove_file(&probe);
    match write_result {
        Ok(()) => checks.push(check("writable", DoctorStatus::Ok, "home is writable")),
        Err(error) => checks.push(check("writable", DoctorStatus::Fail, error.to_string())),
    }

    Ok(finish_report(root, checks))
}

fn check_journal(root: &Path, id: &str, name: &str, kind: AuditKind) -> DoctorCheck {
    let path = root.join(name);
    match fs::symlink_metadata(&path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            check(id, DoctorStatus::Fail, format!("{name} is a symlink"))
        }
        Ok(meta) if !meta.is_file() => check(
            id,
            DoctorStatus::Fail,
            format!("{name} is not a regular file"),
        ),
        Ok(_) => match audit_store(kind, &path, now_ms()) {
            Ok(receipt) => check(
                id,
                DoctorStatus::Ok,
                format!("{name}: {} events verified", receipt.events),
            ),
            Err(error) => check(id, DoctorStatus::Fail, error.to_string()),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            check(id, DoctorStatus::Ok, format!("{name} is not present yet"))
        }
        Err(error) => check(id, DoctorStatus::Fail, error.to_string()),
    }
}

fn check_private_key_file(root: &Path, id: &str, name: &str) -> DoctorCheck {
    let path = root.join(name);
    match fs::symlink_metadata(&path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            check(id, DoctorStatus::Fail, format!("{name} is a symlink"))
        }
        Ok(meta) if !meta.is_file() => check(
            id,
            DoctorStatus::Fail,
            format!("{name} is not a regular file"),
        ),
        Ok(meta) => {
            #[cfg(not(unix))]
            let _ = &meta;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = meta.permissions().mode() & 0o777;
                if mode & 0o077 != 0 {
                    return check(
                        id,
                        DoctorStatus::Fail,
                        format!("{name} mode {mode:o} is accessible by other users"),
                    );
                }
            }
            check(id, DoctorStatus::Ok, format!("{name} is private"))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            check(id, DoctorStatus::Ok, format!("{name} is not created yet"))
        }
        Err(error) => check(id, DoctorStatus::Fail, error.to_string()),
    }
}

pub fn export(root: &Path, output: &Path, now_ms: u128) -> Result<ExportOutcome, HomeError> {
    let root = absolute(root)?;
    let manifest = read_manifest(&root)?;
    ensure_supported(&manifest)?;
    let output = absolute(output)?;
    ensure_disjoint(
        &root,
        &output,
        "export output must be outside the runtime home",
    )?;
    prepare_empty_output(&output)?;

    let mut paths = Vec::new();
    collect_files(&root, &mut paths)?;
    paths.sort();

    let mut entries = Vec::new();
    for path in paths {
        let relative = path
            .strip_prefix(&root)
            .map_err(|_| HomeError::UnsafeLayout(format!("{} escapes the home", path.display())))?
            .to_path_buf();
        let display = relative_display(&relative)?;
        entries.push((path, relative, display));
    }

    let mut files = Vec::new();
    let mut excluded = Vec::new();
    for (path, relative, display) in entries {
        if is_secret_name(&display) {
            excluded.push(display);
            continue;
        }
        let bytes = fs::read(&path)?;
        let digest = hex_sha256(&bytes);
        let destination = output.join(&relative);
        if let Some(parent) = destination.parent() {
            create_private_dirs(parent)?;
        }
        write_new_private(&destination, &bytes)?;
        files.push(ExportFile {
            path: display,
            sha256: digest,
            bytes: bytes.len() as u64,
        });
    }
    excluded.sort();

    let export_manifest = ExportManifest {
        schema: EXPORT_SCHEMA.to_string(),
        created_ms: now_ms,
        home_version: manifest.version,
        files,
        excluded,
        key_included: false,
    };
    write_json_file(&output.join(EXPORT_MANIFEST_NAME), &export_manifest)?;
    sync_dir(&output)?;
    Ok(ExportOutcome {
        output,
        manifest: export_manifest,
    })
}

pub fn verify_export(output: &Path) -> Result<ExportManifest, HomeError> {
    let output = absolute(output)?;
    let manifest_path = output.join(EXPORT_MANIFEST_NAME);
    let metadata = fs::symlink_metadata(&manifest_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            HomeError::InvalidBundle(format!("{} is missing", manifest_path.display()))
        } else {
            HomeError::Io(error)
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(HomeError::UnsafeLayout(format!(
            "{} is not a regular file",
            manifest_path.display()
        )));
    }
    let manifest: ExportManifest = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    if manifest.schema != EXPORT_SCHEMA {
        return Err(HomeError::InvalidBundle(format!(
            "unexpected schema {}",
            manifest.schema
        )));
    }
    if manifest.key_included {
        return Err(HomeError::InvalidBundle(
            "key material must not be included in a home export".to_string(),
        ));
    }
    if manifest.home_version != HOME_VERSION {
        return Err(HomeError::UnsupportedVersion {
            found: manifest.home_version,
            supported: HOME_VERSION,
        });
    }
    if !manifest.files.iter().any(|file| file.path == MANIFEST_NAME) {
        return Err(HomeError::InvalidBundle(format!(
            "{MANIFEST_NAME} is missing"
        )));
    }
    let marker: HomeManifest = serde_json::from_slice(&fs::read(output.join(MANIFEST_NAME))?)
        .map_err(|error| HomeError::InvalidBundle(format!("{MANIFEST_NAME}: {error}")))?;
    if marker.schema != HOME_SCHEMA || marker.version != manifest.home_version {
        return Err(HomeError::InvalidBundle(format!(
            "{MANIFEST_NAME} does not match the export version"
        )));
    }
    let mut listed = std::collections::BTreeSet::new();
    for file in &manifest.files {
        let relative = safe_relative(&file.path)
            .ok_or_else(|| HomeError::InvalidBundle(format!("unsafe path {}", file.path)))?;
        if is_secret_name(&file.path) {
            return Err(HomeError::InvalidBundle(format!(
                "key or credential file listed: {}",
                file.path
            )));
        }
        let path = output.join(&relative);
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(HomeError::InvalidBundle(format!(
                "{} is not a regular file",
                file.path
            )));
        }
        let bytes = fs::read(&path)?;
        if bytes.len() as u64 != file.bytes {
            return Err(HomeError::HashMismatch(file.path.clone()));
        }
        if hex_sha256(&bytes) != file.sha256 {
            return Err(HomeError::HashMismatch(file.path.clone()));
        }
        if !listed.insert(file.path.clone()) {
            return Err(HomeError::InvalidBundle(format!(
                "duplicate path {}",
                file.path
            )));
        }
    }
    let mut actual = Vec::new();
    collect_files(&output, &mut actual)?;
    for path in actual {
        let relative = path
            .strip_prefix(&output)
            .map_err(|_| HomeError::InvalidBundle("path escapes bundle".to_string()))?;
        let display = relative_display(relative)?;
        if display == EXPORT_MANIFEST_NAME {
            continue;
        }
        if !listed.remove(&display) {
            return Err(HomeError::InvalidBundle(format!("unlisted file {display}")));
        }
    }
    if !listed.is_empty() {
        return Err(HomeError::InvalidBundle(format!(
            "manifest lists missing files: {:?}",
            listed
        )));
    }
    Ok(manifest)
}

pub fn restore(bundle: &Path, root: &Path, _now_ms: u128) -> Result<RestoreOutcome, HomeError> {
    let manifest = verify_export(bundle)?;
    let bundle = absolute(bundle)?;
    let root = absolute(root)?;
    if let Some(metadata) = existing_metadata(&root)? {
        if !metadata.is_dir() {
            return Err(HomeError::UnsafeLayout(format!(
                "{} is not a directory",
                root.display()
            )));
        }
        if fs::read_dir(&root)?.next().is_some() {
            return Err(HomeError::OutputNotEmpty(root));
        }
    } else {
        create_private_dirs(&root)?;
    }
    set_mode(&root, 0o700)?;
    let mut files = manifest.files.clone();
    files.sort_by_key(|file| file.path == MANIFEST_NAME);
    for file in &files {
        let relative = safe_relative(&file.path)
            .ok_or_else(|| HomeError::InvalidBundle(format!("unsafe path {}", file.path)))?;
        let destination = root.join(&relative);
        if let Some(parent) = destination.parent() {
            create_private_dirs(parent)?;
        }
        let bytes = fs::read(bundle.join(&relative))?;
        write_new_private(&destination, &bytes)?;
    }
    sync_dir(&root)?;
    Ok(RestoreOutcome {
        root,
        files: manifest.files.len(),
    })
}

pub fn upgrade(
    root: &Path,
    check_only: bool,
    rollback: bool,
    now_ms: u128,
) -> Result<UpgradeOutcome, HomeError> {
    let root = absolute(root)?;
    let mut manifest = read_manifest(&root)?;
    let from_version = manifest.version;
    if from_version > HOME_VERSION {
        return Err(HomeError::UnsupportedVersion {
            found: from_version,
            supported: HOME_VERSION,
        });
    }
    if rollback {
        if from_version != HOME_VERSION {
            return Err(HomeError::UnsupportedVersion {
                found: from_version,
                supported: HOME_VERSION,
            });
        }
        let repaired = rollback_key_layout(&root, check_only)?;
        if !check_only {
            manifest.version = 1;
            manifest.updated_ms = now_ms;
            write_manifest(&root, &manifest)?;
        }
        return Ok(UpgradeOutcome {
            root,
            check_only,
            from_version,
            to_version: 1,
            rolled_back: true,
            repaired,
            changed: !check_only,
        });
    }
    if from_version == 1 {
        let mut repaired = migrate_key_layout(&root, check_only)?;
        if !check_only {
            manifest.version = 2;
            manifest.updated_ms = now_ms;
            write_manifest(&root, &manifest)?;
            repaired.extend(repair_layout(&root, false)?);
        } else {
            repaired.extend(missing_layout(&root)?);
        }
        return Ok(UpgradeOutcome {
            root,
            check_only,
            from_version,
            to_version: 2,
            rolled_back: false,
            repaired,
            changed: !check_only,
        });
    }
    if from_version != HOME_VERSION {
        return Err(HomeError::UnsupportedVersion {
            found: from_version,
            supported: HOME_VERSION,
        });
    }
    let repaired = repair_layout(&root, check_only)?;
    let changed = !check_only && !repaired.is_empty();
    if changed {
        manifest.updated_ms = now_ms;
        write_manifest(&root, &manifest)?;
    }
    Ok(UpgradeOutcome {
        root,
        check_only,
        from_version,
        to_version: manifest.version,
        rolled_back: false,
        repaired,
        changed,
    })
}

fn repair_layout(root: &Path, check_only: bool) -> Result<Vec<String>, HomeError> {
    let mut repaired = Vec::new();
    for name in HOME_DIRS {
        let path = root.join(name);
        match fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(HomeError::UnsafeLayout(format!(
                    "{} is a symlink",
                    path.display()
                )));
            }
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => {
                return Err(HomeError::UnsafeLayout(format!(
                    "{} is not a directory",
                    path.display()
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if !check_only {
                    create_private_dirs(&path)?;
                }
                repaired.push(name.to_string());
            }
            Err(error) => return Err(HomeError::Io(error)),
        }
    }
    Ok(repaired)
}

fn missing_layout(root: &Path) -> Result<Vec<String>, HomeError> {
    let mut missing = Vec::new();
    for name in HOME_DIRS {
        match fs::symlink_metadata(root.join(name)) {
            Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
                return Err(HomeError::UnsafeLayout(format!(
                    "{} is not a safe directory",
                    root.join(name).display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(name.to_string())
            }
            Err(error) => return Err(HomeError::Io(error)),
        }
    }
    Ok(missing)
}

fn migrate_key_layout(root: &Path, check_only: bool) -> Result<Vec<String>, HomeError> {
    let keys = root.join(KEYS_DIR);
    let mut repaired = Vec::new();
    match fs::symlink_metadata(&keys) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
            return Err(HomeError::UnsafeLayout(format!(
                "{} is not a safe directory",
                keys.display()
            )));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if !check_only {
                create_private_dirs(&keys)?;
            }
            repaired.push(KEYS_DIR.to_string());
        }
        Err(error) => return Err(HomeError::Io(error)),
    }
    let mut moves = Vec::new();
    for name in [MEMORY_KEY, MEMORY_LEDGER] {
        let source = root.join(name);
        let destination = keys.join(name);
        if let Some(metadata) = existing_metadata(&source)? {
            if !metadata.is_file() {
                return Err(HomeError::UnsafeLayout(format!(
                    "{} is not a regular file",
                    source.display()
                )));
            }
            if existing_metadata(&destination)?.is_some() {
                return Err(HomeError::UnsafeLayout(format!(
                    "refusing to overwrite {}",
                    destination.display()
                )));
            }
            repaired.push(format!("{KEYS_DIR}/{name}"));
            moves.push((source, destination));
        }
    }
    if !check_only {
        for (source, destination) in moves {
            fs::rename(&source, &destination)?;
        }
    }
    let stale_lock = root.join(format!("{MEMORY_KEY}.lock"));
    if existing_metadata(&stale_lock)?.is_some() && !check_only {
        let _ = fs::remove_file(&stale_lock);
    }
    if !check_only {
        sync_dir(root)?;
        if keys.is_dir() {
            sync_dir(&keys)?;
        }
    }
    Ok(repaired)
}

fn rollback_key_layout(root: &Path, check_only: bool) -> Result<Vec<String>, HomeError> {
    let keys = root.join(KEYS_DIR);
    match fs::symlink_metadata(&keys) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
            return Err(HomeError::UnsafeLayout(format!(
                "{} is not a safe directory",
                keys.display()
            )));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(HomeError::Io(error)),
    }
    let lock_name = format!("{MEMORY_KEY}.lock");
    let mut allowed = vec![MEMORY_KEY, MEMORY_LEDGER, lock_name.as_str()];
    for entry in fs::read_dir(&keys)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(|| {
            HomeError::UnsafeLayout("keys directory contains a non-UTF-8 name".into())
        })?;
        if !allowed.contains(&name) {
            return Err(HomeError::UnsafeLayout(format!(
                "keys directory contains unexpected file {name}"
            )));
        }
    }
    let mut repaired = Vec::new();
    let mut moves = Vec::new();
    for name in [MEMORY_KEY, MEMORY_LEDGER] {
        let source = keys.join(name);
        if let Some(metadata) = existing_metadata(&source)? {
            if !metadata.is_file() {
                return Err(HomeError::UnsafeLayout(format!(
                    "{} is not a regular file",
                    source.display()
                )));
            }
            let destination = root.join(name);
            if existing_metadata(&destination)?.is_some() {
                return Err(HomeError::UnsafeLayout(format!(
                    "refusing to overwrite {}",
                    destination.display()
                )));
            }
            repaired.push(name.to_string());
            moves.push((source, destination));
        }
    }
    if !check_only {
        for (source, destination) in moves {
            fs::rename(&source, &destination)?;
        }
    }
    if !check_only {
        let _ = fs::remove_file(keys.join(&lock_name));
        if fs::read_dir(&keys)?.next().is_some() {
            return Err(HomeError::UnsafeLayout(
                "keys directory is not empty after rollback".into(),
            ));
        }
        fs::remove_dir(&keys)?;
        sync_dir(root)?;
    }
    allowed.clear();
    Ok(repaired)
}

pub fn uninstall(root: &Path, purge: bool, confirmed: bool) -> Result<UninstallOutcome, HomeError> {
    let root = absolute(root)?;
    if existing_metadata(&root)?.is_none() {
        return Ok(UninstallOutcome {
            root,
            removed: false,
            message: "runtime home does not exist".to_string(),
        });
    }
    if !purge {
        return Ok(UninstallOutcome {
            root,
            removed: false,
            message: "data retained; rerun with --purge --yes to delete".to_string(),
        });
    }
    if !confirmed {
        return Err(HomeError::ConfirmationRequired);
    }
    read_manifest(&root)?;
    ensure_safe_delete(&root)?;
    fs::remove_dir_all(&root)?;
    Ok(UninstallOutcome {
        root,
        removed: true,
        message: "runtime home removed".to_string(),
    })
}

fn ensure_safe_delete(root: &Path) -> Result<(), HomeError> {
    let canonical = fs::canonicalize(root)?;
    if canonical == Path::new("/") {
        return Err(HomeError::Refused("refusing to delete /".to_string()));
    }
    let names = canonical
        .components()
        .filter(|component| matches!(component, Component::Normal(_)))
        .count();
    if names < 2 {
        return Err(HomeError::Refused(format!(
            "refusing to delete top-level path {}",
            canonical.display()
        )));
    }
    if let Some(home) = std::env::var_os("HOME")
        && let Ok(home) = fs::canonicalize(home)
        && canonical == home
    {
        return Err(HomeError::Refused(
            "refusing to delete the user home directory".to_string(),
        ));
    }
    let cwd = fs::canonicalize(std::env::current_dir()?)?;
    if cwd.starts_with(&canonical) {
        return Err(HomeError::Refused(format!(
            "refusing to delete {} while the current directory is inside it",
            canonical.display()
        )));
    }
    Ok(())
}

fn ensure_layout(root: &Path) -> Result<(), HomeError> {
    for name in HOME_DIRS {
        let path = root.join(name);
        match fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(HomeError::UnsafeLayout(format!(
                    "{} is a symlink",
                    path.display()
                )));
            }
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => {
                return Err(HomeError::UnsafeLayout(format!(
                    "{} is not a directory",
                    path.display()
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                create_private_dirs(&path)?;
            }
            Err(error) => return Err(HomeError::Io(error)),
        }
    }
    Ok(())
}

fn ensure_disjoint(root: &Path, path: &Path, message: &str) -> Result<(), HomeError> {
    let root = fs::canonicalize(root)?;
    let path = canonicalize_for_compare(path)?;
    if path.starts_with(&root) || root.starts_with(&path) {
        return Err(HomeError::UnsafeLayout(message.to_string()));
    }
    Ok(())
}

/// Resolve a possibly nonexistent path through its nearest existing ancestor so
/// a symlinked parent cannot smuggle an output back into the home.
fn canonicalize_for_compare(path: &Path) -> Result<PathBuf, HomeError> {
    let mut existing = path.to_path_buf();
    let mut suffix = Vec::new();
    loop {
        match fs::symlink_metadata(&existing) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(name) = existing.file_name() else {
                    return Err(HomeError::UnsafeLayout(format!(
                        "{} has no resolvable parent",
                        path.display()
                    )));
                };
                suffix.push(name.to_os_string());
                existing = existing.parent().map(Path::to_path_buf).ok_or_else(|| {
                    HomeError::UnsafeLayout(format!("{} has no resolvable parent", path.display()))
                })?;
            }
            Err(error) => return Err(HomeError::Io(error)),
        }
    }
    let mut resolved = fs::canonicalize(&existing)?;
    for name in suffix.iter().rev() {
        resolved.push(name);
    }
    Ok(resolved)
}

fn ensure_supported(manifest: &HomeManifest) -> Result<(), HomeError> {
    if manifest.version != HOME_VERSION {
        return Err(HomeError::UnsupportedVersion {
            found: manifest.version,
            supported: HOME_VERSION,
        });
    }
    Ok(())
}

fn read_manifest(root: &Path) -> Result<HomeManifest, HomeError> {
    let path = root.join(MANIFEST_NAME);
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            HomeError::MissingHome(root.to_path_buf())
        } else {
            HomeError::Io(error)
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(HomeError::UnsafeLayout(format!(
            "{} is not a regular file",
            path.display()
        )));
    }
    let manifest: HomeManifest = serde_json::from_slice(&fs::read(&path)?)?;
    if manifest.schema != HOME_SCHEMA {
        return Err(HomeError::UnsafeLayout(format!(
            "unexpected home schema {}",
            manifest.schema
        )));
    }
    Ok(manifest)
}

fn has_manifest(root: &Path) -> Result<bool, HomeError> {
    match fs::symlink_metadata(root.join(MANIFEST_NAME)) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(HomeError::UnsafeLayout(format!(
                    "{} is not a regular file",
                    root.join(MANIFEST_NAME).display()
                )));
            }
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(HomeError::Io(error)),
    }
}

fn write_manifest(root: &Path, manifest: &HomeManifest) -> Result<(), HomeError> {
    let path = root.join(MANIFEST_NAME);
    let bytes = serde_json::to_vec_pretty(manifest)?;
    let temp = root.join(format!(
        ".{MANIFEST_NAME}.tmp-{}-{}",
        std::process::id(),
        now_ms()
    ));
    let mut file = open_new_private(&temp)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    set_mode(&temp, 0o600)?;
    drop(file);
    fs::rename(&temp, &path)?;
    sync_dir(root)?;
    Ok(())
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), HomeError> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(HomeError::UnsafeLayout(format!(
                "{} is a symlink",
                path.display()
            )));
        }
        if metadata.is_dir() {
            collect_files(&path, out)?;
        } else if metadata.is_file() {
            out.push(path);
        } else {
            return Err(HomeError::UnsafeLayout(format!(
                "{} is not a regular file or directory",
                path.display()
            )));
        }
    }
    Ok(())
}

fn prepare_empty_output(output: &Path) -> Result<(), HomeError> {
    match fs::symlink_metadata(output) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(HomeError::UnsafeLayout(format!(
            "{} is a symlink",
            output.display()
        ))),
        Ok(metadata) if metadata.is_dir() => {
            if fs::read_dir(output)?.next().is_some() {
                Err(HomeError::OutputNotEmpty(output.to_path_buf()))
            } else {
                set_mode(output, 0o700)?;
                Ok(())
            }
        }
        Ok(_) => Err(HomeError::OutputNotEmpty(output.to_path_buf())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            create_private_dirs(output)?;
            Ok(())
        }
        Err(error) => Err(HomeError::Io(error)),
    }
}

fn write_json_file(path: &Path, value: &impl Serialize) -> Result<(), HomeError> {
    let bytes = serde_json::to_vec_pretty(value)?;
    write_new_private(path, &bytes)
}

fn write_new_private(path: &Path, bytes: &[u8]) -> Result<(), HomeError> {
    let mut file = open_new_private(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn open_new_private(path: &Path) -> Result<fs::File, HomeError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    Ok(options.open(path)?)
}

fn create_private_dirs(path: &Path) -> Result<(), HomeError> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)?;
    set_mode(path, 0o700)?;
    Ok(())
}

fn sync_dir(path: &Path) -> Result<(), HomeError> {
    let file = fs::File::open(path)?;
    file.sync_all()?;
    Ok(())
}

fn existing_metadata(path: &Path) -> Result<Option<fs::Metadata>, HomeError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(HomeError::UnsafeLayout(format!(
                    "{} is a symlink",
                    path.display()
                )));
            }
            Ok(Some(metadata))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(HomeError::Io(error)),
    }
}

fn absolute(path: &Path) -> Result<PathBuf, HomeError> {
    if path.as_os_str().is_empty() {
        return Err(HomeError::UnsafeLayout("empty path".to_string()));
    }
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    Ok(normalize_lexically(&joined))
}

fn normalize_lexically(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn safe_relative(path: &str) -> Option<PathBuf> {
    let path = Path::new(path);
    if path.is_absolute() {
        return None;
    }
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => out.push(part),
            _ => return None,
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

fn relative_display(path: &Path) -> Result<String, HomeError> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                let part = part.to_str().ok_or_else(|| {
                    HomeError::UnsafeLayout(format!(
                        "filename is not valid UTF-8: {}",
                        path.display()
                    ))
                })?;
                parts.push(part);
            }
            _ => {
                return Err(HomeError::UnsafeLayout(format!(
                    "unsafe relative path: {}",
                    path.display()
                )));
            }
        }
    }
    Ok(parts.join("/"))
}

fn is_secret_name(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower
        .split('/')
        .any(|part| part.contains(".key") || part.contains("credential") || part.contains("secret"))
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

fn check(id: &str, status: DoctorStatus, detail: impl Into<String>) -> DoctorCheck {
    DoctorCheck {
        id: id.to_string(),
        status,
        detail: detail.into(),
    }
}

fn finish_report(root: PathBuf, checks: Vec<DoctorCheck>) -> DoctorReport {
    let status = checks
        .iter()
        .map(|check| check.status)
        .max()
        .unwrap_or(DoctorStatus::Ok);
    DoctorReport {
        root,
        status,
        checks,
    }
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<(), HomeError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<(), HomeError> {
    Ok(())
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

/// Timestamp helper for CLI callers that do not inject a clock.
pub fn current_time_ms() -> u128 {
    now_ms()
}
