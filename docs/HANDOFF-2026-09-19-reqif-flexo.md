# HANDOFF — ReqIF / Flexo requirements viewpoints

**Date:** 2026-09-19  
**Audience:** lead developer and project manager  
**Implementation branch:** `feature/flexo-reqif-implementation`  
**Checkpoint branch:** `feature/flexo-reqif-requirements` at `c9be32a`

## Executive state

The first runnable, stateless requirements-view slice is implemented in kr0ki.
It accepts a normalized requirements graph, produces one of five typed viewpoints,
and can render that induced graph through the existing D2/Kroki cache. It does **not**
parse ReqIF, persist a Flexo baseline, or add a requirements database. Those remain
adapter/upstream work by design.

The branch stack is intentional:

| Branch | Commit | Purpose |
|---|---:|---|
| `feature/flexo-reqif-requirements` | `c9be32a` | Full checkpoint of the pre-existing local Playb00k, story-agent, catalog, and initial requirements changes. |
| `feature/flexo-reqif-implementation` | `f9039be` | Follow-on requirements hardening; branch from here for further work. |

Both commits are local. No PR, deployment, Flexo instance, StrictDoc sidecar, or
external-repository change has been created from this work.

## What is working

### Normalized requirements model

`crates/kr0ki-core/src/requirements.rs` contains the backend-neutral model:

- immutable `BaselineIdentity`, source `Provenance`, `Requirement`, and `EvidenceRef`;
- the asserted vocabulary: `contains`, `derives`, `refines`, `requires`, `satisfies`,
  `verifies`, `implements`, `traces`, `allocated_to`, and `precedes`;
- asserted, inferred, and proposed relation authority. Non-authoritative edges carry
  confidence, rationale, evidence, and model identity; `promote_relation()` is the
  explicit mutation that makes an edge authoritative;
- graph validation for duplicate identities, unknown endpoints, invalid confidence,
  and requirements whose baseline differs from the graph baseline.

### Five viewpoints

`RequirementGraph::view(&ViewRequest)` returns a typed induced graph, never renderer
source. Available `ViewKind` values are:

1. `decomposition` — contains/derives/refines.
2. `traceability` — satisfies/verifies/implements/traces/allocated-to.
3. `impact` — bounded upstream/downstream/both traversal; reports real cycle edges.
4. `behaviour` — deterministic activity/sequence/state recommendation. Rendering needs
   an explicit `confirmed_behaviour`; a human may choose a different view deliberately.
5. `verification_coverage` — coverage matrix plus unverified requirements and orphan
   evidence.

Authoritative viewpoints exclude inferred/proposed edges until promotion. `review`
scope includes them so a client can present the distinction without treating an
inference as fact. Output is sorted by id before rendering to make equivalent inputs
stable.

### HTTP and rendering boundary

| Endpoint | Contract |
|---|---|
| `POST /requirements/views` | JSON `{ graph, request }` → typed `ViewResult` JSON. No D2 or Kroki call. |
| `POST /render/requirements-view?output=svg\|png` | Same JSON → induced graph → D2 → existing cache-aware renderer. A behaviour request without confirmation returns `409 behaviour_confirmation_required`. |

`crates/kr0ki-core/src/requirements_render.rs` is the only graph-to-D2 adapter. This
keeps ReqIF/Flexo mechanics outside `kr0ki-core` rendering semantics.

## Verification performed

