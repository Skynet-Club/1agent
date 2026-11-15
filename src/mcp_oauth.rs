//! MCP OAuth 2.1 authorization-code + PKCE for remote HTTP servers.
//!
//! The flow is loopback-only: a temporary `127.0.0.1` listener receives the
//! authorization code, the code is exchanged with PKCE S256, and the token is
//! written to a 0600 file when `token_file` is configured. Tokens are bound to
//! the configured `resource` (RFC 8707) when provided.

use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use uuid::Uuid;

const CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Error)]
pub enum OAuthError {
    #[error("OAuth io: {0}")]
    Io(#[from] std::io::Error),
    #[error("OAuth URL: {0}")]
    Url(String),
    #[error("OAuth HTTP: {0}")]
    Http(String),
    #[error("OAuth response: {0}")]
    Response(String),
    #[error("OAuth state mismatch")]
    StateMismatch,
    #[error("OAuth callback missing code")]
    MissingCode,
    #[error("OAuth issuer mismatch: expected {expected}, got {actual}")]
    IssuerMismatch { expected: String, actual: String },
    #[error("OAuth callback is missing the RFC 9207 issuer")]
    MissingIssuer,
    #[error("OAuth token file is not private: {0}")]
    InsecureTokenFile(PathBuf),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OAuthConfig {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub client_id: String,
    /// CIMD: an HTTPS metadata-document URL used as `client_id`
    /// (draft-ietf-oauth-client-id-metadata-document). When set, it overrides
    /// `client_id` and the document must declare the same URL.
    #[serde(default)]
    pub client_metadata_document: Option<String>,
    /// Expected RFC 9207 authorization-server issuer on the callback.
    #[serde(default)]
    pub issuer: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub resource: Option<String>,
    #[serde(default)]
    pub redirect_port: Option<u16>,
    /// Persist and reuse the token; highly recommended for non-interactive runs.
    #[serde(default)]
    pub token_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TokenSet {
    pub access_token: String,
    #[serde(default = "default_token_type")]
    pub token_type: String,
    #[serde(default)]
    pub expires_in: u64,
    #[serde(default)]
    pub refresh_token: Option<String>,
    pub expires_at_ms: u128,
}

fn default_token_type() -> String {
    "Bearer".into()
}

impl OAuthConfig {
    /// The client identifier presented to the authorization server. CIMD
    /// replaces a pre-registered id with a stable HTTPS metadata URL.
    pub fn effective_client_id(&self) -> Result<&str, OAuthError> {
        match &self.client_metadata_document {
            Some(document) => {
                validate_client_metadata_document(document)?;
                Ok(document)
            }
            None => Ok(&self.client_id),
        }
    }
}

/// CIMD client identifiers are HTTPS URLs with a path, no fragment and no
/// embedded credentials. The authorization server fetches the document and
/// checks that its `client_id` equals this URL.
pub fn validate_client_metadata_document(document: &str) -> Result<(), OAuthError> {
    let url = reqwest::Url::parse(document).map_err(|error| OAuthError::Url(error.to_string()))?;
    if url.scheme() != "https" {
        return Err(OAuthError::Url(
            "client_metadata_document must use https".into(),
        ));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(OAuthError::Url(
            "client_metadata_document must not embed credentials".into(),
        ));
    }
    if url.fragment().is_some() {
        return Err(OAuthError::Url(
            "client_metadata_document must not have a fragment".into(),
        ));
    }
    if url.host_str().is_none() || url.path() == "/" || url.path().is_empty() {
        return Err(OAuthError::Url(
            "client_metadata_document must have a host and a path component".into(),
        ));
    }
    Ok(())
}

fn check_issuer(expected: Option<&str>, actual: Option<&str>) -> Result<(), OAuthError> {
    let Some(expected) = expected else {
        return Ok(());
    };
    let Some(actual) = actual else {
        return Err(OAuthError::MissingIssuer);
    };
    if expected != actual {
        return Err(OAuthError::IssuerMismatch {
            expected: expected.into(),
            actual: actual.into(),
        });
    }
    Ok(())
}

impl TokenSet {
    pub fn is_expired(&self, now_ms: u128, skew_ms: u128) -> bool {
        self.expires_at_ms <= now_ms.saturating_add(skew_ms)
    }
}

/// RFC 7636 S256 challenge for a verifier.
pub fn pkce_challenge(verifier: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn random_token() -> String {
    Uuid::new_v4().simple().to_string() + &Uuid::new_v4().simple().to_string()
}

pub async fn access_token(config: &OAuthConfig) -> Result<TokenSet, OAuthError> {
    if let Some(path) = &config.token_file
        && let Some(mut token) = load_token(path)?
    {
        if !token.is_expired(now_ms(), 30_000) {
            return Ok(token);
        }
        if let Some(refresh) = token.refresh_token.clone() {
            match refresh_token(config, &refresh).await {
                Ok(new_token) => {
                    save_token(path, &new_token)?;
                    return Ok(new_token);
                }
                Err(_) => {
                    token.refresh_token = None;
                }
            }
        }
    }
    let token = authorize(config).await?;
    if let Some(path) = &config.token_file {
        save_token(path, &token)?;
    }
    Ok(token)
}

pub async fn authorize(config: &OAuthConfig) -> Result<TokenSet, OAuthError> {
    let listener = TcpListener::bind(("127.0.0.1", config.redirect_port.unwrap_or(0))).await?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");
    let verifier = random_token();
    let challenge = pkce_challenge(&verifier);
    let state = Uuid::new_v4().to_string();
    let client_id = config.effective_client_id()?.to_string();
    let mut url = reqwest::Url::parse(&config.authorization_endpoint)
        .map_err(|error| OAuthError::Url(error.to_string()))?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("response_type", "code");
        query.append_pair("client_id", &client_id);
        query.append_pair("redirect_uri", &redirect_uri);
        query.append_pair("state", &state);
        query.append_pair("code_challenge", &challenge);
        query.append_pair("code_challenge_method", "S256");
        if !config.scopes.is_empty() {
            query.append_pair("scope", &config.scopes.join(" "));
        }
        if let Some(resource) = &config.resource {
            query.append_pair("resource", resource);
        }
    }
    eprintln!("MCP OAuth: open this URL in a browser:\n{url}");
    try_open_browser(url.as_str());
    let code = wait_for_code(listener, &state, config.issuer.as_deref()).await?;
    exchange_code(config, &code, &redirect_uri, &verifier).await
}

async fn wait_for_code(
    listener: TcpListener,
    expected_state: &str,
    expected_issuer: Option<&str>,
) -> Result<String, OAuthError> {
    let accept = listener.accept();
    let (mut stream, _) = tokio::time::timeout(CALLBACK_TIMEOUT, accept)
        .await
        .map_err(|_| OAuthError::Response("OAuth callback timed out".into()))??;
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if buffer.len() > 64 * 1024 {
            return Err(OAuthError::Response("OAuth callback too large".into()));
        }
    }
    let request = String::from_utf8_lossy(&buffer);
    let request_line = request
        .lines()
        .next()
        .ok_or_else(|| OAuthError::Response("empty OAuth callback".into()))?;
    let target = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| OAuthError::Response("malformed OAuth callback".into()))?;
    let callback = reqwest::Url::parse(&format!("http://127.0.0.1{target}"))
        .map_err(|error| OAuthError::Url(error.to_string()))?;
    let mut code = None;
    let mut state = None;
    let mut issuer = None;
    let mut error = None;
    let mut error_description = None;
    for (key, value) in callback.query_pairs() {
        match key.as_ref() {
            "code" => code = Some(value.to_string()),
            "state" => state = Some(value.to_string()),
            "iss" => issuer = Some(value.to_string()),
            "error" => error = Some(value.to_string()),
            "error_description" => error_description = Some(value.to_string()),
            _ => {}
        }
    }
    let _ = stream
        .write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n<html><body>1Agent authorized. You can close this window.</body></html>",
        )
        .await;
    if state.as_deref() != Some(expected_state) {
        return Err(OAuthError::StateMismatch);
    }
    if let Some(error) = error {
        return Err(OAuthError::Response(match error_description {
            Some(description) => format!("authorization failed: {error}: {description}"),
            None => format!("authorization failed: {error}"),
        }));
    }
    check_issuer(expected_issuer, issuer.as_deref())?;
    code.ok_or(OAuthError::MissingCode)
}

