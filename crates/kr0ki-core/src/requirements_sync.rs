//! Feeds rule-evaluation results into a project's `RequirementGraph` as
//! inferred `RequirementRelationKind::Satisfies` edges, reusing
//! `ufo_types::mbse::requirements`'s existing inferred/proposed-relation
//! promotion machinery rather than building a parallel one. See
//! `docs/superpowers/specs/2026-09-22-requirements-rules-system-design.md`
//! §5, including its revision note.

use crate::rule_docs::RuleDoc;
use kr0ki_sysmlv2_client::ModelSnapshot;
use ufo_types::mbse::requirements::{
    BaselineIdentity, EvidenceRef, ModelIdentity, NonAuthoritativeRelation, Provenance,
    RelationAuthority, Requirement, RequirementGraph, RequirementRelation, RequirementRelationKind,
};
use ufo_types::ontology::{OntologicalEdge, SourceAnchor};
use ufo_types::satisfies::{Disposition, SatisfiesResult};
use ufo_types::sysml_model::ElementId;

pub fn baseline_for(snapshot: &ModelSnapshot) -> BaselineIdentity {
    BaselineIdentity {
        id: snapshot.project_id.clone(),
        revision: snapshot.commit_id.clone(),
        import_artifact_sha256: None,
        exported_baseline_sha256: None,
    }
}

/// Idempotent: a rule doc already present (by id) is left untouched.
pub fn register_rule_doc(graph: &mut RequirementGraph, doc: &RuleDoc) {
    let id = doc.id.as_str().to_string();
    if graph.requirements.iter().any(|r| r.id == id) {
        return;
    }
    let mut attributes = std::collections::BTreeMap::new();
    attributes.insert("kind".to_string(), "rego-rule".to_string());
    graph.requirements.push(Requirement {
        id: id.clone(),
        title: doc.name.clone(),
        text: doc.rego_source.clone(),
        baseline: graph.baseline.clone(),
        provenance: Provenance {
            source_uri: format!("kerml:{id}"),
            artifact_sha256: None,
            locator: None,
        },
        attributes,
        evidence: Vec::new(),
    });
}

fn provenance_from_anchor(anchor: &SourceAnchor) -> Provenance {
    match anchor {
        SourceAnchor::KermlQualifiedName(qn) => Provenance {
            source_uri: format!("kerml:{qn}"),
            artifact_sha256: None,
            locator: None,
        },
        SourceAnchor::Vcs { repo, commit, path } => Provenance {
            source_uri: repo.clone().unwrap_or_else(|| commit.clone()),
            artifact_sha256: None,
            locator: path.clone(),
        },
        SourceAnchor::SysmlFile { path, line } => Provenance {
            source_uri: format!("sysml-file:{path}"),
            artifact_sha256: None,
            locator: line.map(|l| l.to_string()),
        },
        SourceAnchor::K8sObject {
            api_version,
            kind,
            namespace,
            name,
            ..
        } => Provenance {
            source_uri: format!(
                "k8s:{api_version}/{kind}/{}/{name}",
                namespace.as_deref().unwrap_or("")
            ),
            artifact_sha256: None,
            locator: None,
        },
        SourceAnchor::RustSpan { file, line, .. } => Provenance {
            source_uri: format!("rust-span:{file}"),
            artifact_sha256: None,
            locator: Some(line.to_string()),
        },
        SourceAnchor::SymbolPath(p) => Provenance {
            source_uri: format!("symbol:{p}"),
            artifact_sha256: None,
            locator: None,
        },
        SourceAnchor::Other(s) => Provenance {
            source_uri: s.clone(),
            artifact_sha256: None,
            locator: None,
        },
        // `SourceAnchor` is `#[non_exhaustive]` (defined in ufo-types): a
        // wildcard arm is required so a future variant added upstream
        // degrades gracefully here instead of breaking the build.
        _ => Provenance {
            source_uri: "unknown-anchor".to_string(),
            artifact_sha256: None,
            locator: None,
        },
    }
}

