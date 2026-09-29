//! Box 3→4 bridge for the Rust-source arm (`PLAN-KR0KI-003.md`).
//!
//! `rust_recognizer::{walk_and_recognize, recognize_source}` already emits
//! `ufo_types::iso_ir::{Node, Edge}` with `edge_type` strings drawn directly
//! from `docs/PATTERNS-rust-source.md`'s vocabulary table (`has_part`,
//! `flows_to`, `satisfies`, `requires`) — verified against that file's own
//! `UfoRelation::canonical_name()` mapping, so this is a straight rename,
//! not a reinterpretation. This module is the same role `k8s_recognizer.rs`
//! plays inline for the Kubernetes arm: converting the recognizer's raw
//! output into `OntologicalEdge`s that `sysml_lift::lift_edges` (box 4,
//! unchanged) can consume.
//!
//! `governed_by` (generic bounds, PATTERNS-rust-source.md §2 last-but-one
//! row) is not yet emitted by the recognizer — an edge_type this module
//! doesn't recognize is skipped rather than causing the whole conversion to
//! fail, so partial recognizer coverage degrades gracefully.

use ufo_types::iso_ir::Edge as IsoEdge;
use ufo_types::ontology::{OntologicalEdge, SourceAnchor, UfoRelation};
use ufo_types::sysml_model::ElementId;

/// Convert `edge_type` (as emitted by `rust_recognizer.rs`) into the
/// matching `UfoRelation`, or `None` for a classifier this bridge doesn't
/// (yet) know how to lift.
fn relation_for(edge_type: &str) -> Option<UfoRelation> {
    match edge_type {
        "has_part" => Some(UfoRelation::HasPart),
        "flows_to" => Some(UfoRelation::FlowsTo),
        "satisfies" => Some(UfoRelation::Satisfies),
        "requires" => Some(UfoRelation::Requires),
        "governed_by" => Some(UfoRelation::GovernedBy),
        _ => None,
    }
}

/// Lift `rust_recognizer`'s raw `iso_ir::Edge`s into `OntologicalEdge`s.
/// Provenance is `SourceAnchor::SymbolPath` on both ends (the recognizer
/// doesn't currently carry line/col spans — see module docs) rather than
/// `RustSpan`, so as not to assert location precision the source data
/// doesn't have.
pub fn lift_rust_edges(edges: &[IsoEdge]) -> Vec<OntologicalEdge> {
    edges
        .iter()
        .filter_map(|e| {
            let relation = relation_for(&e.edge_type)?;
            let source = ElementId::new(e.from.clone());
            let target = ElementId::new(e.to.clone());
            let id = format!("{}~{}~{}", e.from, relation.canonical_name(), e.to);
            let mut oe = OntologicalEdge::new(id, source, target, relation);
            oe.provenance.push(SourceAnchor::SymbolPath(e.from.clone()));
            Some(oe)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ufo_types::iso_ir::Edge;

    fn edge(from: &str, to: &str, edge_type: &str) -> Edge {
        Edge {
            id: format!("{from}_{edge_type}_{to}"),
            from: from.to_string(),
            to: to.to_string(),
            edge_type: edge_type.to_string(),
            kind: None,
        }
    }

    #[test]
    fn known_relations_lift_with_correct_variant() {
        let edges = vec![
            edge("Pkg", "Pkg::Item", "has_part"),
            edge("run", "helper", "flows_to"),
            edge("Car", "Drive", "satisfies"),
            edge("mycrate", "othercrate::Thing", "requires"),
        ];
        let lifted = lift_rust_edges(&edges);
        assert_eq!(lifted.len(), 4);
        assert_eq!(lifted[0].relation, UfoRelation::HasPart);
        assert_eq!(lifted[0].source, ElementId::new("Pkg"));
        assert_eq!(lifted[0].target, ElementId::new("Pkg::Item"));
        assert_eq!(lifted[1].relation, UfoRelation::FlowsTo);
        assert_eq!(lifted[2].relation, UfoRelation::Satisfies);
        assert_eq!(lifted[3].relation, UfoRelation::Requires);
    }

    #[test]
    fn provenance_is_symbol_path_not_rust_span() {
        let edges = vec![edge("a", "b", "has_part")];
        let lifted = lift_rust_edges(&edges);
        assert_eq!(
            lifted[0].provenance,
            vec![SourceAnchor::SymbolPath("a".to_string())]
        );
    }

    #[test]
    fn unrecognized_edge_type_is_skipped_not_fatal() {
        let edges = vec![
            edge("a", "b", "has_part"),
            edge("c", "d", "some_future_relation_this_bridge_does_not_know"),
        ];
        let lifted = lift_rust_edges(&edges);
        assert_eq!(lifted.len(), 1);
        assert_eq!(lifted[0].source, ElementId::new("a"));
    }
}
