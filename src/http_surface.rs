//! Minimal loopback-only HTTP transport for the Surface v0 contract.
//!
//! The server is intentionally small: fixed routes, bearer token, body/header
//! limits, read/write timeouts, no chunked bodies and no directory serving.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::surface_api::{
    AuthContext, SurfaceError, SurfaceErrorCode, SurfaceRequest, openapi_document,
};
use crate::surface_events::SurfaceEventBus;
use crate::surface_service::SurfaceService;

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 256 * 1024;

#[derive(Debug, Error)]
pub enum HttpSurfaceError {
    #[error("http surface io: {0}")]
    Io(#[from] std::io::Error),
    #[error("http surface bad request: {0}")]
    BadRequest(String),
    #[error("http surface payload too large")]
    PayloadTooLarge,
    #[error("http surface unauthorized")]
    Unauthorized,
    #[error("http surface json: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn json(status: u16, value: &serde_json::Value) -> Result<Self, HttpSurfaceError> {
        Ok(Self {
            status,
            content_type: "application/json".into(),
            body: serde_json::to_vec(value)?,
        })
    }

    pub fn error(status: u16, error: SurfaceError) -> Result<Self, HttpSurfaceError> {
        Self::json(status, &serde_json::json!({"error": error}))
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let reason = match self.status {
            200 => "OK",
            400 => "Bad Request",
            401 => "Unauthorized",
            403 => "Forbidden",
            404 => "Not Found",
            405 => "Method Not Allowed",
            409 => "Conflict",
            413 => "Payload Too Large",
            429 => "Too Many Requests",
            500 => "Internal Server Error",
            501 => "Not Implemented",
            503 => "Service Unavailable",
            _ => "Error",
        };
        format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
            self.status,
            reason,
            self.content_type,
            self.body.len()
        )
        .into_bytes()
        .into_iter()
        .chain(self.body.iter().copied())
        .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HttpSurfaceEnvelope {
    pub auth: AuthContext,
    pub request: SurfaceRequest,
}

pub fn parse_http_request(bytes: &[u8], max_body: usize) -> Result<HttpRequest, HttpSurfaceError> {
    let header_end = find_header_end(bytes)
        .ok_or_else(|| HttpSurfaceError::BadRequest("missing header terminator".into()))?;
    if header_end > MAX_HEADER_BYTES {
        return Err(HttpSurfaceError::BadRequest("header too large".into()));
    }
    let header = std::str::from_utf8(&bytes[..header_end])
        .map_err(|_| HttpSurfaceError::BadRequest("header is not utf-8".into()))?;
    let mut lines = header.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| HttpSurfaceError::BadRequest("missing request line".into()))?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| HttpSurfaceError::BadRequest("missing method".into()))?;
    let path = parts
        .next()
        .ok_or_else(|| HttpSurfaceError::BadRequest("missing path".into()))?;
    let version = parts
        .next()
        .ok_or_else(|| HttpSurfaceError::BadRequest("missing version".into()))?;
    if !matches!(version, "HTTP/1.0" | "HTTP/1.1") || parts.next().is_some() {
        return Err(HttpSurfaceError::BadRequest("invalid request line".into()));
    }
    if !matches!(method, "GET" | "POST") {
        return Err(HttpSurfaceError::BadRequest("unsupported method".into()));
    }
    if !path.starts_with('/') || path.contains("..") || path.contains('\\') {
        return Err(HttpSurfaceError::BadRequest("unsafe path".into()));
    }
    let mut headers = BTreeMap::new();
    for line in lines {
        let (key, value) = line
            .split_once(':')
            .ok_or_else(|| HttpSurfaceError::BadRequest("invalid header".into()))?;
        let key = key.trim().to_ascii_lowercase();
        if key.is_empty() {
            return Err(HttpSurfaceError::BadRequest("empty header name".into()));
        }
        if key == "transfer-encoding" {
            return Err(HttpSurfaceError::BadRequest(
                "chunked transfer encoding is not supported".into(),
            ));
        }
        if key == "content-length" && headers.contains_key(&key) {
            return Err(HttpSurfaceError::BadRequest(
                "duplicate content-length".into(),
            ));
        }
        headers.insert(key, value.trim().to_string());
    }
    let content_length = headers
        .get("content-length")
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|_| HttpSurfaceError::BadRequest("invalid content-length".into()))
        })
        .transpose()?
        .unwrap_or(0);
    if content_length > max_body {
        return Err(HttpSurfaceError::PayloadTooLarge);
    }
    let body_start = header_end + 4;
    let body_end = body_start
        .checked_add(content_length)
        .ok_or(HttpSurfaceError::PayloadTooLarge)?;
    if bytes.len() < body_end {
        return Err(HttpSurfaceError::BadRequest("incomplete body".into()));
    }
    Ok(HttpRequest {
        method: method.to_string(),
        path: path.to_string(),
        headers,
        body: bytes[body_start..body_end].to_vec(),
    })
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

