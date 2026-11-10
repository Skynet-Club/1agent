//! Read-only web fetching for public HTTPS content.
//!
//! The tool blocks non-HTTPS URLs, private/loopback/link-local addresses,
//! non-text content and responses over 2 MiB. Redirects are followed manually
//! so every hop is revalidated and pinned to a public address.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use async_trait::async_trait;
use reqwest::header::{CONTENT_TYPE, LOCATION};
use reqwest::{Client, Url};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use super::{AgentTool, ToolError, ToolOutcome};
use crate::domain::{PermissionSet, RiskLevel, ToolSpec};

const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_TEXT_CHARS: usize = 128 * 1024;
const MAX_REDIRECTS: usize = 5;
const TIMEOUT: Duration = Duration::from_secs(30);
const USER_AGENT: &str = "1agent/0.95 (read-only fetch)";

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => is_public_ipv6(ip),
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    !ip.is_private()
        && !ip.is_loopback()
        && !ip.is_link_local()
        && !ip.is_broadcast()
        && !ip.is_documentation()
        && !ip.is_unspecified()
        && !ip.is_multicast()
        && !(octets[0] == 100 && (64..=127).contains(&octets[1]))
        && !(octets[0] == 198 && matches!(octets[1], 18 | 19))
        && octets[0] < 240
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    if ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || ip.is_unique_local()
        || ip.is_unicast_link_local()
    {
        return false;
    }
    let segments = ip.segments();
    if segments[0] == 0x2001 && segments[1] == 0x0db8 {
        return false;
    }
    // IPv4-mapped addresses (::ffff:a.b.c.d) follow IPv4 rules.
    if segments[0..5] == [0, 0, 0, 0, 0] && segments[5] == 0xffff {
        let v4 = Ipv4Addr::new(
            (segments[6] >> 8) as u8,
            segments[6] as u8,
            (segments[7] >> 8) as u8,
            segments[7] as u8,
        );
        return is_public_ipv4(v4);
    }
    true
}

pub(crate) fn validate_url(url: &Url) -> Result<(), ToolError> {
    if url.scheme() != "https" {
        return Err(ToolError::InvalidArguments(
            "only https:// URLs are allowed".into(),
        ));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(ToolError::InvalidArguments(
            "URLs with embedded credentials are not allowed".into(),
        ));
    }
    if url.host_str().is_none() {
        return Err(ToolError::InvalidArguments("URL has no host".into()));
    }
    Ok(())
}

pub(crate) async fn resolve_public(url: &Url) -> Result<(String, IpAddr), ToolError> {
    let host = url
        .host_str()
        .ok_or_else(|| ToolError::InvalidArguments("URL has no host".into()))?
        .to_string();
    let port = url.port_or_known_default().unwrap_or(443);
    if let Ok(ip) = host.parse::<IpAddr>() {
        if !is_public_ip(ip) {
            return Err(ToolError::PermissionDenied(
                "URL resolves to a private or reserved address".into(),
            ));
        }
        return Ok((host, ip));
    }
    let addresses = tokio::net::lookup_host((host.as_str(), port))
        .await
        .map_err(|error| ToolError::Failed(format!("cannot resolve {host}: {error}")))?
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err(ToolError::Failed(format!(
            "{host} resolved to no addresses"
        )));
    }
    if addresses.iter().any(|address| !is_public_ip(address.ip())) {
        return Err(ToolError::PermissionDenied(
            "URL resolves to a private or reserved address".into(),
        ));
    }
    Ok((host, addresses[0].ip()))
}

fn is_text_content_type(content_type: &str) -> bool {
    let content_type = content_type.to_ascii_lowercase();
    let base = content_type.split(';').next().unwrap_or("").trim();
    base.starts_with("text/")
        || base == "application/json"
        || base == "application/xml"
        || base == "application/xhtml+xml"
        || base.ends_with("+json")
        || base.ends_with("+xml")
}

