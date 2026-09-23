# Flexo Baseline Adapter (M3) — reqif_export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `kr0ki_core::reqif_export`, the missing ReqIF *export* direction (`RequirementGraph` → ReqIF XML), so a validated ReqIF import can eventually round-trip through a live SysML v2 project without losing information.

**Architecture:** One new flat module, `crates/kr0ki-core/src/reqif_export.rs`, mirroring `reqif_import.rs`'s style. Two public functions — `export_bundle(graph) -> Result<ReqIfBundle, ReqIfExportError>` and `export_bundle_to_xml(graph) -> Result<String, ReqIfExportError>` — plus one private helper that synthesizes a fixed, minimal ReqIF type layer (one `SpecObjectType`, one `AttributeDefinition` for body text, ten `SpecRelationType`s, one for each `RequirementRelationKind` variant). `reqrs` is added as a **direct** `kr0ki-core` dependency (promoted from the existing transitive pull via `ufo-types`' `reqif` feature) — this crate is a pure XML parser/unparser with no `[features]` table at all (verified: no `http`/`net`-style builtin-registration concern like `regorus` had on the sibling `requirements-rules-system` PR).

**Tech Stack:** Rust, `reqrs` 0.2.2 (direct dependency, this plan), `ufo_types::mbse::requirements` (already a dependency via `pub use ufo_types::mbse::requirements` in `lib.rs`).

**Spec:** `docs/superpowers/specs/2026-09-23-flexo-baseline-adapter-design.md` (§1 only — this plan implements the export module and the two `no network, no Flexo` tests from §5; §2/§3, the Flexo storage write path and the live round-trip contract test, are **out of scope for this plan** — see "Explicitly out of scope" below).

## Global Constraints

