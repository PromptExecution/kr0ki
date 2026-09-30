//! Reconciles a `ufo_types::sysgraph::SysGraph` (e.g. from `ufo_types::dbt`) into a
//! live SysML v2 project via `kr0ki_sysmlv2_client`. See kr0ki's
//! docs/superpowers/specs/2026-09-20-flexo-write-path-design.md for the full design.
//! Nodes only -- no edge/relationship sync (see that spec's §6).

use crate::sync_engine::sync_managed;
pub use crate::sync_engine::{SyncConfig, SyncError, MAX_SYNC_ATTEMPTS};
use kr0ki_sysmlv2_client::{Commit, DataVersion, Element, Ref, SysmlV2Client};
use std::collections::{BTreeMap, BTreeSet};
use ufo_types::sysgraph::SysGraph;

/// An element's `identifier` field, if present. `identifier` lives in `Element`'s
/// flattened `fields` map (not a dedicated struct field) -- it's an OMG-API-defined
/// field this crate doesn't otherwise model.
fn element_identifier(element: &Element) -> Option<&str> {
    element.fields.get("identifier").and_then(|v| v.as_str())
}

/// `graph` restricted to nodes whose id starts with `dbt:`. Used on the
/// `sync_dbt_graph` side of the diff to mirror the `dbt:`-prefix filter already
/// applied to fetched elements -- the two sides of `build_changeset`'s diff must be
/// filtered symmetrically, or a non-`dbt:` node gets (re-)created every sync.
fn filter_dbt_nodes(graph: &SysGraph) -> SysGraph {
    SysGraph {
        nodes: graph
            .nodes
            .iter()
            .filter(|n| n.id.0.starts_with("dbt:"))
            .cloned()
            .collect(),
        edges: Vec::new(),
    }
}

