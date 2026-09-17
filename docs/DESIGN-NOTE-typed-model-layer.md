# DESIGN NOTE — the typed model layer (SysML v2 / KerML only)

**Status:** pre-decision input to `PLAN-KR0KI-002`. **Not approved. Not a plan.**
PRD-KR0KI-001 §6.6 sequences `PLAN-KR0KI-002` *after* §5 (D1–D6) is settled; this note
exists so the conclusions of the 2026-09-05 architecture review survive until then and
inform D1/D3/D6 rather than being re-litigated.
**Created:** 2026-09-06 · **Owner:** PromptExecution (@elasticdotventures)
**Changelog 2026-09-17:** §4's D1 row is stale as originally written — D1 was resolved
2026-09-10, a week before a research pass caught the staleness (see
`docs/EVAL-sysml-derive.md` for the resolution and `docs/DESIGN-NOTE-agentic-mbse-generation.md`
for the fuller context). kr0ki's SysML v2 authoring path is unblocked. Left the original
§4 table row struck through rather than deleted, so the D1-open framing this note argued
from is still legible.

---

## 0. What this is about

A "hallucinated vision" Rust design was submitted for the typed model layer that would
sit **above** `ufo_types::iso_ir::{Node, Edge}` (the generic transport floor) and
**below** any renderer (kr0ki, the isometric SVG path, the systhread-explorer). It
proposed `SysGraph` / `Element` / a closed `ElementKind` / typed `Relation` /
`DiagramKind` / `BehaviorDiagram` / `StructureDiagram` / `UmlRelation` / a sealed
`Viewpoint` trait hierarchy / `SchemaVersion` / `ElementId = u128` / `Provenance`.

It was reviewed against what is actually shipped:

- `ufo-types` v0.11.0 — `iso_ir::{Node, Edge}` (free-form `String` classification, no
  container, `id: String`, no provenance, no schema version), `stereotype::UfoStereotype`
  (14 closed variants), `sysml::validate_sysml_v2` (wraps `sysml-v2-parser` 0.54,
  **syntax only**), `mbse::to_sysml_v2` (emits SysML **v2** concrete text).
- `systhread-core` — untyped `serde_json::Value` graph container, string-comparison edge
  dispatch, a *geometric* `Layout::{TwoD, ThreeD}` sum type, **no** viewpoint / view /
  diagram-kind type anywhere.
- `nem-poweragent-lab` design doc `2026-08-26-systhread-3d-explorer-design.md` §3/§7 —
  specifies (deferred, unbuilt) a typed intermediate model above `Node`/`Edge` that
  retains "the source SysML/KerML construct and UFO stereotype for each visual element",
  per-element source provenance, and **multiple `ViewDefinition`s over one model**.
- `kr0ki` P0 — `DiagramFormat` (8 companion-free Kroki slugs) + `RenderBackend` trait +
  content-addressed `FsCache`. Consumes text, not a model. Zero coupling to any model
  crate.

---

## 1. SysML v2 / KerML ONLY

The reference's `DiagramKind` / `BehaviorDiagram` / `StructureDiagram` / `UmlRelation`
are **SysML v1** and MUST NOT be adopted.

- The 9-diagram taxonomy (Activity / Sequence / State Machine / Use Case | Requirement |
  BDD / IBD / Package / Parametric) and its `SameAsUml2` / `ModifiedFromUml2` /
  `NewInSysml` tagging are a SysML 1.x pedagogical artifact — they describe how each
  SysML 1.x diagram derives from **UML 2**.
- SysML v2 is **not** derived from UML 2. It is a ground-up textual language on a KerML
  metamodel. There is nothing for `UmlRelation` to compare against; the enum is
  meaningless in a v2-only codebase.
- SysML v2 has **no fixed diagram-kind enum**. Views are first-class *model elements*:
  `ViewpointDefinition` (a stakeholder concern + the constraints a conforming view must
  satisfy), `ViewDefinition` / `ViewUsage` (a query over the model plus a rendering),
  `RenderingDefinition` / `RenderingUsage`, and `Expose` (what the view pulls in). These
  are authored in `.sysml` / KerML text, not hard-coded as a Rust enum.
- Everything already shipped is v2/KerML: `sysml-v2-parser` 0.54, `mbse::to_sysml_v2`
  emitting `part def` / `attribute … : ScalarValues::…`, the design doc's "OMG SysML v2
  / KerML specs, Pilot Implementation" reference toolchain. There is zero v1 anywhere.

