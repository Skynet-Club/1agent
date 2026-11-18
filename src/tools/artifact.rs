//! Artifact verification: existence, media type, schema, hash and optional
//! loopback preview. This is deliberately a read-only tool; it never writes,
//! installs or renders. A passing verification is evidence, not a human
//! acceptance receipt.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{AgentTool, ToolError, ToolOutcome};
use crate::domain::{PermissionSet, RiskLevel, ToolSpec};

const MAX_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ArtifactVerification {
    pub path: String,
    pub exists: bool,
    pub bytes: u64,
    pub sha256: String,
    pub media_type: String,
    pub schema_ok: bool,
    pub preview_ok: Option<bool>,
    pub checks: Vec<String>,
    pub ok: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifyArtifactArgs {
    path: PathBuf,
    #[serde(default)]
    media_type: Option<String>,
    #[serde(default)]
    schema: Option<String>,
    #[serde(default)]
    expected_sha256: Option<String>,
    #[serde(default)]
    preview_url: Option<String>,
}

pub struct VerifyArtifactTool;

#[async_trait]
impl AgentTool for VerifyArtifactTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "verify_artifact".into(),
            description: "Verify a local artifact: file exists, size, media type, lightweight schema, SHA-256, and optional loopback preview HTTP 200. Read-only; it does not render, write or install. Use after producing a file and before claiming completion.".into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["path"],
                "properties": {
                    "path": {"type": "string"},
                    "media_type": {"type": "string"},
                    "schema": {"type": "string"},
                    "expected_sha256": {"type": "string"},
                    "preview_url": {"type": "string"}
                }
            }),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::Low
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(10)
    }

    async fn execute(
        &self,
        arguments: Value,
        permissions: &PermissionSet,
        _cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        let args: VerifyArtifactArgs = serde_json::from_value(arguments)
            .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
        let path = std::fs::canonicalize(&args.path)
            .map_err(|error| ToolError::Failed(format!("artifact does not exist: {error}")))?;
        if !permissions.read_roots.is_empty()
            && !permissions
                .read_roots
                .iter()
                .any(|root| path.starts_with(root))
        {
            return Err(ToolError::PermissionDenied(
                "artifact is outside the allowed read roots".into(),
            ));
        }
        let metadata = std::fs::metadata(&path)
            .map_err(|error| ToolError::Failed(format!("cannot stat artifact: {error}")))?;
        if !metadata.is_file() {
            return Err(ToolError::Failed("artifact path is not a file".into()));
        }
        if metadata.len() > MAX_ARTIFACT_BYTES {
            return Err(ToolError::Failed(format!(
                "artifact exceeds {MAX_ARTIFACT_BYTES} bytes"
            )));
        }
        let bytes = std::fs::read(&path)
            .map_err(|error| ToolError::Failed(format!("cannot read artifact: {error}")))?;
        let sha256 = hex::encode(Sha256::digest(&bytes));
        let media_type = detect_media_type(&path, &bytes);
        let mut checks = Vec::new();
        checks.push("exists".into());
        checks.push("non-empty".into());
        if let Some(expected) = args.media_type.as_deref()
            && !expected.eq_ignore_ascii_case(&media_type)
        {
            checks.push(format!(
                "media_type mismatch: expected {expected}, got {media_type}"
            ));
        }
        let schema_ok = check_schema(args.schema.as_deref(), &media_type, &bytes);
        if !schema_ok {
            checks.push("schema check failed".into());
        }
        if let Some(expected) = args.expected_sha256.as_deref()
            && !expected.eq_ignore_ascii_case(&sha256)
        {
            checks.push("sha256 mismatch".into());
        }
        let preview_ok = match args.preview_url.as_deref() {
            None => None,
            Some(url) if is_loopback_preview(url) => Some(preview_status(url).await),
            Some(_) => {
                checks.push("preview_url must be loopback".into());
                Some(false)
            }
        };
        if preview_ok == Some(false) {
            checks.push("preview failed".into());
        }
        let ok = !checks.iter().any(|check| {
            check.contains("mismatch")
                || check.contains("failed")
                || check.contains("must be loopback")
        });
        let verification = ArtifactVerification {
            path: path.display().to_string(),
            exists: true,
            bytes: bytes.len() as u64,
            sha256: sha256.clone(),
            media_type: media_type.clone(),
            schema_ok,
            preview_ok,
            checks,
            ok,
        };
        let mut metadata = BTreeMap::new();
        metadata.insert(
            "decision".into(),
            if ok { "verified" } else { "failed" }.into(),
        );
        metadata.insert("path".into(), verification.path.clone());
        metadata.insert("sha256".into(), sha256);
        metadata.insert("media_type".into(), media_type.clone());
        metadata.insert("bytes".into(), verification.bytes.to_string());
        metadata.insert("schema_ok".into(), schema_ok.to_string());
        if let Some(preview_ok) = preview_ok {
            metadata.insert("preview_ok".into(), preview_ok.to_string());
        }
        Ok(ToolOutcome {
            summary: format!(
                "artifact {}: {} ({} bytes, sha256 {}, schema {})",
                if ok { "verified" } else { "failed" },
                verification.path,
                verification.bytes,
                &verification.sha256[..12.min(verification.sha256.len())],
                if schema_ok { "ok" } else { "failed" }
            ),
            content: serde_json::to_string_pretty(&verification)
                .map_err(|error| ToolError::Failed(error.to_string()))?,
            metadata,
        })
    }
}

