# kr0ki — Agent Orientation

**What this is:** the cut-node in the b00tyverse graph between Kroki and b00t/systhread.
It renders diagrams from validated models. It does not model, and it does not invent
grammars. Everything upstream of kr0ki is `ufo-types`/`systhread`; everything downstream
is a CDN-cached artifact.

**Ecosystem home:** `PromptExecution/kr0ki` — upstream of `elasticdotventures/_b00t_#1177`.
**Parent PRD:** `docs/PRD-KR0KI-001-foundational.md`. **Ingestion plan:**
`docs/PLAN-KR0KI-002.md`. **Typed layer design note:** `docs/DESIGN-NOTE-typed-model-layer.md`.

---

## 0. Quick start (copy-paste)

```bash
just test          # unit + in-process HTTP (all pass)
just check         # fmt + clippy gate
just run           # binds 0.0.0.0:8787, logs hostname + docs URL
# visit http://<hostname>:8787/docs  ← live docgen from kr0ki's own source
```

Live render against Kroki (needs network):
```bash
just test-live        # SVG template render
curl -X POST http://localhost:8787/render/graphviz?output=png \
  --data-binary "digraph { a -> b }" --output /tmp/test.png
```

---

## 1. Architecture — the five-box pipeline

kr0ki's long-term scope is a **five-box ingestion chain** (PLAN §2). Only Box 5 (the
render loop) is implemented. Boxes 1–4 are upstream dependencies (`ufo-types`,
`sysml-v2-parser`, model servers) that kr0ki consumes but does not host.

```
Box 1 ──► Box 2 ──► Box 3 ──► Box 4 ──► Box 5
source   UFO       pattern   SysML v2  renderer
front-end semantic  recog-    constructs adapters
         graph      nizers    (viewpts)
```

| Box | What | Status | Blocked on |
|---|---|---|---|
| 1 | Source front-ends (SysML-v2 API client, Rust AST, k8s manifests) | **Client partial** — `kr0ki-sysmlv2-client` reads OMG-API servers; Rust/k8s front-ends named but not scoped | `ufo-types` graph builder (Box 2) |
| 2 | Canonical UFO-typed semantic graph | **Design only** — `ufo-types` has `iso_ir`, `ontology`, `view` layers but no container type | `ufo-types` v0.15+ graph container |
| 3 | Pattern recognizers (Kubernetes first) | **Design only** — `PATTERNS-kubernetes.md` has rule mapping; no code | Box 2 + `KubeDiagrams` oracle |
| 4 | SysML v2 model constructs / view definitions | **Partial** — `ElementKind` (24), `Relation` (12), `SysmlViewKind` done; `ViewDefinition` as data not yet | Box 2 + Box 3 |
| 5 | **Renderer adapters + HTTP service** | **✅ P0 implemented** — `kr0ki-core` + `kr0ki-server`, cache, auth, PNG, docgen | CDN tier (D5), artifact resolver (FR6) |

**Current implemented surface:**
- `kr0ki-core`: `RenderService` (cache + backend), `FsCache`, `HttpKrokiBackend`, `DiagramFormat` (8 slugs), docgen (syn harvester + formatters)
- `kr0ki-server`: axum routes `/health`, `/formats`, `/render/{format}`, `/cache/{key}`, `/docs*` (HTML/JSON/tomllm/rustdoc)
- `kr0ki-sysmlv2-client`: OMG-API REST client, `ModelSnapshot` with `content_hash`

---

## 2. Crate map

```
crates/
├── kr0ki-core/         # P0 render loop + docgen (NO model code)
│   ├── src/cache.rs     # SHA-256 content-addressed FsCache; OutputKind {Svg,Png}
│   ├── src/format.rs    # DiagramFormat enum (8 Kroki slugs, no Mermaid)
│   ├── src/render.rs    # RenderBackend trait; HttpKrokiBackend POST /{slug}/{output}
│   ├── src/docgen/      # syn-based source harvester + formatters (json, tomllm, rustdoc, html)
│   └── tests/           # template_render.rs (live), live_png.rs (live), conformance.rs (ignored)
├── kr0ki-server/        # axum HTTP service
│   ├── src/main.rs      # entrypoint; KR0KI_BIND, KR0KI_BACKEND_URL, KR0KI_AUTH_TOKEN env
│   ├── src/app.rs       # router: health, formats, render, cache + auth middleware
│   ├── src/docs.rs      # /docs, /docs/api.json, /docs/api.tomllm, /docs/api.rustdoc
│   └── tests/http.rs    # in-process HTTP tests (no backend needed)
└── kr0ki-sysmlv2-client/# OMG Systems Modeling API client
    ├── src/lib.rs       # Client, Project, Commit, Branch, Element, Relationship, ModelSnapshot
    └── tests/live.rs    # env-gated live test vs real OMG-API server
```

---

## 3. Environment / config

| Env var | Default | Purpose |
|---|---|---|
| `KR0KI_BIND` | `0.0.0.0:8787` | TCP listen address |
| `KR0KI_BACKEND_URL` | `https://kroki.io` | Kroki backend (SECURE-mode for local) |
| `KR0KI_CACHE_DIR` | `./.kr0ki-cache` | Filesystem cache root |
| `KR0KI_AUTH_TOKEN` | unset | If set, require `Authorization: Bearer <token>` on all routes except `/health` |
| `KR0KI_TEST_BACKEND` | unset | Live render test backend (e.g. `https://kroki.io`) |
| `KR0KI_SYSMLV2_BASE_URL` | unset | Live SysML-v2 client test target |