/// One `EvidenceRef` per incident edge (as source or target of
/// `element_id`) that carries provenance. `OntologicalNode` has no
/// provenance of its own (spec §5) -- this is the honest v1 approximation.
/// Idempotent by evidence id. Returns the registered ids, in edge order.
fn register_node_evidence(
    graph: &mut RequirementGraph,
    element_id: &ElementId,
    edges: &[OntologicalEdge],
) -> Vec<String> {
    let mut ids = Vec::new();
    let incident = edges
        .iter()
        .filter(|e| &e.source == element_id || &e.target == element_id);
    for (edge_idx, edge) in incident.enumerate() {
        for (anchor_idx, anchor) in edge.provenance.iter().enumerate() {
            let evidence_id = format!("node:{}#{edge_idx}-{anchor_idx}", element_id.as_str());
            if !graph.evidence.iter().any(|e| e.id == evidence_id) {
                graph.evidence.push(EvidenceRef {
                    id: evidence_id.clone(),
                    label: element_id.as_str().to_string(),
                    uri: None,
                    provenance: Some(provenance_from_anchor(anchor)),
                });
            }
            ids.push(evidence_id);
        }
    }
    ids
}

fn register_fallback_evidence(graph: &mut RequirementGraph, element_id: &ElementId) -> String {
    let id = format!("node:{}#none", element_id.as_str());
    if !graph.evidence.iter().any(|e| e.id == id) {
        graph.evidence.push(EvidenceRef {
            id: id.clone(),
            label: element_id.as_str().to_string(),
            uri: None,
            provenance: None,
        });
    }
    id
}

