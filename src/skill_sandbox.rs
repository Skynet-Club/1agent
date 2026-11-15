//! Opt-in script sandbox for skill directories.
//!
//! Skills themselves are instruction text and are never auto-executed. When an
//! operator explicitly runs a bundled script, it runs under bubblewrap with no
//! network, no host home, a read-only runtime, a tmpfs `/tmp`, dropped
//! capabilities and a hard timeout. If bwrap is unavailable the command fails
//! closed; there is no unsandboxed fallback.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::AsyncReadExt;

const MAX_SCRIPT_BYTES: u64 = 1024 * 1024;
const DEFAULT_MAX_OUTPUT_BYTES: usize = 1024 * 1024;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Error)]
pub enum ScriptSandboxError {
    #[error("skill sandbox io: {0}")]
    Io(#[from] std::io::Error),
    #[error("bubblewrap (bwrap) is not available; refusing to run a skill script unsandboxed")]
    BwrapUnavailable,
    #[error("unsafe skill script path: {0}")]
    UnsafePath(String),
    #[error("skill script is {0} bytes (limit {MAX_SCRIPT_BYTES})")]
    ScriptTooLarge(u64),
    #[error("unsupported skill script type; use .sh/.bash/.py or an executable")]
    UnsupportedInterpreter,
    #[error("skill script timed out after {0:?}")]
    TimedOut(Duration),
    #[error("skill sandbox failed: {0}")]
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ScriptRunReport {
    pub sandbox: String,
    pub network: bool,
    pub script: String,
    pub command: Vec<String>,
    pub exit_code: Option<i32>,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

pub struct ScriptRunOptions {
    pub timeout: Duration,
    pub max_output_bytes: usize,
}

impl Default for ScriptRunOptions {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
        }
    }
}

/// Run `script` (relative to `<skill_dir>/scripts`) under bwrap. Arguments are
/// passed as a vector, never through a shell.
pub async fn run_skill_script(
    skill_dir: &Path,
    script: &str,
    arguments: &[String],
    options: &ScriptRunOptions,
) -> Result<ScriptRunReport, ScriptSandboxError> {
    if script.is_empty()
        || script.len() > 256
        || script.contains('\0')
        || Path::new(script).is_absolute()
        || Path::new(script)
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(ScriptSandboxError::UnsafePath(script.into()));
    }
    let skill_root = std::fs::canonicalize(skill_dir)?;
    let scripts_root = std::fs::canonicalize(skill_root.join("scripts"))?;
    let script_path = std::fs::canonicalize(scripts_root.join(script))?;
    if !script_path.starts_with(&scripts_root) {
        return Err(ScriptSandboxError::UnsafePath(script.into()));
    }
    let metadata = std::fs::symlink_metadata(&script_path)?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(ScriptSandboxError::UnsafePath(script.into()));
    }
    if metadata.len() > MAX_SCRIPT_BYTES {
        return Err(ScriptSandboxError::ScriptTooLarge(metadata.len()));
    }
    let (program, prefix) = interpreter(&script_path, &metadata)?;
    if arguments.len() > 32 {
        return Err(ScriptSandboxError::UnsafePath(
            "too many script arguments".into(),
        ));
    }
    if arguments.iter().any(|argument| argument.len() > 4096) {
        return Err(ScriptSandboxError::UnsafePath(
            "script argument is too long".into(),
        ));
    }
    check_bwrap().await?;

    let mut command = tokio::process::Command::new(bwrap_program());
    command
        .arg("--die-with-parent")
        .arg("--new-session")
        .arg("--unshare-all")
        .arg("--clearenv")
        .arg("--cap-drop")
        .arg("ALL");
    for path in ["/usr", "/bin", "/sbin", "/lib", "/lib64"] {
        if Path::new(path).exists() {
            command.arg("--ro-bind-try").arg(path).arg(path);
        }
    }
    command
        .arg("--dev")
        .arg("/dev")
        .arg("--proc")
        .arg("/proc")
        .arg("--tmpfs")
        .arg("/tmp")
        .arg("--ro-bind")
        .arg(&skill_root)
        .arg(&skill_root)
        .arg("--chdir")
        .arg("/tmp")
        .arg("--setenv")
        .arg("PATH")
        .arg("/usr/local/bin:/usr/bin:/bin")
        .arg("--setenv")
        .arg("HOME")
        .arg("/tmp")
        .arg("--setenv")
        .arg("LANG")
        .arg("C.UTF-8")
        .arg("--")
        .arg(&program);
    for argument in &prefix {
        command.arg(argument);
    }
    command.arg(&script_path);
    for argument in arguments {
        command.arg(argument);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = command.spawn().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ScriptSandboxError::BwrapUnavailable
        } else {
            ScriptSandboxError::Io(error)
        }
    })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ScriptSandboxError::Failed("sandbox has no stdout".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| ScriptSandboxError::Failed("sandbox has no stderr".into()))?;
    let limit = options.max_output_bytes.max(1024);
    let run = async {
        let (out_result, err_result) =
            tokio::join!(read_capped(stdout, limit), read_capped(stderr, limit),);
        let (out, stdout_truncated) = out_result?;
        let (err, stderr_truncated) = err_result?;
        if stdout_truncated || stderr_truncated {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(ScriptSandboxError::Failed(format!(
                "sandbox output exceeded {limit} bytes"
            )));
        }
        let status = child.wait().await?;
        Ok::<_, ScriptSandboxError>((out, err, status.code()))
    };
    let (stdout_bytes, stderr_bytes, exit_code) = tokio::time::timeout(options.timeout, run)
        .await
        .map_err(|_| ScriptSandboxError::TimedOut(options.timeout))??;
    let stdout = sanitize_output(&stdout_bytes);
    let stderr = sanitize_output(&stderr_bytes);
    let command = vec![
        "bwrap".into(),
        "--unshare-all".into(),
        "--clearenv".into(),
        program.display().to_string(),
        script_path.display().to_string(),
        format!("<{} argument(s) redacted>", arguments.len()),
    ];
    Ok(ScriptRunReport {
        sandbox: "bwrap".into(),
        network: false,
        script: script.into(),
        command,
        exit_code,
        success: exit_code == Some(0),
        stdout,
        stderr,
    })
}

