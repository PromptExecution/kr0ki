//! KR-A06: the tool gateway — authenticate the caller, check its capability grant for the
//! operation, audit the decision, and only then run the handler. **For every transport.**
//!
//! # Why this is in the server and not the MCP bridge
//!
//! The MCP bridge forwards `tools/call` to this server's HTTP routes with the bearer token it
//! was started with; a direct HTTP client sends the same kind of request. Enforcing here, on
//! the route a request actually reaches, covers both identically. A bridge that hid a tool, a
//! description claiming a tool is read-only, or a `readOnlyHint` annotation is *advice to the
//! model*, not enforcement (MCP spec 2026-07-28: tool annotations "should be considered
//! untrusted"); nothing here relies on them.
//!
//! # Model
//!
//! - An [`Identity`] has an id and a set of **grants**. Tokens are never stored: the identities
//!   file holds each token's SHA-256, compared in constant time.
//! - Every route is **classified** by its matched route pattern into an [`Operation`] requiring
//!   one grant. A route with no table entry — or no route at all — requires `admin`:
//!   **deny by default**. `tests` below fail if a route is added to the router without being
//!   classified, or classified without existing.
//! - `admin` implies every grant.
//! - [`AuthMode`]: `Identities` (a file of identities), `SharedToken` (the legacy single
//!   `KR0KI_AUTH_TOKEN`, which is admin — FR7 minimal, unchanged), or `Open` (no auth, local
//!   development). Only `Identities` mode can express a read-only caller.
//!
//! # Audit
//!
//! With an [`AuditLog`] configured, one record is written **before** the handler runs (who,
//! what, the decision) and one after (the status). If the decision record cannot be written,
//! the request is refused (503): the gateway fails closed rather than act unrecorded.
//! `GET /health` is exempt from both authentication and audit — a readiness probe is not a
//! tool invocation.
//!
//! A caller-supplied `X-Request-ID` is adopted as the correlation id (so callers can correlate
//! their own logs); it is length-capped here and uniqueness comes from the log's `seq`.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use axum::extract::{MatchedPath, Request};
use axum::http::{header, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use kr0ki_core::assurance_baseline::sha256_hex;
use kr0ki_core::audit_log::{now_rfc3339, AuditDraft, AuditLog, Decision, Phase};
use kr0ki_core::mcp_tool::McpTool;
use serde::Deserialize;

use crate::contract::RequestId;

/// Every grant an identity may hold.
pub const GRANTS: [&str; 9] = [
    "render",
    "model.read",
    "model.commit",
    "assurance.read",
    "assurance.propose",
    "assurance.verify",
    "audit.read",
    "ui.steer",
    "admin",
];

// ── Identities ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub id: String,
    grants: BTreeSet<String>,
    token_sha256: String,
}

impl Identity {
    pub fn new(id: impl Into<String>, grants: &[&str], token: &str) -> Self {
        Self {
            id: id.into(),
            grants: grants.iter().map(|g| (*g).to_string()).collect(),
            token_sha256: sha256_hex(token.as_bytes()),
        }
    }

