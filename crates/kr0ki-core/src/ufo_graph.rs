//! Box 2 of the five-box ingestion pipeline (PLAN-KR0KI-002 §2): the
//! `ModelSnapshot` → canonical UFO semantic-graph builder for the SysML-v2
//! source arm.
//!
//! ```text
//! ModelSnapshot ──▶ [THIS] ──▶ Vec<OntologicalEdge> ──▶ (box 3, a pattern
//!  (box 1 arm)                  (box 2)                  recognizer — not
//!                                                         built here; #12)
//! ```
//!
//! This module does **not** build a Kubernetes-shaped graph — that is the
//! box-3 recognizer's job (`docs/PATTERNS-kubernetes.md`), which reads raw
//! `iso_ir`/manifest data and disambiguates overloaded domain verbs. Here the
//! raw KerML relationship *element* `@type` already speaks the same closed
//! vocabulary `ufo_types::sysml_model::Relation` types over (§2.2 of
//! `docs/DESIGN-NOTE-typed-model-layer.md`), so normalizing it into a
//! [`UfoRelation`] is a direct, generic lookup — no domain recognizer needed
//! for this arm.
//!
//! ## Mapping
//!
//! | Raw KerML relationship `@type` | Endpoint fields | [`UfoRelation`] |
//! |---|---|---|
//! | `FeatureMembership` | `owner`, `member` | [`UfoRelation::HasPart`] |
//! | `Specialization` | `specific`, `general` | [`UfoRelation::Specializes`] |
//! | `Subsetting` | `subset`, `superset` | [`UfoRelation::Specializes`] |
//! | `Redefinition` | `redefining`, `redefined` | [`UfoRelation::Specializes`] |
//! | `Connection` | `ends` (exactly 2) | [`UfoRelation::Binds`] |
//! | `Succession` | `source`, `target` | [`UfoRelation::Precedes`] |
//! | `Satisfy` | `subject`, `requirement` | [`UfoRelation::Satisfies`] |
//! | `Verify` | `by`, `requirement` | [`UfoRelation::Verifies`] |
//! | `Refine` | `refining`, `refined` | [`UfoRelation::TracesTo`] |
//! | `Dependency` | `client`, `supplier` | [`UfoRelation::Requires`] |
//!
//! Field names and the raw `@type` vocabulary itself come from
//! `ufo_types::sysml_model::Relation`'s own variants (shipped v0.14.0),
//! which is explicitly "a data-level sum type over the KerML / SysML v2
//! relationship set" — i.e. these are the metamodel's own names, not an
//! invented convention.
//!
//! `Allocation` is **deliberately not mapped**: `docs/PATTERNS-kubernetes.md`
//! §4 shows the box-3 recognizer collapsing four distinct ontological
//! relations (`controls` / `observes` / `governed_by` / `authorized_by`)
//! down to `Relation::Allocation` when going *forward*; there is no single
//! correct `UfoRelation` to recover going backward. Per
//! `docs/DESIGN-NOTE-typed-model-layer.md` §2.1's own rule — "anything not
//! expressible ... simply stays at the `iso_ir` layer and is never lifted" —
//! an `Allocation` element is left un-lifted here rather than guessed at.
//! The same reasoning excludes `Conjugation`/`FeatureTyping`: neither has a
//! confirmed raw-API field shape in this repo's design docs, so they stay
//! un-lifted rather than risk a wrong guess.
//!
//! Endpoint values follow this API's existing reference-object convention
//! (see [`kr0ki_sysmlv2_client::Ref`] / `Commit.owning_project`): a single
//! `{"@id": "..."}` object. A bare string id or a one-element array are also
//! accepted defensively, since the target servers (Flexo, the OMG pilot,
//! SysON) each implement a slightly different slice of the same spec
//! (`docs/EVAL-flexo.md`, `docs/EVAL-syson.md`). A relationship element that
//! doesn't resolve both endpoints is skipped, not an error — a partial or
//! unrecognized element must never abort the whole snapshot's graph build.

use kr0ki_sysmlv2_client::{Element, ModelSnapshot};
use serde_json::Value;
use ufo_types::ontology::{OntologicalEdge, UfoRelation};
use ufo_types::sysml_model::ElementId;

/// Build the canonical UFO semantic graph for one `ModelSnapshot`.
///
/// Walks every element, lifting the ones whose `@type` is a recognized raw
/// KerML relationship kind (see module docs) into an [`OntologicalEdge`].
/// Non-relationship elements (`PartUsage`, `PartDefinition`, `Package`, …)
/// and unrecognized/malformed relationship elements are silently excluded —
/// they stay representable at the `ModelSnapshot`/`iso_ir` layer for a later
/// pipeline stage, never dropped as an error.
pub fn build_ufo_graph(snapshot: &ModelSnapshot) -> Vec<OntologicalEdge> {
    snapshot
        .elements
        .iter()
        .filter_map(ontological_edge_for)
        .collect()
}

