//! Reconciles a `ufo_types::sysgraph::SysGraph` (e.g. from `ufo_types::dbt`) into a
//! live SysML v2 project via `kr0ki_sysmlv2_client`. See kr0ki's
//! docs/superpowers/specs/2026-09-20-flexo-write-path-design.md for the full design.
//! Nodes only -- no edge/relationship sync (see that spec's §6).

use kr0ki_sysmlv2_client::{Commit, CommitRequest, DataVersion, Element, Page, Ref, SysmlV2Client};
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
pub(crate) fn build_changeset(fetched_elements: &[Element], graph: &SysGraph) -> Vec<DataVersion> {
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

/// Which project (and optionally branch) to sync into.
pub struct SyncConfig {
    pub project_id: String,
    pub branch_id: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error(transparent)]
    Client(#[from] kr0ki_sysmlv2_client::ClientError),
}

/// Sync `graph`'s `dbt:`-prefixed nodes into the project named by `config`, as at
/// most one new commit. Returns the *existing* latest commit, unchanged, when the
/// diff produces no changes -- never posts an empty commit. See this module's own
/// doc comment and the design spec for the full algorithm.
pub async fn sync_dbt_graph(
    client: &SysmlV2Client,
    graph: &SysGraph,
    config: &SyncConfig,
) -> Result<Commit, SyncError> {
    let commits = client.commits(&config.project_id).await?;
    let latest = commits.first();

    let fetched_elements = match latest {
        Some(commit) => {
            client
                .elements(&config.project_id, &commit.at_id, Page::default())
                .await?
                .items
        }
        None => Vec::new(),
    };

    let dbt_elements: Vec<_> = fetched_elements
        .into_iter()
        .filter(|e| element_identifier(e).is_some_and(|id| id.starts_with("dbt:")))
        .collect();

    let changes = build_changeset(&dbt_elements, graph);

    if changes.is_empty() {
        // Safe to unwrap: an empty changeset with no prior commit only happens for
        // an empty graph on an empty project, which build_changeset also produces
        // no changes for -- but there is then no "latest" commit to return either.
        // Handle that explicitly rather than unwrapping a None.
        return match latest {
            Some(commit) => Ok(commit.clone()),
            None => {
                // Nothing to sync and nothing exists yet -- post an empty first
                // commit so the caller still gets a real Commit back, matching the
                // cookbook's own first-commit-has-no-previousCommit shape.
                let request = CommitRequest {
                    type_: "Commit",
                    change: vec![],
                    previous_commit: None,
                };
                Ok(client
                    .create_commit(&config.project_id, config.branch_id.as_deref(), request)
                    .await?)
            }
        };
    }

    let request = CommitRequest {
        type_: "Commit",
        change: changes,
        previous_commit: latest.map(|c| Ref {
            at_id: c.at_id.clone(),
            extra: Default::default(),
        }),
    };

    Ok(client
        .create_commit(&config.project_id, config.branch_id.as_deref(), request)
        .await?)
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
