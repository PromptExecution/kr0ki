# TODO — kr0ki working list

The single actionable list. Requirement definitions live in
[`PRD-KR0KI-001-foundational.md`](PRD-KR0KI-001-foundational.md); the ingestion design
in [`PLAN-KR0KI-002.md`](PLAN-KR0KI-002.md); the typed-layer shape in
[`DESIGN-NOTE-typed-model-layer.md`](DESIGN-NOTE-typed-model-layer.md). This file just
tracks *what is left to do*, ordered by the five-box pipeline.

_Last updated: 2026-09-05._

---

## Done

- [x] **P0 render loop** — `kr0ki-core` (`RenderService` = `FsCache` + `RenderBackend`),
  `kr0ki-server` (axum), content-addressed cache, `HttpKrokiBackend`. 17 tests,
  live-verified against `kroki.io`. (`#1`)
- [x] **SysML-v2-Release conformance harness (phase 1)** — consumes `sysml-v2-parser`
  0.55, pins tag `2026-07`, baselines 56/56 validation + 94/94 standard library,
  separate non-blocking CI job. (`#5`, `CONFORMANCE.md`)
- [x] **`kr0ki-sysmlv2-client`** — server-agnostic OMG Systems Modeling API REST client
  (Flexo / OMG pilot / OpenSysML / SysON); `ModelSnapshot` with a deterministic,
  order-independent SHA-256 `content_hash`. 20 tests. (`#7`)
- [x] **`ufo-types` type layers** — `sysml_model::{ElementKind (24), Relation (12 +
  Domain), ElementId}` (`#20`), `ontology::{UfoRelation (25), OntologicalEdge,
  TemporalExtent, SourceAnchor}` (`#21`), `view::SysmlViewKind` (`#20`); `ElementId`
  recognises `sha256:` / `blake3:` / `git:` (`#22`). ufo-types v0.14.0.
- [x] **Evaluations** — `EVAL-flexo.md`, `EVAL-syson.md`, `EVAL-kubediagrams.md`.
- [x] **Datums** — `kr0ki.repo`, `sysml-v2-release`, `sysml-v2-parser`, `eclipse-syson`,
  `kubediagrams` in `elasticdotventures/_b00t_`.
- [x] **D3** resolved — the typed layer lives in `ufo-types`.
- [x] **D6** resolved — [`VOCABULARY.md`](VOCABULARY.md): OMG spec terms verbatim,
  "projection" banned, "digital thread" always qualified.

---

## Box 1 — source front-ends

- [ ] **Commit poll loop** — Flexo (and the OMG pilot) expose **no webhooks**; poll
  `GET /projects/{id}/commits` newest-first (or `…/branches/{b}`) to detect new
  `(projectId, commitId)`, enqueue a render. `Tag` ids are a natural "render releases"
  trigger. `previousCommit` is `null` in Flexo's PSM → no cheap deltas; re-snapshot.
- [ ] **Wire `ModelSnapshot.content_hash` into the render cache key** — `model_cache_key
  = SHA256("kr0ki/v1" ‖ view_kind ‖ notation ‖ ModelSnapshot.content_hash)`
  (PLAN §3). Extends `kr0ki-core::cache::cache_key`.
- [ ] **`page-after` bracket-form fallback** — client uses hyphenated `page-after` /
  `page-size`; some OMG-pilot servers use JSON:API `page[after]`. Make `Page`
  construction configurable per target. (client crate docs flag the swap point.)
- [ ] _(later, named not scoped)_ **Rust-source front-end** — Rust AST → UFO graph.
- [ ] _(later)_ **k8s-source front-end** — manifests / kustomize / Helm → UFO graph via
  the Kubernetes recognizer (box 3).

## Box 2 — canonical UFO-typed semantic graph (in `ufo-types`)

