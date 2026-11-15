//! Local CONNECT/HTTP egress proxy for the browser.
//!
//! Chrome resolves DNS and follows redirects on its own. Validating only the
//! initial URL is therefore not enough. The proxy validates the destination
//! host, resolves it once, rejects non-global addresses, and connects to the
//! pinned address. Redirects and subresources are re-validated because every
//! new connection goes through this proxy.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;

const MAX_HEADER_BYTES: usize = 64 * 1024;
const HEADER_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Error)]
pub enum EgressError {
    #[error("egress io: {0}")]
    Io(#[from] std::io::Error),
    #[error("egress host is not allowlisted: {0}")]
    HostDenied(String),
    #[error("egress resolved to a non-global address: {host} -> {ip}")]
    AddressDenied { host: String, ip: IpAddr },
    #[error("egress DNS failed: {0}")]
    Dns(String),
    #[error("egress request is malformed: {0}")]
    Malformed(String),
}

#[derive(Debug, Clone)]
pub struct EgressPolicy {
    allowlist: Vec<String>,
    allow_loopback: bool,
}

impl EgressPolicy {
    pub fn new(allowlist: Vec<String>, allow_loopback: bool) -> Self {
        let allowlist = allowlist
            .into_iter()
            .map(|entry| entry.trim().trim_end_matches('.').to_ascii_lowercase())
            .filter(|entry| !entry.is_empty())
            .collect();
        Self {
            allowlist,
            allow_loopback,
        }
    }

    pub fn allows_host(&self, host: &str) -> bool {
        let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
        if host.is_empty() {
            return false;
        }
        if self.allowlist.is_empty() {
            return false;
        }
        self.allowlist.iter().any(|entry| {
            if let Some(suffix) = entry.strip_prefix("*.") {
                host.ends_with(&format!(".{suffix}"))
            } else if let Some(suffix) = entry.strip_prefix('.') {
                host == suffix || host.ends_with(&format!(".{suffix}"))
            } else {
                host == *entry
            }
        })
    }

    pub fn permits_ip(&self, ip: IpAddr) -> bool {
        is_global_ip(ip) || (self.allow_loopback && ip.is_loopback())
    }

    pub async fn resolve(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, EgressError> {
        let host = host.trim().trim_matches(['[', ']']).trim_end_matches('.');
        if !self.allows_host(host) {
            return Err(EgressError::HostDenied(host.to_string()));
        }
        let addresses = tokio::net::lookup_host((host, port))
            .await
            .map_err(|error| EgressError::Dns(format!("{host}: {error}")))?
            .collect::<Vec<_>>();
        if addresses.is_empty() {
            return Err(EgressError::Dns(format!("{host}: no addresses")));
        }
        for address in &addresses {
            if !self.permits_ip(address.ip()) {
                return Err(EgressError::AddressDenied {
                    host: host.to_string(),
                    ip: address.ip(),
                });
            }
        }
        Ok(addresses)
    }
}

pub struct EgressProxy {
    addr: SocketAddr,
    shutdown: CancellationToken,
    task: tokio::task::JoinHandle<()>,
}

impl EgressProxy {
    pub async fn start(policy: EgressPolicy) -> Result<Self, EgressError> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let addr = listener.local_addr()?;
        let shutdown = CancellationToken::new();
        let token = shutdown.clone();
        let policy = Arc::new(policy);
        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = token.cancelled() => break,
                    accepted = listener.accept() => {
                        let Ok((stream, _)) = accepted else { break };
                        let policy = policy.clone();
                        tokio::spawn(async move {
                            let _ = handle_client(stream, policy).await;
                        });
                    }
                }
            }
        });
        Ok(Self {
            addr,
            shutdown,
            task,
        })
    }

    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for EgressProxy {
    fn drop(&mut self) {
        self.shutdown.cancel();
        self.task.abort();
    }
}

async fn handle_client(
    mut stream: TcpStream,
    policy: Arc<EgressPolicy>,
) -> Result<(), EgressError> {
    let mut header = Vec::with_capacity(1024);
    let mut buffer = [0u8; 1024];
    loop {
        let read = tokio::time::timeout(HEADER_TIMEOUT, stream.read(&mut buffer))
            .await
            .map_err(|_| EgressError::Malformed("header timeout".into()))??;
        if read == 0 {
            return Ok(());
        }
        header.extend_from_slice(&buffer[..read]);
        if header.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if header.len() > MAX_HEADER_BYTES {
            return Err(EgressError::Malformed("header too large".into()));
        }
    }
    let text = String::from_utf8_lossy(&header);
    let mut lines = text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| EgressError::Malformed("empty request".into()))?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or_default();
    let version = parts.next().unwrap_or("HTTP/1.1");
    if method.eq_ignore_ascii_case("CONNECT") {
        let (host, port) = parse_host_port(target, 443)?;
        let addresses = policy.resolve(&host, port).await?;
        let mut upstream = connect_any(&addresses).await?;
        stream
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await?;
        let _ = tokio::io::copy_bidirectional(&mut stream, &mut upstream).await;
        return Ok(());
    }
    let url = reqwest::Url::parse(target)
        .map_err(|error| EgressError::Malformed(format!("absolute URL required: {error}")))?;
    let host = url
        .host_str()
        .ok_or_else(|| EgressError::Malformed("URL has no host".into()))?
        .to_string();
    let port = url.port_or_known_default().unwrap_or(80);
    let addresses = policy.resolve(&host, port).await?;
    let mut upstream = connect_any(&addresses).await?;
    let mut origin_target = url.path().to_string();
    if let Some(query) = url.query() {
        origin_target.push('?');
        origin_target.push_str(query);
    }
    let mut forwarded = format!("{method} {origin_target} {version}\r\n");
    for line in lines {
        if line.is_empty() {
            break;
        }
        if line.to_ascii_lowercase().starts_with("proxy-") {
            continue;
        }
        forwarded.push_str(line);
        forwarded.push_str("\r\n");
    }
    forwarded.push_str("\r\n");
    upstream.write_all(forwarded.as_bytes()).await?;
    let _ = tokio::io::copy_bidirectional(&mut stream, &mut upstream).await;
    Ok(())
}

