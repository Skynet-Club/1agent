//! Tree-wide subagent budget shared by every process in a spawn tree.
//!
//! Children are separate processes, so the budget lives in a small JSON file
//! guarded by an exclusive file lock. The root fixes the limits when it creates
//! the file; children inherit the path and cannot widen them.
//!
//! A crash between a child exiting and `finish` undercounts that child's usage.
//! Stale reservations are pruned by process liveness; orphaned children are
//! handled by MA-011.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::usage_ledger::UsageTotals;

pub const TREE_FILE_ENV: &str = "ONEAGENT_SUBAGENT_TREE_FILE";
pub const TREE_MAX_AGENTS_ENV: &str = "ONEAGENT_SUBAGENT_TREE_MAX_AGENTS";
pub const TREE_MAX_COST_ENV: &str = "ONEAGENT_SUBAGENT_TREE_MAX_COST_MICROS";
pub const TREE_MAX_TOKENS_ENV: &str = "ONEAGENT_SUBAGENT_TREE_MAX_TOKENS";
pub const DEFAULT_MAX_AGENTS: u64 = 16;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreeLimits {
    pub max_agents: u64,
    #[serde(default)]
    pub max_cost_micros: Option<u64>,
    #[serde(default)]
    pub max_tokens: Option<u64>,
}

impl Default for TreeLimits {
    fn default() -> Self {
        Self {
            max_agents: DEFAULT_MAX_AGENTS,
            max_cost_micros: None,
            max_tokens: None,
        }
    }
}

impl TreeLimits {
    fn from_env() -> Result<Self, String> {
        let mut limits = Self::default();
        if let Ok(value) = std::env::var(TREE_MAX_AGENTS_ENV) {
            limits.max_agents = parse_limit(TREE_MAX_AGENTS_ENV, &value)?;
        }
        if let Ok(value) = std::env::var(TREE_MAX_COST_ENV) {
            limits.max_cost_micros = Some(parse_limit(TREE_MAX_COST_ENV, &value)?);
        }
        if let Ok(value) = std::env::var(TREE_MAX_TOKENS_ENV) {
            limits.max_tokens = Some(parse_limit(TREE_MAX_TOKENS_ENV, &value)?);
        }
        Ok(limits)
    }
}

fn parse_limit(name: &str, value: &str) -> Result<u64, String> {
    value
        .trim()
        .parse()
        .map_err(|_| format!("{name} must be a non-negative integer, got '{value}'"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TreeState {
    schema_version: u32,
    root_pid: u32,
    limits: TreeLimits,
    spent_cost_micros: u64,
    spent_tokens: u64,
    spawned: u64,
    active: Vec<ActiveSpawn>,
}

impl TreeState {
    fn new(limits: TreeLimits) -> Self {
        Self {
            schema_version: 1,
            root_pid: std::process::id(),
            limits,
            spent_cost_micros: 0,
            spent_tokens: 0,
            spawned: 0,
            active: Vec::new(),
        }
    }

    fn prune_dead(&mut self) {
        self.active
            .retain(|spawn| pid_alive(spawn.owner_pid) || spawn.child_pid.is_some_and(pid_alive));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ActiveSpawn {
    id: String,
    owner_pid: u32,
    #[serde(default)]
    child_pid: Option<u32>,
}

#[cfg(unix)]
fn pid_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    // SAFETY: kill(pid, 0) performs no signal delivery; it only checks existence.
    let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
    result == 0 || io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(not(unix))]
fn pid_alive(_pid: u32) -> bool {
    true
}

/// Budget file for one spawn tree. `path` is either inherited from the parent
/// (`ONEAGENT_SUBAGENT_TREE_FILE`) or derived from the root run id.
pub struct TreeBudget {
    path: PathBuf,
    limits: TreeLimits,
}

impl TreeBudget {
    pub fn resolve(
        instance_id: Uuid,
        run_id: Option<&str>,
        cache_root: &Path,
    ) -> Result<Self, String> {
        let path = match std::env::var_os(TREE_FILE_ENV) {
            Some(path) if !path.is_empty() => PathBuf::from(path),
            _ => {
                let key = run_id
                    .filter(|run_id| !run_id.trim().is_empty())
                    .map(safe_key)
                    .unwrap_or_else(|| format!("instance-{instance_id}"));
                cache_root.join("trees").join(format!("{key}.json"))
            }
        };
        Ok(Self {
            path,
            limits: TreeLimits::from_env()?,
        })
    }

    #[cfg(test)]
    pub fn at(path: PathBuf, limits: TreeLimits) -> Self {
        Self { path, limits }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Atomically take one concurrency slot.
    pub fn reserve(&self, id: String) -> Result<TreeReservation, String> {
        self.with_state(|state| {
            state.prune_dead();
            if let Some(limit) = state.limits.max_cost_micros
                && state.spent_cost_micros >= limit
            {
                return Err(format!("cost budget {limit} reached"));
            }
            if let Some(limit) = state.limits.max_tokens
                && state.spent_tokens >= limit
            {
                return Err(format!("token budget {limit} reached"));
            }
            if state.active.len() as u64 >= state.limits.max_agents {
                return Err(format!(
                    "concurrency limit {} reached",
                    state.limits.max_agents
                ));
            }
            state.active.push(ActiveSpawn {
                id: id.clone(),
                owner_pid: std::process::id(),
                child_pid: None,
            });
            state.spawned = state.spawned.saturating_add(1);
            Ok(())
        })?;
        Ok(TreeReservation {
            path: self.path.clone(),
            id,
            finished: false,
        })
    }

    fn with_state<T>(
        &self,
        update: impl FnOnce(&mut TreeState) -> Result<T, String>,
    ) -> Result<T, String> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| format!("tree budget path {} has no parent", self.path.display()))?;
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "cannot create tree budget directory {}: {error}",
                parent.display()
            )
        })?;
        let lock = lock_file(&self.path)?;
        let mut state = match std::fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| {
                format!(
                    "unreadable tree budget {}: {error}; remove it to start a new tree",
                    self.path.display()
                )
            })?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => TreeState::new(self.limits),
            Err(error) => {
                return Err(format!(
                    "cannot read tree budget {}: {error}",
                    self.path.display()
                ));
            }
        };
        let result = update(&mut state);
        let written = write_state(&self.path, &state);
        let _ = lock.unlock();
        match (result, written) {
            (Ok(value), Ok(())) => Ok(value),
            (Ok(_), Err(error)) => Err(error),
            (Err(error), _) => Err(error),
        }
    }
}