async fn read_body(mut response: reqwest::Response) -> Result<(Vec<u8>, bool), ToolError> {
    let mut body = Vec::new();
    let mut truncated = false;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| ToolError::Failed(format!("cannot read response body: {error}")))?
    {
        if body.len() + chunk.len() > MAX_BYTES {
            let take = MAX_BYTES - body.len();
            body.extend_from_slice(&chunk[..take]);
            truncated = true;
            break;
        }
        body.extend_from_slice(&chunk);
    }
    Ok((body, truncated))
}

struct FetchedPage {
    final_url: Url,
    status: reqwest::StatusCode,
    content_type: String,
    body: Vec<u8>,
    truncated: bool,
}

async fn fetch_public(mut current: Url) -> Result<FetchedPage, ToolError> {
    for _ in 0..=MAX_REDIRECTS {
        validate_url(&current)?;
        let (host, ip) = resolve_public(&current).await?;
        let port = current.port_or_known_default().unwrap_or(443);
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(TIMEOUT)
            .user_agent(USER_AGENT)
            .resolve(&host, SocketAddr::new(ip, port))
            .build()
            .map_err(|error| ToolError::Failed(format!("cannot build HTTP client: {error}")))?;
        let response = client
            .get(current.clone())
            .send()
            .await
            .map_err(|error| ToolError::Failed(format!("fetch failed: {error}")))?;
        let status = response.status();
        if status.is_redirection() {
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| ToolError::Failed("redirect response has no Location".into()))?;
            current = current.join(location).map_err(|error| {
                ToolError::Failed(format!("invalid redirect location: {error}"))
            })?;
            continue;
        }
        if !status.is_success() {
            return Err(ToolError::Failed(format!("HTTP {status} for {current}")));
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("text/plain")
            .to_string();
        let (body, truncated) = read_body(response).await?;
        return Ok(FetchedPage {
            final_url: current,
            status,
            content_type,
            body,
            truncated,
        });
    }
    Err(ToolError::Failed("too many redirects".into()))
}

fn decode_entities(text: &str) -> String {
    let mut decoded = text
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ");
    // Numeric entities.
    let mut output = String::with_capacity(decoded.len());
    let mut chars = decoded.char_indices().peekable();
    while let Some((index, character)) = chars.next() {
        if character != '&' {
            output.push(character);
            continue;
        }
        let rest = &decoded[index + 1..];
        if let Some(end) = rest.find(';')
            && end <= 8
        {
            let entity = &rest[..end];
            let value = if let Some(hex) = entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
            {
                u32::from_str_radix(hex, 16).ok()
            } else if let Some(decimal) = entity.strip_prefix('#') {
                decimal.parse::<u32>().ok()
            } else {
                None
            };
            if let Some(value) = value
                && let Some(character) = char::from_u32(value)
            {
                output.push(character);
                for _ in 0..=end {
                    chars.next();
                }
                continue;
            }
        }
        output.push(character);
    }
    decoded = output.replace("&amp;", "&");
    decoded
}

fn strip_block(html: &str, tag: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let mut output = String::with_capacity(html.len());
    let mut cursor = 0;
    while let Some(relative_start) = lower[cursor..].find(&format!("<{tag}")) {
        let start = cursor + relative_start;
        output.push_str(&html[cursor..start]);
        let Some(relative_end) = lower[start..].find(&format!("</{tag}>")) else {
            cursor = html.len();
            break;
        };
        cursor = start + relative_end + tag.len() + 3;
    }
    output.push_str(&html[cursor..]);
    output
}

