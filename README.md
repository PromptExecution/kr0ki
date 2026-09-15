# kr0ki

**The b00tyverse cut-node between [Kroki](https://kroki.io) and b00t/systhread.**

kr0ki is the rendering + CDN-cached-artifact service layer for SysML/KerML diagrams in
the b00t ecosystem. It is deliberately *one node in the graph* — the boundary where an
abstract, validated model (owned upstream by `systhread` / `ufo-types`) becomes a
concrete, cached, referenceable picture (Mermaid / PlantUML / GraphViz / D2 / SVG,
served from `kr0ki.b00t.promptexecution.com`).

It does **not** own the model. It does **not** re-implement diagram grammars. It
wraps an existing multi-format renderer ([`kroki-mcp`](vendor/kroki-mcp), vendored)
and an existing isometric renderer (`systhread-core`'s `layout.rs`/`render.rs`),
and adds the one thing neither has: a caching, cross-referencing service surface.

> Status: **foundational**. The decision-independent render loop (P0 — FR2 + FR5) is
> [built and merged](#p0--the-render-loop-built-2026-09-05). The SysML-model path
> (FR1/FR3/FR4) has started: its **client** — a generic OMG *Systems Modeling API*
> REST client — is in `crates/kr0ki-sysmlv2-client` (D3 resolved 2026-09-05); the typed
> adapter above it, and anything that emits SysML v2 text, stays blocked on D1/D6.
> Read [`docs/PRD-KR0KI-001-foundational.md`](docs/PRD-KR0KI-001-foundational.md) first,
> then [`docs/PLAN-KR0KI-002.md`](docs/PLAN-KR0KI-002.md) for the model path and
> [`docs/DESIGN-NOTE-typed-model-layer.md`](docs/DESIGN-NOTE-typed-model-layer.md) for
> the reviewed (not yet approved) shape of the deferred typed layer.

## Orientation for agents

| Thing | Where | Why it matters here |
|---|---|---|
| b00t SysML v2 spine epic (closed) | [`elasticdotventures/_b00t_#1177`](https://github.com/elasticdotventures/_b00t_/issues/1177) | The upstream model/validation layer kr0ki renders *from*. P0–P3 shipped. |
| `ufo-types` v0.11.0 | [`PromptExecution/ufo-types`](https://github.com/PromptExecution/ufo-types) | `iso_ir::{Node,Edge}`, `stereotype::UfoStereotype`, `sysml::validate_sysml_v2`, `mbse`. kr0ki's input vocabulary. |
| Live P1 prototype | `elasticdotventures/_b00t_` → `b00t-cli/src/dispatch_sysml.rs` | Working `Rust type → iso_ir → SysML v2 / Mermaid / Rhai`. The pattern kr0ki's adapter follows. |
| `systhread-core` | `fungible-farm/nem-poweragent-lab` → `rust/systhread-core` | Owns `iso_ir`/`layout`/`render`/`sysml_gen`. Its isometric `render.rs` becomes a kr0ki input format. |
| systhread v2 SysML/KerML viz scope | [`nem-poweragent-lab#53`](https://github.com/fungible-farm/nem-poweragent-lab/pull/53) (merged) | Defines the typed-model → views contract kr0ki renders. Names **cim-gridy** as first consumer. |
| kroki-b00t (comic engine) | [`PromptExecution/infrastructure#217`](https://github.com/PromptExecution/infrastructure/issues/217) | A **downstream consumer**, disconnected as its own leaf — the comic team renders kr0ki SVGs to make jokes about b00t. Not part of kr0ki's core. |
| Dependency-posture decisions (OPEN) | [`ledgrrr#202`](https://github.com/PromptExecution/ledgrrr/issues/202), [`ledgrrr#203`](https://github.com/PromptExecution/ledgrrr/issues/203) | extend-vs-wrap-vs-re-export `sysml-derive`; whether `holon-viz` becomes a real dep. **kr0ki defers to these — does not pre-empt them.** |
| Generic Kroki datum | `elasticdotventures/_b00t_` → `_b00t_/kroki.mcp.toml` | The existing b00t MCP datum wrapping public/self-hosted Kroki. kr0ki supersedes it as the *service*, keeps it as the *client*. |
| KerML anchor | `elasticdotventures/_b00t_` → `_b00t_/types/b00tyverse.kerm` | The canonical KerML the whole thread serializes/reasons over. |
| Vendored renderer | [`vendor/kroki-mcp`](vendor/kroki-mcp) → [`PromptExecution/kroki-mcp`](https://github.com/PromptExecution/kroki-mcp) | b00tyverse fork of [`utain/kroki-mcp`](https://github.com/utain/kroki-mcp) (MIT, Go), pinned at `08765f64`. |
| SysML-v2 tooling survey | `PromptExecution/ledgrrr` → `docs/sysml-v2-tooling-survey.md` | Decided infra (`holon-viz`, `ufo-types`, wrap-vs-build for LSP/MCP). Read before proposing new tooling. |

## Layout

```
kr0ki/
├── README.md
├── crates/
│   ├── kr0ki-core/               ← P0 render loop (RenderService = cache + RenderBackend)
│   ├── kr0ki-server/             ← P0 axum service
│   └── kr0ki-sysmlv2-client/     ← generic OMG "Systems Modeling API" REST client (SysML-model path)
├── docs/
│   ├── PRD-KR0KI-001-foundational.md    ← the requirements document
│   ├── DESIGN-NOTE-typed-model-layer.md ← reviewed shape of the deferred SysML-v2 typed layer (pre-D1/D6)
│   ├── PLAN-KR0KI-002.md                ← the SysML-model ingestion path (5-box pipeline; client path unblocked 2026-09-05)
│   ├── PATTERNS-kubernetes.md           ← first pattern recognizer (UFO bridge + 25 canonical relations + KubeDiagrams prior art)
│   ├── EVAL-flexo.md                    ← Flexo MMS / flexo-mms-sysmlv2 evaluation (primary API target)
│   ├── EVAL-syson.md                    ← Eclipse SysON evaluation (reference oracle, not a competitor)
│   ├── EVAL-kubediagrams.md             ← KubeDiagrams for k8s/IaC rendering — backend (leaf) vs recognizer prior art vs oracle
│   ├── VOCABULARY.md                    ← D6 resolution — OMG SysML v2 spec terms verbatim; "projection" banned
│   ├── CONFORMANCE.md                   ← SysML-v2-Release conformance harness (phase 1)
│   └── TODO.md                          ← the working list — what is left, by pipeline box
└── vendor/
    └── kroki-mcp/                       ← submodule, PromptExecution/kroki-mcp @ 08765f64
```

### SysML-model path (in progress)

`crates/kr0ki-sysmlv2-client` is the first increment of the SysML-model ingestion path.
It is a **server-agnostic** async REST client for the OMG *Systems Modeling API and
Services* PSM — it works against Flexo `flexo-mms-sysmlv2`, the OMG Java pilot
`Systems-Modeling/SysML-v2-API-Services`, `Open-MBEE/OpenSysML`, and Eclipse SysON's
`/api/rest/`. It is **not Flexo-coupled**. It reads projects / branches / tags / commits
/ elements / relationships / roots and produces a content-hashed `ModelSnapshot` — the
model-side cache key for PRD FR5, since no target server exposes its own content hash.

This is possible now because the operator resolved decision **D3** on 2026-09-05: *kr0ki
consumes an upstream OMG-API model server; it does not host the model.*

`ModelSnapshot` feeds the **SysML-v2 source** arm of a five-box ingestion pipeline
(`source → canonical UFO-typed semantic graph → pattern recognizers → SysML v2
viewpoints → kr0ki renderer adapters`). kr0ki MUST NOT infer architecture from diagram
syntax or raw `iso_ir` strings — the UFO semantic graph (owned by `ufo-types`, a
follow-up PR in flight) is the pivot. FR1/FR4 are therefore blocked on that layer plus a
pattern recognizer (Kubernetes first); the typed adapter and any SysML-v2 text emit stay
blocked on D1/D6. See [`docs/PLAN-KR0KI-002.md`](docs/PLAN-KR0KI-002.md) and
[`docs/PATTERNS-kubernetes.md`](docs/PATTERNS-kubernetes.md).

## P0 — the render loop (built 2026-09-05)

The decision-independent slice of PRD-KR0KI-001 (FR2 + FR5) is implemented and tested:
raw Kroki-family diagram text → rendered SVG, with a content-addressed cache. Nothing
here depends on `ufo-types` / `systhread-core` / `holon-viz` — the SysML-model
ingestion path (FR1/FR3/FR4) stays blocked on §5 decisions D1–D6.

```
crates/
├── kr0ki-core/    RenderService = cache in front of a RenderBackend
│   ├── format.rs  DiagramFormat — the 8 companion-free Kroki formats only (NFR3)
│   ├── cache.rs   cache_key() = SHA256(domain ‖ 0x1f-delimited fields) ; FsCache (atomic writes)
│   └── render.rs  HttpKrokiBackend — POST {base}/{slug}/{output}
└── kr0ki-server/  axum service
    GET  /health                              {"status":"ok",...}
    GET  /formats                             supported slugs
    POST /render/{format}?output=svg|png      body = diagram source → SVG or PNG
                                              (X-Kr0ki-Cache: hit|miss, X-Kr0ki-Key)
    GET  /cache/{key}?output=svg|png          previously rendered artifact by content hash
```

**Verified:** `cargo test --workspace` (17 pass) · `cargo clippy -- -D warnings` clean ·
live render against `https://kroki.io` (miss → SVG → byte-identical cache hit) · running
server smoke-tested end to end.

**Now in P0+:** caller auth (FR7 minimal — `KR0KI_AUTH_TOKEN` env var gates all routes
except `/health` with `Authorization: Bearer <token>`), PNG output (`?output=png` on
render and cache endpoints). **Still not in P0+:** CDN tier (FR5's real target = D5),
the vendored `kroki-mcp` (direct HTTP is enough for raw text), PDF output, and the
entire SysML-model path.

## Endpoints

| Route | Method | Query | Body | Response |
|---|---|---|---|---|
| `/health` | GET | — | — | `{"status":"ok","service":"kr0ki","version":"..."}` |
| `/formats` | GET | — | — | `["plantuml","c4plantuml","graphviz","d2",...]` |
| `/render/{format}` | POST | `?output=svg\|png` | raw diagram text | rendered bytes + `Content-Type` + `X-Kr0ki-Cache` + `X-Kr0ki-Key` |
| `/cache/{key}` | GET | `?output=svg\|png` | — | cached bytes or 404 |
| `/docs` | GET | — | — | HTML docs (harvested from kr0ki source) |
| `/docs/api.json` | GET | — | — | JSON symbol export |
| `/docs/api.tomllm` | GET | — | — | b00t-format .tomllm export |
| `/docs/api.rustdoc` | GET | — | — | rustdoc-style export |

Auth: set `KR0KI_AUTH_TOKEN` env var to require `Authorization: Bearer <token>` on all
routes except `/health`.

**Docgen / mdb00k:** kr0ki harvests its own Rust source at runtime (via `syn`) and serves
structured docs in b00t `docgen.rs` formats. Pattern derived from `b00t-cli/src/commands/docgen.rs`
— not duplicated, but extended for Rust source. Endpoints above are live; visit `/docs` after
starting the server.

## Templates — b00t stack orchestration pattern

`templates/` contains a **functional template** for services that consume kr0ki as their rendering cut-node:

| File | Purpose |
|---|---|
| [`templates/b00t-stack-orchestration.d2`](templates/b00t-stack-orchestration.d2) | **Executable diagram** — valid D2 source rendered by kr0ki P0 (`POST /render/d2`). Describes the b00t orchestration invariant: N services × 1 cut-node = DAG, not mesh. |
| [`templates/datum.template.toml`](templates/datum.template.toml) | **b00t registration datum** — copy and submit to `elasticdotventures/_b00t_` (FR8). |
| [`templates/service-integration.template.rs`](templates/service-integration.template.rs) | **Rust integration snippet** — how a sibling service calls `RenderService` programmatically. |

Render the template:
```bash
just render-template              # via public Kroki
just render-template http://localhost:8787   # via local kr0ki-server
```

The template is exercised in CI by `crates/kr0ki-core/tests/template_render.rs` (env-gated on `KR0KI_TEST_BACKEND`, same pattern as `live_render.rs`).

```bash
just test          # unit + in-process HTTP
just kroki-up      # local SECURE-mode Kroki on :8000
just run           # default: bind 0.0.0.0:8787, backend https://kroki.io
just run 127.0.0.1:8787 http://localhost:8000
```