    pub fn permits(&self, grant: &str) -> bool {
        self.grants.contains("admin") || self.grants.contains(grant)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    #[error("identities file: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("identity `{0}` is declared twice")]
    DuplicateId(String),
    #[error("identities `{0}` and `{1}` share a token")]
    DuplicateToken(String, String),
    #[error("identity `{id}` has unknown grant `{grant}` (known: {})", GRANTS.join(", "))]
    UnknownGrant { id: String, grant: String },
    #[error(
        "identity `{0}`: token_sha256 must be 64 lowercase hex digits (the SHA-256 of the token)"
    )]
    BadDigest(String),
    #[error("the identities file declares no identity")]
    Empty,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdentitiesDoc {
    #[serde(default)]
    identity: Vec<IdentityDoc>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdentityDoc {
    id: String,
    token_sha256: String,
    grants: Vec<String>,
}

/// Parse and validate an identities file.
pub fn parse_identities(text: &str) -> Result<Vec<Identity>, IdentityError> {
    let doc: IdentitiesDoc = toml::from_str(text)?;
    if doc.identity.is_empty() {
        return Err(IdentityError::Empty);
    }
    let mut out: Vec<Identity> = Vec::new();
    for i in doc.identity {
        if out.iter().any(|o| o.id == i.id) {
            return Err(IdentityError::DuplicateId(i.id));
        }
        let digest_ok = i.token_sha256.len() == 64
            && i.token_sha256
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        if !digest_ok {
            return Err(IdentityError::BadDigest(i.id));
        }
        if let Some(g) = i.grants.iter().find(|g| !GRANTS.contains(&g.as_str())) {
            return Err(IdentityError::UnknownGrant {
                id: i.id.clone(),
                grant: g.clone(),
            });
        }
        if let Some(other) = out.iter().find(|o| o.token_sha256 == i.token_sha256) {
            return Err(IdentityError::DuplicateToken(other.id.clone(), i.id));
        }
        out.push(Identity {
            id: i.id,
            grants: i.grants.into_iter().collect(),
            token_sha256: i.token_sha256,
        });
    }
    Ok(out)
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

#[derive(Debug, Clone)]
pub enum AuthMode {
    /// No authentication (local development). Every caller is `anonymous`, with every grant.
    Open,
    /// The legacy single `KR0KI_AUTH_TOKEN`. The caller is `shared-token`, with every grant.
    SharedToken(String),
    /// Per-identity tokens and grants.
    Identities(Vec<Identity>),
}

// ── Operations ──────────────────────────────────────────────────────────────────

/// What a route does, for authorisation and audit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    /// The MCP tool name when one is bound to this route (so a call is recorded under one name
    /// whichever transport it arrived on), else `METHOD pattern`.
    pub name: String,
    /// The grant required; `None` = any authenticated caller.
    pub grant: Option<&'static str>,
}

/// `(method, route pattern, required grant)`. Anything not here requires `admin`.
///
/// Patterns are the literals passed to `Router::route` (axum `:param` style).
pub const ROUTES: &[(&str, &str, Option<&str>)] = &[
    // public to any authenticated caller
    ("GET", "/", None),
    ("GET", "/welcome", None),
    ("GET", "/formats", None),
    ("GET", "/mcp/tools", None),
    ("GET", "/mcp/resources", None),
    ("GET", "/mcp/prompts", None),
    ("GET", "/capabilities", None),
    ("GET", "/api/examples", None),
    ("GET", "/playbook/api/examples.json", None),
    ("GET", "/api/catalog", None),
    ("GET", "/brand", None),
    ("GET", "/playbook", None),
    ("GET", "/playbook/", None),
    ("GET", "/playbook/*path", None),
    ("GET", "/docs", None),
    ("GET", "/docs/examples/kr0ki-render-flow.svg", None),
    ("GET", "/docs/api.json", None),
    ("GET", "/docs/api.tomllm", None),
    ("GET", "/docs/api.rustdoc", None),
    // rendering
    ("POST", "/api/catalog/suggest", Some("render")),
    ("POST", "/render/:format", Some("render")),
    ("POST", "/render/requirements-view", Some("render")),
    ("POST", "/render/kubediagram", Some("render")),
    ("POST", "/render/k8s-topology", Some("render")),
    ("POST", "/render/rust-source", Some("render")),
    ("POST", "/render/rust-behavior", Some("render")),
    (
        "POST",
        "/render/sysmlv2/projects/:project_id/commits/:commit_id",
        Some("render"),
    ),
    ("GET", "/cache/:key", Some("render")),
    ("GET", "/ui/:session/events", Some("render")),
    // read-only computation and model reads
    ("POST", "/sparql", Some("model.read")),
    ("POST", "/sparql/shapes", Some("model.read")),
    ("POST", "/sparql/rollup", Some("model.read")),
    ("POST", "/sysml/validate", Some("model.read")),
    ("POST", "/sysml/parse", Some("model.read")),
    ("POST", "/sysml/symbols", Some("model.read")),
    ("POST", "/sysml/summary", Some("model.read")),
    ("POST", "/requirements/import", Some("model.read")),
    ("POST", "/requirements/import/url", Some("model.read")),
    ("POST", "/requirements/export", Some("model.read")),
    ("POST", "/requirements/views", Some("model.read")),
    ("GET", "/model/projects", Some("model.read")),
    (
        "GET",
        "/model/projects/:project_id/commits",
        Some("model.read"),
    ),
    (
        "GET",
        "/model/projects/:project_id/commits/:commit_id/snapshot",
        Some("model.read"),
    ),
    (
        "GET",
        "/model/projects/:project_id/commits/:commit_id/elements",
        Some("model.read"),
    ),
    (
        "GET",
        "/model/projects/:project_id/commits/:commit_id/roots",
        Some("model.read"),
    ),
    (
        "GET",
        "/model/projects/:project_id/commits/:commit_id/elements/:element_id/relationships",
        Some("model.read"),
    ),
    (
        "POST",
        "/model/projects/:project_id/recompute",
        Some("model.read"),
    ),
    ("GET", "/model/graph/query", Some("model.read")),
    ("GET", "/b00t-graph/:tag", Some("model.read")),
    // writes to the model
    (
        "POST",
        "/model/projects/:project_id/sync",
        Some("model.commit"),
    ),
    (
        "POST",
        "/model/projects/:project_id/requirements",
        Some("model.commit"),
    ),
    // steering the UI
    ("POST", "/ui/:session/navigate", Some("ui.steer")),
    // the assurance thread
    ("GET", "/assurance/requirements", Some("assurance.read")),
    ("GET", "/assurance/requirements/:id", Some("assurance.read")),
    (
        "GET",
        "/assurance/requirements/:id/trace",
        Some("assurance.read"),
    ),
    ("GET", "/assurance/view", Some("assurance.read")),
    ("GET", "/assurance/evidence", Some("assurance.read")),
    ("POST", "/assurance/changes", Some("assurance.propose")),
    (
        "GET",
        "/assurance/changes/:change_id",
        Some("assurance.read"),
    ),
    (
        "POST",
        "/assurance/changes/:change_id/commit",
        Some("model.commit"),
    ),
    (
        "POST",
        "/assurance/verify/:case_id",
        Some("assurance.verify"),
    ),
    ("GET", "/assurance/audit", Some("audit.read")),
    ("GET", "/assurance/skills/:name", Some("assurance.read")),
];

/// The static Playb00k UI. With `KR0KI_PUBLIC_UI` on these `GET` routes skip authentication and audit, because a
/// browser navigation cannot send an `Authorization` header, so the page could not otherwise load. They serve
/// built JavaScript and example metadata only; every data route stays behind the gateway.
pub const PUBLIC_UI_PATTERNS: &[&str] = &[
    "/playbook",
    "/playbook/",
    "/playbook/*path",
    "/playbook/api/examples.json",
];

/// The grant a `(method, pattern)` requires. Unknown → `admin` (deny by default).
pub fn required_grant(method: &str, pattern: Option<&str>) -> Option<&'static str> {
    let Some(pattern) = pattern else {
        return Some("admin");
    };
    ROUTES
        .iter()
        .find(|(m, p, _)| *m == method && *p == pattern)
        .map(|(_, _, g)| *g)
        .unwrap_or(Some("admin"))
}

