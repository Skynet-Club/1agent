//! Loopback static preview server for generated pages.
//!
//! The tool serves a single file or a directory on `127.0.0.1` and verifies
//! it with an HTTP 200 fetch before returning the URL. The server task lives
//! for the lifetime of the 1Agent process; it is an interactive preview, not a
//! public server.

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;

use super::{AgentTool, ToolError, ToolOutcome, find_containing_root};
use crate::domain::{PermissionSet, RiskLevel, ToolSpec};

const MAX_REQUEST_BYTES: usize = 8 * 1024;
const MAX_PREVIEW_BYTES: u64 = 16 * 1024 * 1024;
const MAX_SERVERS: usize = 8;
const VERIFY_TIMEOUT: Duration = Duration::from_secs(2);

struct PreviewServer {
    target: PathBuf,
    port: u16,
    url: String,
    shutdown: CancellationToken,
    task: tokio::task::JoinHandle<()>,
}

impl PreviewServer {
    fn stop(self) {
        self.shutdown.cancel();
        self.task.abort();
    }
}

fn servers() -> &'static Mutex<Vec<PreviewServer>> {
    static SERVERS: OnceLock<Mutex<Vec<PreviewServer>>> = OnceLock::new();
    SERVERS.get_or_init(|| Mutex::new(Vec::new()))
}

pub struct PreviewFileTool;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PreviewArgs {
    path: String,
    #[serde(default)]
    port: Option<u16>,
}

#[async_trait]
impl AgentTool for PreviewFileTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "preview_file".into(),
            description: "Serve a local HTML file or directory on 127.0.0.1 and verify it with an HTTP 200 fetch. Use after writing a page so the user can click a URL; the server lives while this 1Agent process runs. Read-only; the path must be inside a read root.".into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["path"],
                "properties": {
                    "path": {"type": "string"},
                    "port": {"type": "integer", "minimum": 1, "maximum": 65535}
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
        cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        if cancellation.is_cancelled() {
            return Err(ToolError::Cancelled);
        }
        let args: PreviewArgs = serde_json::from_value(arguments)
            .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
        if args.path.trim().is_empty() {
            return Err(ToolError::InvalidArguments("path must not be empty".into()));
        }
        let target = resolve_target(&args.path, permissions).await?;
        let (url, port) = ensure_server(&target, args.port).await?;
        let (status, body_len) = match verify_preview(port).await {
            Ok(result) => result,
            Err(error) => {
                if let Some(server) = remove_server(port) {
                    server.stop();
                }
                return Err(error);
            }
        };
        if status != 200 || body_len == 0 {
            if let Some(server) = remove_server(port) {
                server.stop();
            }
            return Err(ToolError::Failed(format!(
                "preview verification failed: HTTP {status}, {body_len} body bytes"
            )));
        }
        let served = if target.is_file() {
            target.clone()
        } else {
            target.join("index.html")
        };
        let bytes = tokio::fs::read(&served)
            .await
            .map_err(|error| ToolError::Failed(format!("cannot read preview artifact: {error}")))?;
        let sha256 = hex::encode(Sha256::digest(&bytes));
        let mut metadata = BTreeMap::new();
        metadata.insert("url".into(), url.clone());
        metadata.insert("http_status".into(), status.to_string());
        metadata.insert("bytes".into(), body_len.to_string());
        metadata.insert("sha256".into(), sha256.clone());
        metadata.insert("target".into(), served.display().to_string());
        metadata.insert("verified".into(), "true".into());
        let content = json!({
            "url": url,
            "target": served.display().to_string(),
            "http_status": status,
            "bytes": body_len,
            "sha256": sha256,
            "verified": true,
            "note": "preview server lives while this 1Agent process is running"
        })
        .to_string();
        Ok(ToolOutcome {
            summary: format!("preview {url} (HTTP {status}, {body_len} bytes)"),
            content,
            metadata,
        })
    }
}

async fn resolve_target(path: &str, permissions: &PermissionSet) -> Result<PathBuf, ToolError> {
    if permissions.read_roots.is_empty() {
        return Err(ToolError::PermissionDenied(
            "no read roots configured".into(),
        ));
    }
    let requested = PathBuf::from(path);
    let requested = if requested.is_absolute() {
        requested
    } else {
        permissions.read_roots[0].join(requested)
    };
    let metadata = tokio::fs::symlink_metadata(&requested)
        .await
        .map_err(|error| ToolError::InvalidArguments(format!("cannot inspect path: {error}")))?;
    if metadata.file_type().is_symlink() {
        return Err(ToolError::PermissionDenied(
            "path must not be a symbolic link".into(),
        ));
    }
    let canonical = tokio::fs::canonicalize(&requested)
        .await
        .map_err(|error| ToolError::InvalidArguments(format!("cannot resolve path: {error}")))?;
    if find_containing_root(&canonical, &permissions.read_roots).is_none() {
        return Err(ToolError::PermissionDenied(
            "path is outside configured read roots".into(),
        ));
    }
    let metadata = tokio::fs::metadata(&canonical)
        .await
        .map_err(|error| ToolError::Failed(format!("cannot read metadata: {error}")))?;
    if !metadata.is_file() && !metadata.is_dir() {
        return Err(ToolError::InvalidArguments(
            "path must be a regular file or directory".into(),
        ));
    }
    Ok(canonical)
}

