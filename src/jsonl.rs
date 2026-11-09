//! Small JSONL durability helpers shared by append-only stores.

use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Cross-process advisory lock for an append-only store. The lock file is a
/// sibling named `<store>.lock`; callers must hold the guard for the whole
/// read-check-write section.
pub struct FileLock {
    _file: File,
}

#[derive(Debug)]
pub enum LockedAppendError {
    Io(std::io::Error),
    ConcurrentWrite {
        expected_bytes: u64,
        actual_bytes: u64,
    },
}

/// Lock, verify that the log still has the expected length, append, and return
/// the new length. A stale caller fails closed instead of duplicating a
/// sequence.
pub fn append_batch_locked(
    path: &Path,
    expected_bytes: u64,
    buffer: &str,
) -> Result<u64, LockedAppendError> {
    let _lock = FileLock::exclusive(path).map_err(LockedAppendError::Io)?;
    let actual_bytes = std::fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    if actual_bytes != expected_bytes {
        return Err(LockedAppendError::ConcurrentWrite {
            expected_bytes,
            actual_bytes,
        });
    }
    append_batch(path, buffer).map_err(LockedAppendError::Io)?;
    Ok(std::fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(expected_bytes))
}

impl FileLock {
    pub fn exclusive(path: &Path) -> std::io::Result<Self> {
        let mut lock_path = path.as_os_str().to_os_string();
        lock_path.push(".lock");
        let lock_path = PathBuf::from(lock_path);
        ensure_parent_exists(&lock_path)?;
        let mut options = OpenOptions::new();
        options.create(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let file = options.open(&lock_path)?;
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
            if result != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        Ok(Self { _file: file })
    }
}

pub fn repair_torn_tail(path: &Path) -> std::io::Result<()> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if metadata.file_type().is_symlink() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "jsonl path must not be a symlink",
        ));
    }
    if !metadata.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "jsonl path must be a regular file",
        ));
    }
    let data = std::fs::read(path)?;
    if data.is_empty() || data.ends_with(b"\n") {
        return Ok(());
    }
    let last_newline = data
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let tail = &data[last_newline..];
    let valid_tail = serde_json::from_slice::<serde_json::Value>(tail).is_ok();
    let mut options = OpenOptions::new();
    options.write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options.open(path)?;
    if valid_tail {
        file.seek(SeekFrom::End(0))?;
        file.write_all(b"\n")?;
    } else {
        file.set_len(last_newline as u64)?;
    }
    file.sync_data()?;
    Ok(())
}

/// Append one buffer and truncate back to the previous length on a partial
/// write, so a retry cannot leave a merged/torn line behind.
pub fn append_batch(path: &Path, buffer: &str) -> std::io::Result<()> {
    let mut file = open_append_private(path)?;
    let start = file.metadata()?.len();
    let result = (|| -> std::io::Result<()> {
        file.write_all(buffer.as_bytes())?;
        file.sync_data()?;
        Ok(())
    })();
    if let Err(error) = result {
        if file.set_len(start).is_err() {
            return Err(std::io::Error::other(
                "append failed and rollback failed; the log may contain a torn line",
            ));
        }
        let _ = file.sync_data();
        return Err(error);
    }
    Ok(())
}

pub fn open_append_private(path: &Path) -> std::io::Result<File> {
    ensure_parent_private(path)?;
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = file.metadata()?.permissions();
        permissions.set_mode(0o600);
        file.set_permissions(permissions)?;
    }
    Ok(file)
}

fn ensure_parent_exists(path: &Path) -> std::io::Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    if parent.as_os_str().is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(parent)
}

pub fn ensure_parent_private(path: &Path) -> std::io::Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    if parent.as_os_str().is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(parent)?.permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(parent, permissions)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn torn_tail_is_truncated_to_the_last_complete_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        std::fs::write(&path, "{\"a\":1}\n{\"b\":2}\n{\"c\"").unwrap();
        repair_torn_tail(&path).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{\"a\":1}\n{\"b\":2}\n"
        );
    }

    #[test]
    fn complete_json_without_newline_gets_one() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        std::fs::write(&path, "{\"a\":1}\n{\"b\":2}").unwrap();
        repair_torn_tail(&path).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{\"a\":1}\n{\"b\":2}\n"
        );
    }

    #[test]
    fn missing_file_is_a_noop_and_directories_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        repair_torn_tail(&dir.path().join("missing.jsonl")).unwrap();
        let error = repair_torn_tail(dir.path()).unwrap_err();
        assert!(error.to_string().contains("regular file"));
    }

    #[cfg(unix)]
    #[test]
    fn repair_refuses_symlinks_and_leaves_the_target_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let victim = dir.path().join("victim.jsonl");
        std::fs::write(&victim, "{\"a\":1}").unwrap();
        let link = dir.path().join("link.jsonl");
        std::os::unix::fs::symlink(&victim, &link).unwrap();
        assert!(repair_torn_tail(&link).is_err());
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "{\"a\":1}");
    }

    #[cfg(unix)]
    #[test]
    fn append_is_private_and_refuses_symlinks() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("private.jsonl");
        let mut file = open_append_private(&path).unwrap();
        file.write_all(b"{}\n").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        let link = dir.path().join("link.jsonl");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(open_append_private(&link).is_err());
    }
}
