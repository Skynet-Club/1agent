//! RFC 8693-style OAuth token exchange for MCP HTTP servers.
//!
//! The subject token and optional client secret are read only from named
//! environment variables. The returned access token is never logged; callers
//! write it to a private file or keep it in memory.

use std::net::IpAddr;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const DEFAULT_MAX_TTL_SECONDS: u64 = 3600;
const MAX_TTL_SECONDS: u64 = 86_400;
const FORM_GRANT: &str = "urn:ietf:params:oauth:grant-type:token-exchange";
const FORM_SUBJECT_TYPE: &str = "urn:ietf:params:oauth:token-type:access_token";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TokenExchangeConfig {
    pub token_endpoint: String,
    pub audience: String,
    pub scope: String,
    pub subject_token_env: String,
    #[serde(default)]
    pub client_id_env: Option<String>,
    #[serde(default)]
    pub client_secret_env: Option<String>,
    #[serde(default = "default_max_ttl_seconds")]
    pub max_ttl_seconds: u64,
}

fn default_max_ttl_seconds() -> u64 {
    DEFAULT_MAX_TTL_SECONDS
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExchangedToken {
    pub access_token: String,
    pub token_type: String,
    pub expires_in_seconds: u64,
}

#[derive(Debug, Error)]
pub enum TokenExchangeError {
    #[error("token exchange config is invalid: {0}")]
    InvalidConfig(String),
    #[error("token exchange environment variable {0} is not set or empty")]
    MissingEnv(String),
    #[error("token exchange request failed: {0}")]
    Http(String),
    #[error("token exchange response is invalid: {0}")]
    Response(String),
}

/// Exchanges a subject token for an audience-restricted MCP access token.
pub async fn exchange(config: &TokenExchangeConfig) -> Result<ExchangedToken, TokenExchangeError> {
    if config.audience.trim().is_empty() {
        return Err(TokenExchangeError::InvalidConfig(
            "audience must not be empty".into(),
        ));
    }
    if config.scope.trim().is_empty() {
        return Err(TokenExchangeError::InvalidConfig(
            "scope must not be empty".into(),
        ));
    }
    if config.max_ttl_seconds == 0 || config.max_ttl_seconds > MAX_TTL_SECONDS {
        return Err(TokenExchangeError::InvalidConfig(format!(
            "max_ttl_seconds must be 1..={MAX_TTL_SECONDS}"
        )));
    }
    if config.client_id_env.is_some() != config.client_secret_env.is_some() {
        return Err(TokenExchangeError::InvalidConfig(
            "client_id_env and client_secret_env are required together".into(),
        ));
    }
    let endpoint = reqwest::Url::parse(&config.token_endpoint)
        .map_err(|_| TokenExchangeError::InvalidConfig("invalid token endpoint".into()))?;
    if !endpoint.username().is_empty() || endpoint.password().is_some() {
        return Err(TokenExchangeError::InvalidConfig(
            "token endpoint must not embed credentials".into(),
        ));
    }
    let loopback = endpoint.host_str().is_some_and(|host| {
        host.eq_ignore_ascii_case("localhost")
            || host
                .parse::<IpAddr>()
                .map(|address| address.is_loopback())
                .unwrap_or(false)
    });
    if endpoint.scheme() != "https" && !(endpoint.scheme() == "http" && loopback) {
        return Err(TokenExchangeError::InvalidConfig(
            "token endpoint must use https (http is allowed only for loopback)".into(),
        ));
    }

    let subject_token = read_env(&config.subject_token_env)?;
    let client_id = config.client_id_env.as_deref().map(read_env).transpose()?;
    let client_secret = config
        .client_secret_env
        .as_deref()
        .map(read_env)
        .transpose()?;

    let mut form = vec![
        ("grant_type", FORM_GRANT.to_string()),
        ("subject_token", subject_token),
        ("subject_token_type", FORM_SUBJECT_TYPE.to_string()),
        ("audience", config.audience.trim().to_string()),
        ("scope", config.scope.trim().to_string()),
    ];
    if let (Some(client_id), Some(client_secret)) = (client_id, client_secret) {
        form.push(("client_id", client_id));
        form.push(("client_secret", client_secret));
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| TokenExchangeError::Http(error.without_url().to_string()))?;
    let response = client
        .post(endpoint)
        .form(&form)
        .send()
        .await
        .map_err(|error| TokenExchangeError::Http(error.without_url().to_string()))?;
    let status = response.status();
    if !status.is_success() {
        return Err(TokenExchangeError::Http(format!(
            "token endpoint returned status {}",
            status.as_u16()
        )));
    }
    let mut body = Vec::new();
    let mut response = response;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| TokenExchangeError::Http(error.without_url().to_string()))?
    {
        if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(TokenExchangeError::Response(
                "token response exceeds the size cap".into(),
            ));
        }
        body.extend_from_slice(&chunk);
    }
    let value: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|_| TokenExchangeError::Response("response is not JSON".into()))?;
    let access_token = value
        .get("access_token")
        .and_then(serde_json::Value::as_str)
        .filter(|token| !token.trim().is_empty())
        .ok_or_else(|| TokenExchangeError::Response("access_token is missing".into()))?;
    let token_type = value
        .get("token_type")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| TokenExchangeError::Response("token_type is missing".into()))?;
    if !token_type.eq_ignore_ascii_case("Bearer") {
        return Err(TokenExchangeError::Response(
            "token_type must be Bearer".into(),
        ));
    }
    let expires_in_seconds = value
        .get("expires_in")
        .and_then(serde_json::Value::as_u64)
        .filter(|seconds| *seconds > 0 && *seconds <= config.max_ttl_seconds)
        .ok_or_else(|| {
            TokenExchangeError::Response(format!(
                "expires_in must be 1..={} seconds",
                config.max_ttl_seconds
            ))
        })?;
    Ok(ExchangedToken {
        access_token: access_token.to_string(),
        token_type: token_type.to_string(),
        expires_in_seconds,
    })
}

