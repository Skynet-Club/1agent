//! systemd user-unit rendering for the local daemon.
//!
//! The unit supervises the existing daemon; it does not add auto-resume or
//! auto-kill semantics. It deliberately does not enable lingering.

use std::path::Path;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum UnitError {
    #[error("unit io: {0}")]
    Io(#[from] std::io::Error),
}

pub fn render_user_unit(
    executable: &Path,
    store: &Path,
    journal_dir: &Path,
    run_ledger: &Path,
    home: &Path,
) -> String {
    let executable = escape(executable);
    let store = escape(store);
    let journal_dir = escape(journal_dir);
    let run_ledger = escape(run_ledger);
    let home = escape(home);
    format!(
        "[Unit]\n\
Description=1Agent local daemon\n\
After=network-online.target\n\
\n\
[Service]\n\
Type=notify\n\
NotifyAccess=main\n\
WatchdogSec=60\n\
ExecStart={executable} daemon --store {store} --journal-dir {journal_dir} --run-ledger {run_ledger}\n\
Restart=on-failure\n\
RestartSec=5\n\
TimeoutStopSec=30\n\
KillMode=mixed\n\
NoNewPrivileges=true\n\
PrivateTmp=true\n\
ProtectSystem=strict\n\
ProtectHome=read-only\n\
ReadWritePaths={home}\n\
UMask=0077\n\
\n\
[Install]\n\
WantedBy=default.target\n"
    )
}

pub fn write_user_unit(path: &Path, unit: &str) -> Result<(), UnitError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, unit)?;
    Ok(())
}

fn escape(path: &Path) -> String {
    let value = path.display().to_string();
    if value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "/._-:%@+".contains(character))
    {
        value
    } else {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_a_hardened_user_unit_without_linger() {
        let unit = render_user_unit(
            Path::new("/usr/local/bin/1agent"),
            Path::new("/home/u/.1a/schedules.json"),
            Path::new("/home/u/.1a/daemon/journals"),
            Path::new("/home/u/.1a/schedule-runs.jsonl"),
            Path::new("/home/u/.1a"),
        );
        assert!(unit.contains("ExecStart=/usr/local/bin/1agent daemon"));
        assert!(unit.contains("NoNewPrivileges=true"));
        assert!(unit.contains("ReadWritePaths=/home/u/.1a"));
        assert!(unit.contains("WantedBy=default.target"));
        assert!(!unit.to_ascii_lowercase().contains("linger"));
    }

    #[test]
    fn quotes_paths_with_spaces() {
        let unit = render_user_unit(
            Path::new("/opt/my agent/1agent"),
            Path::new("/tmp/a b/schedules.json"),
            Path::new("/tmp/a b/journals"),
            Path::new("/tmp/a b/runs.jsonl"),
            Path::new("/tmp/a b"),
        );
        assert!(unit.contains("ExecStart=\"/opt/my agent/1agent\""));
        assert!(unit.contains("ReadWritePaths=\"/tmp/a b\""));
    }
}
