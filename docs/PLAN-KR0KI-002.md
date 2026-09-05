# PLAN-KR0KI-002 — the SysML-model ingestion path

**Parent:** [`PRD-KR0KI-001-foundational.md`](PRD-KR0KI-001-foundational.md) (FR1 / FR3 /
FR4 / FR5-model). **Typed-layer shape:**
[`DESIGN-NOTE-typed-model-layer.md`](DESIGN-NOTE-typed-model-layer.md).
**Owner:** PromptExecution (@elasticdotventures). **Created:** 2026-09-05.

**Status:** the **client path is unblocked** by the operator on **2026-09-05** — decision
**D3** ("where does the typed layer live") is resolved in favour of *kr0ki consumes an
upstream OMG-API model server; it does not host the model*. The
`sysml-derive` **authoring** path stays **blocked on D1** (D6 — vocabulary — resolved
2026-09-05, [`VOCABULARY.md`](VOCABULARY.md)). This plan covers ingestion + view rendering + caching of a model kr0ki reads from an
external server. It does **not** cover kr0ki emitting SysML v2 / KerML text.

---

## 0. One paragraph

kr0ki ingests a validated SysML v2 model by reading it, per historical commit, from an
external server implementing the **OMG Systems Modeling API and Services** (Flexo
`flexo-mms-sysmlv2` the primary target). Each `(projectId, commitId)` is pulled into a
content-hashed `ModelSnapshot` by `kr0ki-sysmlv2-client` (this PR). That snapshot is
**not** lowered straight to diagram syntax. It feeds a **five-box layered pipeline**
(§2) whose pivot is a **canonical UFO-typed semantic graph**: kr0ki renderer adapters
sit at the far end, and everything they draw is *derived from* the UFO graph, never
inferred from diagram syntax or from raw `iso_ir` strings.

---

## 1. Model source

- **Any** server implementing the OMG Systems Modeling API PSM: Flexo
  `flexo-mms-sysmlv2`, the OMG Java pilot `Systems-Modeling/SysML-v2-API-Services`,
  `Open-MBEE/OpenSysML`, Eclipse SysON (read-mostly).
- **Primary integration target: Flexo `flexo-mms-sysmlv2`** — see
  [`EVAL-flexo.md`](EVAL-flexo.md). RDF/Fuseki-backed, Apache-2.0, JPL-led, OMG PSM
  **partial** (read/navigate core present; Diff/Merge/Meta/`changes` stubbed;
  `previousCommit` null).
- kr0ki holds the client (`kr0ki-sysmlv2-client`) and **not** the model. This is the D3
  resolution: no typed model layer is defined or hosted inside kr0ki.

## 2. Ingest pipeline — the five-box layered chain

kr0ki does **not** infer architecture (in particular Kubernetes topology) directly from
diagram syntax or from raw `iso_ir` classification strings. The **canonical UFO
semantic graph is the pivot**; every layer downstream of it is *derived*, not
re-inferred.

```
┌─ 1 ─────────────────────────────┐
│  Kubernetes / Rust / SysML-v2   │   source front-ends (siblings)
│  source                         │
└────────────┬────────────────────┘
             │   (this PR feeds the SysML-v2 arm: ModelSnapshot, a validated model
             │    pulled from an OMG-API server. Rust-source and Kubernetes-source
             │    are sibling front-ends into the same graph — named here, out of
             │    scope for this PR.)
             ▼
┌─ 2 ─────────────────────────────┐
│  canonical UFO-typed semantic   │   ufo_types:
│  graph                          │     - UfoStereotype on nodes            (exists)
│                                 │     - UfoRelation on edges              (ufo-types PR, IN FLIGHT)
│                                 │     - OntologicalEdge                   (ufo-types PR, IN FLIGHT)
│                                 │     - temporal extent, provenance
│                                 │   Defined in ufo-types, NOT in kr0ki.
└────────────┬────────────────────┘
             ▼
┌─ 3 ─────────────────────────────┐
│  pattern recognizers            │   domain rule sets that lift the UFO graph into
│                                 │   SysML constructs. First target: the Kubernetes
│                                 │   recognizer — see §2.1 and docs/PATTERNS-kubernetes.md.
└────────────┬────────────────────┘
             ▼
┌─ 4 ─────────────────────────────┐
│  SysML v2 model constructs      │   ufo_types::sysml_model::{ElementKind, Relation}
│  (KerML abstract syntax)        │   — MERGED (ufo-types#20 / v0.12.0). DOWNSTREAM of
│                                 │   the UFO graph. A `ViewDefinition` here is one
│                                 │   ElementKind, not the layer's name (D6).
└────────────┬────────────────────┘
             ▼
┌─ 5 ─────────────────────────────┐
│  kr0ki renderer adapters        │   DiagramFormat / isometric / Mermaid / D2
│                                 │   (FR1, FR3, FR4). kr0ki owns only this box and
│                                 │   the source-side client (box 1, SysML-v2 arm).
└─────────────────────────────────┘
```