pub fn read_http_request(
    stream: &mut TcpStream,
    max_body: usize,
) -> Result<HttpRequest, HttpSurfaceError> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        if let Some(position) = find_header_end(&bytes) {
            break position;
        }
        if bytes.len() > MAX_HEADER_BYTES {
            return Err(HttpSurfaceError::BadRequest("header too large".into()));
        }
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            return Err(HttpSurfaceError::BadRequest("connection closed".into()));
        }
        bytes.extend_from_slice(&chunk[..read]);
    };
    let header = std::str::from_utf8(&bytes[..header_end])
        .map_err(|_| HttpSurfaceError::BadRequest("header is not utf-8".into()))?;
    let content_length = header
        .split("\r\n")
        .filter_map(|line| line.split_once(':'))
        .find(|(key, _)| key.trim().eq_ignore_ascii_case("content-length"))
        .map(|(_, value)| {
            value
                .trim()
                .parse::<usize>()
                .map_err(|_| HttpSurfaceError::BadRequest("invalid content-length".into()))
        })
        .transpose()?
        .unwrap_or(0);
    if content_length > max_body {
        return Err(HttpSurfaceError::PayloadTooLarge);
    }
    let body_start = header_end + 4;
    let body_end = body_start
        .checked_add(content_length)
        .ok_or(HttpSurfaceError::PayloadTooLarge)?;
    while bytes.len() < body_end {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            return Err(HttpSurfaceError::BadRequest("incomplete body".into()));
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
    parse_http_request(&bytes, max_body)
}

pub fn handle_http(
    request: &HttpRequest,
    service: &mut SurfaceService,
    token: &str,
    server_auth: &AuthContext,
) -> Result<HttpResponse, HttpSurfaceError> {
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/v0/health") => Ok(HttpResponse::json(
            200,
            &serde_json::json!({"ok": true, "version": "v0"}),
        )?),
        ("GET", "/v0/openapi") => {
            let document = openapi_document();
            Ok(HttpResponse::json(200, &document)?)
        }
        ("GET", "/v0/events") => Ok(HttpResponse::error(
            501,
            SurfaceError::new(
                SurfaceErrorCode::Unavailable,
                "SSE event bus is not wired in v0",
                false,
            ),
        )?),
        ("POST", "/v0/surface") => {
            require_bearer(request, token)?;
            let envelope: HttpSurfaceEnvelope = serde_json::from_slice(&request.body)
                .map_err(|_| HttpSurfaceError::BadRequest("invalid envelope".into()))?;
            // The bearer token identifies the server-side principal. Never
            // trust actor/scopes supplied in the request body.
            let response = service.handle(&envelope.request, server_auth, now_ms());
            let status = if response.ok {
                200
            } else {
                response.error.as_ref().map(status_for).unwrap_or(500)
            };
            Ok(HttpResponse::json(
                status,
                &serde_json::to_value(&response)?,
            )?)
        }
        (method, "/v0/surface") => Ok(HttpResponse::error(
            405,
            SurfaceError::new(
                SurfaceErrorCode::InvalidRequest,
                format!("{method} not allowed"),
                false,
            ),
        )?),
        _ => Ok(HttpResponse::error(
            404,
            SurfaceError::new(SurfaceErrorCode::NotFound, "route not found", false),
        )?),
    }
}

fn require_bearer(request: &HttpRequest, token: &str) -> Result<(), HttpSurfaceError> {
    let value = request
        .headers
        .get("authorization")
        .ok_or(HttpSurfaceError::Unauthorized)?;
    let provided = value
        .strip_prefix("Bearer ")
        .ok_or(HttpSurfaceError::Unauthorized)?;
    if token.is_empty() || !constant_time_eq(provided.as_bytes(), token.as_bytes()) {
        return Err(HttpSurfaceError::Unauthorized);
    }
    Ok(())
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (left, right) in left.iter().zip(right.iter()) {
        diff |= left ^ right;
    }
    diff == 0
}