- [ ] **Graph container type** — a typed envelope holding UFO-stereotyped nodes +
  `Vec<OntologicalEdge>` (the v2-correct "SysGraph"; **not** `SchemaVersion`-carrying —
  content-hash + golden-fixture discipline, DESIGN-NOTE §2.7/§2.8). serde JSON,
  ouroboros round-trip test (same bar as `systhread-core`'s `PositionedGraph`).
- [ ] **`ModelSnapshot → UFO graph` builder** — element `@type` → `UfoStereotype` +
  `ElementKind`; KerML relationships → `Relation`; non-KerML edges → `UfoRelation` /
  `Relation::Domain`. Lives in `ufo-types` (or the D3-designated crate), **not** kr0ki.
- [ ] **Provenance population** — one `SourceAnchor` per node/edge:
  `KermlQualifiedName` from `@id`, `Vcs { commit }` from the snapshot, `SysmlFile` /
  `K8sObject` from the source front-end. Deterministic only.

## Box 3 — pattern recognizers

- [ ] **Kubernetes recognizer** — port `philippemerle/KubeDiagrams`'
  `bin/kube-diagrams.yaml` (~51 GVK entries + Gateway API) into a **static Rust rule
  table**, keeping the per-JSONPath distinction KubeDiagrams collapses → the 25
  canonical `UfoRelation` kinds (`OWNER`→`has_part`, `CONTROLLED_BY`→`controls`,
  `SELECTOR`→`selects` map 1:1; the `REFERENCE` bucket splits by JSONPath — table in
  `EVAL-kubediagrams.md` §3). **Do not** port their `exec()` mechanism. Add a `NOTICE`
  (Apache-2.0). Emit `OntologicalEdge`s into box 2.
- [ ] **Recognizer rule-set versioning** — fold a hash of the rule-set version into the
  model cache key so a recognizer change invalidates derived views (PLAN §3).
- [ ] **CRD extension point** — mirror KubeDiagrams' `.kdc` config pattern
  (`PATTERNS-kubernetes.md`).
- [ ] _(later)_ additional recognizers — the pattern is established by the k8s one.

## Box 4 — SysML v2 model constructs (`ufo-types`, mostly done)

- [x] `ElementKind` / `Relation` / `SysmlViewKind` — merged (`#20`).
- [ ] **`ViewDefinition` / `ViewpointDefinition` instances as data** — authored in
  `.sysml` / a datum, resolved at runtime; the data-driven view mechanism, **not** a
  sealed Rust trait (DESIGN-NOTE §2.5).
- [ ] **`ViewUsage.exposedElement` consumption** — take view scoping from the model
  where present rather than re-inventing heuristics (`EVAL-syson.md`).

## Box 5 — renderer adapters (kr0ki)

- [x] `HttpKrokiBackend` — P0.
- [ ] **`iso_ir → Mermaid / D2` adapter (FR1)** — follows
  `b00t-cli/src/dispatch_sysml.rs` conventions (`edge_type: "sequence"` etc.). Consumes
  box-4 output, **not** a raw `ModelSnapshot`.
- [ ] **per-`ViewDefinition` rendering (FR4)** — box-4 constructs + `SysmlViewKind` →
  notation. Blocked on boxes 2+3.
- [ ] **`systhread-core` isometric backend (FR3)** — call its `render.rs`; do not port
  or re-solve the Cassowary/kasuari layout.
- [ ] **`KubeDiagramsBackend` — leaf feature, NOT the pipeline** — sandboxed subprocess
  (rootless, no network, ro FS), k8s YAML bundle → SVG, distinct route
  (`POST /render/k8s`). **Never expose `-c`** (arbitrary Python via `exec()`). Hash the
  normalised `dot_json`, not the PNG. Pin Graphviz. Ship KubeDiagrams as its own
  container / sidecar, not in the Rust image. (`EVAL-kubediagrams.md` §2a/§4)
- [ ] **Wire `vendor/kroki-mcp`** — P0 talks direct HTTP; the MCP hop matters for SVG
  normalisation / inline-embedding (`render.rs` module doc).
- [ ] **PNG / PDF output** — later Kroki capability; `OutputKind` currently `Svg` only.

## Cross-cutting

- [ ] **FR6 — artifact reference resolver** — given a rendered artifact, resolve the
  references it carries (datum ids, `iso_ir` node ids, other kr0ki keys) to URLs, *as
  of the artifact's commit*. May call `ledgrrr`; does not implement graph reasoning.
- [ ] **FR7 — caller auth** — the service will hold ecosystem credentials; no
  unauthenticated render calls (learn from the `xero-mcp-server-b00t` review). Blocks
  the service leaving localhost. `main.rs` currently logs a loud `warn!` about this.
- [ ] **CDN tier (FR5 / D5)** — Cloudflare R2 + Workers in front of `FsCache`; a cache
  hit must never start a renderer (NFR4).
- [ ] **NFR5 — b00t interface** — agent-facing ops through `mcp__b00t-mcp__*` / `b00t`
  CLI, not bespoke HTTP. (A `kr0ki` MCP surface, or extend `_b00t_/kroki.mcp.toml`.)

## Conformance / QA

- [ ] **Phase 2 conformance** — once the FR1 adapter exists, upstream a kr0ki-adapter
  fixture set (`iso_ir` / Mermaid / D2 lowering goldens) into `sysml-v2-parser`'s own
  scorecard so the parser crate owns the shared corpus and kr0ki owns only its lowering
  deltas (`CONFORMANCE.md`).
- [ ] **KubeDiagrams oracle CI job** — diff kr0ki's k8s topology output against
  KubeDiagrams' `dot_json` over its `examples/` corpus (argo / istio / cert-manager /
  kube-prometheus-stack / online-boutique). Topology-recall regression signal.
  (`EVAL-kubediagrams.md` §2c)