/// `(method, pattern)` → MCP tool name, from the tools' own HTTP bindings.
fn tool_names() -> BTreeMap<(String, String), &'static str> {
    McpTool::ALL
        .iter()
        .map(|t| {
            let b = t.http_binding();
            let method = format!("{:?}", b.method).to_uppercase();
            let pattern = b.path_template.replace('{', ":").replace('}', "");
            ((method, pattern), t.name())
        })
        .collect()
}

// ── The gateway ─────────────────────────────────────────────────────────────────

pub struct Gateway {
    mode: AuthMode,
    audit: Option<Arc<AuditLog>>,
    tools: BTreeMap<(String, String), &'static str>,
    model_revision: Option<Box<dyn Fn() -> Option<String> + Send + Sync>>,
    public_ui: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Authn {
    Caller(Identity),
    Failed,
}

impl Gateway {
    pub fn new(mode: AuthMode, audit: Option<Arc<AuditLog>>) -> Self {
        Self {
            mode,
            audit,
            tools: tool_names(),
            model_revision: None,
            public_ui: false,
        }
    }

    /// Serve the static Playb00k UI (`PUBLIC_UI_PATTERNS`) without a token. Off by default: an operator opts in.
    #[allow(dead_code)] // used by main.rs and tests/assurance_a06.rs; other test crates include this file without it
    pub fn with_public_ui(mut self, on: bool) -> Self {
        self.public_ui = on;
        self
    }

