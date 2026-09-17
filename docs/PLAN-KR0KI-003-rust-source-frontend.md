# PLAN-KR0KI-003 — the Rust-source front-end (box 1, Rust arm)

**Parent:** [`PRD-KR0KI-001-foundational.md`](PRD-KR0KI-001-foundational.md) §1.1 (the
`Rust types → iso_ir` prototype already named there). **Sibling:**
[`PLAN-KR0KI-002.md`](PLAN-KR0KI-002.md) (the five-box pipeline this plan reuses
verbatim; §2 there names the Rust-source front-end as "out of scope for this PR").
**Scopes:** `docs/TODO.md`'s box-1 item *"(later, named not scoped) Rust-source
front-end — Rust AST → UFO graph"*.
**Owner:** PromptExecution (@elasticdotventures). **Created:** 2026-09-16.

**Status:** proposed, unimplemented. Written to correct a false assumption made while
landing the docgen/playbook work (`#14`, `#15`, `#16`) — see §1 — before any further
code is written against it.

---

## 0. One paragraph

Turning Rust source (or Kubernetes IaC) into a rendered diagram is **not** a
docgen-then-eyeball step. It is the same five-box pipeline `PLAN-KR0KI-002` already
defined for the SysML-v2 arm, with Rust source as a second box-1 front-end: an
AST-driven harvester lowers Rust code into the canonical UFO-typed semantic graph, a
pattern recognizer lifts that graph into SysML v2 constructs, and only *then* does a
kr0ki renderer adapter emit diagram-as-code text (D2/Mermaid/etc.) for Kroki to
rasterize. Nothing upstream of that last step is allowed to be hand-authored diagram
syntax, and nothing in this pipeline is a documentation format or a visual test
harness — those are legitimate, separate tools that must not be mistaken for it.

---

## 1. The false assumption (why this doc exists)

The docgen/playbook work shipped three real, useful things that do **not**, individually
or together, add up to "Rust code → diagram" generation:

- `kr0ki-core/src/docgen/harvest.rs` — a `syn::visit::Visit`-based `SymbolVisitor` that
  walks the workspace and emits `Symbol { name, qualified_name, kind, signature,
  docstring, … }`. Real AST introspection, but every consumer in `format.rs`
  (`format_json`, `format_tomllm`, `format_rustdoc`, `format_html_with_live_flow`) turns
  that symbol list into a **documentation format** — a symbol index, not a diagram.
- The "Rendered Rust flow" diagram shown at `/docs` and bundled by
  `write_static_mdb00k` (`docgen/mod.rs:124-137`) is **hand-authored** D2 text
  (`templates/b00t-stack-orchestration.d2`, pulled in via `include_str!`/file read) that
  happens to describe the `RenderService` pipeline in prose-as-D2. It is never derived
  from the `Symbol` list. `TODO.md`'s "Live playb00k render harness" entry calls this the
  "executable D2 Rust-flow fixture" — true in the sense that it renders and is tested,
  false if read as "generated from the Rust flow." It is not; it is written by hand and
  happens to render.
- `playbook/` (Histoire + Vue) renders a static example catalog
  (`kr0ki_core::examples::ALL`) through the real `RenderService` for **visual-regression
  testing** of the renderer itself. It browses and diffs renders a human or a fixture
  wrote; it does not generate diagram sources from anything.

None of the three is wrong to have. The mistake was treating their combination as if it
satisfied "diagrams as code, generated from the code" — it doesn't, because nothing
connects `Symbol` (or any AST-derived structure) to diagram syntax. This plan is that
missing connection, and it must not live inside `docgen/` or `playbook/`.

## 2. The pipeline, Rust arm

Reusing `PLAN-KR0KI-002` §2's five boxes without modification to boxes 2/4/5:

