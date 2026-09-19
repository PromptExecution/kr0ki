//! Diagram adapter for an already-induced requirements view.
//!
//! It consumes [`crate::requirements::ViewResult`] rather than a ReqIF or
//! Flexo payload, preserving the semantic-layer/renderer boundary.

use std::collections::BTreeMap;

use crate::b00t_graph::{d2_escape_label, d2_quote};
use crate::requirements::ViewResult;

/// Deterministically lower an induced requirement view to D2 source.
pub fn to_d2(view: &ViewResult) -> String {
    let mut labels = BTreeMap::new();
    for requirement in &view.graph.requirements {
        labels.insert(requirement.id.as_str(), requirement.title.as_str());
    }
    for evidence in &view.graph.evidence {
        labels.insert(evidence.id.as_str(), evidence.label.as_str());
    }
    let mut output = String::new();
    for (id, label) in labels {
        output.push_str(&format!("{}: {}\n", d2_quote(id), d2_escape_label(label)));
    }
    for relation in &view.graph.relations {
        output.push_str(&format!(
            "{} -> {}: {:?} ({})\n",
            d2_quote(&relation.source),
            d2_quote(&relation.target),
            relation.kind,
            match relation.authority {
                crate::requirements::RelationAuthority::Asserted => "asserted",
                crate::requirements::RelationAuthority::Inferred(_) => "inferred",
                crate::requirements::RelationAuthority::Proposed(_) => "proposed",
            },
        ));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::to_d2;
    use crate::requirements::{
        BaselineIdentity, Provenance, RelationAuthority, Requirement, RequirementGraph,
        RequirementRelation, RequirementRelationKind, TraversalDirection, ViewKind, ViewRequest,
        ViewScope,
    };
    use std::collections::BTreeMap;

    #[test]
    fn emits_only_the_nodes_and_edges_in_the_induced_view() {
        let baseline = BaselineIdentity {
            id: "b".into(),
            revision: "c".into(),
            import_artifact_sha256: None,
            exported_baseline_sha256: None,
        };
        let provenance = Provenance {
            source_uri: "x".into(),
            artifact_sha256: None,
            locator: None,
        };
        let graph = RequirementGraph {
            baseline: baseline.clone(),
            requirements: vec![
                Requirement {
                    id: "r1".into(),
                    title: "One".into(),
                    text: "".into(),
                    baseline: baseline.clone(),
                    provenance: provenance.clone(),
                    attributes: BTreeMap::new(),
                    evidence: vec![],
                },
                Requirement {
                    id: "r2".into(),
                    title: "Two".into(),
                    text: "".into(),
                    baseline,
                    provenance: provenance.clone(),
                    attributes: BTreeMap::new(),
                    evidence: vec![],
                },
            ],
            evidence: vec![],
            relations: vec![RequirementRelation {
                id: "e".into(),
                source: "r1".into(),
                target: "r2".into(),
                kind: RequirementRelationKind::Contains,
                authority: RelationAuthority::Asserted,
                provenance,
                promotion: None,
            }],
        };
        let view = graph
            .view(&ViewRequest {
                kind: ViewKind::Decomposition,
                scope: ViewScope::Authoritative,
                root_id: None,
                max_depth: None,
                direction: TraversalDirection::Downstream,
                confirmed_behaviour: None,
            })
            .unwrap();
        assert_eq!(
            to_d2(&view),
            "\"r1\": One\n\"r2\": Two\n\"r1\" -> \"r2\": Contains (asserted)\n"
        );
    }
}