    /// How to learn "the model revision in play" for routes that name none in the request
    /// (the assurance routes evaluate against the service's current model revision).
    pub fn with_model_revision(
        mut self,
        f: impl Fn() -> Option<String> + Send + Sync + 'static,
    ) -> Self {
        self.model_revision = Some(Box::new(f));
        self
    }

    pub fn authenticate(&self, authorization: Option<&str>) -> Authn {
        let bearer = authorization.and_then(|v| v.strip_prefix("Bearer "));
        match &self.mode {
            AuthMode::Open => Authn::Caller(Identity::new("anonymous", &["admin"], "")),
            AuthMode::SharedToken(token) => match bearer {
                Some(b)
                    if constant_time_eq(
                        &sha256_hex(b.as_bytes()),
                        &sha256_hex(token.as_bytes()),
                    ) =>
                {
                    Authn::Caller(Identity::new("shared-token", &["admin"], token))
                }
                _ => Authn::Failed,
            },
            AuthMode::Identities(identities) => {
                let Some(b) = bearer else {
                    return Authn::Failed;
                };
                let presented = sha256_hex(b.as_bytes());
                // Compare against every identity: no early exit on a match.
                let mut found: Option<&Identity> = None;
                for i in identities {
                    if constant_time_eq(&presented, &i.token_sha256) {
                        found = Some(i);
                    }
                }
                found.map_or(Authn::Failed, |i| Authn::Caller(i.clone()))
            }
        }
    }

    pub fn operation(&self, method: &Method, pattern: Option<&str>) -> Operation {
        let grant = required_grant(method.as_str(), pattern);
        let name = pattern
            .and_then(|p| {
                self.tools
                    .get(&(method.as_str().to_string(), p.to_string()))
            })
            .map(|n| (*n).to_string())
            .unwrap_or_else(|| format!("{} {}", method, pattern.unwrap_or("(no route)")));
        Operation { name, grant }
    }

    /// The model revision a request is about: a commit id in the path, an `expected_revision`
    /// or `revision` in the query, else the service's current model revision for assurance
    /// routes, else none.
    fn revision_for(
        &self,
        path: &str,
        query: Option<&str>,
        pattern: Option<&str>,
    ) -> Option<String> {
        let mut segments = path.split('/');
        while let Some(s) = segments.next() {
            if s == "commits" {
                if let Some(id) = segments.next().filter(|s| !s.is_empty()) {
                    return Some(id.chars().take(128).collect());
                }
            }
        }
        for pair in query.unwrap_or("").split('&') {
            if let Some(("expected_revision" | "revision", v)) = pair.split_once('=') {
                if !v.is_empty() {
                    return Some(v.chars().take(128).collect());
                }
            }
        }
        if pattern.is_some_and(|p| p.starts_with("/assurance")) {
            return self.model_revision.as_ref().and_then(|f| f());
        }
        None
    }
}

fn deny_response(status: StatusCode, code: &str, detail: &str, request_id: &str) -> Response {
    (
        status,
        Json(serde_json::json!({"error": code, "message": detail, "detail": detail, "request_id": request_id})),
    )
        .into_response()
}

fn cap(s: &str) -> String {
    s.chars().take(128).collect()
}

/// The middleware. Layer it *inside* the contract middleware so a `RequestId` exists.
pub async fn gateway_middleware(req: Request, next: Next, gateway: Arc<Gateway>) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    // A readiness probe is not a tool invocation.
    if method == Method::GET && path == "/health" {
        return next.run(req).await;
    }

