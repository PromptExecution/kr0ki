# dbt manifest → canonical UFO graph (box-1 front-end)

**Parent:** [`PLAN-KR0KI-006-dbt-digital-thread.md`](../../PLAN-KR0KI-006-dbt-digital-thread.md)
(umbrella — this spec is its first of two tracked pieces; the second, the Flexo write
path, is tracked there and not designed here). **Reads against:**
[`DESIGN-NOTE-versioned-dialects-and-b00t-loader.md`](../../DESIGN-NOTE-versioned-dialects-and-b00t-loader.md)
(the `Upgrade`/`DialectUrn` contract this reuses),
[`DESIGN-NOTE-typed-model-layer.md`](../../DESIGN-NOTE-typed-model-layer.md) §2.4/§2.6/§2.8
(binding: `ElementId` identity convention, `SysGraph` as an acceptable typed envelope,
no `SchemaVersion` field on canonical types), `ufo-types/src/sysgraph.rs`,
`ufo-types/src/reqif.rs` (the adapter shape this mirrors).
**Owner:** PromptExecution (@elasticdotventures). **Created:** 2026-09-20.

**Status:** approved for implementation (brainstorm session, 2026-09-20) — proceed to
`writing-plans`.

---

## 0. One paragraph

Data infrastructure (dbt-modeled tables, columns, and their `ref()`/`source()`
lineage) becomes a second kind of box-1 front-end for kr0ki's five-box pipeline,
alongside the existing SysML-v2-API-client and Rust-source arms: a `dbt-manifest`
front-end parses dbt's own versioned `manifest.json` artifact and lowers it into the
same canonical `ufo_types::sysgraph::SysGraph` every other front-end produces, so
requirements, code, and data assets end up traceable in one graph — a real "digital
thread" — rather than three graphs an operator has to reconcile by hand. This spec
covers only that lowering step: bytes in, `SysGraph` out, in-memory and testable, the
same shape `ufo_types::reqif` already proved. Persisting the result into a durable
Flexo-backed model store is a separate, follow-on spec (§9).

## 1. Why this shape, not a standalone contract

`ufo_types::mbse::requirements::RequirementGraph` earned its own module because ReqIF
has real domain-specific semantics — baselines, promotion of inferred/proposed edges,
evidence, five induced viewpoints — that don't map onto SysML v2 cleanly. dbt's
vocabulary doesn't have that problem: a `model` is a data asset with an identity and a
schema, `ref()`/`source()` lineage is a dependency relation, both map onto SysML v2's
existing `PartUsage`/dependency vocabulary directly. Forcing dbt through a second
bespoke contract would just be a second graph to keep in sync with the first — the
opposite of the zero-drift goal. So: dbt lowers into `SysGraph`, full stop, the same
target every other box-1 front-end already lowers into.

## 2. Two-step versioning, not one `Upgrade` impl on `SysGraph`

Rust's coherence rules allow exactly one `impl Upgrade for SysGraph` in the whole
program. Kubernetes, Rust-source, and dbt will all eventually need to produce
`SysGraph` — none of them can individually own that impl. The fix is the same
two-step shape `ufo_types::reqif` already uses for ReqIF, applied here instead to
dbt's *own* versioned wire format:

```rust
// ufo_types::dbt (new module)

/// One parsed dbt manifest.json, at whatever historical schema version it
/// declared. This is the `Upgrade` target -- NOT SysGraph.
pub struct DbtManifest { /* nodes, sources, macros, depends_on -- see §3 */ }

impl Upgrade for DbtManifest {
    const DIALECT: DialectUrn = DialectUrn {
        crate_name: "dbt",
        path: "manifest",
        version: /* current: 12.0.0, mirroring dbt's own v12 schema */,
    };

    fn upgrade(bytes: &[u8], from_version: &semver::Version) -> Result<Self, DialectError> {
        // Historical dbt manifest schema versions (v9..v12 observed on
        // crates.io-adjacent dbt releases) differ in field shape; each
        // from_version this type supports gets its own match arm here,
        // converging to one DbtManifest. Major-mismatch (a hypothetical
        // future v2 wire-incompatible dbt manifest format) is rejected per
        // Upgrade's contract, not attempted.
    }
}

/// Ordinary lowering -- not Upgrade. Mirrors bundle_to_requirement_graph's
/// exact shape: a parsed intermediate in, the canonical graph out.
pub fn dbt_manifest_to_sysgraph(manifest: &DbtManifest, config: &DbtLiftConfig) -> SysGraph { .. }

/// Combined bytes -> SysGraph entry point, mirroring reqif::parse_and_lower.
pub fn parse_and_lift(bytes: &[u8], config: &DbtLiftConfig) -> Result<SysGraph, DbtLiftError> {
    let manifest = DbtManifest::upgrade(bytes, /* declared version, read from
        the manifest's own metadata.dbt_schema_version URL before full parse */)?;
    Ok(dbt_manifest_to_sysgraph(&manifest, config))
}
```

dbt's `metadata.dbt_schema_version` field is a URL
(`https://schemas.getdbt.com/dbt/manifest/v12.json`) that already encodes the exact
version being decoded — `from_version` for the `Upgrade` call is read from that field,
not guessed or defaulted.

## 3. `DbtManifest` — what's actually modeled

Deliberately narrow: only what's needed to produce nodes + lineage edges, not a full
dbt manifest reimplementation. From dbt's `nodes` map (filtered to
`resource_type` in `{model, source, seed, snapshot}` — tests and macros are excluded,
see §7 for why):

