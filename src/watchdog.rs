//! Out-of-band daemon watchdog.
//!
//! The daemon writes a small heartbeat file on every loop. A separate process
//! can then check its age without sharing memory or a socket with the daemon.

use std::io::Write;
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WatchdogAction {
    Ok,
    Warn,
    Pause,
    Repair,
}

impl WatchdogAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Pause => "pause",
            Self::Repair => "repair",
        }
    }
}

/// Escalation thresholds. `Pause` and `Repair` are recommendations: this
/// module never signals a process or expands authority.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WatchdogPolicy {
    pub warn_after_ms: u128,
    pub pause_after_ms: u128,
    pub repair_after_ms: u128,
}

impl WatchdogPolicy {
    pub fn new(
        warn_after_ms: u128,
        pause_after_ms: u128,
        repair_after_ms: u128,
    ) -> Result<Self, WatchdogError> {
        if warn_after_ms == 0
            || warn_after_ms >= pause_after_ms
            || pause_after_ms >= repair_after_ms
        {
            return Err(WatchdogError::Invalid(
                "watchdog thresholds must be positive and strictly increasing".into(),
            ));
        }
        Ok(Self {
            warn_after_ms,
            pause_after_ms,
            repair_after_ms,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WatchdogDecision {
    pub action: WatchdogAction,
    pub reason: String,
    pub pid: Option<u32>,
    pub age_ms: Option<u128>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WatchdogIntervention {
    pub at_ms: u128,
    pub action: WatchdogAction,
    pub pid: Option<u32>,
    pub age_ms: Option<u128>,
    pub reason: String,
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

/// Map a heartbeat age to an escalation action. A missing heartbeat goes
/// straight to `Repair` because there is nothing to observe.
pub fn decide(
    heartbeat: Option<&Heartbeat>,
    now_ms: u128,
    policy: WatchdogPolicy,
) -> WatchdogDecision {
    let Some(heartbeat) = heartbeat else {
        return WatchdogDecision {
            action: WatchdogAction::Repair,
            reason: "heartbeat is missing; operator must inspect the daemon".into(),
            pid: None,
            age_ms: None,
        };
    };
    let age_ms = now_ms.saturating_sub(heartbeat.updated_at_ms);
    let action = if age_ms > policy.repair_after_ms {
        WatchdogAction::Repair
    } else if age_ms > policy.pause_after_ms {
        WatchdogAction::Pause
    } else if age_ms > policy.warn_after_ms {
        WatchdogAction::Warn
    } else {
        WatchdogAction::Ok
    };
    let reason = match action {
        WatchdogAction::Ok => "heartbeat is fresh".into(),
        WatchdogAction::Warn => {
            format!("heartbeat is stale by {age_ms} ms; inspect daemon logs")
        }
        WatchdogAction::Pause => {
            format!("heartbeat is stale by {age_ms} ms; pause new work")
        }
        WatchdogAction::Repair => {
            format!("heartbeat is stale by {age_ms} ms; operator repair required")
        }
    };
    WatchdogDecision {
        action,
        reason,
        pid: Some(heartbeat.pid),
        age_ms: Some(age_ms),
    }
}

/// Append an intervention record when the action changes. Repeating the same
/// action is not logged again; a recovery to `Ok` is logged once.
pub fn append_intervention(
    path: &Path,
    decision: &WatchdogDecision,
    now_ms: u128,
) -> Result<(), WatchdogError> {
    let previous = last_intervention(path)?;
    if previous.as_ref().map(|record| record.action) == Some(decision.action)
        || (previous.is_none() && decision.action == WatchdogAction::Ok)
    {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let record = WatchdogIntervention {
        at_ms: now_ms,
        action: decision.action,
        pid: decision.pid,
        age_ms: decision.age_ms,
        reason: decision.reason.clone(),
    };
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    serde_json::to_writer(&mut file, &record)?;
    file.write_all(b"\n")?;
    file.flush()?;
    Ok(())
}

fn last_intervention(path: &Path) -> Result<Option<WatchdogIntervention>, WatchdogError> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let Some(line) = bytes
        .split(|byte| *byte == b'\n')
        .rev()
        .find(|line| !line.iter().all(u8::is_ascii_whitespace))
    else {
        return Ok(None);
    };
    let record =
        serde_json::from_slice(line).map_err(|error| WatchdogError::Invalid(error.to_string()))?;
    Ok(Some(record))
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
    fn policy_rejects_invalid_threshold_order() {
        assert!(WatchdogPolicy::new(0, 2_000, 4_000).is_err());
        assert!(WatchdogPolicy::new(2_000, 2_000, 4_000).is_err());
        assert!(WatchdogPolicy::new(2_000, 3_000, 3_000).is_err());
        assert!(WatchdogPolicy::new(1_000, 2_000, 4_000).is_ok());
    }

    #[test]
    fn decide_escalates_through_the_ladder() {
        let policy = WatchdogPolicy::new(1_000, 2_000, 4_000).unwrap();
        let fresh = heartbeat(1_000);
        assert_eq!(
            decide(Some(&fresh), 1_500, policy).action,
            WatchdogAction::Ok
        );
        assert_eq!(
            decide(Some(&fresh), 2_500, policy).action,
            WatchdogAction::Warn
        );
        assert_eq!(
            decide(Some(&fresh), 3_500, policy).action,
            WatchdogAction::Pause
        );
        assert_eq!(
            decide(Some(&fresh), 5_500, policy).action,
            WatchdogAction::Repair
        );
        assert_eq!(decide(None, 1_500, policy).action, WatchdogAction::Repair);
    }

    #[test]
    fn intervention_log_records_transitions_only() {
        let path = std::env::temp_dir().join(format!(
            "1agent-watchdog-interventions-{}-{}.jsonl",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let policy = WatchdogPolicy::new(1_000, 2_000, 4_000).unwrap();
        let fresh = heartbeat(1_000);
        let warn = decide(Some(&fresh), 2_500, policy);
        append_intervention(&path, &warn, 2_500).unwrap();
        append_intervention(&path, &warn, 2_600).unwrap();
        let pause = decide(Some(&fresh), 3_500, policy);
        append_intervention(&path, &pause, 3_500).unwrap();
        let recovered = decide(Some(&heartbeat(3_600)), 3_700, policy);
        append_intervention(&path, &recovered, 3_700).unwrap();
        append_intervention(&path, &recovered, 3_800).unwrap();

        let records = std::fs::read_to_string(&path).unwrap();
        let actions = records
            .lines()
            .map(|line| {
                serde_json::from_str::<WatchdogIntervention>(line)
                    .unwrap()
                    .action
            })
            .collect::<Vec<_>>();
        assert_eq!(
            actions,
            [
                WatchdogAction::Warn,
                WatchdogAction::Pause,
                WatchdogAction::Ok
            ]
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
