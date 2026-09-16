# DESIGN — MCP/HTTP capability parity, with a pattern for expansion

**Status:** approved design, not yet implemented. **Owner:** PromptExecution
(@elasticdotventures). **Created:** 2026-09-16.

---

## 0. Why

Today `kr0ki-mcp`'s stdio bridge (`containers/kr0ki-mcp/bridge.py`) hand-maintains a
`TOOLS` list that duplicates, and can silently drift from, what `kr0ki-server` actually
supports (`crates/kr0ki-server/src/app.rs`'s routes, `DiagramFormat::ALL`). Separately,
`render_kubernetes_manifest` (KubeDiagrams — see "Where is KubeDiagrams in the UX?",
2026-09-16 conversation) is reachable **only** via MCP (`kubectl exec` into the pod) —
there is no HTTP route, unlike every Kroki-family format and the b00t-graph arm
(kr0ki#13).

This work makes kr0ki-server the single source of truth for every capability's shape
(name, description, JSON schema, HTTP binding), and turns `kr0ki-mcp` into a thin,
mostly-generic dispatcher over that manifest — so a new capability added on the Rust
side needs zero Python changes to appear over MCP.

## 1. Scope

**In scope:**
- A `GET /mcp/tools` manifest endpoint on `kr0ki-server`, backed by a new closed
  `McpTool` enum in `kr0ki-core` (mirrors `DiagramFormat`'s shape: one `ALL` const,
  one variant per tool).
- `POST /render/kubediagram` — an HTTP route for KubeDiagrams rendering, proxying to a
  new internal HTTP listener in the `kr0ki-mcp` container (replaces that container's
  `sleep infinity` idle command).
- Rewriting the stdio `bridge.py` to fetch the manifest once and dispatch every
  `tools/call` generically from it, instead of hand-coded per-tool branches.
- Deleting `containers/kubediagram-mcp/bridge.py` (its stdio-JSON-RPC-over-subprocess
  hop becomes redundant once the new internal HTTP listener shells to `kube-diagrams`
  directly).
- A live self-render check: `deploy/kr0ki-local.pod.yaml` rendered through
  `/render/kubediagram`, as both a deployment-validation step and a permanent playbook
  example (kr0ki draws a picture of its own deployment — same spirit as the existing
  `d2-rust-flow` fixture, which diagrams kr0ki's own Box-5 render flow).

**Explicit non-goals (backlog, not forgotten):**
- **Content-addressed caching for `/render/kubediagram`.** Punted to *after* the MBSE
  work below — revisit once the shape of "what gets cached" is informed by that
  design, rather than guessing now. `/render/kubediagram` is an uncached passthrough
  for this pass.
- **A generalized capability-registration framework spanning `DiagramFormat` /
  `McpTool` / future SysML view kinds.** Per the 2026-09-16 design conversation:
  YAGNI until a second concrete family actually needs the same shape as `McpTool`.
  This design adds exactly one new closed enum, for exactly one family.
- **SysML v2 concrete-syntax output for the b00t-graph route** (`b00t_graph.rs`
  currently emits D2 only via `holon_viz::type_graph`; `holon_viz::emitter::SysmlV2Emitter`
  already exists and could add a second output mode later).
- **The broader MBSE rendering subsystem** — SHACL-driven validation, tree
  search/traversal over a model graph, subsystem detail roll-up, all 9 SysML v1-lineage
  diagram kinds (Activity/Sequence/State Machine/Use Case/Requirement/BDD/IBD/Package/
  Parametric) plus custom types, and kr0ki's broader framing as "a frontend
  visualization layer for OpenMBEE Flexo's procedurally-reconciled artifacts." Each of
  these is its own sub-project and needs its own brainstorm — in particular, "9 fixed
  diagram kinds" as asked for in the parent conversation directly contradicts the
  already-resolved decision in `docs/DESIGN-NOTE-typed-model-layer.md` §1 that SysML v2
  has no fixed diagram-kind enum (that taxonomy is a SysML v1 artifact); that tension
  needs to be resolved explicitly when this sub-project is brainstormed, not silently
  overridden here.
- **The AI-agent-authoring-prompts/skill revision** mentioned in the same conversation
  — unrelated to this repo's Rust/Python code; out of scope for this spec entirely.

## 2. Architecture

```
Agent (MCP)                    Browser/curl (HTTP)
    |                                  |
    v                                  v
bridge.py (stdio,              kr0ki-server (axum)
kubectl exec)                   +- GET  /mcp/tools          <- manifest, new
    |  tools/list                +- POST /render/:format     <- existing, unchanged
    |  -> GET /mcp/tools -------> (McpTool::ALL, from kr0ki-core)
    |  tools/call                +- POST /render/kubediagram <- new
    |  -> generic HTTP dispatch->        |
    v                                    v proxies to
(no per-tool Python code            kr0ki-mcp:8788/render (new internal HTTP listener,
 beyond the dispatcher)              same container, replaces `sleep infinity`)
                                          |
                                          v subprocess
                                      kube-diagrams CLI (already installed there)
```

## 3. Components

### 3.1 `kr0ki-core`: `McpTool` (new module, e.g. `src/mcp_tool.rs`)

A closed enum, one variant per callable capability, structurally analogous to
`DiagramFormat` (`diagram_formats!` macro → named consts + `ALL`):

```rust
pub enum McpTool {
    RenderDiagram,
    ListFormats,
    RenderKubeDiagram,
}
```

Each variant carries (via methods, not a macro necessarily — evaluate at
implementation time whether a `mcp_tools!`-style macro pays for itself at 3 variants,
or whether plain match arms are clearer until there are more):

- `name() -> &'static str` — the MCP tool name (`"render_diagram"`, …).
- `description() -> &'static str`.
- `input_schema() -> serde_json::Value` — MCP `inputSchema` (JSON Schema).
- An HTTP binding: method, path template, and how the JSON `arguments` object maps
  onto path segments / query params / raw body. Exact representation TBD at
  implementation time (e.g. an enum `HttpBinding { Get { path: &'static str },
  PostBody { path_template: &'static str, body_field: &'static str, query_fields:
  &'static [&'static str] } }` — expressive enough for today's three tools without
  over-generalizing).

`McpTool::ALL: &'static [McpTool]`.

### 3.2 `kr0ki-server`

- `GET /mcp/tools` → `Json(McpTool::ALL.iter().map(|t| t.to_manifest_json()).collect())`
  — **both** halves per tool: the MCP schema (name/description/`inputSchema`, used
  verbatim as `bridge.py`'s `tools/list` response) and the HTTP binding (method, path
  template, path/query/body field mapping — used by `bridge.py`'s generic dispatcher
  to actually place a `tools/call`'s arguments on the wire). There's no reason to
  split these: the binding shape isn't sensitive, and the stdio bridge cannot dispatch
  generically without it — that's the entire point of making it fetch a manifest
  instead of hand-coding each call.
- `POST /render/kubediagram?output=svg|dot_json` — body is the manifest YAML. Reads
  `state.kubediagram_worker_url` (new `AppState` field, env-configured, defaulting to
  `http://127.0.0.1:8788` — the sidecar's new listener), proxies the request, returns
  the worker's response bytes with the same header shape `/render/:format` uses
  (`Content-Type`, but no `X-Kr0ki-Cache`/`X-Kr0ki-Key` this pass — see §1 non-goals).
  503 (not a panic) if the worker is unreachable, mirroring `/b00t-graph`'s and
  `/capabilities`'s established "unconfigured/unavailable is a clean 503" convention.

### 3.3 `kr0ki-mcp` container

- `deploy/kr0ki-local.pod.yaml`: the `kr0ki-mcp` container's `command` changes from
  `["sleep", "infinity"]` to running a new small script (e.g.
  `containers/kr0ki-mcp/http_worker.py`) — a stdlib `http.server`-based process,
  `POST /render` (internal-only, not the public path — `kr0ki-server` names it
  `/render/kubediagram` externally), doing exactly what
  `containers/kubediagram-mcp/bridge.py`'s `render()` does today (temp dir, `kube-diagrams
  - -f {output} -o {artifact}`, read the artifact back). Listens on `127.0.0.1:8788`
  (or a `KR0KI_MCP_WORKER_PORT` env var, matching this repo's existing
  configurability convention).
- `containers/kubediagram-mcp/bridge.py` is **deleted** — its role (stdio JSON-RPC
  wrapping a `kube-diagrams` subprocess call) is now redundant; `http_worker.py`
  shells to `kube-diagrams` directly. `containers/kr0ki-mcp/Containerfile`'s `COPY
  containers/kubediagram-mcp/bridge.py …` line is removed accordingly; the
  `vendor/kubediagrams/bin` COPY (the actual `kube-diagrams` command + its
  `kube-diagrams.yaml` catalogue) stays, since `http_worker.py` still needs the real
  CLI on `PATH`.
- `containers/kr0ki-mcp/bridge.py` (the stdio MCP bridge, still invoked per-session via
  `kubectl exec` — unchanged mechanism) is rewritten:
  - On `initialize`/first use, `GET {KR0KI_URL}/mcp/tools` once, cache the manifest for
    the process's lifetime, and answer `tools/list` from it.
  - On `tools/call`, look up the named tool in the cached manifest, apply its HTTP
    binding to the given `arguments` (path/query/body), issue the HTTP request to
    `KR0KI_URL`, and wrap the response as MCP `content` blocks — one generic function,
    not one branch per tool name. `render_diagram`'s existing PNG-as-base64-image
    special-case (vs. plain text for SVG) is the one genuinely response-shape-specific
    bit that likely needs to stay as a small `output`-aware branch in the wrapper, not
    something the manifest itself encodes (YAGNI — one special case doesn't justify a
    general response-shape schema yet).

## 4. Data flow — `render_kubernetes_manifest` end to end

1. Agent calls MCP tool `render_kubernetes_manifest` with `{manifest, output}`.
2. `bridge.py` (already holding the fetched manifest) finds `McpTool::RenderKubeDiagram`'s
   binding: `POST /render/kubediagram`, `manifest` → body, `output` → query param.
3. `bridge.py` → `kr0ki-server`.
4. `kr0ki-server`'s handler proxies the body/query straight through to
   `http://127.0.0.1:8788/render` in the same container as `bridge.py`'s own pod, i.e.
   the sidecar `kr0ki-mcp` container's new persistent listener.
5. `http_worker.py` runs `kube-diagrams` and returns the artifact bytes.
6. Response flows back: worker → `kr0ki-server` → `bridge.py` → MCP `content` block.

The same manifest-driven path serves a plain `curl -X POST
http://<host>:8787/render/kubediagram` directly, with no MCP hop at all — that is the
"HTTP parity" this design delivers.

## 5. Error handling

- Sidecar unreachable → `kr0ki-server` returns `503 kubediagram_worker_unavailable`
  (same convention as `/b00t-graph`/`/capabilities`).
- `kube-diagrams` itself fails (bad manifest) → `http_worker.py` returns its own
  non-2xx with the CLI's stderr; `kr0ki-server` passes that through as `422` (matching
  `/render/:format`'s `bad_source` convention for "the input was rejected", not a
  5xx).
- Manifest exceeds the existing 1 MiB MCP bridge limit → same 1 MiB ceiling enforced
  once, in `kr0ki-server`'s handler (the authoritative HTTP entry point), not
  duplicated in `bridge.py` and `http_worker.py` separately.

## 6. Testing

- `crates/kr0ki-core`: unit tests on `McpTool` — every variant has a name/description/
  schema, `McpTool::ALL` has no duplicate names (mirrors `format.rs`'s own duplicate-slug
  guard test).
- `crates/kr0ki-server` (wiremock-backed, matching `b00t_graph`'s test style): `GET
  /mcp/tools` returns both the MCP-schema and HTTP-binding halves for all three tools;
  `POST /render/kubediagram` proxies correctly; 503 when the worker is unreachable;
  422 on a worker-reported bad manifest — all before any real `kube-diagrams` process
  runs.
- **Live self-render validation** (the "make sure kr0ki can render itself" check):
  after deployment, `POST /render/kubediagram` with `deploy/kr0ki-local.pod.yaml`'s own
  content and confirm a real SVG comes back — kr0ki drawing a picture of its own
  deployment. Add as a permanent playbook example (new entry in
  `fixtures/playbook-examples.json`, format `"kubediagram"`, alongside the existing
  `d2-rust-flow` self-referential example) and as a live-gated integration test
  (`#[ignore]`, `KR0KI_TEST_BACKEND`-style, matching `b00t_graph_live.rs`'s pattern) —
  not just a one-off manual curl.
- A Python-side test for `bridge.py`'s generic dispatcher against a stub manifest
  (no real `kr0ki-server`/`kube-diagrams` needed) — exact harness (unittest vs. a
  simple assert script, matching this repo's existing bar for the Python bridges,
  which currently have none) to be decided at implementation time.

## 7. Migration / compatibility

- `render_diagram` and `list_formats`' behavior over MCP does not change from a
  caller's perspective — only their internal wiring (manifest-driven dispatch instead
  of hardcoded branches).
- `render_kubernetes_manifest` over MCP does not change its input/output contract
  either; only its internal path (through `kr0ki-server` now, not a direct subprocess
  hop) changes.
- No consumer currently depends on `containers/kubediagram-mcp/` existing as a
  separate file — safe to delete in the same change that adds `http_worker.py`.