fn register_relation(
    graph: &mut RequirementGraph,
    doc: &RuleDoc,
    rationale: &str,
    confidence: f64,
    evidence_ids: Vec<String>,
) {
    let target = evidence_ids
        .first()
        .cloned()
        .unwrap_or_else(|| doc.id.as_str().to_string());
    let evidence_refs: Vec<EvidenceRef> = evidence_ids
        .iter()
        .filter_map(|id| graph.evidence.iter().find(|e| &e.id == id).cloned())
        .collect();
    let relation_id = format!("rule-eval:{}:{target}", doc.id.as_str());
    let relation = RequirementRelation {
        id: relation_id.clone(),
        source: doc.id.as_str().to_string(),
        target,
        kind: RequirementRelationKind::Satisfies,
        authority: RelationAuthority::Inferred(NonAuthoritativeRelation {
            confidence,
            rationale: rationale.to_string(),
            evidence: evidence_refs,
            model: ModelIdentity {
                name: "kr0ki-rule-eval".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        }),
        provenance: Provenance {
            source_uri: format!("rule-eval:{}", doc.id.as_str()),
            artifact_sha256: None,
            locator: None,
        },
        promotion: None,
    };
    if let Some(existing) = graph.relations.iter_mut().find(|r| r.id == relation_id) {
        *existing = relation;
    } else {
        graph.relations.push(relation);
    }
}

/// Feed one rule's evaluation result into `graph`. No-op for
/// `Disposition::Satisfied`/`Unknown` -- see Global Constraints.
pub fn register_violations(
    graph: &mut RequirementGraph,
    doc: &RuleDoc,
    edges: &[OntologicalEdge],
    result: &SatisfiesResult,
) {
    let reason = match &result.disposition {
        Disposition::Violated { reason } => reason.clone(),
        _ => return,
    };
    register_rule_doc(graph, doc);
    for node in &result.evidence_nodes {
        let element_id = match node.as_str().strip_prefix("node:") {
            Some(id) => ElementId::new(id),
            None => continue,
        };
        let mut evidence_ids = register_node_evidence(graph, &element_id, edges);
        if evidence_ids.is_empty() {
            evidence_ids.push(register_fallback_evidence(graph, &element_id));
        }
        register_relation(graph, doc, &reason, result.confidence, evidence_ids);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_docs::RuleBackendKind;
    use ufo_types::ontology::UfoRelation;
    use ufo_types::satisfies::NodeId;

    fn empty_graph() -> RequirementGraph {
        RequirementGraph {
            baseline: BaselineIdentity {
                id: "p1".into(),
                revision: "c1".into(),
                import_artifact_sha256: None,
                exported_baseline_sha256: None,
            },
            requirements: Vec::new(),
            evidence: Vec::new(),
            relations: Vec::new(),
        }
    }

    fn doc() -> RuleDoc {
        RuleDoc {
            id: ElementId::new("rule:no-bad-parts"),
            name: "No BadPart allowed".into(),
            rego_source: "package kr0ki\n\nviolations := []\n".into(),
            backend: RuleBackendKind::Rego,
        }
    }

    fn attested_edge() -> OntologicalEdge {
        OntologicalEdge {
            id: "fm-1".into(),
            source: ElementId::new("pkg-1"),
            target: ElementId::new("elem-1"),
            relation: UfoRelation::HasPart,
            occurrence: None,
            provenance: vec![SourceAnchor::KermlQualifiedName("fm-1".into())],
        }
    }

    #[test]
    fn a_violation_registers_the_rule_as_a_requirement_and_creates_a_relation() {
        let mut graph = empty_graph();
        let edges = vec![attested_edge()];
        let result = SatisfiesResult::violated("BadPart is not allowed".to_string())
            .with_confidence(1.0)
            .with_evidence(vec![NodeId::new("node:elem-1")]);
        register_violations(&mut graph, &doc(), &edges, &result);

        assert_eq!(graph.requirements.len(), 1);
        assert_eq!(graph.requirements[0].id, "rule:no-bad-parts");

        assert_eq!(graph.relations.len(), 1);
        let relation = &graph.relations[0];
        assert_eq!(relation.source, "rule:no-bad-parts");
        assert_eq!(relation.kind, RequirementRelationKind::Satisfies);
        match &relation.authority {
            RelationAuthority::Inferred(details) => {
                assert_eq!(details.confidence, 1.0);
                assert_eq!(details.rationale, "BadPart is not allowed");
                assert_eq!(details.evidence.len(), 1);
            }
            other => panic!("expected Inferred, got {other:?}"),
        }
        assert!(graph.validate().is_ok());
    }

    #[test]
    fn a_violated_node_with_no_provenance_bearing_edge_still_validates() {
        let mut graph = empty_graph();
        let result = SatisfiesResult::violated("no anchor yet".to_string())
            .with_confidence(1.0)
            .with_evidence(vec![NodeId::new("node:elem-1")]);
        register_violations(&mut graph, &doc(), &[], &result);
        assert_eq!(graph.relations.len(), 1);
        assert!(graph.validate().is_ok());
    }

    #[test]
    fn a_satisfied_result_registers_nothing() {
        let mut graph = empty_graph();
        let result = SatisfiesResult::satisfied(1.0, Vec::new());
        register_violations(&mut graph, &doc(), &[], &result);
        assert!(graph.requirements.is_empty());
        assert!(graph.relations.is_empty());
    }

    #[test]
    fn promoting_a_registered_relation_flips_its_authority() {
        let mut graph = empty_graph();
        let edges = vec![attested_edge()];
        let result = SatisfiesResult::violated("BadPart is not allowed".to_string())
            .with_confidence(1.0)
            .with_evidence(vec![NodeId::new("node:elem-1")]);
        register_violations(&mut graph, &doc(), &edges, &result);
        let relation_id = graph.relations[0].id.clone();
        graph
            .promote_relation(&relation_id, "brianh", "reviewed manually")
            .expect("promote");
        assert!(graph.relations[0].authority.is_asserted());
    }

    #[test]
    fn a_second_recompute_upserts_rather_than_duplicates() {
        let mut graph = empty_graph();
        let edges = vec![attested_edge()];
        let result = SatisfiesResult::violated("BadPart is not allowed".to_string())
            .with_confidence(1.0)
            .with_evidence(vec![NodeId::new("node:elem-1")]);
        register_violations(&mut graph, &doc(), &edges, &result);
        register_violations(&mut graph, &doc(), &edges, &result);
        assert_eq!(graph.requirements.len(), 1);
        assert_eq!(graph.relations.len(), 1);
    }
}