---

## 4. What NOT to build here (scope guardrails)

These are **upstream or sibling concerns** — build them in their proper crates:

- **Typed model layer** → `ufo-types` (D3 resolved: kr0ki consumes, does not host)
- **SysML v2 parser** → `sysml-v2-parser` crate (consumed as dev-dependency for conformance)
- **Kubernetes recognizer** → starts in `PATTERNS-kubernetes.md`, probably a new crate
- **Isometric renderer** → `systhread-core` (call it, don't port it)
- **Comic engine / joke renderer** → `kroki-b00t` (downstream leaf, not core)
- **CDN / edge caching** → infrastructure / Cloudflare R2 (D5)
- **Graph reasoning / SHACL** → `ledgrrr`

---

## 5. Docgen / mdb00k capability (new)

`kr0ki-core/src/docgen/` is a clean-room implementation of the b00t `docgen.rs` pattern.
It uses `syn` (not `codebase-memory-mcp`) to harvest Rust source and formats output
matching b00t conventions so downstream tooling consumes it without translation.

**Endpoints:**
- `GET /docs` — HTML with dark/light mode, symbol index, diagram example
- `GET /docs/api.json` — JSON array of `Symbol` structs
- `GET /docs/api.tomllm` — `.tomllm` with `b00t:map v1` trailer
- `GET /docs/api.rustdoc` — rustdoc-style `///` blocks

The HTML includes a **live diagram example** — the b00t-stack-orchestration D2 template
with copy-paste curl command. When kr0ki is running, `/docs` is self-documenting.

---

## 6. Test discipline

- **Unit tests** in each crate (`cargo test --workspace`) — must pass
- **Live tests** are `#[ignore]` — require env vars (`KR0KI_TEST_BACKEND`, `KR0KI_SYSMLV2_BASE_URL`)
- **HTTP tests** in `kr0ki-server/tests/http.rs` — run against stub backend, no network
- **Template render test** — `tests/template_render.rs` renders `templates/b00t-stack-orchestration.d2` live
- **PNG live test** — `tests/live_png.rs` verifies byte-identical cache hits for PNG
- **Conformance** — `tests/conformance.rs` gates SysML-v2 handling (ignored, Phase 1)

Run the gate:
```bash
just check         # fmt + clippy
just test          # workspace unit tests
just test-live     # live SVG render
just test-live-png # live PNG render
```

---

## 7. Sharp corners & tribal knowledge

🤓 **D2 PNG does not exist.** Kroki returns 400 for `POST /d2/png`. GraphViz/PlantUML
support PNG; D2 does not. The `OutputKind::Png` plumbing is correct — it's a Kroki/D2
upstream limitation.

🤓 **Mermaid is companion-only.** `DiagramFormat::ALL` excludes Mermaid because Kroki's
Mermaid renderer requires a companion (puppeteer/Chrome). kr0ki only advertises the 8
standalone formats.

🤓 **Cache key must include output kind.** Same D2 source → SVG and PNG must have
different cache keys (already implemented — `cache_key()` hashes `(format, output, source)`).

🤓 **Docgen workspace root detection walks up** looking for `Cargo.toml` containing
`[workspace]`. If kr0ki is vendored into another workspace, this may need adjustment.

🤓 **b00t MCP bridge was down during initial build** (`bad handshake`). If `b00t whoami`
fails with handshake errors, it's an external MCP transport issue — not a kr0ki bug.
Use direct HTTP or `b00t-cli` instead.

🚩 **Auth is FR7 minimal** — single shared bearer token via env var. No OAuth, no JWT,
no per-key rate limiting. Suitable for localhost/trusted-proxy only until D4/D5 land.

---

## 8. b00t interface (NFR5)

kr0ki exposes a service surface; agent interaction should go through b00t conventions:
- `b00t task add "kr0ki: <desc>"` — track work
- `b00t lfmf kr0ki "<lesson>"` — memoize tribal knowledge
- `just <recipe>` — preferred over raw commands
- Datum registration: `templates/datum.template.toml` is the copy-paste template for
  submitting to `elasticdotventures/_b00t_`

Long-term: a `kr0ki.mcp.toml` datum or vendored `kroki-mcp` server for MCP-native
render calls. P0 uses direct HTTP.

---

## 9. Open decisions (not kr0ki's to make)

| Decision | What | Status |
|---|---|---|
| D1 | `sysml-derive` posture (extend/wrap/re-export) for `UfoStereotype` types | Blocks authoring path; read path unblocked |
| D3 | Typed layer lives in `ufo-types` — kr0ki consumes, does not host | ✅ Resolved 2026-09-05 |
| D4 | DNS provisioning (`kr0ki.b00t.promptexecution.com`) | Infra-owned |
| D5 | CDN tier (Cloudflare R2 + Workers) | Infra-owned |
| D6 | Vocabulary (OMG terms verbatim, "projection" banned) | ✅ Resolved 2026-09-05, `VOCABULARY.md` |

---

<!-- b00t:map v1
summary: kr0ki agent orientation — ecosystem context, crate map, scope guardrails, docgen
tags: kr0ki, b00t, agent, orientation, docgen, render, sysml, ufo-types
tier: frontier
cmds: just test, just check, just run, just test-live, just test-live-png
complexity: 7
-->
