# ReqIF HTTPS-fetch source acquisition Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an SSRF-hardened HTTPS fetcher (`kr0ki_core::reqif_fetch`) that safely acquires a ReqIF/ReqIFz artifact from a URL and feeds it into the existing `reqif_import::import_reqif_artifact` boundary, plus an HTTP route and MCP tool to call it.

**Architecture:** One new pure module (`reqif_fetch::is_disallowed_address` — an IP-range classifier, no I/O) feeds one new async orchestration function (`reqif_fetch::fetch_reqif_url` — DNS pre-resolution, connection pinned to the validated address, redirects disabled and manually re-validated per hop, size-capped streaming). A new `kr0ki-server` route (`POST /requirements/import/url`, raw-string body) wires the fetcher to the existing import boundary and returns the same response shape `/requirements/import` already does. A new `McpTool::ImportReqIfUrl` variant exposes it over MCP using the existing generic manifest-dispatch mechanism — no new `ArgPlacement` variant, no bridge.py change.

**Tech Stack:** Rust, `reqwest` (already a dependency; this plan enables its `stream` feature), `futures-util` (already a transitive dependency, promoted to direct), `tokio::net::lookup_host`, `wiremock` (already a dev-dependency).

**Spec:** [`docs/superpowers/specs/2026-09-22-reqif-https-fetch-design.md`](../specs/2026-09-22-reqif-https-fetch-design.md) — the plan argues from this spec; executors should read both.

## Global Constraints

- HTTPS-only: reject any URL whose scheme isn't exactly `https` before any DNS resolution (spec §2.1 step 1).
- SSRF: resolve the host via `tokio::net::lookup_host` and reject the *entire* request if ANY resolved address is disallowed (spec §2.1 step 2, §2.2) — never pick "the first allowed one" out of a mixed set.
- Disallowed-address check canonicalizes first (`IpAddr::to_canonical()`, stable since Rust 1.75.0 — closes the IPv4-mapped-IPv6 encoding bypass) then checks: loopback / unspecified / multicast (generic `IpAddr` methods, stable), plus IPv4 private/link-local/broadcast/documentation (`Ipv4Addr` methods, stable since 1.50.0) or IPv6 unique-local/unicast-link-local (`Ipv6Addr` methods, stable since 1.84.0). Every method used here was verified against the actual `core::net::ip_addr` stdlib source for this toolchain (`rustc 1.98.1`) before this plan was written — do not substitute an `IpAddr`-level `is_private`/`is_documentation` call, neither is stable at that level.
- The connection must pin to the exact validated `SocketAddr` (`reqwest::ClientBuilder::resolve(host, addr)`) with automatic redirects disabled (`reqwest::redirect::Policy::none()`) — every redirect hop is manually re-validated from scratch (scheme check, then DNS resolution, then range check), never followed automatically. Default cap: 3 redirects (`FetchConfig::max_redirects`).
- The response body is read via `reqwest`'s `stream` feature (`Response::bytes_stream()`), aborting the moment the running total exceeds `FetchConfig::max_bytes` — never call `.bytes().await` (buffers the whole response first, defeating the cap).
- No CLI/MCP local-file loader in this plan — that stays a separate, undone `docs/TODO.md` item.
- No credential/auth-header forwarding to the fetched URL.
- No response caching layer — `import_reqif_artifact`'s existing `artifact_sha256` is the only content-addressing this plan relies on.
- The new HTTP route's request body is the raw URL as UTF-8 text (`axum::body::Bytes`, not `Json<...>`) — matches the `ArgPlacement::Body` convention every other MCP-bound POST route in this codebase already uses (see `crates/kr0ki-core/src/mcp_tool.rs`'s doc comment on `ArgPlacement::Body` and `containers/kr0ki-mcp/manifest_dispatch.py`'s `apply_binding`, which does `str(value).encode("utf-8")` — a JSON body would not round-trip through that dispatcher).

---

## Task 1: IP-range classifier + core types

**Files:**
- Create: `crates/kr0ki-core/src/reqif_fetch.rs`
- Modify: `crates/kr0ki-core/src/lib.rs` (register the module, alphabetically BEFORE `reqif_import` — `reqif_fetch` < `reqif_import` since `f` < `i` — the current file has `pub mod render;` at line 38 then `pub mod reqif_import;` at line 39 in that order already, since `render` < `reqif_import` alphabetically too; `reqif_fetch` goes directly after `render`, directly before `reqif_import`)
- Test: inline `#[cfg(test)] mod tests` in the new file

