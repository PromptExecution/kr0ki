# VOCABULARY — the D6 resolution

**Status:** RESOLVED 2026-09-05. Closes PRD-KR0KI-001 §5 **D6** ("vocabulary
collisions: systhread 'thread' vs KerML feature-chain, 'viewpoint' overload,
'projection' vs SysML view/viewpoint").
**Owner:** PromptExecution (@elasticdotventures).

---

## Decision

**The b00t / kr0ki / ufo-types surface uses OMG SysML v2 and KerML spec vocabulary
verbatim for every model construct, carries no informal synonym for any of them,
always spells the systhread concept "digital thread" (never bare "thread"), and bans
"projection".**

This is decidable **independently of D1** and de-risks it: no b00t-invented name
exists that a D1 outcome (`sysml-derive` posture) could later invalidate, because the
names are OMG's, which are stable. D6 is therefore **un-entangled from D1** and closed
now; D1 remains open on its own merits (ledgrrr#202).

## Canonical terms

| Contested term | Canonical form (b00t surface) | Do NOT use | Why |
|---|---|---|---|
| **thread** | **"digital thread"** — always qualified; the systhread program-comprehension concept | bare "thread" for the modeling concept | KerML has `FeatureChain`; an OS/runtime has threads; keeping "digital thread" always-qualified stops all three colliding |
| KerML `a.b.c` chain | **"feature chain"** / `FeatureChain` (KerML verbatim) | "thread", "path", "dotted name" | it is a named KerML construct |
| **viewpoint** | `ViewpointDefinition` / `ViewpointUsage` (OMG SysML v2 verbatim) — a formal model element: stakeholder concern + conformance constraints | informal "viewpoint" meaning "a way of looking at the model"; calling `SysmlViewKind` a "viewpoint" | v2 makes viewpoint a first-class element; any informal use shadows it. `ufo_types::SysmlViewKind` is a **view kind**, not a viewpoint. |
| **view** | `ViewDefinition` / `ViewUsage` (OMG verbatim); the kind axis is `ufo_types::SysmlViewKind` | informal "view" for an arbitrary data slice | there is no informal view — if it is a view it is a `ViewDefinition`; if it is the rendered output it is a `RenderingUsage` |
| **projection** | **BANNED.** Say `ViewDefinition` (the view) + `Expose` (its query) + `RenderingUsage` (its output) | "projection" anywhere in kr0ki / ufo-types code, docs, datums, or commit messages | not a SysML/KerML term; using it implies a formal construct that does not exist and collides conceptually with view/viewpoint |
| **rendering** (the model element) | `RenderingUsage` / `RenderingDefinition` (OMG verbatim) | bare "rendering" for the model element | keeps it distinct from kr0ki's implementation-side "renderer" |
| **renderer / render adapter / render backend** | kr0ki's box-5 implementation machinery (`RenderBackend`, `HttpKrokiBackend`, a future `KubeDiagramsBackend`) | conflating these with `RenderingUsage` | box 5 is code; `RenderingUsage` is model data — both kept, they do not collide when the model element is always spelled `RenderingUsage` |
| **cut / cut-node** | kr0ki's own graph-topology term — the single sanctioned crossing between the model side and the render side (PRD §0, Glossary) | — | no SysML collision; already established |
| **pattern recognizer** | kr0ki's box-3 term — a domain rule set lifting the UFO semantic graph into SysML constructs | "detector", "matcher", "inference" (imply automation guarantees not made) | consistent with PLAN-KR0KI-002 |

## Consequences

- `ufo_types::view::SysmlViewKind`, `ufo_types::sysml_model::{ElementKind, Relation}`,
  and `ufo_types::ontology::*` are already compliant — they use spec terms or UFO terms
  (`UfoRelation`, `OntologicalEdge`), none of which collide. No rename needed.
- `PLAN-KR0KI-002` and `DESIGN-NOTE-typed-model-layer` should have every occurrence of
  "projection" replaced with "view" / `ViewDefinition` + `Expose` as the edit lands
  naturally; existing occurrences are grandfathered but not to be added to.
- **systhread / nem-poweragent-lab**: the recommended convention for that codebase is
  the same — "digital thread" always qualified, `FeatureChain` for the KerML sense.
  systhread owns its own repo's naming; this resolution does not force a rename there,
  it records the b00tyverse-wide recommendation so a future systhread rename (if any)
  moves toward these names, not away.
- Anything that *emits* SysML v2 / KerML text is still gated on **D1** — but no longer
  on D6.