/// A held concurrency slot. Dropping it without `finish` releases the slot;
/// finishing also adds the child's usage to the tree totals.
#[derive(Debug)]
pub struct TreeReservation {
    path: PathBuf,
    id: String,
    finished: bool,
}

impl TreeReservation {
    pub fn set_child_pid(&mut self, pid: u32) {
        let id = self.id.clone();
        let _ = update_existing(&self.path, |state| {
            if let Some(spawn) = state.active.iter_mut().find(|spawn| spawn.id == id) {
                spawn.child_pid = Some(pid);
            }
        });
    }

    /// Release the slot and add usage. Best effort: a failed write only
    /// undercounts, it must not turn a completed child into a failed tool call.
    pub fn finish(&mut self, usage: &UsageTotals) {
        if self.finished {
            return;
        }
        self.finished = true;
        let id = self.id.clone();
        let _ = update_existing(&self.path, |state| {
            state.active.retain(|spawn| spawn.id != id);
            state.spent_cost_micros = state.spent_cost_micros.saturating_add(usage.cost_micros);
            state.spent_tokens = state
                .spent_tokens
                .saturating_add(usage.input_tokens)
                .saturating_add(usage.output_tokens);
        });
    }
}

impl Drop for TreeReservation {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        let id = self.id.clone();
        let _ = update_existing(&self.path, |state| {
            state.active.retain(|spawn| spawn.id != id);
        });
    }
}

fn lock_file(path: &Path) -> Result<File, String> {
    let lock_path = path.with_extension("lock");
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|error| format!("cannot open tree lock {}: {error}", lock_path.display()))?;
    file.lock()
        .map_err(|error| format!("cannot lock tree budget {}: {error}", path.display()))?;
    Ok(file)
}

fn write_state(path: &Path, state: &TreeState) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(state)
        .map_err(|error| format!("cannot encode tree budget: {error}"))?;
    let temp = path.with_extension(format!("tmp.{}", Uuid::new_v4()));
    std::fs::write(&temp, bytes)
        .map_err(|error| format!("cannot write tree budget {}: {error}", temp.display()))?;
    std::fs::rename(&temp, path).map_err(|error| {
        let _ = std::fs::remove_file(&temp);
        format!("cannot replace tree budget {}: {error}", path.display())
    })
}

fn update_existing(path: &Path, update: impl FnOnce(&mut TreeState)) -> Result<(), String> {
    if !path.is_file() {
        return Ok(());
    }
    let lock = lock_file(path)?;
    let mut state: TreeState = match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| error.to_string())?,
        Err(error) => return Err(error.to_string()),
    };
    update(&mut state);
    let written = write_state(path, &state);
    let _ = lock.unlock();
    written
}