fn status_for(error: &SurfaceError) -> u16 {
    match error.code {
        SurfaceErrorCode::InvalidRequest => 400,
        SurfaceErrorCode::Unauthorized => 401,
        SurfaceErrorCode::Forbidden => 403,
        SurfaceErrorCode::NotFound => 404,
        SurfaceErrorCode::Conflict | SurfaceErrorCode::IdempotencyConflict => 409,
        SurfaceErrorCode::RateLimited => 429,
        SurfaceErrorCode::Unavailable => 503,
        SurfaceErrorCode::Internal => 500,
    }
}

pub fn bind_surface_http(addr: &str) -> Result<TcpListener, HttpSurfaceError> {
    let parsed: SocketAddr = addr
        .parse()
        .map_err(|_| HttpSurfaceError::BadRequest("invalid listen address".into()))?;
    if !parsed.ip().is_loopback() {
        return Err(HttpSurfaceError::BadRequest(
            "surface HTTP must bind to a loopback address".into(),
        ));
    }
    let listener = TcpListener::bind(parsed)?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

pub fn serve_surface_http(
    addr: &str,
    token: String,
    auth: AuthContext,
    service: Arc<Mutex<SurfaceService>>,
    events: Arc<Mutex<SurfaceEventBus>>,
    shutdown: Arc<AtomicBool>,
) -> Result<(), HttpSurfaceError> {
    let listener = bind_surface_http(addr)?;
    println!(
        "surface http listening on http://{}",
        listener.local_addr()?
    );
    serve_bound(listener, token, auth, service, events, shutdown)
}

pub fn serve_bound(
    listener: TcpListener,
    token: String,
    auth: AuthContext,
    service: Arc<Mutex<SurfaceService>>,
    events: Arc<Mutex<SurfaceEventBus>>,
    shutdown: Arc<AtomicBool>,
) -> Result<(), HttpSurfaceError> {
    if token.is_empty() {
        return Err(HttpSurfaceError::BadRequest(
            "surface HTTP token must not be empty".into(),
        ));
    }
    while !shutdown.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                stream.set_nonblocking(false)?;
                stream.set_write_timeout(Some(Duration::from_secs(5)))?;
                let request = match read_http_request(&mut stream, MAX_BODY_BYTES) {
                    Ok(request) => request,
                    Err(error) => {
                        let _ = stream.write_all(&error_response(error).to_bytes());
                        continue;
                    }
                };
                if request.method == "GET" && request.path == "/v0/events" {
                    if let Err(error) = require_bearer(&request, &token) {
                        let _ = stream.write_all(&error_response(error).to_bytes());
                        continue;
                    }
                    let stream_events = events.clone();
                    let shutdown_sse = shutdown.clone();
                    std::thread::spawn(move || {
                        serve_sse(stream, stream_events, shutdown_sse);
                    });
                    continue;
                }
                let response = {
                    let mut service = service
                        .lock()
                        .map_err(|_| HttpSurfaceError::BadRequest("service lock".into()))?;
                    handle_http(&request, &mut service, &token, &auth)
                }
                .unwrap_or_else(error_response);
                let _ = stream.write_all(&response.to_bytes());
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(HttpSurfaceError::Io(error)),
        }
    }
    Ok(())
}

