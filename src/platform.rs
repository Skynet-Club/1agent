//! Platform capability registry.
//!
//! Domain code stays platform-neutral. Adapters report one status per
//! capability so a release can say exactly what is verified, degraded,
//! unsupported or still untested on each target.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Linux,
    Macos,
    Windows,
    Other,
}

impl Platform {
    pub fn current() -> Self {
        match std::env::consts::OS {
            "linux" => Self::Linux,
            "macos" => Self::Macos,
            "windows" => Self::Windows,
            _ => Self::Other,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Linux => "linux",
            Self::Macos => "macos",
            Self::Windows => "windows",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStatus {
    /// Exercised by the release gate on this target.
    Verified,
    /// Works, but with a documented limitation.
    Degraded,
    /// Not implemented on this target.
    Unsupported,
    /// Not exercised yet; never treat as supported.
    Untested,
}

impl CapabilityStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Degraded => "degraded",
            Self::Unsupported => "unsupported",
            Self::Untested => "untested",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Capability {
    pub name: String,
    pub status: CapabilityStatus,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlatformReport {
    pub platform: Platform,
    pub os: String,
    pub arch: String,
    pub capabilities: Vec<Capability>,
}

pub fn report() -> PlatformReport {
    report_for(Platform::current())
}

pub fn report_for(platform: Platform) -> PlatformReport {
    PlatformReport {
        platform,
        os: platform.as_str().to_string(),
        arch: std::env::consts::ARCH.to_string(),
        capabilities: capabilities_for(platform),
    }
}

fn capabilities_for(platform: Platform) -> Vec<Capability> {
    let entries: &[(&str, CapabilityStatus, &str)] = match platform {
        Platform::Linux => &[
            (
                "runtime_home",
                CapabilityStatus::Verified,
                "single $HOME/.1a home",
            ),
            (
                "secure_file",
                CapabilityStatus::Verified,
                "0600/0700 and flock; ACLs not used",
            ),
            (
                "pty",
                CapabilityStatus::Degraded,
                "terminal tools require a running Workshop session; no native PTY backend",
            ),
            (
                "browser_read",
                CapabilityStatus::Degraded,
                "headless read-only rendering",
            ),
            (
                "computer_use",
                CapabilityStatus::Unsupported,
                "clicks, typing, screenshots and uploads are not implemented",
            ),
            (
                "service",
                CapabilityStatus::Degraded,
                "daemon is a manual process; no systemd unit",
            ),
            (
                "notifications",
                CapabilityStatus::Degraded,
                "webhook delivery only; no desktop notification adapter",
            ),
            ("clipboard", CapabilityStatus::Verified, "TUI copy path"),
            (
                "signing",
                CapabilityStatus::Degraded,
                "HMAC report approval; asymmetric release signatures pending",
            ),
            (
                "package",
                CapabilityStatus::Degraded,
                "Linux x86_64 only; macOS/Windows not released",
            ),
        ],
        Platform::Macos | Platform::Windows | Platform::Other => &[
            (
                "runtime_home",
                CapabilityStatus::Untested,
                "adapter not exercised on this target",
            ),
            (
                "secure_file",
                CapabilityStatus::Untested,
                "ACL/DPAPI path not exercised",
            ),
            (
                "pty",
                CapabilityStatus::Untested,
                "ConPTY/openpty path not exercised",
            ),
            (
                "browser_read",
                CapabilityStatus::Untested,
                "browser path not exercised",
            ),
            ("computer_use", CapabilityStatus::Untested, "not exercised"),
            (
                "service",
                CapabilityStatus::Untested,
                "service manager not exercised",
            ),
            (
                "notifications",
                CapabilityStatus::Untested,
                "desktop notification adapter not implemented",
            ),
            (
                "clipboard",
                CapabilityStatus::Untested,
                "clipboard adapter not exercised",
            ),
            (
                "signing",
                CapabilityStatus::Untested,
                "platform signing not exercised",
            ),
            (
                "package",
                CapabilityStatus::Untested,
                "no package on this target",
            ),
        ],
    };
    entries
        .iter()
        .map(|(name, status, note)| Capability {
            name: (*name).to_string(),
            status: *status,
            note: (*note).to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_core_is_verified_and_gaps_are_explicit() {
        let report = report_for(Platform::Linux);
        let by_name = |name: &str| {
            report
                .capabilities
                .iter()
                .find(|capability| capability.name == name)
                .unwrap()
                .status
        };
        assert_eq!(by_name("runtime_home"), CapabilityStatus::Verified);
        assert_eq!(by_name("secure_file"), CapabilityStatus::Verified);
        assert_eq!(by_name("pty"), CapabilityStatus::Degraded);
        assert_eq!(by_name("computer_use"), CapabilityStatus::Unsupported);
        assert_eq!(by_name("package"), CapabilityStatus::Degraded);
    }

    #[test]
    fn untested_platforms_never_claim_verified() {
        for platform in [Platform::Macos, Platform::Windows, Platform::Other] {
            let report = report_for(platform);
            assert!(
                report
                    .capabilities
                    .iter()
                    .all(|capability| capability.status != CapabilityStatus::Verified),
                "{platform:?} must not claim a verified capability"
            );
        }
    }

    #[test]
    fn report_round_trips_through_json() {
        let report = report_for(Platform::Linux);
        let encoded = serde_json::to_string(&report).unwrap();
        let decoded: PlatformReport = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, report);
    }
}
