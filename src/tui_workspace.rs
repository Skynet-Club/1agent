use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::tui_session::TuiSession;

const MAX_PRESET_BYTES: u64 = 8 * 1024;
const MAX_PRESET_INSTRUCTION_CHARS: usize = 4_000;
const MAX_COMMAND_OUTPUT: usize = 24_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TuiPreset {
    pub schema_version: u32,
    pub name: String,
    pub instruction: String,
}

impl TuiPreset {
    pub fn new(name: &str, instruction: &str) -> Result<Self, String> {
        let name = name.trim();
        if !valid_preset_name(name) {
            return Err("preset 名称只能包含字母、数字、点、下划线和短横线。".into());
        }
        let instruction = instruction.trim();
        if instruction.is_empty() {
            return Err("preset 指令不能为空。".into());
        }
        if instruction.chars().count() > MAX_PRESET_INSTRUCTION_CHARS
            || instruction
                .chars()
                .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
        {
            return Err("preset 指令包含不安全或过长的内容。".into());
        }
        Ok(Self {
            schema_version: 1,
            name: name.into(),
            instruction: instruction.into(),
        })
    }
}

pub fn preset_root(session: &TuiSession) -> PathBuf {
    session.journal_root().join(".presets")
}

pub fn save_preset(session: &TuiSession, preset: &TuiPreset) -> Result<PathBuf, String> {
    let root = preset_root(session);
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let path = root.join(format!("{}.json", preset.name));
    let mut bytes = serde_json::to_vec_pretty(preset).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    let mut temporary =
        tempfile::NamedTempFile::new_in(&root).map_err(|error| error.to_string())?;
    std::io::Write::write_all(&mut temporary, &bytes).map_err(|error| error.to_string())?;
    temporary
        .persist(&path)
        .map_err(|error| error.error.to_string())?;
    Ok(path)
}

pub fn load_preset(session: &TuiSession, name: &str) -> Result<TuiPreset, String> {
    if !valid_preset_name(name) {
        return Err("preset 名称无效。".into());
    }
    let path = preset_root(session).join(format!("{name}.json"));
    let metadata = std::fs::metadata(&path).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_PRESET_BYTES {
        return Err("preset 文件超过 8 KiB。".into());
    }
    let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
    let preset: TuiPreset = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    if preset.schema_version != 1 || preset.name != name {
        return Err("preset 身份或版本不匹配。".into());
    }
    TuiPreset::new(&preset.name, &preset.instruction)?;
    Ok(preset)
}

pub fn list_presets(session: &TuiSession) -> Result<Vec<TuiPreset>, String> {
    let root = preset_root(session);
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut presets = Vec::new();
    for entry in std::fs::read_dir(root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|name| name.to_str()) else {
            continue;
        };
        if let Ok(preset) = load_preset(session, name) {
            presets.push(preset);
        }
    }
    presets.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(presets)
}

pub fn trash_preset(session: &TuiSession, name: &str) -> Result<PathBuf, String> {
    let path =
        load_preset(session, name).map(|_| preset_root(session).join(format!("{}.json", name)))?;
    let trash = preset_root(session).join(".trash");
    std::fs::create_dir_all(&trash).map_err(|error| error.to_string())?;
    let target = trash.join(format!("{name}-{}.json", unix_ms()));
    std::fs::rename(path, &target).map_err(|error| error.to_string())?;
    Ok(target)
}

pub fn git_diff(workspace: &Path) -> Result<String, String> {
    let status = run_git(workspace, &["status", "--short"])?;
    let stat = run_git(workspace, &["diff", "--stat"])?;
    let diff = run_git(workspace, &["diff"])?;
    Ok(bounded(format!(
        "Git status:\n{}\n\nDiff stat:\n{}\n\nDiff:\n{}",
        status.trim_end(),
        stat.trim_end(),
        diff.trim_end()
    )))
}

pub fn git_worktrees(workspace: &Path) -> Result<String, String> {
    run_git(workspace, &["worktree", "list", "--porcelain"]).map(bounded)
}

pub fn add_git_worktree(workspace: &Path, path: &Path, branch: &str) -> Result<PathBuf, String> {
    if path.exists() {
        return Err(format!("worktree 路径已存在：{}", path.display()));
    }
    let parent = path
        .parent()
        .ok_or_else(|| "worktree 路径没有父目录。".to_string())?;
    if !parent.is_dir() {
        return Err(format!("worktree 父目录不存在：{}", parent.display()));
    }
    if branch.trim().is_empty()
        || branch
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err("worktree branch 名称无效。".into());
    }
    run_git(
        workspace,
        &["worktree", "add", "-b", branch, &path.display().to_string()],
    )?;
    std::fs::canonicalize(path).map_err(|error| error.to_string())
}

pub fn init_agents_file(workspace: &Path) -> Result<PathBuf, String> {
    let path = workspace.join("AGENTS.md");
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                "AGENTS.md 已存在，拒绝覆盖。".into()
            } else {
                error.to_string()
            }
        })?;
    std::io::Write::write_all(
        &mut file,
        b"# Project Instructions\n\n- Describe mandatory build, test and review commands here.\n- Do not overwrite existing project instructions.\n",
    )
    .map_err(|error| error.to_string())?;
    Ok(path)
}

fn run_git(workspace: &Path, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(workspace)
        .output()
        .map_err(|error| format!("无法执行 git：{error}"))?;
    if !output.status.success() {
        return Err(bounded(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    Ok(bounded(
        String::from_utf8_lossy(&output.stdout).into_owned(),
    ))
}

fn valid_preset_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 80
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn bounded(mut text: String) -> String {
    if text.chars().count() > MAX_COMMAND_OUTPUT {
        text = text.chars().take(MAX_COMMAND_OUTPUT).collect();
        text.push_str("\n…（输出已截断）");
    }
    text.replace('\0', "")
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_names_and_instruction_bounds_fail_closed() {
        assert!(TuiPreset::new("../escape", "task").is_err());
        assert!(TuiPreset::new("review", "").is_err());
        assert!(TuiPreset::new("review", &"x".repeat(4_001)).is_err());
        assert!(TuiPreset::new("review", "line one\nline two").is_ok());
    }

    #[test]
    fn agents_template_never_overwrites() {
        let directory = tempfile::tempdir().unwrap();
        let path = init_agents_file(directory.path()).unwrap();
        assert!(path.is_file());
        assert!(init_agents_file(directory.path()).is_err());
    }

    #[test]
    fn presets_round_trip_and_move_to_trash() {
        let directory = tempfile::tempdir().unwrap();
        let session = TuiSession::open(directory.path(), None).unwrap();
        let preset = TuiPreset::new("review", "Review the current patch").unwrap();
        let path = save_preset(&session, &preset).unwrap();
        assert!(path.is_file());
        assert_eq!(load_preset(&session, "review").unwrap(), preset);
        assert_eq!(list_presets(&session).unwrap(), vec![preset]);
        let trashed = trash_preset(&session, "review").unwrap();
        assert!(trashed.is_file());
        assert!(list_presets(&session).unwrap().is_empty());
    }

    #[test]
    fn worktree_path_conflict_is_rejected_before_git_runs() {
        let directory = tempfile::tempdir().unwrap();
        let existing = directory.path().join("existing");
        std::fs::create_dir_all(&existing).unwrap();
        let error = add_git_worktree(directory.path(), &existing, "feature-test").unwrap_err();
        assert!(error.contains("已存在"));
    }
}