/// [`build_ufo_graph`], wrapped in `ufo_types::sysgraph::SysGraph`
/// (`docs/TODO.md` box 2's "Graph container type") — one node per
/// non-relationship element (every `@type` that parses as an
/// `ufo_types::sysml_model::ElementKind`), plus every edge
/// `build_ufo_graph` already produces, unchanged.
///
/// The `ElementKind → UfoStereotype` mapping has no prior precedent
/// anywhere in this codebase or `ufo-types` (`docs/DESIGN-NOTE-typed-model-
/// layer.md` §2.10 — resolved there, this function is that section's
/// implementation): a `*Definition` → `Kind`, a `*Usage` → `Role`, `Package`
/// → `Kind("Package")` (neither, per `ElementKind::is_definition`/
/// `is_usage`, which are already exhaustive over the other 23 variants). A
/// relationship element's `@type` (`FeatureMembership`, `Specialization`,
/// …) isn't one of `ElementKind`'s 24 variants at all, so it parses to
/// `Err` and is silently excluded here — it's already an edge, not a node.
pub fn to_sysgraph(snapshot: &ModelSnapshot) -> ufo_types::sysgraph::SysGraph {
    use ufo_types::stereotype::UfoStereotype;
    use ufo_types::sysgraph::{OntologicalNode, SysGraph};
    use ufo_types::sysml_model::ElementKind;

    let mut graph = SysGraph::new();
    for el in &snapshot.elements {
        let Ok(kind) = el.ty().parse::<ElementKind>() else {
            continue;
        };
        let kind_name = kind.kerml_name().to_string();
        let stereotype = if kind.is_usage() {
            UfoStereotype::Role(kind_name)
        } else {
            // is_definition(), or Package (neither) -- both are Kind.
            UfoStereotype::Kind(kind_name)
        };
        let node = match el.name() {
            Some(name) => OntologicalNode::with_label(ElementId::new(el.id()), stereotype, name),
            None => OntologicalNode::new(ElementId::new(el.id()), stereotype),
        };
        graph.push_node(node);
    }
    for edge in build_ufo_graph(snapshot) {
        graph.push_edge(edge);
    }
    graph
}

fn ontological_edge_for(el: &Element) -> Option<OntologicalEdge> {
    let (relation, source, target) = match el.ty() {
        "FeatureMembership" => (
            UfoRelation::HasPart,
            field_id(el, "owner")?,
            field_id(el, "member")?,
        ),
        "Specialization" => (
            UfoRelation::Specializes,
            field_id(el, "specific")?,
            field_id(el, "general")?,
        ),
        "Subsetting" => (
            UfoRelation::Specializes,
            field_id(el, "subset")?,
            field_id(el, "superset")?,
        ),
        "Redefinition" => (
            UfoRelation::Specializes,
            field_id(el, "redefining")?,
            field_id(el, "redefined")?,
        ),
        "Connection" => {
            let (a, b) = connection_endpoints(el)?;
            (UfoRelation::Binds, a, b)
        }
        "Succession" => (
            UfoRelation::Precedes,
            field_id(el, "source")?,
            field_id(el, "target")?,
        ),
        "Satisfy" => (
            UfoRelation::Satisfies,
            field_id(el, "subject")?,
            field_id(el, "requirement")?,
        ),
        "Verify" => (
            UfoRelation::Verifies,
            field_id(el, "by")?,
            field_id(el, "requirement")?,
        ),
        "Refine" => (
            UfoRelation::TracesTo,
            field_id(el, "refining")?,
            field_id(el, "refined")?,
        ),
        "Dependency" => (
            UfoRelation::Requires,
            field_id(el, "client")?,
            field_id(el, "supplier")?,
        ),
        _ => return None,
    };
    Some(OntologicalEdge::new(
        el.id().to_string(),
        source,
        target,
        relation,
    ))
}

/// Resolve one reference-valued field on `el` to an [`ElementId`].
fn field_id(el: &Element, key: &str) -> Option<ElementId> {
    el.get(key).and_then(ref_id).map(ElementId::new)
}

/// The `Connection` element's exactly-two endpoints, from its `ends` field.
///
/// `Relation::Connection` (box 4) carries an arbitrary-arity `ends: Vec<_>`,
/// but [`OntologicalEdge`] is binary. A `Connection` with any arity other
/// than two cannot be represented as a single edge here without dropping
/// information, so it is skipped rather than truncated.
fn connection_endpoints(el: &Element) -> Option<(ElementId, ElementId)> {
    let ends = el.get("ends").map(ref_ids).unwrap_or_default();
    match ends.as_slice() {
        [a, b] => Some((ElementId::new(a.clone()), ElementId::new(b.clone()))),
        _ => None,
    }
}

