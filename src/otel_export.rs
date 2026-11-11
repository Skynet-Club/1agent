//! OTLP/HTTP JSON export for the journal-derived OTel span projection.
//!
//! This builds the OTLP `resourceSpans` envelope and posts it to a caller's
//! collector. It is a small wire adapter, not an OTel SDK.

use std::net::IpAddr;
use std::time::Duration;

use serde_json::{Value, json};
use thiserror::Error;

use crate::otel::{OtelSpan, SpanKind, SpanStatus};

#[derive(Debug, Error)]
pub enum OtelExportError {
    #[error("OTLP endpoint is invalid: {0}")]
    InvalidEndpoint(String),
    #[error("OTLP token environment variable {0} is not set")]
    MissingToken(String),
    #[error("OTLP export failed: {0}")]
    Http(String),
}

pub fn otlp_json(spans: &[OtelSpan], service_name: &str) -> Value {
    json!({
        "resourceSpans": [{
            "resource": {
                "attributes": [{
                    "key": "service.name",
                    "value": {"stringValue": service_name},
                }],
            },
            "scopeSpans": [{
                "scope": {
                    "name": "1agent",
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "spans": spans.iter().map(span_json).collect::<Vec<_>>(),
            }],
        }],
    })
}

fn span_json(span: &OtelSpan) -> Value {
    let mut value = json!({
        "traceId": span.trace_id,
        "spanId": span.span_id,
        "name": span.name,
        "kind": match span.kind {
            SpanKind::Internal => 1,
            SpanKind::Client => 3,
        },
        "startTimeUnixNano": span.start_unix_nano.to_string(),
        "endTimeUnixNano": span.end_unix_nano.to_string(),
        "attributes": span.attributes.iter().map(|(key, value)| {
            json!({"key": key, "value": {"stringValue": value}})
        }).collect::<Vec<_>>(),
    });
    if let Some(parent) = &span.parent_span_id {
        value["parentSpanId"] = json!(parent);
    }
    match span.status {
        SpanStatus::Unset => {}
        SpanStatus::Ok => value["status"] = json!({"code": 1}),
        SpanStatus::Error => value["status"] = json!({"code": 2}),
    }
    value
}

/// Posts the OTLP JSON envelope. The endpoint must be HTTPS or loopback HTTP,
/// must not embed credentials, and redirects are not followed. A bearer token
/// is read only from `token_env`; it is never logged or returned.
pub async fn export_otlp(
    endpoint: &str,
    token_env: Option<&str>,
    payload: &Value,
) -> Result<u16, OtelExportError> {
    let parsed = reqwest::Url::parse(endpoint)
        .map_err(|_| OtelExportError::InvalidEndpoint("invalid URL".into()))?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(OtelExportError::InvalidEndpoint(
            "URL must not embed credentials".into(),
        ));
    }
    let loopback = parsed.host_str().is_some_and(|host| {
        host.eq_ignore_ascii_case("localhost")
            || host
                .parse::<IpAddr>()
                .map(|address| address.is_loopback())
                .unwrap_or(false)
    });
    if parsed.scheme() != "https" && !(parsed.scheme() == "http" && loopback) {
        return Err(OtelExportError::InvalidEndpoint(
            "endpoint must use https (http is allowed only for loopback)".into(),
        ));
    }
    let token = match token_env {
        Some(variable) => {
            let value = std::env::var(variable)
                .map_err(|_| OtelExportError::MissingToken(variable.into()))?;
            if value.trim().is_empty() {
                return Err(OtelExportError::MissingToken(variable.into()));
            }
            Some(value)
        }
        None => None,
    };
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| OtelExportError::Http(error.without_url().to_string()))?;
    let mut request = client.post(parsed).json(payload);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request
        .send()
        .await
        .map_err(|error| OtelExportError::Http(error.without_url().to_string()))?;
    if !response.status().is_success() {
        return Err(OtelExportError::Http(format!(
            "collector returned status {}",
            response.status().as_u16()
        )));
    }
    Ok(response.status().as_u16())
}