**Updated 2026-09-19 (post-M1, post-PR-#36-review):** the semantic model moved to
`ufo_types::mbse::requirements`; the promotion/impact/behaviour/coverage tests moved
with it and now run in that crate, not this one. The line below originally read
"5 passed" for `cargo test -p kr0ki-core requirements` — that count was the semantic
tests plus the D2 adapter test, all still local to kr0ki-core at the time this doc was
written. Once M1 landed, only the D2 adapter test remained local; this doc's own claim
went stale in the same PR that made it true. Corrected numbers below, plus the
consumer-level contract suite PR #36 review asked for.

```text
(in the ufo-types repo) cargo test mbse::requirements                          # 4 passed (the moved semantic tests; not runnable from kr0ki's workspace — ufo-types is a git dependency, not a workspace member)
cargo test -p kr0ki-core --test requirements_contract                           # 5 passed (kr0ki_core::requirements re-export + requirements_render::to_d2, exercised through kr0ki-core's public API only)
cargo test -p kr0ki-core requirements                                           # 2 passed (requirements_render's own D2-adapter tests, incl. adversarial-title coverage)
cargo test -p kr0ki-server --test http requirements_view_returns_typed_induced_graph_not_renderer_source  # passed
cargo clippy -p kr0ki-core -p kr0ki-server -- -D warnings                       # clean
```

Tests cover promotion isolation, bounded impact/cycles, behaviour recommendation plus
human selection, coverage gaps, deterministic D2 emission (including adversarial ReqIF
titles containing D2 structural characters — PR #36 review), typed HTTP response, and
the confirmation gate. They do not exercise a live Kroki backend, Flexo, ReqIF, or
StrictDoc.

## Deliberate boundaries — do not erode these

- **kr0ki is not the requirements database.** Flexo MMS is the durable RDF/versioned
  model store. kr0ki accepts materialized graphs and makes ephemeral views/artifacts.
- **No handwritten ReqIF XML parser.** **Decided 2026-09-20** (superseding the
  original "Python sidecar/HTTP/MCP boundary" plan below): use
  [`reqrs`](https://crates.io/crates/reqrs) (Apache-2.0, MIT-compatible), a direct
  Rust port of the same mandated StrictDoc `reqif` package, as a normal
  `kr0ki-core` Cargo dependency — no sidecar process, no HTTP/MCP hop for
  parsing itself. Verified hands-on before deciding (see
  [kr0ki#39](https://github.com/PromptExecution/kr0ki/issues/39)): parses a
  real fixture correctly, rejects a structurally-invalid one with a clearer
  error than the Python original gave for the identical case, and round-trips
  byte-identically on consistently-formatted input. Caveats carried forward,
  not yet resolved: `reqrs` is young (created 2026-05-28, pre-1.0, ~1 star,
  single maintainer) and its behavior on `reqif` 0.0.48's confirmed
  no-namespace parsing bug (found in
  [reqif-opa-mcp#25](https://github.com/PromptExecution/reqif-opa-mcp/pull/25))
  hasn't been checked yet — verify before the adapter depends on it for
  anything namespace-optional. Original plan, kept for context: use the
  Apache-2.0 StrictDoc `reqif` package for ReqIF/ReqIFz parsing, unparsing,
  basic validation, and optional OMG schema validation via a small
  sidecar/HTTP or MCP boundary, since it's a Python library. `reqrs` makes
  that boundary unnecessary for parsing; Rust still owns the normalized model
  and viewpoints either way.
- **`ufo-types` owns shared semantics.** The current module is a kr0ki integration
  seam, not permission to fork the canonical semantic model. Move it upstream as
  `ufo_types::mbse::requirements` before another consumer depends on this local path,
  then re-export/consume the upstream release here.
- **`reqif-opa-mcp` owns its existing artifact → policy → ReqIF workflow.** Refactor it
  by consuming the shared types/contracts; do not duplicate its OPA policy engine in
  kr0ki.
- **Use `View`/`Viewpoint`, not “projection”, in new user-facing documentation.**
  `docs/VOCABULARY.md` remains authoritative.

## Lead developer next actions

1. **Upstream semantic model (first).** Open a focused `ufo-types` change adding
   `mbse::requirements`; move the structures and tests from `kr0ki-core` with no
   renderer dependency. Cut/tag a release, update kr0ki, and delete the local mirror.
2. **ReqIF adapter contract.** In/alongside `reqif-opa-mcp`, expose import, validation,
   progress, normalized-graph, and deterministic-export contracts over HTTP or MCP.
   Use StrictDoc parser/unparser; support ReqIFz attachments and invoke schema
   validation. Preserve original artifact digest and export digest in `BaselineIdentity`.
3. **Flexo adapter.** Add a thin, upstream-first Flexo extension/service that stores
   validated input plus deterministic exported baseline per Flexo commit and
   materializes the normalized graph. Keep commit identity as a required request field.
4. **Bind Playb00k only after the adapter exists.** The Vue workspace should call the
   stable contracts: import/validate, baseline list/select, graph/view request, then
   cached-artifact URL. Do not make browser state authoritative.
5. **MCP parity.** Add requirements view/render tools to the manifest-driven MCP bridge
   only after their request schema is accepted; update the existing test that currently
   asserts twelve tools.

## Project-manager decisions and delivery gates

| Decision / gate | Owner | Needed before | Status |
|---|---|---|---|
| Confirm `ufo-types` accepts `mbse::requirements` and name/version strategy | PM + ufo-types maintainer | shared-model work | open |
| Choose sidecar deployment and trust boundary for Python StrictDoc | PM + platform | ReqIF import MVP | open |
| Confirm Flexo extension API/storage path and upstream contribution owner | PM + Flexo maintainer | durable baselines | open |
| Define ReqIF fixture corpus, max file size, attachment retention, and progress UX | PM + QA | import acceptance | open |
| Define who may promote inferred/proposed relations and required audit identity | PM + governance | write/promotion UX | open |
| Approve API contracts before Vue/Flexo Web Modeler work | PM + lead | client implementation | open |

Suggested milestones:

1. **M1 — shared types:** upstream release + kr0ki consumes it; contract tests green.
2. **M2 — validated interchange:** ReqIF/ReqIFz import/export fixture suite and
   deterministic baseline digest.
3. **M3 — Flexo baseline:** commit ↔ graph ↔ exported ReqIF equivalence test.
4. **M4 — client:** Playb00k import/browse/five-view flow and cached artifact opening.
5. **M5 — interoperability:** Flexo Web Modeler client uses the same requests; only
   then assess the optional GraalVM LLVM/Rust spike.

## Current risk register

| Risk | Mitigation |
|---|---|
| Shared types fork between kr0ki and `ufo-types` | Treat the local module as temporary; upstream before adapter work expands. |
| ReqIF parser is Python while core is Rust | Isolate it behind HTTP/MCP; do not introduce in-process runtime coupling. |
| Flexo API/storage gap | Upstream-first spike with an explicit failing contract; fork only after a demonstrated gap. |
| Inferred edges accidentally influence authoritative artifacts | Default `authoritative` scope and promotion test are already in place; retain this invariant in every adapter/UI. |
| Scope expansion into a kr0ki store | Do not add persistence fields/routes here; materialization belongs in Flexo adapter. |

<!-- b00t:map v1
summary: ReqIF and Flexo requirements viewpoints handoff, branch state, ownership, and delivery gates
tags: kr0ki, reqif, flexo, requirements, handoff, ufo-types, strictdoc, playb00k
tier: frontier
cmds: cargo test -p kr0ki-core requirements, cargo clippy -p kr0ki-core -p kr0ki-server -- -D warnings
complexity: 8
-->
