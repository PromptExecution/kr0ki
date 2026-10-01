//! FR1 — the `iso_ir → Mermaid / D2` adapter (`docs/TODO.md` box 5).
//!
//! Consumes box-4 output (`ufo_types::sysml_model::Relation`, as produced by
//! [`crate::sysml_lift`]), **not** a raw `ModelSnapshot` — this module has no
//! knowledge of `kr0ki-sysmlv2-client` or the KerML `@type` vocabulary, only
//! the already-typed `Relation` sum type.
//!
//! ```text
//! Vec<Relation> ──▶ [THIS] ──▶ D2 text  (existing DiagramFormat::D2 → RenderBackend)
//!  (box 4)                 └─▶ Mermaid text  (b00t-cli::dispatch_sysml precedent)
//! ```
//!
//! Two emitters, following [`crate::b00t_graph::D2Emitter`]'s D2 conventions
//! (reusing its `d2_quote`/`d2_escape_label` helpers directly — same quoting
//! rules, same "nodes first, then edges" determinism) and
//! `b00t-cli::dispatch_sysml::dispatch_chain_to_mermaid`'s Mermaid
//! conventions (a `%%{ init: ... }%%` header, `flowchart TD`, plain string
//! assembly — there is no Mermaid grammar validator in this workspace the
//! way `sysml-v2-parser` exists for SysML v2, so tests assert on string
//! content, not a parsed AST, matching that precedent exactly).
//!
//! # Node labels
//!
//! A [`Relation`] carries only opaque [`ElementId`]s, no separate human
//! label (unlike [`crate::b00t_graph`]'s `CytoscapeGraph`, whose nodes carry
//! both). Both emitters use the id's own string form as its label — a
//! richer label needs an id → display-name lookup, which the caller supplies
//! through [`to_d2_named`]: the **id stays the node key** (so an overlay can
//! find the element by its well-known identifier) and the name becomes the
//! label. [`to_d2`] is the no-lookup form and keeps the id as the label.
//!
//! # Edge decomposition
//!
//! [`Relation::endpoints`] already gives a stable, variant-correct endpoint
//! order (2 for every variant except [`Relation::Connection`], which can
//! carry more). This module turns each relation into one edge per
//! consecutive endpoint pair (`endpoints.windows(2)`) — a normal 2-endpoint
//! relation is one edge; an N-ended `Connection` becomes an (N-1)-edge
//! chain. Every edge is labeled with [`relation_label`], the KerML
//! relationship name (or, for [`Relation::Domain`], its own `kind` string —
//! never a generic "domain" placeholder, since `kind` is exactly the
//! information that would otherwise be lost).

use ufo_types::sysml_model::{ElementId, Relation};

use crate::b00t_graph::{d2_escape_label, d2_quote};

/// The KerML/SysML-v2 relationship name for a relation's edges, or (for
/// [`Relation::Domain`]) its own free-form `kind` classifier.
pub fn relation_label(relation: &Relation) -> &str {
    match relation {
        Relation::FeatureMembership { .. } => "feature_membership",
        Relation::Specialization { .. } => "specialization",
        Relation::Subsetting { .. } => "subsetting",
        Relation::Redefinition { .. } => "redefinition",
        Relation::Connection { .. } => "connection",
        Relation::Succession { .. } => "succession",
        Relation::Allocation { .. } => "allocation",
        Relation::Satisfy { .. } => "satisfy",
        Relation::Verify { .. } => "verify",
        Relation::Refine { .. } => "refine",
        Relation::Dependency { .. } => "dependency",
        Relation::Domain { kind, .. } => kind.as_str(),
        // Relation is #[non_exhaustive]; a future variant this module hasn't
        // been updated for still renders, just with a generic edge label.
        _ => "relation",
    }
}

/// One relation decomposed into labeled `(source, target)` edges.
fn edges_for(relation: &Relation) -> Vec<(&ElementId, &ElementId, &str)> {
    let label = relation_label(relation);
    relation
        .endpoints()
        .windows(2)
        .map(|pair| (pair[0], pair[1], label))
        .collect()
}

/// Every distinct [`ElementId`] referenced by `relations`, in first-seen
/// order (deterministic — no `HashSet` iteration order).
fn distinct_nodes(relations: &[Relation]) -> Vec<&ElementId> {
    let mut seen = Vec::new();
    for relation in relations {
        for id in relation.endpoints() {
            if !seen.contains(&id) {
                seen.push(id);
            }
        }
    }
    seen
}

