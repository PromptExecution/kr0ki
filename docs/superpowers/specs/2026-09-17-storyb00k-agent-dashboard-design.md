# DESIGN — storyb00k: an agentic digital-thread query & storytelling panel for kr0ki's playbook

**Status:** draft design, pending review. Not approved. Not a plan. **Revision 2** —
rewritten per explicit correction: harmonized with `PLAN-KR0KI-003`, LLM backend made
generic (any `OPENAI_API_KEY`/`OPENAI_API_URL`), model-**query** capability added as a
first-class Phase 1 concern (not just rendering), a named future discovery phase added,
and two more sources incorporated (arXiv:2609.16252, and `AI4MBSE/MBSE-AI-SysML-V2`'s
concepts re-emphasized as **Cameo-free** inspiration).
**Owner:** PromptExecution (@elasticdotventures). **Created:** 2026-09-17.

---

## 0. Why

kr0ki can render a diagram from text it's handed. It cannot currently **answer a
question about a live SysML v2 model** — `kr0ki-sysmlv2-client` already has a working
read API (`projects()`, `branches()`, `commits()`, `elements()`, `roots()`,
`relationships()`, `snapshot()`) but it is used **only internally**, inside the
model-render path (`RenderService::render_model`); nothing external can query it.
Nor can kr0ki narrate, walk a stakeholder through a system's digital thread, or
assemble several renders and query results into one presentation-ready dashboard.

Separately, there is a real local inference asset unused by kr0ki: the
`b00t-ch0nky-llamacpp` container (OpenAI-compatible API, currently at
`http://127.0.0.1:8001/v1/`). This spec's LLM backend config is **generic**, not tied
to that one endpoint — see §4.

This spec adds **storyb00k** — a new panel in kr0ki's existing `playbook/` Vue
frontend, backed by a new sidecar agent process, that lets a user have an LLM-narrated
session in which the agent (a) **queries** the live SysML v2 model for system design
comprehension and (b) operates kr0ki's existing render tools to build a live,
multi-panel dashboard explaining what it found.

## 1. Inspiration sources — what's actually being reused, concept by concept

| Source | Concept taken | Explicitly NOT taken |
|---|---|---|
| SysTemp (arXiv:2506.21608) | Skeleton→fill→parser-repair loop — already validated by kr0ki's own `sysml-derive`+`validate_sysml_v2`, cited as external confirmation, not new design | Runtime Jinja2 string templating (kr0ki's compile-time typed template is stronger) |
| Ji, "AI as Consumer and Participant" (arXiv:2604.25526) | Epistemic Accountability / Contribution Governance framing — motivates §6's labeling design | No concrete architecture proposed in that paper; not itself implemented here |
| **Gower, Henshaw, Ji, "Models as Governed Interfaces" (arXiv:2609.16252)** — new this revision | **Read-side adequacy (EA1–EA4)**: an ungoverned comprehension query confabulates 9/15 under deadline pressure; a governed epistemic-status layer cuts that to ~2%. **Candidate B**: a *sideways* RDF/named-graph metadata layer keyed by stable element ids, joined by the querying agent on identifier — their own recommended low-risk starting point, requiring no change to the model engineering semantics. Three-tier agent framing (retrieval / synthesis / governance) — retrieval and synthesis both "confabulate under pressure," safety comes from substrate refusal, not agent restraint. **Write-side admissibility (AC1–AC8)**, esp. AC5/AC6 quarantine — now a *third* independent source (after Ji's position paper and `reqif-opa-mcp`'s OPA pattern) converging on the identical Phase 2 governance shape | GQAF is unimplemented, architecture-only — nothing here is code to import. System discovery is explicitly out of that paper's scope. |
| `AI4MBSE/MBSE-AI-SysML-V2` — **concepts, not Cameo** | The **MagicGrid** package-organization convention (`Stakeholders`/`Requirements`/`SystemContext`/`LogicalArchitecture`/`PhysicalArchitecture`) as a narrative scaffold for whole-model comprehension walkthroughs; a reusable INCOSE/EARS requirements-quality prompt fragment; the plan→generate→validate→repair control-loop shape | **Everything Cameo/MagicDraw-specific**: no `ElementsFactory`/Open API calls, no `localhost:8765`/`8770` REST harness, no dependency on a commercial MBSE tool anywhere in this design. storyb00k talks to kr0ki's own `/mcp/tools` manifest exclusively. |

