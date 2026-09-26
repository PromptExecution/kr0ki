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
just build-assistant-ui-vue  # build vendored @assistant-ui/vue from the submodule
just test          # unit + in-process HTTP (all pass, builds assistant-ui first)
just check         # fmt + clippy gate
just run           # binds 0.0.0.0:8787, logs hostname + docs URL
# visit http://<hostname>:8787/docs  ← live docgen from kr0ki's own source
```

Live render against the private, air-gapped kroki-compatible backend:
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
| 2 | Canonical UFO-typed semantic graph | **Partial** — `ufo-types` ships the `SysGraph` container (`sysgraph` module); the SysML-v2 arm builds it (`ufo_graph.rs`). The dbt arm's builder (`PLAN-KR0KI-006` piece 1) is spec-only, not yet in `ufo-types` | dbt `SysGraph` builder (upstream, `ufo-types`) |
| 3 | Pattern recognizers (Kubernetes first) | **✅ Kubernetes arm shipped** — `k8s_recognizer.rs` (955 lines), wired into `kr0ki-server`'s routes, oracle-tested against real KubeDiagrams. Rust arm's recognizer (`rust_recognizer.rs`, `PLAN-KR0KI-003`) is implemented but not yet wired to any route; its call-graph/trait-dispatch coverage is explicitly deferred | Rust arm: route/MCP wiring, call-graph coverage |
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
| `KR0KI_BACKEND_URL` | `http://127.0.0.1:8010` in dev; `http://127.0.0.1:8000` in the pod | Private kroki-compatible backend; never point production at the public Kroki service |
| `KR0KI_CACHE_DIR` | `./.kr0ki-cache` | Filesystem cache root |
| `KR0KI_AUTH_TOKEN` | unset | If set, require `Authorization: Bearer <token>` on all routes except `/health` |
| `KR0KI_TEST_BACKEND` | unset | Live render test backend; use the local/private kroki-compatible service |
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

🤓 **@assistant-ui/vue is a preview (not on npm).** The package is vendored as a
git submodule at `vendor/assistant-ui/` (branch `b00t-vue-package`). It must be built
before playbook tests can run: `just build-assistant-ui-vue` (or it runs as part of
`just test` / `just build`). The built `dist/` is not committed — it lives in the
submodule's `.gitignore`. If `just test` fails on playbook import, run
`just build-assistant-ui-vue` then `pnpm --dir playbook install`.

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

## 8a. Agent communications and infrastructure squawks

Agent-to-agent work is migrating to A2A semantics over NATS JetStream. NATS is the
internal durable transport; A2A supplies the agent identity, capability, task,
context, acknowledgement, and lifecycle envelope. This is an internal NATS
adaptation, not a claim that raw NATS subjects are HTTP A2A wire-compatible. The
HTTP Agent Card/API bridge remains a later edge concern.

Every agent MUST register before doing shared or infrastructure work and MUST renew
its lease/heartbeat. Registration includes a stable `agent_id`, host, process or
session identifier, protocol version, capabilities, and the subjects it consumes.
An agent that cannot register MUST remain read-only and report the condition.

The durable stream is `B00T_A2A` with the following subject contract:

| Subject | Purpose |
|---|---|
| `a2a.v1.registry.register` | Agent Card and capability registration |
| `a2a.v1.registry.heartbeat` | Lease renewal; stale agents are not routable |
| `a2a.v1.registry.leave` | Graceful deregistration |
| `a2a.v1.msg.<agent_id>` | Point-to-point A2A messages/tasks |
| `a2a.v1.task.<task_id>.events` | Durable task lifecycle events |
| `a2a.v1.ack.<correlation_id>` | Transport and agent-level acknowledgements |
| `a2a.v1.squawk.infra` | Common infrastructure activity/audit channel |
| `a2a.v1.audit.>` | Immutable raw traffic for later emergence analysis |

Infrastructure changes MUST squawk `PREPARE`, `START`, `COMPLETE`, `FAIL`, or
`CANCEL` to `a2a.v1.squawk.infra`. Each event carries `event_id`,
`correlation_id`, `agent_id`, host, action, scope, reason, risk/approval state,
UTC timestamp, repository and git SHA, the `just`/b00t recipe or command, and a
redacted result. Never put credentials, tokens, or private keys in a message.
Other agents and the historian should consume this channel with durable consumers;
it is the shared operational “squawk” log, not an ephemeral chat room.

Use JetStream publish acknowledgements plus explicit consumer acknowledgements.
Set a durable consumer, bounded redelivery (`MaxDeliver`), an acknowledgement
deadline (`AckWait`), and a deduplication id (`Nats-Msg-Id` = `event_id`). A
transport ACK only means NATS accepted the event; the sender must separately wait
for an agent/task ACK and report timeout or failure. Handlers must be idempotent by
`event_id`/`correlation_id`.

During migration, `b00t agent` Redis messaging is a compatibility bridge only. Do
not add new Redis-only protocols. Configure NATS with `NATS_URL` (use the private
tailnet/service address for remote hosts such as fung1), and use `just`/b00t
wrappers so registration, squawks, and historian capture are automatic. If a
message is accepted but no agent ACK arrives, verify that the peer is registered,
has a live heartbeat, and owns a durable consumer; do not infer that delivery means
the remote Codex is online.

Migration stages and acceptance criteria are recorded in
`docs/DESIGN-NOTE-a2a-nats-durable.md`.

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
