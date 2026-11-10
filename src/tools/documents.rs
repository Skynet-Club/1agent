//! Local document text extraction. PDF support calls the system `pdftotext`
//! (poppler-utils); the binary is discovered at tool construction so tests can
//! inject a fake extractor.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;

use crate::domain::{PermissionSet, RiskLevel, ToolSpec};
use crate::safety::contains_disallowed_control;

use super::{AgentTool, ToolError, ToolOutcome, find_containing_root};

const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

pub struct PdfReadTool {
    binary: Arc<PathBuf>,
}

impl PdfReadTool {
    pub fn new(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: Arc::new(binary.into()),
        }
    }

    pub fn from_env() -> Self {
        let binary = std::env::var_os("PDFTOTEXT_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("pdftotext"));
        Self::new(binary)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PdfReadArgs {
    path: String,
    #[serde(default = "default_max_chars")]
    max_chars: usize,
}

fn default_max_chars() -> usize {
    20_000
}

#[async_trait]
impl AgentTool for PdfReadTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "read_pdf".into(),
            description: "Extract text from a local PDF inside an allowed read root.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["path"],
                "properties": {
                    "path": {"type": "string"},
                    "max_chars": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 100000,
                        "default": 20000
                    }
                }
            }),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::Medium
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(40)
    }

    async fn execute(
        &self,
        arguments: Value,
        permissions: &PermissionSet,
        cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        let args: PdfReadArgs = serde_json::from_value(arguments)
            .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
        if args.max_chars == 0 || args.max_chars > 100_000 {
            return Err(ToolError::InvalidArguments(
                "max_chars must be between 1 and 100000".into(),
            ));
        }
        if permissions.read_roots.is_empty() {
            return Err(ToolError::PermissionDenied(
                "no read roots configured".into(),
            ));
        }
        let requested = PathBuf::from(&args.path);
        let requested = if requested.is_absolute() {
            requested
        } else {
            permissions.read_roots[0].join(requested)
        };
        let requested_metadata = tokio::fs::symlink_metadata(&requested)
            .await
            .map_err(|error| ToolError::Failed(format!("cannot inspect file path: {error}")))?;
        if requested_metadata.file_type().is_symlink() {
            return Err(ToolError::PermissionDenied(
                "file path must not be a symbolic link".into(),
            ));
        }
        let canonical = tokio::fs::canonicalize(&requested)
            .await
            .map_err(|error| ToolError::Failed(format!("cannot resolve file: {error}")))?;
        let root = find_containing_root(&canonical, &permissions.read_roots).ok_or_else(|| {
            ToolError::PermissionDenied("path is outside configured read roots".into())
        })?;
        if !canonical.is_file() {
            return Err(ToolError::Failed("path is not a regular file".into()));
        }
        if !canonical
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
        {
            return Err(ToolError::InvalidArguments(
                "read_pdf only accepts .pdf files".into(),
            ));
        }
        if cancellation.is_cancelled() {
            return Err(ToolError::Cancelled);
        }

        let mut command = tokio::process::Command::new(self.binary.as_ref());
        command
            .args(["-layout", "-enc", "UTF-8"])
            .arg(&canonical)
            .arg("-")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        // A concurrently forked child can briefly hold a freshly written fake
        // extractor open for writing; ETXTBSY clears once that child execs.
        let mut attempts = 0;
        let mut child = loop {
            match command.spawn() {
                Ok(child) => break child,
                Err(error)
                    if error.kind() == std::io::ErrorKind::ExecutableFileBusy && attempts < 5 =>
                {
                    attempts += 1;
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                Err(error) => {
                    return Err(ToolError::Failed(format!(
                        "read_pdf requires pdftotext (poppler-utils) at {}: {error}",
                        self.binary.display()
                    )));
                }
            }
        };
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ToolError::Failed("cannot capture pdftotext output".into()))?;
        let mut limited = stdout.take(MAX_OUTPUT_BYTES as u64);
        let mut bytes = Vec::new();
        limited
            .read_to_end(&mut bytes)
            .await
            .map_err(|error| ToolError::Failed(format!("cannot read pdftotext output: {error}")))?;
        let output_truncated = bytes.len() >= MAX_OUTPUT_BYTES;
        drop(limited);
        if output_truncated {
            let _ = child.start_kill();
        }
        let status = child
            .wait()
            .await
            .map_err(|error| ToolError::Failed(format!("pdftotext failed: {error}")))?;
        if !status.success() && !output_truncated {
            return Err(ToolError::Failed(format!(
                "pdftotext exited with status {}",
                status.code().unwrap_or(-1)
            )));
        }
        if cancellation.is_cancelled() {
            return Err(ToolError::Cancelled);
        }

        // pdftotext separates pages with a form feed; surface it as a blank
        // line so the normal control-character gate still applies.
        let text = String::from_utf8_lossy(&bytes).replace('\u{000C}', "\n\n");
        if contains_disallowed_control(&text) {
            return Err(ToolError::UnsafeContent(
                "PDF text contains hidden or bidirectional control characters".into(),
            ));
        }
        let char_count = text.chars().count();
        let truncated = output_truncated || char_count > args.max_chars;
        let content = if char_count > args.max_chars {
            text.chars().take(args.max_chars).collect()
        } else {
            text
        };
        let relative = canonical
            .strip_prefix(&root)
            .unwrap_or(canonical.as_path())
            .to_string_lossy()
            .to_string();
        let mut metadata = BTreeMap::new();
        metadata.insert("relative_path".into(), relative.clone());
        metadata.insert("chars".into(), content.chars().count().to_string());
        metadata.insert("truncated".into(), truncated.to_string());
        Ok(ToolOutcome {
            summary: format!(
                "extracted {} characters from {relative}",
                content.chars().count()
            ),
            content,
            metadata,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    fn fake_pdftotext(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("pdftotext");
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(file, "#!/bin/sh\nprintf '%b' '{}'", body).unwrap();
        drop(file);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn permissions(root: &Path) -> PermissionSet {
        PermissionSet {
            read_roots: vec![root.to_path_buf()],
            allow_network: false,
            allow_write: false,
        }
    }

    fn pdf(root: &Path) -> PathBuf {
        let path = root.join("doc.pdf");
        std::fs::write(&path, b"%PDF-1.4 fake").unwrap();
        path
    }

    #[tokio::test]
    async fn extracts_text_with_a_fake_pdftotext() {
        let dir = tempfile::tempdir().unwrap();
        let binary = fake_pdftotext(dir.path(), "first line\\nsecond line");
        let document = pdf(dir.path());
        let tool = PdfReadTool::new(binary);
        let outcome = tool
            .execute(
                serde_json::json!({"path": document.display().to_string()}),
                &permissions(dir.path()),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(outcome.content, "first line\nsecond line");
        assert_eq!(outcome.metadata["truncated"], "false");
    }

    #[tokio::test]
    async fn truncates_by_chars() {
        let dir = tempfile::tempdir().unwrap();
        let binary = fake_pdftotext(dir.path(), "abcdef");
        let document = pdf(dir.path());
        let tool = PdfReadTool::new(binary);
        let outcome = tool
            .execute(
                serde_json::json!({"path": document.display().to_string(), "max_chars": 3}),
                &permissions(dir.path()),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(outcome.content, "abc");
        assert_eq!(outcome.metadata["truncated"], "true");
    }

    #[tokio::test]
    async fn rejects_paths_outside_roots_symlinks_and_non_pdf_files() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let binary = fake_pdftotext(root.path(), "text");
        let tool = PdfReadTool::new(binary);
        let outside_pdf = outside.path().join("outside.pdf");
        std::fs::write(&outside_pdf, b"%PDF").unwrap();
        let error = tool
            .execute(
                serde_json::json!({"path": outside_pdf.display().to_string()}),
                &permissions(root.path()),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::PermissionDenied(_)));

        let target = pdf(root.path());
        let link = root.path().join("link.pdf");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let error = tool
            .execute(
                serde_json::json!({"path": link.display().to_string()}),
                &permissions(root.path()),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::PermissionDenied(_)));

        let text = root.path().join("note.txt");
        std::fs::write(&text, b"text").unwrap();
        let error = tool
            .execute(
                serde_json::json!({"path": text.display().to_string()}),
                &permissions(root.path()),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::InvalidArguments(_)));
    }

    #[tokio::test]
    async fn missing_extractor_and_hidden_controls_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let document = pdf(dir.path());
        let tool = PdfReadTool::new(dir.path().join("does-not-exist"));
        let error = tool
            .execute(
                serde_json::json!({"path": document.display().to_string()}),
                &permissions(dir.path()),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::Failed(_)));

        let binary = fake_pdftotext(dir.path(), "\\033[31mred");
        let tool = PdfReadTool::new(binary);
        let error = tool
            .execute(
                serde_json::json!({"path": document.display().to_string()}),
                &permissions(dir.path()),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(error, ToolError::UnsafeContent(_)),
            "unexpected error: {error:?}"
        );
    }
}