/// Emit [D2](https://d2lang.com) text for `relations`: one node declaration
/// per distinct [`ElementId`], then one edge line per relation-edge (in
/// input order). Deterministic — same input always yields byte-identical
/// output.
pub fn to_d2(relations: &[Relation]) -> String {
    to_d2_named(relations, &std::collections::BTreeMap::new())
}

/// [`to_d2`] with display names: `names` maps an element id to its human name. The id remains the D2 node key
/// (the stable identifier a downstream overlay selects on); the name is only the label. An id without a
/// non-blank name keeps its id as the label, so output never loses information. Two elements may share a name.
pub fn to_d2_named(
    relations: &[Relation],
    names: &std::collections::BTreeMap<String, String>,
) -> String {
    let mut out = String::new();
    for id in distinct_nodes(relations) {
        let label = names
            .get(id.as_str())
            .map(|n| n.trim())
            .filter(|n| !n.is_empty())
            .unwrap_or(id.as_str());
        out.push_str(&format!(
            "{}: {}\n",
            d2_quote(id.as_str()),
            d2_escape_label(label)
        ));
    }
    for relation in relations {
        for (source, target, label) in edges_for(relation) {
            out.push_str(&format!(
                "{} -> {}: {}\n",
                d2_quote(source.as_str()),
                d2_quote(target.as_str()),
                d2_escape_label(label)
            ));
        }
    }
    out
}

/// Emit a Mermaid flowchart for `relations`. Mermaid auto-declares a node
/// the first time an id appears in an edge statement, so (unlike [`to_d2`])
/// no separate node-declaration pass is needed.
///
/// Mermaid node ids may not contain most punctuation; [`ElementId`]s can
/// (content hashes, KerML qualified names with `::`). Each id is therefore
/// mapped to a synthetic `n0`, `n1`, … id (first-seen order, matching
/// [`distinct_nodes`]) with the real id as that node's `["label"]` text —
/// the same "opaque id, separate display label" split D2 gets for free from
/// its quoted-string ids.
pub fn to_mermaid(relations: &[Relation]) -> String {
    let nodes = distinct_nodes(relations);
    let mermaid_id = |id: &ElementId| -> String {
        let idx = nodes
            .iter()
            .position(|n| *n == id)
            .expect("id was collected from these same relations");
        format!("n{idx}")
    };

    let mut out = String::from("%%{ init: { 'theme': 'neutral' } }%%\nflowchart TD\n");
    for (idx, id) in nodes.iter().enumerate() {
        out.push_str(&format!(
            "    n{idx}[\"{}\"]\n",
            mermaid_escape_label(id.as_str())
        ));
    }
    for relation in relations {
        for (source, target, label) in edges_for(relation) {
            out.push_str(&format!(
                "    {} -->|{}| {}\n",
                mermaid_id(source),
                mermaid_escape_label(label),
                mermaid_id(target)
            ));
        }
    }
    out
}

