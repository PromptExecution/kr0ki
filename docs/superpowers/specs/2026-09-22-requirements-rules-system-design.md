# Requirements-Centric Model + Rules-as-Documents Tracking System — Design

**Status:** proposed, pending review. This spec merges two sub-projects the
user asked for in one pass — (a) a requirements-first view over the canonical
UFO graph, and (b) a rules-as-documents tracking system attached to SysML v2
projects — because (b) is the product shape of (a). A third sub-project
(versioned vue-flow UX, copy/edit/save to Flexo) and a fourth (dbt as a graph
source) are explicitly deferred; see "Out of scope" below.

**Origin:** brainstormed 2026-09-22 via `superpowers:brainstorming`
(architectural path). Prior art referenced throughout: `docs/TODO.md`,
`docs/PLAN-KR0KI-006-dbt-digital-thread.md`, `docs/superpowers/plans/
2026-09-20-flexo-write-path.md`, and `ufo-types` (pinned rev `ee873488f`).

## 0. One paragraph

kr0ki's canonical graph and its ReqIF/requirements layer already exist
(`ufo_types::sysgraph::SysGraph`, `ufo_types::mbse::requirements::
RequirementGraph`), and kr0ki-sysmlv2-client can already write a commit back
to a live Flexo project. What's missing is a way for a project to carry its
own policy — "rules" — as first-class, versioned content, and a mechanism
that recomputes the graph from the latest SysML commit and checks it against
those rules, surfacing violations anchored to the offending elements. This
spec designs that mechanism, deliberately reusing existing idioms
(`Satisfies<C>`, the requirements-graph promotion gate, `SourceAnchor`
provenance) rather than inventing parallel ones.

## 1. Decisions already made (Q&A record)

These were resolved through brainstorming and are binding for this spec,
not open questions:

1. **Graph shape** — non-invasive first. Build a requirements-first API/view
   surface on top of the existing graph; do **not** force every node to
   trace to a `Requirement`. A stricter "mandatory anchor" constrained-graph
   mode stays possible later, opt-in per project, not built now.
2. **Rule format** — OPA/Rego for v1, evaluated via an embedded, pure-Rust
   engine (no Python sidecar). The evaluation interface is designed so a
   JEV-backed rule type can be added later without reworking the interface.
3. **Project scope** — rule documents attach to the durable SysML v2/Flexo
   project (`kr0ki-sysmlv2-client::Project`), which is the source of truth.
   StoryB00k's ephemeral, thread-scoped project (`project_store.py`) may
   reference/inherit a Flexo project's rules but never owns them.
4. **Rule storage** — rule docs are versioned *inside* the Flexo project as
   KerML/SysML elements, committed via the write path already shipped
   (`kr0ki-sysmlv2-client::create_commit`, landed `013e653`), not a separate
   kr0ki-owned store and not a sibling git repo.
5. **Recompute trigger** — on-demand only. `recompute_and_evaluate` is a
   pure function of one `ModelSnapshot`; "automatic" is delivered by the
   Playb00k UI polling that endpoint while a project view is open, not a
   server-side daemon. The already-tracked `docs/TODO.md` "Commit poll loop"
   item remains a possible later addition, not part of this spec.
