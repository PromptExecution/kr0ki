# dbt Manifest → SysGraph Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `ufo_types::dbt` module that parses a dbt `manifest.json` artifact and
lowers it into `ufo_types::sysgraph::SysGraph` — the same canonical graph every other
box-1 front-end produces.

**Architecture:** Two-step, mirroring `ufo_types::reqif`'s exact shape: `DbtManifest`
implements `dialect::Upgrade` (decodes dbt's own versioned wire format), then an
ordinary lowering function (`dbt_manifest_to_sysgraph`) maps the decoded manifest onto
`SysGraph` nodes/edges. A combined `parse_and_lift(bytes, config) -> SysGraph` entry
point chains both steps, mirroring `reqif::parse_and_lower`.

**Tech Stack:** Rust, `serde`/`serde_json` (already a dependency — no new crate
needed), the `semver` crate (already added for `dialect::DialectUrn`).

**Spec:** [`docs/superpowers/specs/2026-09-20-dbt-manifest-digital-thread-design.md`](../specs/2026-09-20-dbt-manifest-digital-thread-design.md)
(kr0ki repo). **All file paths below are in the `ufo-types` repo**
(`~/promptexecution/ufo-types`, currently `main` at `ee873488`), not kr0ki.

## Global Constraints

- No new external dependency — `serde_json` and `semver` are already present.
- No feature flag — unlike `reqif`'s `reqrs` dependency, nothing here is a
  young/small-bus-factor crate, so this module is unconditional (`pub mod dbt;`
  in `src/lib.rs`, no `#[cfg(feature = ...)]`).
- No new types added to `ontology.rs` or `sysgraph.rs` — additive-only at the
  `ufo_types::dbt` module level (spec §4).
- Node identity is `dbt:{unique_id}` (spec §5) — never a new provenance field.
- Only `model`/`seed`/`snapshot`/`source` resource types are lowered; everything
  else (tests, macros, exposures, docs, selectors) is out of scope (spec §3, §7).
- First cut supports dbt manifest schema **v12 only** (current as of this plan).
  `Upgrade`'s multi-version machinery is structurally in place (the trait, the
  major-mismatch rejection path); back-filling v9–v11 match arms is deferred until
  a real consumer needs an older manifest — this codebase's own stated convention
  ("grows only when a second real consumer needs it, not speculatively").
- `DanglingDependency` (a `depends_on` edge referencing a node this module doesn't
  model, e.g. a macro) is a hard error in this first cut, not skip-and-warn — the
  spec (§6) left this as an explicit "decide with evidence" call; no real corpus
  exists yet, so the conservative/simplest choice is made now and can be relaxed
  later once real fixtures show it's too strict.

---

## Task 1: `DbtManifest` parsing

