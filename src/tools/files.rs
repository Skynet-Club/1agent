//! File write and edit tools.
//!
//! Both tools are write-gated by the registry, stay inside the configured read
//! roots, reject symlink targets and replace files atomically through a
//! same-directory temporary file.

use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{AgentTool, ToolError, ToolOutcome};
use crate::domain::{PermissionSet, RiskLevel, ToolSpec};

const MAX_CONTENT_CHARS: usize = 256 * 1024;
const MAX_FILE_BYTES: usize = 1024 * 1024;
const MAX_PATH_CHARS: usize = 4096;

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

async fn resolve_target(path: &str, permissions: &PermissionSet) -> Result<PathBuf, ToolError> {
    if path.is_empty() || path.chars().count() > MAX_PATH_CHARS {
        return Err(ToolError::InvalidArguments(
            "path must be 1..=4096 characters".into(),
        ));
    }
    if permissions.read_roots.is_empty() {
        return Err(ToolError::PermissionDenied(
            "write roots are not configured".into(),
        ));
    }

    let requested = PathBuf::from(path);
    let requested = if requested.is_absolute() {
        requested
    } else {
        std::env::current_dir()
            .map_err(|error| ToolError::Failed(format!("cannot determine cwd: {error}")))?
            .join(requested)
    };
    let parent = requested
        .parent()
        .ok_or_else(|| ToolError::InvalidArguments("path has no parent directory".into()))?;
    let canonical_parent = tokio::fs::canonicalize(parent).await.map_err(|error| {
        ToolError::InvalidArguments(format!("parent directory is not accessible: {error}"))
    })?;
    let file_name = requested
        .file_name()
        .ok_or_else(|| ToolError::InvalidArguments("path has no file name".into()))?;
    let target = canonical_parent.join(file_name);

    let mut allowed = false;
    for root in &permissions.read_roots {
        if let Ok(root) = tokio::fs::canonicalize(root).await
            && target.starts_with(&root)
        {
            allowed = true;
            break;
        }
    }
    if !allowed {
        return Err(ToolError::PermissionDenied(
            "path is outside the allowed write roots".into(),
        ));
    }

    if let Ok(metadata) = tokio::fs::symlink_metadata(&target).await {
        if metadata.file_type().is_symlink() {
            return Err(ToolError::PermissionDenied(
                "refusing to write through a symlink".into(),
            ));
        }
        if !metadata.is_file() {
            return Err(ToolError::InvalidArguments(
                "target is not a regular file".into(),
            ));
        }
    }
    Ok(target)
}

async fn atomic_write(target: &Path, content: &str) -> Result<(), ToolError> {
    let parent = target
        .parent()
        .ok_or_else(|| ToolError::Failed("target has no parent directory".into()))?;
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let temporary = parent.join(format!(".{name}.{}.tmp", Uuid::new_v4()));
    tokio::fs::write(&temporary, content)
        .await
        .map_err(|error| ToolError::Failed(format!("cannot write temporary file: {error}")))?;
    if let Err(error) = tokio::fs::rename(&temporary, target).await {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(ToolError::Failed(format!(
            "cannot replace {}: {error}",
            target.display()
        )));
    }
    Ok(())
}

fn validate_content(value: &str, label: &str) -> Result<(), ToolError> {
    if value.chars().count() > MAX_CONTENT_CHARS {
        return Err(ToolError::InvalidArguments(format!(
            "{label} must be at most {MAX_CONTENT_CHARS} characters"
        )));
    }
    Ok(())
}

/// Write a complete file inside the allowed roots.
pub struct WriteFileTool;