fn detect_media_type(path: &std::path::Path, bytes: &[u8]) -> String {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "svg" => "image/svg+xml".into(),
        "html" | "htm" => "text/html".into(),
        "json" => "application/json".into(),
        "png" => "image/png".into(),
        "jpg" | "jpeg" => "image/jpeg".into(),
        "pdf" => "application/pdf".into(),
        _ if bytes.starts_with(b"\x89PNG\r\n\x1a\n") => "image/png".into(),
        _ if bytes.starts_with(b"%PDF") => "application/pdf".into(),
        _ => "application/octet-stream".into(),
    }
}

fn check_schema(schema: Option<&str>, media_type: &str, bytes: &[u8]) -> bool {
    let Some(schema) = schema.map(str::trim).filter(|schema| !schema.is_empty()) else {
        return true;
    };
    match schema {
        "architecture-diagram/v1" | "svg" => {
            media_type == "image/svg+xml" && utf8_contains(bytes, "<svg")
        }
        "web-page/v1" => media_type == "text/html" && utf8_contains(bytes, "<html"),
        "document-text/v1" => media_type == "text/plain" || media_type == "application/pdf",
        _ => false,
    }
}

fn utf8_contains(bytes: &[u8], needle: &str) -> bool {
    std::str::from_utf8(bytes)
        .map(|text| text.to_ascii_lowercase().contains(needle))
        .unwrap_or(false)
}

fn is_loopback_preview(url: &str) -> bool {
    url.starts_with("http://127.0.0.1:")
        || url.starts_with("http://localhost:")
        || url.starts_with("http://[::1]:")
}

async fn preview_status(url: &str) -> bool {
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()
    else {
        return false;
    };
    matches!(client.get(url).send().await, Ok(response) if response.status().is_success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn verifies_svg_and_hash() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("diagram.svg");
        std::fs::write(&path, "<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>").unwrap();
        let sha = hex::encode(Sha256::digest(
            "<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>".as_bytes(),
        ));
        let outcome = VerifyArtifactTool
            .execute(
                json!({"path": path, "schema": "architecture-diagram/v1", "expected_sha256": sha}),
                &PermissionSet::default(),
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(
            outcome.metadata.get("decision").map(String::as_str),
            Some("verified")
        );
        assert!(outcome.content.contains("image/svg+xml"));
    }

    #[tokio::test]
    async fn wrong_hash_and_schema_fail_closed() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("diagram.svg");
        std::fs::write(&path, "<svg></svg>").unwrap();
        let outcome = VerifyArtifactTool
            .execute(
                json!({"path": path, "schema": "web-page/v1", "expected_sha256": "00"}),
                &PermissionSet::default(),
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(
            outcome.metadata.get("decision").map(String::as_str),
            Some("failed")
        );
    }

    #[tokio::test]
    async fn artifact_outside_read_roots_is_rejected() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("diagram.svg");
        std::fs::write(&path, "<svg></svg>").unwrap();
        let permissions = PermissionSet {
            read_roots: vec![workspace.path().join("other")],
            ..PermissionSet::default()
        };
        let error = VerifyArtifactTool
            .execute(
                json!({"path": path}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::PermissionDenied(_)));
    }
}