**Files:**
- Create: `src/dbt.rs`
- Modify: `src/lib.rs` (add `pub mod dbt;`, alphabetically after `data_format` and
  before `dialect` — matches this file's existing alphabetical module ordering)
- Test: inline `#[cfg(test)] mod tests` in `src/dbt.rs`

**Interfaces:**
- Produces: `pub struct DbtManifest { pub metadata: DbtManifestMetadata, pub nodes: std::collections::BTreeMap<String, DbtNode>, pub sources: std::collections::BTreeMap<String, DbtNode> }`,
  `pub struct DbtManifestMetadata { pub dbt_schema_version: String }`,
  `pub struct DbtNode { pub unique_id: String, pub resource_type: String, pub name: String, pub schema: Option<String>, pub database: Option<String>, pub depends_on: DbtDependsOn }`,
  `pub struct DbtDependsOn { pub nodes: Vec<String> }` — all `#[derive(Debug, Clone, serde::Deserialize)]`,
  fields default-empty where noted below.

- [ ] **Step 1: Write the failing test**

```rust
// src/dbt.rs (new file)

//! dbt manifest.json -> ufo_types::sysgraph::SysGraph (kr0ki digital-thread box-1
//! front-end). See kr0ki's docs/superpowers/specs/2026-09-20-dbt-manifest-digital-
//! thread-design.md for the full design.

use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct DbtManifestMetadata {
    pub dbt_schema_version: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DbtDependsOn {
    #[serde(default)]
    pub nodes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DbtNode {
    pub unique_id: String,
    pub resource_type: String,
    pub name: String,
    #[serde(default)]
    pub schema: Option<String>,
    #[serde(default)]
    pub database: Option<String>,
    #[serde(default)]
    pub depends_on: DbtDependsOn,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DbtManifest {
    pub metadata: DbtManifestMetadata,
    #[serde(default)]
    pub nodes: BTreeMap<String, DbtNode>,
    #[serde(default)]
    pub sources: BTreeMap<String, DbtNode>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
        "metadata": {"dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json"},
        "nodes": {
            "model.jaffle_shop.stg_customers": {
                "unique_id": "model.jaffle_shop.stg_customers",
                "resource_type": "model",
                "name": "stg_customers",
                "schema": "staging",
                "database": "analytics",
                "depends_on": {"nodes": ["source.jaffle_shop.raw.customers"]}
            },
            "model.jaffle_shop.customers": {
                "unique_id": "model.jaffle_shop.customers",
                "resource_type": "model",
                "name": "customers",
                "schema": "marts",
                "database": "analytics",
                "depends_on": {"nodes": ["model.jaffle_shop.stg_customers"]}
            }
        },
        "sources": {
            "source.jaffle_shop.raw.customers": {
                "unique_id": "source.jaffle_shop.raw.customers",
                "resource_type": "source",
                "name": "customers",
                "schema": "raw",
                "database": "analytics"
            }
        }
    }"#;

    #[test]
    fn parses_metadata_nodes_and_sources() {
        let manifest: DbtManifest = serde_json::from_str(FIXTURE).unwrap();
        assert_eq!(
            manifest.metadata.dbt_schema_version,
            "https://schemas.getdbt.com/dbt/manifest/v12.json"
        );
        assert_eq!(manifest.nodes.len(), 2);
        assert_eq!(manifest.sources.len(), 1);

        let stg = &manifest.nodes["model.jaffle_shop.stg_customers"];
        assert_eq!(stg.resource_type, "model");
        assert_eq!(stg.depends_on.nodes, vec!["source.jaffle_shop.raw.customers"]);

        let src = &manifest.sources["source.jaffle_shop.raw.customers"];
        assert_eq!(src.resource_type, "source");
        assert!(src.depends_on.nodes.is_empty());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd ~/promptexecution/ufo-types && cargo test -p ufo-types dbt::tests::parses_metadata`
Expected: FAIL with "module `dbt` not found" — `src/lib.rs` doesn't declare it yet.

- [ ] **Step 3: Wire the module in**

In `src/lib.rs`, find `pub mod data_format;` and add immediately after it:

```rust
pub mod dbt;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd ~/promptexecution/ufo-types && cargo test -p ufo-types dbt::tests::parses_metadata`
Expected: PASS (1 passed)

- [ ] **Step 5: Commit**

```bash
cd ~/promptexecution/ufo-types
git add src/dbt.rs src/lib.rs
git commit -m "feat(dbt): parse manifest.json metadata/nodes/sources"
```

---

## Task 2: `Upgrade` impl + version peeking

**Files:**
- Modify: `src/dbt.rs`

**Interfaces:**
- Consumes: `dialect::{DialectUrn, DialectError, Upgrade}` (already in `ufo-types`,
  landed this session — `src/dialect.rs`), `DbtManifest`/`DbtManifestMetadata` from
  Task 1.
- Produces: `impl Upgrade for DbtManifest`, `fn peek_dbt_schema_version(bytes: &[u8]) -> Result<semver::Version, DbtLiftError>`
  (used by Task 4's `parse_and_lift`), `pub enum DbtLiftError` (grown across this
  task and Task 4 — this task adds its first two variants).

- [ ] **Step 1: Write the failing test**

Add to `src/dbt.rs`, above the existing `#[cfg(test)] mod tests`:

```rust
use crate::dialect::{DialectError, DialectUrn, Upgrade};

#[derive(Debug, thiserror::Error)]
pub enum DbtLiftError {
    #[error("could not parse manifest.json: {0}")]
    Malformed(#[from] serde_json::Error),
    #[error(transparent)]
    Dialect(#[from] DialectError),
}

/// Extract the schema version dbt itself publishes in every manifest, e.g.
/// "https://schemas.getdbt.com/dbt/manifest/v12.json" -> 12.0.0. Doesn't fully
/// parse the manifest -- callers use this to know which `Upgrade::upgrade`
/// call to make before paying for the full deserialize.
fn peek_dbt_schema_version(bytes: &[u8]) -> Result<semver::Version, DbtLiftError> {
    #[derive(Deserialize)]
    struct MetaOnly {
        metadata: DbtManifestMetadata,
    }
    let meta: MetaOnly = serde_json::from_slice(bytes)?;
    let url = &meta.metadata.dbt_schema_version;
    let major: u64 = url
        .rsplit('/')
        .next()
        .and_then(|last| last.strip_prefix('v'))
        .and_then(|v| v.strip_suffix(".json"))
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| {
            DbtLiftError::Dialect(DialectError::Malformed {
                dialect: Box::new(DbtManifest::DIALECT),
                from_version: semver::Version::new(0, 0, 0),
                reason: format!("could not extract a version number from '{url}'"),
            })
        })?;
    Ok(semver::Version::new(major, 0, 0))
}

impl Upgrade for DbtManifest {
    const DIALECT: DialectUrn = DialectUrn {
        crate_name: "dbt",
        path: "manifest",
        version: semver::Version::new(12, 0, 0),
    };

    fn upgrade(bytes: &[u8], from_version: &semver::Version) -> Result<Self, DialectError> {
        if from_version.major != Self::DIALECT.version.major {
            return Err(DialectError::MajorMismatch {
                from_version: from_version.clone(),
                dialect: Box::new(Self::DIALECT),
                expected_major: Self::DIALECT.version.major,
            });
        }
        serde_json::from_slice(bytes).map_err(|error| DialectError::Malformed {
            dialect: Box::new(Self::DIALECT),
            from_version: from_version.clone(),
            reason: error.to_string(),
        })
    }
}
```

Add to `#[cfg(test)] mod tests`:

```rust
#[test]
fn peeks_schema_version_from_metadata() {
    let version = peek_dbt_schema_version(FIXTURE.as_bytes()).unwrap();
    assert_eq!(version, semver::Version::new(12, 0, 0));
}

#[test]
fn upgrade_succeeds_for_matching_major() {
    let version = semver::Version::new(12, 0, 0);
    let manifest = DbtManifest::upgrade(FIXTURE.as_bytes(), &version).unwrap();
    assert_eq!(manifest.nodes.len(), 2);
}

#[test]
fn upgrade_rejects_major_mismatch_without_attempting_decode() {
    let version = semver::Version::new(9, 0, 0);
    let err = DbtManifest::upgrade(FIXTURE.as_bytes(), &version).unwrap_err();
    assert!(matches!(err, DialectError::MajorMismatch { expected_major: 12, .. }));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd ~/promptexecution/ufo-types && cargo test -p ufo-types dbt::tests`
Expected: compile error — `peek_dbt_schema_version` etc. exist already from Step 1's
same edit, so instead run before adding the `impl Upgrade` block to confirm the
*trait* isn't satisfied yet if you're doing this test-first at finer grain; otherwise
proceed straight to Step 3 since this task's Step 1 already includes the
implementation inline (parsing + version-peeking is simple enough that TDD-ing the
plumbing separately from the three-line body adds no signal — the three tests above
are the real spec).

- [ ] **Step 3: Run tests to verify they pass**

Run: `cd ~/promptexecution/ufo-types && cargo test -p ufo-types dbt::tests`
Expected: PASS (4 passed — the Task 1 test plus these three)

- [ ] **Step 4: Commit**

```bash
cd ~/promptexecution/ufo-types
git add src/dbt.rs
git commit -m "feat(dbt): Upgrade impl + schema-version peeking"
```

---

## Task 3: Node lowering

**Files:**
- Modify: `src/dbt.rs`

**Interfaces:**
- Consumes: `sysgraph::{SysGraph, OntologicalNode}`, `stereotype::UfoStereotype`,
  `sysml_model::ElementId` (all existing `ufo-types` public API), `DbtManifest`/
  `DbtNode` from Task 1.
- Produces: `pub struct DbtLiftConfig` (empty for now — a placeholder for future
  options, e.g. a project-name prefix; Task 5's tests construct it as
  `DbtLiftConfig::default()`), `fn lower_nodes(manifest: &DbtManifest, graph: &mut SysGraph)`
  (private — `dbt_manifest_to_sysgraph` in Task 4 is the public surface; this is
  tested indirectly through it in Task 4, and directly here to keep this task's
  diff reviewable on its own).

- [ ] **Step 1: Write the failing test**

Add to `src/dbt.rs`:

```rust
use crate::sysgraph::{OntologicalNode, SysGraph};
use crate::sysml_model::ElementId;
use crate::stereotype::UfoStereotype;

#[derive(Debug, Clone, Default)]
pub struct DbtLiftConfig {}

fn dbt_element_id(unique_id: &str) -> ElementId {
    ElementId::new(format!("dbt:{unique_id}"))
}

fn lower_nodes(manifest: &DbtManifest, graph: &mut SysGraph) {
    let qualifying = manifest
        .sources
        .values()
        .chain(manifest.nodes.values().filter(|n| {
            matches!(n.resource_type.as_str(), "model" | "seed" | "snapshot")
        }));
    for node in qualifying {
        graph.push_node(OntologicalNode::with_label(
            dbt_element_id(&node.unique_id),
            UfoStereotype::Kind("DbtModel".into()),
            node.name.clone(),
        ));
    }
}
```

Add to `#[cfg(test)] mod tests`:

```rust
#[test]
fn lowers_models_and_sources_but_not_other_resource_types() {
    let manifest: DbtManifest = serde_json::from_str(FIXTURE).unwrap();
    let mut graph = SysGraph::new();
    lower_nodes(&manifest, &mut graph);

    assert_eq!(graph.nodes.len(), 3, "2 models + 1 source");
    let ids: Vec<&str> = graph.nodes.iter().map(|n| n.id.0.as_str()).collect();
    assert!(ids.contains(&"dbt:model.jaffle_shop.stg_customers"));
    assert!(ids.contains(&"dbt:model.jaffle_shop.customers"));
    assert!(ids.contains(&"dbt:source.jaffle_shop.raw.customers"));
}

#[test]
fn excludes_non_qualifying_resource_types() {
    let json = r#"{
        "metadata": {"dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json"},
        "nodes": {
            "test.jaffle_shop.not_null_customers_id": {
                "unique_id": "test.jaffle_shop.not_null_customers_id",
                "resource_type": "test",
                "name": "not_null_customers_id"
            }
        }
    }"#;
    let manifest: DbtManifest = serde_json::from_str(json).unwrap();
    let mut graph = SysGraph::new();
    lower_nodes(&manifest, &mut graph);
    assert!(graph.nodes.is_empty(), "test resource_type must not lower to a node");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd ~/promptexecution/ufo-types && cargo test -p ufo-types dbt::tests::lowers_models_and_sources`
Expected: FAIL — `lower_nodes` doesn't exist before this step's edit (apply Step 1's
production code and test together, then run).

- [ ] **Step 3: Run tests to verify they pass**

Run: `cd ~/promptexecution/ufo-types && cargo test -p ufo-types dbt::tests`
Expected: PASS (6 passed)

- [ ] **Step 4: Commit**

```bash
cd ~/promptexecution/ufo-types
git add src/dbt.rs
git commit -m "feat(dbt): lower qualifying dbt nodes into SysGraph nodes"
```

---

## Task 4: Edge lowering + combined entry point

**Files:**
- Modify: `src/dbt.rs`

**Interfaces:**
- Consumes: `ontology::{OntologicalEdge, UfoRelation}` (existing `ufo-types` public
  API), everything from Tasks 1–3.
- Produces: `pub fn dbt_manifest_to_sysgraph(manifest: &DbtManifest, config: &DbtLiftConfig) -> Result<SysGraph, DbtLiftError>`,
  `pub fn parse_and_lift(bytes: &[u8], config: &DbtLiftConfig) -> Result<SysGraph, DbtLiftError>`
  — this is the module's public entry point, called by anything acquiring dbt
  manifest bytes (out of scope here per spec §7 — acquisition-neutral). Grows
  `DbtLiftError` with its third variant.

- [ ] **Step 1: Write the failing test**

Add to `src/dbt.rs`:

```rust
use crate::ontology::{OntologicalEdge, UfoRelation};

// Extend DbtLiftError (replace the enum from Task 2 with this):
#[derive(Debug, thiserror::Error)]
pub enum DbtLiftError {
    #[error("could not parse manifest.json: {0}")]
    Malformed(#[from] serde_json::Error),
    #[error(transparent)]
    Dialect(#[from] DialectError),
    #[error("{from} depends on {to}, which is not present in this manifest")]
    DanglingDependency { from: String, to: String },
}

fn lower_edges(manifest: &DbtManifest, graph: &mut SysGraph) -> Result<(), DbtLiftError> {
    let all_nodes = manifest.sources.values().chain(manifest.nodes.values());
    for node in all_nodes {
        for dep_id in &node.depends_on.nodes {
            let dep_exists = manifest.nodes.contains_key(dep_id) || manifest.sources.contains_key(dep_id);
            if !dep_exists {
                return Err(DbtLiftError::DanglingDependency {
                    from: node.unique_id.clone(),
                    to: dep_id.clone(),
                });
            }
            graph.push_edge(OntologicalEdge {
                id: format!("dbt:{}->{}", node.unique_id, dep_id),
                source: dbt_element_id(&node.unique_id),
                target: dbt_element_id(dep_id),
                relation: UfoRelation::Requires,
                occurrence: None,
                provenance: vec![],
            });
        }
    }
    Ok(())
}

pub fn dbt_manifest_to_sysgraph(
    manifest: &DbtManifest,
    _config: &DbtLiftConfig,
) -> Result<SysGraph, DbtLiftError> {
    let mut graph = SysGraph::new();
    lower_nodes(manifest, &mut graph);
    lower_edges(manifest, &mut graph)?;
    Ok(graph)
}

pub fn parse_and_lift(bytes: &[u8], config: &DbtLiftConfig) -> Result<SysGraph, DbtLiftError> {
    let version = peek_dbt_schema_version(bytes)?;
    let manifest = DbtManifest::upgrade(bytes, &version)?;
    dbt_manifest_to_sysgraph(&manifest, config)
}
```

Add to `#[cfg(test)] mod tests`:

```rust
#[test]
fn lowers_lineage_edges_with_requires_relation() {
    let manifest: DbtManifest = serde_json::from_str(FIXTURE).unwrap();
    let graph = dbt_manifest_to_sysgraph(&manifest, &DbtLiftConfig::default()).unwrap();

    assert_eq!(graph.edges.len(), 2);
    let stg_edge = graph
        .edges
        .iter()
        .find(|e| e.source.0 == "dbt:model.jaffle_shop.stg_customers")
        .unwrap();
    assert_eq!(stg_edge.target.0, "dbt:source.jaffle_shop.raw.customers");
    assert_eq!(stg_edge.relation, UfoRelation::Requires);
    assert!(graph.dangling_edges().is_empty());
}

#[test]
fn dangling_dependency_is_a_typed_error_not_a_panic() {
    let json = r#"{
        "metadata": {"dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json"},
        "nodes": {
            "model.jaffle_shop.orphan": {
                "unique_id": "model.jaffle_shop.orphan",
                "resource_type": "model",
                "name": "orphan",
                "depends_on": {"nodes": ["model.jaffle_shop.does_not_exist"]}
            }
        }
    }"#;
    let manifest: DbtManifest = serde_json::from_str(json).unwrap();
    let err = dbt_manifest_to_sysgraph(&manifest, &DbtLiftConfig::default()).unwrap_err();
    assert!(matches!(err, DbtLiftError::DanglingDependency { .. }));
}

#[test]
fn parse_and_lift_round_trips_the_fixture_end_to_end() {
    let graph = parse_and_lift(FIXTURE.as_bytes(), &DbtLiftConfig::default()).unwrap();
    assert_eq!(graph.nodes.len(), 3);
    assert_eq!(graph.edges.len(), 2);
}

#[test]
fn parse_and_lift_rejects_malformed_json() {
    let err = parse_and_lift(b"{not json", &DbtLiftConfig::default()).unwrap_err();
    assert!(matches!(err, DbtLiftError::Malformed(_)));
}

#[test]
fn parse_and_lift_on_zero_qualifying_nodes_is_an_empty_graph_not_an_error() {
    let json = r#"{
        "metadata": {"dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json"},
        "nodes": {
            "test.jaffle_shop.only_a_test": {
                "unique_id": "test.jaffle_shop.only_a_test",
                "resource_type": "test",
                "name": "only_a_test"
            }
        }
    }"#;
    let graph = parse_and_lift(json.as_bytes(), &DbtLiftConfig::default()).unwrap();
    assert!(graph.nodes.is_empty());
    assert!(graph.edges.is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd ~/promptexecution/ufo-types && cargo test -p ufo-types dbt::tests`
Expected: FAIL to compile until this step's production code is in place (edge
lowering + combined entry point didn't exist before).

- [ ] **Step 3: Run tests to verify they pass**

Run: `cd ~/promptexecution/ufo-types && cargo test -p ufo-types dbt::tests`
Expected: PASS (11 passed — everything from Tasks 1–3 plus these 5)

- [ ] **Step 4: Run the full workspace test suite and clippy**

Run: `cd ~/promptexecution/ufo-types && cargo test && cargo clippy --all-targets -- -D warnings`
Expected: all tests pass (260+11), clippy clean. This confirms nothing in the new
module regresses the rest of the crate and matches this crate's own stated bar
(`reqif`'s PR set this precedent).

- [ ] **Step 5: Commit**

```bash
cd ~/promptexecution/ufo-types
git add src/dbt.rs
git commit -m "feat(dbt): lower lineage edges, add parse_and_lift entry point"
```

---

## Task 5: Push a branch and open a PR

**Files:** none (git/GitHub operations only).

- [ ] **Step 1: Push the branch**

```bash
cd ~/promptexecution/ufo-types
git checkout -b feat/dbt-manifest-sysgraph
git push origin feat/dbt-manifest-sysgraph
```

(If Tasks 1–4 were committed directly on a feature branch already, skip the
`checkout -b` and just push.)

- [ ] **Step 2: Open the PR**

```bash
gh pr create --repo PromptExecution/ufo-types \
  --title "feat(dbt): manifest.json -> SysGraph (digital-thread box-1 front-end)" \
  --body "Implements kr0ki's docs/superpowers/specs/2026-09-20-dbt-manifest-digital-thread-design.md (piece 1 of PLAN-KR0KI-006). New ufo_types::dbt module, unconditional (no feature flag — no risky dependency, unlike reqif). Test plan: 11 new tests, full workspace suite + clippy clean."
```

- [ ] **Step 3: Confirm CI, then merge**

Wait for checks (if any are configured for this repo — `reqif`'s PR had none reported;
if this one is the same, that's expected, not a blocker). Squash-merge once green or
once confirmed there's nothing to gate on:

```bash
gh pr merge --repo PromptExecution/ufo-types --squash --delete-branch
```

---

## Self-Review

**Spec coverage:** §1 (canonical graph, no standalone contract) — satisfied, no new
types outside `ufo_types::dbt`. §2 (two-step `Upgrade`/lowering) — Task 2 + Task 4.
§3 (`DbtManifest` fields) — Task 1, scoped to exactly what §3 lists. §4 (node/edge
mapping) — Tasks 3–4, using the corrected `UfoRelation::Requires` (spec fixed
2026-09-20 after this plan's own type-check against real source). §5 (zero-drift via
`dbt:`-prefixed `ElementId`) — Task 3's `dbt_element_id`. §6 (error handling) — Task
2 + Task 4's `DbtLiftError`, `DanglingDependency` implemented as a hard error per this
plan's Global Constraints (the spec's explicitly-deferred call, made here). §7
(non-goals) — respected: no tests/macros/exposures, no column-level lineage, no
Flexo write, no `b00t://` loader; `parse_and_lift` takes bytes directly. §8 (test
plan) — every listed case has a task-5 test: no-deps, model→model, model→source,
`Upgrade` per-version (only v12 exists to test against; major-mismatch tested against
a hypothetical v9), malformed JSON, empty-qualifying-nodes.

**Placeholder scan:** none found — every step has real code, no "TBD"/"add error
handling"/"similar to Task N".

**Type consistency:** `DbtLiftConfig`, `DbtLiftError`, `DbtManifest`, `DbtNode`,
`dbt_element_id`, `lower_nodes`, `lower_edges`, `dbt_manifest_to_sysgraph`,
`parse_and_lift` — same names and signatures used consistently from their
introducing task through Task 4's combined entry point.

---

**Plan complete and saved to `docs/superpowers/plans/2026-09-20-dbt-manifest-ufo-types.md`
(kr0ki repo). Two execution options:**

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
