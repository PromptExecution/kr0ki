//! SSRF-hardened HTTPS-only source acquisition for ReqIF documents, feeding
//! `reqif_import::import_reqif_artifact`. See
//! docs/superpowers/specs/2026-09-22-reqif-https-fetch-design.md for the
//! full design and threat model. This module deliberately does not add a
//! CLI/MCP local-file loader -- that is a separate, undone docs/TODO.md item.

use std::net::IpAddr;
use std::time::Duration;

use crate::reqif_import::DEFAULT_MAX_REQIF_IMPORT_BYTES;

/// Acquisition policy for one fetch. `Default` matches this module's own
/// recommended values -- callers only override what they need to.
#[derive(Debug, Clone)]
pub struct FetchConfig {
    pub max_bytes: usize,
    pub timeout: Duration,
    pub max_redirects: u8,
}

impl Default for FetchConfig {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_REQIF_IMPORT_BYTES,
            timeout: Duration::from_secs(15),
            max_redirects: 3,
        }
    }
}

/// The bytes a successful fetch produced, plus provenance headers the
/// caller can carry into `reqif_import::ReqIfImportConfig`.
#[derive(Debug, Clone)]
pub struct FetchedArtifact {
    pub bytes: Vec<u8>,
    pub final_url: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("only https:// URLs are permitted, got {scheme}")]
    UnsupportedScheme { scheme: String },
    #[error("could not parse URL: {0}")]
    InvalidUrl(String),
    #[error("host resolves to a disallowed address: {0}")]
    DisallowedAddress(IpAddr),
    #[error("DNS resolution failed: {0}")]
    Resolution(String),
    #[error("too many redirects (max {max})")]
    TooManyRedirects { max: u8 },
    #[error("redirect target is not permitted: {0}")]
    DisallowedRedirect(String),
    #[error("response exceeds {maximum} bytes")]
    TooLarge { maximum: usize },
    #[error("request timed out")]
    Timeout,
    #[error(transparent)]
    Http(#[from] reqwest::Error),
}

/// True if `addr` must never be connected to: loopback, unspecified,
/// multicast (checked generically), or -- once canonicalized, which
/// unwraps an IPv4-mapped IPv6 address to plain IPv4 first and closes that
/// encoding bypass -- IPv4 private/link-local/broadcast/documentation or
/// IPv6 unique-local/unicast-link-local ranges.
///
/// Every method here is called on the *concrete* `Ipv4Addr`/`Ipv6Addr`
/// type, not the `IpAddr` enum -- `IpAddr::is_private()` and
/// `IpAddr::is_documentation()` are NOT stable, only the per-family methods
/// are (verified against `core::net::ip_addr` source, rustc 1.98.1).
pub(crate) fn is_disallowed_address(addr: IpAddr) -> bool {
    let addr = addr.to_canonical();
    if addr.is_loopback() || addr.is_unspecified() || addr.is_multicast() {
        return true;
    }
    match addr {
        IpAddr::V4(v4) => {
            v4.is_private() || v4.is_link_local() || v4.is_broadcast() || v4.is_documentation()
        }
        IpAddr::V6(v6) => v6.is_unique_local() || v6.is_unicast_link_local(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn loopback_v4_is_disallowed() {
        assert!(is_disallowed_address(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
    }

    #[test]
    fn loopback_v6_is_disallowed() {
        assert!(is_disallowed_address(IpAddr::V6(Ipv6Addr::LOCALHOST)));
    }

    #[test]
    fn private_v4_ranges_are_disallowed() {
        assert!(is_disallowed_address(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert!(is_disallowed_address(IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))));
        assert!(is_disallowed_address(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
    }

    #[test]
    fn link_local_v4_including_cloud_metadata_is_disallowed() {
        // 169.254.169.254 -- the AWS/GCP/Azure instance-metadata endpoint,
        // the concrete case this whole module exists to close.
        assert!(is_disallowed_address(IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254))));
    }

    #[test]
    fn unique_local_and_link_local_v6_are_disallowed() {
        assert!(is_disallowed_address(IpAddr::V6(Ipv6Addr::new(
            0xfc00, 0, 0, 0, 0, 0, 0, 1
        ))));
        assert!(is_disallowed_address(IpAddr::V6(Ipv6Addr::new(
            0xfe80, 0, 0, 0, 0, 0, 0, 1
        ))));
    }

    #[test]
    fn ipv4_mapped_ipv6_private_address_is_disallowed() {
        // ::ffff:10.0.0.1 -- encoding a private IPv4 address as its
        // IPv4-mapped IPv6 form, a known SSRF-filter-bypass technique.
        let mapped = Ipv4Addr::new(10, 0, 0, 1).to_ipv6_mapped();
        assert!(is_disallowed_address(IpAddr::V6(mapped)));
    }

    #[test]
    fn a_normal_public_v4_address_is_allowed() {
        // 8.8.8.8 -- a real, stable public address (Google DNS), not a
        // network we're claiming to reach, just a known-public literal.
        assert!(!is_disallowed_address(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
    }
}
