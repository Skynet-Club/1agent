//! Shared host checks for outbound HTTP policy.

use std::net::IpAddr;

/// Whether a URL host is `localhost` or a loopback IP, including IPv6 literals
/// such as `[::1]` as returned by `Url::host_str`.
pub fn is_loopback_host(host: &str) -> bool {
    let host = host.trim_start_matches('[').trim_end_matches(']');
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .map(|address| address.is_loopback())
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_hosts_include_ipv6_literals() {
        assert!(is_loopback_host("127.0.0.1"));
        assert!(is_loopback_host("::1"));
        assert!(is_loopback_host("[::1]"));
        assert!(is_loopback_host("localhost"));
        assert!(is_loopback_host("LOCALHOST"));
        assert!(!is_loopback_host("example.com"));
        assert!(!is_loopback_host("10.0.0.1"));
    }
}