async fn exchange_code(
    config: &OAuthConfig,
    code: &str,
    redirect_uri: &str,
    verifier: &str,
) -> Result<TokenSet, OAuthError> {
    let client_id = config.effective_client_id()?;
    let mut form = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", client_id),
        ("code_verifier", verifier),
    ];
    if let Some(resource) = &config.resource {
        form.push(("resource", resource));
    }
    token_request(config, &form, None).await
}

pub async fn refresh_token(
    config: &OAuthConfig,
    refresh_token: &str,
) -> Result<TokenSet, OAuthError> {
    let form = vec![
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", config.effective_client_id()?),
    ];
    token_request(config, &form, Some(refresh_token)).await
}

async fn token_request(
    config: &OAuthConfig,
    form: &[(&str, &str)],
    previous_refresh: Option<&str>,
) -> Result<TokenSet, OAuthError> {
    let client = reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| OAuthError::Http(error.to_string()))?;
    let response = client
        .post(&config.token_endpoint)
        .form(form)
        .send()
        .await
        .map_err(|error| OAuthError::Http(error.to_string()))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| OAuthError::Http(error.to_string()))?;
    if !status.is_success() {
        return Err(OAuthError::Response(format!(
            "token endpoint HTTP {status}: {body}"
        )));
    }
    #[derive(Deserialize)]
    struct WireToken {
        access_token: String,
        #[serde(default)]
        token_type: Option<String>,
        #[serde(default)]
        expires_in: Option<u64>,
        #[serde(default)]
        refresh_token: Option<String>,
    }
    let wire: WireToken = serde_json::from_str(&body)
        .map_err(|error| OAuthError::Response(format!("invalid token response: {error}")))?;
    Ok(TokenSet {
        access_token: wire.access_token,
        token_type: wire.token_type.unwrap_or_else(default_token_type),
        expires_in: wire.expires_in.unwrap_or(3600),
        refresh_token: wire
            .refresh_token
            .or_else(|| previous_refresh.map(str::to_string)),
        expires_at_ms: now_ms() + u128::from(wire.expires_in.unwrap_or(3600)) * 1_000,
    })
}

