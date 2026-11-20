//! Typed source reading: the model names a source, the runtime picks the backend.
//!
//! Lark/Feishu document URLs go through `lark-cli docs +fetch`; public HTTPS
//! URLs use `fetch_url`; local files use `read_document`; local directories and
//! knowledge bases use `search_files`. The model never composes CLI flags.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use super::skill_exec::run_readonly_cli;
use super::{AgentTool, FetchUrlTool, ReadDocumentTool, SearchFilesTool, ToolError, ToolOutcome};
use crate::domain::{PermissionSet, RiskLevel, ToolSpec};
use crate::safety::contains_disallowed_control;

const MAX_SOURCE_LEN: usize = 4_096;
const DEFAULT_MAX_CHARS: usize = 64_000;
const MAX_MAX_CHARS: usize = 200_000;
const MAX_QUERY_CHARS: usize = 2_000;
const MAX_LARK_BYTES: usize = 4 * 1024 * 1024;
const MAX_ERROR_CHARS: usize = 2_000;
const TOOL_TIMEOUT: Duration = Duration::from_secs(90);

pub struct ReadSourceTool {
    lark_bin: Option<PathBuf>,
}

impl ReadSourceTool {
    pub fn from_env() -> Self {
        let lark_bin = std::env::var_os("LARK_CLI_BIN")
            .map(PathBuf::from)
            .or_else(|| find_binary("lark-cli"));
        Self { lark_bin }
    }

    pub fn new(lark_bin: Option<PathBuf>) -> Self {
        Self { lark_bin }
    }

    async fn read_lark(&self, url: &str, max_chars: usize) -> Result<ToolOutcome, ToolError> {
        let Some(bin) = &self.lark_bin else {
            return Err(ToolError::Failed(
                "Lark source requires lark-cli; install lark-cli or set LARK_CLI_BIN".into(),
            ));
        };
        let args = vec![
            "docs".into(),
            "+fetch".into(),
            "--doc".into(),
            url.into(),
            "--doc-format".into(),
            "markdown".into(),
        ];
        let output = run_readonly_cli(bin, &args, TOOL_TIMEOUT).await?;
        if output.stdout.len() > MAX_LARK_BYTES {
            return Err(ToolError::Failed(format!(
                "lark-cli output exceeds {MAX_LARK_BYTES} bytes"
            )));
        }
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(ToolError::Failed(format!(
                "lark-cli docs +fetch failed ({}): {}{}",
                output.status,
                bounded(&stderr, MAX_ERROR_CHARS),
                if stdout.trim().is_empty() {
                    String::new()
                } else {
                    format!("; stdout: {}", bounded(&stdout, MAX_ERROR_CHARS))
                }
            )));
        }
        let response: LarkResponse = serde_json::from_slice(&output.stdout).map_err(|error| {
            ToolError::Failed(format!(
                "invalid lark-cli response: {error}; stdout: {}",
                bounded(&String::from_utf8_lossy(&output.stdout), MAX_ERROR_CHARS)
            ))
        })?;
        if !response.ok {
            let error = response
                .error
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown error".into());
            return Err(ToolError::Failed(format!(
                "lark-cli returned an error: {}",
                bounded(&error, MAX_ERROR_CHARS)
            )));
        }
        let document = response
            .data
            .and_then(|data| data.document)
            .ok_or_else(|| ToolError::Failed("lark-cli returned no document content".into()))?;
        if document.content.trim().is_empty() {
            return Err(ToolError::Failed("Lark document is empty".into()));
        }
        let (content, truncated) = bounded_chars(&document.content, max_chars);
        let mut metadata = BTreeMap::new();
        metadata.insert("source_kind".into(), "lark".into());
        if let Some(document_id) = document.document_id {
            metadata.insert("document_id".into(), document_id);
        }
        if let Some(revision_id) = document.revision_id {
            metadata.insert("revision_id".into(), revision_id.to_string());
        }
        metadata.insert("truncated".into(), truncated.to_string());
        Ok(ToolOutcome {
            summary: "read Lark document via lark-cli".into(),
            content,
            metadata,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadSourceArgs {
    source: String,
    #[serde(default)]
    query: String,
    #[serde(default = "default_max_chars")]
    max_chars: usize,
}

fn default_max_chars() -> usize {
    DEFAULT_MAX_CHARS
}

#[derive(Debug, Deserialize)]
struct LarkResponse {
    #[serde(default)]
    ok: bool,
    #[serde(default)]
    data: Option<LarkData>,
    #[serde(default)]
    error: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct LarkData {
    #[serde(default)]
    document: Option<LarkDocument>,
}

#[derive(Debug, Deserialize)]
struct LarkDocument {
    #[serde(default)]
    content: String,
    #[serde(default)]
    document_id: Option<String>,
    #[serde(default)]
    revision_id: Option<u64>,
}

#[async_trait]
impl AgentTool for ReadSourceTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "read_source".into(),
            description: "Read a source named by the task. The runtime chooses the backend: Lark/Feishu document URL -> lark-cli; public https URL -> fetch_url; local file -> read_document; local directory or knowledge base -> search_files. Pass query to search a directory. Remote reads require --allow-network. Use this instead of composing CLI flags.".into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["source"],
                "properties": {
                    "source": {"type": "string", "maxLength": MAX_SOURCE_LEN},
                    "query": {"type": "string", "maxLength": MAX_QUERY_CHARS},
                    "max_chars": {
                        "type": "integer",
                        "default": DEFAULT_MAX_CHARS
                    }
                }
            }),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::Medium
    }