## 2. Harmonization with `PLAN-KR0KI-003`

`PLAN-KR0KI-003` (the Rust-source front-end: AST/IaC → typed graph → diagram-as-code →
render) is a **producer** storyb00k is designed to consume, not a pipeline storyb00k
duplicates or competes with. That plan's own §3 table already names this relationship:
*"`playbook/` ... may eventually display diagrams produced by this pipeline as one more
fixture in its catalog, but it is a consumer, never the generator."* storyb00k is
exactly that consumer, generalized from "one more fixture" to "one more dashboard
panel type, narrated in context":

- **Today** (Phase 1, this spec): storyb00k's dashboard panels are sourced from (a) kr0ki's
  existing `render_diagram`/`render_kubernetes_manifest` tools and (b) the new
  model-**query** tools (§5) — never from hand-authored diagram text pretending to be
  generated, and never by storyb00k inventing its own AST-introspection logic.
  Everything upstream of "call an existing render/query tool" is out of scope here,
  exactly per `PLAN-KR0KI-003` §5's non-goals.
- **Once `PLAN-KR0KI-003`'s Rust arm ships** (unrelated, separate work): a
  `render_rust_source_diagram`-shaped tool would appear in kr0ki's `/mcp/tools`
  manifest the same way `render_kubernetes_manifest` already does. storyb00k's sidecar
  needs **zero code changes** to pick it up — it dispatches generically from whatever
  the manifest advertises (§4), the same "add a capability, nothing downstream changes"
  property `bridge.py` already has. This is the concrete form of "harmonization": the
  two plans compose through kr0ki's existing manifest-dispatch boundary, not through any
  new coupling this spec introduces.
