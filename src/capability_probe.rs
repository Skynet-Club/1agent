use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const SCHEMA_VERSION: u32 = 1;
const MAX_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_TIMEOUT_SECONDS: u64 = 300;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CapabilityProbeManifest {
    pub schema_version: u32,
    pub suite_id: String,
    pub tasks: Vec<CapabilityTask>,
    pub runtimes: Vec<CapabilityRuntime>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CapabilityTask {
    pub id: String,
    pub prompt: String,
    pub timeout_seconds: u64,
    pub success_rule: ProbeSuccessRule,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProbeSuccessRule {
    Exact { expected: String },
    Contains { expected: String },
    JsonObject,
    JsonPathEquals { path: String, expected: String },
    JsonPathJsonObject { path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CapabilityRuntime {
    pub runtime: String,
    pub command: Vec<String>,
    pub prompt_delivery: PromptDelivery,
    #[serde(default)]
    pub blocked_patterns: Vec<String>,
    #[serde(default)]
    pub success_rule_overrides: BTreeMap<String, ProbeSuccessRule>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PromptDelivery {
    Arg,
    Stdin,
    None,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProbeStatus {
    Passed,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProbeFailureKind {
    WrongExit,
    WrongOutput,
    Timeout,
    BlockedAuthentication,
    BlockedNetwork,
    BlockedFilesystem,
    SpawnError,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CapabilityProbeResult {
    pub runtime: String,
    pub command_sha256: String,
    pub task_id: String,
    pub status: ProbeStatus,
    #[serde(default)]
    pub failure_kind: Option<ProbeFailureKind>,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    pub stdout_sha256: String,
    pub stderr_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CapabilityProbeReport {
    pub schema_version: u32,
    pub suite_id: String,
    pub agent_version: String,
    pub environment: String,
    pub manifest_sha256: String,
    pub generated_unix_ms: u128,
    pub results: Vec<CapabilityProbeResult>,
}

impl CapabilityProbeManifest {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(format!(
                "unsupported capability manifest version {}",
                self.schema_version
            ));
        }
        validate_name("suite_id", &self.suite_id)?;
        if self.tasks.is_empty() || self.tasks.len() > 32 {
            return Err("capability suite must contain 1..=32 tasks".into());
        }
        if self.runtimes.is_empty() || self.runtimes.len() > 16 {
            return Err("capability suite must contain 1..=16 runtimes".into());
        }
        for task in &self.tasks {
            validate_name("task id", &task.id)?;
            if task.prompt.chars().count() > 4_000
                || task
                    .prompt
                    .chars()
                    .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
            {
                return Err(format!("task '{}' prompt is unsafe or too large", task.id));
            }
            if !(1..=MAX_TIMEOUT_SECONDS).contains(&task.timeout_seconds) {
                return Err(format!(
                    "task '{}' timeout must be 1..={MAX_TIMEOUT_SECONDS}",
                    task.id
                ));
            }
            match &task.success_rule {
                ProbeSuccessRule::Exact { expected } | ProbeSuccessRule::Contains { expected } => {
                    if expected.chars().count() > 8_192 {
                        return Err(format!("task '{}' expected output is too large", task.id));
                    }
                }
                ProbeSuccessRule::JsonObject => {}
                ProbeSuccessRule::JsonPathEquals { path, expected } => {
                    validate_json_path(path)?;
                    if expected.chars().count() > 8_192 {
                        return Err(format!("task '{}' expected output is too large", task.id));
                    }
                }
                ProbeSuccessRule::JsonPathJsonObject { path } => validate_json_path(path)?,
            }
        }
        for runtime in &self.runtimes {
            validate_name("runtime", &runtime.runtime)?;
            if runtime.command.is_empty() || runtime.command.len() > 64 {
                return Err(format!(
                    "runtime '{}' command must contain 1..=64 arguments",
                    runtime.runtime
                ));
            }
            if runtime.command.iter().any(|argument| {
                argument.contains('\0')
                    || argument
                        .chars()
                        .any(|character| character.is_control() && !matches!(character, '\n'))
            }) {
                return Err(format!(
                    "runtime '{}' command contains unsafe characters",
                    runtime.runtime
                ));
            }
            if runtime.blocked_patterns.len() > 32
                || runtime
                    .blocked_patterns
                    .iter()
                    .any(|pattern| pattern.trim().is_empty() || pattern.len() > 128)
            {
                return Err(format!(
                    "runtime '{}' blocked patterns are invalid",
                    runtime.runtime
                ));
            }
            if runtime
                .success_rule_overrides
                .keys()
                .any(|task_id| !self.tasks.iter().any(|task| task.id == *task_id))
            {
                return Err(format!(
                    "runtime '{}' overrides an unknown task",
                    runtime.runtime
                ));
            }
        }
        Ok(())
    }
}

pub fn filter_capability_manifest_by_runtime(
    mut manifest: CapabilityProbeManifest,
    runtime_names: &[String],
) -> Result<CapabilityProbeManifest, String> {
    if runtime_names.is_empty() {
        return Ok(manifest);
    }
    for runtime in runtime_names {
        if !manifest
            .runtimes
            .iter()
            .any(|candidate| candidate.runtime == *runtime)
        {
            return Err(format!("unknown probe runtime '{runtime}'"));
        }
    }
    manifest
        .runtimes
        .retain(|runtime| runtime_names.contains(&runtime.runtime));
    manifest.validate()?;
    Ok(manifest)
}

pub fn run_capability_suite(
    manifest: &CapabilityProbeManifest,
) -> Result<CapabilityProbeReport, String> {
    manifest.validate()?;
    let mut results = Vec::new();
    for runtime in &manifest.runtimes {
        for task in &manifest.tasks {
            results.push(run_probe(runtime, task));
        }
    }
    let manifest_sha256 = serde_json::to_vec(manifest)
        .map(|bytes| hex::encode(Sha256::digest(bytes)))
        .map_err(|error| error.to_string())?;
    Ok(CapabilityProbeReport {
        schema_version: SCHEMA_VERSION,
        suite_id: manifest.suite_id.clone(),
        agent_version: env!("CARGO_PKG_VERSION").into(),
        environment: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        manifest_sha256,
        generated_unix_ms: unix_ms(),
        results,
    })
}

pub fn write_capability_report(report: &CapabilityProbeReport, path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "capability report has no parent directory".to_string())?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec_pretty(report).map_err(|error| error.to_string())?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    temporary
        .write_all(&bytes)
        .and_then(|_| temporary.write_all(b"\n"))
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|error| error.to_string())?;
    temporary
        .persist(path)
        .map_err(|error| error.error.to_string())?;
    Ok(())
}

pub fn render_capability_report(report: &CapabilityProbeReport) -> String {
    let mut output = format!(
        "Capability probe · suite={} · runs={}",
        report.suite_id,
        report.results.len()
    );
    for result in &report.results {
        output.push_str(&format!(
            "\n- {} / {} · {:?} · exit={:?} · duration={}ms",
            result.runtime, result.task_id, result.status, result.exit_code, result.duration_ms
        ));
        if let Some(kind) = result.failure_kind {
            output.push_str(&format!(" · reason={kind:?}"));
        }
    }
    output
}

fn run_probe(runtime: &CapabilityRuntime, task: &CapabilityTask) -> CapabilityProbeResult {
    let started = Instant::now();
    let mut command = Command::new(&runtime.command[0]);
    command
        .args(&runtime.command[1..])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if runtime.prompt_delivery == PromptDelivery::Arg {
        command.arg(&task.prompt);
    }
    if runtime.prompt_delivery == PromptDelivery::Stdin {
        command.stdin(Stdio::piped());
    } else {
        command.stdin(Stdio::null());
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => {
            return empty_result(
                runtime,
                task,
                ProbeStatus::Blocked,
                ProbeFailureKind::SpawnError,
                started.elapsed(),
            );
        }
    };
    if runtime.prompt_delivery == PromptDelivery::Stdin
        && let Some(mut stdin) = child.stdin.take()
    {
        let _ = stdin.write_all(task.prompt.as_bytes());
    }
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_reader = stdout.map(|stdout| std::thread::spawn(move || read_bounded(stdout)));
    let stderr_reader = stderr.map(|stderr| std::thread::spawn(move || read_bounded(stderr)));
    let deadline = started + Duration::from_secs(task.timeout_seconds);
    let timed_out = loop {
        match child.try_wait() {
            Ok(Some(_)) => break false,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                break true;
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                break false;
            }
        }
    };
    let status = child.wait().ok();
    let stdout = stdout_reader
        .and_then(|reader| reader.join().ok())
        .unwrap_or_default();
    let stderr = stderr_reader
        .and_then(|reader| reader.join().ok())
        .unwrap_or_default();
    let duration = started.elapsed();
    let exit_code = status.and_then(|status| status.code());
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&stdout),
        String::from_utf8_lossy(&stderr)
    )
    .to_ascii_lowercase();
    let (probe_status, failure_kind) = if timed_out {
        (ProbeStatus::Blocked, Some(ProbeFailureKind::Timeout))
    } else if runtime
        .blocked_patterns
        .iter()
        .any(|pattern| combined.contains(&pattern.to_ascii_lowercase()))
    {
        let kind = if combined.contains("network")
            || combined.contains("connection")
            || combined.contains("dns")
        {
            ProbeFailureKind::BlockedNetwork
        } else if combined.contains("read-only file system") {
            ProbeFailureKind::BlockedFilesystem
        } else {
            ProbeFailureKind::BlockedAuthentication
        };
        (ProbeStatus::Blocked, Some(kind))
    } else if exit_code != Some(0) {
        (ProbeStatus::Failed, Some(ProbeFailureKind::WrongExit))
    } else if success_rule_matches(
        runtime
            .success_rule_overrides
            .get(&task.id)
            .unwrap_or(&task.success_rule),
        &stdout,
    ) {
        (ProbeStatus::Passed, None)
    } else {
        (ProbeStatus::Failed, Some(ProbeFailureKind::WrongOutput))
    };
    CapabilityProbeResult {
        runtime: runtime.runtime.clone(),
        command_sha256: command_sha256(runtime),
        task_id: task.id.clone(),
        status: probe_status,
        failure_kind,
        exit_code,
        duration_ms: duration.as_millis().min(u128::from(u64::MAX)) as u64,
        stdout_bytes: stdout.len() as u64,
        stderr_bytes: stderr.len() as u64,
        stdout_sha256: hex::encode(Sha256::digest(&stdout)),
        stderr_sha256: hex::encode(Sha256::digest(&stderr)),
    }
}

fn success_rule_matches(rule: &ProbeSuccessRule, stdout: &[u8]) -> bool {
    let text = String::from_utf8_lossy(stdout);
    match rule {
        ProbeSuccessRule::Exact { expected } => text.trim() == expected.trim(),
        ProbeSuccessRule::Contains { expected } => text.contains(expected),
        ProbeSuccessRule::JsonObject => serde_json::from_str::<serde_json::Value>(text.trim())
            .is_ok_and(|value| value.is_object()),
        ProbeSuccessRule::JsonPathEquals { path, expected } => {
            let Ok(value) = serde_json::from_str::<serde_json::Value>(text.trim()) else {
                return false;
            };
            json_path_value(&value, path)
                .and_then(serde_json::Value::as_str)
                .is_some_and(|value| value == expected)
        }
        ProbeSuccessRule::JsonPathJsonObject { path } => {
            let Ok(value) = serde_json::from_str::<serde_json::Value>(text.trim()) else {
                return false;
            };
            json_path_value(&value, path).is_some_and(|value| match value {
                serde_json::Value::Object(_) => true,
                serde_json::Value::String(text) => serde_json::from_str::<serde_json::Value>(text)
                    .is_ok_and(|value| value.is_object()),
                _ => false,
            })
        }
    }
}

fn json_path_value<'a>(value: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    path.split('.')
        .try_fold(value, |current, segment| current.get(segment))
}

fn empty_result(
    runtime: &CapabilityRuntime,
    task: &CapabilityTask,
    status: ProbeStatus,
    failure_kind: ProbeFailureKind,
    duration: Duration,
) -> CapabilityProbeResult {
    CapabilityProbeResult {
        runtime: runtime.runtime.clone(),
        command_sha256: command_sha256(runtime),
        task_id: task.id.clone(),
        status,
        failure_kind: Some(failure_kind),
        exit_code: None,
        duration_ms: duration.as_millis().min(u128::from(u64::MAX)) as u64,
        stdout_bytes: 0,
        stderr_bytes: 0,
        stdout_sha256: hex::encode(Sha256::digest([])),
        stderr_sha256: hex::encode(Sha256::digest([])),
    }
}

fn command_sha256(runtime: &CapabilityRuntime) -> String {
    hex::encode(Sha256::digest(runtime.command.join("\0").as_bytes()))
}

fn read_bounded(mut reader: impl Read) -> Vec<u8> {
    let mut collected = Vec::new();
    let mut buffer = [0u8; 8 * 1024];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => {
                let remaining = MAX_OUTPUT_BYTES.saturating_sub(collected.len());
                collected.extend_from_slice(&buffer[..read.min(remaining)]);
            }
        }
    }
    collected
}

