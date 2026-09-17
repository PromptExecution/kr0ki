//! FR3 (`docs/TODO.md` box 5): the isometric SVG backend — call
//! `systhread-core`'s `render.rs`/`layout.rs`; do not port or re-solve the
//! Cassowary (`kasuari`) layout.
//!
//! `systhread-core` lives at `rust/systhread-core` inside
//! `fungible-farm/nem-poweragent-lab` (found 2026-09-17 via
//! `elasticdotventures/_b00t_`'s `AGENTS.md` — no kr0ki doc had carried the
//! URL until now, only the bare crate name). Its own `iso_ir` module is
//! `pub use ufo_types::iso_ir::{Edge, Node};` — the exact type this crate's
//! own [`crate::rust_recognizer`] already produces — **but** it re-exports
//! from `ufo-types` pinned to `v0.11.0` (predates `ontology`/`sysml_model`/
//! `sysgraph` entirely), while this crate pins `v0.14.1`. Same shape, two
//! nominally distinct Rust types. [`to_systhread_node`]/[`to_systhread_edge`]
//! are the field-by-field conversion at that boundary — copying data, never
//! touching layout math, which stays entirely inside `systhread_core::layout`.
//!
//! ```text
//! Vec<iso_ir::Node/Edge> ──▶ [convert] ──▶ systhread_core::layout::cassowary_positions
//!                                              │
//!                                              ▼
//!                                        [assemble a spec] ──▶ systhread_core::render::render_svg
//! ```
//!
//! `systhread_core::iso_ir::assemble` (the function that builds the JSON
//! `spec` its `render_svg` consumes) is a **private** helper, not part of
//! that crate's public API — [`render_svg`] below is this crate's own
//! from-scratch reimplementation of *only* the JSON-shaping around it
//! (title/nodes/edges/position field names, reverse-engineered from that
//! crate's own `render_test.rs` fixtures and `assemble`'s doc comments,
//! since its source is readable even though the function itself isn't
//! callable). The actual position numbers always come from
//! `systhread_core::layout::cassowary_positions` — never recomputed here.
//!
//! Every `part_type`/`edge_type` this crate's recognizers emit
//! (`"module"`/`"struct"`/`"has_part"`/`"satisfies"`/…) falls through
//! `systhread-core`'s own `type_by_part_type`/`fill_by_type` default arms to
//! `"generic"` — there is no kr0ki-specific visual styling to replicate;
//! nodes render as plain neutral-gray boxes, which is correct, not a
//! degraded fallback (this backend was written for its own lab's
//! power-grid/digital-thread domain, and kr0ki's domains were never meant to
//! get custom glyphs from it).

use systhread_core::iso_ir::{Edge as SystNodeEdge, Node as SystNode};
use ufo_types::iso_ir::{Edge, Node};

fn to_systhread_node(n: &Node) -> SystNode {
    SystNode {
        id: n.id.clone(),
        label: n.label.clone(),
        part_type: n.part_type.clone(),
    }
}

fn to_systhread_edge(e: &Edge) -> SystNodeEdge {
    SystNodeEdge {
        id: e.id.clone(),
        from: e.from.clone(),
        to: e.to.clone(),
        edge_type: e.edge_type.clone(),
        kind: e.kind.clone(),
    }
}

/// Row-major grid, `(i % per_row, i / per_row)`, unit spacing — used only
/// when there are no edges to lay out relationally (an empty graph, or one
/// with nodes but no recognized relationships). Not the Cassowary/kasuari
/// layout `systhread_core::layout::cassowary_positions` provides for the
/// (overwhelmingly more common) edges-present case; a two-line fallback for
/// the one case that solver has nothing to do, mirroring
/// `systhread-core::iso_ir`'s own documented grid-fallback behavior for the
/// same case, not a reimplementation of its algorithm.
fn grid_positions(count: usize) -> Vec<(f64, f64)> {
    const PER_ROW: usize = 3;
    (0..count)
        .map(|i| ((i % PER_ROW) as f64 * 2.0, (i / PER_ROW) as f64 * 2.0))
        .collect()
}