**Fast-path leaf (NOT part of the pipeline):** a sandboxed `KubeDiagramsBackend`
(`RenderBackend`) may render Kubernetes YAML bundles straight to SVG via
`philippemerle/KubeDiagrams` for the "just draw my cluster" case, behind a distinct
route/format. It bypasses boxes 2-4 by design and must be labelled as a leaf feature,
never sold as the SysML path. Content-address the normalised `dot_json`, pin Graphviz,
never expose KubeDiagrams' `-c` (arbitrary Python). See
[`EVAL-kubediagrams.md`](EVAL-kubediagrams.md).

### 2.1 First pattern recognizer: Kubernetes

The Kubernetes recognizer (box 3) lifts a UFO semantic graph derived from cluster
state / manifests into SysML constructs using the UFO 4-category split. Its extraction
ruleset is **ported from `philippemerle/KubeDiagrams`' `kube-diagrams.yaml`** (Apache-2.0)
— a ~51-kind GVK→relationship catalogue — keeping the per-JSONPath relation distinction
KubeDiagrams collapses at render time. See [`EVAL-kubediagrams.md`](EVAL-kubediagrams.md)
§2(b)/§3.

- **Endurants** (things that persist through time): Cluster, Node, Namespace, Pod,
  Service, ConfigMap.
- **Perdurants** (things that happen / unfold in time): reconciliation, scheduling,
  rollout, request-handling, failure, recovery.
- **Moments** (existentially-dependent truth-makers): health, readiness,
  ownership-binding, policy-applicability.
- **Abstracts** (non-spatiotemporal): selectors, constraints, quantities.

The full `k8s → UFO stereotype + relation-normalization` table (external consultant
input) lives in [`docs/PATTERNS-kubernetes.md`](PATTERNS-kubernetes.md) — the endurant/
perdurant/moment/abstract bridge, the 25-row canonical relationship vocabulary with its
Kubernetes synonyms, the explicit non-conflations, and the concept→`UfoStereotype`
table. Not duplicated here.

### 2.2 Where this PR's `ModelSnapshot` sits

`kr0ki-sysmlv2-client` → `ModelSnapshot { project_id, commit_id, elements, roots,
content_hash }` is the **SysML-v2 source arm of box 1**. It is a validated model
snapshot, content-hashed (deterministic SHA-256; no OMG-API server exposes its own —
verified in `crates/kr0ki-sysmlv2-client/tests/client.rs`). It is the input to the UFO
graph builder (box 2), which is `ufo-types`' responsibility, not kr0ki's.

## 3. Cache key — model path

Extends P0's `cache_key` (`kr0ki-core::cache`):

```
model_cache_key = SHA256( "kr0ki/v1"
                        ‖ view_kind                       // ufo_types::SysmlViewKind
                        ‖ notation                        // DiagramFormat: mermaid|d2|…
                        ‖ ModelSnapshot.content_hash )     // from kr0ki-sysmlv2-client
```

`ModelSnapshot.content_hash` is
`SHA256("kr0ki-sysmlv2/v1" ‖ per-element(@id, canonical fields incl. @type) ‖ 0x1e ‖ sorted root ids)`
— deterministic and order-independent. Because `(projectId, commitId)` is immutable on
the server, a model diagram for a given commit is renderable once and cacheable forever
(PRD §3). When the UFO-graph / recognizer layers land, the key folds in a hash of the
recognizer rule-set version so a recognizer change invalidates derived views.