    let request_id = req
        .extensions()
        .get::<RequestId>()
        .map(|r| cap(&r.0))
        .unwrap_or_default();
    let pattern = req
        .extensions()
        .get::<MatchedPath>()
        .map(|m| m.as_str().to_string());
    // Opt-in: the static UI loads without a token (a browser navigation cannot send one). GET only,
    // and only the exact patterns listed; everything else, including a POST to a /playbook path, is
    // authenticated and audited as usual.
    if gateway.public_ui
        && method == Method::GET
        && pattern
            .as_deref()
            .is_some_and(|p| PUBLIC_UI_PATTERNS.contains(&p))
    {
        return next.run(req).await;
    }
    let transport = match req
        .headers()
        .get("x-kr0ki-transport")
        .and_then(|v| v.to_str().ok())
    {
        Some("mcp") => "mcp",
        _ => "http",
    };
    let authorization = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    let authn = gateway.authenticate(authorization);
    let op = gateway.operation(&method, pattern.as_deref());
    let revision = gateway.revision_for(&path, req.uri().query(), pattern.as_deref());

    let (caller, decision, reason, deny_status) = match &authn {
        Authn::Failed => (
            "unauthenticated".to_string(),
            Decision::Deny,
            Some("missing or invalid bearer token".to_string()),
            StatusCode::UNAUTHORIZED,
        ),
        Authn::Caller(identity) => match op.grant {
            Some(grant) if !identity.permits(grant) => (
                identity.id.clone(),
                Decision::Deny,
                Some(format!(
                    "identity `{}` has no `{grant}` grant for `{}`",
                    identity.id, op.name
                )),
                StatusCode::FORBIDDEN,
            ),
            _ => (identity.id.clone(), Decision::Permit, None, StatusCode::OK),
        },
    };

    let draft = |phase: Phase, status: Option<u16>| AuditDraft {
        correlation_id: request_id.clone(),
        phase,
        caller: caller.clone(),
        operation: op.name.clone(),
        method: method.to_string(),
        path: path.clone(),
        decision,
        reason: reason.clone(),
        model_revision: revision.clone(),
        transport: transport.to_string(),
        status,
    };