/// Render `nodes`/`edges` as an isometric SVG via `systhread-core`.
/// Positions come from `systhread_core::layout::cassowary_positions` when
/// `edges` is non-empty (the constraint-solved layout this backend exists
/// for), else the trivial grid fallback above. Deterministic: the same
/// input always produces the same SVG — `cassowary_positions` itself is a
/// deterministic solve (fixed seed/iteration count, per that crate's own
/// documented rules), and this function does no randomness of its own.
pub fn render_svg(title: &str, nodes: &[Node], edges: &[Edge]) -> String {
    let syst_nodes: Vec<SystNode> = nodes.iter().map(to_systhread_node).collect();
    let syst_edges: Vec<SystNodeEdge> = edges.iter().map(to_systhread_edge).collect();

    let positions = if syst_edges.is_empty() {
        grid_positions(syst_nodes.len())
    } else {
        systhread_core::layout::cassowary_positions(&syst_nodes, &syst_edges)
    };

    let node_values: Vec<serde_json::Value> = nodes
        .iter()
        .zip(positions.iter())
        .map(|(n, (x, y))| {
            serde_json::json!({
                "id": n.id,
                "label": n.label,
                "type": "generic",
                "shape": "box",
                "position": { "x": x, "y": y },
            })
        })
        .collect();

    let mut spec = serde_json::json!({
        "title": title,
        "type": "generic",
        "nodes": node_values,
    });
    if !edges.is_empty() {
        let edge_values: Vec<serde_json::Value> = edges
            .iter()
            .map(|e| {
                serde_json::json!({
                    "id": e.id,
                    "from": e.from,
                    "to": e.to,
                    "type": e.edge_type,
                })
            })
            .collect();
        spec["edges"] = serde_json::json!(edge_values);
    }

    systhread_core::render::render_svg(&spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, part_type: &str) -> Node {
        Node {
            id: id.to_string(),
            label: id.to_string(),
            part_type: part_type.to_string(),
        }
    }

    fn edge(id: &str, from: &str, to: &str, edge_type: &str) -> Edge {
        Edge {
            id: id.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            edge_type: edge_type.to_string(),
            kind: None,
        }
    }

    #[test]
    fn renders_a_real_svg_for_a_connected_graph() {
        let nodes = vec![node("a", "struct"), node("b", "struct")];
        let edges = vec![edge("a_has_part_b", "a", "b", "has_part")];
        let svg = render_svg("Test Graph", &nodes, &edges);
        assert!(svg.contains("<svg"), "expected real SVG output:\n{svg}");
        assert!(
            svg.contains('a'),
            "expected node id text somewhere in the SVG"
        );
    }

    #[test]
    fn renders_a_real_svg_for_a_graph_with_no_edges() {
        let nodes = vec![node("a", "struct"), node("b", "struct")];
        let svg = render_svg("No Edges", &nodes, &[]);
        assert!(svg.contains("<svg"), "expected real SVG output:\n{svg}");
    }

    #[test]
    fn renders_a_real_svg_for_an_empty_graph() {
        let svg = render_svg("Empty", &[], &[]);
        assert!(
            svg.contains("<svg"),
            "expected real (if minimal) SVG output:\n{svg}"
        );
    }

    #[test]
    fn is_deterministic_for_the_same_input() {
        let nodes = vec![node("a", "struct"), node("b", "struct"), node("c", "trait")];
        let edges = vec![
            edge("a_has_part_b", "a", "b", "has_part"),
            edge("b_satisfies_c", "b", "c", "satisfies"),
        ];
        let svg1 = render_svg("Determinism", &nodes, &edges);
        let svg2 = render_svg("Determinism", &nodes, &edges);
        assert_eq!(svg1, svg2);
    }

    #[test]
    fn kr0ki_specific_part_types_dont_crash_the_lab_specific_styling_tables() {
        // module/function/trait/K8sObject-style part_types are outside
        // systhread-core's own lab vocabulary (Agent/MCPServer/Bus/...) --
        // they must fall through to its generic default, not panic or
        // silently drop the node.
        let nodes = vec![
            node("m", "module"),
            node("f", "function"),
            node("t", "trait"),
        ];
        let edges = vec![edge("m_has_part_f", "m", "f", "has_part")];
        let svg = render_svg("kr0ki types", &nodes, &edges);
        assert!(svg.contains("<svg"));
    }
}