/// Mermaid edge/node labels are quoted with `["..."]`/`|...|`; neutralize
/// characters that would otherwise end the quoted span or the edge-label
/// pipe early.
fn mermaid_escape_label(label: &str) -> String {
    label.replace('\n', " ").replace('"', "'").replace('|', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_relations() -> Vec<Relation> {
        vec![
            Relation::FeatureMembership {
                owner: ElementId::new("pkg"),
                member: ElementId::new("svc-a"),
            },
            Relation::Connection {
                ends: vec![
                    ElementId::new("svc-a"),
                    ElementId::new("svc-b"),
                    ElementId::new("svc-c"),
                ],
            },
            Relation::Domain {
                source: ElementId::new("svc-a"),
                target: ElementId::new("svc-c"),
                kind: "traces_to".to_string(),
            },
        ]
    }

    #[test]
    fn d2_declares_every_distinct_node_once_in_first_seen_order() {
        let d2 = to_d2(&sample_relations());
        let node_lines: Vec<&str> = d2.lines().filter(|l| !l.contains("->")).collect();
        assert_eq!(
            node_lines,
            vec![
                "\"pkg\": \"pkg\"",
                "\"svc-a\": \"svc-a\"",
                "\"svc-b\": \"svc-b\"",
                "\"svc-c\": \"svc-c\"",
            ]
        );
    }

    #[test]
    fn d2_connection_with_three_ends_becomes_a_two_edge_chain() {
        let d2 = to_d2(&sample_relations());
        assert!(d2.contains("\"svc-a\" -> \"svc-b\": \"connection\""));
        assert!(d2.contains("\"svc-b\" -> \"svc-c\": \"connection\""));
        assert!(!d2.contains("\"svc-a\" -> \"svc-c\": \"connection\""));
    }

    #[test]
    fn d2_domain_edge_is_labeled_with_its_own_kind_not_a_generic_placeholder() {
        let d2 = to_d2(&sample_relations());
        assert!(d2.contains("\"svc-a\" -> \"svc-c\": \"traces_to\""));
    }

    #[test]
    fn d2_output_is_deterministic() {
        let relations = sample_relations();
        assert_eq!(to_d2(&relations), to_d2(&relations));
    }

    #[test]
    fn mermaid_is_a_flowchart_with_a_generated_header() {
        let mermaid = to_mermaid(&sample_relations());
        assert!(mermaid.starts_with("%%{ init: { 'theme': 'neutral' } }%%\nflowchart TD\n"));
    }

    #[test]
    fn mermaid_declares_every_node_with_its_id_as_the_label() {
        let mermaid = to_mermaid(&sample_relations());
        for id in ["pkg", "svc-a", "svc-b", "svc-c"] {
            assert!(
                mermaid.contains(&format!("[\"{id}\"]")),
                "missing node label {id} in:\n{mermaid}"
            );
        }
    }

    #[test]
    fn mermaid_edges_carry_the_relation_label() {
        let mermaid = to_mermaid(&sample_relations());
        assert!(mermaid.contains("-->|feature_membership|"));
        assert!(mermaid.contains("-->|connection|"));
        assert!(mermaid.contains("-->|traces_to|"));
    }

    #[test]
    fn empty_input_produces_a_valid_empty_diagram() {
        assert_eq!(to_d2(&[]), "");
        assert_eq!(
            to_mermaid(&[]),
            "%%{ init: { 'theme': 'neutral' } }%%\nflowchart TD\n"
        );
    }

    #[test]
    fn relation_label_covers_every_variant_with_a_non_empty_string() {
        let samples = [
            Relation::FeatureMembership {
                owner: ElementId::new("a"),
                member: ElementId::new("b"),
            },
            Relation::Specialization {
                specific: ElementId::new("a"),
                general: ElementId::new("b"),
            },
            Relation::Subsetting {
                subset: ElementId::new("a"),
                superset: ElementId::new("b"),
            },
            Relation::Redefinition {
                redefining: ElementId::new("a"),
                redefined: ElementId::new("b"),
            },
            Relation::Connection {
                ends: vec![ElementId::new("a"), ElementId::new("b")],
            },
            Relation::Succession {
                source: ElementId::new("a"),
                target: ElementId::new("b"),
            },
            Relation::Allocation {
                source: ElementId::new("a"),
                target: ElementId::new("b"),
            },
            Relation::Satisfy {
                requirement: ElementId::new("a"),
                subject: ElementId::new("b"),
            },
            Relation::Verify {
                requirement: ElementId::new("a"),
                by: ElementId::new("b"),
            },
            Relation::Refine {
                refined: ElementId::new("a"),
                refining: ElementId::new("b"),
            },
            Relation::Dependency {
                client: ElementId::new("a"),
                supplier: ElementId::new("b"),
            },
            Relation::Domain {
                source: ElementId::new("a"),
                target: ElementId::new("b"),
                kind: "custom".to_string(),
            },
        ];
        for relation in &samples {
            assert!(!relation_label(relation).is_empty(), "{relation:?}");
        }
    }

    #[test]
    fn named_d2_keeps_the_id_as_the_key_and_uses_the_name_as_the_label() {
        let names = std::collections::BTreeMap::from([
            ("svc-a".to_string(), "Engine".to_string()),
            ("svc-b".to_string(), "   ".to_string()), // blank: falls back to the id
        ]);
        let d2 = to_d2_named(&sample_relations(), &names);
        assert!(d2.contains("\"svc-a\": \"Engine\""), "{d2}");
        assert!(d2.contains("\"svc-b\": \"svc-b\""), "{d2}");
        assert!(d2.contains("\"svc-c\": \"svc-c\""), "{d2}");
        // edges still reference the stable ids, never the names
        assert!(d2.contains("\"svc-a\" -> \"svc-b\": \"connection\""));
        assert!(!d2.contains("Engine\" ->"));
    }

    #[test]
    fn names_are_escaped_like_any_label_and_the_unnamed_form_is_unchanged() {
        let names = std::collections::BTreeMap::from([(
            "svc-a".to_string(),
            "a \"quoted\" name".to_string(),
        )]);
        let d2 = to_d2_named(&sample_relations(), &names);
        assert!(
            !d2.contains("a \"quoted\" name"),
            "unescaped quote leaked: {d2}"
        );
        assert_eq!(
            to_d2(&sample_relations()),
            to_d2_named(&sample_relations(), &Default::default())
        );
    }
}
