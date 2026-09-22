//! Rule evaluation backends. [`RuleBackend`] is the seam a future
//! JEV-backed backend slots into without changing its output type (spec
//! §6) -- [`RegorusBackend`] is the only implementation today.

use crate::rule_docs::RuleDoc;
use serde::Deserialize;
use ufo_types::satisfies::{NodeId, SatisfiesResult};
use ufo_types::sysgraph::SysGraph;

pub trait RuleBackend {
    fn evaluate(&self, graph: &SysGraph, doc: &RuleDoc) -> SatisfiesResult;
}

/// Embeds `regorus::Engine` directly -- no sidecar process. Never panics or
/// propagates an error: a policy that fails to parse, a graph that fails to
/// serialize, or a malformed evaluation result all become
/// `SatisfiesResult::unknown()` for that one rule, so one bad rule doc can
/// never abort a whole recompute (spec §9).
pub struct RegorusBackend;

#[derive(Debug, Deserialize)]
struct ViolationEntry {
    element_id: String,
    reason: String,
}

impl RuleBackend for RegorusBackend {
    fn evaluate(&self, graph: &SysGraph, doc: &RuleDoc) -> SatisfiesResult {
        let mut engine = regorus::Engine::new();
        if engine
            .add_policy(doc.id.as_str().to_string(), doc.rego_source.clone())
            .is_err()
        {
            return SatisfiesResult::unknown();
        }
        let graph_json = match serde_json::to_string(graph) {
            Ok(s) => s,
            Err(_) => return SatisfiesResult::unknown(),
        };
        if engine.set_input_json(&graph_json).is_err() {
            return SatisfiesResult::unknown();
        }
        let raw = match engine.eval_rule("data.kr0ki.violations".to_string()) {
            Ok(v) => v,
            Err(_) => return SatisfiesResult::unknown(),
        };
        let raw_json = match serde_json::to_value(&raw) {
            Ok(v) => v,
            Err(_) => return SatisfiesResult::unknown(),
        };
        let entries: Vec<ViolationEntry> = match serde_json::from_value(raw_json) {
            Ok(entries) => entries,
            Err(_) => return SatisfiesResult::unknown(),
        };
        if entries.is_empty() {
            return SatisfiesResult::satisfied(1.0, Vec::new());
        }
        let reason = entries
            .iter()
            .map(|e| format!("{}: {}", e.element_id, e.reason))
            .collect::<Vec<_>>()
            .join("; ");
        let evidence_nodes = entries
            .iter()
            .map(|e| NodeId::new(format!("node:{}", e.element_id)))
            .collect();
        SatisfiesResult::violated(reason)
            .with_confidence(1.0)
            .with_evidence(evidence_nodes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_docs::RuleBackendKind;
    use ufo_types::satisfies::Disposition;
    use ufo_types::stereotype::UfoStereotype;
    use ufo_types::sysgraph::OntologicalNode;
    use ufo_types::sysml_model::ElementId;

    const RULE_SOURCE: &str = r#"
package kr0ki

violations := [v |
    some n
    input.nodes[n].label == "BadPart"
    v := {"element_id": input.nodes[n].id, "reason": "BadPart is not allowed"}
]
"#;

    fn doc(rego_source: &str) -> RuleDoc {
        RuleDoc {
            id: ElementId::new("rule:no-bad-parts"),
            name: "No BadPart allowed".into(),
            rego_source: rego_source.to_string(),
            backend: RuleBackendKind::Rego,
        }
    }

    #[test]
    fn reports_a_violation_for_a_matching_node() {
        let mut graph = SysGraph::new();
        graph.push_node(OntologicalNode::with_label(
            ElementId::new("elem-1"),
            UfoStereotype::Kind("PartUsage".into()),
            "BadPart",
        ));
        let result = RegorusBackend.evaluate(&graph, &doc(RULE_SOURCE));
        assert!(result.is_violated());
        assert_eq!(result.confidence, 1.0);
        assert_eq!(result.evidence_nodes, vec![NodeId::new("node:elem-1")]);
    }

    #[test]
    fn reports_satisfied_when_no_node_matches() {
        let mut graph = SysGraph::new();
        graph.push_node(OntologicalNode::with_label(
            ElementId::new("elem-1"),
            UfoStereotype::Kind("PartUsage".into()),
            "GoodPart",
        ));
        let result = RegorusBackend.evaluate(&graph, &doc(RULE_SOURCE));
        assert!(result.is_satisfied());
        assert_eq!(result.confidence, 1.0);
    }

    #[test]
    fn reports_unknown_for_a_policy_that_fails_to_parse() {
        let graph = SysGraph::new();
        let broken = doc("package kr0ki\n\nviolations := [v | v := ");
        let result = RegorusBackend.evaluate(&graph, &broken);
        assert_eq!(result.disposition, Disposition::Unknown);
        assert_eq!(result.confidence, 0.0);
    }
}