fn validate_name(label: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        Err(format!("{label} is empty, unsafe or too long"))
    } else {
        Ok(())
    }
}

fn validate_json_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.len() > 256
        || path
            .split('.')
            .any(|segment| segment.is_empty() || segment.chars().any(char::is_control))
    {
        Err(format!("invalid JSON probe path '{path}'"))
    } else {
        Ok(())
    }
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

    fn manifest(command: Vec<String>, blocked_patterns: Vec<String>) -> CapabilityProbeManifest {
        CapabilityProbeManifest {
            schema_version: 1,
            suite_id: "suite-1".into(),
            tasks: vec![CapabilityTask {
                id: "exact".into(),
                prompt: "OK".into(),
                timeout_seconds: 5,
                success_rule: ProbeSuccessRule::Exact {
                    expected: "OK".into(),
                },
            }],
            runtimes: vec![CapabilityRuntime {
                runtime: "fake".into(),
                command,
                prompt_delivery: PromptDelivery::Arg,
                blocked_patterns,
                success_rule_overrides: BTreeMap::new(),
            }],
        }
    }

    #[cfg(unix)]
    #[test]
    fn capability_probe_records_pass_fail_and_blocked_without_raw_output() {
        let passed = run_capability_suite(&manifest(vec!["/bin/echo".into()], Vec::new())).unwrap();
        assert_eq!(passed.results[0].status, ProbeStatus::Passed);
        assert!(!serde_json::to_string(&passed).unwrap().contains("\"OK\""));

        let failed = run_capability_suite(&manifest(
            vec!["/bin/echo".into(), "wrong".into()],
            Vec::new(),
        ))
        .unwrap();
        assert_eq!(failed.results[0].status, ProbeStatus::Failed);

        let blocked = run_capability_suite(&manifest(
            vec![
                "/bin/sh".into(),
                "-c".into(),
                "echo authentication required >&2; exit 1".into(),
            ],
            vec!["authentication required".into()],
        ))
        .unwrap();
        assert_eq!(blocked.results[0].status, ProbeStatus::Blocked);
        assert_eq!(
            blocked.results[0].failure_kind,
            Some(ProbeFailureKind::BlockedAuthentication)
        );
    }

    #[test]
    fn json_path_rules_inspect_nested_provider_results() {
        let stdout = br#"{"answer":{"conclusion":"OK"},"nested":{"value":"{\"ok\":true}"}}"#;
        assert!(success_rule_matches(
            &ProbeSuccessRule::JsonPathEquals {
                path: "answer.conclusion".into(),
                expected: "OK".into(),
            },
            stdout
        ));
        assert!(success_rule_matches(
            &ProbeSuccessRule::JsonPathJsonObject {
                path: "nested.value".into(),
            },
            stdout
        ));
    }

    #[cfg(unix)]
    #[test]
    fn filesystem_block_is_not_misclassified_as_authentication() {
        let blocked = run_capability_suite(&manifest(
            vec![
                "/bin/sh".into(),
                "-c".into(),
                "echo read-only file system >&2; exit 1".into(),
            ],
            vec!["read-only file system".into()],
        ))
        .unwrap();
        assert_eq!(
            blocked.results[0].failure_kind,
            Some(ProbeFailureKind::BlockedFilesystem)
        );
    }

    #[test]
    fn runtime_filter_rejects_unknown_names_and_keeps_requested_runtime() {
        let mut manifest = manifest(vec!["/bin/echo".into()], Vec::new());
        manifest.runtimes.push(CapabilityRuntime {
            runtime: "other".into(),
            command: vec!["/bin/echo".into()],
            prompt_delivery: PromptDelivery::Arg,
            blocked_patterns: Vec::new(),
            success_rule_overrides: BTreeMap::new(),
        });
        let filtered =
            filter_capability_manifest_by_runtime(manifest.clone(), &["other".into()]).unwrap();
        assert_eq!(filtered.runtimes.len(), 1);
        assert_eq!(filtered.runtimes[0].runtime, "other");
        assert!(filter_capability_manifest_by_runtime(manifest, &["missing".into()]).is_err());
    }
}