- [ ] **Negative / error corpus** — `SysML-v2-Release` has positive fixtures only; port
  selected error cases from the OMG Pilot Implementation's `org.omg.*.xpect.tests/**/*.xt`
  (EPL-2.0). Deferred; noted in `CONFORMANCE.md`.
- [ ] 🚩 **`sysml-v2-parser` 0.55 deep-nesting stack overflow** — a few deeply-nested
  `sysml/src/examples/` models SIGABRT with the default 2 MiB test-thread stack
  (contained in the harness with a 256 MiB worker). Reduce to a minimal standalone
  repro on the crate's own public `parse()` API, then file upstream + `b00t task add`.

## Open decisions (not kr0ki's to make)

- [ ] **D1** — `sysml-derive` extend-vs-wrap-vs-re-export for `UfoStereotype`-tagged
  types ([`ledgrrr#202`](https://github.com/PromptExecution/ledgrrr/issues/202)). Gates
  **only** the authoring / SysML-v2-text-emit path; the read/ingest path is unblocked.
- [ ] **D2** — real (non-dev) dependency on `holon-viz`?
  ([`ledgrrr#203`](https://github.com/PromptExecution/ledgrrr/issues/203)). Only bites
  the `CytoscapeGraph` wrapping question, which this ingestion path does not touch.
- [ ] **D4** — DNS: who provisions `kr0ki.b00t.promptexecution.com` (pingap config in
  `b00t-node.yaml.tpl`)? Infra-owned. Blocks FR7's public surface.
- [ ] **D5** — CDN: Cloudflare R2 + Workers, matching `_b00t_#1069`'s edge pattern?
  Infra-owned. Blocks FR5's cache tier.

## Datums / housekeeping

- [ ] Refresh `_b00t_/kr0ki.repo.toml` — its `🤓` comment still says "five open
  decisions"; D3 and D6 are resolved.
- [ ] Re-run the datum-graph pre-commit hook locally once (needs a warm `b00t-cli`
  build) — the last three datum PRs were admin-merged past a backed-up `cargo check`
  queue after `validate graph references` passed.
