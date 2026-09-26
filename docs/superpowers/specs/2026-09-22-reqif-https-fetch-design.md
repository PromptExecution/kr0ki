# ReqIF HTTPS-fetch source acquisition — design

**Status:** proposed, pending approval.

## 0. Context

`docs/TODO.md`'s ReqIF/Flexo stream has an open item: **"Validated source
acquisition"** — add callers over the existing bounded-byte import seam
(`kr0ki_core::reqif_import::import_reqif_artifact`), not new parser paths.
That module's own doc comment already names the intended shape: *"the
caller owns acquisition policy (browser upload, an allow-listed local path,
or a separately hardened HTTPS fetch); intake owns format detection, size
limits, hashing, parsing, attachment inventory, and normalization."*

Browser/API upload already exists (`POST /requirements/import`, raw bytes).
This spec covers only the **HTTPS fetcher** — the security-sensitive piece
that was still unbuilt. The CLI/MCP local loader (confined to an
allow-listed root directory) is explicitly **out of scope** here and stays
a separate follow-up item in `docs/TODO.md` — bundling it in would double
this diff's review surface for two independently-shippable capabilities.

## 1. Threat model

A URL supplied to a server-side fetcher is a classic SSRF vector: an
attacker-controlled URL can target the server's own loopback interface,
its cloud metadata endpoint (`169.254.169.254`), or other hosts on its
private network, either directly or via DNS rebinding (a hostname that
resolves to a public IP at check-time and a private IP at connect-time).
This design must close both the direct and rebinding variants.

## 2. Module: `kr0ki_core::reqif_fetch`

```rust
pub struct FetchConfig {
    pub max_bytes: usize,           // reuse DEFAULT_MAX_REQIF_IMPORT_BYTES by default
    pub timeout: std::time::Duration, // default 15s
    pub max_redirects: u8,           // default 3
}

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
    DisallowedAddress(std::net::IpAddr),
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

pub async fn fetch_reqif_url(
    url: &str,
    config: &FetchConfig,
) -> Result<FetchedArtifact, FetchError>;
```

### 2.1 Algorithm

1. Parse `url`. Reject anything whose scheme isn't exactly `https`
   (`FetchError::UnsupportedScheme`) — this alone rules out `file://`,
   `http://` (no plaintext downgrade), `ftp://`, `gopher://`, etc.
2. Extract the host. Resolve it via `tokio::net::lookup_host((host,
   443))`. For **every** resolved address, reject if it falls in a
   disallowed range (§2.2). If ANY resolved address is disallowed, reject
   the whole request — a hostname that resolves to both a public and a
   private address is still a rebinding vector if the client library picks
   between them nondeterministically.
3. Build a `reqwest::Client` with `.redirect(Policy::none())` (manual
   redirect handling, never automatic — automatic-redirect following in
   reqwest would re-resolve and re-connect for us, outside our validation
   loop) and `.resolve(host, validated_socket_addr)` — pins the connection
   to the exact address we validated in step 2, so a second DNS lookup by
   the TLS/TCP layer (the rebinding window) can't substitute a different,
   disallowed address. SNI/`Host` header still use the original hostname
   (`resolve()` overrides only the connection target, not the TLS
   handshake identity), so normal HTTPS certificate validation still
   applies.
4. Issue the GET with `.timeout(config.timeout)`. Stream the body,
   aborting with `FetchError::TooLarge` the moment `config.max_bytes` is
   exceeded — never buffer an unbounded response first.
5. If the response is a redirect (3xx with `Location`), and
   `redirects_followed < config.max_redirects`: parse the `Location`
   header as an absolute URL (relative locations resolve against the
   current URL), and recurse to step 1 for the new URL — every hop gets
   full scheme + DNS + range revalidation, no exceptions. Exceeding
   `max_redirects` returns `FetchError::TooManyRedirects`.
6. On a non-redirect response, capture `ETag`/`Last-Modified` headers and
   the final URL, return `FetchedArtifact`.

### 2.2 Disallowed address ranges

IPv4: loopback (`127.0.0.0/8`), private (`10.0.0.0/8`, `172.16.0.0/12`,
`192.168.0.0/16` — `Ipv4Addr::is_private()`, stable std), link-local
(`169.254.0.0/16`, `Ipv4Addr::is_link_local()` — this is what catches cloud
metadata endpoints), unspecified (`0.0.0.0`), broadcast, documentation
ranges, multicast.

IPv6: loopback (`::1`), unique local (`fc00::/7`), link-local
(`fe80::/10`), unspecified (`::`), multicast. IPv4-mapped IPv6 addresses
(`::ffff:0:0/96`) are unwrapped to their IPv4 form before the IPv4 checks
above apply — this is itself a known SSRF-filter-bypass technique
(encoding a private IPv4 address as its IPv6-mapped form) and must not be
missed.