**Consequence:** the "land `DiagramKind` + `UmlRelation` now, low-risk" recommendation
from the review is **withdrawn** — it was a v1 taxonomy. The v2-equivalent axis (which
concrete notation to emit) already exists as kr0ki's `DiagramFormat`; the v2 "viewpoint"
concept is `ViewpointDefinition` / `ViewDefinition` model data, blocked on D6. There is
no v2-correct, collision-free item to land ahead of D1/D3/D6.

---

## 2. The typed layer — v2/KerML restatement

All of this is **`PLAN-KR0KI-002` scope**, blocked on §5. Recorded here as the reviewed
starting point, not as an approved design.

### 2.1 `ElementKind` — closed, KerML/SysML-v2 abstract syntax only, def/usage paired

Closed because the KerML + SysML-v2 abstract syntax is a fixed, spec-governed set:

```
Package
PartDefinition        | PartUsage
AttributeUsage
PortDefinition         | PortUsage
ConnectionUsage
InterfaceDefinition    | InterfaceUsage
ItemDefinition         | ItemUsage
ActionDefinition       | ActionUsage
StateDefinition        | StateUsage
RequirementDefinition  | RequirementUsage
ConstraintDefinition   | ConstraintUsage
AllocationUsage
ViewDefinition         | ViewUsage
ViewpointDefinition
RenderingUsage
```

- Domain concepts (`Mission`, `Protocol`, `Agent`, `MCPServer`, `Bus`, `Generator`,
  `Phase`, …) are **NOT** members. They stay `iso_ir::Node::part_type` strings — or a
  *downstream* domain enum owned by the consuming crate. Putting them in this enum
  re-introduces exactly the domain↔SysML coupling `iso_ir` was split out (v0.11.0) to
  remove; `iso_ir`'s own doc comment already refuses it ("the set of meaningful values
  is entirely the consuming domain's business, not this crate's").
- No escape hatch is needed *in `ElementKind`*: anything not expressible in KerML/SysML-v2
  abstract syntax simply stays at the `iso_ir` layer and is never lifted.

### 2.2 `Relation` — typed over the KerML relationship set, + one `Domain` escape hatch

Not the reference's v1-flavoured `Contains` / `Flow` / `Transition`. The KerML/SysML-v2
relationships:

```
FeatureMembership { owner, member }
Specialization    { specific, general }      // + Subsetting, Redefinition, Conjugation
ConnectionUsage   { ends: [ElementId] }
Succession        { source, target }          // v2 control flow — replaces v1 "Transition";
                                              //   guard/trigger live on the Succession /
                                              //   ActionUsage, and MUST NOT be dropped
AllocationUsage   { source, target }
Satisfy           { requirement, subject }
Verify            { requirement, by }
Refine            { refined, refining }
Dependency        { client, supplier }
```

plus the single escape hatch:

```
Domain            { source, target, kind: String }
```

for edges that are genuinely not a KerML/SysML-v2 relationship — b00t digital-thread
"attachment", a pipeline "sequence" that is not a real `Succession`, etc. This keeps the
closed set closed without forcing every domain edge through it, and without a free-form
`edge_type: String` on the typed layer (that stays only at `iso_ir`).

### 2.3 `Provenance` — on every typed element, deterministic

- A Rust source span (`file`, `byte_range`) **or** a stable symbol path
  (`crate::module::Item`) **or** a KerML qualified name.
- MUST NOT be a UUID, timestamp, or wall-clock value — the byte-identical-output NFR
  (NFR1) forbids it. `systhread-core`'s `semantic_type: Option<String>` is the current
  v1 placeholder for exactly this slot.

### 2.4 Identity — `id: String`, git/content hash **optional**

- `id: String`, human-meaningful ids stay valid (`"bus-3"`, `"agent.foo"`).
- A content hash or git blob/commit hash **MAY** back the string
  (`sha256:…`, `git:<blob-sha>`) where an element needs a stable derived identity — this
  is an available option, not the required form.
- **NOT** `ElementId = u128`. A random `u128` breaks deterministic output; a derived one
  is just a less legible string id. The whole artifact model is "byte-identical,
  git-diffable" — string ids are load-bearing for that.

### 2.5 Views & viewpoints — data, not Rust traits

- A **view** is a SysML-v2 `ViewDefinition` — an `Expose` query over the model plus a
  `RenderingUsage`. A **viewpoint** is a `ViewpointDefinition` — a stakeholder concern
  plus the constraints a conforming view must meet.