    fn executes(&self) -> bool {
        true
    }

    fn timeout(&self) -> Duration {
        TOOL_TIMEOUT
    }

    async fn execute(
        &self,
        arguments: Value,
        permissions: &PermissionSet,
        cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        let args: ReadSourceArgs = serde_json::from_value(arguments)
            .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
        let source = args.source.trim();
        if source.is_empty()
            || source.chars().count() > MAX_SOURCE_LEN
            || contains_disallowed_control(source)
        {
            return Err(ToolError::InvalidArguments(
                "source must be a non-empty path or URL without control characters".into(),
            ));
        }
        if args.query.chars().count() > MAX_QUERY_CHARS || contains_disallowed_control(&args.query)
        {
            return Err(ToolError::InvalidArguments(
                "query is too long or contains control characters".into(),
            ));
        }
        let max_chars = args.max_chars.clamp(1_000, MAX_MAX_CHARS);

        if source.starts_with("http://") || source.starts_with("https://") {
            if !permissions.allow_network {
                return Err(ToolError::PermissionDenied(
                    "remote source reads need network permission; pass --allow-network".into(),
                ));
            }
            if is_lark_url(source) {
                return self.read_lark(source, max_chars).await;
            }
            return FetchUrlTool
                .execute(
                    json!({"url": source, "max_chars": max_chars}),
                    permissions,
                    cancellation,
                )
                .await;
        }

        let path = local_path(source, permissions)?;
        let canonical = path.canonicalize().map_err(|_| {
            ToolError::InvalidArguments(format!("source not found: {}", path.display()))
        })?;
        if canonical.is_dir() {
            let outcome = SearchFilesTool
                .execute(
                    json!({
                        "path": canonical,
                        "query": args.query,
                        "max_results": 20
                    }),
                    permissions,
                    cancellation,
                )
                .await?;
            Ok(with_source_kind(outcome, "directory"))
        } else {
            let outcome = ReadDocumentTool::from_env()
                .execute(
                    json!({"path": canonical, "max_chars": max_chars}),
                    permissions,
                    cancellation,
                )
                .await?;
            Ok(with_source_kind(outcome, "local_file"))
        }
    }
}

fn with_source_kind(mut outcome: ToolOutcome, kind: &str) -> ToolOutcome {
    outcome
        .metadata
        .insert("source_kind".into(), kind.to_string());
    outcome
}