**Interfaces:**
- Consumes: `crate::reqif_import::DEFAULT_MAX_REQIF_IMPORT_BYTES` (existing).
- Produces: `pub struct FetchConfig { pub max_bytes: usize, pub timeout: std::time::Duration, pub max_redirects: u8 }` (with `Default`), `pub struct FetchedArtifact { pub bytes: Vec<u8>, pub final_url: String, pub etag: Option<String>, pub last_modified: Option<String> }`, `pub enum FetchError { UnsupportedScheme{scheme: String}, InvalidUrl(String), DisallowedAddress(std::net::IpAddr), Resolution(String), TooManyRedirects{max: u8}, DisallowedRedirect(String), TooLarge{maximum: usize}, Timeout, Http(#[from] reqwest::Error) }`, `pub(crate) fn is_disallowed_address(addr: std::net::IpAddr) -> bool` — Task 2 calls this directly (same module, so `pub(crate)` is enough; it does not need to be `pub`).

This task is pure logic and types only — no network, no async. Task 2 (dispatched separately) adds `fetch_reqif_url` on top.

- [ ] **Step 1: Write the failing test**

Create `crates/kr0ki-core/src/reqif_fetch.rs`:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/kr0ki-core && cargo test reqif_fetch`
Expected: FAIL — module not registered in `src/lib.rs` yet.

- [ ] **Step 3: Register the module**

In `crates/kr0ki-core/src/lib.rs`, find:

```rust
pub mod render;
pub mod reqif_import;
```

Change to:

```rust
pub mod render;
pub mod reqif_fetch;
pub mod reqif_import;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd crates/kr0ki-core && cargo test reqif_fetch`
Expected: PASS (7 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/kr0ki-core/src/reqif_fetch.rs crates/kr0ki-core/src/lib.rs
git commit -m "feat(reqif-fetch): SSRF-hardened IP-range classifier and core types"
```

---

## Task 2: `fetch_reqif_url` orchestration