## 4. Conformance

The `Systems-Modeling/SysML-v2-Release` corpus, exercised through `sysml-v2-parser`
(version pinned in lockstep with `ufo-types`, per the design note §2.7). Landed on
`main` (`kr0ki#5`): validation corpus 56/56, standard library 94/94.

**Kubernetes recognizer oracle:** once the box-3 recognizer exists, diff its topology
output against `KubeDiagrams`' `dot_json` over the KubeDiagrams `examples/` corpus
(argo / istio / cert-manager / kube-prometheus-stack / online-boutique) in CI — a
topology-recall regression signal. See [`EVAL-kubediagrams.md`](EVAL-kubediagrams.md) §2(c).

## 5. FR-by-FR status

| FR | What | Status |
|---|---|---|
| **FR1** | `iso_ir` graph JSON → Mermaid + D2 | ◑ **blocked on the UFO semantic-graph layer (`ufo-types`, in flight) + a pattern recognizer.** No direct `iso_ir`→diagram lowering — it must route through boxes 2→3→4. Client + `ModelSnapshot` (box 1 arm) done. |
| **FR3** | `systhread-core` isometric layout JSON → its `render.rs` | unchanged — separate renderer (box 5), not on the UFO-graph critical path |
| **FR4** | typed SysML-v2/KerML view model → per-`ViewDefinition` rendering | ◑ **blocked on the UFO semantic-graph layer (`ufo-types`, in flight) + a pattern recognizer.** The view consumes box 4 (`ufo_types::sysml_model::{ElementKind, Relation}`, merged), which is itself derived from the UFO graph via a recognizer — not lowered directly from `ModelSnapshot`. |
| **FR5** (model) | content-address the model render | ◑ hash done (`ModelSnapshot.content_hash`); key extension §3 TBD; CDN tier = **D5** |
| **FR6** | intra-ecosystem reference resolver | unchanged — needs `ledgrrr` |
| **FR7** | caller auth on the service | unchanged — P0 gap, tracked in PRD §6.4 |

## 6. Open decisions still in force

- **D1** — `sysml-derive` extend-vs-wrap-vs-re-export. **Only matters if kr0ki ever
  *emits* SysML v2 / KerML text.** The read/ingest path in this plan does not, so D1
  does not block ingestion — it blocks the authoring adapter.
- **D5** — CDN (Cloudflare R2 + Workers). Blocks the cache *tier*, not the key
  derivation.
- ~~**D6**~~ — view / viewpoint / projection / thread vocabulary. **RESOLVED
  2026-09-05** ([`VOCABULARY.md`](VOCABULARY.md)): OMG SysML v2 / KerML spec terms
  verbatim, no informal synonyms, "digital thread" always qualified, "projection"
  banned. `ufo_types::sysml_model` / `view` / `ontology` are already compliant.

`D2` (holon-viz as a real dep), `D3` (typed-layer home) and `D6` (vocabulary) are **no
longer blocking**: D3 is resolved (kr0ki consumes, does not host), D6 is resolved
(spec vocabulary verbatim), and D2 only bites the `CytoscapeGraph` wrapping question
which this ingestion path does not touch. **D1** (`sysml-derive` posture) remains the
only §5 decision that gates the authoring path.

### New upstream dependency introduced by the five-box pipeline

The UFO semantic-graph layer (box 2 — `ufo_types`' `UfoRelation` / `OntologicalEdge` +
temporal extent + provenance) is **in flight in a `ufo-types` PR**. FR1 and FR4 are
gated on it landing **and** on at least one pattern recognizer (box 3) existing. kr0ki
defines none of these types; it consumes `ufo-types` once they merge.

---

*Cross-reference: [`DESIGN-NOTE-typed-model-layer.md`](DESIGN-NOTE-typed-model-layer.md)
for the reviewed (still unapproved) shape of a typed layer, should the authoring path
later be unblocked; [`PRD-KR0KI-001-foundational.md`](PRD-KR0KI-001-foundational.md) as
the parent requirements; [`PATTERNS-kubernetes.md`](PATTERNS-kubernetes.md) for the
first pattern recognizer's mapping table (stub).*