Use `std::net::{Ipv4Addr, Ipv6Addr}`'s stable `is_private`/`is_loopback`/
`is_link_local`/`is_multicast`/`is_unspecified` methods directly; no new
dependency needed for range checking.

## 3. Route: `POST /requirements/import/url`

Request body: the raw URL as `text/plain` bytes (not JSON) — this
deliberately matches the existing `ArgPlacement::Body` convention every
other MCP-bound POST route already uses (a single string value becomes
the entire raw request body; see `crates/kr0ki-core/src/mcp_tool.rs`'s
`ArgPlacement::Body` doc comment and `containers/kr0ki-mcp/
manifest_dispatch.py`'s `apply_binding`). Using a bare string body here —
instead of a JSON envelope — is what keeps this route immediately
MCP-bindable without touching the shared dispatcher (the `RequirementsViewInput`
JSON-body gap discovered while scoping this work stays a separate,
undone item).

Handler: parse the body as UTF-8 (reject non-UTF-8 with 400), call
`reqif_fetch::fetch_reqif_url`, map `FetchError` variants to HTTP status
(4xx for anything the caller controls — bad scheme, disallowed address,
too many redirects, too large; 502 for `Http`/`Resolution`/`Timeout`,
matching `client_error_response`'s existing convention for upstream
failures elsewhere in this file), then on success call
`reqif_import::import_reqif_artifact(&fetched.bytes,
&ReqIfImportConfig { source_uri: Some(fetched.final_url), revision:
fetched.etag.or(fetched.last_modified), ..Default::default() })` and
return the same `ReqIfImportResult` JSON shape `/requirements/import`
already returns — no new response schema.

## 4. MCP tool: `import_reqif_url`

Follows the existing `McpTool` pattern exactly (see `ImportReqIf` for the
closest precedent): one Body-placed `url` argument, `HttpMethod::Post`,
path `/requirements/import/url`. No new `ArgPlacement` variant needed.

## 5. Non-goals (explicit)

- No CLI/MCP local-file loader (separate follow-up, `docs/TODO.md`).
- No credential/auth-header support for the fetched URL (a future item if
  a real private-source use case appears — v1 is public HTTPS sources
  only).
- No response caching — every call re-fetches. Content-addressed reuse
  happens naturally via `import_reqif_artifact`'s existing `artifact_sha256`,
  not a new cache layer.
- No streaming response back to the caller — the import result is
  returned whole, matching `/requirements/import`'s existing contract.

## 6. Testing

The security-critical rejection paths (scheme check, DNS-range check) both
happen *before* `fetch_reqif_url` ever opens a connection — so they need
no mock server or network access at all, and are ordinary `#[test]`s:

- Unit tests for the IP-range classifier (loopback, private v4/v6,
  link-local incl. `169.254.169.254`, IPv4-mapped-IPv6 bypass, a normal
  public IP passes) — pure function, no I/O.
- `#[tokio::test]` (no `#[ignore]`, no network): `fetch_reqif_url("http://example.com/x", ..)`
  rejected with `UnsupportedScheme` before any resolution is attempted.
- `#[tokio::test]` (no `#[ignore]`, no network): `fetch_reqif_url("https://127.0.0.1/x", ..)`
  rejected with `DisallowedAddress` — `127.0.0.1` resolves instantly (it's
  already a literal), so this proves the reject-before-connect ordering
  without needing DNS or a live socket.
- `#[tokio::test]` (no `#[ignore]`, no network): same for `https://169.254.169.254/latest/meta-data/`
  (the AWS/GCP/Azure metadata endpoint — the concrete case this whole
  design exists to close) and for an IPv4-mapped-IPv6 literal encoding a
  private address.

The happy-path behaviors (successful fetch, `ETag`/`Last-Modified`
capture, redirect-chain following with per-hop revalidation, oversized-
response abort) all require a real HTTPS endpoint, since the fetcher only
accepts `https://` and pins to a DNS-resolved IP — there is no
plain-HTTP or non-network stand-in that exercises the real code path.
These become `#[ignore]`d live tests against public, stable HTTPS test
endpoints (`https://httpbin.org/redirect/2` for the redirect chain,
`https://httpbin.org/bytes/{n}` for the size-limit abort, gated the same
way every other live test in this repo already is — no new gating
convention). This matches the existing precedent (`kr0ki-sysmlv2-client`'s
and `kr0ki-core`'s own live tests) rather than inventing a new one.