/// Diff `fetched_elements` (the project's current elements, already known to be
/// `dbt:`-prefixed by identifier -- filtering happens before this call, in
/// `sync_dbt_graph`) against `graph`'s nodes (also expected to be pre-filtered to
/// `dbt:`-prefixed identifiers by the caller), producing the create/update/delete
/// changeset. Returns an empty `Vec` when nothing changed.
pub(crate) fn build_changeset(
    fetched_elements: &[Element],
    graph: &SysGraph,
) -> Result<Vec<DataVersion>, SyncError> {
    // Both sides are keyed by identifier, so a repeated identifier would silently drop
    // an element (never updated or deleted) or create the same one twice. Refuse instead.
    let mut duplicates = BTreeSet::new();
    let mut by_identifier: BTreeMap<&str, &Element> = BTreeMap::new();
    for e in fetched_elements {
        if let Some(id) = element_identifier(e) {
            if by_identifier.insert(id, e).is_some() {
                duplicates.insert(id.to_string());
            }
        }
    }
    let mut graph_ids = BTreeSet::new();
    for node in &graph.nodes {
        if !graph_ids.insert(node.id.0.as_str()) {
            duplicates.insert(node.id.0.clone());
        }
    }
    if !duplicates.is_empty() {
        return Err(SyncError::DuplicateIdentifier(
            duplicates.into_iter().collect(),
        ));
    }

    let mut changes = Vec::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();

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
                if existing.name() != Some(label) {
                    // update -- DataVersion.payload is a full replacement of the
                    // element's data-resource state, not a patch: the OMG reference
                    // implementation (JpaCommitDao.persist) never merges an incoming
                    // payload with the element's prior version, so any field this
                    // payload omits comes back null/empty on the new version. Start
                    // from the element's current fields (already fetched for the
                    // diff above) and only overwrite `name`, so ownership/other
                    // attributes another tool set survive this sync untouched.
                    let mut payload = existing.fields.clone();
                    payload.insert(
                        "@type".to_string(),
                        serde_json::Value::String(existing.ty().to_string()),
                    );
                    payload.insert(
                        "name".to_string(),
                        serde_json::Value::String(label.to_string()),
                    );
                    changes.push(DataVersion {
                        type_: "DataVersion",
                        payload: Some(serde_json::Value::Object(payload)),
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

    Ok(changes)
}

/// Sync `graph`'s `dbt:`-prefixed nodes into the project named by `config`, as at
/// most one new commit. Returns `Ok(Some(commit))` with the *existing* head commit,
/// unchanged, when the diff produces no changes but a commit already exists; returns
/// `Ok(None)` when there is nothing to sync AND no commit exists yet -- this function
/// never posts an empty commit. Head resolution, `previousCommit`, and conflict retry
/// are handled by [`crate::sync_engine`]; see the design spec for the full algorithm.
pub async fn sync_dbt_graph(
    client: &SysmlV2Client,
    graph: &SysGraph,
    config: &SyncConfig,
) -> Result<Option<Commit>, SyncError> {
    let dbt_graph = filter_dbt_nodes(graph);
    sync_managed(
        client,
        config,
        |e| element_identifier(e).is_some_and(|id| id.starts_with("dbt:")),
        |managed| build_changeset(managed, &dbt_graph),
    )
    .await
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

        let changes = build_changeset(&[], &graph).unwrap();

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

        let changes = build_changeset(&fetched, &graph).unwrap();

        assert!(changes.is_empty());
    }

    #[test]
    fn changed_label_produces_an_update_with_identity() {
        let mut graph = SysGraph::new();
        graph.push_node(node("dbt:model.a", "A (staging)"));
        let fetched = vec![element("srv-1", "dbt:model.a", "A (marts)")];

        let changes = build_changeset(&fetched, &graph).unwrap();

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].identity.as_ref().unwrap().at_id, "srv-1");
        assert_eq!(changes[0].payload.as_ref().unwrap()["name"], "A (staging)");
    }

    /// Regression guard: `DataVersion.payload` is a full replacement per the
    /// OMG reference implementation (no merge-with-prior-version step), so an
    /// update must carry forward every field the fetched element already had
    /// -- not just `name`/`identifier` -- or another tool's data (ownership,
    /// other attributes) silently disappears on the next sync.
    #[test]
    fn changed_label_update_preserves_other_existing_fields() {
        let mut graph = SysGraph::new();
        graph.push_node(node("dbt:model.a", "A (staging)"));
        let mut existing = element("srv-1", "dbt:model.a", "A (marts)");
        existing.fields.insert(
            "owner".to_string(),
            serde_json::Value::String("owner-team".to_string()),
        );
        let fetched = vec![existing];

        let changes = build_changeset(&fetched, &graph).unwrap();

        assert_eq!(changes.len(), 1);
        let payload = changes[0].payload.as_ref().unwrap();
        assert_eq!(payload["name"], "A (staging)");
        assert_eq!(payload["identifier"], "dbt:model.a");
        assert_eq!(payload["@type"], "PartUsage");
        assert_eq!(payload["owner"], "owner-team");
    }

    #[test]
    fn identifier_missing_from_fresh_graph_produces_a_delete() {
        let graph = SysGraph::new();
        let fetched = vec![element("srv-1", "dbt:model.gone", "Gone")];

        let changes = build_changeset(&fetched, &graph).unwrap();

        assert_eq!(changes.len(), 1);
        assert!(changes[0].payload.is_none());
        assert_eq!(changes[0].identity.as_ref().unwrap().at_id, "srv-1");
    }

    #[test]
    fn filter_dbt_nodes_excludes_non_dbt_prefixed_nodes() {
        let mut graph = SysGraph::new();
        graph.push_node(node("dbt:model.a", "A (marts)"));
        graph.push_node(node("not-dbt:model.b", "B (unrelated)"));

        let filtered = filter_dbt_nodes(&graph);

        assert_eq!(filtered.nodes.len(), 1);
        assert_eq!(filtered.nodes[0].id.0, "dbt:model.a");
    }

    #[test]
    fn non_dbt_prefixed_node_produces_no_create_once_filtered() {
        let mut graph = SysGraph::new();
        graph.push_node(node("not-dbt:model.b", "B (unrelated)"));

        let filtered = filter_dbt_nodes(&graph);
        let changes = build_changeset(&[], &filtered).unwrap();

        assert!(changes.is_empty());
    }

    #[test]
    fn duplicate_identifier_on_the_server_is_an_error_not_a_silent_collapse() {
        let fetched = vec![
            element("srv-1", "dbt:model.a", "A"),
            element("srv-2", "dbt:model.a", "A again"),
            element("srv-3", "dbt:model.b", "B"),
        ];
        let mut graph = SysGraph::new();
        graph.push_node(node("dbt:model.a", "A"));
        let err = build_changeset(&fetched, &graph).unwrap_err();
        assert!(
            matches!(&err, SyncError::DuplicateIdentifier(ids) if ids == &["dbt:model.a"]),
            "{err}"
        );
    }

    #[test]
    fn duplicate_identifier_in_the_graph_is_an_error_not_two_creates() {
        let mut graph = SysGraph::new();
        graph.push_node(node("dbt:model.a", "A"));
        graph.push_node(node("dbt:model.a", "A (dup)"));
        assert!(matches!(
            build_changeset(&[], &graph),
            Err(SyncError::DuplicateIdentifier(_))
        ));
    }
}