async fn ensure_server(target: &Path, port: Option<u16>) -> Result<(String, u16), ToolError> {
    if let Some((url, port)) = {
        let servers = servers().lock().unwrap_or_else(|error| error.into_inner());
        servers
            .iter()
            .find(|server| server.target == target && (port.is_none() || Some(server.port) == port))
            .map(|server| (server.url.clone(), server.port))
    } {
        return Ok((url, port));
    }

    let bind_port = port.unwrap_or(0);
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, bind_port)))
        .await
        .map_err(|error| {
            ToolError::Failed(format!("cannot bind 127.0.0.1:{bind_port}: {error}"))
        })?;
    let port = listener
        .local_addr()
        .map_err(|error| ToolError::Failed(format!("cannot read preview address: {error}")))?
        .port();
    let url = format!("http://127.0.0.1:{port}/");
    let shutdown = CancellationToken::new();
    let task = tokio::spawn(run_server(listener, target.to_path_buf(), shutdown.clone()));

    let mut servers = servers().lock().unwrap_or_else(|error| error.into_inner());
    let evicted = (servers.len() >= MAX_SERVERS).then(|| servers.remove(0));
    servers.push(PreviewServer {
        target: target.to_path_buf(),
        port,
        url: url.clone(),
        shutdown,
        task,
    });
    drop(servers);
    if let Some(server) = evicted {
        server.stop();
    }
    Ok((url, port))
}

fn remove_server(port: u16) -> Option<PreviewServer> {
    let mut servers = servers().lock().unwrap_or_else(|error| error.into_inner());
    servers
        .iter()
        .position(|server| server.port == port)
        .map(|index| servers.remove(index))
}

async fn run_server(listener: TcpListener, target: PathBuf, shutdown: CancellationToken) {
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            accepted = listener.accept() => {
                let Ok((stream, _)) = accepted else {
                    continue;
                };
                let target = target.clone();
                tokio::spawn(async move {
                    let _ = serve_connection(stream, target).await;
                });
            }
        }
    }
}

async fn serve_connection(mut stream: TcpStream, target: PathBuf) -> std::io::Result<()> {
    let mut request = Vec::new();
    let mut buffer = [0u8; 1024];
    loop {
        let read = stream.read(&mut buffer).await?;
        if read == 0 {
            return Ok(());
        }
        request.extend_from_slice(&buffer[..read]);
        if request.len() > MAX_REQUEST_BYTES {
            write_status(
                &mut stream,
                431,
                "Request Header Fields Too Large",
                "request too large",
            )
            .await?;
            return Ok(());
        }
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }

    let request = String::from_utf8_lossy(&request);
    let mut parts = request.lines().next().unwrap_or("").split_whitespace();
    let method = parts.next().unwrap_or("");
    let raw_path = parts.next().unwrap_or("/");
    if method != "GET" && method != "HEAD" {
        write_status(&mut stream, 405, "Method Not Allowed", "method not allowed").await?;
        return Ok(());
    }
    let request_path = raw_path.split('?').next().unwrap_or("/");
    let Some(file) = resolve_request_file(&target, request_path).await else {
        write_status(&mut stream, 404, "Not Found", "not found").await?;
        return Ok(());
    };
    let metadata = tokio::fs::metadata(&file).await?;
    if !metadata.is_file() {
        write_status(&mut stream, 404, "Not Found", "not found").await?;
        return Ok(());
    }
    if metadata.len() > MAX_PREVIEW_BYTES {
        write_status(
            &mut stream,
            413,
            "Payload Too Large",
            "preview file is too large",
        )
        .await?;
        return Ok(());
    }
    let bytes = tokio::fs::read(&file).await?;
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        content_type(&file),
        bytes.len()
    );
    stream.write_all(header.as_bytes()).await?;
    if method != "HEAD" {
        stream.write_all(&bytes).await?;
    }
    stream.shutdown().await.ok();
    Ok(())
}