pub(crate) fn html_to_text(html: &str) -> String {
    let mut text = strip_block(html, "script");
    text = strip_block(&text, "style");
    while let Some(start) = text.find("<!--") {
        let Some(end) = text[start + 4..].find("-->") else {
            text.truncate(start);
            break;
        };
        text.replace_range(start..start + 4 + end + 3, "");
    }

    let block_tags = [
        "p",
        "div",
        "br",
        "li",
        "tr",
        "td",
        "th",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "section",
        "article",
        "header",
        "footer",
        "pre",
        "blockquote",
    ];
    let mut output = String::with_capacity(text.len());
    let mut in_tag = false;
    let mut tag = String::new();
    for character in text.chars() {
        if in_tag {
            if character == '>' {
                let name = tag
                    .trim()
                    .trim_start_matches('/')
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if block_tags.contains(&name.as_str()) {
                    output.push('\n');
                }
                in_tag = false;
                tag.clear();
            } else {
                tag.push(character);
            }
            continue;
        }
        if character == '<' {
            in_tag = true;
            continue;
        }
        output.push(character);
    }

    let decoded = decode_entities(&output);
    decoded
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn bounded(text: &str, max_chars: usize) -> (String, bool) {
    let mut bounded: String = text.chars().take(max_chars).collect();
    let truncated = text.chars().nth(max_chars).is_some();
    if truncated {
        bounded.push('…');
    }
    (bounded, truncated)
}

pub struct FetchUrlTool;

#[async_trait]
impl AgentTool for FetchUrlTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "fetch_url".into(),
            description: "Fetch a public HTTPS URL and return bounded readable text. Private, loopback and link-local addresses are rejected; redirects are limited to 5 and revalidated; non-text content and responses over 2 MiB are refused. Requires network permission (--allow-network).".into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["url"],
                "properties": {
                    "url": {
                        "type": "string",
                        "maxLength": 4096,
                        "description": "Public https:// URL."
                    },
                    "max_chars": {
                        "type": "integer",
                        "minimum": 1000,
                        "maximum": MAX_TEXT_CHARS,
                        "description": "Maximum characters of extracted text, default 131072."
                    }
                }
            }),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::Medium
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(45)
    }

    async fn execute(
        &self,
        arguments: Value,
        permissions: &PermissionSet,
        _cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        if !permissions.allow_network {
            return Err(ToolError::PermissionDenied(
                "network permission is disabled; pass --allow-network".into(),
            ));
        }
        let raw_url = arguments
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::InvalidArguments("url is required".into()))?;
        let max_chars = arguments
            .get("max_chars")
            .and_then(Value::as_u64)
            .unwrap_or(MAX_TEXT_CHARS as u64)
            .clamp(1000, MAX_TEXT_CHARS as u64) as usize;

        let current = Url::parse(raw_url)
            .map_err(|error| ToolError::InvalidArguments(format!("invalid URL: {error}")))?;
        let page = fetch_public(current).await?;
        if !is_text_content_type(&page.content_type) {
            return Err(ToolError::Failed(format!(
                "refusing non-text content type {}",
                page.content_type
            )));
        }
        let body_text = String::from_utf8_lossy(&page.body);
        let is_html = page.content_type.to_ascii_lowercase().contains("html");
        let extracted = if is_html {
            html_to_text(&body_text)
        } else {
            body_text.to_string()
        };
        let (text, text_truncated) = bounded(&extracted, max_chars);
        let mut metadata = std::collections::BTreeMap::new();
        metadata.insert("final_url".into(), page.final_url.to_string());
        metadata.insert("status".into(), page.status.as_u16().to_string());
        metadata.insert("content_type".into(), page.content_type.clone());
        metadata.insert("bytes".into(), page.body.len().to_string());
        metadata.insert(
            "truncated".into(),
            (page.truncated || text_truncated).to_string(),
        );
        let summary = format!(
            "已抓取 {}（HTTP {}，{} 字符{}）",
            page.final_url,
            page.status.as_u16(),
            text.chars().count(),
            if page.truncated || text_truncated {
                "，已截断"
            } else {
                ""
            }
        );
        Ok(ToolOutcome {
            summary,
            content: text,
            metadata,
        })
    }
}

const SEARCH_ENDPOINT: &str = "https://www.bing.com/search";
const MAX_RESULTS: usize = 8;
const MAX_QUERY_CHARS: usize = 500;

#[derive(Debug, PartialEq, Eq)]
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