fn serve_sse(
    mut stream: TcpStream,
    events: Arc<Mutex<SurfaceEventBus>>,
    shutdown: Arc<AtomicBool>,
) {
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    if stream
        .write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\n\r\n",
        )
        .is_err()
    {
        return;
    }
    let subscription = match events.lock() {
        Ok(mut bus) => bus.subscribe(64),
        Err(_) => return,
    };
    while !shutdown.load(Ordering::Acquire) {
        match subscription
            .receiver()
            .recv_timeout(Duration::from_secs(10))
        {
            Ok(event) => {
                let Ok(frame) = event.to_sse() else {
                    continue;
                };
                if stream.write_all(frame.as_bytes()).is_err() {
                    break;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if stream.write_all(b": heartbeat\n\n").is_err() {
                    break;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn error_response(error: HttpSurfaceError) -> HttpResponse {
    let (status, code, message) = match error {
        HttpSurfaceError::PayloadTooLarge => (
            413,
            SurfaceErrorCode::InvalidRequest,
            "payload too large".to_string(),
        ),
        HttpSurfaceError::Unauthorized => (
            401,
            SurfaceErrorCode::Unauthorized,
            "unauthorized".to_string(),
        ),
        HttpSurfaceError::BadRequest(message) => (400, SurfaceErrorCode::InvalidRequest, message),
        HttpSurfaceError::Io(error) => (
            500,
            SurfaceErrorCode::Internal,
            format!("transport error: {error}"),
        ),
        HttpSurfaceError::Json(error) => (
            500,
            SurfaceErrorCode::Internal,
            format!("json error: {error}"),
        ),
    };
    HttpResponse::error(status, SurfaceError::new(code, message, false)).unwrap_or_else(|_| {
        HttpResponse {
            status: 500,
            content_type: "application/json".into(),
            body: br#"{"error":{"code":"internal","message":"internal","retryable":false}}"#
                .to_vec(),
        }
    })
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server_auth() -> AuthContext {
        AuthContext {
            actor_id: "operator".into(),
            scopes: vec!["*".into()],
        }
    }

    #[test]
    fn parser_rejects_smuggling_oversize_and_unsafe_paths() {
        let request = b"POST /v0/surface HTTP/1.1\r\nContent-Length: 2\r\n\r\n{}";
        let parsed = parse_http_request(request, 10).unwrap();
        assert_eq!(parsed.method, "POST");
        assert_eq!(parsed.body, b"{}");
        assert!(matches!(
            parse_http_request(
                b"POST /v0/surface HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n",
                10
            )
            .unwrap_err(),
            HttpSurfaceError::BadRequest(_)
        ));
        assert!(matches!(
            parse_http_request(b"GET /../etc/passwd HTTP/1.1\r\n\r\n", 10).unwrap_err(),
            HttpSurfaceError::BadRequest(_)
        ));
        assert!(matches!(
            parse_http_request(
                b"POST /v0/surface HTTP/1.1\r\nContent-Length: 11\r\n\r\n12345678901",
                10
            )
            .unwrap_err(),
            HttpSurfaceError::PayloadTooLarge
        ));
    }

    #[test]
    fn handler_requires_bearer_for_surface_but_not_health() {
        let dir = tempfile::tempdir().unwrap();
        let service = SurfaceService::open(dir.path().join("graph.jsonl")).unwrap();
        let mut service = service;
        let health = HttpRequest {
            method: "GET".into(),
            path: "/v0/health".into(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        assert_eq!(
            handle_http(&health, &mut service, "token", &server_auth())
                .unwrap()
                .status,
            200
        );
        let surface = HttpRequest {
            method: "POST".into(),
            path: "/v0/surface".into(),
            headers: BTreeMap::new(),
            body: b"{}".to_vec(),
        };
        assert!(matches!(
            handle_http(&surface, &mut service, "token", &server_auth()).unwrap_err(),
            HttpSurfaceError::Unauthorized
        ));
    }

    #[test]
    fn body_auth_cannot_escalate_the_server_principal() {
        let dir = tempfile::tempdir().unwrap();
        let mut service = SurfaceService::open(dir.path().join("graph.jsonl")).unwrap();
        let envelope = HttpSurfaceEnvelope {
            auth: AuthContext {
                actor_id: "owner-1".into(),
                scopes: vec!["ticket:approve".into()],
            },
            request: SurfaceRequest {
                protocol_version: "v0".into(),
                request_id: "request-1".into(),
                trace_id: "trace-1".into(),
                idempotency_key: None,
                method: crate::surface_api::SurfaceMethod::TicketApprove {
                    ticket_id: "ticket-1".into(),
                    actor_id: "owner-1".into(),
                    decision: "allow_once".into(),
                    expires_ms: None,
                },
            },
        };
        let mut headers = BTreeMap::new();
        headers.insert("authorization".into(), "Bearer token".into());
        let request = HttpRequest {
            method: "POST".into(),
            path: "/v0/surface".into(),
            headers,
            body: serde_json::to_vec(&envelope).unwrap(),
        };
        let response = handle_http(
            &request,
            &mut service,
            "token",
            &AuthContext {
                actor_id: "operator".into(),
                scopes: Vec::new(),
            },
        )
        .unwrap();
        assert_eq!(response.status, 403);
    }

    #[test]
    fn constant_time_compare_and_route_errors_are_stable() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
        assert_eq!(status_for(&SurfaceError::invalid("bad")), 400);
        assert_eq!(
            status_for(&SurfaceError::new(
                SurfaceErrorCode::RateLimited,
                "slow",
                true
            )),
            429
        );
    }
}
