# kr0ki

**The b00tyverse cut-node between [Kroki](https://kroki.io) and b00t/systhread —
now with an AI-narrated StoryB00k chat over the live model.**

kr0ki is the rendering + CDN-cached-artifact service layer for SysML/KerML diagrams in
the b00t ecosystem. It is deliberately *one node in the graph* — the boundary where an
abstract, validated model (owned upstream by `systhread` / `ufo-types`) becomes a
concrete, cached, referenceable picture (Mermaid / PlantUML / GraphViz / D2 / SVG,
served from `kr0ki.b00t.promptexecution.com`).

It does **not** own the model. It does **not** re-implement diagram grammars. It
wraps an existing multi-format renderer ([`kroki-mcp`](vendor/kroki-mcp), vendored)
and an existing isometric renderer (`systhread-core`'s `layout.rs`/`render.rs`),
and adds the one thing neither has: a caching, cross-referencing service surface.

## Capabilities (current)

- **Render loop** — raw Kroki-family diagram text → SVG/PNG through 27+ formats,
  content-addressed caching (`X-Kr0ki-Cache`/`X-Kr0ki-Key`), deterministic bytes.
- **Kubernetes arm** — `POST /render/k8s-topology` parses multi-doc manifests into a
  UFO-typed semantic graph and renders live topology diagrams; `/render/kubediagram`
  proxies the vendored KubeDiagrams worker.
- **b00t-graph arm** — `GET /b00t-graph/{tag}` renders committed `_b00t_` Turtle
  artifacts via holon-viz.
- **Rust-source arm** — `POST /render/rust-source` parses one Rust file with `syn` into
  `UfoRelation`-typed edges (`has_part`, `flows_to`, `requires`, `governed_by`, …) and renders them;
  method and trait-dispatch calls are deliberately not resolved (needs type information).
- **SysML v2 client** — server-agnostic OMG *Systems Modeling API* REST client
  (`kr0ki-sysmlv2-client`); `/model/*` routes light up when `KR0KI_SYSMLV2_BASE_URL`
  is configured, materializing a disposable RDF graph for querying.
- **Write path** — `POST /model/projects/{id}/sync` (and the `sync_digital_thread` MCP tool) reconciles a
  `SysGraph`'s `dbt:` nodes into a SysML v2 project as one commit: it diffs against the target branch's head,
  keeps other tools' fields (payloads are full replacements), retries on concurrent commits, and never touches
  elements it does not own. `kr0ki_core::flexo_reqif_sync` stores a ReqIF baseline the same way and reads it back.
- **Requirements** — ReqIF/ReqIFz import from bytes or an SSRF-hardened HTTPS fetch, asserted-relation export,
  requirement views, and `POST /model/projects/{id}/recompute` (Rego rules over the project's graph).
- **`kr0ki-svg`** — a WASM crate for SVG parsing and constraint-aware force layout (`fdg-sim`). Built and tested;
  not yet wired into the playbook.
- **Contract headers** — every response carries `X-Kr0ki-Contract` and `X-Kr0ki-Request-Id`.
- **Playb00k** — Vue 3 interactive harness at `/playbook/`: gallery of every format's
  test-backed fixtures, per-format editor with render + cache verification, Histoire
  stories, and `just test-playbook`/`just playbook-e2e` executable documentation.
- **StoryB00k chat** — AG-UI SSE sidecar (`:8789`) with an OpenAI-compatible LLM:
  narrates the live model, calls kr0ki tools (render, query, model reads) in a
  multi-round loop, and proposes **disposable draft changes gated behind explicit
  user approval** — the authoritative model is never mutated by the agent.
  Rendered diagrams carry an **EDIT** button that flips the playb00k editor
  preloaded with that diagram's source. Server-side session observability:
  `GET /debug/sessions[/{threadId}]`, per-thread JSONL transcripts, structured
  `kubectl logs` lines.
- **Deep health** — `/health` reports compiled version, uptime, dependency probes
  (kroki, kubediagram worker, storyb00k agent), LLM endpoint check (model count
  only — no billable request), store status, and auth posture. The welcome page
  validates backend reachability on load.
- **Self-documenting** — `/docs` harvests kr0ki's own Rust source at runtime
  (`/docs/api.json`, `/docs/api.tomllm`, `/docs/api.rustdoc`).

## License

kr0ki itself is **MIT** ([`LICENSE`](LICENSE)), applied workspace-wide via
`license.workspace = true` in every crate and `license` in `playbook/package.json`.

Vendored/depended-on components keep their own licenses:

| Component | License | Notes |
|---|---|---|
| [`vendor/kroki-mcp`](vendor/kroki-mcp) | MIT | b00tyverse fork of `utain/kroki-mcp`; the kr0ki-mcp sidecar's KubeDiagrams listener |
| [`vendor/assistant-ui`](vendor/assistant-ui) | MIT | fork tracking our upstream PR; `@assistant-ui/vue` source for the playb00k |
| [`vendor/kubediagrams`](vendor/kubediagrams) | Apache-2.0 | oracle + prior art for the k8s recognizer |
| `ufo-types` (crates.io/git dep) | MIT | the UFO semantic-graph vocabulary |
| Kroki (upstream project) | MIT-ish per-component | consumed as a container; kr0ki ships its own pinned `kroki-compat` image |

MIT-compatible throughout; the Apache-2.0 vendored component is noted here per its
NOTICE requirements. No GPL/AGPL components are vendored.

> Status: **foundational**. The decision-independent render loop (P0 — FR2 + FR5) is
> [built and merged](#p0--the-render-loop-built-2026-09-05). The SysML-model path
> (FR1/FR3/FR4) has started: its **client** — a generic OMG *Systems Modeling API*
> REST client — is in `crates/kr0ki-sysmlv2-client` (D3 resolved 2026-09-05); the typed
> adapter above it, and anything that emits SysML v2 text, stays blocked on D1/D6.
> Read [`docs/PRD-KR0KI-001-foundational.md`](docs/PRD-KR0KI-001-foundational.md) first,
> then [`docs/PLAN-KR0KI-002.md`](docs/PLAN-KR0KI-002.md) for the model path and
> [`docs/DESIGN-NOTE-typed-model-layer.md`](docs/DESIGN-NOTE-typed-model-layer.md) for
> the reviewed (not yet approved) shape of the deferred typed layer.

## Roadmap

Shipped since earlier versions of this README: the diagram-type catalog and discovery agent
([`PLAN-KR0KI-005`](docs/PLAN-KR0KI-005-gallery-catalog-type-discovery.md)), the Rust-source arm, the requirements
stack, and the hardened SysML v2 write path with its Flexo ReqIF adapter. Open, roughly in priority order
(the working list is [`docs/TODO.md`](docs/TODO.md)):

- **Live validation against a real OMG-API server** of the write path: how it answers a stale `previousCommit`,
  which fields `GET …/elements` returns, and whether custom `reqif_*` fields are accepted. The logic is covered by
  unit tests and a faithful stub, not by a real server.
- **Raster → diagram-as-code**: upload a diagram image from a local project directory and have a multimodal
  model iterate until a render "matches" (planned as `PLAN-KR0KI-007`).
- **Revisioned procedural workspace** ([`PLAN-KR0KI-004`](docs/PLAN-KR0KI-004-revisioned-procedural-workspace.md)):
  Phase 0 contracts are done; durable revisions exist today only as the agent's `jj` chart store.
- **Wire `kr0ki-svg`** into the playbook, and decide whether the layout engine stays in this repo.
- **CDN tier and artifact resolver** (D5 / FR6), the dbt manifest → `SysGraph` builder (upstream in `ufo-types`),
  and method/trait-dispatch call resolution for the Rust arm.

## Documentation

| Read | For |
|---|---|
| [`AGENTS.md`](AGENTS.md) | orientation for agents: what this is, the pipeline and its real status, scope guardrails |
| [`docs/OPERATIONS.md`](docs/OPERATIONS.md) | running, configuring, verifying and troubleshooting the services |
| [`docs/PRD-KR0KI-001-foundational.md`](docs/PRD-KR0KI-001-foundational.md) and the `PLAN-KR0KI-*` files | requirements and plans |
| [`docs/LESSONS-playbook-and-agent.md`](docs/LESSONS-playbook-and-agent.md) | incidents and how they are guarded against |
| [`docs/DESIGN-HISTORY.md`](docs/DESIGN-HISTORY.md) | earlier plans and what of them exists |

## Orientation for agents

| Thing | Where | Why it matters here |
|---|---|---|
| b00t SysML v2 spine epic (closed) | [`elasticdotventures/_b00t_#1177`](https://github.com/elasticdotventures/_b00t_/issues/1177) | The upstream model/validation layer kr0ki renders *from*. P0–P3 shipped. |
| `ufo-types` v0.14.0 | [`PromptExecution/ufo-types`](https://github.com/PromptExecution/ufo-types) | `ontology::{UfoRelation,OntologicalEdge}` (box 2, consumed by `kr0ki-core::ufo_graph`), `sysml_model::{ElementKind,Relation,ElementId}` (box 4), `view::SysmlViewKind`. kr0ki's input vocabulary. |
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
│   ├── kr0ki-core/               ← render loop, recognizers, requirements, write path
│   ├── kr0ki-server/             ← axum service
│   ├── kr0ki-svg/                ← WASM SVG parsing + constraint layout
│   └── kr0ki-sysmlv2-client/     ← generic OMG "Systems Modeling API" REST client (read + commit)
├── playbook/                     ← Vue 3/Vite playb00k (gallery, code editor, agent, setup)
├── containers/                   ← kr0ki-server, kr0ki-mcp bridge, kroki-compat, storyb00k agent images
├── deploy/                       ← local k0s pod manifest
├── docs/
│   ├── OPERATIONS.md                    ← run, configure, verify, troubleshoot
│   ├── LESSONS-playbook-and-agent.md    ← incidents: symptom, root cause, guard
│   ├── DESIGN-HISTORY.md                ← earlier plans and what of them exists
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

### SysML-model path

`crates/kr0ki-sysmlv2-client` is a **server-agnostic** async REST client for the OMG *Systems Modeling API and
Services* PSM — it works against Flexo `flexo-mms-sysmlv2`, the OMG Java pilot
`Systems-Modeling/SysML-v2-API-Services`, `Open-MBEE/OpenSysML`, and Eclipse SysON's `/api/rest/`. It is **not
Flexo-coupled**. It reads projects / branches / tags / commits / elements / relationships / roots, produces a
content-hashed `ModelSnapshot` (the model-side cache key, since no target server exposes its own content hash),
and can create commits (`create_commit`) for the write path.

kr0ki *consumes* an upstream OMG-API model server; it does not host the model (decision D3, 2026-09-05).

`ModelSnapshot` feeds the SysML-v2 arm of a five-box ingestion pipeline (`source → canonical UFO-typed
semantic graph → pattern recognizers → SysML v2 viewpoints → renderer adapters`). Which boxes exist, per arm, is
tracked in the table in [`AGENTS.md` §1](AGENTS.md); the plan is [`docs/PLAN-KR0KI-002.md`](docs/PLAN-KR0KI-002.md)
and the Kubernetes patterns are in [`docs/PATTERNS-kubernetes.md`](docs/PATTERNS-kubernetes.md).

## The render loop

`RenderService` is a content-addressed cache in front of a `RenderBackend`: raw Kroki-family diagram text in,
deterministic SVG/PNG out, served from cache on every repeat. The loop itself has no `ufo-types` /
`systhread-core` dependency. Around it, `kr0ki-core` hosts the recognizers and lifts (`k8s_recognizer`,
`rust_recognizer`, `ufo_graph`, `sysml_lift`, `sysml_render`), a separate box-5 arm (`b00t_graph`: a committed
`_b00t_` Turtle graph → D2 via `holon-viz`), the requirements stack, and the write path.

```
crates/kr0ki-core/src/
├── format.rs        DiagramFormat — the 26 companion-free Kroki formats only (NFR3)
├── cache.rs         cache_key() = SHA256(domain ‖ 0x1f-delimited fields); FsCache (atomic writes)
├── render.rs        HttpKrokiBackend — POST {base}/{slug}/{output}; flatten.rs rasterizes SVG when a format has no native PNG
├── k8s_recognizer.rs · rust_recognizer.rs · rust_lift.rs · ufo_graph.rs · sysml_lift.rs · sysml_render.rs
├── reqif_import.rs · reqif_export.rs · reqif_fetch.rs · requirements_*.rs · rule_eval.rs · recompute.rs
├── sync_engine.rs · digital_thread_sync.rs · flexo_reqif_sync.rs     (write path)
├── catalog.rs · examples.rs · mcp_tool.rs · docgen/                  (playbook catalog, tool manifest, /docs)
└── b00t_graph.rs
```

Quality gates: `just check` (fmt + clippy `-D warnings`) and `just test`; live tests are `#[ignore]`d and need a
backend or model server (see [`docs/OPERATIONS.md`](docs/OPERATIONS.md) §6). Not built yet: the CDN tier (FR5's
real target, D5), the artifact reference resolver (FR6), PDF output, and the vendored `kroki-mcp` hop (direct
HTTP is enough for raw text).

## Endpoints

| Route | Method | Notes |
|---|---|---|
| `/health` | GET | deep report `{status, service, version, started_at, uptime_secs, contract, checks{kroki_backend, kubediagram_worker, storyb00k_agent, llm, stores, caller_auth}}`; always 200, `status: ok\|degraded`; **never requires auth** |
| `/`, `/welcome` | GET | landing page; the welcome page shows a live health badge |
| `/formats` | GET | the 26 supported slugs |
| `/render/{format}` | POST | `?output=svg\|png`; body = diagram text; returns bytes + `X-Kr0ki-Cache` + `X-Kr0ki-Key` |
| `/render/k8s-topology` | POST | `?output`, `?view`; multi-doc Kubernetes YAML → UFO graph → D2 → render |
| `/render/kubediagram` | POST | `?output=svg\|dot_json`; manifest (≤1 MiB) proxied to the KubeDiagrams worker; not cached |
| `/render/rust-source` | POST | `?output`, `?view`; one Rust file (≤1 MiB) |
| `/render/requirements-view` | POST | requirement views as a diagram |
| `/render/sysmlv2/projects/{p}/commits/{c}` | POST | `?output`, `?view`; needs `KR0KI_SYSMLV2_BASE_URL` |
| `/requirements/import`, `/requirements/import/url`, `/requirements/views` | POST | ReqIF/ReqIFz import (bytes / HTTPS fetch) and views |
| `/model/projects`, `/model/projects/{p}/commits`, `…/commits/{c}/snapshot`, `…/elements`, `…/roots`, `…/elements/{e}/relationships` | GET | read-only proxy of the model server; `snapshot` also loads the disposable graph |
| `/model/graph/query` | GET | bounded query over that disposable graph |
| `/model/projects/{p}/recompute` | POST | rules over the project's graph → violations |
| `/model/projects/{p}/sync` | POST | `?branch_id`; body = `SysGraph` JSON; responds `{"commit": …}`; 409 conflict, 422 duplicate identifiers, 400 bad graph |
| `/b00t-graph/{tag}` | GET | `?output`; committed `_b00t_` Turtle → D2 → render |
| `/cache/{key}` | GET | `?output`; cached bytes or 404 |
| `/mcp/tools` | GET | the 16-tool manifest the `containers/kr0ki-mcp` bridge dispatches from |
| `/capabilities` | GET | backend's self-reported companion status per converter, or 503 |
| `/api/examples`, `/playbook/api/examples.json`, `/api/catalog` | GET | fixture catalog and the intent-first diagram-type taxonomy |
| `/playbook`, `/playbook/…` | GET | the built Vue playb00k |
| `/docs`, `/docs/api.json`, `/docs/api.tomllm`, `/docs/api.rustdoc` | GET | docs harvested from kr0ki's own Rust source |

**StoryB00k sidecar** (port `:8789`, same host; CORS-gated to the playbook origins):

| Route | Method | Body | Response |
|---|---|---|---|
| `/health` | GET | — | `{status, service, llm_configured, active_threads, max_model_tool_rounds, max_clarifying_questions}` |
| `/run` | POST | AG-UI `RunAgentInput` | SSE event stream (AG-UI protocol): narration, tool calls, state deltas, usage; interrupts for draft proposals |
| `/respond-to-interrupt` | POST | `{threadId, interruptId, approved}` | applies/declines a pending draft proposal |
| `/threads/{id}` | GET | — | pending proposals + draft graph as Turtle |
| `/debug/sessions` | GET | — | per-thread session summaries (runs, events, errors) |
| `/debug/sessions/{threadId}` | GET | — | **full ordered session log**: AG-UI frames emitted, LLM rounds with usage/model, tool calls with latency/ok, errors with tracebacks |

The sidecar also writes one JSONL transcript per thread under `KR0KI_DEBUG_LOG_DIR`
(mounted at `/var/lib/kr0ki/storyb00k-debug` in the pod) — greppable, restart-survivable
within the pod's lifetime.

Auth: set `KR0KI_AUTH_TOKEN` env var to require `Authorization: Bearer <token>` on all
routes except `/health`.

**Docgen / mdb00k:** kr0ki harvests its own Rust source at runtime (via `syn`) and serves
structured docs in b00t `docgen.rs` formats. Pattern derived from `b00t-cli/src/commands/docgen.rs`
— not duplicated, but extended for Rust source. Endpoints above are live; visit `/docs` after
starting the server.

**LAN docs:** run `just lan-url <LAN-host>`, then open its `/docs` path from another device
on the network. The playb00k includes the
implemented Rust render flow,
rendered alongside inspectable KerML and SysML v2 fixtures, then concrete health/render/cache/
docgen examples and test recipes. The fixture is documentation, not a replacement for the
deferred upstream `ModelSnapshot → UFO → recognizer → ViewDefinition` path.

Run `just playbook-e2e` after `just pod-up` to validate the deployed page itself:
health, the genuine D2-backed Rust-flow SVG, repeated render bytes/cache key, and a
second-request cache hit.

## Vue/Vite playb00k

`http://<LAN-host>:8787/playbook/` is the interactive evaluation
surface. Its left sidebar lists every standalone kr0ki input format; each format has
a dropdown of test-backed examples, editable source, SVG/PNG selector where supported,
and rendered-artifact preview. The source catalog is `kr0ki_core::examples::ALL`, so
Rust coverage, the live API (`/api/examples`), and mdb00k's static
`playbook/api/examples.json` remain one contract.

`playbook/` is a Vue 3/Vite app. `RendererPanel.story.vue` is its Histoire story for
isolated visual review and regression capture; it is a developer harness, not a
runtime dependency of the Rust service.

Run `just test-playbook` to submit every documented fixture through the deployed
HTTP surface twice. It verifies output signatures, deterministic artifact bytes,
and cache hits for every format/output combination that the UI advertises.

### StoryB00k — AI narration over the live model

The **storyb00k** view in the playb00k is an AG-UI chat backed by the
`kr0ki-storyb00k-agent` sidecar (`:8789`) and any OpenAI-compatible LLM
(`OPENAI_API_URL`/`OPENAI_API_KEY` from the machine-local `.env`):

- **Reads, never writes**: the agent calls kr0ki tools (`list_formats`,
  `render_diagram`, `query_model_elements`, …) through a multi-round loop and
  narrates what it finds. Model changes are only ever **disposable draft
  proposals** that the user must approve/decline in the chat.
- **Rendered charts carry an EDIT button** — one click flips to the playb00k
  editor with that diagram's source preloaded in the matching format, ready to
  tweak and re-render.
- **Observable**: every run is recorded server-side (see `/debug/sessions`
  above); the browser console logs the full AG-UI event stream and every
  send/finish/error (`debug: {events, lifecycle}`).
- Robustness: streaming text events, tool-error feedback to the LLM, output
  truncation guards, per-thread TTL, graceful client-disconnect handling.

### StoryB00k local settings

The StoryB00k sidecar is configured only from the machine-local `.env`, never
from committed deployment values. Copy `.env.example` to `.env`, set the
OpenAI-compatible endpoint/key and the browser origins that may use the sidecar,
then run `just pod-up`. That recipe creates or updates the local k0s
`kr0ki-local-env` Secret without displaying its values. The playbook derives the
sidecar host from the page host, so opening `http://<host>:8787/playbook/` reaches
`http://<host>:8789` on the same machine.

The model/tool-round safety ceiling (`KR0KI_STORYB00K_MAX_MODEL_TOOL_ROUNDS`,
default 15) is independent of the per-project user clarification budget
(`KR0KI_STORYB00K_MAX_CLARIFYING_QUESTIONS`, default 6). Candidate generation,
rendering, and image inspection use internal rounds, not clarification questions.

### Fast local dev loop (no k0s)

For format/fixture iteration, skip the podman-build → k0s-import → pod-recreate
cycle entirely: `just dev` runs our own pinned `kroki-compat` image via plain
`podman run` (not k0s) on `127.0.0.1:8010`, and `kr0ki-server` via `cargo run`
against it on `0.0.0.0:8787`. `just dev-kroki-down` stops the container when
done; `just dev` reuses an already-running one.

```bash
just dev            # Ctrl-C stops kr0ki-server; kroki-compat keeps running
just lan-url <LAN-host> # prints the URL to open from another LAN device
just playbook-e2e   # runs locally against 127.0.0.1:8787
just test-playbook  # runs locally against 127.0.0.1:8787
```

This is for iteration speed only — it can drift from the real k0s deployment
(different image, different pod securityContext), so always re-verify with the
full `just pod-up` cycle below before calling format or fixture work done.
`podman run` needs explicit `--memory`/`--cpus` (b00t's OCI limits hook rejects a
run without them); `dev-kroki-up` already sets them to match the pod manifest's
own 2Gi/1 CPU budget.

### Local k0s renderer compatibility

The k0s pod uses a pinned Kroki image with the x86-64-v3 PlantUML executable
replaced by the same-version, checksum-verified JVM JAR. This overlay is required
on sm3lly, whose k0s node exposes x86-64-v2. It does not loosen Kroki's `secure`
safe mode.

```bash
just pod-build
just k0s-load localhost/kr0ki-server:dev
just k0s-load localhost/kr0ki-mcp:dev
just k0s-load localhost/kr0ki-kroki-compat:dev
kubectl --context Default -n kr0ki delete pod kr0ki-local
kubectl --context Default apply -f deploy/kr0ki-local.pod.yaml
kubectl --context Default -n kr0ki wait --for=condition=Ready pod/kr0ki-local --timeout=180s
just playbook-e2e
just test-playbook
```

## Templates — b00t stack orchestration pattern

`templates/` contains a **functional template** for services that consume kr0ki as their rendering cut-node:

| File | Purpose |
|---|---|
| [`templates/b00t-stack-orchestration.d2`](templates/b00t-stack-orchestration.d2) | **Executable diagram** — valid D2 source rendered by kr0ki P0 (`POST /render/d2`). Describes the b00t orchestration invariant: N services × 1 cut-node = DAG, not mesh. |
| [`templates/datum.template.toml`](templates/datum.template.toml) | **b00t registration datum** — copy and submit to `elasticdotventures/_b00t_` (FR8). |
| [`templates/service-integration.template.rs`](templates/service-integration.template.rs) | **Rust integration snippet** — how a sibling service calls `RenderService` programmatically. |

Render the template:
```bash
just render-template              # via local kr0ki-server and private kroki-compat
just render-template http://localhost:8787   # explicit local kr0ki-server URL
```

The template is exercised in CI by `crates/kr0ki-core/tests/template_render.rs` (env-gated on `KR0KI_TEST_BACKEND`, same pattern as `live_render.rs`).

```bash
just test          # unit + in-process HTTP
just run           # default: bind 0.0.0.0:8787, backend http://127.0.0.1:8010
just run 0.0.0.0:8787 http://127.0.0.1:8010
```