fn xml_value(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = block.find(&open)? + open.len();
    let end = block[start..].find(&close)? + start;
    Some(html_to_text(&block[start..end]).trim().to_string())
}

fn parse_bing_rss(xml: &str) -> Vec<SearchResult> {
    let lower = xml.to_ascii_lowercase();
    let mut results = Vec::new();
    let mut cursor = 0;
    while results.len() < MAX_RESULTS {
        let Some(relative_start) = lower[cursor..].find("<item>") else {
            break;
        };
        let start = cursor + relative_start;
        let Some(relative_end) = lower[start..].find("</item>") else {
            break;
        };
        let end = start + relative_end + "</item>".len();
        let block = &xml[start..end];
        let title = xml_value(block, "title").unwrap_or_default();
        let url = xml_value(block, "link").unwrap_or_default();
        let snippet = xml_value(block, "description").unwrap_or_default();
        if !title.is_empty()
            && (url.starts_with("https://") || url.starts_with("http://"))
            && !results
                .iter()
                .any(|result: &SearchResult| result.url == url)
        {
            results.push(SearchResult {
                title: bounded(&title, 200).0,
                url,
                snippet: bounded(&snippet, 400).0,
            });
        }
        cursor = end;
    }
    results
}

/// Best-effort public web search using Bing's RSS endpoint.
pub struct WebSearchTool;