fn safe_key(run_id: &str) -> String {
    run_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_budget(max_agents: u64) -> (tempfile::TempDir, TreeBudget) {
        let dir = tempfile::tempdir().unwrap();
        let budget = TreeBudget::at(
            dir.path().join("tree.json"),
            TreeLimits {
                max_agents,
                max_cost_micros: None,
                max_tokens: None,
            },
        );
        (dir, budget)
    }

    #[test]
    fn invalid_limit_text_is_rejected() {
        assert_eq!(parse_limit("X", "12").unwrap(), 12);
        assert!(parse_limit("X", "oops").is_err());
        assert!(parse_limit("X", "-1").is_err());
    }

    #[test]
    fn concurrency_slot_is_released_on_drop() {
        let (_dir, budget) = temp_budget(1);
        let first = budget.reserve("first".into()).unwrap();
        let error = budget.reserve("second".into()).unwrap_err();
        assert!(error.contains("concurrency limit 1"), "{error}");
        drop(first);
        budget.reserve("second".into()).unwrap();
    }

    #[test]
    fn finished_usage_is_charged_to_the_tree() {
        let (dir, budget) = temp_budget(4);
        let mut first = budget.reserve("first".into()).unwrap();
        first.finish(&UsageTotals {
            calls: 2,
            input_tokens: 100,
            output_tokens: 50,
            cost_micros: 7,
            zero_cost_calls: 0,
        });
        let text = std::fs::read_to_string(dir.path().join("tree.json")).unwrap();
        let state: TreeState = serde_json::from_str(&text).unwrap();
        assert_eq!(state.spent_cost_micros, 7);
        assert_eq!(state.spent_tokens, 150);
        assert!(state.active.is_empty());
        assert_eq!(state.spawned, 1);
    }

    #[test]
    fn cost_and_token_limits_reject_new_spawns() {
        let dir = tempfile::tempdir().unwrap();
        let budget = TreeBudget::at(
            dir.path().join("tree.json"),
            TreeLimits {
                max_agents: 4,
                max_cost_micros: Some(10),
                max_tokens: Some(20),
            },
        );
        let mut first = budget.reserve("first".into()).unwrap();
        first.finish(&UsageTotals {
            calls: 1,
            input_tokens: 20,
            output_tokens: 0,
            cost_micros: 10,
            zero_cost_calls: 0,
        });
        let error = budget.reserve("second".into()).unwrap_err();
        assert!(error.contains("cost budget 10"), "{error}");
    }

    #[test]
    fn token_limit_rejects_new_spawns() {
        let dir = tempfile::tempdir().unwrap();
        let budget = TreeBudget::at(
            dir.path().join("tree.json"),
            TreeLimits {
                max_agents: 4,
                max_cost_micros: None,
                max_tokens: Some(20),
            },
        );
        let mut first = budget.reserve("first".into()).unwrap();
        first.finish(&UsageTotals {
            calls: 1,
            input_tokens: 10,
            output_tokens: 10,
            cost_micros: 0,
            zero_cost_calls: 1,
        });
        let error = budget.reserve("second".into()).unwrap_err();
        assert!(error.contains("token budget 20"), "{error}");
    }

    #[test]
    fn limits_are_frozen_when_the_root_creates_the_file() {
        let (dir, root_budget) = temp_budget(1);
        let _held = root_budget.reserve("held".into()).unwrap();
        let wider = TreeBudget::at(
            dir.path().join("tree.json"),
            TreeLimits {
                max_agents: 8,
                ..TreeLimits::default()
            },
        );
        let error = wider.reserve("second".into()).unwrap_err();
        assert!(error.contains("concurrency limit 1"), "{error}");
    }

    #[test]
    fn dead_reservations_are_pruned() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tree.json");
        let state = TreeState {
            schema_version: 1,
            root_pid: 999_999,
            limits: TreeLimits {
                max_agents: 1,
                max_cost_micros: None,
                max_tokens: None,
            },
            spent_cost_micros: 0,
            spent_tokens: 0,
            spawned: 0,
            active: vec![ActiveSpawn {
                id: "dead".into(),
                owner_pid: 999_999,
                child_pid: Some(999_998),
            }],
        };
        std::fs::write(&path, serde_json::to_vec(&state).unwrap()).unwrap();
        let budget = TreeBudget::at(
            path,
            TreeLimits {
                max_agents: 1,
                ..TreeLimits::default()
            },
        );
        budget.reserve("live".into()).unwrap();
    }
}