#[async_trait]
impl AgentTool for WriteFileTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "write_file".into(),
            description: "Write a complete UTF-8 file inside an allowed root. The parent directory must already exist. Existing files are refused unless overwrite=true. Writes are atomic. Requires write permission.".into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["path", "content"],
                "properties": {
                    "path": {
                        "type": "string",
                        "maxLength": MAX_PATH_CHARS,
                        "description": "Absolute path or path relative to the process cwd."
                    },
                    "content": {
                        "type": "string",
                        "maxLength": MAX_CONTENT_CHARS,
                        "description": "Complete file content."
                    },
                    "overwrite": {
                        "type": "boolean",
                        "description": "Replace an existing file. Default false."
                    }
                }
            }),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::Medium
    }

    fn requires_write(&self) -> bool {
        true
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(30)
    }

    async fn execute(
        &self,
        arguments: Value,
        permissions: &PermissionSet,
        _cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        let path = arguments
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::InvalidArguments("path is required".into()))?;
        let content = arguments
            .get("content")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::InvalidArguments("content is required".into()))?;
        validate_content(content, "content")?;
        let overwrite = arguments
            .get("overwrite")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let target = resolve_target(path, permissions).await?;
        let existed = tokio::fs::symlink_metadata(&target).await.is_ok();
        if existed && !overwrite {
            return Err(ToolError::Failed(
                "file already exists; pass overwrite=true to replace it".into(),
            ));
        }
        atomic_write(&target, content).await?;

        let bytes = content.len();
        let content_json = json!({
            "path": target.display().to_string(),
            "bytes": bytes,
            "created": !existed,
            "sha256": sha256_hex(content.as_bytes())
        });
        Ok(ToolOutcome {
            summary: format!(
                "已{}文件 {}（{bytes} 字节）",
                if existed { "覆盖" } else { "创建" },
                target.display()
            ),
            content: content_json.to_string(),
            metadata: Default::default(),
        })
    }
}

/// Replace text in an existing file inside the allowed roots.
pub struct EditFileTool;

