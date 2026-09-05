# PLAN-KR0KI-002 — the SysML-model ingestion path

**Parent:** [`PRD-KR0KI-001-foundational.md`](PRD-KR0KI-001-foundational.md) (FR1 / FR3 /
FR4 / FR5-model). **Typed-layer shape:**
[`DESIGN-NOTE-typed-model-layer.md`](DESIGN-NOTE-typed-model-layer.md).
**Owner:** PromptExecution (@elasticdotventures). **Created:** 2026-09-05.

**Status:** the **client path is unblocked** by the operator on **2026-09-05** — decision
**D3** ("where does the typed layer live") is resolved in favour of *kr0ki consumes an
upstream OMG-API model server; it does not host the model*. The
`ViewpointDefinition` / `sysml-derive` **authoring** path stays **blocked on D1 and
D6**. This plan therefore covers only ingestion + projection + caching of a model kr0ki
reads from an external server. It does **not** cover kr0ki emitting SysML v2 / KerML
text.

---

## 0. One paragraph

kr0ki ingests a validated SysML v2 model by reading it, per historical commit, from an
external server implementing the **OMG Systems Modeling API and Services** (Flexo
`flexo-mms-sysmlv2` the primary target). Each `(projectId, commitId)` is pulled into a
content-hashed `ModelSnapshot` by `kr0ki-sysmlv2-client` (this PR). A later increment
lowers that snapshot through an adapter into `iso_ir` `Node`/`Edge` JSON (for **FR1** —
Mermaid / D2) and into per-view projections keyed by view kind (for **FR4**), then
renders and caches them with a key that extends P0's `cache_key` with the snapshot's
content hash.

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

## 2. Ingest pipeline

```
kr0ki-sysmlv2-client                (this PR — done)
   projects / commits / elements / roots / relationships
        │
        ▼
ModelSnapshot { project_id, commit_id, elements: Vec<Element>, roots: Vec<String>,
                content_hash }        (this PR — done; deterministic SHA-256)
        │
        ▼
adapter  (TBD — next increment; D1/D6-gated for anything that emits SysML v2 text)
        ├──► iso_ir Node/Edge JSON  +  Mermaid / D2         → FR1
        │       (follows b00t-cli/src/dispatch_sysml.rs conventions:
        │        edge_type "sequence" etc.)
        └──► per-view projection, keyed by ufo_types::SysmlViewKind → FR4
                (projection uses ViewUsage.exposedElement from the model where present —
                 see EVAL-syson.md — rather than reinventing scoping heuristics)
```

The **adapter is the real remaining design work**, and it is gated: the moment it needs
to *emit* SysML v2 / KerML concrete text it hits **D1** (`sysml-derive` posture) and
**D6** (view/viewpoint vocabulary). The pure `ModelSnapshot → iso_ir Node/Edge` lowering
for FR1 does **not** emit SysML text and could proceed first.

## 3. Cache key — model path

Extends P0's `cache_key` (`kr0ki-core::cache`):

```
model_cache_key = SHA256( "kr0ki/v1"
                        ‖ view_kind                       // e.g. SysmlViewKind::Interconnection
                        ‖ notation                        // DiagramFormat: mermaid|d2|…
                        ‖ ModelSnapshot.content_hash )     // from kr0ki-sysmlv2-client
```

`ModelSnapshot.content_hash` is itself
`SHA256("kr0ki-sysmlv2/v1" ‖ per-element(@id, canonical fields incl. @type) ‖ 0x1e ‖ sorted root ids)`
— deterministic and order-independent (verified: see
`crates/kr0ki-sysmlv2-client/tests/client.rs`). Because `(projectId, commitId)` is
immutable on the server, a model diagram for a given commit is renderable once and
cacheable forever (PRD §3).

## 4. Conformance

The `Systems-Modeling/SysML-v2-Release` corpus, exercised through `sysml-v2-parser`
(version pinned in lockstep with `ufo-types`, per the design note §2.7). Lands as a
**separate PR on branch `feat/conformance-harness`** — not in scope here.

## 5. FR-by-FR status

| FR | What | Status |
|---|---|---|
| **FR1** | `iso_ir` graph JSON → Mermaid + D2 | ◑ client + `ModelSnapshot` done; **adapter TBD** |
| **FR3** | `systhread-core` isometric layout JSON → its `render.rs` | unchanged — separate renderer, not touched by this path |
| **FR4** | typed SysML-v2/KerML view model → per-view projection | ◑ needs `ufo_types::SysmlViewKind` (landing in `ufo-types`, **separate PR**) + the projection step |
| **FR5** (model) | content-address the model render | ◑ hash done (`ModelSnapshot.content_hash`); key extension §3 TBD; CDN tier = **D5** |
| **FR6** | intra-ecosystem reference resolver | unchanged — needs `ledgrrr` |
| **FR7** | caller auth on the service | unchanged — P0 gap, tracked in PRD §6.4 |

## 6. Open decisions still in force

- **D1** — `sysml-derive` extend-vs-wrap-vs-re-export. **Only matters if kr0ki ever
  *emits* SysML v2 / KerML text.** The read/ingest path in this plan does not, so D1
  does not block ingestion — it blocks the authoring adapter.
- **D5** — CDN (Cloudflare R2 + Workers). Blocks the cache *tier*, not the key
  derivation.
- **D6** — view / viewpoint / projection / thread vocabulary. Mitigated here by using
  the OMG spec names (`SysmlViewKind` variants = spec view names) so the b00t-surface
  naming decision can land later without a breaking rename.

`D2` (holon-viz as a real dep) and `D3` (typed-layer home) are **no longer blocking**:
D3 is resolved (kr0ki consumes, does not host), and D2 only bites the `CytoscapeGraph`
wrapping question which this ingestion path does not touch.

---

*Cross-reference: [`DESIGN-NOTE-typed-model-layer.md`](DESIGN-NOTE-typed-model-layer.md)
for the reviewed (still unapproved) shape of a typed layer, should the authoring path
later be unblocked; [`PRD-KR0KI-001-foundational.md`](PRD-KR0KI-001-foundational.md) as
the parent requirements.*