#[async_trait]
impl AgentTool for WebSearchTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "web_search".into(),
            description: "Search the public web and return up to 8 results with title, URL and snippet. Uses Bing's RSS endpoint as a best-effort provider; results may be rate-limited or localized. Requires network permission (--allow-network).".into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["query"],
                "properties": {
                    "query": {
                        "type": "string",
                        "maxLength": MAX_QUERY_CHARS,
                        "description": "Search query."
                    }
                }
            }),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::Medium
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(45)
    }

    async fn execute(
        &self,
        arguments: Value,
        permissions: &PermissionSet,
        _cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        if !permissions.allow_network {
            return Err(ToolError::PermissionDenied(
                "network permission is disabled; pass --allow-network".into(),
            ));
        }
        let query = arguments
            .get("query")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::InvalidArguments("query is required".into()))?;
        let query = query.trim();
        if query.is_empty() {
            return Err(ToolError::InvalidArguments(
                "query must not be empty".into(),
            ));
        }
        if query.chars().count() > MAX_QUERY_CHARS {
            return Err(ToolError::InvalidArguments(format!(
                "query must be at most {MAX_QUERY_CHARS} characters"
            )));
        }
        if crate::safety::contains_disallowed_control(query) {
            return Err(ToolError::InvalidArguments(
                "query contains hidden or disallowed control characters".into(),
            ));
        }

        let mut url = Url::parse(SEARCH_ENDPOINT)
            .map_err(|error| ToolError::Failed(format!("invalid search endpoint: {error}")))?;
        url.query_pairs_mut()
            .append_pair("q", query)
            .append_pair("format", "rss")
            .append_pair("setlang", "en");
        let page = fetch_public(url).await?;
        if !is_text_content_type(&page.content_type) {
            return Err(ToolError::Failed(format!(
                "search provider returned non-text content type {}",
                page.content_type
            )));
        }
        let xml = String::from_utf8_lossy(&page.body);
        let results = parse_bing_rss(&xml);
        if results.is_empty() {
            return Err(ToolError::Failed(
                "search returned no parseable results; the provider may be rate-limiting".into(),
            ));
        }

        let content = json!({
            "query": query,
            "provider": "bing-rss",
            "results": results.iter().map(|result| json!({
                "title": result.title,
                "url": result.url,
                "snippet": result.snippet
            })).collect::<Vec<_>>()
        });
        let mut metadata = std::collections::BTreeMap::new();
        metadata.insert("provider".into(), "bing-rss".into());
        metadata.insert("results".into(), results.len().to_string());
        Ok(ToolOutcome {
            summary: format!("网页搜索完成：{} 条结果", results.len()),
            content: content.to_string(),
            metadata,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_private_and_reserved_addresses() {
        for ip in [
            "127.0.0.1",
            "10.0.0.1",
            "192.168.1.1",
            "169.254.1.1",
            "100.64.0.1",
            "198.18.0.1",
            "0.0.0.0",
            "224.0.0.1",
            "255.255.255.255",
            "::1",
            "fc00::1",
            "fe80::1",
            "2001:db8::1",
            "::ffff:10.0.0.1",
        ] {
            let ip: IpAddr = ip.parse().unwrap();
            assert!(!is_public_ip(ip), "{ip}");
        }
        for ip in ["1.1.1.1", "8.8.8.8", "2606:4700:4700::1111"] {
            let ip: IpAddr = ip.parse().unwrap();
            assert!(is_public_ip(ip), "{ip}");
        }
    }

    #[test]
    fn validates_https_and_rejects_credentials() {
        assert!(validate_url(&Url::parse("https://example.com/a").unwrap()).is_ok());
        assert!(validate_url(&Url::parse("http://example.com/a").unwrap()).is_err());
        assert!(validate_url(&Url::parse("https://user:pass@example.com/a").unwrap()).is_err());
    }

    #[test]
    fn html_to_text_strips_scripts_styles_and_entities() {
        let html = "<html><head><style>body{color:red}</style><script>alert(1)</script></head><body><h1>Title</h1><p>Hello &amp; welcome.</p><div>Line&nbsp;two</div></body></html>";
        let text = html_to_text(html);
        assert!(text.contains("Title"));
        assert!(text.contains("Hello & welcome."));
        assert!(text.contains("Line two"));
        assert!(!text.contains("alert"));
        assert!(!text.contains("color"));
        assert!(!text.contains('<'));
    }

    #[tokio::test]
    async fn network_permission_is_required_before_fetching() {
        let error = FetchUrlTool
            .execute(
                json!({"url": "https://example.com"}),
                &PermissionSet::default(),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::PermissionDenied(_)));
    }

    #[tokio::test]
    async fn http_urls_are_rejected() {
        let permissions = PermissionSet {
            allow_network: true,
            ..PermissionSet::default()
        };
        let error = FetchUrlTool
            .execute(
                json!({"url": "http://example.com"}),
                &permissions,
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::InvalidArguments(_)));
    }

    #[tokio::test]
    async fn private_hosts_are_rejected() {
        let permissions = PermissionSet {
            allow_network: true,
            ..PermissionSet::default()
        };
        let error = FetchUrlTool
            .execute(
                json!({"url": "https://127.0.0.1/"}),
                &permissions,
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::PermissionDenied(_)));
    }

    #[test]
    fn parses_bing_rss_results_and_entities() {
        let xml = r#"<?xml version="1.0"?><rss><channel>
        <item><title>Example &amp; Site</title><link>https://example.com/</link><description>An example <b>snippet</b>.</description></item>
        <item><title>Rust</title><link>https://www.rust-lang.org/</link><description>The Rust language.</description></item>
        </channel></rss>"#;
        let results = parse_bing_rss(xml);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Example & Site");
        assert_eq!(results[0].url, "https://example.com/");
        assert_eq!(results[0].snippet, "An example snippet.");
        assert_eq!(results[1].title, "Rust");
        assert_eq!(results[1].url, "https://www.rust-lang.org/");
    }

    #[tokio::test]
    async fn search_rejects_empty_queries() {
        let permissions = PermissionSet {
            allow_network: true,
            ..PermissionSet::default()
        };
        let error = WebSearchTool
            .execute(
                json!({"query": "  "}),
                &permissions,
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::InvalidArguments(_)));
    }

    #[tokio::test]
    async fn search_requires_network_permission() {
        let error = WebSearchTool
            .execute(
                json!({"query": "rust"}),
                &PermissionSet::default(),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::PermissionDenied(_)));
    }
}
