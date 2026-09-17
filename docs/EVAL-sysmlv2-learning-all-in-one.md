# EVAL — `sysmlv2-learning-all-in-one` (a.k.a. SynFeld)

**Evaluated:** 2026-09-17 · **For:** a tool/library survey ("don't reinvent the
wheel") and a search for existing agentic-generation patterns for SysML v2.
**Verdict:** not a curated learning list as the name implies — a full MBSE
learning *platform*. No overlap with kr0ki's read/render path (no model
storage/versioning at all); its AI-teacher subsystem is the single most
relevant find in this whole research batch for a future generation feature.

---

## What it is

`FlyingCpp/sysmlv2-learning-all-in-one` is **SynFeld**, an EPL-2.0, v0.1.0,
actively developed, Node.js 24 full-stack platform: `apps/{api,teacher,
validator,web}` (services), `packages/{sysml-plantuml-service,
agent-resource-policy,teacher-contract}` (shared libs), `courses/` +
`knowledge-packs/` (content), `scripts/` (build/validation gates). It vendors
the **Official SysML v2 Pilot Implementation** (pinned "2026-04 / kernel
0.59.0", hash-verified) and PlantUML as runtimes — not reinvented parsers.

## Tool/library catalog

| Name | What it does | Category | kr0ki status |
|---|---|---|---|
| Official SysML v2 Pilot Implementation (kernel 0.59.0) | Canonical parser/validator, Java | Validator | kr0ki uses `sysml-v2-parser` (a Rust binding over the same upstream lineage) instead |
| **ELK.js 0.11.1** (Eclipse Layout Kernel, EPL-2.0) | Graph auto-layout: hierarchy, node sizing, port anchors | Layout engine | **New to kr0ki** — no equivalent; kr0ki delegates all layout to Kroki backends (Graphviz/PlantUML's own layout), with no domain-aware (ports, nested containers, cross-hierarchy) layout of its own |
| PlantUML (Java drawing API, used as a low-level `UGraphic`/`ImageBuilder` primitive, not the text-DSL) | SVG drawing primitive | Renderer primitive | Different usage mode than kr0ki's (kr0ki uses PlantUML only via Kroki's text-in/SVG-out compiler) |
| LiteLLM | Multi-provider LLM gateway/proxy | Agent infra | New — relevant only if kr0ki ever adds AI generation |
| Graphviz | Layout/render helper | Renderer | kr0ki already has this via Kroki |

No Flexo, SysON, or `Systems-Modeling/SysML-v2-API-Services` references
anywhere — this project doesn't touch model storage/versioning at all, only
local text-file authoring, validation, and rendering. **Zero overlap risk**
with kr0ki's existing read-path evaluations.

## Notable example pattern — external confirmation of kr0ki's own vocabulary

`scripts/fixtures/plantuml-view-regressions/*.sysml` ships real idiomatic
fixtures. One, verbatim:

```sysml
package CjkLayout {
  requirement def FunctionalRequirement { doc /* ... */ }
  requirement goal001 : FunctionalRequirement { doc /* ... */ }
  view cjkRequirementView : StandardViewDefinitions::GeneralView {
    expose FunctionalRequirement;
    expose goal001;
  }
}
```

This is a **direct, load-bearing confirmation** of `docs/VOCABULARY.md`'s
canonical terms in real-world idiomatic use — `view ... : GeneralView { expose
... }` matches kr0ki's `ViewDefinition`/`Expose` vocabulary exactly, not a
paraphrase. D6's resolution to use OMG spec terms verbatim is externally
validated by an unrelated third party's actual usage.

## Curriculum as a MECE reference

The 32-lesson EV modeling course progresses structure → interfaces → behavior
→ requirements → verification — a clean, externally-authored MECE
decomposition of SysML v2 capability areas. Checked against kr0ki's current
scope: kr0ki covers **rendering only**, and within that, only structure /
interconnection views (via KubeDiagrams / box-5). **Zero current coverage of
behavior (Action/State) or verification-relationship rendering** — a real,
named gap, though explicitly out of kr0ki's stated scope today (blocked on
boxes 2+3 per `docs/TODO.md`). Worth keeping as a checklist for future
per-`ViewDefinition` rendering (FR4) completeness.

## AI/multi-agent generation content — highest-value finding

`apps/teacher/agent/` implements a **main-orchestrator + worker-delegate**
architecture (`intent-orchestrator-v2.mts` dispatches to `candidate-worker.mts`,
`repair-worker.mts`, `validator-repair-worker.mts`, `final-answer-worker.mts`),
governed by its own `AI_TEACHER_ARCHITECTURE.md`. The load-bearing rule,
stated in their own docs:

> Candidate/Repair generates full candidate text; the server binds content
> hash + Official Validator result + delivery state — schema validity proves
> structure only, never business correctness.

This is a strict validator-in-the-loop repair loop (bounded: 3 repair rounds,
256 KiB candidate cap, shared query budget) where **server-side code owns
identity, permission, hash, and persistence; the LLM only proposes.**

**TRIZ framing:** *"agents must generate freely" vs. "the model must stay a
single verified source of truth"* — resolved by never trusting agent output as
committed state until it passes the same Official Validator gate a human edit
would, and binding the accepted result to a content hash before delivery. This
is the same shape as SysTemp's Writer↔Parser loop (see
`docs/DESIGN-NOTE-agentic-mbse-generation.md`) but goes one step further: it
also binds *delivery/commit authority* to the validator result, not just
syntax acceptance — closer to what a real multi-agent commit gate needs.

## New tools/patterns kr0ki should evaluate (shortlist)

1. **ELK.js** — the one genuine capability gap surfaced here: kr0ki has no
   domain-aware auto-layout. This repo proves "official model → ELK layout →
   drawing API" works at production scale with explicit budgets and
   fail-closed behavior — a strong reference for kr0ki's still-unbuilt
   per-`ViewDefinition` rendering (FR4), not an immediate priority.
2. **The "model decides what/how-connected; layout decides where/how-routed;
   layout never writes back to the model" separation** in
   `packages/sysml-plantuml-service` — independently validates kr0ki's own
   cut-node boundary principle at production scale, in an unrelated codebase.
   Not directly consumable (JS/Java, not Rust) — cite as prior art, not a
   dependency.
3. **The Candidate→Validator-Repair→bind-hash-then-deliver worker pattern** —
   directly transplantable *pattern*, not code, for any future kr0ki/ufo-types
   agentic-generation feature: never treat LLM-authored SysML v2 text as
   trusted until `validate_sysml_v2` passes, and bind delivery to a content
   hash exactly like kr0ki's own `ModelSnapshot.content_hash` discipline
   already does on the read side.
4. **LiteLLM** — reuse rather than hand-roll multi-provider LLM access, if and
   when kr0ki adds a generation feature.

**Deployment-footprint caveat:** this repo's validator/rendering stack (Java
21 + ELK.js + PlantUML drawing API + Graphviz, in one Node/Java hybrid
service) is much heavier than kr0ki's lightweight Rust+Kroki-container model.
Any adoption here is pattern-borrowing, never dependency-borrowing.