async fn read_capped<R>(mut reader: R, limit: usize) -> Result<(Vec<u8>, bool), ScriptSandboxError>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut output = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            return Ok((output, false));
        }
        if output.len() + read > limit {
            output.extend_from_slice(&buffer[..limit.saturating_sub(output.len())]);
            return Ok((output, true));
        }
        output.extend_from_slice(&buffer[..read]);
    }
}

fn interpreter(
    script_path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(PathBuf, Vec<String>), ScriptSandboxError> {
    let extension = script_path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match extension.as_str() {
        "sh" => Ok((PathBuf::from("/bin/sh"), Vec::new())),
        "bash" => Ok((PathBuf::from("/bin/bash"), Vec::new())),
        "py" => Ok((PathBuf::from("/usr/bin/python3"), Vec::new())),
        _ if is_executable(metadata) => Ok((script_path.to_path_buf(), Vec::new())),
        _ => Err(ScriptSandboxError::UnsupportedInterpreter),
    }
}

fn is_executable(metadata: &std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        false
    }
}

fn bwrap_program() -> PathBuf {
    std::env::var_os("ONEAGENT_BWRAP")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("bwrap"))
}

async fn check_bwrap() -> Result<(), ScriptSandboxError> {
    let status = tokio::process::Command::new(bwrap_program())
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                ScriptSandboxError::BwrapUnavailable
            } else {
                ScriptSandboxError::Io(error)
            }
        })?;
    if !status.success() {
        return Err(ScriptSandboxError::BwrapUnavailable);
    }
    Ok(())
}

fn sanitize_output(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .chars()
        .map(|character| {
            if crate::safety::is_disallowed_control(character) {
                '?'
            } else {
                character
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill_with_script(contents: &str, name: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("SKILL.md"),
            "---\nname: sandbox\ndescription: sandbox test\nversion: 1.0.0\n---\nbody\n",
        )
        .unwrap();
        std::fs::create_dir(dir.path().join("scripts")).unwrap();
        std::fs::write(dir.path().join("scripts").join(name), contents).unwrap();
        dir
    }

    #[tokio::test]
    async fn runs_a_script_without_home_network_or_writes() {
        if check_bwrap().await.is_err() {
            eprintln!("skipping: bwrap is not available");
            return;
        }
        let secret_dir = tempfile::tempdir().unwrap();
        let secret = secret_dir.path().join("secret.txt");
        std::fs::write(&secret, "TOP-SECRET").unwrap();
        let script = format!(
            "#!/bin/sh\necho sandbox-ok\nif [ -e {secret:?} ]; then echo HOME-LEAK; cat {secret:?}; fi\necho write > /tmp/probe && echo tmp-ok\n",
            secret = secret.display().to_string()
        );
        let dir = skill_with_script(&script, "probe.sh");
        let report = run_skill_script(dir.path(), "probe.sh", &[], &ScriptRunOptions::default())
            .await
            .unwrap();
        assert_eq!(report.exit_code, Some(0));
        assert!(report.stdout.contains("sandbox-ok"));
        assert!(!report.stdout.contains("HOME-LEAK"));
        assert!(!report.stdout.contains("TOP-SECRET"));
        assert!(report.stdout.contains("tmp-ok"));
        assert!(!report.network);
    }

    #[tokio::test]
    async fn rejects_symlink_escape_and_path_traversal() {
        #[cfg(unix)]
        {
            let dir = skill_with_script("#!/bin/sh\necho ok\n", "ok.sh");
            std::os::unix::fs::symlink("/etc/passwd", dir.path().join("scripts/escape.sh"))
                .unwrap();
            assert!(matches!(
                run_skill_script(dir.path(), "escape.sh", &[], &ScriptRunOptions::default())
                    .await
                    .unwrap_err(),
                ScriptSandboxError::UnsafePath(_)
            ));
            assert!(matches!(
                run_skill_script(dir.path(), "../SKILL.md", &[], &ScriptRunOptions::default())
                    .await
                    .unwrap_err(),
                ScriptSandboxError::UnsafePath(_)
            ));
        }
    }

    #[tokio::test]
    async fn timeout_kills_a_hung_script() {
        if check_bwrap().await.is_err() {
            eprintln!("skipping: bwrap is not available");
            return;
        }
        let dir = skill_with_script("#!/bin/sh\nsleep 30\n", "sleepy.sh");
        let error = run_skill_script(
            dir.path(),
            "sleepy.sh",
            &[],
            &ScriptRunOptions {
                timeout: Duration::from_millis(500),
                max_output_bytes: 4096,
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(error, ScriptSandboxError::TimedOut(_)), "{error}");
    }

    #[tokio::test]
    async fn output_limit_fails_closed() {
        if check_bwrap().await.is_err() {
            eprintln!("skipping: bwrap is not available");
            return;
        }
        let dir = skill_with_script("#!/bin/sh\nyes x\n", "noisy.sh");
        let error = run_skill_script(
            dir.path(),
            "noisy.sh",
            &[],
            &ScriptRunOptions {
                timeout: Duration::from_secs(5),
                max_output_bytes: 2048,
            },
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("output exceeded"), "{error}");
    }
}