6. **v1 acceptance bar** — proven end-to-end on one real Flexo project: 2-3
   real Rego rules authored as elements in it, a recompute triggered from
   Playb00k (or, for this spec's own v1, from a direct API/MCP call — the
   Playb00k button itself is UX sub-project #3's job), pass/fail results
   returned with links to the offending elements.

## 2. Key architectural discovery

`ufo-types` already has the exact idiom this system needs — it does not need
inventing:

- `ufo_types::satisfies::{Satisfies<C>, Constraint, SatisfiesResult,
  Disposition, NodeId}` — the crate's single canonical constraint-evaluation
  shape, used in `sysml.rs`, `coherence.rs`, and ~60 call sites in
  `ledgrrr`/`ledger-core`. `SatisfiesResult{ disposition, confidence: f64,
  evidence_nodes: Vec<NodeId> }` maps directly onto JEV's typed-decision
  shape: `confidence` *is* JEV's calibrated confidence; `Disposition::Unknown`
  *is* JEV's `Noul` "unsure" case. Rule evaluation should be `impl
  Satisfies<C> for ...`, not a bespoke violation type.
- `ufo_types::mbse::requirements::RequirementGraph` already has an inferred/
  proposed-relation promotion workflow (`RelationAuthority::Inferred`/
  `Proposed`, `Promotion`, `RequirementGraph::promote_relation`) built and
  tested for the ReqIF work. Rule-evaluation results should feed this
  workflow as inferred `RequirementRelationKind::Satisfies` edges, not build
  a second promotion mechanism.

  **Naming collision, not a design conflict:** `ufo_graph.rs` already emits
  a *different* `Satisfies` — `ontology::UfoRelation::Satisfies`, from
  SysML `Satisfy` elements, into `SysGraph` edges — before this work
  exists. That is a distinct enum in a distinct graph (`SysGraph`'s general
  UFO-relation vocabulary) from `mbse::requirements::
  RequirementRelationKind::Satisfies` (the `RequirementGraph`-specific
  traceability vocabulary this spec adds edges to). Same English word, two
  separate types already coexisting in the codebase — worth a one-line
  callout in the implementation plan so the two never get conflated.
- `ufo_types::sysgraph::{SysGraph, OntologicalNode}` plus
  `ontology::{OntologicalEdge, SourceAnchor}` are the existing graph
  envelope and provenance types. The SysML-v2 arm (`kr0ki-core/src/
  ufo_graph.rs`) does not yet populate `SourceAnchor` on the edges it
  produces (`docs/TODO.md`, Box 2, "genuinely still open") — this spec
  makes that population a hard prerequisite, since a violation without a
  `SourceAnchor` cannot point back to an offending element.

## 3. Rule-document representation

A rule document is a KerML element with a `rule:<slug>` identity prefix
(mirroring the `dbt:` prefix convention from `PLAN-KR0KI-006`), its Rego
source carried as the element's text content. This is a **design choice**,
not an existing codebase precedent — a repo-wide search found no prior
`"Documentation"`-element handling anywhere in kr0ki to mirror. What *is*
verified: `kr0ki-sysmlv2-client::DataVersion.payload: Option<serde_json::Value>`
and `Element.fields` are both fully generic (`serde_json::Value`/
`serde_json::Map`), so carrying an arbitrary Rego-text field needs **no new
wire types** in `kr0ki-sysmlv2-client` — that part of the claim holds. The
element-shape convention itself (which `@type` and field name the Rego text
lives under) is this spec's decision, to be made concrete in the
implementation plan, e.g. `{"@type": "Documentation", "body": "<rego
source>"}` or a purpose-built `{"@type": "RuleDocument", "rego": "..."}` —
pick one and document it there; either is compatible with `create_commit`.

New module `kr0ki_core::rule_docs`:

```rust
pub struct RuleDoc {
    pub id: ElementId,
    pub name: String,
    pub rego_source: String,
    pub backend: RuleBackendKind, // default: Rego
}

pub fn extract_rule_docs(snapshot: &ModelSnapshot) -> Vec<RuleDoc>;
```

Mirrors how `ufo_graph.rs` already walks a `ModelSnapshot` — no new
traversal primitives needed.

## 4. Evaluation engine

`kr0ki-core` takes [`regorus`](https://github.com/microsoft/regorus)
(Microsoft, pure-Rust Rego interpreter, actively maintained, MIT/Apache) as
a direct dependency — the same "no sidecar" precedent already set by `reqrs`
for ReqIF parsing (`docs/TODO.md`'s ReqIF stream). The vendored
`reqif-opa-mcp`'s own artifact→document-graph→candidate→OPA→ReqIF pipeline
is a different, already-scoped concern and is not touched by this work.

**Verified against `microsoft/regorus`'s actual `src/engine.rs` (not just its
README) on 2026-09-22** — `regorus` is not yet a dependency anywhere in this
repo, so this is still to be confirmed again once it's actually pinned (an
API can move between that check and the pin date), but the signatures below
are read from source, not remembered:

```rust
pub fn add_policy(&mut self, path: String, rego: String) -> Result<String>;
pub fn set_input_json(&mut self, input_json: &str) -> Result<()>;
pub fn add_data_json(&mut self, data_json: &str) -> Result<()>;
pub fn eval_rule(&mut self, rule: String) -> Result<Value>;
```

For each `RuleDoc`: build a `regorus::Engine`, `add_policy(doc.id.to_string(),
doc.rego_source.clone())`, `set_input_json(&serde_json::to_string(graph)?)`
with the current `SysGraph` serialized, `eval_rule("data.kr0ki.violations"
.to_string())` against a documented entrypoint convention — `violations`, a
JSON array rule authors populate, one entry per violation with at least an
`element_id` and a `reason`. (`add_data_json` is unused in v1 — no rule
needs Rego-side static data beyond the input graph itself; noted here so a
future rule type that does isn't a surprise API gap.)

```rust
pub trait RuleBackend {
    fn evaluate(&self, graph: &SysGraph, doc: &RuleDoc) -> SatisfiesResult;
}

pub struct RegorusBackend;
impl RuleBackend for RegorusBackend { /* ... */ }
```

`RegorusBackend::evaluate` maps `data.kr0ki.violations` entries to
`Disposition::Violated{reason}` (confidence `1.0`, deterministic), an empty
array to `Disposition::Satisfied`, and a Rego evaluation error to
`Disposition::Unknown` (confidence `0.0`) rather than failing the whole
recompute — one bad rule doc must not block every other rule's results.
`evidence_nodes: Vec<NodeId>` are just the violated `SysGraph` node ids,
each formatted `node:<ElementId>` (`NodeId` is an opaque string newtype).
This is *not* a `SourceAnchor` conversion: `OntologicalNode` carries no
provenance field of its own, only `OntologicalEdge` does. Resolving a node
id back into actual provenance is Section 5's job, done once at the
`RequirementGraph`-integration boundary, not here — `RegorusBackend` stays
ignorant of `SourceAnchor`/`Provenance` entirely.

## 5. Graph integration — feeding the requirements graph

The good-faith review pass on this spec's first draft found two real gaps
here (a field-shape mismatch and a missing registration step) and one
genuine contradiction (node vs. edge provenance) — this section is the
fixed version; see the revision note at the end of this section for what
changed and why.

`RequirementGraph`'s actual shapes (verified against `ufo-types` rev
`ee87348`, `src/mbse/requirements.rs`):

```rust
pub struct RequirementGraph {
    pub baseline: BaselineIdentity,
    pub requirements: Vec<Requirement>,
    pub evidence: Vec<EvidenceRef>,
    pub relations: Vec<RequirementRelation>,
}
pub struct Requirement { id, title, text, baseline, provenance: Provenance,
    attributes: BTreeMap<String,String>, evidence: Vec<EvidenceRef> }
pub struct EvidenceRef { id, label, uri: Option<String>, provenance: Option<Provenance> }
pub struct Provenance { source_uri: String, artifact_sha256: Option<String>, locator: Option<String> }
pub struct RequirementRelation { id, source: String, target: String, kind,
    authority: RelationAuthority, provenance: Provenance, promotion: Option<Promotion> }
pub struct NonAuthoritativeRelation { confidence: f64, rationale: String,
    evidence: Vec<EvidenceRef>, model: ModelIdentity }
```

`RequirementGraph::validate()` requires every relation's `source`/`target`
to already resolve to an id present in `requirements` or `evidence` — a
`RuleDoc` and a violated `SysGraph` node are neither, by default. So
recompute registers both sides explicitly before creating the relation:

1. **Rule doc → `Requirement`.** Each `RuleDoc`, on first use, becomes a
   `Requirement` in the graph: `id = doc.id` (already `rule:<slug>`),
   `title = doc.name`, `text = doc.rego_source`, `baseline =
   graph.baseline` (the rule doc shares the same commit-derived baseline as
   everything else in this recompute — it is not a separate baseline),
   `provenance = Provenance{ source_uri: format!("kerml:{}", doc.id),
   artifact_sha256: None, locator: None }`, `attributes = {"kind":
   "rego-rule"}` (tags it apart from ReqIF-sourced requirements),
   `evidence = vec![]`. Idempotent: recompute upserts by id rather than
   appending duplicates on every call.

2. **Violated `SysGraph` node → `EvidenceRef`, with provenance resolved
   from incident edges (fixes the node/edge contradiction).**
   `OntologicalNode` carries no provenance of its own — only
   `OntologicalEdge.provenance: Vec<SourceAnchor>` does. Rather than
   picking one edge arbitrarily (undefined) or waiting on an upstream
   `ufo-types` change to add node-level provenance (out of kr0ki's hands —
   see `AGENTS.md` §9's existing "not kr0ki's to make" list, and this would
   be a new entry on it, not something to decide unilaterally here), v1
   registers **one `EvidenceRef` per incident edge that carries
   provenance** (a node touched by zero, one, or several such edges yields
   zero, one, or several `EvidenceRef`s — this is honest about what's
   actually known, not a forced single answer):
   ```rust
   fn provenance_from_anchor(anchor: &SourceAnchor) -> Provenance {
       match anchor {
           SourceAnchor::KermlQualifiedName(qn) =>
               Provenance { source_uri: format!("kerml:{qn}"), artifact_sha256: None, locator: None },
           SourceAnchor::Vcs { repo, commit, path } => Provenance {
               source_uri: repo.clone().unwrap_or_else(|| commit.clone()),
               artifact_sha256: None, locator: path.clone(),
           },
           SourceAnchor::SysmlFile { path, line } => Provenance {
               source_uri: format!("sysml-file:{path}"), artifact_sha256: None,
               locator: line.map(|l| l.to_string()),
           },
           SourceAnchor::K8sObject { api_version, kind, namespace, name, .. } => Provenance {
               source_uri: format!("k8s:{api_version}/{kind}/{}/{name}",
                   namespace.as_deref().unwrap_or("")),
               artifact_sha256: None, locator: None,
           },
           SourceAnchor::RustSpan { file, line, .. } => Provenance {
               source_uri: format!("rust-span:{file}"), artifact_sha256: None,
               locator: Some(line.to_string()),
           },
           SourceAnchor::SymbolPath(p) =>
               Provenance { source_uri: format!("symbol:{p}"), artifact_sha256: None, locator: None },
           SourceAnchor::Other(s) =>
               Provenance { source_uri: s.clone(), artifact_sha256: None, locator: None },
       }
   }
   ```
   Each resulting `EvidenceRef` gets `id = format!("node:{}#{n}", element_id,
   edge_index)` (distinct per incident edge, still traceable back to the
   node via its prefix), `label` from the node's `label` (falling back to
   its id), `uri = None` for v1 (no stable Playb00k deep-link scheme yet —
   that's the UX sub-project's job), `provenance = Some(provenance_from_anchor(anchor))`.

3. **The relation itself.** One `RequirementRelation` per `(rule doc,
   violated node)` pair: `source = doc.id`, `target = ` the violated node's
   canonical evidence id (or, if it produced zero `EvidenceRef`s because no
   incident edge carried provenance yet — expected to be rare once Section
   2's `SourceAnchor`-population prerequisite lands, not zero on day one —
   register a single provenance-less `EvidenceRef{provenance: None}` as a
   fallback so the relation still validates), `kind =
   RequirementRelationKind::Satisfies`, `authority =
   RelationAuthority::Inferred(NonAuthoritativeRelation{ confidence:
   result.confidence, rationale: <the Disposition::Violated reason, or
   "no violations found" for Satisfied>, evidence: <the EvidenceRefs from
   step 2>, model: ModelIdentity{ name: "kr0ki-rule-eval".into(), version:
   env!("CARGO_PKG_VERSION").into() } })`, `provenance = Provenance{
   source_uri: format!("rule-eval:{}", doc.id), artifact_sha256: None,
   locator: None }`.

Violations are visible immediately (returned in the recompute response,
Section 7) but do not silently become asserted/trusted relations — a human
explicitly calls `RequirementGraph::promote_relation`, exactly like every
other inferred relation in the existing ReqIF/Flexo work. No new promotion
*mechanism* is built.

**Open gap, flagged rather than silently assumed away:** recompute rebuilds
`RequirementGraph` fresh from the current `ModelSnapshot` + rule docs on
every call (Section 1, decision 5 — no new persistence layer). A promotion
therefore only lasts for the response it's made in unless something writes
it back to Flexo as a durable, asserted element — this spec does not design
that write-back. v1 scope: `promote_relation` is callable and its effect is
visible within one recompute/promote cycle (enough to demonstrate a
human-in-the-loop review interaction, e.g. from Playb00k), but *durable*
promotion persistence is out of scope here — see Section 10.

**Revision note:** the first draft of this section claimed `confidence`
and `evidence_nodes` "carried straight through" into
`RelationAuthority::Inferred` and that node provenance came from "each
violated element's `SourceAnchor`." Neither survived independent review
against the actual `ufo-types` source: the inferred-relation payload is
`NonAuthoritativeRelation{confidence, rationale, evidence: Vec<EvidenceRef>,
model: ModelIdentity}` (not a direct copy of `SatisfiesResult`), both
`rationale` and `model` are mandatory fields the original draft never
populated, and `OntologicalNode` has no `SourceAnchor` to read from at all
— only edges do. The design above is the corrected version.

## 6. JEV interface (designed now, not implemented)

`RuleBackend` (Section 4) is the seam. `RegorusBackend` is the only real
implementation in v1. Because the trait already returns `SatisfiesResult`, a
future `JevBackend` — calling a self-hosted, `api.typesafe.ai/v1/systemone`-
compatible endpoint (intent: Cloudflare-hosted, not the paid third-party
API; see the standing "self-hosted JEV pattern" direction) — requires no
shape change: JEV's `Noul` "unsure" answer *is* `Disposition::Unknown`,
JEV's calibrated confidence *is* `SatisfiesResult.confidence`. `RuleDoc`
already carries `backend: RuleBackendKind` (default `Rego`) so a project can
mix deterministic and JEV-scored rules once a second backend exists.

Once a `JevBackend` is built and proven end-to-end against a real project,
contributing it back to `awesome-jev-tools` (github.com/v-modal/
awesome-jev-tools) via a GitHub Action is a reasonable fast-follow — out of
scope for this spec, since v1 ships no real second backend to contribute.

## 7. API / MCP surface

- `kr0ki-server`: `POST /model/{project_id}/recompute` →
  `{ graph: SysGraph, violations: [{ rule_id, result: SatisfiesResult }] }`.
  One route, synchronous. No new persistence layer — results aren't stored
  server-side beyond the existing render cache; the `RequirementGraph`'s
  promoted/inferred relation state, living in Flexo, is the durable record.
- MCP: one new tool, `recompute_and_evaluate`, added to `McpTool::ALL`
  (`docs/TODO.md`'s existing "MCP requirements tools... do not hard-code a
  second dispatcher" item applies directly — no bespoke dispatch path).

## 8. Testing

- Unit: `rule_docs::extract_rule_docs` against fixture `ModelSnapshot`s;
  `RegorusBackend::evaluate` against fixture Rego policies + fixture
  `SysGraph`s (pass, fail, and malformed-policy cases) — no network.
- Integration: one `#[ignore]`d live test against a real Flexo project
  (gated on `KR0KI_SYSMLV2_BASE_URL`/`KR0KI_SYSMLV2_TOKEN`, matching the
  existing live-test convention) that authors 2-3 real rules, calls
  recompute, and asserts violations point at the right elements — this *is*
  the v1 acceptance bar from Section 1 item 6.

## 9. Error handling

- A single rule doc's Rego source failing to parse/compile →
  `Disposition::Unknown` for that rule only; recompute still returns results
  for every other rule and the graph itself. Never a 500 for one bad policy.
- Missing `SourceAnchor` provenance on a violated element → the violation is
  still returned (with an empty `evidence_nodes`) rather than dropped
  silently; this is expected to become rare once Section 2's prerequisite
  (SourceAnchor population on the SysML-v2 arm) lands, not something to
  design deeper error handling around now.
- Empty rule-doc set for a project → recompute still returns the graph,
  `violations: []` — never an error; a project with no rules is a valid
  state, not a misconfiguration.

## 10. Out of scope (explicitly, not forgotten)

- **Playb00k UI** — the recompute button, poll-while-open behavior, and
  violation rendering anchored to elements. This is sub-project "3. UX"
  (versioned vue-flow, copy/edit/save to Flexo), brainstormed separately.
- **Background poll-loop daemon** — `docs/TODO.md`'s "Commit poll loop" item
  stays open and undesigned; this spec's recompute is UX-polled, not
  server-scheduled.
- **`JevBackend` implementation** — interface only (Section 6); no code
  calls a JEV-shaped endpoint in this spec.
- **dbt as a graph source** (`PLAN-KR0KI-006` piece 1) — unaffected;
  whatever front-end produces a `SysGraph` (SysML today, dbt later) is
  interchangeable to this system by construction (Section 4 operates on
  `SysGraph`, not on SysML specifically).
- **Mandatory-anchor / constrained-graph mode** (Section 1, item 1's
  declined option) — stays a future opt-in, not built now.
- **`reqif-opa-mcp`'s own refactor** onto the upstream `ufo-types` contract
  (`docs/TODO.md`'s existing item) — unrelated to this work; that pipeline
  stays a separate artifact→document-graph→candidate→OPA→ReqIF concern.
- **Durable persistence of promoted relations** — Section 5's promotion
  step mutates an in-memory `RequirementGraph` that recompute rebuilds
  fresh on every call; writing a promotion back to Flexo as a durable,
  asserted element is not designed here. v1's promotion is only guaranteed
  to survive within one recompute/promote cycle.
- **`OntologicalNode` provenance** — if node-level `SourceAnchor` ever turns
  out to be needed beyond Section 5's per-incident-edge workaround, that is
  an upstream `ufo-types` change, raised as a new entry on `AGENTS.md` §9's
  "open decisions, not kr0ki's to make" list, not something this spec or
  its implementation plan decides unilaterally.