- `reqrs = "0.2.2"` exactly (matches the version already pulled transitively via `ufo-types`' `reqif` feature — a version drift here would mean two different `reqrs` versions in the dependency graph).
- No `unwrap()`/`expect()`/`panic!` in non-test code — every fallible step returns `Result<_, ReqIfExportError>`, matching `reqif_import.rs`'s existing discipline.
- Only asserted relations (`RelationAuthority::Asserted`) export — inferred/proposed edges are silently skipped, never promoted (design doc §1, already decided, not this plan's call to revisit).
- No ReqIFz (zip) export, no attachment round-tripping, no `Specification`/`SpecHierarchy` construction (design doc §4 non-goals; also: `ufo_types::reqif::bundle_to_requirement_graph` never reads `specifications`/`SpecHierarchy` on import either, confirmed by reading its source, so omitting them from export is symmetric, not a gap).

## Explicitly out of scope (tracked, not forgotten)

- **Flexo storage** (design doc §2: writing an exported baseline as a live SysML v2 commit, `flexo_reqif_sync` module) — has a real, currently-unmet dependency: it needs `kr0ki_core::digital_thread_sync::build_changeset`-shaped logic to diff against, which as of 2026-09-23 only exists on PR #52's still-unmerged branch (verified: `digital_thread_sync.rs` does not exist on `main`). Write that plan once PR #52 lands.
- **The full commit↔graph↔export contract test** (design doc §3) — needs both the above and live OMG-pilot credentials this environment doesn't have. Stays `#[ignore]`d per the spec's own §5 convention once the Flexo plan exists.

---

## Task 1: Direct `reqrs` dependency + module skeleton

**Files:**
- Modify: `crates/kr0ki-core/Cargo.toml` (add dependency, right after the existing `reqrs`-history comment block ending `zip = { ... }`)
- Create: `crates/kr0ki-core/src/reqif_export.rs`
- Modify: `crates/kr0ki-core/src/lib.rs:39` (add `pub mod reqif_export;` after `pub mod reqif_import;`)

**Interfaces:**
- Produces: `pub struct ReqIfExportError(...)` (a `thiserror` enum), used by every later task.

- [ ] **Step 1: Add the dependency**

In `crates/kr0ki-core/Cargo.toml`, immediately after the existing block that ends with:
```toml
zip = { version = "2.4.2", default-features = false, features = ["deflate"] }
```
add:
```toml
# M3 (docs/superpowers/specs/2026-09-23-flexo-baseline-adapter-design.md):
# reqif_export needs reqrs::{model, unparse} directly to build and
# serialize a ReqIfBundle for the export direction -- ufo_types::reqif
# deliberately narrows its own scope to the import lowering only (see
# that module's doc comment). Promotes the existing transitive pull
# (via ufo-types' `reqif` feature) to direct; adds no new supply-chain
# surface. Unlike regorus (feat/requirements-rules-system), reqrs has
# no [features] table at all -- nothing to disable.
reqrs = "0.2.2"
```

- [ ] **Step 2: Create the module skeleton**

Create `crates/kr0ki-core/src/reqif_export.rs`:
```rust
//! `RequirementGraph` -> ReqIF XML export. The missing direction:
//! `ufo_types::reqif` only lowers ReqIF -> `RequirementGraph`; this module
//! builds a `reqrs::model::ReqIfBundle` from a `RequirementGraph` and
//! serializes it back to XML. See
//! docs/superpowers/specs/2026-09-23-flexo-baseline-adapter-design.md.

use crate::requirements::RequirementGraph;

#[derive(Debug, thiserror::Error)]
pub enum ReqIfExportError {
    #[error(transparent)]
    Unparse(#[from] reqrs::ReqIfError),
}

pub fn export_bundle(graph: &RequirementGraph) -> Result<reqrs::model::ReqIfBundle, ReqIfExportError> {
    todo!("Task 2")
}

pub fn export_bundle_to_xml(graph: &RequirementGraph) -> Result<String, ReqIfExportError> {
    let bundle = export_bundle(graph)?;
    Ok(reqrs::ReqIfUnparser::unparse(&bundle, reqrs::unparse::FormatMode::Canonical)?)
}
```

- [ ] **Step 3: Wire the module into `lib.rs`**

In `crates/kr0ki-core/src/lib.rs`, change:
```rust
pub mod reqif_import;
```
to:
```rust
pub mod reqif_export;
pub mod reqif_import;
```

- [ ] **Step 4: Verify it compiles (it will not run yet -- `todo!()` panics if called, but nothing calls it)**

Run: `cargo check -p kr0ki-core`
Expected: compiles clean (a `todo!()` body type-checks against any return type).

- [ ] **Step 5: Commit**

```bash
git add crates/kr0ki-core/Cargo.toml crates/kr0ki-core/src/reqif_export.rs crates/kr0ki-core/src/lib.rs
git commit -m "feat(reqif-export): module skeleton + direct reqrs dependency"
```

---

## Task 2: Synthesize the fixed ReqIF type layer

`reqrs::model::AttributeValue` carries no name/key string of its own — only an opaque `definition_ref: AttributeDefId` pointing at an `AttributeDefinition` (verified against real `reqrs` 0.2.2 source, 2026-09-23; this was missing from the design doc's original assumptions entirely — see spec §1a). Export must synthesize this type layer once per bundle: one `SpecObjectType` (id `ST-REQUIREMENT`) whose `spec_attributes` holds one `AttributeDefinition::String` (id `AD-TEXT`, for `Requirement.text` — `Requirement.title` round-trips via `SpecObject.long_name` instead, since `ufo_types::reqif::spec_object_to_requirement` checks `long_name` before any attribute, so this is the simplest form guaranteed to round-trip), pointing at one synthesized `DataType::String` (id `DT-STRING`); plus ten `SpecRelationType`s, one per `RequirementRelationKind` variant, named to match `ufo_types::reqif::relation_kind_from_name`'s recognized strings exactly (`contains`/`derives`/`refines`/`requires`/`satisfies`/`verifies`/`implements`/`traces`/`allocated_to`/`precedes`) so a re-import recovers the same kind.

**Files:**
- Modify: `crates/kr0ki-core/src/reqif_export.rs`

**Interfaces:**
- Consumes: nothing external.
- Produces: `fn type_layer() -> TypeLayer` where `TypeLayer { data_type: reqrs::model::DataType, text_attr_def_id: reqrs::model::AttributeDefId, spec_object_type: reqrs::model::SpecType, relation_types: Vec<reqrs::model::SpecType> }` — later tasks consume `text_attr_def_id` (Task 3) and need to map a `RequirementRelationKind` to one of `relation_types`' ids (Task 4, via a sibling function `relation_type_id(kind: RequirementRelationKind) -> reqrs::model::SpecTypeId` defined in this same task).

- [ ] **Step 1: Write the failing test**

Add to `crates/kr0ki-core/src/reqif_export.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::requirements::RequirementRelationKind;

    #[test]
    fn type_layer_has_one_spec_object_type_and_ten_relation_types() {
        let layer = type_layer();
        assert_eq!(layer.relation_types.len(), 10);
        match &layer.spec_object_type {
            reqrs::model::SpecType::SpecObject(t) => {
                let attrs = t.common.spec_attributes.as_ref().expect("spec_attributes");
                assert_eq!(attrs.len(), 1);
            }
            other => panic!("expected SpecType::SpecObject, got {other:?}"),
        }
    }

    #[test]
    fn relation_type_id_covers_every_kind_with_the_recognized_name() {
        use RequirementRelationKind::*;
        for kind in [
            Contains, Derives, Refines, Requires, Satisfies, Verifies, Implements, Traces,
            AllocatedTo, Precedes,
        ] {
            // Every kind must resolve to a SpecTypeId (no panic) -- and the
            // id must be present among type_layer()'s relation_types.
            let id = relation_type_id(kind);
            let layer = type_layer();
            let found = layer.relation_types.iter().any(|st| match st {
                reqrs::model::SpecType::SpecRelation(t) => t.common.identifier == id,
                _ => false,
            });
            assert!(found, "relation_type_id({kind:?}) = {id:?} not in type_layer()");
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p kr0ki-core reqif_export::tests -- --include-ignored`
Expected: FAIL to compile — `type_layer`, `TypeLayer`, `relation_type_id` not defined yet.

- [ ] **Step 3: Implement `type_layer()` and `relation_type_id()`**

Add to `crates/kr0ki-core/src/reqif_export.rs`, above the `#[cfg(test)]` module:
```rust
use reqrs::model::{
    AttributeDefCommon, AttributeDefId, AttributeDefinition, AttributeDefinitionString,
    DataType, DataTypeCommon, DataTypeId, DataTypeString, DefaultValuePresence, SpecType,
    SpecTypeCommon, SpecTypeId, SpecRelationType,
};
use crate::requirements::RequirementRelationKind;

const DATA_TYPE_STRING_ID: &str = "DT-STRING";
const SPEC_OBJECT_TYPE_ID: &str = "ST-REQUIREMENT";
const TEXT_ATTR_DEF_ID: &str = "AD-TEXT";

struct TypeLayer {
    data_type: DataType,
    text_attr_def_id: AttributeDefId,
    spec_object_type: SpecType,
    relation_types: Vec<SpecType>,
}

fn data_type_common() -> DataTypeCommon {
    DataTypeCommon {
        description: None,
        last_change: None,
        long_name: None,
        was_self_closing: true,
        comments_before: Vec::new(),
    }
}

fn attr_def_common() -> AttributeDefCommon {
    AttributeDefCommon {
        description: None,
        last_change: None,
        long_name: Some("Text".to_string()),
        is_editable: None,
        was_self_closing: false,
    }
}

fn spec_type_common(identifier: &str, long_name: &str, spec_attributes: Option<Vec<AttributeDefinition>>) -> SpecTypeCommon {
    SpecTypeCommon {
        identifier: SpecTypeId::new(identifier),
        description: None,
        last_change: None,
        long_name: Some(long_name.to_string()),
        was_self_closing: spec_attributes.is_none(),
        spec_attributes,
        comments_before: Vec::new(),
    }
}

/// `(RequirementRelationKind variant, spec-relation-type id, LONG-NAME)` --
/// LONG-NAME must exactly match one of `ufo_types::reqif::relation_kind_from_name`'s
/// recognized strings so re-import recovers the same kind.
const RELATION_KINDS: &[(RequirementRelationKind, &str, &str)] = {
    use RequirementRelationKind::*;
    &[
        (Contains, "ST-CONTAINS", "contains"),
        (Derives, "ST-DERIVES", "derives"),
        (Refines, "ST-REFINES", "refines"),
        (Requires, "ST-REQUIRES", "requires"),
        (Satisfies, "ST-SATISFIES", "satisfies"),
        (Verifies, "ST-VERIFIES", "verifies"),
        (Implements, "ST-IMPLEMENTS", "implements"),
        (Traces, "ST-TRACES", "traces"),
        (AllocatedTo, "ST-ALLOCATED-TO", "allocated_to"),
        (Precedes, "ST-PRECEDES", "precedes"),
    ]
};

fn relation_type_id(kind: RequirementRelationKind) -> SpecTypeId {
    RELATION_KINDS
        .iter()
        .find(|(k, _, _)| *k == kind)
        .map(|(_, id, _)| SpecTypeId::new(*id))
        .expect("RELATION_KINDS covers every RequirementRelationKind variant")
}

fn type_layer() -> TypeLayer {
    let data_type = DataType::String(DataTypeString {
        identifier: DataTypeId::new(DATA_TYPE_STRING_ID),
        common: data_type_common(),
        max_length: None,
    });
    let text_attr_def_id = AttributeDefId::new(TEXT_ATTR_DEF_ID);
    let text_attr_def = AttributeDefinition::String(AttributeDefinitionString {
        identifier: text_attr_def_id.clone(),
        common: attr_def_common(),
        type_ref: DataTypeId::new(DATA_TYPE_STRING_ID),
        default_value: DefaultValuePresence::Absent,
    });
    let spec_object_type = SpecType::SpecObject(reqrs::model::SpecObjectType {
        common: spec_type_common(SPEC_OBJECT_TYPE_ID, "Requirement", Some(vec![text_attr_def])),
    });
    let relation_types = RELATION_KINDS
        .iter()
        .map(|(_, id, long_name)| SpecType::SpecRelation(SpecRelationType {
            common: spec_type_common(id, long_name, None),
        }))
        .collect();
    TypeLayer {
        data_type,
        text_attr_def_id,
        spec_object_type,
        relation_types,
    }
}
```

`RequirementRelationKind` needs `PartialEq`/`Copy`/`Debug` for this to compile as written (used in a `const` array comparison and test loop) — check `ufo_types::mbse::requirements::RequirementRelationKind`'s derive list first (`cargo doc -p ufo-types --no-deps --open` or `grep -n "enum RequirementRelationKind" -B 3` on the vendored checkout); it was verified 2026-09-23 to derive at least `Debug, Clone, Copy, PartialEq, Eq` as part of the `ufo-types` v0.15 contract kr0ki already depends on -- if a future `ufo-types` bump drops one of these derives, this step's `RELATION_KINDS.iter().find(|(k,_,_)| *k == kind)` line is where the compile error will surface; swap to `.enumerate()` + manual `match` if so.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p kr0ki-core reqif_export::tests`
Expected: both tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/kr0ki-core/src/reqif_export.rs
git commit -m "feat(reqif-export): synthesize the fixed ReqIF type layer"
```

---

## Task 3: Map requirements to `SpecObject`s (no relations yet)

**Files:**
- Modify: `crates/kr0ki-core/src/reqif_export.rs`

**Interfaces:**
- Consumes: `TypeLayer.text_attr_def_id` (Task 2), `RequirementGraph.requirements: Vec<Requirement>` (`Requirement{id, title, text, ...}`, already a kr0ki-core dependency via `crate::requirements`).
- Produces: `fn requirement_to_spec_object(req: &Requirement, text_attr_def_id: &AttributeDefId) -> reqrs::model::SpecObject`, consumed by `export_bundle` in Task 5.

- [ ] **Step 1: Write the failing test**

Add inside the `#[cfg(test)] mod tests` block:
```rust
    use crate::requirements::{BaselineIdentity, Provenance, Requirement};
    use std::collections::BTreeMap;

    fn baseline() -> BaselineIdentity {
        BaselineIdentity {
            id: "BL-1".to_string(),
            revision: "r1".to_string(),
            import_artifact_sha256: None,
            exported_baseline_sha256: None,
        }
    }

    fn provenance() -> Provenance {
        Provenance {
            source_uri: "test:fixture".to_string(),
            artifact_sha256: None,
            locator: None,
        }
    }

    fn requirement(id: &str, title: &str, text: &str) -> Requirement {
        Requirement {
            id: id.to_string(),
            title: title.to_string(),
            text: text.to_string(),
            baseline: baseline(),
            provenance: provenance(),
            attributes: BTreeMap::new(),
            evidence: Vec::new(),
        }
    }

    #[test]
    fn requirement_maps_title_to_long_name_and_text_to_the_text_attribute() {
        let layer = type_layer();
        let req = requirement("REQ-1", "Encrypt at rest", "System shall encrypt data at rest.");
        let spec_object = requirement_to_spec_object(&req, &layer.text_attr_def_id);

        assert_eq!(spec_object.identifier.as_str(), "REQ-1");
        assert_eq!(spec_object.long_name.as_deref(), Some("Encrypt at rest"));
        assert_eq!(spec_object.spec_object_type, SpecTypeId::new(SPEC_OBJECT_TYPE_ID));
        assert_eq!(spec_object.attributes.len(), 1);
        match &spec_object.attributes[0] {
            reqrs::model::AttributeValue::String(v) => {
                assert_eq!(v.definition_ref, layer.text_attr_def_id);
                assert_eq!(v.value, "System shall encrypt data at rest.");
            }
            other => panic!("expected AttributeValue::String, got {other:?}"),
        }
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kr0ki-core reqif_export::tests::requirement_maps_title`
Expected: FAIL to compile — `requirement_to_spec_object` not defined.

- [ ] **Step 3: Implement `requirement_to_spec_object`**

Add above the test module:
```rust
use reqrs::model::{AttributeValue, AttributeValueString, SpecObject, SpecObjectId};
use crate::requirements::Requirement;

fn requirement_to_spec_object(req: &Requirement, text_attr_def_id: &AttributeDefId) -> SpecObject {
    SpecObject {
        identifier: SpecObjectId::new(req.id.as_str()),
        description: None,
        last_change: None,
        long_name: Some(req.title.clone()),
        spec_object_type: SpecTypeId::new(SPEC_OBJECT_TYPE_ID),
        attributes: vec![AttributeValue::String(AttributeValueString {
            definition_ref: text_attr_def_id.clone(),
            value: req.text.clone(),
            comments_before: Vec::new(),
        })],
        children_order: Vec::new(),
        comments_before: Vec::new(),
        values_trailing_comments: Vec::new(),
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p kr0ki-core reqif_export::tests`
Expected: all tests so far PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/kr0ki-core/src/reqif_export.rs
git commit -m "feat(reqif-export): map Requirement to SpecObject"
```

---

## Task 4: Map asserted relations to `SpecRelation`s

**Files:**
- Modify: `crates/kr0ki-core/src/reqif_export.rs`

**Interfaces:**
- Consumes: `relation_type_id(kind)` (Task 2), `RequirementGraph.relations: Vec<RequirementRelation>` (`RequirementRelation{id, source, target, kind, authority: RelationAuthority, ...}`, `RelationAuthority::{Asserted, Inferred(_), Proposed(_)}`).
- Produces: `fn asserted_relations_to_spec_relations(relations: &[RequirementRelation]) -> Vec<reqrs::model::SpecRelation>`, consumed by `export_bundle` in Task 5. Filters out non-`Asserted` relations.

- [ ] **Step 1: Write the failing test**

Add inside `mod tests`:
```rust
    use crate::requirements::{RelationAuthority, RequirementRelation};

    fn relation(id: &str, source: &str, target: &str, kind: RequirementRelationKind, authority: RelationAuthority) -> RequirementRelation {
        RequirementRelation {
            id: id.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            kind,
            authority,
            provenance: provenance(),
            promotion: None,
        }
    }

    #[test]
    fn only_asserted_relations_are_exported() {
        let relations = vec![
            relation("R1", "A", "B", RequirementRelationKind::Derives, RelationAuthority::Asserted),
            relation("R2", "C", "D", RequirementRelationKind::Traces, RelationAuthority::Inferred(Default::default())),
        ];
        let spec_relations = asserted_relations_to_spec_relations(&relations);
        assert_eq!(spec_relations.len(), 1);
        assert_eq!(spec_relations[0].identifier.as_str(), "R1");
        assert_eq!(spec_relations[0].source.as_str(), "A");
        assert_eq!(spec_relations[0].target.as_str(), "B");
        assert_eq!(spec_relations[0].relation_type, relation_type_id(RequirementRelationKind::Derives));
    }
```

If `RelationAuthority::Inferred`'s inner `NonAuthoritativeRelation` does not implement `Default`, replace `Default::default()` in the test with a real value constructed the same way `RequirementGraph::promote_relation`'s own tests build one (check `ufo_types::mbse::requirements`'s test module for the pattern before writing this step's final form).

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kr0ki-core reqif_export::tests::only_asserted_relations_are_exported`
Expected: FAIL to compile — `asserted_relations_to_spec_relations` not defined.

- [ ] **Step 3: Implement `asserted_relations_to_spec_relations`**

Add above the test module:
```rust
use reqrs::model::{SpecObjectId as SpecRelObjId, SpecRelation};
use crate::requirements::{RelationAuthority, RequirementRelation};

fn asserted_relations_to_spec_relations(relations: &[RequirementRelation]) -> Vec<SpecRelation> {
    relations
        .iter()
        .filter(|r| matches!(r.authority, RelationAuthority::Asserted))
        .map(|r| SpecRelation {
            identifier: reqrs::model::SpecRelationId::new(r.id.as_str()),
            description: None,
            last_change: None,
            long_name: None,
            relation_type: relation_type_id(r.kind),
            source: SpecRelObjId::new(r.source.as_str()),
            target: SpecRelObjId::new(r.target.as_str()),
            values: None,
            children_order: Vec::new(),
            comments_before: Vec::new(),
        })
        .collect()
}
```

(`SpecObjectId as SpecRelObjId` is just to avoid a duplicate-import error against Task 3's `SpecObjectId` — if both imports are already in scope from a shared `use` block by this point, drop the alias and reuse the one import.)

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p kr0ki-core reqif_export::tests`
Expected: all tests so far PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/kr0ki-core/src/reqif_export.rs
git commit -m "feat(reqif-export): map asserted RequirementRelations to SpecRelation, skip inferred/proposed"
```

---

## Task 5: Assemble `export_bundle` and wire up `export_bundle_to_xml`

**Files:**
- Modify: `crates/kr0ki-core/src/reqif_export.rs`

**Interfaces:**
- Consumes: `type_layer()` (Task 2), `requirement_to_spec_object` (Task 3), `asserted_relations_to_spec_relations` (Task 4).
- Produces: real `export_bundle`/`export_bundle_to_xml` bodies, replacing Task 1's `todo!()`.

- [ ] **Step 1: Write the failing test**

Add inside `mod tests`:
```rust
    #[test]
    fn export_bundle_to_xml_produces_parseable_output_containing_the_requirement() {
        let mut graph = RequirementGraph {
            baseline: baseline(),
            requirements: vec![requirement("REQ-1", "Encrypt at rest", "Body text.")],
            evidence: Vec::new(),
            relations: Vec::new(),
        };
        let xml = export_bundle_to_xml(&graph).expect("export should succeed");
        assert!(xml.contains("REQ-1"));
        assert!(xml.contains("Encrypt at rest"));
        assert!(xml.contains("Body text."));

        // Must be valid enough for reqrs itself to parse back.
        let reparsed = reqrs::ReqIfParser::parse_str(&xml).expect("exported XML must re-parse");
        let content = reparsed
            .core_content
            .as_ref()
            .and_then(|c| c.req_if_content.as_ref())
            .expect("re-parsed bundle must have content");
        assert_eq!(content.spec_objects.as_ref().map(|v| v.len()), Some(1));

        let _ = &mut graph; // silence unused_mut if the test grows
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kr0ki-core reqif_export::tests::export_bundle_to_xml_produces_parseable_output_containing_the_requirement`
Expected: FAIL — `todo!()` in `export_bundle` panics at runtime (`"not yet implemented: Task 2"`).

- [ ] **Step 3: Implement `export_bundle`, replacing the `todo!()` from Task 1**

Replace the Task-1 body of `export_bundle` in `crates/kr0ki-core/src/reqif_export.rs`:
```rust
use reqrs::model::{CoreContent, ListForms, NamespaceInfo, ObjectLookup, ReqIfBundle, ReqIfContent, ReqIfHeader};

pub fn export_bundle(graph: &RequirementGraph) -> Result<ReqIfBundle, ReqIfExportError> {
    let layer = type_layer();

    let spec_objects: Vec<SpecObject> = graph
        .requirements
        .iter()
        .map(|r| requirement_to_spec_object(r, &layer.text_attr_def_id))
        .collect();
    let spec_relations = asserted_relations_to_spec_relations(&graph.relations);

    let mut spec_types = vec![layer.spec_object_type];
    spec_types.extend(layer.relation_types);

    let content = ReqIfContent {
        data_types: Some(vec![layer.data_type]),
        spec_types: Some(spec_types),
        spec_objects: Some(spec_objects),
        spec_relations: Some(spec_relations),
        specifications: None,
        relation_groups: None,
        list_forms: ListForms::default(),
        data_types_trailing_comments: Vec::new(),
        spec_types_trailing_comments: Vec::new(),
        spec_objects_trailing_comments: Vec::new(),
        spec_relations_trailing_comments: Vec::new(),
        specifications_trailing_comments: Vec::new(),
        relation_groups_trailing_comments: Vec::new(),
    };
    let lookup = ObjectLookup::build(&content);

    let header = ReqIfHeader {
        identifier: graph.baseline.id.clone(),
        comment: None,
        creation_time: None,
        repository_id: None,
        req_if_tool_id: None,
        req_if_version: None,
        source_tool_id: None,
        title: None,
    };

    Ok(ReqIfBundle {
        namespace_info: NamespaceInfo::default(),
        header: Some(header),
        core_content: Some(CoreContent { req_if_content: Some(content) }),
        tool_extensions: Default::default(),
        lookup,
        exceptions: Vec::new(),
    })
}
```

The six `*_trailing_comments` field names on `ReqIfContent` are inferred from the pattern documented for every other `*_comments`-style field in this crate (`comments_before`, `values_trailing_comments`) — confirm the exact six names via `grep -n "trailing_comments" ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/reqrs-0.2.2/src/model/content.rs` before writing this step's final form; they are one-per-container (`data_types`/`spec_types`/`spec_objects`/`spec_relations`/`specifications`/`relation_groups`).

`ReqIfHeader`'s exact field list beyond `identifier: String` (verified) was reported as "everything else `Option`" without each field's exact name spelled out one-by-one — confirm the six optional field names (`comment`, `creation_time`, `repository_id`, `req_if_tool_id`, `req_if_version`, `source_tool_id`, `title` is the reported set) via `grep -n "pub struct ReqIfHeader" -A 10 ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/reqrs-0.2.2/src/model/header.rs` before this step's final form, since a mismatched field name is a compile error the test alone won't catch until Step 2 re-runs.

`export_bundle_to_xml` (Task 1's body) needs no change — it already calls `export_bundle` then `ReqIfUnparser::unparse(&bundle, FormatMode::Canonical)`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p kr0ki-core reqif_export::tests`
Expected: all tests PASS. **If the re-parse assertion fails specifically because the output XML has no recognizable root namespace** (a genuinely open question flagged in the design doc's §1a — `NamespaceInfo::default()` leaves `schema_namespace`/`namespace` as `None`), change `NamespaceInfo::default()` above to `NamespaceInfo { namespace: Some("http://www.omg.org/spec/ReqIF/20110401/reqif.xsd".to_string()), ..Default::default() }`, or whichever of `NamespaceInfo`'s ten fields the failure actually points at, and re-run. Do not guess this preemptively — let the test tell you.

- [ ] **Step 5: Commit**

```bash
git add crates/kr0ki-core/src/reqif_export.rs
git commit -m "feat(reqif-export): assemble export_bundle, wire export_bundle_to_xml"
```

---

## Task 6: Round-trip fixture test (§5's second bullet — no network, no Flexo)

**Files:**
- Create: `crates/kr0ki-core/tests/fixtures/reqif/roundtrip.reqif`
- Create: `crates/kr0ki-core/tests/reqif_export.rs`

**Interfaces:**
- Consumes: `ufo_types::reqif::{parse_and_lower, ReqIfAdapterConfig}` (already used by `reqif_import.rs`), `kr0ki_core::reqif_export::export_bundle_to_xml`.

- [ ] **Step 1: Create the fixture**

Create `crates/kr0ki-core/tests/fixtures/reqif/roundtrip.reqif` (two spec objects, one derives relation — adapted from `ufo-types`' own `reqif.rs` test fixture, which is already proven to parse and lower correctly):
```xml
<?xml version="1.0" encoding="UTF-8"?>
<REQ-IF xmlns="http://www.omg.org/spec/ReqIF/20110401/reqif.xsd">
  <THE-HEADER>
    <REQ-IF-HEADER IDENTIFIER="BL-ROUNDTRIP"><TITLE>Round-trip test baseline</TITLE></REQ-IF-HEADER>
  </THE-HEADER>
  <CORE-CONTENT>
    <REQ-IF-CONTENT>
      <DATATYPES>
        <DATATYPE-DEFINITION-STRING IDENTIFIER="DT-STRING" LONG-NAME="String"/>
      </DATATYPES>
      <SPEC-TYPES>
        <SPEC-OBJECT-TYPE IDENTIFIER="ST-REQUIREMENT" LONG-NAME="Requirement">
          <SPEC-ATTRIBUTES>
            <ATTRIBUTE-DEFINITION-STRING IDENTIFIER="AD-TEXT" LONG-NAME="Text">
              <TYPE><DATATYPE-DEFINITION-STRING-REF>DT-STRING</DATATYPE-DEFINITION-STRING-REF></TYPE>
            </ATTRIBUTE-DEFINITION-STRING>
          </SPEC-ATTRIBUTES>
        </SPEC-OBJECT-TYPE>
        <SPEC-RELATION-TYPE IDENTIFIER="ST-DERIVES" LONG-NAME="derives"/>
      </SPEC-TYPES>
      <SPEC-OBJECTS>
        <SPEC-OBJECT IDENTIFIER="REQ-1" LONG-NAME="Encryption at rest">
          <TYPE><SPEC-OBJECT-TYPE-REF>ST-REQUIREMENT</SPEC-OBJECT-TYPE-REF></TYPE>
          <VALUES>
            <ATTRIBUTE-VALUE-STRING THE-VALUE="System shall encrypt data at rest.">
              <DEFINITION><ATTRIBUTE-DEFINITION-STRING-REF>AD-TEXT</ATTRIBUTE-DEFINITION-STRING-REF></DEFINITION>
            </ATTRIBUTE-VALUE-STRING>
          </VALUES>
        </SPEC-OBJECT>
        <SPEC-OBJECT IDENTIFIER="REQ-2" LONG-NAME="Derived encryption">
          <TYPE><SPEC-OBJECT-TYPE-REF>ST-REQUIREMENT</SPEC-OBJECT-TYPE-REF></TYPE>
          <VALUES>
            <ATTRIBUTE-VALUE-STRING THE-VALUE="Derived encryption must remain protected.">
              <DEFINITION><ATTRIBUTE-DEFINITION-STRING-REF>AD-TEXT</ATTRIBUTE-DEFINITION-STRING-REF></DEFINITION>
            </ATTRIBUTE-VALUE-STRING>
          </VALUES>
        </SPEC-OBJECT>
      </SPEC-OBJECTS>
      <SPEC-RELATIONS>
        <SPEC-RELATION IDENTIFIER="REL-1">
          <TYPE><SPEC-RELATION-TYPE-REF>ST-DERIVES</SPEC-RELATION-TYPE-REF></TYPE>
          <SOURCE><SPEC-OBJECT-REF>REQ-2</SPEC-OBJECT-REF></SOURCE>
          <TARGET><SPEC-OBJECT-REF>REQ-1</SPEC-OBJECT-REF></TARGET>
        </SPEC-RELATION>
      </SPEC-RELATIONS>
    </REQ-IF-CONTENT>
  </CORE-CONTENT>
</REQ-IF>
```

- [ ] **Step 2: Write the failing test**

Create `crates/kr0ki-core/tests/reqif_export.rs`:
```rust
//! Round-trip contract for `reqif_export`, independent of Flexo/network:
//! parse a real fixture -> RequirementGraph -> export_bundle_to_xml ->
//! re-parse -> assert semantic equality (ids/titles/texts/asserted
//! relations -- not byte-identical XML; ReqIF's own attribute ordering
//! isn't meaningful, per design doc §3).

use kr0ki_core::reqif_export::export_bundle_to_xml;
use ufo_types::reqif::{parse_and_lower, ReqIfAdapterConfig};

const FIXTURE: &[u8] = include_bytes!("fixtures/reqif/roundtrip.reqif");

fn config() -> ReqIfAdapterConfig {
    ReqIfAdapterConfig {
        source_uri: "test:roundtrip".to_string(),
        revision: "r1".to_string(),
        import_artifact_sha256: None,
    }
}

#[test]
fn roundtrip_preserves_ids_titles_texts_and_asserted_relations() {
    let original = parse_and_lower(FIXTURE, &config()).expect("fixture must parse and lower");

    let xml = export_bundle_to_xml(&original).expect("export must succeed");

    let reexported_bundle = reqrs::ReqIfParser::parse_str(&xml).expect("exported XML must re-parse");
    let reimported = ufo_types::reqif::bundle_to_requirement_graph(&reexported_bundle, &config())
        .expect("re-parsed bundle must lower");

    let mut original_reqs: Vec<(String, String, String)> = original
        .requirements
        .iter()
        .map(|r| (r.id.clone(), r.title.clone(), r.text.clone()))
        .collect();
    let mut reimported_reqs: Vec<(String, String, String)> = reimported
        .requirements
        .iter()
        .map(|r| (r.id.clone(), r.title.clone(), r.text.clone()))
        .collect();
    original_reqs.sort();
    reimported_reqs.sort();
    assert_eq!(original_reqs, reimported_reqs);

    let mut original_rels: Vec<(String, String, String)> = original
        .relations
        .iter()
        .map(|r| (r.source.clone(), r.target.clone(), format!("{:?}", r.kind)))
        .collect();
    let mut reimported_rels: Vec<(String, String, String)> = reimported
        .relations
        .iter()
        .map(|r| (r.source.clone(), r.target.clone(), format!("{:?}", r.kind)))
        .collect();
    original_rels.sort();
    reimported_rels.sort();
    assert_eq!(original_rels, reimported_rels);
}
```

`kr0ki_core::reqif_export::export_bundle_to_xml` must be `pub` and `reqif_export` module `pub` (both already true from Task 1's `pub mod reqif_export;` and `pub fn export_bundle_to_xml`) for this integration test (a separate crate target) to reach it.

- [ ] **Step 3: Run test to verify it fails or passes**

Run: `cargo test -p kr0ki-core --test reqif_export`
Expected: given Tasks 1-5 are already implemented by this point in the plan, this SHOULD pass immediately — it is a regression/integration test over already-built functionality, not new production code. If it fails, the failure is real signal about a gap in Tasks 1-5 (most likely: the title-round-trip path, or a relation-kind name mismatch) — fix the root cause in `reqif_export.rs`, not this test.

- [ ] **Step 4: Confirm pass**

Run: `cargo test -p kr0ki-core --test reqif_export`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/kr0ki-core/tests/fixtures/reqif/roundtrip.reqif crates/kr0ki-core/tests/reqif_export.rs
git commit -m "test(reqif-export): fixture-based round-trip contract test"
```

---

## Task 7: Gate and docs

**Files:**
- Modify: `docs/TODO.md` (mark the export-direction item done, if one exists matching this work — grep for "ReqIF" + "export" first; if no matching line exists, add one under the ReqIF/Flexo stream referencing this plan and PR)
- Modify: `docs/superpowers/specs/2026-09-23-flexo-baseline-adapter-design.md` (update `**Status:**` line from `proposed, pending approval` to reflect §1 being implemented, once this task lands)

- [ ] **Step 1: Run the full gate**

Run: `cd /home/brianh/promptexecution/kr0ki && just check && cargo test -p kr0ki-core`
Expected: `fmt`/`clippy` clean, all `kr0ki-core` tests pass (including everything from Tasks 2-6).

- [ ] **Step 2: Update `docs/TODO.md`**

Grep first: `grep -n "ReqIF" docs/TODO.md | grep -i export`. Update or add the line to reflect §1 (export direction) done, §2/§3 (Flexo storage, live contract test) still open and blocked on PR #52.

- [ ] **Step 3: Update the design doc status line**

In `docs/superpowers/specs/2026-09-23-flexo-baseline-adapter-design.md`, change:
```markdown
**Status:** proposed, pending approval.
```
to:
```markdown
**Status:** §1 (ReqIF export) implemented. §2 (Flexo storage) and §3 (round-trip contract test) blocked on PR #52 (`digital_thread_sync`) merging -- see docs/superpowers/plans/2026-09-23-flexo-baseline-adapter.md.
```

- [ ] **Step 4: Commit**

```bash
git add docs/TODO.md docs/superpowers/specs/2026-09-23-flexo-baseline-adapter-design.md
git commit -m "docs: mark reqif_export (M3 §1) implemented, note §2/§3 blocked on PR #52"
```

- [ ] **Step 5: Open a PR**

```bash
git push -u origin feat/flexo-m3-reqif-export-plan
gh pr create --title "feat(reqif-export): RequirementGraph -> ReqIF XML export (M3 §1)" --body "..."
```

---

## Self-Review Notes (for whoever executes this plan)

Three places in this plan intentionally defer a small, real uncertainty to a test's own runtime signal rather than asserting an unverified guess as fact — this is not a placeholder (each has a concrete first attempt and a concrete, specific contingency), it's honest engineering about `reqrs` behavior nobody has exercised yet:
1. Task 5 Step 4 — whether `NamespaceInfo::default()` produces re-parseable-by-`reqrs` XML, or needs `schema_namespace`/`namespace` populated.
2. Task 5 Step 3 — the exact six `*_trailing_comments` field names on `ReqIfContent`, and `ReqIfHeader`'s exact optional-field name list — both call out the precise `grep` to run to confirm before finalizing the step, rather than asserting the guessed names compile.
3. Task 4 Step 1 — whether `NonAuthoritativeRelation` implements `Default`.

All three are resolved by running the step's own commands, not by guessing further.