- Authored in `.sysml` / KerML text or a datum; resolved at runtime. **No** sealed
  `Viewpoint` trait, **no** `BehavioralView` / `StructuralView` / `AnalyticalView`
  marker hierarchy, **no** per-type `accepts(element)` / `accepts_relation(relation)`
  compiled into Rust. "Add a viewpoint" MUST be a data change, not a recompile — the
  same argument the reference itself makes for `DiagramKind` ("a data-level sum type …
  not trait inheritance"), applied one level up.
- Rust **traits are reserved for rendering capability** — `Render<Mermaid> for Model`,
  `Render<IsometricSvg> for Model`, etc. This is orthogonal to model classification and
  is the one place the reference's "traits providing … rendering capabilities" intent
  fits cleanly. kr0ki P0 already embodies the split: `RenderBackend` trait +
  `DiagramFormat` data.
- **Naming: D6 RESOLVED 2026-09-05** ([`VOCABULARY.md`](VOCABULARY.md)). The b00t
  surface adopts the OMG SysML v2 / KerML spec terms verbatim (`ViewDefinition`,
  `ViewpointDefinition`, `ViewUsage`, `RenderingUsage`, `Expose`, `FeatureChain`) with
  no informal synonyms; "digital thread" is always qualified; **"projection" is
  banned** (say `ViewDefinition` + `Expose`). This un-entangles the naming question
  from D1 — spec names are stable, so nothing here can be invalidated by a D1 outcome.

### 2.6 Container — a typed envelope is fine *here* (it is not `iso_ir`)

A typed `Model` (the reference's `SysGraph`) holding `elements`, `relations`, and
resolved `views` is acceptable at this layer. serde JSON, round-trip ("ouroboros")
tested — the same bar as `systhread-core`'s `PositionedGraph`.

### 2.7 SysML v2 text MUST be grammar-validated — always

Any typed element or view that emits SysML v2 / KerML concrete text MUST have that text
parsed by [`sysml-v2-parser`](https://crates.io/crates/sysml-v2-parser) before it is
written, cached, or handed to a renderer — never "visually inspected", never assumed
well-formed because a template produced it.

- The gate is `ufo_types::sysml::validate_sysml_v2` (wraps `sysml-v2-parser`'s resilient
  `parse_for_editor`; zero diagnostics ⇒ pass). Reuse it — do not hand-roll a second
  parser wrapper. It is **syntax only** by design; deeper semantic checks stay behind an
  oracle boundary (design doc §7), but the syntax gate is non-optional and runs on every
  emit path.
- `sysml-derive`'s `#[derive(SysmlBlock)]` already holds itself to this bar
  ("real-grammar-validated ... not just visually inspected"); the typed layer inherits
  the same contract, and its golden fixtures MUST be validated output, not just
  byte-stable output.
- **Version:** pin `sysml-v2-parser` to exactly the version `ufo-types` tracks and bump
  the two in lockstep (`ufo-types` v0.11.0 → `0.54`; crates.io latest is `0.55.0` as of
  2026-08-27). A parser-version skew between `ufo-types` and this layer means the two
  disagree on what "valid SysML v2" is — treat the pin as a wire-format constant, like
  `LAYOUT_SEED`.

### 2.8 `SchemaVersion` — rejected

No `{ major, minor }` field. Stability is enforced by golden fixtures + `sha256`
content hashes + frozen wire constants (`systhread-core`'s `LAYOUT_SEED` is the
precedent: "treat it as a wire-format constant, not a tuning knob"). A version field
invites the drift the fixture discipline exists to prevent.

### 2.9 Disposition of `DiagramKind` / `BehaviorDiagram` / `StructureDiagram` / `UmlRelation`

The review was asked to "land `DiagramKind` + `UmlRelation` as data-level sum types,
colliding with nothing". They cannot be landed as written — see §1 — but the request's
*intent* (a closed, data-level classification of "what kind of render is this", not a
trait hierarchy) is sound and has a v2 disposition. It splits in two:

| v1 construct | v2 replacement | Where it lives | Status |
|---|---|---|---|
| `DiagramKind` — semantic *view* kind (Behavior / Requirement / Structure) | SysML v2 `ViewpointDefinition` / `ViewDefinition` — a stakeholder concern + an `Expose` query + a `RenderingUsage`, authored as KerML **data** | the typed layer's model, read from `.sysml` / a datum | deferred — naming blocked on **D6**, home blocked on **D3** |
| `DiagramKind` — concrete *notation* to emit (SVG via PlantUML vs. GraphViz vs. Mermaid vs. isometric) | `kr0ki::format::DiagramFormat` — a closed `#[derive(Copy)]` enum, exactly the "data-level sum type, no trait" shape the request asked for | **`kr0ki-core`, already shipped** (PR #1) | **done** — this is the part that could land collision-free, and it already has |
| `UmlRelation { SameAsUml2, ModifiedFromUml2, NewInSysml }` | — none — | — | **deleted.** It expresses "how does this SysML 1.x diagram differ from UML 2"; v2 is not a UML-2 derivative, so there is nothing to classify. No replacement type. |

So the answer to "land it" is: the notation axis is already landed as `DiagramFormat`;
the view-kind axis is v2 model data, not a Rust enum, and is D3/D6-blocked; `UmlRelation`
has no successor. Nothing new to add.

### 2.10 `ElementKind` → `UfoStereotype` — resolved 2026-09-17, no prior precedent

`2026-08-26-systhread-3d-explorer-design.md` wanted per-element UFO stereotype
retention but left it "deferred, unbuilt"; nothing else in `ufo-types` or kr0ki derives
a `UfoStereotype` from an `ElementKind` (`ufo_types::mbse::MbseExport` goes the
*opposite* direction — a `Stereotyped` Rust value → SysML v2 text — the caller already
supplies the stereotype). This is that missing mapping, needed for
`ufo_graph::to_sysgraph` (kr0ki, SysGraph adoption for the SysML-v2 arm) to give every
node a stereotype the way `PATTERNS-kubernetes.md` §3 and `PATTERNS-rust-source.md` §3
already do for their arms.

Mechanical, using `ElementKind::is_definition`/`is_usage` (already exhaustive over 23 of
24 variants — the 24th, `Package`, is neither, by its own doc comment):

| `ElementKind` | `UfoStereotype` | Reason |
|---|---|---|
| `Package` | `Kind("Package")` | a persistent namespace/container — same reasoning as `PATTERNS-rust-source.md`'s `module → Kind` |
| any `*Definition` | `Kind(kind_name)` | a `*Definition` metaclass defines a class of things — UFO `Kind`'s core sense, rigid and essential |
| any `*Usage` | `Role(kind_name)` | a `*Usage` only exists as a contextual, relationally-dependent occurrence of its definition within a particular containing structure — matches `Role`'s "anti-rigid, relationally dependent type" more than a second `Kind` would (two `Kind`s for one concept, defined vs. used, would blur the def/usage distinction §2.1 exists to keep) |

`kind_name` is the `ElementKind`'s own name (`"PartDefinition"`, `"PartUsage"`, …), not
the model element's own `name()` — mirrors both existing arms' precedent (Kubernetes:
`SubKind { name: "Deployment", .. }`, not the specific Deployment's own name; Rust:
`Kind("struct")`, not the specific struct's own name). A relationship element (whose
`@type` isn't one of `ElementKind`'s 24 variants at all — `FeatureMembership`,
`Specialization`, …) simply produces no node here; it already becomes an edge via
`build_ufo_graph`, not a node.

---

## 3. What the reference got right

Independently corroborated by `2026-08-26-systhread-3d-explorer-design.md`:

- a typed model layer **above** the `Node`/`Edge` floor, not replacing it (§3);
- per-element source provenance (§7 acceptance hooks);
- **multiple** `ViewDefinition`s over one model, not one universal graph (§7);
- sum types for genuinely incompatible variants (verbatim the `Layout::{TwoD,ThreeD}`
  rationale already in `positioned.rs`);
- serde JSON as the interchange boundary with round-trip tests (already how
  `PositionedGraph` works).

## 4. Blocked on

| Dep | Question | Effect on this layer |
|---|---|---|
| ~~[`ledgrrr#202`](https://github.com/PromptExecution/ledgrrr/issues/202) (D1)~~ | ~~`sysml-derive` extend-vs-wrap-vs-re-export for `UfoStereotype`-tagged types~~ | **RESOLVED 2026-09-10** — separation of concerns, not a single macro: `sysml-derive` stays structure-only (`#[derive(SysmlBlock)]`), `UfoStereotype → SysML v2 metadata` is emitted by `ufo_types::mbse::MbseExport` (already shipped), consumers compose both. See `docs/EVAL-sysml-derive.md`. |
| `nem-poweragent-lab#53` follow-up (D3) | does the typed layer live in `ufo-types`, `systhread-core`, or its own crate | where these types are defined — *operator-resolved 2026-09-05: `ufo-types` (`sysml_model` + `ontology` + `view` merged)* |
| ~~D6~~ | ~~vocabulary: "view" / "viewpoint" / "projection" / "thread" collisions~~ | **RESOLVED 2026-09-05** — [`VOCABULARY.md`](VOCABULARY.md): OMG spec terms verbatim, "projection" banned, "digital thread" always qualified; un-entangled from D1 |