- `unique_id` (e.g. `model.jaffle_shop.stg_customers`) — becomes the `ElementId`.
- `resource_type`, `name`, `schema`, `database` — enough to label the node and
  distinguish a model from a source.
- `depends_on.nodes` — a list of other `unique_id`s this node references; becomes
  outgoing lineage edges.

Everything else in a real `manifest.json` (compiled SQL, raw SQL, descriptions,
column-level metadata, macros, docs blocks, exposures, selectors) is out of scope for
this first cut — extend `DbtManifest` when a real second consumer needs a specific
field, not speculatively (matches this crate's own stated `DataFormat` convention:
"deliberately small, grows only when a second real consumer needs a variant").

## 4. Node / edge mapping

- Each qualifying dbt node → `OntologicalNode { id, stereotype: UfoStereotype::Kind("DbtModel"), label: Some(name) }`.
  One stereotype for all four resource types in this first cut (source vs. model
  distinction lives in the `ElementId` prefix and a debug-only label suffix, not a
  second stereotype) — revisit only if a real consumer needs to filter by
  resource_type specifically.
- Each `depends_on.nodes` entry → `OntologicalEdge` using `UfoRelation::Requires`
  (`source = the node whose depends_on list this is`, `target = the referenced
  upstream node`) — its doc comment's own synonym list is literally "consumes,
  depends-on, needs"; no new relation variant needed. (Corrected 2026-09-20: this
  spec originally named a `DependsOn` variant that does not exist in
  `ontology::UfoRelation`'s real 25-member vocabulary — `Requires` is the actual
  match, found while writing the implementation plan.)
- No new types added to `ontology.rs` or `sysgraph.rs`. This spec is additive-only at
  the `ufo_types::dbt` module level.

## 5. Zero-drift identity, not a new provenance field

`OntologicalNode` has no provenance field today (only `OntologicalEdge` carries
`SourceAnchor`). Rather than adding one — which would touch a type every other box-1
front-end also produces — this reuses `ElementId`'s own documented convention
(`DESIGN-NOTE-typed-model-layer.md` §2.4: *"a content hash or git blob/commit hash MAY
back the string where an element needs a stable derived identity"*): every dbt-derived
`ElementId` is `format!("dbt:{unique_id}")`, e.g. `dbt:model.jaffle_shop.stg_customers`.

This is the whole zero-drift mechanism for this spec's scope: a re-ingest of the same
manifest always produces the same `ElementId`s for the same dbt nodes (deterministic,
not content-hash-derived, since dbt's own `unique_id` is already stable across
recompiles), and any future reconciliation pass (§9, out of scope here) can safely
diff/replace everything under the `dbt:` prefix without touching anything else in the
graph — human-authored or another front-end's elements never carry that prefix.

## 6. Error handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum DbtLiftError {
    #[error("could not parse manifest.json: {0}")]
    Malformed(#[from] serde_json::Error),
    #[error(transparent)]
    Dialect(#[from] DialectError),
    #[error("dependency edge {from} -> {to} references a node not present in this manifest")]
    DanglingDependency { from: String, to: String },
}
```

`DanglingDependency` is a real possibility (a manifest can reference a node type this
spec doesn't model, e.g. a macro or an exposure) — downgrade to a skip-and-warn rather
than a hard error if that turns out to be common in real fixtures; decide with
evidence from the test corpus (§8), not up front.

## 7. Explicit non-goals

- No test/macro/exposure/selector nodes — only `model`/`source`/`seed`/`snapshot`.
- No column-level lineage — node-level only, matching what `SysGraph`/`UfoRelation`
  already support without new types. Column-level is a real future need but needs its
  own design (likely a new, narrower `UfoRelation` or a sub-element concept) — not
  bolted on here speculatively.
- No Flexo write path — §9.
- No `b00t://` loader / live acquisition — this spec takes `bytes: &[u8]` exactly like
  `reqif::parse_and_lift`; whoever calls it (kr0ki-core, a future CLI, a future MCP
  tool) owns acquisition, per the same acquisition-neutral split
  `reqif_import.rs` already established. Not building the URI resolution layer here.

## 8. Test plan

Golden-fixture discipline, matching `reqif`'s bar:

- A real (small, hand-trimmed) `manifest.json` fixture per supported dbt schema
  version, each exercising: a model with no deps, a model depending on another model,
  a model depending on a source.
- `parse_and_lift` round-trip: fixture bytes → `SysGraph`, assert exact node/edge
  counts and `ElementId` values.
- `DbtManifest::upgrade` per historical version → same canonical shape (the
  cross-version compatibility `Upgrade`'s contract requires).
- Malformed JSON → `DbtLiftError::Malformed`, not a panic.
- A manifest with zero qualifying nodes (all tests/macros) → empty `SysGraph`, not an
  error.

## 9. Follow-on spec (tracked, not designed here)

**Flexo write path** — how `kr0ki-sysmlv2-client` (currently read-only: `ListModelProjects`,
`ListModelCommits`, `GetModelSnapshot`) commits `SysGraph` elements into a live Flexo
project, and how the `dbt:`-prefixed `ElementId` reconciliation described in §5
actually applies against Flexo's commit history (diff against the previous commit's
`dbt:`-prefixed elements, commit adds/updates/removals, leave everything else
untouched). Needs its own design pass — this spec's job ends at a `SysGraph` value in
memory.
