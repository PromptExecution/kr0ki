# MCP/HTTP Capability Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `kr0ki-server` the single source of truth for every MCP-callable capability's shape (name, JSON schema, HTTP binding), give KubeDiagrams rendering a real HTTP route for the first time, and turn `kr0ki-mcp`'s stdio bridge into a generic dispatcher over that manifest instead of hand-coded per-tool branches.

**Architecture:** A new closed `McpTool` enum in `kr0ki-core` (mirrors `DiagramFormat`'s shape) is the source of truth, served by a new `GET /mcp/tools` route. `POST /render/kubediagram` proxies to a new persistent HTTP listener inside the `kr0ki-mcp` container (`http_worker.py`, replacing that container's idle `sleep infinity`), which shells to the real `kube-diagrams` CLI already installed there. `bridge.py` (the stdio MCP bridge, unchanged invocation mechanism) fetches the manifest once and dispatches every `tools/call` generically from it.

**Tech Stack:** Rust (axum, reqwest, serde_json), Python 3 stdlib only (`http.server`, `urllib`, `subprocess`), Kubernetes Pod manifest (k0s).

**Spec:** `docs/superpowers/specs/2026-09-16-mcp-http-parity-design.md`

## Global Constraints

- No caching for `/render/kubediagram` this pass (backlog, post-MBSE — spec §1).
- One closed enum for this family only (`McpTool`) — do not unify with `DiagramFormat` or build a generalized cross-family framework (spec §1, YAGNI).
- `containers/kubediagram-mcp/` is deleted entirely once `http_worker.py` lands — its stdio-JSON-RPC-over-subprocess hop becomes redundant (spec §3.3).
- The 1 MiB manifest/source size limit is enforced exactly once, in `kr0ki-server`'s HTTP handler — not duplicated in `bridge.py` or `http_worker.py` (spec §5). `bridge.py` keeps its own 1 MiB check only because it's the outermost boundary for an MCP-originated call before any HTTP hop happens.
- Every new route follows the existing "unconfigured/unavailable is a clean 503, not a panic or 500" convention already used by `/b00t-graph` and `/capabilities`.
- **Deviation from spec §6:** the spec's testing section suggested adding the self-render check as a new entry in `fixtures/playbook-examples.json`. This plan does not do that: `crates/kr0ki-server/tests/http.rs::examples_catalog_covers_every_advertised_format` asserts the examples count equals `DiagramFormat::ALL.len()` exactly — that fixture is scoped to `DiagramFormat`, which `kubediagram` deliberately isn't part of (different request shape, spec §1's per-family YAGNI). Task 6 achieves the same validation goal (a permanent, runnable check that kr0ki renders its own deployment) as a dedicated `#[ignore]`-gated integration test reading `deploy/kr0ki-local.pod.yaml` via `include_str!`, matching `b00t_graph_live.rs`'s existing pattern, without touching that unrelated invariant.
- Every `AppState` struct literal across the codebase must be updated in the same task that adds a new field to `AppState` — the compiler will find them, but list them explicitly per task so nothing is missed mid-task.

---

### Task 1: `McpTool` closed enum in `kr0ki-core`

**Files:**
- Create: `crates/kr0ki-core/src/mcp_tool.rs`
- Modify: `crates/kr0ki-core/src/lib.rs` (add `pub mod mcp_tool;`)

**Interfaces:**
- Produces: `kr0ki_core::mcp_tool::McpTool` (enum: `RenderDiagram`, `ListFormats`, `RenderKubeDiagram`), `McpTool::ALL: &'static [McpTool]`, `McpTool::to_manifest_json(self) -> serde_json::Value`. Task 2 consumes `McpTool::ALL` and `to_manifest_json`.

- [ ] **Step 1: Write the failing test**

Create `crates/kr0ki-core/src/mcp_tool.rs` with just the test module first:

```rust
//! The MCP-callable capabilities kr0ki-server exposes, and how each maps onto
//! an HTTP request (kr0ki#mcp-http-parity, 2026-09-16 design). `GET
//! /mcp/tools` (kr0ki-server) serializes `McpTool::ALL` so the stdio MCP
//! bridge (`containers/kr0ki-mcp/bridge.py`) can dispatch every `tools/call`
//! generically instead of hand-coding one branch per tool name.
//!
//! One closed enum for this family only — see
//! `docs/superpowers/specs/2026-09-16-mcp-http-parity-design.md` §1 for why
//! this deliberately isn't unified with `DiagramFormat` or generalized across
//! future families.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_three_tools_have_unique_names() {
        let mut names: Vec<&str> = McpTool::ALL.iter().map(|t| t.name()).collect();
        let before = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), before, "duplicate McpTool name in ALL");
        assert_eq!(McpTool::ALL.len(), 3);
    }

    #[test]
    fn render_diagram_binds_format_to_path_source_to_body_output_to_query() {
        let binding = McpTool::RenderDiagram.http_binding();
        assert!(matches!(binding.method, HttpMethod::Post));
        assert_eq!(binding.path_template, "/render/{format}");
        assert_eq!(binding.args.len(), 3);
        assert!(binding
            .args
            .iter()
            .any(|a| a.name == "format" && matches!(a.placement, ArgPlacement::Path)));
        assert!(binding
            .args
            .iter()
            .any(|a| a.name == "source" && matches!(a.placement, ArgPlacement::Body)));
        assert!(binding
            .args
            .iter()
            .any(|a| a.name == "output" && matches!(a.placement, ArgPlacement::Query)));
    }

    #[test]
    fn list_formats_binds_to_a_plain_get_with_no_args() {
        let binding = McpTool::ListFormats.http_binding();
        assert!(matches!(binding.method, HttpMethod::Get));
        assert_eq!(binding.path_template, "/formats");
        assert!(binding.args.is_empty());
    }

    #[test]
    fn render_kube_diagram_binds_manifest_to_body_output_to_query() {
        let binding = McpTool::RenderKubeDiagram.http_binding();
        assert!(matches!(binding.method, HttpMethod::Post));
        assert_eq!(binding.path_template, "/render/kubediagram");
        assert_eq!(binding.args.len(), 2);
        assert!(binding
            .args
            .iter()
            .any(|a| a.name == "manifest" && matches!(a.placement, ArgPlacement::Body)));
        assert!(binding
            .args
            .iter()
            .any(|a| a.name == "output" && matches!(a.placement, ArgPlacement::Query)));
    }

    #[test]
    fn to_manifest_json_carries_both_the_mcp_schema_and_the_http_binding() {
        let json = McpTool::RenderKubeDiagram.to_manifest_json();
        assert_eq!(json["name"], "render_kubernetes_manifest");
        assert!(json["description"].is_string());
        assert!(json["inputSchema"]["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("manifest")));
        assert_eq!(json["httpBinding"]["method"], "POST");
        assert_eq!(json["httpBinding"]["pathTemplate"], "/render/kubediagram");
        let args = json["httpBinding"]["args"].as_array().unwrap();
        assert!(args
            .iter()
            .any(|a| a["name"] == "manifest" && a["placement"] == "body"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kr0ki-core --lib mcp_tool`
Expected: FAIL to compile — `McpTool`, `HttpMethod`, `ArgPlacement` not defined.

- [ ] **Step 3: Write the implementation**

Add above the `#[cfg(test)]` block in the same file:

```rust
/// One callable capability kr0ki-server exposes over both HTTP and MCP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpTool {
    RenderDiagram,
    ListFormats,
    RenderKubeDiagram,
}

impl McpTool {
    pub const ALL: &'static [McpTool] = &[Self::RenderDiagram, Self::ListFormats, Self::RenderKubeDiagram];

    pub const fn name(self) -> &'static str {
        match self {
            Self::RenderDiagram => "render_diagram",
            Self::ListFormats => "list_formats",
            Self::RenderKubeDiagram => "render_kubernetes_manifest",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::RenderDiagram => "Render supported diagram source through the local kr0ki service.",
            Self::ListFormats => "List formats currently supported by the local kr0ki service.",
            Self::RenderKubeDiagram => {
                "Render Kubernetes manifest YAML through the internal KubeDiagrams worker."
            }
        }
    }

    pub fn input_schema(self) -> serde_json::Value {
        match self {
            Self::RenderDiagram => serde_json::json!({
                "type": "object",
                "required": ["format", "source"],
                "properties": {
                    "format": {"type": "string", "description": "kr0ki diagram format slug."},
                    "source": {"type": "string", "description": "UTF-8 diagram source."},
                    "output": {"type": "string", "enum": ["svg", "png"], "default": "svg"}
                }
            }),
            Self::ListFormats => serde_json::json!({"type": "object", "properties": {}}),
            Self::RenderKubeDiagram => serde_json::json!({
                "type": "object",
                "required": ["manifest"],
                "properties": {
                    "manifest": {"type": "string", "description": "Kubernetes YAML manifest bundle."},
                    "output": {"type": "string", "enum": ["svg", "dot_json"], "default": "svg"}
                }
            }),
        }
    }

    pub const fn http_binding(self) -> HttpBinding {
        match self {
            Self::RenderDiagram => HttpBinding {
                method: HttpMethod::Post,
                path_template: "/render/{format}",
                args: &[
                    ArgBinding { name: "format", placement: ArgPlacement::Path },
                    ArgBinding { name: "source", placement: ArgPlacement::Body },
                    ArgBinding { name: "output", placement: ArgPlacement::Query },
                ],
            },
            Self::ListFormats => HttpBinding {
                method: HttpMethod::Get,
                path_template: "/formats",
                args: &[],
            },
            Self::RenderKubeDiagram => HttpBinding {
                method: HttpMethod::Post,
                path_template: "/render/kubediagram",
                args: &[
                    ArgBinding { name: "manifest", placement: ArgPlacement::Body },
                    ArgBinding { name: "output", placement: ArgPlacement::Query },
                ],
            },
        }
    }

    /// The full manifest entry `GET /mcp/tools` serves for this tool — both
    /// the MCP schema half (used verbatim as an MCP `tools/list` entry) and
    /// the HTTP binding half (used by the stdio bridge's generic dispatcher
    /// to place a `tools/call`'s arguments on the wire). See module docs on
    /// why these aren't split.
    pub fn to_manifest_json(self) -> serde_json::Value {
        let binding = self.http_binding();
        serde_json::json!({
            "name": self.name(),
            "description": self.description(),
            "inputSchema": self.input_schema(),
            "httpBinding": {
                "method": match binding.method {
                    HttpMethod::Get => "GET",
                    HttpMethod::Post => "POST",
                },
                "pathTemplate": binding.path_template,
                "args": binding.args.iter().map(|a| serde_json::json!({
                    "name": a.name,
                    "placement": match a.placement {
                        ArgPlacement::Path => "path",
                        ArgPlacement::Query => "query",
                        ArgPlacement::Body => "body",
                    }
                })).collect::<Vec<_>>()
            }
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum HttpMethod {
    Get,
    Post,
}

/// Where in the HTTP request a `tools/call` argument by this name lands.
#[derive(Debug, Clone, Copy)]
pub enum ArgPlacement {
    /// Substituted into the path template at `{name}`.
    Path,
    /// Becomes a `?name=value` query parameter.
    Query,
    /// This argument's string value becomes the entire raw request body.
    Body,
}

#[derive(Debug, Clone, Copy)]
pub struct ArgBinding {
    pub name: &'static str,
    pub placement: ArgPlacement,
}

#[derive(Debug, Clone, Copy)]
pub struct HttpBinding {
    pub method: HttpMethod,
    pub path_template: &'static str,
    pub args: &'static [ArgBinding],
}
```

- [ ] **Step 4: Register the module**

In `crates/kr0ki-core/src/lib.rs`, change:

```rust
pub mod k8s_recognizer;
pub mod probe;
```

to:

```rust
pub mod k8s_recognizer;
pub mod mcp_tool;
pub mod probe;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p kr0ki-core --lib mcp_tool`
Expected: PASS (5 tests).

- [ ] **Step 6: Format, lint, commit**

```bash
cargo fmt -p kr0ki-core
cargo clippy -p kr0ki-core --all-targets -- -D warnings
git add crates/kr0ki-core/src/mcp_tool.rs crates/kr0ki-core/src/lib.rs
git commit -m "feat: add McpTool closed enum — the MCP/HTTP capability manifest source of truth"
```

---

### Task 2: `GET /mcp/tools` route

**Files:**
- Modify: `crates/kr0ki-server/src/app.rs`
- Modify: `crates/kr0ki-server/tests/http.rs`

**Interfaces:**
- Consumes: `kr0ki_core::mcp_tool::McpTool` (Task 1).
- Produces: nothing new consumed by later tasks (this route is a leaf for now; `bridge.py` in Task 5 calls it over HTTP, not as a Rust dependency).

- [ ] **Step 1: Write the failing test**

Add to `crates/kr0ki-server/tests/http.rs`, after `formats_lists_supported_slugs_only`:

```rust
#[tokio::test]
async fn mcp_tools_lists_all_three_tools_with_bindings() {
    let app = test_app(test_state("mcp-tools"));
    let resp = app
        .oneshot(Request::get("/mcp/tools").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    let tools: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    assert_eq!(tools.len(), 3);
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"render_diagram"));
    assert!(names.contains(&"list_formats"));
    assert!(names.contains(&"render_kubernetes_manifest"));

    let render = tools
        .iter()
        .find(|t| t["name"] == "render_diagram")
        .unwrap();
    assert_eq!(render["httpBinding"]["method"], "POST");
    assert_eq!(render["httpBinding"]["pathTemplate"], "/render/{format}");
    assert!(render["inputSchema"]["required"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("format")));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kr0ki-server --test http mcp_tools_lists_all_three_tools_with_bindings`
Expected: FAIL — 404 (no such route yet).

- [ ] **Step 3: Write the implementation**

In `crates/kr0ki-server/src/app.rs`, add `"/mcp/tools"` to the router (right after `/formats`):

```rust
        .route("/formats", get(formats))
        .route("/mcp/tools", get(mcp_tools))
```

Add the handler right after `formats()`:

```rust
/// `GET /mcp/tools` — the MCP/HTTP capability manifest (mcp-http-parity
/// design, 2026-09-16). One entry per `McpTool`, each carrying both its MCP
/// schema and its HTTP binding — `containers/kr0ki-mcp/bridge.py` fetches
/// this once and dispatches every `tools/call` generically from it.
async fn mcp_tools() -> Json<Vec<serde_json::Value>> {
    Json(
        kr0ki_core::mcp_tool::McpTool::ALL
            .iter()
            .map(|t| t.to_manifest_json())
            .collect(),
    )
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p kr0ki-server --test http mcp_tools_lists_all_three_tools_with_bindings`
Expected: PASS.

- [ ] **Step 5: Format, lint, commit**

```bash
cargo fmt -p kr0ki-server
cargo clippy -p kr0ki-server --all-targets -- -D warnings
git add crates/kr0ki-server/src/app.rs crates/kr0ki-server/tests/http.rs
git commit -m "feat: add GET /mcp/tools capability manifest route"
```

---

### Task 3: `POST /render/kubediagram` proxy route

**Files:**
- Modify: `crates/kr0ki-server/Cargo.toml` (add `reqwest` to `[dependencies]`, `wiremock` to `[dev-dependencies]`)
- Modify: `crates/kr0ki-server/src/app.rs` (`AppState.kubediagram_worker_url`, new route + handler)
- Modify: `crates/kr0ki-server/src/main.rs` (env wiring)
- Modify: `crates/kr0ki-server/tests/http.rs` (update `test_state`, new tests)
- Modify: `crates/kr0ki-server/tests/b00t_graph_live.rs` (update the one other `AppState` literal)

**Interfaces:**
- Produces: `AppState.kubediagram_worker_url: Option<String>` — Task 6's live test constructs `AppState` directly and needs this field name.

- [ ] **Step 1: Add dependencies**

In `crates/kr0ki-server/Cargo.toml`, change:

```toml
[dependencies]
kr0ki-core = { path = "../kr0ki-core" }
anyhow.workspace = true
serde = { workspace = true }
serde_json.workspace = true
tokio = { workspace = true }
axum.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true

[dev-dependencies]
reqwest = { workspace = true, features = ["json"] }
tower = { version = "0.5", features = ["util"] }
http-body-util = "0.1"
```

to:

```toml
[dependencies]
kr0ki-core = { path = "../kr0ki-core" }
anyhow.workspace = true
serde = { workspace = true }
serde_json.workspace = true
tokio = { workspace = true }
axum.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
# POST /render/kubediagram proxies to the kr0ki-mcp sidecar's internal
# http_worker.py listener — mcp-http-parity design, 2026-09-16.
reqwest = { workspace = true }

[dev-dependencies]
reqwest = { workspace = true, features = ["json"] }
tower = { version = "0.5", features = ["util"] }
http-body-util = "0.1"
# Mocks the kubediagram worker's HTTP responses in render_kubediagram tests.
wiremock = "0.6"
```

- [ ] **Step 2: Write the failing tests**

In `crates/kr0ki-server/tests/http.rs`, first update `test_state` (add the new field) and `b00t_graph_live.rs`'s single `AppState` literal — both currently list every field explicitly, so the compiler will refuse to build until both are updated:

In `test_state()` in `tests/http.rs`, change:

```rust
    AppState {
        service: Arc::new(service),
        playbook_dir: std::env::temp_dir().join("kr0ki-no-playbook-assets"),
        b00t_graph_artifacts_path: None,
        capabilities_path: None,
    }
}
```

to:

```rust
    AppState {
        service: Arc::new(service),
        playbook_dir: std::env::temp_dir().join("kr0ki-no-playbook-assets"),
        b00t_graph_artifacts_path: None,
        capabilities_path: None,
        kubediagram_worker_url: None,
    }
}
```

In `crates/kr0ki-server/tests/b00t_graph_live.rs`, change:

```rust
        b00t_graph_artifacts_path: Some(base.clone()),
        capabilities_path: None,
    };
```

to:

```rust
        b00t_graph_artifacts_path: Some(base.clone()),
        capabilities_path: None,
        kubediagram_worker_url: None,
    };
```

Now add these tests to `tests/http.rs`, after `capabilities_returns_the_file_contents_as_json`:

```rust
#[tokio::test]
async fn render_kubediagram_is_503_when_not_configured() {
    // test_state() leaves kubediagram_worker_url: None.
    let app = test_app(test_state("kubediagram-unconfigured"));
    let resp = app
        .oneshot(
            Request::post("/render/kubediagram")
                .body(Body::from("apiVersion: v1\nkind: Pod"))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("kubediagram_worker_not_configured"));
}

fn test_state_with_kubediagram_worker(tag: &str, worker_url: String) -> AppState {
    let mut state = test_state(tag);
    state.kubediagram_worker_url = Some(worker_url);
    state
}

#[tokio::test]
async fn render_kubediagram_empty_body_is_400_before_any_worker_call() {
    // http://127.0.0.1:1 is unreachable — if the handler validates before
    // proxying, this call never actually reaches it.
    let app = test_app(test_state_with_kubediagram_worker(
        "kubediagram-empty",
        "http://127.0.0.1:1".to_string(),
    ));
    let resp = app
        .oneshot(
            Request::post("/render/kubediagram")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("empty_manifest"));
}

#[tokio::test]
async fn render_kubediagram_rejects_invalid_output_before_any_worker_call() {
    let app = test_app(test_state_with_kubediagram_worker(
        "kubediagram-badoutput",
        "http://127.0.0.1:1".to_string(),
    ));
    let resp = app
        .oneshot(
            Request::post("/render/kubediagram?output=png")
                .body(Body::from("apiVersion: v1\nkind: Pod"))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("invalid_output"));
}

#[tokio::test]
async fn render_kubediagram_proxies_to_the_worker_and_returns_svg() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/render"))
        .and(wiremock::matchers::query_param("output", "svg"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_bytes(b"<svg>ok</svg>".to_vec()),
        )
        .mount(&server)
        .await;

    let app = test_app(test_state_with_kubediagram_worker(
        "kubediagram-proxy",
        server.uri(),
    ));
    let resp = app
        .oneshot(
            Request::post("/render/kubediagram")
                .body(Body::from("apiVersion: v1\nkind: Pod"))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("<svg>ok</svg>"));
}

#[tokio::test]
async fn render_kubediagram_maps_worker_422_to_bad_manifest() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/render"))
        .respond_with(
            wiremock::ResponseTemplate::new(422).set_body_string("kube-diagrams failed: bad yaml"),
        )
        .mount(&server)
        .await;

    let app = test_app(test_state_with_kubediagram_worker(
        "kubediagram-badmanifest",
        server.uri(),
    ));
    let resp = app
        .oneshot(
            Request::post("/render/kubediagram")
                .body(Body::from("not: valid: yaml: at all"))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body.contains("bad_manifest"));
}

#[tokio::test]
async fn render_kubediagram_is_503_when_worker_unreachable() {
    let app = test_app(test_state_with_kubediagram_worker(
        "kubediagram-unreachable",
        "http://127.0.0.1:1".to_string(),
    ));
    let resp = app
        .oneshot(
            Request::post("/render/kubediagram")
                .body(Body::from("apiVersion: v1\nkind: Pod"))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("kubediagram_worker_unreachable"));
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p kr0ki-server --test http kubediagram`
Expected: FAIL to compile — `AppState` has no field `kubediagram_worker_url`.

- [ ] **Step 4: Write the implementation**

In `crates/kr0ki-server/src/app.rs`, add the field to `AppState`:

```rust
    /// Path to the `kroki` container's self-reported `capabilities.json`
    /// (kr0ki#20), written to a shared pod volume at its startup — see
    /// `deploy/kr0ki-local.pod.yaml`. `None` disables `/capabilities` (503).
    pub capabilities_path: Option<PathBuf>,
    /// Base URL of the `kr0ki-mcp` sidecar's internal KubeDiagrams-rendering
    /// listener (`http_worker.py`), e.g. `http://127.0.0.1:8788` — mcp-http-
    /// parity design, 2026-09-16. `None` disables `/render/kubediagram`
    /// (503, not a panic).
    pub kubediagram_worker_url: Option<String>,
}
```

Add the route:

```rust
        .route("/render/:format", post(render))
        .route("/render/kubediagram", post(render_kubediagram))
```

Add the handler right after `b00t_graph` (before `fn rendered_response`):

```rust
/// `POST /render/kubediagram?output=svg|dot_json` — mcp-http-parity design,
/// 2026-09-16. Body is a Kubernetes manifest (multi-doc YAML). Proxies to
/// the `kr0ki-mcp` sidecar's internal `/render` listener — never through
/// kr0ki-server's own content-addressed cache (backlog, see the design
/// doc's §1 — kube-diagrams' output isn't itself Kroki-renderable text, so
/// it needs its own cache-key derivation, deliberately deferred).
async fn render_kubediagram(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    body: Bytes,
) -> Response {
    let Some(worker_url) = state.kubediagram_worker_url.as_ref() else {
        return error_json(
            StatusCode::SERVICE_UNAVAILABLE,
            "kubediagram_worker_not_configured",
            "KR0KI_KUBEDIAGRAM_WORKER_URL is not set on this server",
        );
    };

    if body.is_empty() {
        return error_json(
            StatusCode::BAD_REQUEST,
            "empty_manifest",
            "kubernetes manifest body is empty",
        );
    }

    let output = params.get("output").map(String::as_str).unwrap_or("svg");
    if output != "svg" && output != "dot_json" {
        return error_json(
            StatusCode::BAD_REQUEST,
            "invalid_output",
            "output must be svg or dot_json",
        );
    }

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{worker_url}/render?output={output}"))
        .body(body.to_vec())
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {
            let content_type = if output == "svg" {
                "image/svg+xml"
            } else {
                "application/json"
            };
            match r.bytes().await {
                Ok(bytes) => (StatusCode::OK, [(header::CONTENT_TYPE, content_type)], bytes).into_response(),
                Err(e) => error_json(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "kubediagram_worker_read_failed",
                    &e.to_string(),
                ),
            }
        }
        Ok(r) if r.status() == StatusCode::UNPROCESSABLE_ENTITY => {
            let body = r.text().await.unwrap_or_default();
            error_json(StatusCode::UNPROCESSABLE_ENTITY, "bad_manifest", &body)
        }
        Ok(r) => {
            let status = r.status();
            let body = r.text().await.unwrap_or_default();
            error_json(
                StatusCode::BAD_GATEWAY,
                "kubediagram_worker_error",
                &format!("{status}: {body}"),
            )
        }
        Err(e) => error_json(
            StatusCode::SERVICE_UNAVAILABLE,
            "kubediagram_worker_unreachable",
            &e.to_string(),
        ),
    }
}
```

In `crates/kr0ki-server/src/main.rs`, add env wiring. Change the doc header:

```rust
//!   KR0KI_CAPABILITIES_PATH    if set, enables GET /capabilities (kr0ki#20) —
//!                              the kroki container's self-reported
//!                              companion-required status, on a shared pod
//!                              volume it wrote at its own startup. Unset
//!                              disables the route with a 503, not a panic.
```

to:

```rust
//!   KR0KI_CAPABILITIES_PATH    if set, enables GET /capabilities (kr0ki#20) —
//!                              the kroki container's self-reported
//!                              companion-required status, on a shared pod
//!                              volume it wrote at its own startup. Unset
//!                              disables the route with a 503, not a panic.
//!   KR0KI_KUBEDIAGRAM_WORKER_URL if set, enables POST /render/kubediagram —
//!                              the kr0ki-mcp sidecar's internal
//!                              http_worker.py listener, e.g.
//!                              http://127.0.0.1:8788. Unset disables the
//!                              route with a 503, not a panic.
```

Change:

```rust
    let capabilities_path = std::env::var("KR0KI_CAPABILITIES_PATH")
        .ok()
        .map(std::path::PathBuf::from);
```

to:

```rust
    let capabilities_path = std::env::var("KR0KI_CAPABILITIES_PATH")
        .ok()
        .map(std::path::PathBuf::from);
    let kubediagram_worker_url = std::env::var("KR0KI_KUBEDIAGRAM_WORKER_URL").ok();
```

Change:

```rust
        capabilities_path = ?capabilities_path.as_ref().map(|p| p.display().to_string()),
        "starting kr0ki"
    );
```

to:

```rust
        capabilities_path = ?capabilities_path.as_ref().map(|p| p.display().to_string()),
        kubediagram_worker_url = ?kubediagram_worker_url,
        "starting kr0ki"
    );
```

Change:

```rust
    let state = AppState {
        service: Arc::new(service),
        playbook_dir,
        b00t_graph_artifacts_path,
        capabilities_path,
    };
```

to:

```rust
    let state = AppState {
        service: Arc::new(service),
        playbook_dir,
        b00t_graph_artifacts_path,
        capabilities_path,
        kubediagram_worker_url,
    };
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p kr0ki-server`
Expected: PASS (all tests, including the 5 new ones and the pre-existing suite).

- [ ] **Step 6: Format, lint, commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/kr0ki-server/Cargo.toml crates/kr0ki-server/src/app.rs \
        crates/kr0ki-server/src/main.rs crates/kr0ki-server/tests/http.rs \
        crates/kr0ki-server/tests/b00t_graph_live.rs Cargo.lock
git commit -m "feat: add POST /render/kubediagram, proxying to the kr0ki-mcp sidecar"
```

---

### Task 4: `http_worker.py` — persistent internal listener in the `kr0ki-mcp` container

**Files:**
- Create: `containers/kr0ki-mcp/http_worker.py`
- Create: `containers/kr0ki-mcp/test_http_worker.py`
- Delete: `containers/kubediagram-mcp/` (both `bridge.py` and `Containerfile` — already confirmed, in the 2026-09-16 conversation that produced this plan's spec, that neither `justfile` nor any `deploy/*.yaml` builds or references a standalone `kubediagram-mcp` image; `containers/kr0ki-mcp/Containerfile` only ever `COPY`s its `bridge.py` source file in, per Step 6 below)
- Modify: `containers/kr0ki-mcp/Containerfile`
- Modify: `deploy/kr0ki-local.pod.yaml`

**Interfaces:**
- Produces: an HTTP service on `0.0.0.0:{KR0KI_MCP_WORKER_PORT:-8788}` with `GET /health` and `POST /render?output=svg|dot_json`. Task 3's `render_kubediagram` handler is this worker's only caller; Task 6's live test exercises it end to end via that route.

- [ ] **Step 1: Write the failing test**

Create `containers/kr0ki-mcp/test_http_worker.py`:

```python
#!/usr/bin/env python3
"""Unit tests for http_worker.py. Run with:
   cd containers/kr0ki-mcp && python3 test_http_worker.py
"""
import http.client
import json
import threading
import unittest
from unittest.mock import Mock, patch

import http_worker


class HttpWorkerTest(unittest.TestCase):
    def setUp(self):
        self.server = http_worker.ThreadingHTTPServer(("127.0.0.1", 0), http_worker.Handler)
        self.port = self.server.server_address[1]
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()

    def _conn(self):
        return http.client.HTTPConnection("127.0.0.1", self.port, timeout=5)

    def test_health_returns_ok(self):
        conn = self._conn()
        conn.request("GET", "/health")
        resp = conn.getresponse()
        self.assertEqual(resp.status, 200)
        self.assertEqual(json.loads(resp.read())["status"], "ok")

    def test_unknown_path_is_404(self):
        conn = self._conn()
        conn.request("GET", "/nope")
        resp = conn.getresponse()
        self.assertEqual(resp.status, 404)

    def test_render_with_empty_body_is_400(self):
        conn = self._conn()
        conn.request("POST", "/render", body=b"")
        resp = conn.getresponse()
        self.assertEqual(resp.status, 400)
        self.assertIn(b"empty manifest", resp.read())

    def test_render_with_invalid_output_is_400(self):
        conn = self._conn()
        conn.request("POST", "/render?output=png", body=b"apiVersion: v1\nkind: Pod")
        resp = conn.getresponse()
        self.assertEqual(resp.status, 400)

    def test_render_over_size_limit_is_400(self):
        conn = self._conn()
        oversized = b"a" * (http_worker.MAX_MANIFEST_BYTES + 1)
        conn.request("POST", "/render", body=oversized)
        resp = conn.getresponse()
        self.assertEqual(resp.status, 400)
        self.assertIn(b"1 MiB", resp.read())

    @patch("http_worker.subprocess.run")
    def test_render_success_returns_artifact_bytes(self, mock_run):
        def fake_run(cmd, input, stdout, stderr, timeout, check):
            artifact_path = cmd[cmd.index("-o") + 1]
            with open(artifact_path, "wb") as f:
                f.write(b"<svg>fake</svg>")
            return Mock(returncode=0, stderr=b"")

        mock_run.side_effect = fake_run
        conn = self._conn()
        conn.request("POST", "/render", body=b"apiVersion: v1\nkind: Pod")
        resp = conn.getresponse()
        self.assertEqual(resp.status, 200)
        self.assertEqual(resp.read(), b"<svg>fake</svg>")
        self.assertEqual(resp.getheader("Content-Type"), "image/svg+xml")

    @patch("http_worker.subprocess.run")
    def test_render_failure_returns_422(self, mock_run):
        mock_run.return_value = Mock(returncode=1, stderr=b"bad manifest")
        conn = self._conn()
        conn.request("POST", "/render", body=b"not a manifest")
        resp = conn.getresponse()
        self.assertEqual(resp.status, 422)
        self.assertIn(b"bad manifest", resp.read())


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd containers/kr0ki-mcp && python3 test_http_worker.py`
Expected: FAIL — `ModuleNotFoundError: No module named 'http_worker'`.

- [ ] **Step 3: Write the implementation**

Create `containers/kr0ki-mcp/http_worker.py`:

```python
#!/usr/bin/env python3
"""Persistent internal HTTP listener for KubeDiagrams rendering.

Runs as the kr0ki-mcp container's own long-running process (its ENTRYPOINT).
kr0ki-server's POST /render/kubediagram proxies here directly over the pod's
localhost network — this replaces the old stdio-JSON-RPC-over-subprocess hop
that containers/kubediagram-mcp/bridge.py used to provide. Internal-only:
never exposed outside the pod's network namespace.
"""

import json
import os
import subprocess
import tempfile
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse

MAX_MANIFEST_BYTES = 1_048_576


class Handler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        pass  # quiet; matches kr0ki-mcp's existing minimal logging

    def do_GET(self):
        if self.path == "/health":
            self._respond(200, b'{"status":"ok","service":"kubediagram-worker"}', "application/json")
            return
        self._respond(404, b'{"error":"not_found"}', "application/json")

    def do_POST(self):
        parsed = urlparse(self.path)
        if parsed.path != "/render":
            self._respond(404, b'{"error":"not_found"}', "application/json")
            return

        output = parse_qs(parsed.query).get("output", ["svg"])[0]
        if output not in {"svg", "dot_json"}:
            self._respond(400, json.dumps({"error": "invalid output"}).encode(), "application/json")
            return

        length = int(self.headers.get("Content-Length", 0))
        if length == 0:
            self._respond(400, json.dumps({"error": "empty manifest"}).encode(), "application/json")
            return
        if length > MAX_MANIFEST_BYTES:
            self._respond(
                400,
                json.dumps({"error": "manifest exceeds the 1 MiB limit"}).encode(),
                "application/json",
            )
            return
        manifest = self.rfile.read(length)

        with tempfile.TemporaryDirectory() as temporary_directory:
            artifact = Path(temporary_directory) / f"diagram.{output}"
            process = subprocess.run(
                ["kube-diagrams", "-", "-f", output, "-o", str(artifact)],
                input=manifest,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=60,
                check=False,
            )
            if process.returncode != 0:
                self._respond(422, process.stderr[:2000], "text/plain")
                return
            if not artifact.is_file():
                self._respond(500, b"kube-diagrams completed without an output artifact", "text/plain")
                return
            content_type = "image/svg+xml" if output == "svg" else "application/json"
            self._respond(200, artifact.read_bytes(), content_type)

    def _respond(self, status, body, content_type):
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def main():
    port = int(os.environ.get("KR0KI_MCP_WORKER_PORT", "8788"))
    server = ThreadingHTTPServer(("0.0.0.0", port), Handler)
    server.serve_forever()


if __name__ == "__main__":
    main()
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd containers/kr0ki-mcp && python3 test_http_worker.py`
Expected: PASS (7 tests). Note: this does NOT require `kube-diagrams` to be
installed — the two subprocess-dependent tests mock `subprocess.run`.

- [ ] **Step 5: Delete the now-redundant `kubediagram-mcp` worker**

```bash
git rm -r containers/kubediagram-mcp
```

- [ ] **Step 6: Update the Containerfile**

In `containers/kr0ki-mcp/Containerfile`, change:

```dockerfile
WORKDIR /opt/kr0ki-mcp
COPY containers/kr0ki-mcp/bridge.py /opt/kr0ki-mcp/bridge.py
COPY containers/kubediagram-mcp/bridge.py /opt/kubediagram-mcp/bridge.py
COPY vendor/kubediagrams/bin /usr/local/bin/

USER 65532:65532
ENTRYPOINT ["python3", "/opt/kr0ki-mcp/bridge.py"]
```

to:

```dockerfile
WORKDIR /opt/kr0ki-mcp
COPY containers/kr0ki-mcp/bridge.py /opt/kr0ki-mcp/bridge.py
COPY containers/kr0ki-mcp/http_worker.py /opt/kr0ki-mcp/http_worker.py
COPY vendor/kubediagrams/bin /usr/local/bin/

USER 65532:65532
# http_worker.py is the container's real, long-running service (mcp-http-
# parity design, 2026-09-16). bridge.py (stdio MCP) is invoked explicitly
# per-session via `kubectl exec` — see .mcp.json — independent of this
# ENTRYPOINT.
ENTRYPOINT ["python3", "/opt/kr0ki-mcp/http_worker.py"]
```

- [ ] **Step 7: Update the pod manifest**

In `deploy/kr0ki-local.pod.yaml`, change the `kr0ki-mcp` container's spec from:

```yaml
    - name: kr0ki-mcp
      image: localhost/kr0ki-mcp:dev
      imagePullPolicy: Never
      command: ["sleep", "infinity"]
      env:
        - name: KR0KI_URL
          value: http://127.0.0.1:8787
      resources:
        limits:
          cpu: "250m"
          memory: 128Mi
      volumeMounts:
        - name: kubediagram-tmp
          mountPath: /tmp
      securityContext:
        allowPrivilegeEscalation: false
        capabilities:
          drop: ["ALL"]
        readOnlyRootFilesystem: true
```

to:

```yaml
    - name: kr0ki-mcp
      image: localhost/kr0ki-mcp:dev
      imagePullPolicy: Never
      env:
        - name: KR0KI_URL
          value: http://127.0.0.1:8787
      ports:
        - containerPort: 8788
      readinessProbe:
        httpGet:
          path: /health
          port: 8788
        initialDelaySeconds: 1
        periodSeconds: 2
        timeoutSeconds: 2
        failureThreshold: 15
      resources:
        limits:
          cpu: "250m"
          memory: 128Mi
      volumeMounts:
        - name: kubediagram-tmp
          mountPath: /tmp
      securityContext:
        allowPrivilegeEscalation: false
        capabilities:
          drop: ["ALL"]
        readOnlyRootFilesystem: true
```

(The `command: ["sleep", "infinity"]` line is removed — the image's new `ENTRYPOINT` from Step 6 is now the container's real process.)

In the same file, add the new env var to the `kr0ki` container (right after `KR0KI_CAPABILITIES_PATH`):

```yaml
        # kr0ki#20 — the kroki container's own self-reported companion status,
        # written to this same shared volume at its startup.
        - name: KR0KI_CAPABILITIES_PATH
          value: /var/lib/kr0ki/capabilities/capabilities.json
        # mcp-http-parity design, 2026-09-16 — the kr0ki-mcp sidecar's
        # internal KubeDiagrams-rendering listener, reachable over the pod's
        # shared network namespace exactly like KR0KI_BACKEND_URL is.
        - name: KR0KI_KUBEDIAGRAM_WORKER_URL
          value: http://127.0.0.1:8788
```

- [ ] **Step 8: Commit**

```bash
git add containers/kr0ki-mcp/http_worker.py containers/kr0ki-mcp/test_http_worker.py \
        containers/kr0ki-mcp/Containerfile deploy/kr0ki-local.pod.yaml
git commit -m "feat: add http_worker.py, the kr0ki-mcp sidecar's internal KubeDiagrams listener"
```

---

### Task 5: Generic manifest-driven dispatch in the stdio `bridge.py`

**Files:**
- Modify: `containers/kr0ki-mcp/bridge.py`
- Create: `containers/kr0ki-mcp/test_bridge.py`

**Interfaces:**
- Consumes: `GET {KR0KI_URL}/mcp/tools`'s response shape from Task 2 (`name`/`description`/`inputSchema`/`httpBinding.{method,pathTemplate,args[].{name,placement}}`).

- [ ] **Step 1: Write the failing test**

Create `containers/kr0ki-mcp/test_bridge.py`:

```python
#!/usr/bin/env python3
"""Unit tests for bridge.py's generic manifest-driven dispatch. Run with:
   cd containers/kr0ki-mcp && python3 test_bridge.py
"""
import json
import unittest
from unittest.mock import patch

import bridge

SAMPLE_MANIFEST = [
    {
        "name": "render_diagram",
        "description": "Render supported diagram source through the local kr0ki service.",
        "inputSchema": {"type": "object"},
        "httpBinding": {
            "method": "POST",
            "pathTemplate": "/render/{format}",
            "args": [
                {"name": "format", "placement": "path"},
                {"name": "source", "placement": "body"},
                {"name": "output", "placement": "query"},
            ],
        },
    },
    {
        "name": "list_formats",
        "description": "List formats currently supported by the local kr0ki service.",
        "inputSchema": {"type": "object"},
        "httpBinding": {"method": "GET", "pathTemplate": "/formats", "args": []},
    },
]


class BridgeDispatchTest(unittest.TestCase):
    def setUp(self):
        bridge._manifest_cache = None

    @patch("bridge.http_call")
    def test_tools_list_uses_fetched_manifest(self, mock_http_call):
        mock_http_call.return_value = ("application/json", json.dumps(SAMPLE_MANIFEST).encode())
        result = bridge.handle({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})
        names = [t["name"] for t in result["result"]["tools"]]
        self.assertEqual(names, ["render_diagram", "list_formats"])
        mock_http_call.assert_called_once_with("GET", f"{bridge.BASE_URL}/mcp/tools")

    @patch("bridge.http_call")
    def test_render_diagram_call_maps_path_body_and_query(self, mock_http_call):
        mock_http_call.side_effect = [
            ("application/json", json.dumps(SAMPLE_MANIFEST).encode()),
            ("image/svg+xml", b"<svg>ok</svg>"),
        ]
        result = bridge.handle(
            {
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {
                    "name": "render_diagram",
                    "arguments": {"format": "d2", "source": "a -> b", "output": "svg"},
                },
            }
        )
        method, url, body = mock_http_call.call_args_list[1][0]
        self.assertEqual(method, "POST")
        self.assertEqual(url, f"{bridge.BASE_URL}/render/d2?output=svg")
        self.assertEqual(body, b"a -> b")
        self.assertEqual(result["result"]["content"][0]["text"], "<svg>ok</svg>")

    @patch("bridge.http_call")
    def test_list_formats_call_has_no_body_and_no_query(self, mock_http_call):
        mock_http_call.side_effect = [
            ("application/json", json.dumps(SAMPLE_MANIFEST).encode()),
            ("application/json", b'["d2","plantuml"]'),
        ]
        bridge.handle(
            {
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {"name": "list_formats", "arguments": {}},
            }
        )
        method, url, body = mock_http_call.call_args_list[1][0]
        self.assertEqual(method, "GET")
        self.assertEqual(url, f"{bridge.BASE_URL}/formats")
        self.assertIsNone(body)

    @patch("bridge.http_call")
    def test_unknown_tool_name_is_a_tool_error(self, mock_http_call):
        mock_http_call.return_value = ("application/json", json.dumps(SAMPLE_MANIFEST).encode())
        result = bridge.handle(
            {
                "jsonrpc": "2.0",
                "id": 4,
                "method": "tools/call",
                "params": {"name": "does_not_exist", "arguments": {}},
            }
        )
        self.assertTrue(result["result"]["isError"])
        self.assertIn("unknown tool", result["result"]["content"][0]["text"])

    def test_oversized_source_is_rejected_before_any_http_call(self):
        bridge._manifest_cache = SAMPLE_MANIFEST
        result = bridge.handle(
            {
                "jsonrpc": "2.0",
                "id": 5,
                "method": "tools/call",
                "params": {
                    "name": "render_diagram",
                    "arguments": {"format": "d2", "source": "a" * 1_048_577},
                },
            }
        )
        self.assertTrue(result["result"]["isError"])
        self.assertIn("1 MiB", result["result"]["content"][0]["text"])


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd containers/kr0ki-mcp && python3 test_bridge.py`
Expected: FAIL — `bridge` module's top-level `for line in sys.stdin:` loop blocks forever on import (the old `bridge.py` has no `if __name__ == "__main__":` guard), or `AttributeError` for `http_call`/`_manifest_cache` not existing yet.

- [ ] **Step 3: Write the implementation**

Replace `containers/kr0ki-mcp/bridge.py` entirely with:

```python
#!/usr/bin/env python3
"""Minimal stdio MCP bridge for a locally running kr0ki service.

Tool definitions (name, schema, HTTP binding) are fetched once from
kr0ki-server's GET /mcp/tools manifest and used to dispatch every
tools/call generically — adding a new McpTool variant on the Rust side
(crates/kr0ki-core/src/mcp_tool.rs) needs zero changes here.
"""

import base64
import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request

BASE_URL = os.environ.get("KR0KI_URL", "http://host.containers.internal:8787").rstrip("/")
AUTH_TOKEN = os.environ.get("KR0KI_AUTH_TOKEN")
MAX_INPUT_BYTES = 1_048_576

_manifest_cache = None


def response(request_id, result):
    return {"jsonrpc": "2.0", "id": request_id, "result": result}


def error(request_id, code, message):
    return {"jsonrpc": "2.0", "id": request_id, "error": {"code": code, "message": message}}


def tool_error(message):
    return {"content": [{"type": "text", "text": message}], "isError": True}


def http_call(method, url, data=None):
    headers = {"Accept": "application/json"}
    if AUTH_TOKEN:
        headers["Authorization"] = f"Bearer {AUTH_TOKEN}"
    if data is not None:
        headers["Content-Type"] = "text/plain; charset=utf-8"
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    with urllib.request.urlopen(req, timeout=60) as result:
        return result.headers.get_content_type(), result.read()


def fetch_manifest():
    global _manifest_cache
    if _manifest_cache is None:
        _, body = http_call("GET", f"{BASE_URL}/mcp/tools")
        _manifest_cache = json.loads(body)
    return _manifest_cache


def mcp_tools_list():
    return [
        {"name": t["name"], "description": t["description"], "inputSchema": t["inputSchema"]}
        for t in fetch_manifest()
    ]


def find_tool(name):
    for t in fetch_manifest():
        if t["name"] == name:
            return t
    return None


def apply_binding(tool, arguments):
    """Turn a tool's httpBinding + a tools/call's arguments into (method, url, body)."""
    binding = tool["httpBinding"]
    path = binding["pathTemplate"]
    query = {}
    body = None
    for arg in binding["args"]:
        name = arg["name"]
        value = arguments.get(name)
        if value is None:
            continue
        if arg["placement"] == "path":
            path = path.replace(f"{{{name}}}", urllib.parse.quote(str(value), safe=""))
        elif arg["placement"] == "query":
            query[name] = str(value)
        elif arg["placement"] == "body":
            body = str(value).encode("utf-8")
    url = f"{BASE_URL}{path}"
    if query:
        url += "?" + urllib.parse.urlencode(query)
    return binding["method"], url, body


def call_tool(arguments):
    name = arguments.get("name")
    params = arguments.get("arguments", {})
    if not isinstance(params, dict):
        return tool_error("arguments must be an object")

    tool = find_tool(name)
    if tool is None:
        return tool_error(f"unknown tool: {name}")

    for key in ("source", "manifest"):
        value = params.get(key)
        if isinstance(value, str) and len(value.encode("utf-8")) > MAX_INPUT_BYTES:
            return tool_error("input exceeds the 1 MiB MCP bridge limit")

    method, url, body = apply_binding(tool, params)
    output = params.get("output", "svg")

    try:
        content_type, response_body = http_call(method, url, body)
    except urllib.error.HTTPError as exc:
        return tool_error(f"kr0ki rejected the request ({exc.code}): {exc.read().decode('utf-8', 'replace')}")
    except urllib.error.URLError as exc:
        return tool_error(f"kr0ki request failed: {exc}")

    if output == "png":
        return {
            "content": [
                {
                    "type": "image",
                    "data": base64.b64encode(response_body).decode("ascii"),
                    "mimeType": content_type or "image/png",
                }
            ]
        }
    return {"content": [{"type": "text", "text": response_body.decode("utf-8", "replace")}]}


def handle(message):
    method = message.get("method")
    request_id = message.get("id")
    if method == "initialize":
        return response(
            request_id,
            {
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "kr0ki-mcp", "version": "0.2.0"},
            },
        )
    if method == "tools/list":
        try:
            return response(request_id, {"tools": mcp_tools_list()})
        except (urllib.error.URLError, urllib.error.HTTPError) as exc:
            return error(request_id, -32000, f"failed to fetch tool manifest: {exc}")
    if method == "tools/call":
        return response(request_id, call_tool(message.get("params", {})))
    if method == "ping":
        return response(request_id, {})
    if request_id is None:
        return None
    return error(request_id, -32601, f"method not found: {method}")


def main():
    for line in sys.stdin:
        try:
            result = handle(json.loads(line))
            if result is not None:
                print(json.dumps(result), flush=True)
        except json.JSONDecodeError:
            print(json.dumps(error(None, -32700, "parse error")), flush=True)
        except Exception as exc:  # keep the stdio JSON-RPC transport alive for callers
            print(json.dumps(error(None, -32603, f"internal error: {exc}")), flush=True)


if __name__ == "__main__":
    main()
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd containers/kr0ki-mcp && python3 test_bridge.py`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add containers/kr0ki-mcp/bridge.py containers/kr0ki-mcp/test_bridge.py
git commit -m "feat: rewrite bridge.py as a generic manifest-driven MCP dispatcher"
```

---

### Task 6: Self-render live validation

**Files:**
- Create: `crates/kr0ki-server/tests/kubediagram_live.rs`

**Interfaces:**
- Consumes: `AppState.kubediagram_worker_url` (Task 3), `POST /render/kubediagram` (Task 3), `containers/kr0ki-mcp/http_worker.py`'s `/render` endpoint (Task 4).

- [ ] **Step 1: Write the test**

Create `crates/kr0ki-server/tests/kubediagram_live.rs`:

```rust
//! Live happy-path test for `POST /render/kubediagram` — mcp-http-parity
//! design, 2026-09-16 (docs/superpowers/specs/2026-09-16-mcp-http-parity-design.md).
//!
//! Env-gated on KR0KI_TEST_KUBEDIAGRAM_WORKER, same pattern as
//! `b00t_graph_live.rs`. Renders kr0ki's own deployment manifest
//! (deploy/kr0ki-local.pod.yaml) through the route — kr0ki drawing a
//! picture of its own deployment. To run against the deployed k0s pod:
//!   kubectl --context Default -n kr0ki port-forward pod/kr0ki-local 8788:8788
//!   KR0KI_TEST_KUBEDIAGRAM_WORKER=http://127.0.0.1:8788 \
//!     cargo test -p kr0ki-server --test kubediagram_live -- --ignored --nocapture

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use kr0ki_core::{cache::FsCache, render::HttpKrokiBackend, RenderService};
use tower::ServiceExt;

#[path = "../src/app.rs"]
mod app;
#[path = "../src/docs.rs"]
mod docs;
use app::{router, AppState};

const KR0KI_OWN_DEPLOYMENT_MANIFEST: &str = include_str!("../../../deploy/kr0ki-local.pod.yaml");

#[tokio::test]
#[ignore = "needs KR0KI_TEST_KUBEDIAGRAM_WORKER pointing at a live kubediagram worker"]
async fn kr0ki_renders_its_own_deployment_manifest() {
    let Ok(worker_url) = std::env::var("KR0KI_TEST_KUBEDIAGRAM_WORKER") else {
        eprintln!("KR0KI_TEST_KUBEDIAGRAM_WORKER unset — skipping");
        return;
    };

    let dir = std::env::temp_dir().join(format!("kr0ki-kubediagram-live-{}", std::process::id()));
    let service = RenderService::new(
        HttpKrokiBackend::new("http://127.0.0.1:1"), // this route never uses the Kroki backend
        FsCache::new(&dir),
    );
    let state = AppState {
        service: Arc::new(service),
        playbook_dir: std::env::temp_dir().join("kr0ki-no-playbook-assets"),
        b00t_graph_artifacts_path: None,
        capabilities_path: None,
        kubediagram_worker_url: Some(worker_url),
    };
    let app = router(state, None);

    let resp = app
        .oneshot(
            Request::post("/render/kubediagram")
                .body(Body::from(KR0KI_OWN_DEPLOYMENT_MANIFEST))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert_eq!(status, StatusCode::OK, "response body: {text}");
    assert!(text.contains("<svg"), "expected SVG output, got: {text}");

    let _ = tokio::fs::remove_dir_all(&dir).await;
}
```

- [ ] **Step 2: Verify it compiles and is skipped by default**

Run: `cargo test -p kr0ki-server --test kubediagram_live`
Expected: `1 ignored` (compiles fine; skipped because `#[ignore]` and no env var set).

- [ ] **Step 3: Commit**

```bash
git add crates/kr0ki-server/tests/kubediagram_live.rs
git commit -m "test: add live self-render validation for POST /render/kubediagram"
```

---

### Task 7: Build, deploy to k0s, and validate live

This task has no code changes — it verifies Tasks 1–6 work together in the
real deployed pod, per the same process already used for kr0ki#11–#20 this
session (`just pod-build`, `just k0s-load`, `kubectl apply`).

- [ ] **Step 1: Full workspace verification**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd containers/kr0ki-mcp && python3 test_http_worker.py && python3 test_bridge.py && cd -
```

Expected: all clean, all green.

- [ ] **Step 2: Build and deploy**

```bash
just pod-build
just k0s-load localhost/kr0ki-server:dev
just k0s-load localhost/kr0ki-mcp:dev
just k0s-load localhost/kr0ki-kroki-compat:dev
kubectl --context Default apply -f deploy/namespace.yaml
kubectl --context Default -n kr0ki delete pod --ignore-not-found kr0ki-local
kubectl --context Default apply -f deploy/kr0ki-local.pod.yaml
kubectl --context Default -n kr0ki get pod kr0ki-local
```

Expected: `kr0ki-local` reaches `3/3 Running` with zero restarts.

- [ ] **Step 3: Validate `GET /mcp/tools`**

```bash
curl -fsS http://127.0.0.1:8787/mcp/tools | python3 -m json.tool
```

Expected: a JSON array of 3 entries (`render_diagram`, `list_formats`,
`render_kubernetes_manifest`), each with `httpBinding`.

- [ ] **Step 4: Validate `POST /render/kubediagram` — the self-render check**

```bash
curl -fsS -X POST http://127.0.0.1:8787/render/kubediagram \
  --data-binary @deploy/kr0ki-local.pod.yaml \
  -D - -o /tmp/kr0ki-self-render.svg
head -c 200 /tmp/kr0ki-self-render.svg
```

Expected: `200 OK`, `content-type: image/svg+xml`, and the file starts with
`<?xml`/`<svg` — kr0ki successfully draws a picture of its own deployment.

- [ ] **Step 5: Validate the MCP path end to end**

Using the `mcp__kr0ki-mcp__render_kubernetes_manifest` tool (this session's
own MCP connection, already wired to this pod via `.mcp.json`), call it with
the same manifest and confirm it also returns a real SVG — proving the MCP
path and the new HTTP path both work through the same underlying route
(`GET /mcp/tools` → generic dispatch → `POST /render/kubediagram`).

- [ ] **Step 6: Run the live-gated Rust test against the deployed pod**

```bash
kubectl --context Default -n kr0ki port-forward pod/kr0ki-local 8788:8788 &
PORT_FORWARD_PID=$!
sleep 2
KR0KI_TEST_KUBEDIAGRAM_WORKER=http://127.0.0.1:8788 \
  cargo test -p kr0ki-server --test kubediagram_live -- --ignored --nocapture
kill $PORT_FORWARD_PID
```

Expected: `1 passed`.

- [ ] **Step 7: Handoff to critical review**

Once all of the above is green, invoke the `code-review` skill (or
equivalent) against the full diff introduced by this plan (Tasks 1–6) for an
independent critical pass before considering this done.
