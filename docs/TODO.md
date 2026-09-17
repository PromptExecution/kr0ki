# TODO — kr0ki working list

The single actionable list. Requirement definitions live in
[`PRD-KR0KI-001-foundational.md`](PRD-KR0KI-001-foundational.md); the ingestion design
in [`PLAN-KR0KI-002.md`](PLAN-KR0KI-002.md); the typed-layer shape in
[`DESIGN-NOTE-typed-model-layer.md`](DESIGN-NOTE-typed-model-layer.md). This file just
tracks *what is left to do*, ordered by the five-box pipeline.

_Last updated: 2026-09-15; corrected 2026-09-17 — boxes 2/3 and the ledgrrr
cross-cutting item were already shipped (kr0ki#12/#13) but left unchecked._

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
- [x] **FR7 minimal — caller auth** — `KR0KI_AUTH_TOKEN` env var gates all routes except `/health` with `Authorization: Bearer <token>`. axum middleware in `kr0ki-server/src/app.rs`. HTTP tests verify 401/200. Suitable for localhost/trusted-proxy only until D4/D5 land.
- [x] **PNG output** — `OutputKind::Png` added; `?output=png` query param on `/render/{format}` and `/cache/{key}`. Live-verified against kroki.io for GraphViz. `d2`/`nomnoml`/`wavedrom`'s Kroki-side 400 ("Unsupported output format") is now covered by the `kr0ki-core::flatten` SVG-to-PNG fallback below — PNG is universal across all 8 formats. Byte-identical cache hit verified in `tests/live_png.rs`.
- [x] **SVG-to-PNG flatten fallback** — some formats Kroki only draws as SVG (`nomnoml`, `d2`, `wavedrom`, confirmed live). `RenderService` now tries native PNG first and only on rejection renders SVG and rasterizes it locally (`kr0ki-core::flatten`, `resvg`/`usvg`/`tiny-skia` — pure Rust, no headless-Chromium companion, NFR3-compliant). A genuinely bad source still fails identically on the SVG attempt, so this can't mask a real syntax error. Flattened output is cached under the normal PNG key. 5 unit tests plus live-verified end-to-end against both the redeployed k0s pod and `kroki.io` directly.
- [x] **Docgen / mdb00k** — `kr0ki-core/src/docgen/` harvests own Rust source via `syn`, serves `/docs` (HTML), `/docs/api.json`, `/docs/api.tomllm` (b00t format), `/docs/api.rustdoc`. Pattern derived from `b00t-cli/src/commands/docgen.rs` — clean-room Rust implementation matching b00t output conventions. 50 symbols harvested from workspace. Server binds `0.0.0.0:8787` by default; logs hostname + docs URL on startup.
- [x] **AGENTS.md + HANDOFF.pm** — ecosystem orientation and comprehensive handoff written.
- [x] **`ModelSnapshot.content_hash` in the render cache key** — `model_cache_key(view_kind, notation, content_hash) = SHA256("kr0ki/v1" ‖ 0x1f ‖ view_kind ‖ 0x1f ‖ notation ‖ 0x1f ‖ content_hash)` (PLAN §3) in `kr0ki-core::cache`; `RenderService::render_model()` uses it, so the SysML path participates in cache identity. 4 tests (determinism, per-field variance, differs from text key, caches on hash).
- [x] **Live playb00k render harness** — `/docs/examples/kr0ki-render-flow.svg` renders a hand-authored D2 fixture describing the Rust flow (not derived from the Rust AST — see `PLAN-KR0KI-003.md` §1) through the same `RenderService` as callers; `/docs` includes it with KerML/SysML v2 source fixtures. `just playbook-e2e` verifies the deployed page, SVG, artifact identity, and cache hit. `pod-up` recreates its standalone Pod after image import so this is hot-reloadable; both Kroki and kr0ki have `/health` readiness probes, so the wait gates on a usable renderer rather than merely started processes.
- [x] **Vue/Vite playb00k harness** — `/playbook/` serves a sidebar-driven Vue UI with one executable fixture per supported format; `kr0ki-core::examples::ALL` is the catalog consumed by Rust tests, live `/api/examples`, and static mdb00k `playbook/api/examples.json`. `playbook/src/components/RendererPanel.story.vue` is the Histoire visual-regression story.

---

## Box 1 — source front-ends

- [ ] **Commit poll loop** — Flexo (and the OMG pilot) expose **no webhooks**; poll
  `GET /projects/{id}/commits` newest-first (or `…/branches/{b}`) to detect new
  `(projectId, commitId)`, enqueue a render. `Tag` ids are a natural "render releases"
  trigger. `previousCommit` is `null` in Flexo's PSM → no cheap deltas; re-snapshot.
- [x] **Wire `ModelSnapshot.content_hash` into the render cache key** — `model_cache_key
  = SHA256("kr0ki/v1" ‖ view_kind ‖ notation ‖ ModelSnapshot.content_hash)`
  (PLAN §3). Implemented as `kr0ki-core::cache::model_cache_key` +
  `RenderService::render_model`; 4 tests. See Done below.
- [x] **`page-after` bracket-form fallback** — added `PageParamStyle` (`Hyphenated`
  default / `JsonApiBracket`) to `kr0ki-sysmlv2-client`; set per-client via
  `SysmlV2Client::with_page_param_style`. Covers both the request-side query keys
  (`elements()`) and the response-side `Link`-header cursor extraction
  (`paging::derive_next_after`). Unit tests in `paging.rs` plus a wiremock
  integration test (`json_api_bracket_style_sends_bracket_form_params_and_follows_link`)
  verifying the bracket form round-trips through `reqwest`.
- [ ] **Rust-source front-end** — Rust AST → UFO graph → recognizer → SysML constructs
  → diagram-as-code notation → render. Scoped in
  [`PLAN-KR0KI-003.md`](PLAN-KR0KI-003-rust-source-frontend.md). **Not** the docgen
  doc-symbol harvest and **not** the Histoire/Vue playbook (both stay documentation/
  testing tools, out of this pipeline — PLAN-003 §1 corrects a prior conflation of the
  two with this item).
- [ ] _(later)_ **k8s-source front-end** — manifests / kustomize / Helm → UFO graph via
  the Kubernetes recognizer (box 3).

## Box 2 — canonical UFO-typed semantic graph (in `ufo-types`)

- [x] **Graph container type** — `SysGraph` (`nodes: Vec<OntologicalNode>`, `edges:
  Vec<OntologicalEdge>`), serde JSON, round-trip tested, no `SchemaVersion` field
  (DESIGN-NOTE §2.6/§2.8). `ufo-types` PR #27 (`feat/sysgraph-box2-container`),
  open, not yet merged/tagged. Lands in `ufo-types::sysgraph`.
- [x] **`ModelSnapshot → UFO graph` builder** — `kr0ki-core/src/ufo_graph.rs`
  (box 2 of `PLAN-KR0KI-002`). Raw KerML relationship `@type` → `UfoRelation` via a
  direct table lookup (`FeatureMembership`→`HasPart`, `Specialization`→`Specializes`,
  etc. — see the module's own mapping table). Already shipped; this checkbox was
  stale.
- [ ] **Provenance population** — the Kubernetes arm already does this
  (`k8s_recognizer.rs` pushes `SourceAnchor::K8sObject` on every edge it builds).
  **The SysML-v2 arm (`ufo_graph.rs`) does not yet** — no `SourceAnchor` is attached
  to the edges it produces. Needs `KermlQualifiedName` from the element's `@id` and
  `Vcs { commit }` from the `ModelSnapshot`. Genuinely still open.

## Box 3 — pattern recognizers

- [x] **Kubernetes recognizer** — `crates/kr0ki-core/src/k8s_recognizer.rs` (kr0ki#12,
  955 lines). Ports `vendor/kubediagrams/bin/kube-diagrams.yaml`'s GVK→relationship
  catalogue into a static Rust rule table (`builtin_rules`), keeping the per-JSONPath
  distinction, emitting `OntologicalEdge`s with `SourceAnchor::K8sObject` provenance.
  Differentially validated against a real KubeDiagrams oracle
  (`tests/kubediagrams_oracle.rs`). Coverage is an intentional subset — see the
  module's own "what's ported vs. deliberately deferred" doc comment (admission
  webhooks, NetworkPolicy rules, Endpoints/EndpointSlice targetRef, Gateway API still
  open, tracked as a kr0ki#12 follow-up, not silently missing). This checkbox and the
  two below were stale — the work already shipped.
- [x] **Recognizer rule-set versioning** — `KubernetesRecognizer::rule_set_version()`
  hashes the active rule set (built-in + extensions); intended to fold into
  `cache::model_cache_key`'s `rule_set_version` field.
- [x] **CRD extension point** — `KubernetesRecognizer::with_rule`/`with_rules` append
  caller-supplied `SimpleFieldRule`s on top of `builtin_rules`.
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
- [ ] ◑ **`KubeDiagramsBackend` — leaf feature, NOT the pipeline** — substantially
  shipped, and (as of mcp-http-parity) available both ways: as an MCP tool and as
  the originally-envisioned native HTTP route (`POST /render/kubediagram`) —
  both paths now exist side by side rather than one having displaced the other
  (see NFR5 below). What's already there: `containers/kr0ki-mcp/http_worker.py`
  spawns `kube-diagrams` as a subprocess with **no `-c`** ever passed
  (arbitrary-Python `exec()` avoided by construction, not by filtering), a 1 MiB
  manifest cap, and a 60s timeout; `kr0ki-server`'s `POST /render/kubediagram`
  route and `bridge.py`'s `render_kubernetes_manifest` MCP tool (dispatched
  generically off the `GET /mcp/tools` manifest, not a hardcoded branch) both
  proxy to it over HTTP. The worker runs as `USER 65532:65532`
  (rootless), and `deploy/kr0ki-local.pod.yaml`'s `kr0ki-mcp` container sets
  `allowPrivilegeEscalation: false`, drops all capabilities, and
  `readOnlyRootFilesystem: true`. Pinned via the container's own `pip install` version
  pins, not a floating `latest`. **Still open:** (1) no content-addressed caching —
  every call re-runs `kube-diagrams` from scratch instead of hashing the normalised
  `dot_json` and reusing `FsCache` the way every other format does; (2) no explicit
  "no network" isolation declared (no `NetworkPolicy` in `deploy/`); (3) `-o` writes
  to a `tempfile.TemporaryDirectory()`, which is a fresh, not-attacker-writable path
  each call, but is not itself sandboxed against the rest of the container's `ro` FS
  beyond what `readOnlyRootFilesystem` + the `emptyDir` `/tmp` mount already provide.
  (`EVAL-kubediagrams.md` §2a/§4)
- [ ] **Wire `vendor/kroki-mcp`** — P0 talks direct HTTP; the MCP hop matters for SVG
  normalisation / inline-embedding (`render.rs` module doc).
- [x] **Playbook: general-purpose custom-diagram mode (FR2 web-ux gap)** —
  `RendererPanel.vue`'s fixture catalog already covers all 8 `DiagramFormat` slugs
  1:1 (no missing-format gap), but its source panel only implicitly allowed editing
  over a fixture. Added explicit "Reset to example" / "Start blank" buttons and a
  file-upload input (`FileReader` → textarea) plus copy changes in `App.vue` making
  clear the panel renders any hand-authored source, not just the catalog. No API/MCP
  change needed (`/formats`, `/render/:format`, `mcp__kr0ki-mcp__render_diagram` /
  `list_formats` already accept arbitrary source). Verified: `vite build`, and
  `scripts/playbook-e2e.sh` now asserts the built bundle ships the new controls,
  run live against a local `kr0ki-server` serving the built `playbook/dist`. See
  `PLAN-KR0KI-003.md` §6.
- [x] **Playbook: gallery view with a post/test mechanism** — new `Gallery.vue`
  (default landing view, toggled against the existing per-format editor via a
  `Gallery`/`Editor` tab in `App.vue`) shows every catalog example as a card and
  lets a caller "Test" one or "Test all" — POSTing every declared output for every
  example against a configurable renderer URL, the same contract
  `every_playbook_fixture_renders_and_caches` checks, now runnable from a browser
  with per-card pass/fail status and artifact thumbnails. "Edit" on a card jumps to
  the existing detail editor for that example. `Gallery.story.vue` added for
  Histoire visual regression, matching `RendererPanel.story.vue`'s pattern.
- [x] **Expand `DiagramFormat` from 8 to 23 — NFR3's companion assumption was wrong
  for the `blockdiag` family** — live-tested every other Kroki-advertised format
  (`GET /health` on the running Kroki lists 30) against the deployed, companion-free
  container. Confirmed genuinely needs a companion (`503`): `mermaid`, `bpmn`,
  `excalidraw`, `diagramsnet`. Confirmed works with zero extra infra (`200`, real
  SVG): `vega`, `blockdiag`, `actdiag`, `seqdiag`, `nwdiag`, `packetdiag`,
  `rackdiag`, `erd`, `umlet`, `pikchr`, `goat`, `bytefield`, `dbml`, `tikz`,
  `svgbob` — added all 15, one fixture each, `outputs: ["svg","png"]` uniformly
  (the flatten fallback covers whichever don't have native Kroki PNG). PRD-KR0KI-001
  NFR3 corrected to match.
- [x] **`wireviz`/`structurizr`/`symbolator` — confirmed companion-free with real
  fixtures** — the earlier 400/500 was bad smoke-test syntax, not a companion
  requirement. `wireviz` (a two-connector YAML wiring harness) and `structurizr` (a
  system-context + container C4 workspace) worked on the first real fixture.
  `symbolator` needed two rounds: `std_logic_vector(N downto 0)` bus ports and a
  `format`-named signal produced a silent `200` with a **0-byte body** — no error,
  just nothing — until swapped for a canonical single-bit-`STD_LOGIC`-port `Port
  (...)` block (Symbolator's own upstream example shape), which rendered a real SVG.
  `DiagramFormat::ALL` now 26. Also caught a false negative while testing: BusyBox
  `wget --post-file=/dev/stdin` on a piped, non-seekable stdin doesn't send
  `Content-Length` and got a `500` from `symbolator` that a proper client (Python
  `urllib`, which buffers first) didn't — a reminder that a single failing client
  isn't proof a format needs a companion; re-test with a client that sends a normal
  request before concluding `503`-equivalent.
- [x] **TikZ fixture richened** — `tikz-line` replaced with a 4-node, 3-edge
  positioned pipeline (`client -> service -> {cache, backend}`) instead of one line
  segment.
- [x] **`DiagramFormat`: enum+match → macro-generated data table** — adding one
  format previously touched 4 places (variant, `kroki_slug()` match arm, `ALL`
  entry, exhaustiveness-test arm). A `diagram_formats!` macro now generates the
  named consts and `ALL` from one list — one place to edit, and a format can't be
  declared and forgotten from `ALL`. The inner `&'static str` stays private —
  verified with a standalone `rustc` compile-fail check that external code can't
  construct an arbitrary `DiagramFormat("anything")`, only via a declared const or
  `FromStr`. That privacy, not the closed-enum shape, is what keeps this a
  whitelist rather than an open pass-through.
- [x] **`just dev` — fast local dev loop, no k0s** — `just dev-kroki-up` runs our
  own pinned `kroki-compat` image via plain `podman run` (not k0s) on
  `127.0.0.1:8010`; `just dev` runs `kr0ki-server` via `cargo run` against it on
  `127.0.0.1:8788`. Skips the podman-build → k0s-import → pod-recreate cycle
  entirely for format/fixture iteration. Needed explicit `--memory=2g
  --memory-swap=2g --cpus=1` — b00t's OCI limits hook (TODO gap, b00t platform
  backlog #4) rejects a bare `podman run` with no resource budget; matches the pod
  manifest's own 2Gi/1 CPU. Live-verified: `playbook-e2e.sh` and `test-playbook`
  both pass against it exactly as they do against the real k0s pod. Documented as
  iteration-only in README — can drift from the real deployment, always
  re-verify with `just pod-up` before calling something done.
- [ ] **PDF output** — later Kroki capability; not yet available.

## Cross-cutting

- [ ] **FR6 — artifact reference resolver** — given a rendered artifact, resolve the
  references it carries (datum ids, `iso_ir` node ids, other kr0ki keys) to URLs, *as
  of the artifact's commit*. May call `ledgrrr`; does not implement graph reasoning.
- [x] **FR7 minimal** — `KR0KI_AUTH_TOKEN` bearer auth implemented. See Done above.
  Full OAuth/JWT rate-limited auth deferred until D4/D5 land.
- [ ] **CDN tier (FR5 / D5)** — Cloudflare R2 + Workers in front of `FsCache`; a cache
  hit must never start a renderer (NFR4).
- [x] **NFR5 — b00t interface** — `containers/kr0ki-mcp/bridge.py` is exactly the
  named option: a `kr0ki` MCP surface (`render_diagram`, `list_formats`,
  `render_kubernetes_manifest`), stdio JSON-RPC, no bespoke HTTP exposed to the
  agent. As of mcp-http-parity, `bridge.py` is a generic manifest-driven
  dispatcher — it fetches `GET /mcp/tools` once and dispatches every
  `tools/call` from that manifest instead of hand-coded per-tool branches; the
  three tool names and capabilities themselves are unchanged. Runs rootless
  (`USER 65532:65532`). Live in this session as `mcp__kr0ki-mcp__*`.
  `_b00t_/kroki.mcp.toml` extension not needed given this already exists.
- [x] **`ledgrrr` / `holon-viz` client seam + E2E** — `crates/kr0ki-core/src/b00t_graph.rs`
  (kr0ki#13): Turtle → `holon_viz::type_graph::TypeRelationshipGraph` →
  `CytoscapeGraph` → `D2Emitter` → the existing Kroki render pipeline. This checkbox
  was stale — the work already shipped.

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

## Gaps discovered during 2026-09-15 session

These were not in this file at the start of the session. They should be addressed or
tracked as they affect production readiness.

| # | Gap | Severity | Mitigation |
|---|---|---|---|
| 1 | Docgen workspace root detection uses string-search `[workspace]` in `Cargo.toml` — brittle if vendored | Low | Switch to `cargo metadata --format-version=1` if robustness needed |
| 2 | Docgen does not harvest `impl` blocks, associated items, or private items | Low | Extend `syn::visit` if needed; currently public API only |
| 3 | ~~`/docs` HTML references `templates/b00t-stack-orchestration.d2` by relative filesystem path — breaks if CWD ≠ repo root~~ | ~~Low~~ | **Fixed** — `docs.rs` now embeds it via `include_str!`, same pattern as the adjacent `RUST_FLOW_D2` const; no runtime file read left on this path |
| 4 | ~~No CI coverage specifically targets the docgen endpoints~~ | ~~Medium~~ | **Already covered** — `crates/kr0ki-server/tests/http.rs` has `docs_json_returns_symbol_array`, `docs_rustdoc_has_source_marker`, `docs_html_returns_valid_page`, `docs_tomllm_has_boilerplate`, `docs_rust_flow_uses_the_render_service_cache`; `.github/workflows/ci.yml`'s `check` job runs `cargo test --workspace` on every push/PR |
| 5 | b00t MCP bridge down (`bad handshake: expected ident at line 1 column 2`) | External | Use direct HTTP or `b00t-cli` instead; not a kr0ki bug |
| 6 | D2 template edge labels with `{slug}` syntax break D2 parser | Fixed | Replaced with `slash slug slash` syntax |
| 7 | ~~`ModelSnapshot.content_hash` not yet wired into cache key~~ | ~~Medium~~ | **Done** — `model_cache_key` + `render_model` in `kr0ki-core`; see Done above |
| 8 | Container build did not copy playb00k fixtures required by `include_str!` | Medium | **Fixed** — `containers/kr0ki-server/Containerfile` copies `templates/`; caught by `just pod-up` before deployment |
| 9 | `ledgrrr` has no kr0ki call path despite the intended holon-viz integration | Medium | Track the D2 emitter/client seam above; do not conflate Mermaid with kr0ki's standalone supported formats |
| 10 | Kroki's upstream PlantUML native executable requires x86-64-v3, while sm3lly k0s exposes x86-64-v2 | High | Use the pinned, checksum-verified JVM PlantUML overlay in `containers/kroki-compat/`; prove every catalog fixture against local k0s before promotion |

## Open decisions (not kr0ki's to make)

- [x] **D1** — `sysml-derive` extend-vs-wrap-vs-re-export for `UfoStereotype`-tagged
  types ([`ledgrrr#202`](https://github.com/PromptExecution/ledgrrr/issues/202)).
  **RESOLVED 2026-09-10** — separation of concerns: `sysml-derive` stays structure-only,
  `ufo_types::mbse::MbseExport` (already shipped) emits stereotype metadata, consumers
  compose both. See `docs/EVAL-sysml-derive.md`. The authoring / SysML-v2-text-emit path
  is now unblocked, same as the read/ingest path.
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

## b00t platform backlog (external)

- [ ] **Credential-provenance / CVE-review hook** — detect when an agent encounters a
  credential-bearing Git remote, determine its provenance with `git config --show-origin`
  without echoing the secret, and classify it as credential-helper behavior or a
  suspicious repository-local override. Route only validated findings to a CVE-style
  b00t review record; its accepted event credits the reporting model with `:cake:` / 🍰
  through a ledgrrr hook. The reward must follow reviewer validation, not a lexical
  credential match. Tracked in b00t task #3.
- [ ] **Podman-kube memory-hook compatibility** — b00t's OCI limits hook must exempt
  only the unlimit-able Podman infra process (`/catatonit -P`), while preserving limits
  for every workload. The root-owned hook requires an operator-applied semantic patch;
  kr0ki's manifests already declare per-workload limits and use b00t's auditable
  `b00t.unlimited=ack` annotation. Tracked in b00t task #4.