/// Extract a single `@id` from a reference-shaped JSON value: a `{"@id":
/// ...}` object (this API's established convention — see module docs), a
/// bare string id, or the first element of a one-item array.
fn ref_id(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Object(map) => map.get("@id").and_then(Value::as_str).map(String::from),
        Value::Array(arr) => arr.first().and_then(ref_id),
        _ => None,
    }
}

/// Extract every `@id` from a reference-shaped JSON value, tolerating both a
/// single reference and a JSON-LD-style array of references.
fn ref_ids(v: &Value) -> Vec<String> {
    match v {
        Value::Array(arr) => arr.iter().filter_map(ref_id).collect(),
        other => ref_id(other).into_iter().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn element(json: Value) -> Element {
        serde_json::from_value(json).unwrap()
    }

    fn snapshot(elements: Vec<Element>) -> ModelSnapshot {
        ModelSnapshot {
            project_id: "p1".into(),
            commit_id: "c1".into(),
            roots: Vec::new(),
            content_hash: "test".into(),
            elements,
        }
    }

    #[test]
    fn feature_membership_lifts_to_has_part() {
        let el = element(json!({
            "@id": "rel1", "@type": "FeatureMembership",
            "owner": { "@id": "e1" }, "member": { "@id": "e2" }
        }));
        let edge = ontological_edge_for(&el).expect("edge");
        assert_eq!(edge.id, "rel1");
        assert_eq!(edge.relation, UfoRelation::HasPart);
        assert_eq!(edge.source, ElementId::new("e1"));
        assert_eq!(edge.target, ElementId::new("e2"));
    }

    #[test]
    fn specialization_family_all_map_to_specializes() {
        let cases = [
            (
                json!({ "@id": "r1", "@type": "Specialization", "specific": "s", "general": "g" }),
                "s",
                "g",
            ),
            (
                json!({ "@id": "r2", "@type": "Subsetting", "subset": "s", "superset": "g" }),
                "s",
                "g",
            ),
            (
                json!({ "@id": "r3", "@type": "Redefinition", "redefining": "s", "redefined": "g" }),
                "s",
                "g",
            ),
        ];
        for (json, want_source, want_target) in cases {
            let edge = ontological_edge_for(&element(json)).expect("edge");
            assert_eq!(edge.relation, UfoRelation::Specializes);
            assert_eq!(edge.source, ElementId::new(want_source));
            assert_eq!(edge.target, ElementId::new(want_target));
        }
    }

    #[test]
    fn connection_with_exactly_two_ends_lifts_to_binds() {
        let el = element(json!({
            "@id": "conn1", "@type": "Connection",
            "ends": [{ "@id": "a" }, { "@id": "b" }]
        }));
        let edge = ontological_edge_for(&el).expect("edge");
        assert_eq!(edge.relation, UfoRelation::Binds);
        assert_eq!(edge.source, ElementId::new("a"));
        assert_eq!(edge.target, ElementId::new("b"));
    }

    #[test]
    fn connection_with_three_ends_is_skipped() {
        let el = element(json!({
            "@id": "conn1", "@type": "Connection",
            "ends": [{ "@id": "a" }, { "@id": "b" }, { "@id": "c" }]
        }));
        assert!(ontological_edge_for(&el).is_none());
    }

    #[test]
    fn succession_lifts_to_precedes() {
        let el = element(json!({
            "@id": "s1", "@type": "Succession",
            "source": { "@id": "a" }, "target": { "@id": "b" }
        }));
        let edge = ontological_edge_for(&el).expect("edge");
        assert_eq!(edge.relation, UfoRelation::Precedes);
    }

    #[test]
    fn satisfy_and_verify_and_refine_and_dependency() {
        let satisfy = element(json!({
            "@id": "sat1", "@type": "Satisfy",
            "subject": "subj", "requirement": "req"
        }));
        let e = ontological_edge_for(&satisfy).expect("edge");
        assert_eq!(e.relation, UfoRelation::Satisfies);
        assert_eq!(e.source, ElementId::new("subj"));
        assert_eq!(e.target, ElementId::new("req"));

        let verify = element(json!({
            "@id": "ver1", "@type": "Verify",
            "by": "test1", "requirement": "req"
        }));
        let e = ontological_edge_for(&verify).expect("edge");
        assert_eq!(e.relation, UfoRelation::Verifies);
        assert_eq!(e.source, ElementId::new("test1"));

        let refine = element(json!({
            "@id": "ref1", "@type": "Refine",
            "refining": "detailed", "refined": "need"
        }));
        let e = ontological_edge_for(&refine).expect("edge");
        assert_eq!(e.relation, UfoRelation::TracesTo);
        assert_eq!(e.source, ElementId::new("detailed"));

        let dep = element(json!({
            "@id": "dep1", "@type": "Dependency",
            "client": "consumer", "supplier": "provider"
        }));
        let e = ontological_edge_for(&dep).expect("edge");
        assert_eq!(e.relation, UfoRelation::Requires);
        assert_eq!(e.source, ElementId::new("consumer"));
        assert_eq!(e.target, ElementId::new("provider"));
    }

    #[test]
    fn allocation_is_deliberately_not_lifted() {
        let el = element(json!({
            "@id": "alloc1", "@type": "Allocation",
            "source": { "@id": "a" }, "target": { "@id": "b" }
        }));
        assert!(ontological_edge_for(&el).is_none());
    }

    #[test]
    fn non_relationship_elements_are_not_lifted() {
        let el = element(json!({ "@id": "e1", "@type": "PartUsage", "name": "pump" }));
        assert!(ontological_edge_for(&el).is_none());
    }

    #[test]
    fn relationship_missing_an_endpoint_is_skipped_not_error() {
        let el = element(
            json!({ "@id": "rel1", "@type": "FeatureMembership", "owner": { "@id": "e1" } }),
        );
        assert!(ontological_edge_for(&el).is_none());
    }

    #[test]
    fn build_ufo_graph_collects_only_relationship_edges() {
        let snap = snapshot(vec![
            element(json!({ "@id": "e1", "@type": "PartUsage", "name": "pump" })),
            element(json!({ "@id": "e2", "@type": "PartDefinition", "name": "Assembly" })),
            element(json!({
                "@id": "rel1", "@type": "FeatureMembership",
                "owner": { "@id": "e2" }, "member": { "@id": "e1" }
            })),
            element(json!({
                "@id": "rel2", "@type": "Dependency",
                "client": "e1", "supplier": "e2"
            })),
        ]);
        let graph = build_ufo_graph(&snap);
        assert_eq!(graph.len(), 2);
        assert_eq!(graph[0].id, "rel1");
        assert_eq!(graph[1].id, "rel2");
    }

    #[test]
    fn to_sysgraph_gives_definitions_kind_and_usages_role() {
        use ufo_types::stereotype::UfoStereotype;

        let snap = snapshot(vec![
            element(json!({ "@id": "e1", "@type": "PartUsage", "name": "pump" })),
            element(json!({ "@id": "e2", "@type": "PartDefinition", "name": "Assembly" })),
            element(json!({ "@id": "e3", "@type": "Package", "name": "root" })),
        ]);
        let graph = to_sysgraph(&snap);

        let usage = graph.node(&ElementId::new("e1")).expect("e1 node");
        assert_eq!(usage.label.as_deref(), Some("pump"));
        assert_eq!(usage.stereotype, UfoStereotype::Role("PartUsage".into()));

        let definition = graph.node(&ElementId::new("e2")).expect("e2 node");
        assert_eq!(
            definition.stereotype,
            UfoStereotype::Kind("PartDefinition".into())
        );

        let package = graph.node(&ElementId::new("e3")).expect("e3 node");
        assert_eq!(package.stereotype, UfoStereotype::Kind("Package".into()));
    }

    #[test]
    fn to_sysgraph_excludes_relationship_elements_from_nodes_but_keeps_their_edges() {
        let snap = snapshot(vec![
            element(json!({ "@id": "e1", "@type": "PartUsage", "name": "pump" })),
            element(json!({ "@id": "e2", "@type": "PartDefinition", "name": "Assembly" })),
            element(json!({
                "@id": "rel1", "@type": "FeatureMembership",
                "owner": { "@id": "e2" }, "member": { "@id": "e1" }
            })),
        ]);
        let graph = to_sysgraph(&snap);
        assert_eq!(
            graph.nodes.len(),
            2,
            "only the two non-relationship elements"
        );
        assert_eq!(graph.edges.len(), 1);
        assert!(
            graph.dangling_edges().is_empty(),
            "both endpoints of rel1 have nodes: {:?}",
            graph.dangling_edges()
        );
    }

    #[test]
    fn to_sysgraph_element_with_no_name_gets_no_label() {
        let snap = snapshot(vec![element(json!({ "@id": "e1", "@type": "PartUsage" }))]);
        let graph = to_sysgraph(&snap);
        let node = graph.node(&ElementId::new("e1")).expect("e1 node");
        assert_eq!(node.label, None);
    }
}