fn local_path(raw: &str, permissions: &PermissionSet) -> Result<PathBuf, ToolError> {
    let path = if raw == "~" {
        home().ok_or_else(|| ToolError::InvalidArguments("HOME is not set".into()))?
    } else if let Some(rest) = raw.strip_prefix("~/") {
        home()
            .ok_or_else(|| ToolError::InvalidArguments("HOME is not set".into()))?
            .join(rest)
    } else {
        PathBuf::from(raw)
    };
    if path.is_absolute() {
        return Ok(path);
    }
    let base = permissions
        .read_roots
        .first()
        .cloned()
        .unwrap_or_else(|| PathBuf::from("."));
    Ok(base.join(path))
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn find_binary(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

fn is_lark_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    let host = [
        ".feishu.cn/",
        ".larksuite.com/",
        ".larkoffice.com/",
        ".doubao.com/",
    ]
    .iter()
    .any(|suffix| lower.contains(suffix));
    host && (lower.contains("/docx/") || lower.contains("/wiki/"))
}

fn bounded(value: &str, max_chars: usize) -> String {
    bounded_chars(value, max_chars).0
}

fn bounded_chars(value: &str, max_chars: usize) -> (String, bool) {
    let mut bounded: String = value.chars().take(max_chars).collect();
    let truncated = value.chars().nth(max_chars).is_some();
    if truncated {
        bounded.push('…');
    }
    (bounded, truncated)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    fn permissions(root: &Path, allow_network: bool) -> PermissionSet {
        PermissionSet {
            read_roots: vec![root.to_path_buf()],
            allow_network,
            allow_write: false,
        }
    }

    fn script(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("lark-cli");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).unwrap();
        path
    }

    #[tokio::test]
    async fn reads_a_local_file_inside_a_read_root() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("note.md");
        std::fs::write(&file, "hello source\n").unwrap();
        let outcome = ReadSourceTool::new(None)
            .execute(
                json!({"source": file}),
                &permissions(dir.path(), false),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(outcome.content.contains("hello source"));
        assert_eq!(outcome.metadata.get("source_kind").unwrap(), "local_file");
    }

    #[tokio::test]
    async fn searches_a_directory_source() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("note.md"), "knowledge\n").unwrap();
        let outcome = ReadSourceTool::new(None)
            .execute(
                json!({"source": dir.path(), "query": "knowledge"}),
                &permissions(dir.path(), false),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(outcome.content.contains("note.md"));
        assert_eq!(outcome.metadata.get("source_kind").unwrap(), "directory");
    }

    #[tokio::test]
    async fn rejects_a_local_file_outside_the_read_roots() {
        let allowed = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let file = other.path().join("note.md");
        std::fs::write(&file, "secret\n").unwrap();
        let error = ReadSourceTool::new(None)
            .execute(
                json!({"source": file}),
                &permissions(allowed.path(), false),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            ToolError::PermissionDenied(_) | ToolError::Failed(_)
        ));
    }

    #[tokio::test]
    async fn remote_sources_require_network_and_lark_cli() {
        let dir = tempfile::tempdir().unwrap();
        let tool = ReadSourceTool::new(None);
        let denied = tool
            .execute(
                json!({"source": "https://x.feishu.cn/docx/abc"}),
                &permissions(dir.path(), false),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(denied, ToolError::PermissionDenied(_)));

        let missing = tool
            .execute(
                json!({"source": "https://x.feishu.cn/docx/abc"}),
                &permissions(dir.path(), true),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(missing, ToolError::Failed(_)));
    }

    #[tokio::test]
    async fn extracts_content_from_a_lark_cli_response() {
        let dir = tempfile::tempdir().unwrap();
        let bin = script(
            dir.path(),
            "printf '%s' '{\"ok\":true,\"data\":{\"document\":{\"content\":\"# Lark\\nbody\",\"document_id\":\"doc1\",\"revision_id\":8}}}'",
        );
        let outcome = ReadSourceTool::new(Some(bin))
            .execute(
                json!({"source": "https://x.feishu.cn/docx/doc1", "max_chars": 1_000_000}),
                &permissions(dir.path(), true),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(outcome.content.contains("# Lark"));
        assert_eq!(outcome.metadata.get("document_id").unwrap(), "doc1");
        assert_eq!(outcome.metadata.get("revision_id").unwrap(), "8");
    }

    #[test]
    fn classifies_lark_and_public_urls() {
        assert!(is_lark_url("https://x.feishu.cn/docx/abc"));
        assert!(is_lark_url("https://x.larksuite.com/wiki/abc"));
        assert!(!is_lark_url("https://example.com/docx/abc"));
        assert!(!is_lark_url("https://x.feishu.cn/base/abc"));
    }
}
