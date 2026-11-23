//! Delivery-time workspace snapshot.
//!
//! The run journal records tool calls and write receipts, but a terminal command
//! can create or change a file without one. This snapshot pins the files that
//! exist when the run delivers, so post-run acceptance can verify the delivered
//! bytes instead of trusting the model.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_FILES: usize = 2048;
pub const MAX_BYTES: u64 = 16 * 1024 * 1024;
const SKIP_DIRS: &[&str] = &[
    ".git",
    ".journals",
    ".journal",
    "target",
    "node_modules",
    ".venv",
    "venv",
    "__pycache__",
    ".pytest_cache",
    ".cache",
    ".1a",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceSnapshot {
    pub root: String,
    pub files: Vec<WorkspaceSnapshotFile>,
    pub truncated: bool,
    pub captured_unix_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceSnapshotFile {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

pub fn capture(root: &Path) -> WorkspaceSnapshot {
    let root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let mut files = Vec::new();
    let mut truncated = false;
    let mut total_bytes = 0u64;
    walk(&root, &mut files, &mut truncated, &mut total_bytes);
    files.sort_by(|left, right| left.path.cmp(&right.path));
    WorkspaceSnapshot {
        root: root.to_string_lossy().into_owned(),
        files,
        truncated,
        captured_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default(),
    }
}

fn walk(
    root: &Path,
    files: &mut Vec<WorkspaceSnapshotFile>,
    truncated: &mut bool,
    total: &mut u64,
) {
    let mut entries = match fs::read_dir(root) {
        Ok(entries) => entries.flatten().collect::<Vec<_>>(),
        Err(_) => return,
    };
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        if files.len() >= MAX_FILES || *total >= MAX_BYTES {
            *truncated = true;
            return;
        }
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if SKIP_DIRS.contains(&name.as_ref()) {
                continue;
            }
            walk(&path, files, truncated, total);
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let bytes = metadata.len();
        if total.saturating_add(bytes) > MAX_BYTES {
            *truncated = true;
            return;
        }
        let Ok(data) = fs::read(&path) else {
            continue;
        };
        *total = total.saturating_add(bytes);
        files.push(WorkspaceSnapshotFile {
            path: path.to_string_lossy().into_owned(),
            sha256: hex::encode(Sha256::digest(&data)),
            bytes,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_files_and_skips_internal_directories() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "a").unwrap();
        std::fs::create_dir_all(dir.path().join("notes")).unwrap();
        std::fs::write(dir.path().join("notes/b.txt"), "b").unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        std::fs::write(dir.path().join(".git/ignored"), "x").unwrap();

        let snapshot = capture(dir.path());
        let names = snapshot
            .files
            .iter()
            .map(|file| {
                Path::new(&file.path)
                    .strip_prefix(&snapshot.root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["a.txt", "notes/b.txt"]);
        assert!(snapshot.files.iter().all(|file| file.sha256.len() == 64));
        assert!(!snapshot.truncated);
    }
}