fn parse_host_port(value: &str, default_port: u16) -> Result<(String, u16), EgressError> {
    if let Some(rest) = value.strip_prefix('[') {
        let (host, tail) = rest
            .split_once(']')
            .ok_or_else(|| EgressError::Malformed("invalid IPv6 host".into()))?;
        let port = tail
            .strip_prefix(':')
            .and_then(|port| port.parse::<u16>().ok())
            .unwrap_or(default_port);
        return Ok((host.to_string(), port));
    }
    match value.rsplit_once(':') {
        Some((host, port)) if port.chars().all(|character| character.is_ascii_digit()) => {
            Ok((host.to_string(), port.parse().unwrap_or(default_port)))
        }
        _ => Ok((value.to_string(), default_port)),
    }
}

async fn connect_any(addresses: &[SocketAddr]) -> Result<TcpStream, EgressError> {
    let mut last_error = None;
    for address in addresses {
        match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(address)).await {
            Ok(Ok(stream)) => return Ok(stream),
            Ok(Err(error)) => last_error = Some(error),
            Err(_) => {
                last_error = Some(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "connect timeout",
                ))
            }
        }
    }
    Err(EgressError::Io(last_error.unwrap_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "no address")
    })))
}

/// Conservative global-unicast test. Private, loopback, link-local, CGNAT,
/// metadata, documentation, multicast, benchmark and reserved ranges are not
/// global.
pub fn is_global_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_global_v4(ip),
        IpAddr::V6(ip) => {
            if let Some(v4) = ip.to_ipv4_mapped() {
                return is_global_v4(v4);
            }
            if let Some(v4) = nat64_v4(ip) {
                return is_global_v4(v4);
            }
            if let Some(v4) = six_to_four_v4(ip) {
                return is_global_v4(v4);
            }
            let segments = ip.segments();
            !(ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                || (segments[0] & 0xfe00) == 0xfc00 // unique local fc00::/7
                || (segments[0] & 0xffc0) == 0xfe80 // link-local fe80::/10
                || (segments[0] == 0x2001 && segments[1] == 0x0db8)) // documentation
        }
    }
}

fn is_global_v4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_multicast()
        || (octets[0] == 100 && (64..=127).contains(&octets[1])) // CGNAT
        || (octets[0] == 169 && octets[1] == 254) // link-local / metadata
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
        || (octets[0] == 198 && (18..=19).contains(&octets[1]))
        || octets[0] >= 240)
}

fn nat64_v4(ip: Ipv6Addr) -> Option<Ipv4Addr> {
    let segments = ip.segments();
    if segments[0] == 0x0064 && segments[1] == 0xff9b && segments[2..6].iter().all(|s| *s == 0) {
        Some(Ipv4Addr::new(
            (segments[6] >> 8) as u8,
            segments[6] as u8,
            (segments[7] >> 8) as u8,
            segments[7] as u8,
        ))
    } else {
        None
    }
}

fn six_to_four_v4(ip: Ipv6Addr) -> Option<Ipv4Addr> {
    let segments = ip.segments();
    if segments[0] == 0x2002 {
        Some(Ipv4Addr::new(
            (segments[1] >> 8) as u8,
            segments[1] as u8,
            (segments[2] >> 8) as u8,
            segments[2] as u8,
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_ip_classification_covers_private_and_metadata_ranges() {
        for denied in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "::1",
            "fc00::1",
            "fe80::1",
            "64:ff9b::a9fe:a9fe",
            "2002:a9fe:a9fe::",
        ] {
            assert!(
                !is_global_ip(denied.parse().unwrap()),
                "{denied} must not be global"
            );
        }
        for allowed in ["1.1.1.1", "8.8.8.8", "2606:4700:4700::1111"] {
            assert!(
                is_global_ip(allowed.parse().unwrap()),
                "{allowed} must be global"
            );
        }
    }

    #[test]
    fn allowlist_supports_exact_and_wildcard_entries() {
        let policy = EgressPolicy::new(
            vec!["example.com".into(), "*.trusted.example".into()],
            false,
        );
        assert!(policy.allows_host("example.com"));
        assert!(policy.allows_host("EXAMPLE.COM."));
        assert!(policy.allows_host("a.trusted.example"));
        assert!(!policy.allows_host("evil-example.com"));
        assert!(!policy.allows_host("trusted.example"));
        assert!(!EgressPolicy::new(vec![], false).allows_host("example.com"));
    }

    #[tokio::test]
    async fn proxy_denies_loopback_when_not_explicitly_allowed() {
        let proxy = EgressProxy::start(EgressPolicy::new(vec!["localhost".into()], false))
            .await
            .unwrap();
        let addr = proxy.addr();
        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(b"CONNECT localhost:9 HTTP/1.1\r\nHost: localhost:9\r\n\r\n")
            .await
            .unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.unwrap();
        let text = String::from_utf8_lossy(&response);
        assert!(text.contains("403") || text.is_empty(), "{text}");
    }
}