```
┌─ 1 (Rust arm) ───────────────────┐
│  syn AST over the workspace      │   generalize harvest.rs's SymbolVisitor from a
│                                   │   doc-symbol collector into a *relationship*
│                                   │   collector: module containment, struct/enum
│                                   │   field types, fn call graph, trait impls —
│                                   │   emitted as ufo_types::iso_ir::{Node, Edge},
│                                   │   NOT as kr0ki's own Symbol type and NOT as
│                                   │   SysML constructs directly (PRD §1.1's cut:
│                                   │   kr0ki lowers to iso_ir, nothing more model-ish).
└────────────┬──────────────────────┘
             ▼
┌─ 2 ──────────────────────────────┐   unchanged — ufo-types' responsibility.
│  canonical UFO-typed semantic    │   iso_ir::{Node, Edge} → UfoStereotype /
│  graph                           │   UfoRelation / OntologicalEdge builder, same
│                                   │   builder the SysML-v2 arm feeds (PLAN-002 §2.2).
└────────────┬──────────────────────┘
             ▼
┌─ 3 (new recognizer) ─────────────┐
│  Rust-code pattern recognizer    │   crate → Package; struct/enum/trait →
│                                   │   PartDefinition/InterfaceDefinition; module
│                                   │   containment → FeatureMembership; fn call edges
│                                   │   → Dependency; trait impl → Satisfy/Specialization
│                                   │   (exact mapping table TBD — same shape as the
│                                   │   Kubernetes recognizer, PLAN-002 §2.1, and
│                                   │   PATTERNS-kubernetes.md; needs its own
│                                   │   PATTERNS-rust-source.md).
└────────────┬──────────────────────┘
             ▼
┌─ 4 ──────────────────────────────┐   unchanged — ufo_types::sysml_model, merged.
└────────────┬──────────────────────┘
             ▼
┌─ 5 ──────────────────────────────┐   unchanged — kr0ki's existing DiagramFormat /
│  kr0ki renderer adapters         │   RenderBackend / Kroki HTTP call. This is where
│                                   │   "diagram-as-code" (D2/Mermaid/etc. text) is
│                                   │   actually emitted — one hop before rasterization,
│                                   │   never earlier.
└───────────────────────────────────┘
```

**Auto-reconciliation:** because box 1→2→3→4 is a pure function of the source tree (no
hand authorship anywhere in the chain), re-running it on every commit regenerates
diagram-as-code that is always in sync with the actual code — this is the "procedural
generation from the IaC/code itself" property. It composes with the existing
content-hash cache discipline: hash the harvested `iso_ir` graph the same way
`ModelSnapshot.content_hash` is computed (`kr0ki-sysmlv2-client::hash`), fold it into a
`model_cache_key` the same way `RenderService::render_model` already does for the
SysML-v2 arm (`PLAN-KR0KI-002` §3) — a source-tree hash that hasn't changed renders from
cache; a changed one regenerates automatically. No separate "keep the diagram in sync"
step is needed or should be built.

## 3. What's reusable vs. new

| Piece | Status | Note |
|---|---|---|
| `syn::visit::Visit`-based AST walker | reusable, needs generalizing | `docgen/harvest.rs`'s `SymbolVisitor` only records doc symbols; it needs a sibling visitor (or an extended one) that also records structural edges (calls, field types, impls, containment) as `iso_ir::Edge`, not `Symbol`. Keep `docgen` unchanged — this is a new module, e.g. `kr0ki-core/src/astgraph/` or similar, not a `docgen` addition. |
| `Rust types → iso_ir` prototype | exists upstream, needs evaluation | `b00t-cli/src/dispatch_sysml.rs` (PRD §1.1) is named as already doing `Rust type → iso_ir → SysML v2 / Mermaid / Rhai`. Before writing a second AST walker, evaluate whether this prototype is directly reusable (crate dep) or whether kr0ki's `syn`-based harvester diverges enough (different goal: doc symbols vs. structural graph) to justify a clean-room implementation, same posture as `docgen/mod.rs`'s existing "clean-room, not `codebase-memory-mcp`" note. |
| UFO graph builder (box 2) | reusable, unmodified | `ufo-types`' responsibility; the Rust arm is just another `iso_ir` producer feeding the same builder as the SysML-v2 arm. |
| Kubernetes recognizer (box 3) | precedent, not reusable code | Same *shape* (a static Rust rule table, PLAN-002 §2.1) but a different domain — a Rust-code recognizer is new work, needs its own `PATTERNS-rust-source.md` mapping table before implementation, mirroring `PATTERNS-kubernetes.md`. |
| `RenderBackend` / `DiagramFormat` / Kroki HTTP (box 5) | reusable, unmodified | Already shipped (P0). The Rust arm's box-4 output is just another input to the existing renderer adapters. |
| `docgen/` (doc-symbol harvest + formats) | out of scope, unchanged | Stays a documentation tool. Do not extend it to emit diagrams — that conflation is exactly §1's mistake. |
| `playbook/` (Histoire + Vue) | out of scope, unchanged | Stays a visual-regression harness for the renderer. It may eventually *display* diagrams produced by this pipeline as one more fixture in its catalog, but it is a consumer, never the generator. |

## 4. Open decisions