    // Record the decision *before* acting on it; if it cannot be recorded, do not act.
    if let Some(audit) = &gateway.audit {
        if let Err(e) = audit.append(draft(Phase::Decision, None), &now_rfc3339()) {
            tracing::error!("audit write failed; refusing the request: {e}");
            return deny_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "audit_unavailable",
                "the audit log cannot be written, so this operation was not performed",
                &request_id,
            );
        }
    }

    if decision == Decision::Deny {
        let code = if deny_status == StatusCode::UNAUTHORIZED {
            "unauthorized"
        } else {
            "forbidden"
        };
        let response = deny_response(
            deny_status,
            code,
            reason.as_deref().unwrap_or("denied"),
            &request_id,
        );
        if let Some(audit) = &gateway.audit {
            let _ = audit.append(
                draft(Phase::Outcome, Some(deny_status.as_u16())),
                &now_rfc3339(),
            );
        }
        return response;
    }

    let response = next.run(req).await;
    if let Some(audit) = &gateway.audit {
        if let Err(e) = audit.append(
            draft(Phase::Outcome, Some(response.status().as_u16())),
            &now_rfc3339(),
        ) {
            tracing::error!("audit outcome write failed: {e}");
        }
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route_literals(source: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let mut rest = source;
        while let Some(i) = rest.find(".route(") {
            rest = &rest[i + ".route(".len()..];
            let trimmed = rest.trim_start();
            if let Some(after) = trimmed.strip_prefix('"') {
                if let Some(end) = after.find('"') {
                    out.insert(after[..end].to_string());
                }
            }
        }
        out
    }

    /// Every pattern registered with the router, as written in the source.
    fn registered() -> BTreeSet<String> {
        let mut all = route_literals(include_str!("app.rs"));
        all.extend(route_literals(include_str!("docs.rs")));
        all
    }

    #[test]
    fn every_registered_route_is_classified_and_every_classification_names_a_real_route() {
        let registered = registered();
        let classified: BTreeSet<String> =
            ROUTES.iter().map(|(_, p, _)| (*p).to_string()).collect();
        // `/health` is exempt before classification; `/assurance/*` routes are registered by the
        // assurance module, which this scan also reads.
        let mut registered = registered;
        registered.extend(route_literals(include_str!("assurance.rs")));
        registered.remove("/health");
        let unclassified: Vec<_> = registered.difference(&classified).collect();
        assert!(
            unclassified.is_empty(),
            "routes registered but not classified in gateway::ROUTES (they would require admin): {unclassified:?}"
        );
        let stale: Vec<_> = classified.difference(&registered).collect();
        assert!(
            stale.is_empty(),
            "gateway::ROUTES entries that match no registered route: {stale:?}"
        );
    }

    #[test]
    fn the_public_ui_patterns_are_only_static_playbook_gets_and_hold_no_grant() {
        for p in PUBLIC_UI_PATTERNS {
            assert!(p.starts_with("/playbook"), "{p}");
            let row = ROUTES.iter().find(|(m, rp, _)| *m == "GET" && rp == p);
            assert!(row.is_some(), "{p} is not a registered GET route");
            assert_eq!(row.unwrap().2, None, "{p} must need no grant");
        }
        // Nothing that serves model or assurance data may ever be listed.
        assert!(PUBLIC_UI_PATTERNS
            .iter()
            .all(|p| !p.contains("assurance") && !p.contains("model") && !p.contains("audit")));
    }

    #[test]
    fn grants_in_the_table_are_all_known() {
        for (m, p, g) in ROUTES {
            if let Some(g) = g {
                assert!(GRANTS.contains(g), "{m} {p}: unknown grant {g}");
            }
        }
    }

    #[test]
    fn unknown_and_unmatched_routes_require_admin() {
        assert_eq!(required_grant("POST", Some("/not/a/route")), Some("admin"));
        assert_eq!(
            required_grant("DELETE", Some("/model/projects")),
            Some("admin"),
            "right path, wrong method"
        );
        assert_eq!(required_grant("GET", None), Some("admin"));
    }

    #[test]
    fn every_write_to_the_model_requires_model_commit() {
        // A POST under /model/ (other than the read-only recompute) or one ending `/commit`
        // writes to the model. `/render/...` routes only read it, whatever their path says.
        for (m, p, g) in ROUTES {
            let writes_model = *m == "POST"
                && (p.starts_with("/model/") || p.ends_with("/commit"))
                && !p.ends_with("/recompute");
            if writes_model {
                assert_eq!(*g, Some("model.commit"), "{m} {p}");
            }
        }
        assert!(ROUTES.iter().any(|(m, p, g)| *m == "POST"
            && *p == "/assurance/changes/:change_id/commit"
            && *g == Some("model.commit")));
    }

    #[test]
    fn identities_files_are_validated() {
        let ok = |s: &str| parse_identities(s);
        let d = sha256_hex(b"t1");
        let d2 = sha256_hex(b"t2");
        assert!(ok(&format!(
            "[[identity]]\nid=\"a\"\ntoken_sha256=\"{d}\"\ngrants=[\"model.read\"]\n"
        ))
        .is_ok());
        assert!(matches!(ok(""), Err(IdentityError::Empty)));
        assert!(matches!(
            ok(&format!(
                "[[identity]]\nid=\"a\"\ntoken_sha256=\"{d}\"\ngrants=[\"root\"]\n"
            )),
            Err(IdentityError::UnknownGrant { .. })
        ));
        assert!(matches!(
            ok("[[identity]]\nid=\"a\"\ntoken_sha256=\"not-hex\"\ngrants=[]\n"),
            Err(IdentityError::BadDigest(_))
        ));
        let dup_id = format!(
            "[[identity]]\nid=\"a\"\ntoken_sha256=\"{d}\"\ngrants=[]\n[[identity]]\nid=\"a\"\ntoken_sha256=\"{d2}\"\ngrants=[]\n"
        );
        assert!(matches!(ok(&dup_id), Err(IdentityError::DuplicateId(_))));
        let dup_tok = format!(
            "[[identity]]\nid=\"a\"\ntoken_sha256=\"{d}\"\ngrants=[]\n[[identity]]\nid=\"b\"\ntoken_sha256=\"{d}\"\ngrants=[]\n"
        );
        assert!(matches!(
            ok(&dup_tok),
            Err(IdentityError::DuplicateToken(_, _))
        ));
        // a typo'd key is an error, not a silently dropped field
        assert!(ok(&format!(
            "[[identity]]\nid=\"a\"\ntoken_sha256=\"{d}\"\ngrant=[\"admin\"]\n"
        ))
        .is_err());
        // a plaintext token in the digest field is rejected, never stored
        assert!(matches!(
            ok("[[identity]]\nid=\"a\"\ntoken_sha256=\"my-secret-token\"\ngrants=[]\n"),
            Err(IdentityError::BadDigest(_))
        ));
    }

    #[test]
    fn authentication_by_mode() {
        let ids = vec![
            Identity::new("reader", &["model.read"], "tok-reader"),
            Identity::new("committer", &["model.read", "model.commit"], "tok-commit"),
        ];
        let gw = Gateway::new(AuthMode::Identities(ids), None);
        let who = |h: Option<&str>| match gw.authenticate(h) {
            Authn::Caller(i) => Some(i.id),
            Authn::Failed => None,
        };
        assert_eq!(who(Some("Bearer tok-reader")).as_deref(), Some("reader"));
        assert_eq!(who(Some("Bearer tok-commit")).as_deref(), Some("committer"));
        assert_eq!(who(Some("Bearer nope")), None);
        assert_eq!(who(Some("tok-reader")), None, "no Bearer scheme");
        assert_eq!(who(None), None);

        let shared = Gateway::new(AuthMode::SharedToken("s3cret".into()), None);
        assert!(
            matches!(shared.authenticate(Some("Bearer s3cret")), Authn::Caller(i) if i.id == "shared-token" && i.permits("anything"))
        );
        assert_eq!(shared.authenticate(Some("Bearer other")), Authn::Failed);

        let open = Gateway::new(AuthMode::Open, None);
        assert!(matches!(open.authenticate(None), Authn::Caller(i) if i.id == "anonymous"));
    }

    #[test]
    fn grants_decide_and_admin_implies_all() {
        let reader = Identity::new("r", &["model.read"], "t");
        assert!(reader.permits("model.read"));
        assert!(!reader.permits("model.commit"));
        assert!(Identity::new("a", &["admin"], "t").permits("model.commit"));
    }

    #[test]
    fn the_operation_is_named_by_its_mcp_tool_when_it_has_one() {
        let gw = Gateway::new(AuthMode::Open, None);
        let op = gw.operation(&Method::GET, Some("/model/projects"));
        assert_eq!(op.name, "list_model_projects");
        assert_eq!(op.grant, Some("model.read"));
        let op = gw.operation(&Method::POST, Some("/sparql"));
        assert_eq!(op.name, "POST /sparql");
        let op = gw.operation(&Method::GET, None);
        assert_eq!(
            (op.name.as_str(), op.grant),
            ("GET (no route)", Some("admin"))
        );
    }

    #[test]
    fn the_model_revision_comes_from_the_path_the_query_or_the_service() {
        let gw =
            Gateway::new(AuthMode::Open, None).with_model_revision(|| Some("model-now".into()));
        assert_eq!(
            gw.revision_for(
                "/model/projects/p/commits/c-42/elements",
                None,
                Some("/model/projects/:project_id/commits/:commit_id/elements")
            )
            .as_deref(),
            Some("c-42")
        );
        assert_eq!(
            gw.revision_for(
                "/assurance/changes/x/commit",
                Some("expected_revision=r7&x=1"),
                Some("/assurance/changes/:change_id/commit")
            )
            .as_deref(),
            Some("r7")
        );
        assert_eq!(
            gw.revision_for("/assurance/view", None, Some("/assurance/view"))
                .as_deref(),
            Some("model-now")
        );
        assert_eq!(gw.revision_for("/formats", None, Some("/formats")), None);
        assert_eq!(
            gw.revision_for(
                "/assurance/view",
                Some("revision="),
                Some("/assurance/view")
            )
            .as_deref(),
            Some("model-now")
        );
    }
}