- storyb00k's own scope stays strictly on the "ask a question, get an answer/dashboard"
  side. It does not do AST walking, IaC parsing, or diagram-as-code emission itself —
  that discipline (`docs/PLAN-KR0KI-003-rust-source-frontend.md` §1's "the specific
  mistake to avoid repeating") is upheld here by construction: storyb00k has no source
  code of its own that touches `syn`, `docgen`, or diagram-as-code generation logic.

## 3. Scope

### Phase 1 (this spec, in scope)

- **New model-query MCP tools** wrapping `kr0ki-sysmlv2-client`'s existing read API —
  see §5. Entirely read-only; no new write capability anywhere.
- A new sidecar container, agent run-loop + AG-UI protocol server, proxying LLM calls to
  a **generically configured** OpenAI-compatible endpoint (§4) and dispatching tool
  calls against kr0ki's `/mcp/tools` manifest — both the pre-existing render tools and
  the new query tools.
- A new `storyb00k` panel/route in `playbook/`, built on `ag-ui-vue`'s composables,
  showing a chat/narration transcript alongside a live-updating multi-panel dashboard
  that visually distinguishes **query results** (retrieval — grounded in the live
  model) from **agent narration** (synthesis — the paper's own confabulation-risk
  category) — see §6.
- The full request → tool-call (query or render) → dashboard-panel → narration loop,
  working end to end against a real local LLM and the real deployed pod.

### Phase 2 (named, explicitly NOT started here)

An **authoring assistant** — interrupt-gated proposal/approval flow for actually
*editing* a SysML v2 model. Blocked on: new kr0ki-side authoring MCP tools
(`create_requirement`, `create_part_definition`, ...), a Flexo MMS **write** path
(`kr0ki-sysmlv2-client` is read-only today), and a full governance composition. Three
independent sources now converge on the same shape for that gate — Ji's Contribution
Governance principle, `reqif-opa-mcp`'s "agents produce facts, a policy engine is the
sole gate" (OPA/Rego), and this revision's AC5/AC6 quarantine-and-reviewed-promotion
pattern (arXiv:2609.16252) — strong evidence the eventual Phase 2 design should adopt
this shape, but it remains undesigned; still its own future brainstorm.

### Phase 3 (named, explicitly NOT started here) — discovery and mapping of existing systems

Turning an *existing*, not-yet-modeled system (running infrastructure, a codebase) into
queryable model data. This is **not new scope for storyb00k to build** — it is
`PLAN-KR0KI-003`'s Rust arm (AST → typed graph) and the already-shipped Kubernetes
recognizer (`k8s_recognizer.rs`, kr0ki#12), generalized as additional "system truth"
sources. storyb00k's role in Phase 3, if and when it arrives, is unchanged from
Phase 1: consume whatever new query/render tools those pipelines expose through the
same manifest-dispatch mechanism, narrate over them. Named here only so Phase 1's
architecture doesn't foreclose it (it doesn't — see §2's harmonization argument).

## 4. Architecture

```
playbook/ (Vue 3 + Vite, static build served by kr0ki-server)
  └─ storyb00k panel (new)
       ag-ui-vue: useChat (narration transcript)
                  useAgentState (typed dashboard state: panels tagged
                                 kind: "query-result" | "render" | "narration")
       ── AG-UI protocol (HTTP POST run, SSE event stream) ──►
                                                    kr0ki-storyb00k-agent (new sidecar)
                                                      - AG-UI SSE server
                                                      - fetches kr0ki's GET /mcp/tools
                                                        manifest (cached, reusing the
                                                        shared dispatch module, §7)
                                                      - LLM calls via a GENERIC
                                                        OpenAI-compatible client:
                                                        OPENAI_API_KEY, OPENAI_API_URL
                                                        (env-configured; this pod's own
                                                        .env points these at the
                                                        existing ch0nky llama.cpp
                                                        container by default, but the
                                                        sidecar has no hardcoded
                                                        endpoint anywhere in its code)
                                                      - dispatches tool calls (query OR
                                                        render) via the shared
                                                        manifest-dispatch module ──►
                                                                 kr0ki-server:
                                                                 - existing /render/*
                                                                 - NEW /model/* (§5),
                                                                   wrapping
                                                                   kr0ki-sysmlv2-client
                                                                   against
                                                                   KR0KI_SYSMLV2_BASE_URL
                                                                   (+ optional
                                                                   KR0KI_SYSMLV2_TOKEN)
                                                                   — the SAME env var
                                                                   names already
                                                                   established by that
                                                                   crate's own live test
```

**Generic LLM backend — no vendor/endpoint lock-in anywhere in the sidecar's code.**
`OPENAI_API_KEY` and `OPENAI_API_URL` are the only LLM-related configuration the
sidecar reads. This makes the local `ch0nky` endpoint one valid configuration among
many (a cloud OpenAI-compatible API, a different local runtime, etc.) rather than a
hardcoded assumption — matching arXiv:2609.16252's own framing (tested against three
frontier LLMs, explicitly model-agnostic) and the general "don't reinvent" instruction
(this is a near-universal community convention already supported by most
OpenAI-compatible clients/SDKs, not a kr0ki invention).

**New: `kr0ki-server` gains a model-query surface, not just a model-render surface.**
`kr0ki-sysmlv2-client`'s existing read API has never been reachable from outside the
render path. §5 closes that gap with a small, read-only set of new HTTP routes +
`McpTool` variants, reusing the crate as-is (zero changes to `kr0ki-sysmlv2-client`
itself) and reusing its already-established `KR0KI_SYSMLV2_BASE_URL`/
`KR0KI_SYSMLV2_TOKEN` env-var convention for which backend (Flexo MMS, the OMG
reference API, SysON) it targets.

**Where the sidecar lives, and why:** the same pod as `kroki`/`kr0ki`/`kr0ki-mcp`, as a
fourth container — following the precedent `kr0ki-mcp` already establishes (a
stateful/session-scoped concern lives in its own sidecar, keeping `kr0ki-server`'s core
`RenderService` stateless and cache-keyed). An AG-UI run-loop is exactly this kind of
side-concern. The new `/model/*` query routes, by contrast, belong in `kr0ki-server`
itself (§5) — they're stateless request/response wrappers around
`kr0ki-sysmlv2-client`, architecturally identical in shape to the existing `/render/*`
routes, not a new stateful concern.

**Language:** Python for the sidecar, matching `kr0ki-mcp`'s established precedent — no
Rust AG-UI SDK exists, and the LLM-facing/tool-calling ecosystem is better supported in
Python today. Rust for the new `/model/*` routes in `kr0ki-server`, same as every other
route there.

## 5. New MCP tools — model query surface

New `kr0ki-core::mcp_tool::McpTool` variants (same closed-enum pattern established in
the mcp-http-parity work) and matching `kr0ki-server` routes, each a thin wrapper over
one `kr0ki-sysmlv2-client` method. Exact route paths/schemas are implementation-plan
detail, not decided here — the **set** of capabilities is the design decision:

| Tool | Wraps | Purpose |
|---|---|---|
| `list_model_projects` | `SysmlV2Client::projects()` | "what systems/projects exist" |
| `list_model_commits` | `commits(project_id)` | which snapshots exist, for the digital-thread history angle |
| `get_model_snapshot` | `snapshot(project_id, commit_id)` | the full content-hashed element+root set kr0ki already uses internally for rendering — now queryable directly |
| `query_model_elements` | `elements()`/`all_elements()` | "what's in this model" — the base comprehension primitive |
| `get_model_roots` | `roots()` | entry points into the model, for narration starting points |
| `query_model_relationships` | `relationships()` | traversal — "what connects to what," `direction=in\|out\|both` already supported by the client |

**Deliberately excluded from Phase 1:** anything that writes. `branches()`/`branch()`/
`tags()`/`tag()` (version-graph navigation) are candidates for a later revision if
storyb00k's narration needs to compare commits, but are not required for the Phase 1
happy path (§6) and are left out to keep this addition minimal — YAGNI.

## 6. Data flow (happy path) — and the retrieval/synthesis distinction

1. User opens the `storyb00k` panel, types a prompt (e.g. "walk me through this
   system's digital thread" or "why does this design have five of this component").
2. Frontend POSTs a `RunAgentInput` to the sidecar's AG-UI endpoint.
3. Sidecar fetches the `/mcp/tools` manifest (query tools + render tools together),
   emits `RunStarted`, calls the configured LLM with the manifest as available tools.
4. The LLM calls a **query** tool first for comprehension questions (e.g.
   `query_model_elements`, `query_model_relationships`) — the sidecar dispatches it,
   gets structured model data back, and emits a `StateDelta` adding a dashboard panel
   tagged `kind: "query-result"`. This panel's content came directly from the live
   model — it is retrieval, not agent invention.
5. The LLM may also call a **render** tool (`render_diagram`/`render_kubernetes_manifest`)
   to visualize what it found — another `StateDelta`, panel tagged `kind: "render"`.
6. The LLM narrates (streamed `TextMessage*` events) explaining what the query/render
   results mean — this narration is separately tagged `kind: "narration"` in the
   dashboard state. **This tagging is the concrete, buildable-now response to
   arXiv:2609.16252's finding**: it doesn't require the paper's full EA1–EA4 metadata
   layer (Candidate B, named as a real future extension point below, not built here) —
   it's a UI/prompt-level convention available today: every dashboard panel is visibly
   labeled by its provenance category (query-result / render / narration), so a viewer
   can immediately tell "this came from the model" apart from "the agent said this
   about the model" — directly mitigating the paper's core risk (ungoverned synthesis
   presented indistinguishably from grounded retrieval) without waiting on Phase 2's
   full governance system.
7. `RunFinished`. The user has a dashboard mixing grounded model data and kr0ki
   renders with agent narration, each visibly labeled, plus the full transcript.

**Extension point, not built here:** arXiv:2609.16252's Candidate B (a sideways
RDF/named-graph metadata layer, keyed by kr0ki's own stable element ids, joined by the
querying agent at query time) is the natural next step if/when storyb00k needs true
epistemic-status tracking (verified vs. target vs. assumption) rather than the
coarser query-vs-narration UI label this spec ships. Explicitly deferred — the paper's
own recommendation is that this is additive (a second graph, not a rewrite of
`kr0ki-sysmlv2-client` or the model engineering semantics), so nothing in Phase 1
forecloses adding it later.

