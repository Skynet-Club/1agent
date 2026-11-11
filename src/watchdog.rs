//! Out-of-band daemon watchdog.
//!
//! The daemon writes a small heartbeat file on every loop. A separate process
//! can then check its age without sharing memory or a socket with the daemon.

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WatchdogError {
    #[error("watchdog heartbeat I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("watchdog heartbeat is invalid: {0}")]
    Invalid(String),
    #[error("watchdog heartbeat JSON failed: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Heartbeat {
    pub pid: u32,
    pub updated_at_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WatchdogStatus {
    pub healthy: bool,
    pub reason: String,
    pub pid: Option<u32>,
    pub age_ms: Option<u128>,
}

pub fn touch(path: &Path, pid: u32, now_ms: u128) -> Result<(), WatchdogError> {
    let heartbeat = Heartbeat {
        pid,
        updated_at_ms: now_ms,
    };
    let temporary = path.with_extension(format!("tmp-{pid}"));
    std::fs::write(&temporary, serde_json::to_vec(&heartbeat)?)?;
    std::fs::rename(&temporary, path)?;
    Ok(())
}

pub fn read(path: &Path) -> Result<Option<Heartbeat>, WatchdogError> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let heartbeat: Heartbeat = serde_json::from_slice(&bytes)
        .map_err(|error| WatchdogError::Invalid(error.to_string()))?;
    Ok(Some(heartbeat))
}

pub fn evaluate(heartbeat: Option<&Heartbeat>, now_ms: u128, max_age_ms: u128) -> WatchdogStatus {
    let Some(heartbeat) = heartbeat else {
        return WatchdogStatus {
            healthy: false,
            reason: "heartbeat is missing".into(),
            pid: None,
            age_ms: None,
        };
    };
    let age_ms = now_ms.saturating_sub(heartbeat.updated_at_ms);
    if age_ms > max_age_ms {
        return WatchdogStatus {
            healthy: false,
            reason: format!("heartbeat is stale by {age_ms} ms"),
            pid: Some(heartbeat.pid),
            age_ms: Some(age_ms),
        };
    }
    WatchdogStatus {
        healthy: true,
        reason: "heartbeat is fresh".into(),
        pid: Some(heartbeat.pid),
        age_ms: Some(age_ms),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heartbeat(updated_at_ms: u128) -> Heartbeat {
        Heartbeat {
            pid: 7,
            updated_at_ms,
        }
    }

    #[test]
    fn fresh_heartbeat_is_healthy() {
        let status = evaluate(Some(&heartbeat(1_000)), 1_500, 1_000);
        assert!(status.healthy);
        assert_eq!(status.age_ms, Some(500));
        assert_eq!(status.pid, Some(7));
    }

    #[test]
    fn stale_and_missing_heartbeats_are_unhealthy() {
        let stale = evaluate(Some(&heartbeat(1_000)), 3_000, 1_000);
        assert!(!stale.healthy);
        assert_eq!(stale.age_ms, Some(2_000));
        let missing = evaluate(None, 1_000, 1_000);
        assert!(!missing.healthy);
        assert!(missing.age_ms.is_none());
    }

    #[test]
    fn heartbeat_round_trips_through_the_file() {
        let path = std::env::temp_dir().join(format!(
            "1agent-heartbeat-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        touch(&path, 42, 1234).unwrap();
        assert_eq!(
            read(&path).unwrap(),
            Some(Heartbeat {
                pid: 42,
                updated_at_ms: 1234,
            })
        );
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_missing_file_reads_as_absent() {
        let path = std::env::temp_dir().join(format!(
            "1agent-heartbeat-missing-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        assert_eq!(read(&path).unwrap(), None);
    }
}