- **D7 — RESOLVED 2026-09-17.** The Rust-code recognizer (box 3) lives in **kr0ki**
  (`crates/kr0ki-core`), not `ufo-types`. Not by re-deriving the D3 reasoning from
  scratch — by observing what actually shipped once box 3 existed for a *second* arm:
  the Kubernetes recognizer (`k8s_recognizer.rs`) and even the SysML-v2 arm's box-2
  builder (`ufo_graph.rs`, despite this plan's own §3 table originally guessing it'd
  be "`ufo-types`' responsibility") both live in kr0ki-core. `ufo-types` hosts the
  *vocabulary* (`UfoStereotype`, `UfoRelation`, `OntologicalEdge`, `Relation`,
  `SysGraph`); kr0ki hosts every *producer* that walks a concrete source and emits
  that vocabulary. The Rust recognizer is a producer, so it follows the same
  precedent: a new `crates/kr0ki-core/src/rust_recognizer.rs`, sibling to
  `k8s_recognizer.rs`. `docs/PATTERNS-rust-source.md` (new) records the mapping table
  this decision depended on having somewhere to live.
- **Reuse vs. clean-room for box 1 — RESOLVED 2026-09-17, clean-room (generalize
  `harvest.rs`, don't adopt `dispatch_sysml.rs`).** Spike: read
  `b00t-cli/src/dispatch_sysml.rs` in full. Its `dispatch_chain_iso_ir()` does **not**
  do AST analysis at all — it builds `iso_ir::{Node, Edge}` from
  `crate::dispatch::default_dispatch_chain()`, an already-materialized `Vec<dyn
  DispatchMode>` of hand-registered trait objects, using each mode's `.name()`. It is
  a bespoke exporter of one specific runtime value collection that b00t-cli's own
  `dispatch` module maintains by hand — not a generic "parse arbitrary Rust source"
  walker, and it has no way to become one (there is no AST in this path to
  generalize). The PRD's "`Rust types → iso_ir` prototype already named" framing
  overstated how reusable this is: it's prior art for *shape* (`iso_ir::{Node, Edge}`
  is the right target type, confirmed) and for the sibling-export pattern (one
  `_to_mermaid()`/`_to_sysml_v2()`/`_to_rhai()` per source), not for *how to get
  there* from raw source text. The thing that actually is a `syn`-based AST walker
  over arbitrary source — `docgen/harvest.rs`'s `SymbolVisitor` — is kr0ki's own code
  and needs exactly the generalization §3's table already anticipated (a sibling
  relationship-collecting visitor, not a `Symbol`-collecting one), not a second
  walker written from scratch.

## 5. Non-goals

- Not a change to `docgen/` or `playbook/` — both stay as-is, scoped to what §1
  describes them as.
- Not SysML v2 / KerML *authoring* (D1, unrelated) — this plan only produces diagram
  notation for rendering, never emits SysML v2 concrete text.
- Not "one big AST-to-everything" — box 1 stays a thin `iso_ir` producer; all semantic
  lifting happens in boxes 2-4, exactly as the SysML-v2 arm already does.
- Not a replacement for raw/hand-authored diagram rendering. This pipeline and
  hand-written diagram-as-code are two permanent, coexisting inputs to box 5, not one
  superseding the other — see §6.

## 6. Why this matters (and why raw diagrams stay first-class too)

This pipeline's benefit is **zero-drift visualization of a system as a diagram without
an agent (or a person) reviewing all the code to get there** — because boxes 1-4 are a
pure function of the source tree (§2's "auto-reconciliation" property), the diagram is
guaranteed current with the code that produced it, and the context an agent would
otherwise spend reading the source to build a mental model is spent instead on the
already-current diagram. That is the whole point of doing the AST/IaC lowering instead
of asking an agent to hand-write a diagram from memory of the code.

It does **not** follow that hand-authored diagram-as-code becomes unnecessary or
second-class. Plenty of legitimate diagrams — a proposed architecture, a whiteboard
sketch turned into D2, a one-off sequence diagram for a design doc — are not derived
from any code or IaC and never will be. `PRD-KR0KI-001` FR2 already requires kr0ki to
accept and render raw diagram-as-code source in any supported format regardless of
provenance; this plan's pipeline is one more *producer* feeding that same box-5 entry
point, not a gate in front of it. The web-ux and MCP surfaces must support both paths
side by side (`docs/TODO.md` box 5's playbook item).

---

*Cross-reference: [`PLAN-KR0KI-002.md`](PLAN-KR0KI-002.md) for the pipeline this plan
extends; [`PATTERNS-kubernetes.md`](PATTERNS-kubernetes.md) for the recognizer-table
precedent a `PATTERNS-rust-source.md` would follow; `docs/TODO.md` box 1 for tracking.*