## 7. Reuse, not reinvention — the manifest-dispatch module

`containers/kr0ki-mcp/bridge.py` (built this session, mcp-http-parity work) already
contains exactly the dispatch logic this sidecar needs: `fetch_manifest()`,
`find_tool()`, `apply_binding()`, `http_call()`. Extract these into a small shared
module (e.g. `containers/kr0ki-mcp/manifest_dispatch.py`) that both `bridge.py` (stdio
MCP) and the new `kr0ki-storyb00k-agent` sidecar (AG-UI/HTTP) import, rather than
reimplementing this a second time. Small, low-risk refactor of already-shipped,
already-reviewed code — its own task in any eventual implementation plan, not bundled
silently into a storyb00k-specific task, since `bridge.py` is production code with its
own test coverage that must not regress.

## 8. What Phase 1 explicitly does not include

- No model editing / authoring tools, no Flexo write path, no interrupt/approval flow —
  every Phase 1 tool call (query or render) is read-only and inherently side-effect-free.
  `ag-ui-vue`'s `useFrontendTool`/`requireConfirmation` is not used in Phase 1.
- No new repository — storyb00k lives inside kr0ki's existing `playbook/` and pod.
- No EA1–EA4 epistemic-status metadata layer (Candidate B, §6) — only the coarser
  query/render/narration UI-level distinction.