fn load_token(path: &Path) -> Result<Option<TokenSet>, OAuthError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !metadata.file_type().is_file() {
        return Err(OAuthError::InsecureTokenFile(path.to_path_buf()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(OAuthError::InsecureTokenFile(path.to_path_buf()));
        }
    }
    let bytes = std::fs::read(path)?;
    let token: TokenSet = serde_json::from_slice(&bytes)
        .map_err(|error| OAuthError::Response(format!("invalid token file: {error}")))?;
    Ok(Some(token))
}

fn save_token(path: &Path, token: &TokenSet) -> Result<(), OAuthError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    let bytes = serde_json::to_vec_pretty(token)
        .map_err(|error| OAuthError::Response(format!("cannot serialize token: {error}")))?;
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(&temporary, &bytes)?;
    }
    std::fs::rename(&temporary, path)?;
    Ok(())
}

fn try_open_browser(url: &str) {
    if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return;
    }
    let _ = std::process::Command::new("xdg-open")
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
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

    #[test]
    fn pkce_matches_the_rfc7636_vector() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            pkce_challenge(verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    fn config() -> OAuthConfig {
        OAuthConfig {
            authorization_endpoint: "https://as.example.com/authorize".into(),
            token_endpoint: "https://as.example.com/token".into(),
            client_id: "pre-registered".into(),
            client_metadata_document: None,
            issuer: None,
            scopes: vec![],
            resource: None,
            redirect_port: None,
            token_file: None,
        }
    }

    #[test]
    fn cimd_document_is_validated_and_used_as_client_id() {
        assert!(validate_client_metadata_document("https://app.example.com/client.json").is_ok());
        for bad in [
            "http://app.example.com/client.json",
            "https://app.example.com/",
            "https://user:pass@app.example.com/client.json",
            "https://app.example.com/client.json#fragment",
            "not a url",
        ] {
            assert!(
                validate_client_metadata_document(bad).is_err(),
                "accepted {bad}"
            );
        }
        let mut config = config();
        assert_eq!(config.effective_client_id().unwrap(), "pre-registered");
        config.client_metadata_document = Some("https://app.example.com/client.json".into());
        assert_eq!(
            config.effective_client_id().unwrap(),
            "https://app.example.com/client.json"
        );
        config.client_metadata_document = Some("http://app.example.com/client.json".into());
        assert!(config.effective_client_id().is_err());
    }

    #[test]
    fn issuer_check_accepts_absent_policy_or_exact_match() {
        assert!(check_issuer(None, None).is_ok());
        assert!(check_issuer(None, Some("https://as.example.com")).is_ok());
        assert!(
            check_issuer(
                Some("https://as.example.com"),
                Some("https://as.example.com")
            )
            .is_ok()
        );
        assert!(matches!(
            check_issuer(Some("https://as.example.com"), None),
            Err(OAuthError::MissingIssuer)
        ));
        assert!(matches!(
            check_issuer(Some("https://as.example.com"), Some("https://evil.example")),
            Err(OAuthError::IssuerMismatch { .. })
        ));
    }

    #[test]
    fn token_expiry_honors_skew() {
        let token = TokenSet {
            access_token: "x".into(),
            token_type: "Bearer".into(),
            expires_in: 60,
            refresh_token: None,
            expires_at_ms: 100_000,
        };
        assert!(!token.is_expired(50_000, 30_000));
        assert!(token.is_expired(80_000, 30_000));
    }
}