**Files:**
- Modify: `crates/kr0ki-core/src/reqif_fetch.rs`
- Modify: `crates/kr0ki-core/Cargo.toml` (enable `reqwest`'s `stream` feature; add `futures-util` as a direct dependency -- it is already a transitive dependency of `reqwest`/`tokio`, this only promotes it to direct so this crate can call `StreamExt::next()`)
- Test: `crates/kr0ki-core/tests/reqif_fetch.rs` (new file, network-free security tests) + a separate `#[ignore]`d live-test section in the same file (public-endpoint happy-path tests)

**Interfaces:**
- Consumes: `is_disallowed_address`, `FetchConfig`, `FetchedArtifact`, `FetchError` (Task 1, same module).
- Produces: `pub async fn fetch_reqif_url(url: &str, config: &FetchConfig) -> Result<FetchedArtifact, FetchError>` — Task 3 calls this directly.

- [ ] **Step 1: Add the `stream` feature and `futures-util` dependency**

In `crates/kr0ki-core/Cargo.toml`, change:

```toml
reqwest = { workspace = true }
```

to:

```toml
reqwest = { workspace = true, features = ["stream"] }
```

(Check the exact current line first -- `reqwest = { workspace = true }` appears once in `[dependencies]`; the workspace root `Cargo.toml`'s own `reqwest` entry already carries `rustls-tls`, this crate-level `features` list only adds to it.)

Add, near the other small crate-local dependencies (e.g. next to `quote`/`walkdir`):

```toml
futures-util = "0.3"
```

- [ ] **Step 2: Write the failing tests (network-free security tests first)**

Create `crates/kr0ki-core/tests/reqif_fetch.rs`:

```rust
//! Security-critical rejection paths for `reqif_fetch::fetch_reqif_url` --
//! these need no network access at all, since the scheme check and the
//! DNS-range check both happen before any connection is opened. See
//! `crates/kr0ki-core/src/reqif_fetch.rs`'s own module doc and
//! docs/superpowers/specs/2026-09-22-reqif-https-fetch-design.md section 6
//! for why the happy-path tests are separate, #[ignore]d, live tests.

use kr0ki_core::reqif_fetch::{fetch_reqif_url, FetchConfig, FetchError};

#[tokio::test]
async fn non_https_scheme_is_rejected_before_resolution() {
    let result = fetch_reqif_url("http://example.com/doc.reqif", &FetchConfig::default()).await;
    assert!(matches!(result, Err(FetchError::UnsupportedScheme { scheme }) if scheme == "http"));
}

#[tokio::test]
async fn loopback_literal_is_rejected() {
    let result = fetch_reqif_url("https://127.0.0.1/doc.reqif", &FetchConfig::default()).await;
    assert!(matches!(result, Err(FetchError::DisallowedAddress(_))));
}

#[tokio::test]
async fn cloud_metadata_endpoint_is_rejected() {
    let result = fetch_reqif_url(
        "https://169.254.169.254/latest/meta-data/",
        &FetchConfig::default(),
    )
    .await;
    assert!(matches!(result, Err(FetchError::DisallowedAddress(_))));
}

#[tokio::test]
async fn private_ipv6_literal_is_rejected() {
    let result = fetch_reqif_url("https://[fc00::1]/doc.reqif", &FetchConfig::default()).await;
    assert!(matches!(result, Err(FetchError::DisallowedAddress(_))));
}

// -- Live tests below require real network access to public HTTPS
// endpoints; #[ignore]d, matching this repo's existing live-test
// convention (see kr0ki-sysmlv2-client/tests/live.rs).

#[tokio::test]
#[ignore = "requires network access to httpbin.org"]
async fn redirect_chain_is_followed_and_revalidated() {
    let result = fetch_reqif_url(
        "https://httpbin.org/redirect/2",
        &FetchConfig::default(),
    )
    .await;
    assert!(result.is_ok(), "expected redirect chain to resolve: {result:?}");
}

#[tokio::test]
#[ignore = "requires network access to httpbin.org"]
async fn oversized_response_is_aborted_mid_stream() {
    let config = FetchConfig {
        max_bytes: 1024,
        ..FetchConfig::default()
    };
    let result = fetch_reqif_url("https://httpbin.org/bytes/1048576", &config).await;
    assert!(matches!(result, Err(FetchError::TooLarge { maximum: 1024 })));
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd crates/kr0ki-core && cargo test --test reqif_fetch`
Expected: FAIL to compile -- `fetch_reqif_url` doesn't exist yet.

- [ ] **Step 4: Implement `fetch_reqif_url`**

Add to `crates/kr0ki-core/src/reqif_fetch.rs`, after `is_disallowed_address`:

```rust
use futures_util::StreamExt;
use reqwest::redirect::Policy;
use reqwest::Client;

/// Fetch a ReqIF/ReqIFz artifact from `url` under `config`'s policy. See
/// this module's doc comment and the design spec for the full algorithm:
/// scheme check, DNS pre-resolution with a full range check on every
/// resolved address, a connection pinned to the validated address with
/// redirects disabled and manually re-validated per hop, and a
/// streaming size cap.
pub async fn fetch_reqif_url(
    url: &str,
    config: &FetchConfig,
) -> Result<FetchedArtifact, FetchError> {
    let mut current = url.to_string();
    let mut redirects = 0u8;

    loop {
        let parsed = reqwest::Url::parse(&current).map_err(|e| FetchError::InvalidUrl(e.to_string()))?;
        if parsed.scheme() != "https" {
            return Err(FetchError::UnsupportedScheme {
                scheme: parsed.scheme().to_string(),
            });
        }
        let host = parsed
            .host_str()
            .ok_or_else(|| FetchError::InvalidUrl("URL has no host".to_string()))?
            .to_string();
        let port = parsed.port_or_known_default().unwrap_or(443);

        let resolved: Vec<std::net::SocketAddr> = tokio::net::lookup_host((host.as_str(), port))
            .await
            .map_err(|e| FetchError::Resolution(e.to_string()))?
            .collect();
        if resolved.is_empty() {
            return Err(FetchError::Resolution(format!("no addresses for {host}")));
        }
        for addr in &resolved {
            if is_disallowed_address(addr.ip()) {
                return Err(FetchError::DisallowedAddress(addr.ip()));
            }
        }
        let pinned = resolved[0];

        let client = Client::builder()
            .redirect(Policy::none())
            .resolve(&host, pinned)
            .timeout(config.timeout)
            .build()?;

        let response = match client.get(parsed.clone()).send().await {
            Ok(r) => r,
            Err(e) if e.is_timeout() => return Err(FetchError::Timeout),
            Err(e) => return Err(FetchError::Http(e)),
        };

        if response.status().is_redirection() {
            redirects += 1;
            if redirects > config.max_redirects {
                return Err(FetchError::TooManyRedirects {
                    max: config.max_redirects,
                });
            }
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| {
                    FetchError::DisallowedRedirect("redirect with no Location header".to_string())
                })?;
            let next = parsed
                .join(location)
                .map_err(|e| FetchError::DisallowedRedirect(e.to_string()))?;
            current = next.to_string();
            continue;
        }

        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        let last_modified = response
            .headers()
            .get(reqwest::header::LAST_MODIFIED)
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        let final_url = response.url().to_string();

        let mut body = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(FetchError::Http)?;
            if body.len() + chunk.len() > config.max_bytes {
                return Err(FetchError::TooLarge {
                    maximum: config.max_bytes,
                });
            }
            body.extend_from_slice(&chunk);
        }

        return Ok(FetchedArtifact {
            bytes: body,
            final_url,
            etag,
            last_modified,
        });
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd crates/kr0ki-core && cargo test --test reqif_fetch` (the 4 non-ignored tests; the 2 `#[ignore]`d live tests do not run here)
Expected: PASS (4 passed, 2 ignored).

- [ ] **Step 6: Run the full workspace test suite and clippy**

Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: all pass, clean, clean.

- [ ] **Step 7: Commit**

```bash
git add crates/kr0ki-core/src/reqif_fetch.rs crates/kr0ki-core/Cargo.toml crates/kr0ki-core/tests/reqif_fetch.rs
git commit -m "feat(reqif-fetch): fetch_reqif_url orchestration (DNS pinning, redirect revalidation, size cap)"
```

---

## Task 3: HTTP route `POST /requirements/import/url`

**Files:**
- Modify: `crates/kr0ki-server/src/app.rs` (add the route + handler)
- Test: `crates/kr0ki-server/tests/http.rs` (append)

**Interfaces:**
- Consumes: `kr0ki_core::reqif_fetch::{fetch_reqif_url, FetchConfig, FetchError}` (Task 2), `kr0ki_core::reqif_import::{import_reqif_artifact, ReqIfImportConfig}` (existing).
- Produces: handler `async fn import_requirements_url(body: Bytes) -> Response` registered at `POST /requirements/import/url` -- no new public Rust interface, this is the route's own handler.

- [ ] **Step 1: Write the failing test**

Append to `crates/kr0ki-server/tests/http.rs`, using this file's own existing helpers exactly as the neighboring `/requirements/import` tests already do — `test_app(test_state("tag"))` builds the router (no `test_router()` helper exists), `body_string(response).await -> (StatusCode, String)` reads the response, `Request::post(path)` builds the request (all already imported at the top of this file):

```rust
#[tokio::test]
async fn import_requirements_url_rejects_non_https_scheme() {
    let response = test_app(test_state("reqif-url-scheme"))
        .oneshot(
            Request::post("/requirements/import/url")
                .header("content-type", "text/plain")
                .body(Body::from("http://example.com/doc.reqif"))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(response).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
}

#[tokio::test]
async fn import_requirements_url_rejects_disallowed_address() {
    let response = test_app(test_state("reqif-url-ssrf"))
        .oneshot(
            Request::post("/requirements/import/url")
                .header("content-type", "text/plain")
                .body(Body::from("https://127.0.0.1/doc.reqif"))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(response).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/kr0ki-server && cargo test --test http import_requirements_url`
Expected: FAIL -- 404, route doesn't exist yet.

- [ ] **Step 3: Add the handler**

In `crates/kr0ki-server/src/app.rs`, add near `import_requirements` (the existing `/requirements/import` handler):

```rust
/// `POST /requirements/import/url` -- fetch a ReqIF/ReqIFz artifact from an
/// HTTPS URL (SSRF-hardened, see kr0ki_core::reqif_fetch) and import it
/// through the same bounded-byte boundary `/requirements/import` uses. The
/// request body is the raw URL as UTF-8 text, not JSON -- this matches the
/// ArgPlacement::Body convention every other MCP-bound POST route here uses.
async fn import_requirements_url(body: Bytes) -> Response {
    let url = match std::str::from_utf8(&body) {
        Ok(s) => s.trim(),
        Err(_) => {
            return error_json(
                StatusCode::BAD_REQUEST,
                "invalid_url_encoding",
                "request body must be UTF-8",
            )
        }
    };

    let fetched = match kr0ki_core::reqif_fetch::fetch_reqif_url(
        url,
        &kr0ki_core::reqif_fetch::FetchConfig::default(),
    )
    .await
    {
        Ok(fetched) => fetched,
        Err(
            error @ (kr0ki_core::reqif_fetch::FetchError::UnsupportedScheme { .. }
            | kr0ki_core::reqif_fetch::FetchError::InvalidUrl(_)
            | kr0ki_core::reqif_fetch::FetchError::DisallowedAddress(_)
            | kr0ki_core::reqif_fetch::FetchError::DisallowedRedirect(_)
            | kr0ki_core::reqif_fetch::FetchError::TooManyRedirects { .. }
            | kr0ki_core::reqif_fetch::FetchError::TooLarge { .. }),
        ) => {
            return error_json(StatusCode::BAD_REQUEST, "reqif_fetch_rejected", &error.to_string())
        }
        Err(error) => {
            return error_json(StatusCode::BAD_GATEWAY, "reqif_fetch_upstream_error", &error.to_string())
        }
    };

    let config = kr0ki_core::reqif_import::ReqIfImportConfig {
        source_uri: Some(fetched.final_url),
        revision: fetched.etag.or(fetched.last_modified),
        ..Default::default()
    };

    match kr0ki_core::reqif_import::import_reqif_artifact(&fetched.bytes, &config) {
        Ok(imported) => Json(imported).into_response(),
        Err(
            kr0ki_core::reqif_import::ReqIfImportError::ArtifactTooLarge { .. }
            | kr0ki_core::reqif_import::ReqIfImportError::ExpandedTooLarge { .. },
        ) => error_json(
            StatusCode::PAYLOAD_TOO_LARGE,
            "reqif_import_too_large",
            "fetched ReqIF artifact exceeds this deployment's size policy",
        ),
        Err(error) => error_json(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_reqif_import",
            &error.to_string(),
        ),
    }
}
```

- [ ] **Step 4: Register the route**

In `crates/kr0ki-server/src/app.rs`'s `router()` function, add directly after the existing `.route("/requirements/import", ...)` entry:

```rust
        .route("/requirements/import/url", post(import_requirements_url))
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd crates/kr0ki-server && cargo test --test http import_requirements_url`
Expected: PASS (2 passed).

- [ ] **Step 6: Run the full workspace test suite and clippy**

Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: all pass, clean, clean.

- [ ] **Step 7: Commit**

```bash
git add crates/kr0ki-server/src/app.rs crates/kr0ki-server/tests/http.rs
git commit -m "feat(server): POST /requirements/import/url -- SSRF-hardened ReqIF URL import"
```

---

## Task 4: MCP tool `import_reqif_url`

**Files:**
- Modify: `crates/kr0ki-core/src/mcp_tool.rs`

**Interfaces:**
- Consumes: nothing new -- this task only adds a manifest entry for Task 3's route.
- Produces: `McpTool::ImportReqIfUrl` variant, wired through every exhaustive match in the file.

- [ ] **Step 1: Write the failing test**

Add to `crates/kr0ki-core/src/mcp_tool.rs`'s existing `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn import_reqif_url_binds_url_to_body_as_a_post() {
        let binding = McpTool::ImportReqIfUrl.http_binding();
        assert!(matches!(binding.method, HttpMethod::Post));
        assert_eq!(binding.path_template, "/requirements/import/url");
        assert_eq!(binding.args.len(), 1);
        assert!(binding
            .args
            .iter()
            .any(|a| a.name == "url" && matches!(a.placement, ArgPlacement::Body)));
    }
```

Also update the existing `all_thirteen_tools_have_unique_names` test's hardcoded count from `13` to `14` (check its exact current name/assertion first -- it may already have been renamed to a count-agnostic name by another branch; if so, just update the numeric literal, not the test name).

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/kr0ki-core && cargo test import_reqif_url`
Expected: FAIL to compile -- `McpTool::ImportReqIfUrl` doesn't exist yet.

- [ ] **Step 3: Add the variant everywhere the enum is matched**

In `crates/kr0ki-core/src/mcp_tool.rs`:

In the `enum McpTool` definition, add `ImportReqIfUrl,` directly after `ImportReqIf,`.

In `McpTool::ALL`, add `Self::ImportReqIfUrl,` directly after `Self::ImportReqIf,`.

In `name()`, add:
```rust
            Self::ImportReqIfUrl => "import_reqif_url",
```
directly after the `ImportReqIf` arm.

In `description()`, add:
```rust
            Self::ImportReqIfUrl => {
                "Fetch a ReqIF/ReqIFz artifact from an HTTPS URL (SSRF-hardened: DNS-resolved and range-checked before connecting) and validate/normalize it."
            }
```
directly after the `ImportReqIf` arm.

In `input_schema()`, add:
```rust
            Self::ImportReqIfUrl => serde_json::json!({
                "type": "object",
                "required": ["url"],
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "HTTPS URL to fetch a ReqIF or ReqIFz artifact from. Only https:// is accepted."
                    }
                }
            }),
```
directly after the `ImportReqIf` arm.

In `http_binding()`, add:
```rust
            Self::ImportReqIfUrl => HttpBinding {
                method: HttpMethod::Post,
                path_template: "/requirements/import/url",
                args: &[ArgBinding {
                    name: "url",
                    placement: ArgPlacement::Body,
                }],
            },
```
directly after the `ImportReqIf` arm.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd crates/kr0ki-core && cargo test mcp_tool`
Expected: PASS, including the new test and the updated count assertion.

- [ ] **Step 5: Run the full workspace test suite and clippy**

Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: all pass, clean, clean.

- [ ] **Step 6: Commit**

```bash
git add crates/kr0ki-core/src/mcp_tool.rs
git commit -m "feat(mcp): add import_reqif_url tool for Task 3's fetch route"
```

---

## Task 5: Push a branch and open a PR

**Files:** none (git/GitHub operations only).

- [ ] **Step 1: Create the branch and push**

```bash
git checkout -b feat/reqif-https-fetch
git push -u origin feat/reqif-https-fetch
```

(If Tasks 1-4 were committed on an isolated worktree branch already, skip `checkout -b` and just push that branch.)

- [ ] **Step 2: Open the PR**

```bash
gh pr create --repo PromptExecution/kr0ki \
  --title "feat: SSRF-hardened HTTPS-fetch source acquisition for ReqIF import" \
  --body "Implements docs/superpowers/specs/2026-09-22-reqif-https-fetch-design.md. Adds kr0ki_core::reqif_fetch (DNS pre-resolution + range rejection incl. the DNS-rebinding case, connection pinned to the validated address, redirects disabled and manually re-validated per hop, streaming size cap), a new POST /requirements/import/url route, and a matching import_reqif_url MCP tool. CLI/MCP local-file loader is an explicit non-goal here, tracked as a separate docs/TODO.md follow-up."
```

- [ ] **Step 3: Confirm CI, then merge**

Wait for checks. Squash-merge once green:

```bash
gh pr merge --repo PromptExecution/kr0ki --squash --delete-branch
```

---

## Self-Review

**Spec coverage:** §2 (module types + algorithm) — Task 1 (types + classifier) + Task 2 (orchestration). §2.2 (disallowed ranges, including the IPv4-mapped-IPv6 bypass) — Task 1, every method verified against real stdlib source for stability before being written into this plan. §3 (route, raw-string body convention) — Task 3. §4 (MCP tool) — Task 4. §5 (non-goals: no local loader, no credentials, no caching) — none of these appear in any task. §6 (testing: network-free security tests vs. `#[ignore]`d live happy-path tests) — Task 2's test split matches exactly.

**Placeholder scan:** none found — every step has real, complete code. Task 3's test step flags that exact helper names must be checked against the real file before writing (not a TBD — an explicit instruction to verify against source rather than guess, consistent with this plan's own practice everywhere else).

**Type consistency:** `FetchConfig`/`FetchedArtifact`/`FetchError` (Task 1) match their use in Task 2's `fetch_reqif_url` and Task 3's handler exactly. `fetch_reqif_url`'s signature (Task 2) matches Task 3's call site. `McpTool::ImportReqIfUrl`'s `http_binding()` (Task 4) matches Task 3's actual route path and body convention exactly.
