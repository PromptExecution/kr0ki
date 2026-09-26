//! Rule evaluation backends. [`RuleBackend`] is the seam a future
//! JEV-backed backend slots into without changing its output type (spec
//! §6) -- [`RegorusBackend`] is the only implementation today.

use crate::rule_docs::RuleDoc;
use regorus::utils::limits::ExecutionTimerConfig;
use serde::Deserialize;
use std::num::NonZeroU32;
use std::time::Duration;
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
        // Bound evaluation wall-clock time: regorus has no default execution
        // limit, and `recompute_and_evaluate` runs inline on an async axum
        // handler (not via `spawn_blocking`), so a pathological rule doc
        // (e.g. a large nested comprehension) could otherwise pin a tokio
        // worker thread indefinitely. A timeout here surfaces as an `Err`
        // from `eval_rule` below, which the existing error handling already
        // maps to `SatisfiesResult::unknown()` -- no other change needed.
        engine.set_execution_timer_config(ExecutionTimerConfig {
            limit: Duration::from_millis(500),
            check_interval: NonZeroU32::new(1000).expect("1000 is nonzero"),
        });
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

    /// Proves the 500ms `ExecutionTimerConfig` wired into
    /// `RegorusBackend::evaluate` actually bounds evaluation: a doubly-nested
    /// comprehension over 50,000 x 50,000 index pairs (2.5 billion checks) is
    /// evaluated by regorus's tree-walking interpreter far too slowly to
    /// finish in 500ms on any realistic machine, so the engine's own
    /// wall-clock check (ticked on every loop iteration, per
    /// `regorus::Engine`'s `check_interval`) aborts evaluation with an
    /// error -- which the existing `eval_rule` error handling already maps
    /// to `SatisfiesResult::unknown()`. This test would hang instead of
    /// failing fast if the timer config were ever dropped, so it also
    /// serves as a regression guard for that wiring.
    #[test]
    fn reports_unknown_when_evaluation_exceeds_the_execution_time_limit() {
        let pathological = r#"
package kr0ki

violations := [v |
    r := numbers.range(1, 50000)
    some i
    some j
    r[i] == r[j]
    v := {"element_id": sprintf("%d-%d", [i, j]), "reason": "slow"}
]
"#;
        let graph = SysGraph::new();
        let result = RegorusBackend.evaluate(&graph, &doc(pathological));
        assert_eq!(result.disposition, Disposition::Unknown);
        assert_eq!(result.confidence, 0.0);
    }

    /// Regression guard for the `Cargo.toml` `default-features = false`
    /// fix: `RuleDocument.rego_source` is untrusted, model-authored input
    /// (anyone able to write a SysML v2 element can supply one), so the
    /// `http` builtin (part of regorus's default/`full-opa` feature set,
    /// which registers `http.send`) must not resolve -- otherwise a rule
    /// doc could make kr0ki-server itself issue arbitrary outbound
    /// requests on every recompute. Bypasses `RegorusBackend::evaluate`
    /// (which maps every error, including a legitimate parse failure, to
    /// the same `SatisfiesResult::unknown()`) to assert directly on
    /// `regorus::Engine`'s own eval error, so this test actually proves
    /// the builtin is unresolved rather than merely that *some* error
    /// occurred.
    #[test]
    fn http_send_builtin_is_not_available() {
        let mut engine = regorus::Engine::new();
        engine
            .add_policy(
                "http-probe.rego".to_string(),
                r#"
package kr0ki

response := http.send({"method": "get", "url": "https://169.254.169.254/"})
"#
                .to_string(),
            )
            .expect("add_policy only parses -- an unresolved builtin call is still valid Rego");
        let err = engine
            .eval_rule("data.kr0ki.response".to_string())
            .expect_err(
                "http.send must not resolve: the regorus \"http\" feature must stay disabled \
                 in Cargo.toml (see the comment above the regorus dependency there)",
            );
        let message = err.to_string();
        assert!(
            message.contains("http.send") || message.to_lowercase().contains("unknown"),
            "expected an unresolved-builtin error, got: {message}"
        );
    }
}