fn read_env(variable: &str) -> Result<String, TokenExchangeError> {
    if variable.trim().is_empty() {
        return Err(TokenExchangeError::InvalidConfig(
            "environment variable name is empty".into(),
        ));
    }
    let value =
        std::env::var(variable).map_err(|_| TokenExchangeError::MissingEnv(variable.into()))?;
    if value.trim().is_empty() {
        return Err(TokenExchangeError::MissingEnv(variable.into()));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> TokenExchangeConfig {
        TokenExchangeConfig {
            token_endpoint: "http://127.0.0.1:9/token".into(),
            audience: "mcp://server".into(),
            scope: "tools.read".into(),
            subject_token_env: "SUBJECT".into(),
            client_id_env: None,
            client_secret_env: None,
            max_ttl_seconds: 60,
        }
    }

    #[tokio::test]
    async fn invalid_configs_are_rejected_before_the_network() {
        let mut empty_audience = config();
        empty_audience.audience.clear();
        assert!(matches!(
            exchange(&empty_audience).await,
            Err(TokenExchangeError::InvalidConfig(_))
        ));

        let mut empty_scope = config();
        empty_scope.scope.clear();
        assert!(matches!(
            exchange(&empty_scope).await,
            Err(TokenExchangeError::InvalidConfig(_))
        ));

        let mut zero_ttl = config();
        zero_ttl.max_ttl_seconds = 0;
        assert!(matches!(
            exchange(&zero_ttl).await,
            Err(TokenExchangeError::InvalidConfig(_))
        ));

        let mut partial_client = config();
        partial_client.client_id_env = Some("CLIENT".into());
        assert!(matches!(
            exchange(&partial_client).await,
            Err(TokenExchangeError::InvalidConfig(_))
        ));

        let insecure = TokenExchangeConfig {
            token_endpoint: "http://example.com/token".into(),
            ..config()
        };
        assert!(matches!(
            exchange(&insecure).await,
            Err(TokenExchangeError::InvalidConfig(_))
        ));
    }

    #[test]
    fn environment_names_and_values_fail_closed() {
        assert!(matches!(
            read_env(""),
            Err(TokenExchangeError::InvalidConfig(_))
        ));
        assert!(matches!(
            read_env("MISSING_TOKEN_EXCHANGE_VALUE_XYZ"),
            Err(TokenExchangeError::MissingEnv(_))
        ));
    }
}