#[async_trait]
impl AgentTool for EditFileTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "edit_file".into(),
            description: "Replace text in an existing UTF-8 file inside an allowed root. old_string must occur exactly once unless replace_all=true. The whole updated file is written atomically. Requires write permission.".into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["path", "old_string", "new_string"],
                "properties": {
                    "path": {
                        "type": "string",
                        "maxLength": MAX_PATH_CHARS,
                        "description": "Absolute path or path relative to the process cwd."
                    },
                    "old_string": {
                        "type": "string",
                        "maxLength": MAX_CONTENT_CHARS,
                        "description": "Exact text to replace."
                    },
                    "new_string": {
                        "type": "string",
                        "maxLength": MAX_CONTENT_CHARS,
                        "description": "Replacement text; may be empty."
                    },
                    "replace_all": {
                        "type": "boolean",
                        "description": "Replace every occurrence. Default false."
                    }
                }
            }),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::Medium
    }

    fn requires_write(&self) -> bool {
        true
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(30)
    }

    async fn execute(
        &self,
        arguments: Value,
        permissions: &PermissionSet,
        _cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        let path = arguments
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::InvalidArguments("path is required".into()))?;
        let old_string = arguments
            .get("old_string")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::InvalidArguments("old_string is required".into()))?;
        if old_string.is_empty() {
            return Err(ToolError::InvalidArguments(
                "old_string must not be empty".into(),
            ));
        }
        validate_content(old_string, "old_string")?;
        let new_string = arguments
            .get("new_string")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::InvalidArguments("new_string is required".into()))?;
        validate_content(new_string, "new_string")?;
        let replace_all = arguments
            .get("replace_all")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let target = resolve_target(path, permissions).await?;
        let metadata = tokio::fs::symlink_metadata(&target)
            .await
            .map_err(|error| {
                ToolError::Failed(format!("cannot stat {}: {error}", target.display()))
            })?;
        if !metadata.is_file() {
            return Err(ToolError::InvalidArguments(
                "target is not a regular file".into(),
            ));
        }
        if metadata.len() > MAX_FILE_BYTES as u64 {
            return Err(ToolError::InvalidArguments(format!(
                "file is larger than {MAX_FILE_BYTES} bytes"
            )));
        }
        let bytes = tokio::fs::read(&target).await.map_err(|error| {
            ToolError::Failed(format!("cannot read {}: {error}", target.display()))
        })?;
        let text = String::from_utf8(bytes)
            .map_err(|_| ToolError::InvalidArguments("target file is not valid UTF-8".into()))?;

        let occurrences = text.matches(old_string).count();
        if occurrences == 0 {
            return Err(ToolError::Failed("old_string was not found".into()));
        }
        if occurrences > 1 && !replace_all {
            return Err(ToolError::Failed(format!(
                "old_string occurs {occurrences} times; pass replace_all=true or provide unique context"
            )));
        }
        let updated = if replace_all {
            text.replace(old_string, new_string)
        } else {
            text.replacen(old_string, new_string, 1)
        };
        atomic_write(&target, &updated).await?;

        let replacements = if replace_all { occurrences } else { 1 };
        let content_json = json!({
            "path": target.display().to_string(),
            "replacements": replacements,
            "bytes_before": text.len(),
            "bytes_after": updated.len(),
            "sha256_before": sha256_hex(text.as_bytes()),
            "sha256_after": sha256_hex(updated.as_bytes())
        });
        Ok(ToolOutcome {
            summary: format!("已编辑 {}（{replacements} 处替换）", target.display()),
            content: content_json.to_string(),
            metadata: Default::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::domain::{ToolCall, ToolStatus};
    use crate::tools::ToolRegistry;

    fn write_permissions(root: &Path) -> PermissionSet {
        PermissionSet {
            read_roots: vec![std::fs::canonicalize(root).unwrap()],
            allow_network: false,
            allow_write: true,
        }
    }

    async fn write(
        tool: &WriteFileTool,
        path: &Path,
        content: &str,
        overwrite: bool,
    ) -> Result<ToolOutcome, ToolError> {
        tool.execute(
            json!({"path": path, "content": content, "overwrite": overwrite}),
            &write_permissions(path.parent().unwrap()),
            &CancellationToken::new(),
        )
        .await
    }

    #[tokio::test]
    async fn writes_new_files_and_requires_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("note.txt");
        let tool = WriteFileTool;

        let outcome = write(&tool, &target, "first\n", false).await.unwrap();
        assert!(outcome.summary.contains("创建"));
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "first\n");

        let error = write(&tool, &target, "second\n", false).await.unwrap_err();
        assert!(matches!(error, ToolError::Failed(_)));
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "first\n");

        write(&tool, &target, "second\n", true).await.unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "second\n");
    }

    #[tokio::test]
    async fn rejects_paths_outside_the_allowed_root() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let tool = WriteFileTool;
        let error = tool
            .execute(
                json!({"path": outside.path().join("x.txt"), "content": "x"}),
                &write_permissions(root.path()),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::PermissionDenied(_)));
    }

    #[tokio::test]
    async fn edit_replaces_unique_text_and_refuses_ambiguity() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("note.txt");
        std::fs::write(&target, "alpha beta alpha\n").unwrap();
        let permissions = write_permissions(dir.path());
        let tool = EditFileTool;

        let error = tool
            .execute(
                json!({"path": target, "old_string": "alpha", "new_string": "gamma"}),
                &permissions,
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::Failed(_)));

        tool.execute(
            json!({"path": target, "old_string": "alpha", "new_string": "gamma", "replace_all": true}),
            &permissions,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "gamma beta gamma\n"
        );
    }

    #[tokio::test]
    async fn edit_rejects_missing_text() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("note.txt");
        std::fs::write(&target, "alpha\n").unwrap();
        let error = EditFileTool
            .execute(
                json!({"path": target, "old_string": "missing", "new_string": "x"}),
                &write_permissions(dir.path()),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::Failed(_)));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn refuses_to_write_through_a_symlink() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real.txt");
        let link = dir.path().join("link.txt");
        std::fs::write(&target, "real\n").unwrap();
        symlink(&target, &link).unwrap();
        let error = WriteFileTool
            .execute(
                json!({"path": link, "content": "x", "overwrite": true}),
                &write_permissions(dir.path()),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::PermissionDenied(_)));
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "real\n");
    }

    #[tokio::test]
    async fn registry_denies_file_tools_without_write_permission() {
        let dir = tempfile::tempdir().unwrap();
        let mut registry = ToolRegistry::default();
        registry.register(Arc::new(WriteFileTool));
        let call = ToolCall {
            id: "call-1".into(),
            name: "write_file".into(),
            arguments: json!({"path": dir.path().join("x.txt"), "content": "x"}),
        };
        let observation = registry
            .execute(&call, &PermissionSet::default(), &CancellationToken::new())
            .await;
        assert_eq!(observation.status, ToolStatus::PermissionDenied);
        assert_eq!(observation.error_code.as_deref(), Some("write_not_allowed"));
    }
}
