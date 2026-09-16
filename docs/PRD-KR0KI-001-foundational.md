# PRD-KR0KI-001 — kr0ki foundational requirements

**Status:** foundational. **P0 (FR2 + FR5 — the decision-independent render loop) is
implemented and merged** (`crates/kr0ki-core`, `crates/kr0ki-server`, PR #1). Everything
on the SysML-model side (FR1/FR3/FR4) remains requirements-only, blocked on §5.
**Owner:** PromptExecution (@elasticdotventures)
**Created:** 2026-09-05
**Supersedes:** nothing. **Superseded by:** nothing yet.

---

## 0. One paragraph

`kr0ki` is the **cut-node in the b00tyverse graph between Kroki and b00t/systhread**: the
single service boundary where a validated, abstract model (owned upstream by `systhread` /
`ufo-types`, serialized as KerML at each historical commit) is turned into a concrete,
CDN-cached, cross-referenceable diagram artifact and served from
`kr0ki.b00t.promptexecution.com`. kr0ki renders; it does not model, and it does not
invent diagram grammars. It **type-entangles** with the existing isometric renderer and
2D project engine in `systhread-core` — it does not duplicate them.

---

## 1. Why this exists (the cut-node argument)

The b00tyverse already has, all real and shipping:

- an abstract model + validation layer — `ufo-types` (`iso_ir::{Node,Edge}`,
  `stereotype::UfoStereotype`, `sysml::validate_sysml_v2`, `mbse::to_sysml_v2`),
  consolidated by the closed epic `elasticdotventures/_b00t_#1177`;
- a working `Rust type → iso_ir → SysML v2 / Mermaid / Rhai` prototype —
  `b00t-cli/src/dispatch_sysml.rs`;
- a deterministic **isometric 2D diagram renderer** — `systhread-core`'s
  `layout.rs`/`render.rs` (Cassowary/`kiwisolver` port of nem-poweragent-lab Lab 6),
  already emitting SVGs;
- a **force-directed 2D/3D graph explorer** design — `systhread-explorer`
  (nem-poweragent-lab spec `2026-08-26-systhread-3d-explorer-design.md`);
- a generic multi-format HTTP renderer — Kroki, wrapped today by the
  `_b00t_/kroki.mcp.toml` datum and by `kroki-mcp` (Go, MIT);
- a graph-reasoning layer — `ledgrrr` (SHACL / gremlin-style query + CLIF / Common
  Logic reasoning over a graph of the *same* KerML).

What is missing is the **node that joins them**: something that (a) accepts any of these
render inputs, (b) caches the rendered output on a CDN so identical models are never
re-rendered, and (c) can resolve references *between* rendered artifacts and other
stereotyped artifacts in the ecosystem. That node is kr0ki. Making it an explicit,
named cut-node — rather than letting each consumer wire its own Kroki call — is what
keeps the dependency graph a DAG with one crossing point instead of N.

### 1.1 What sits on each side of the cut

```
        MODEL SIDE (upstream, not kr0ki's)          │  RENDER SIDE (kr0ki)
                                                    │
  Rust types ──#[derive(SysmlBlock)]──┐             │
  ufo_types::UfoStereotype ───────────┤             │
  systhread-core typed model ─────────┼─► iso_ir ───┼─► kr0ki adapter ─► {Mermaid, PlantUML,
  KerML (_b00t_/types/b00tyverse.kerm)┤   Node/Edge │      │              GraphViz, D2, SVG,
  ledgrrr SHACL/CLIF-validated graph ─┘             │      │              isometric SVG}
                                                    │      ├─► CDN cache (content-addressed)
                                                    │      └─► artifact reference resolver
```

The cut is drawn at `iso_ir` deliberately: it is already the "cross-crate transport
floor" (`ufo-types` docs), already domain-agnostic, already what `dispatch_sysml.rs`
emits. kr0ki consumes `iso_ir` (+ the typed layer above it that `nem-poweragent-lab#53`
scopes) and nothing more model-ish than that.

### 1.2 Downstream consumers are their own leaves

`kroki-b00t` (`PromptExecution/infrastructure#217`) — the comic engine's self-hosted
Kroki + MCP server — is a **disconnected downstream leaf**, not part of kr0ki's core.
The comic team consumes kr0ki's rendered SVGs to make jokes about b00t. Any consumer
(comic engine, `b00t ontology diagram`, a docs site, an LLM tool surface) attaches to
kr0ki's service surface; none of them are in this PRD's scope beyond "kr0ki must not
make their use harder."

---

## 2. Scope

### 2.1 In scope (this PRD defines requirements for)

1. **Render-input contract** — the set of formats kr0ki accepts: `iso_ir` graph JSON,
   the typed SysML-v2/KerML view model from `nem-poweragent-lab#53`, raw Kroki-family
   text (Mermaid/PlantUML/GraphViz/D2/C4/Vega-Lite), and `systhread-core` isometric
   layout JSON.
2. **Renderer wrapping** — `kroki-mcp` (vendored, `vendor/kroki-mcp`) for the
   Kroki-family formats; `systhread-core`'s `render.rs` for isometric; a small
   `iso_ir → Mermaid/D2` adapter following `dispatch_sysml.rs`'s pattern for the graph
   case.
3. **CDN caching** — content-addressed (hash of the *validated* model + render options),
   so an unchanged model at any historical commit is rendered once, ever. Cache lives on
   a CDN (Cloudflare, matching b00t-node's existing edge).
4. **Artifact reference resolver** — given a rendered artifact, resolve references it
   carries to other stereotyped b00t artifacts (datums, other diagrams, `iso_ir` nodes)
   so diagrams can link into the ecosystem rather than being dead pictures.
5. **Service surface** — `kr0ki.b00t.promptexecution.com`, fronted by b00t-node's
   existing pingap (the `[upstreams.*]` `sni`/`verify_cert` pattern already proven for
   `k0s_api`, per `infrastructure#217`).
6. **b00t registration** — kr0ki is a top-level b00tyverse project: a datum in
   `elasticdotventures/_b00t_` pointing here, discoverable via `b00t discover`.

### 2.2 Type-entanglement, not duplication (hard requirement)

kr0ki MUST reuse, by value or by direct dependency (subject to §5), the existing types:

- `ufo_types::iso_ir::{Node, Edge}` — never a parallel graph type.
- `systhread-core`'s isometric layout/render types — kr0ki calls `render.rs`, it does
  not port or re-solve the Cassowary layout.
- `holon-viz::CytoscapeGraph` — where kr0ki touches the explorer's graph shape, it
  wraps the real type (as `systhread-explorer`'s `PositionedGraph` already does),
  pending `ledgrrr#203`.

"Type-entangled" means: one change to the upstream struct is a compile error in kr0ki,
not a silent drift. If that coupling is unacceptable to an upstream owner, §5 is where
that gets decided — kr0ki does not fork the type to dodge the question.

### 2.3 Non-goals

- **No model authoring / editing.** kr0ki renders validated input; it never mutates a
  model or writes back to source. (Same non-goal as `_b00t_#1177`: Rust is the only
  source of truth; SysML/diagram output is compiled, never authoritative.)
- **No new diagram grammar.** If Kroki can't render it, kr0ki doesn't invent it.
- **No SysML-v2 semantic parser of its own.** `ufo_types::sysml::validate_sysml_v2`
  (wrapping the real `sysml-v2-parser`) stays the syntax gate; a richer parser stays
  behind an oracle boundary (per `nem-poweragent-lab#53`).
- **No graph reasoning.** SHACL / gremlin-query / CLIF stays in `ledgrrr`. kr0ki may
  *call* ledgrrr to resolve a reference; it does not implement any of it.
- **No comic-engine / consumer-specific logic.** Those are leaves (§1.2).
- **Not a replacement for `_b00t_/kroki.mcp.toml`'s client role** — that datum stays as
  the way an agent *calls* a Kroki service; kr0ki is what it calls.

---

## 3. The graph, and what "serialized at each historical commit" means

`ledgrrr` treats the KerML (`_b00t_/types/b00tyverse.kerm` and its per-project
equivalents) as a graph, and — this is the load-bearing detail — **serializes that
graph at each historical commit** so it can be queried and CLIF-reasoned over time.
The graph project itself is *not* serialized except for examples; the serialized
artifact is the per-commit KerML snapshot.

kr0ki's caching key MUST be derived from that same per-commit serialized KerML (plus
render options), not from a mutable "latest" pointer. Consequences:

- A diagram for commit `abc123` is immutable and cacheable forever.
- kr0ki never needs to re-render history; a backfill renders each commit once.
- The reference resolver (§2.1.4) resolves against the graph *as of that commit*, so a
  historical diagram links to historically-correct artifacts.

---

## 4. Requirements

### Functional

- **FR1** — Accept `iso_ir` graph JSON and render it to Mermaid and D2, following
  `dispatch_sysml.rs`'s node/edge → diagram conventions (`edge_type: "sequence"` etc.).
- **FR2** ✅ *(P0, PR #1)* — Accept raw Kroki-family text and render it. P0 talks HTTP
  directly to a Kroki instance (`HttpKrokiBackend`); the vendored `kroki-mcp` hop is
  deferred (adds nothing for raw text). `SECURE` mode is the backend Kroki's config,
  not kr0ki's — input is LLM-authored, `!include`/`!includeurl` is an SSRF vector
  (`infrastructure#217`).
  - **Raw/hand-authored diagram testing is a permanent, first-class capability, not a
    stopgap superseded by `PLAN-KR0KI-003`'s generated pipeline.** The two coexist: an
    agent or a person can always paste or upload a hand-written diagram in any
    supported format and render it, independent of whether it was AST/IaC-derived.
    API + MCP already satisfy this (`GET /formats`, `POST /render/:format` take any
    source; `mcp__kr0ki-mcp__render_diagram` + `list_formats` wrap the same). ◑ **Gap:
    the `playbook/` web-ux does not** — `RendererPanel.vue` only edits/renders the
    pre-baked example catalog (`kr0ki_core::examples::ALL`), and its sidebar only lists
    formats that already have a fixture. It needs a general "custom diagram" mode:
    pick any of `DiagramFormat::ALL`'s 8 slugs, start blank (or upload a file), render.
    See `docs/TODO.md` box 5.
- **FR3** — Accept `systhread-core` isometric layout JSON and render via its `render.rs`.
- **FR4** — Accept the typed SysML-v2/KerML view model (`nem-poweragent-lab#53`) and
  lower it to the appropriate `ViewDefinition` before rendering.
- **FR5** ◑ *(P0 partial, PR #1)* — Content-address every render:
  `key = SHA256("kr0ki/v1" ‖ format ‖ output ‖ source)`, lowercase hex. P0 has the
  keying + a local `FsCache` origin store (atomic writes, sharded layout) and serves
  hits from `GET /cache/{key}`. The `serialized_kerml_at_commit` term applies only to
  the SysML-model path (absent in P0). The CDN tier is still D5.
- **FR6** — Resolve intra-ecosystem references carried by an artifact (datum ids,
  `iso_ir` node ids, other kr0ki artifact keys) to resolvable URLs, as of the artifact's
  commit.
- **FR7** — Expose the service at `kr0ki.b00t.promptexecution.com` with an auth boundary
  (no unauthenticated render calls from arbitrary callers — learn from the
  `xero-mcp-server-b00t` review: a service holding ecosystem credentials must
  authenticate its callers).
- **FR8** — Ship a b00t datum in `elasticdotventures/_b00t_` registering kr0ki as a
  top-level project, discoverable via `b00t discover kr0ki`.

### Non-functional

- **NFR1 — Determinism.** Same validated input ⇒ byte-identical artifact. Same bar as
  every other systhread artifact.
- **NFR2 — Vendored, pinned renderer.** `kroki-mcp` is a git submodule pinned to an
  exact SHA (`08765f64` today), built from source. Never `latest`, never an unpinned
  `git clone --branch`.
- **NFR3 — Core Kroki only.** No Mermaid/BlockDiag/Excalidraw companion containers
  (headless-Chromium memory cost — learned in `infrastructure#217`). JVM-bundled formats
  (PlantUML/GraphViz/Vega-Lite/C4/Ditaa) need no companion.
- **NFR4 — CDN-first.** Idle cost ≈ zero; a cache hit never starts a renderer.
- **NFR5 — b00t interface.** Agent-facing operations go through `mcp__b00t-mcp__*` /
  `b00t` CLI, not bespoke HTTP — kr0ki is a b00t datum surface, not a side channel.

---

## 5. Open decisions — kr0ki DOES NOT resolve these

kr0ki sits downstream of three unresolved upstream questions. This PRD explicitly
**defers** to them; implementation MUST NOT start on an assumed answer.

| # | Question | Where it's owned | kr0ki's stake |
|---|---|---|---|
| D1 | extend-vs-wrap-vs-re-export `sysml-derive` for `UfoStereotype`-tagged types | [`ledgrrr#202`](https://github.com/PromptExecution/ledgrrr/issues/202) | Determines how kr0ki's FR1/FR4 adapters get their SysML text. |
| D2 | Is a real (non-dev) dependency on `holon-viz` acceptable? | [`ledgrrr#203`](https://github.com/PromptExecution/ledgrrr/issues/203) | Determines whether kr0ki wraps `CytoscapeGraph` by value or copies the shape. |
| D3 | Does the typed SysML-v2/KerML view model live in `ufo-types`, `systhread-core`, or its own crate? | `nem-poweragent-lab#53` follow-up | Determines kr0ki's FR4 input type's home. |
| D4 | DNS/subdomain: who provisions `kr0ki.b00t.promptexecution.com` and where (pingap config in `b00t-node.yaml.tpl`)? | `PromptExecution/infrastructure` | Blocks FR7. |
| D5 | CDN: Cloudflare R2 + Workers, matching `_b00t_#1069`'s edge pattern? | `PromptExecution/infrastructure` | Blocks FR5's cache tier. |
| ~~D6~~ | ~~Vocabulary collisions: systhread "thread" vs KerML feature-chain, "viewpoint" overload, "projection" vs SysML view/viewpoint~~ | kr0ki | **RESOLVED 2026-09-05** — [`VOCABULARY.md`](VOCABULARY.md): adopt OMG SysML v2 / KerML spec terms verbatim, no informal synonyms, "digital thread" always qualified, "projection" banned. Un-entangled from D1 (spec names are stable — no b00t-invented name for D1 to invalidate). |

The bridge-session review pass (2026-09-05) flagged that D1/D2/D6 overlapped almost
exactly with `systhread`/`systhread-explorer`'s own open questions, and that resolution
MUST reconcile against existing prior art (`systhread-core` already exists; `_b00t_#1177`
is closed) rather than treating it as greenfield — **cross-check both threads before
landing anything here.**

---

## 6. Next steps

1. ~~Land the b00t registration datum (FR8)~~ — **done** (`_b00t_#1272`, `kr0ki.repo.toml`).
2. ~~Build the decision-independent render loop (FR2 + partial FR5)~~ — **done**, PR #1
   (`kr0ki-core` + `kr0ki-server`, 17 tests, live-verified against `kroki.io`).
3. Get the remaining D-answers (or explicit "proceed with X" from the owners). **Resolved so far:** D3 (typed layer lives in `ufo-types`), D6 (vocabulary — [`VOCABULARY.md`](VOCABULARY.md)). **Still open:** D1 (`sysml-derive`), D2 (`holon-viz` dep), D4 (DNS), D5 (CDN).
4. Add caller auth (FR7) so the P0 service can leave localhost.
5. Wire the CDN tier (D5) in front of `FsCache` — completes FR5.
6. Write `PLAN-KR0KI-002` (the SysML-model side, FR1/FR3/FR4). **`PLAN-KR0KI-002` now
   exists** ([`PLAN-KR0KI-002.md`](PLAN-KR0KI-002.md)) for the **client path**: the
   "only after §5 is settled" gate was **operator-lifted for that path only on
   2026-09-05** — decision **D3** is resolved (*kr0ki consumes an upstream OMG Systems
   Modeling API server; it does not host the model*), so `crates/kr0ki-sysmlv2-client`
   (a server-agnostic OMG-API REST client → content-hashed `ModelSnapshot`) shipped
   ahead of the rest of §5. **D1 still gates the authoring path** (D6 — vocabulary — resolved 2026-09-05) — anything that
   *emits* SysML v2 / KerML text, and the typed adapter above `ModelSnapshot`.
   Reviewed (not yet approved) starting shape for that deferred typed layer:
   [`DESIGN-NOTE-typed-model-layer.md`](DESIGN-NOTE-typed-model-layer.md) — **SysML v2 /
   KerML only**; closed `ElementKind` (v2 abstract syntax, def/usage-paired, no domain
   variants), typed `Relation` over the KerML relationship set + one `Domain` escape
   hatch, deterministic `Provenance`, `id: String` (git/content hash optional), views &
   viewpoints as `ViewDefinition` / `ViewpointDefinition` **data** (no sealed trait), no
   `SchemaVersion`. Every SysML-v2 text emit path MUST pass
   `ufo_types::sysml::validate_sysml_v2` (`sysml-v2-parser`, version-pinned in lockstep
   with `ufo-types`) — grammar-validated, never visually inspected. The v1 diagram-kind
   taxonomy (`DiagramKind` / `UmlRelation`) is
   rejected outright — it describes SysML 1.x's derivation from UML 2, which v2 does not
   have.

---

## Glossary

- **cut-node** — a graph vertex whose removal disconnects two subgraphs; here, the
  single sanctioned crossing between the model side and the render side.
- **CLIF** — Common Logic Interchange Format (ISO/IEC 24707); the "formally,
  mathematically reasonable" logic layer `ledgrrr` uses over the KerML graph.
- **iso_ir** — `ufo-types`' generic `Node`/`Edge` graph vocabulary, promoted from
  `systhread-core`; the transport floor kr0ki consumes.
- **KerML** — the Kernel Modeling Language underlying SysML v2; `b00tyverse.kerm` is the
  canonical instance.
- **type-entanglement** — reusing an upstream type such that upstream changes break
  kr0ki's build rather than drifting silently.
