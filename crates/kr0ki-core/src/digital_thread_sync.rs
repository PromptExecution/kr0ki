//! Reconciles a `ufo_types::sysgraph::SysGraph` (e.g. from `ufo_types::dbt`) into a
//! live SysML v2 project via `kr0ki_sysmlv2_client`. See kr0ki's
//! docs/superpowers/specs/2026-09-20-flexo-write-path-design.md for the full design.
//! Nodes only -- no edge/relationship sync (see that spec's §6).

use kr0ki_sysmlv2_client::{DataVersion, Element, Ref};
use ufo_types::sysgraph::SysGraph;

/// An element's `identifier` field, if present. `identifier` lives in `Element`'s
/// flattened `fields` map (not a dedicated struct field) -- it's an OMG-API-defined
/// field this crate doesn't otherwise model.
fn element_identifier(element: &Element) -> Option<&str> {
    element.fields.get("identifier").and_then(|v| v.as_str())
}

/// An element's `name` field, same reasoning as `element_identifier`.
fn element_name(element: &Element) -> Option<&str> {
    element.fields.get("name").and_then(|v| v.as_str())
}

/// Diff `fetched_elements` (the project's current elements, already known to be
/// `dbt:`-prefixed by identifier -- filtering happens before this call, in Task 3)
/// against `graph`'s nodes, producing the create/update/delete changeset. Returns an
/// empty `Vec` when nothing changed.
fn build_changeset(fetched_elements: &[Element], graph: &SysGraph) -> Vec<DataVersion> {
    use std::collections::BTreeMap;

    let by_identifier: BTreeMap<&str, &Element> = fetched_elements
        .iter()
        .filter_map(|e| element_identifier(e).map(|id| (id, e)))
        .collect();

    let mut changes = Vec::new();
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();

    for node in &graph.nodes {
        let identifier = node.id.0.as_str();
        seen.insert(identifier);
        let label = node.label.as_deref().unwrap_or_default();

        match by_identifier.get(identifier) {
            None => {
                // create
                changes.push(DataVersion {
                    type_: "DataVersion",
                    payload: Some(serde_json::json!({
                        "@type": "PartUsage",
                        "name": label,
                        "identifier": identifier,
                    })),
                    identity: None,
                });
            }
            Some(existing) => {
                if element_name(existing) != Some(label) {
                    // update
                    changes.push(DataVersion {
                        type_: "DataVersion",
                        payload: Some(serde_json::json!({
                            "@type": "PartUsage",
                            "name": label,
                            "identifier": identifier,
                        })),
                        identity: Some(Ref {
                            at_id: existing.id().to_string(),
                            extra: Default::default(),
                        }),
                    });
                }
                // else: unchanged, emit nothing
            }
        }
    }

    for (identifier, existing) in &by_identifier {
        if !seen.contains(identifier) {
            // delete: this dbt: identifier is no longer in the fresh graph
            changes.push(DataVersion {
                type_: "DataVersion",
                payload: None,
                identity: Some(Ref {
                    at_id: existing.id().to_string(),
                    extra: Default::default(),
                }),
            });
        }
    }

    changes
}

#[cfg(test)]
mod tests {
    use super::*;
    use ufo_types::stereotype::UfoStereotype;
    use ufo_types::sysgraph::OntologicalNode;
    use ufo_types::sysml_model::ElementId;

    fn element(at_id: &str, identifier: &str, name: &str) -> Element {
        serde_json::from_value(serde_json::json!({
            "@id": at_id,
            "@type": "PartUsage",
            "identifier": identifier,
            "name": name,
        }))
        .unwrap()
    }

    fn node(identifier: &str, label: &str) -> OntologicalNode {
        OntologicalNode::with_label(
            ElementId::new(identifier),
            UfoStereotype::Kind("DbtModel".into()),
            label,
        )
    }

    #[test]
    fn new_node_produces_a_create_with_no_identity() {
        let mut graph = SysGraph::new();
        graph.push_node(node("dbt:model.a", "A (marts)"));

        let changes = build_changeset(&[], &graph);

        assert_eq!(changes.len(), 1);
        assert!(changes[0].identity.is_none());
        let payload = changes[0].payload.as_ref().unwrap();
        assert_eq!(payload["identifier"], "dbt:model.a");
        assert_eq!(payload["name"], "A (marts)");
    }

    #[test]
    fn unchanged_node_produces_nothing() {
        let mut graph = SysGraph::new();
        graph.push_node(node("dbt:model.a", "A (marts)"));
        let fetched = vec![element("srv-1", "dbt:model.a", "A (marts)")];

        let changes = build_changeset(&fetched, &graph);

        assert!(changes.is_empty());
    }

    #[test]
    fn changed_label_produces_an_update_with_identity() {
        let mut graph = SysGraph::new();
        graph.push_node(node("dbt:model.a", "A (staging)"));
        let fetched = vec![element("srv-1", "dbt:model.a", "A (marts)")];

        let changes = build_changeset(&fetched, &graph);

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].identity.as_ref().unwrap().at_id, "srv-1");
        assert_eq!(changes[0].payload.as_ref().unwrap()["name"], "A (staging)");
    }

    #[test]
    fn identifier_missing_from_fresh_graph_produces_a_delete() {
        let graph = SysGraph::new();
        let fetched = vec![element("srv-1", "dbt:model.gone", "Gone")];

        let changes = build_changeset(&fetched, &graph);

        assert_eq!(changes.len(), 1);
        assert!(changes[0].payload.is_none());
        assert_eq!(changes[0].identity.as_ref().unwrap().at_id, "srv-1");
    }
}