async fn resolve_request_file(target: &Path, request_path: &str) -> Option<PathBuf> {
    if target.is_file() {
        let name = target.file_name()?.to_string_lossy();
        let requested = request_path.trim_start_matches('/');
        return (request_path == "/" || requested == "index.html" || requested == name.as_ref())
            .then(|| target.to_path_buf());
    }
    if !target.is_dir() {
        return None;
    }
    let decoded = percent_decode(request_path)?;
    if decoded.contains('\0') {
        return None;
    }
    let relative = decoded.trim_start_matches('/');
    let relative = if relative.is_empty() {
        "index.html"
    } else {
        relative
    };
    let canonical = tokio::fs::canonicalize(target.join(relative)).await.ok()?;
    canonical.starts_with(target).then_some(canonical)
}

fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return None;
            }
            let high = (bytes[index + 1] as char).to_digit(16)?;
            let low = (bytes[index + 2] as char).to_digit(16)?;
            output.push((high * 16 + low) as u8);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(output).ok()
}

fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "txt" | "md" => "text/plain; charset=utf-8",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

async fn write_status(
    stream: &mut TcpStream,
    code: u16,
    reason: &str,
    body: &str,
) -> std::io::Result<()> {
    let response = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await.ok();
    Ok(())
}

async fn verify_preview(port: u16) -> Result<(u16, usize), ToolError> {
    tokio::time::timeout(VERIFY_TIMEOUT, async {
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
            .await
            .map_err(|error| {
                ToolError::Failed(format!("cannot connect to preview server: {error}"))
            })?;
        stream
            .write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
            .await
            .map_err(|error| ToolError::Failed(format!("cannot verify preview: {error}")))?;
        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .await
            .map_err(|error| ToolError::Failed(format!("cannot read preview response: {error}")))?;
        let header_end = response
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .ok_or_else(|| ToolError::Failed("preview response has no headers".into()))?;
        let header = String::from_utf8_lossy(&response[..header_end]);
        let status = header
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|status| status.parse::<u16>().ok())
            .ok_or_else(|| ToolError::Failed("preview response has no status".into()))?;
        let body_len = response.len().saturating_sub(header_end + 4);
        Ok((status, body_len))
    })
    .await
    .map_err(|_| ToolError::Failed("preview verification timed out".into()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn permission_set(root: &Path) -> PermissionSet {
        PermissionSet {
            read_roots: vec![root.to_path_buf()],
            ..PermissionSet::default()
        }
    }

    async fn http_get(port: u16, path: &str) -> String {
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
            .await
            .expect("connect");
        stream
            .write_all(
                format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .await
            .expect("write");
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.expect("read");
        String::from_utf8_lossy(&response).to_string()
    }

    #[tokio::test]
    async fn serves_a_file_and_verifies_http_200() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("index.html");
        tokio::fs::write(&file, "<!doctype html><title>ok</title><h1>preview</h1>")
            .await
            .expect("write");
        let tool = PreviewFileTool;
        let outcome = tool
            .execute(
                json!({"path": file.to_string_lossy()}),
                &permission_set(dir.path()),
                &CancellationToken::new(),
            )
            .await
            .expect("preview");
        assert!(outcome.summary.contains("HTTP 200"));
        let value: Value = serde_json::from_str(&outcome.content).expect("content");
        let url = value["url"].as_str().expect("url");
        let port = url
            .trim_start_matches("http://127.0.0.1:")
            .trim_end_matches('/')
            .parse::<u16>()
            .expect("port");
        let response = http_get(port, "/").await;
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("preview"));
    }

    #[tokio::test]
    async fn rejects_a_path_outside_the_read_roots() {
        let root = tempfile::tempdir().expect("root");
        let outside = tempfile::tempdir().expect("outside");
        let file = outside.path().join("secret.html");
        tokio::fs::write(&file, "secret").await.expect("write");
        let tool = PreviewFileTool;
        let error = tool
            .execute(
                json!({"path": file.to_string_lossy()}),
                &permission_set(root.path()),
                &CancellationToken::new(),
            )
            .await
            .expect_err("outside root must fail");
        assert!(matches!(error, ToolError::PermissionDenied(_)));
    }

    #[tokio::test]
    async fn directory_index_is_served_and_traversal_is_blocked() {
        let dir = tempfile::tempdir().expect("dir");
        tokio::fs::write(dir.path().join("index.html"), "index page")
            .await
            .expect("write");
        let tool = PreviewFileTool;
        let outcome = tool
            .execute(
                json!({"path": dir.path().to_string_lossy()}),
                &permission_set(dir.path()),
                &CancellationToken::new(),
            )
            .await
            .expect("preview");
        let value: Value = serde_json::from_str(&outcome.content).expect("content");
        let port = value["url"]
            .as_str()
            .expect("url")
            .trim_start_matches("http://127.0.0.1:")
            .trim_end_matches('/')
            .parse::<u16>()
            .expect("port");
        assert!(http_get(port, "/").await.contains("index page"));
        assert!(
            http_get(port, "/%2e%2e/secret")
                .await
                .starts_with("HTTP/1.1 404")
        );
    }
}