- No multi-user/multi-session concurrency handling. One browser tab, one agent session.
- No system-discovery/reverse-engineering capability (Phase 3, §3) — storyb00k
  consumes whatever `PLAN-KR0KI-003` and the Kubernetes recognizer already expose or
  will expose; it builds neither.
- No AST walking, IaC parsing, or diagram-as-code emission inside storyb00k itself —
  per §2's harmonization argument, that discipline belongs entirely to
  `PLAN-KR0KI-003`/box 3, never duplicated here.

## 9. Testing / validation approach

- **`kr0ki-server`'s new `/model/*` routes:** same test shape as every existing route
  (`tests/http.rs`) — unconfigured (`KR0KI_SYSMLV2_BASE_URL` unset) → clean 503, not a
  panic, matching the house convention already used by `/b00t-graph` and
  `/capabilities`; a mocked-backend test per route for the happy path.
- **Sidecar:** unit tests for the shared `manifest_dispatch.py` module (covering both
  callers' usage shapes), a mocked-LLM integration test proving the
  tool-call → `StateDelta` → tagged-dashboard-panel path works end to end without
  hitting a real LLM endpoint.
- **Live test:** an env-gated, manually-run test against a real configured LLM endpoint
  (via `OPENAI_API_KEY`/`OPENAI_API_URL`) and a real deployed pod with a real
  `KR0KI_SYSMLV2_BASE_URL` target — matching the house convention already established
  (`live.rs`, `kubediagram_live.rs`, `playbook_live.rs`).
- **Frontend:** component test for dashboard panels given a mocked AG-UI event stream,
  including a test asserting the three panel `kind`s render with visually distinct
  labeling (the concrete, testable form of §6's retrieval/synthesis distinction).

## 10a. Revision 3 addenda — multi-modal panels, RDF/SHACL graph store, session auto-load, and gated live mutation

Four more requirements, folded in per explicit direction. Each is additive to
revisions 1-2 above; nothing in §§0-10 is retracted, only extended.

### Multi-modal panels — SVG/PNG is not the only representation

Dashboard panels (§4, §6) gain a fourth `kind`: **`sysml-text`** alongside
`query-result`/`render`/`narration`. Any panel whose content came from a render call
already has the underlying SysML v2/diagram-as-code source available (it's the request
body); storyb00k must show both the rasterized SVG/PNG **and** the source text side by
side, toggle-able, not source-then-discard. This is a UI/state-shape decision, not new
backend capability — `useAgentState`'s panel schema (§10, already an open item) now
explicitly carries `{kind, rendered: {svg|png bytes}, source: {text, format}}` rather
than rendered-output-only.

### RDF Turtle + SHACL graph store — `oxttl`/`oxrdf` directly, not HelixDB, not the full `oxigraph` database

**HelixDB was investigated and is a poor fit, not adopted.** It is a Rust-native
property-graph + vector database (its own HelixQL query DSL, `g().add_n()`-style) —
exhaustively confirmed to have **zero** native RDF, Turtle, SPARQL, or SHACL support
anywhere in its codebase or docs. Adopting it for this requirement would mean building
an entire RDF-mapping and SHACL-validation layer from scratch on a database with no
concept of either — the opposite of "don't reinvent the wheel."

**Also corrected: no new database dependency at all.** An earlier draft of this
revision proposed embedding the full `oxigraph` crate (a complete triplestore database
with its own storage engine and SPARQL execution). That's more than this need calls
for. **kr0ki already vendors `oxttl` (Turtle parser) and `oxrdf` (RDF term/triple
types) in `crates/kr0ki-core/Cargo.toml`, already used for parsing in
`crates/kr0ki-core/src/b00t_graph.rs`.** Those two crates alone are sufficient: a small,
new, hand-rolled in-memory triple index (a `Vec<Triple>`/`HashMap`-backed store using
`oxrdf`'s own `Triple`/`Term`/`NamedNode` types, no external storage engine) plus a
bounded, hand-rolled pattern-matching query function (not full SPARQL — a closed set of
query shapes, matching kr0ki's own established "closed enum, not general machinery"
philosophy) covers the actual requirement without adding a new dependency family at
all. SHACL validation follows the same posture: a small, hand-rolled shape-checker over
a fixed, closed set of shapes (not a general SHACL engine) — consistent with how kr0ki
already prefers bounded, purpose-built validation over general frameworks elsewhere in
this codebase.

**What it's for — arXiv:2609.16252's Candidate B, now built rather than deferred.** §6
named a "sideways RDF/named-graph metadata layer, keyed by stable element ids, joined
by the querying agent at query time" as a future extension point. This revision pulls
that into Phase 1 scope: a new `kr0ki-core` module (sibling to `b00t_graph.rs`, e.g.
`graph_store.rs`) holds this in-memory triple store. Every model-query tool result (§5)
is also materialized as `oxrdf::Triple`s in this store (element → its properties, its
relationships, its provenance) alongside whatever SysML v2 text/diagrams were already
being produced — giving storyb00k's agent a genuine, traversable graph, not just
one-shot REST calls, for comprehension questions that benefit from graph traversal
("what depends on this, transitively"). The hand-rolled SHACL-style shape checks
validate that materialized graph's structural integrity — the RDF-layer analog of
`validate_sysml_v2`'s role for concrete text, same "always validate, never visually
inspected" discipline `DESIGN-NOTE-typed-model-layer.md` §2.7 established.

**New MCP tool:** `query_model_graph` (a bounded query shape in — e.g. "all triples
about element X," "all elements related to X via relation Y" — RDF triples out) —
joins §5's table as a seventh tool. Not a SPARQL endpoint; a small, closed set of
graph-traversal query shapes over the in-memory store, sized to what storyb00k's
narration agent actually needs.

**Deployment:** embedded in `kr0ki-server` (a Rust module, in-process, no new
dependency, no new container, no new storage engine). Held in memory for the process
lifetime, rebuilt from `kr0ki-sysmlv2-client` query results as needed — it is a
**derived, disposable cache** of the authoritative model, never itself the source of
truth, exactly like `FsCache`'s own content-addressed discipline: losing it and
rebuilding from Flexo is always safe.

### MCP-tools, agent-chat, and skills auto-load at session start

storyb00k's sidecar fetches (once, cached, same pattern as `bridge.py`) three things at
session start, not on first use: (1) kr0ki's `/mcp/tools` manifest — already the
established pattern; (2) a small, **self-contained** set of skill-prompt fragments
bundled with the sidecar container itself (the MagicGrid narrative scaffold and the
INCOSE/EARS requirements-quality fragment named in §1, plus any future additions) —
loaded as static files, not a network fetch; (3) the chat UI itself initializes
pre-connected (no manual "connect" step in `playbook/` — opening the `storyb00k` panel
immediately establishes the AG-UI session and shows the loaded manifest/skills as
context, before the user types anything).

**Noted, not adopted:** a sibling b00t-cli development effort (`CLAUDE_DYNAMIC_SKILLS.md`,
an unshipped, separate-repo design for role-aware skill-mounting via namespaces and a
`CLAUDE_SKILLS_ROOT` env var) is aspirational infrastructure, not yet integrated
anywhere. storyb00k's own skill-bundling stays self-contained or later swaps its
static-file loading for that mount point if/when it ships — the interface (a directory
of skill-prompt fragments) is compatible either way, so nothing here is lost by not
waiting on it.

### Gated live mutation — "until the user and agent agree," scoped to the draft graph, not the authoritative model

This is **not** Phase 2's authoring path (Flexo write + OPA governance) pulled forward
wholesale — that remains a separate, unbuilt, future-brainstormed system. What's in
scope here is narrower and safer: the agent and user can **iteratively edit the
in-session draft** — a copy of the queried triples (§10a's RDF layer) held entirely in
the Python sidecar's own memory, via [`rdflib`](https://github.com/RDFLib/rdflib) (the
standard, pure-Python RDF library — the natural Python-side analog of `oxttl`/`oxrdf`
on the Rust side, not a new architectural pattern) — and/or a panel's SysML v2 source
text, with every proposed change shown as a diff, applied only once both sides agree,
using `ag-ui-vue`'s existing `useFrontendTool`/`requireConfirmation` mechanism (already
a Phase 1 dependency per revision 2 §6 — this activates a capability already present in
the library, per the original §6 "how Phase 1 avoids foreclosing Phase 2" argument, now
exercised rather than dormant).

Concretely: a new tool, `propose_draft_change` (element/triple/text edit proposed,
never auto-applied), triggers an AG-UI interrupt; the frontend shows the diff; the user
approves or declines; on approval, the sidecar applies it to **its own in-memory,
session-scoped `rdflib` graph only** — never to `kr0ki-server`'s own triple store,
never to Flexo, never to the authoritative `kr0ki-sysmlv2-client`-backed model. The
session draft is gone when the browser tab closes; nothing about it is persisted.
Promoting an agreed draft change into the authoritative record (a real Flexo commit) is
explicitly **out of scope** here and remains Phase 2's job, gated by the fuller
governance composition (Flexo branch + OPA-style policy + AC5/AC6 quarantine-and-
reviewed-promotion) — this revision only builds the negotiation UI and the safe,
disposable place its results land.

## 10. Open items for the implementation plan (not decided here)

- Exact `/model/*` route paths, request/response JSON shapes, and `McpTool` variant
  names for §5's six new tools.
- Exact ASGI/HTTP library choice for the sidecar's SSE server (stdlib, matching
  `http_worker.py`'s minimalism, vs. a small async framework) — needs a quick spike.
- Exact dashboard-panel state shape (`useAgentState`'s typed schema), the
  storyb00k-specific system prompt (including whether/how to embed the MagicGrid
  narrative scaffold and the requirements-quality prompt fragment from
  `AI4MBSE/MBSE-AI-SysML-V2`, per §1) — implementation-plan-level detail.
- Pod manifest changes: new container entry, resource `limits`, a route in
  `playbook/`'s router, `OPENAI_API_KEY`/`OPENAI_API_URL`/`KR0KI_SYSMLV2_BASE_URL`/
  `KR0KI_SYSMLV2_TOKEN` wiring (the sidecar needs the last two to reach kr0ki-server's
  new `/model/*` routes, or reads them via the manifest-driven HTTP binding the same
  way it reaches `/render/*` — TBD at implementation time).
