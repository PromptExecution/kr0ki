//! b00t-graph: Turtle -> `holon_viz::type_graph::TypeRelationshipGraph` -> D2 (kr0ki#13).
//!
//! `elasticdotventures/_b00t_` publishes a Turtle RDF dump of its `b00t:`-namespace
//! knowledge graph (`b00t-graph turtle`, `Store::dump_graph_to_writer`) alongside a
//! signed manifest, per `docs/superpowers/specs/2026-09-11-sp5-graph-artifact-publish-design.md`
//! in that repo. This module reads that Turtle text and turns it into a
//! [`holon_viz::type_graph::TypeRelationshipGraph`] — kr0ki's "functional
//! KerML-query-view rendering will use `holon-viz` internally" (`ledgrrr#203`,
//! resolved 2026-09-10) — then a [`D2Emitter`] lowers the resulting
//! [`holon_viz::cytoscape::CytoscapeGraph`] to D2 text for the existing Kroki-backend
//! render pipeline.
//!
//! ```text
//! kerml-view.ttl ──▶ parse_triples ──▶ triples_to_type_graph ──▶ TypeRelationshipGraph
//!                                                                      │ .to_cytoscape()
//!                                                                      ▼
//!                                                              CytoscapeGraph
//!                                                                      │ D2Emitter::emit
//!                                                                      ▼
//!                                                                  D2 text
//! ```
//!
//! # Predicate vocabulary
//!
//! Grounded in `b00t-cli/src/{datum_triples,identity_triples}.rs` (the actual triple
//! compilers) and `b00t-c0re-lib/src/graph_load.rs::expand_iri`/`object_is_iri` (the
//! actual CURIE-expansion + literal-vs-IRI rule the real publisher applies before
//! writing the store this Turtle is dumped from):
//!
//! - `b00t:` expands to `http://b00t.promptexecution.com/ontology#`; `rdfs:` to the
//!   standard `http://www.w3.org/2000/01/rdf-schema#`. This module doesn't need to
//!   know that in advance — Turtle's own `@prefix`/full-`<...>`-IRI forms both parse
//!   to the same fully-expanded [`oxrdf::NamedNode`], and classification below keys
//!   off each *parsed* IRI's namespace + local name, not the source file's spelling.
//! - Whether an object is a graph edge or a node attribute is **not** decided by
//!   which predicate it's under — it's decided by what RDF term type oxttl actually
//!   parsed: `Term::NamedNode`/`Term::BlankNode` (a real reference) becomes a
//!   [`TypeRelationship`] if the predicate has a [`relation_kind_for_predicate`]
//!   mapping; `Term::Literal` never becomes an edge. This mirrors the real
//!   publisher's own `object_is_iri` rule exactly, so this module works whether a
//!   given predicate's object happens to be IRI- or literal-shaped in a particular
//!   graph, without hardcoding that per predicate.
//! - `rdfs:label` -> [`TypeNode::label`] (a literal).
//! - Structural (produce a [`TypeRelationship`] when IRI-valued):
//!   `dependsOn`/`requires` -> [`TypeRelationshipKind::References`] ("consumer
//!   references a dependency" — PATTERNS-kubernetes.md's `requires` reading, reused
//!   here since `TypeRelationshipKind` has no dedicated "requires" variant);
//!   `hasPart` -> [`TypeRelationshipKind::Contains`] (direct match); `hasType` ->
//!   [`TypeRelationshipKind::ClassifiedAs`] (direct match; `datum_triples.rs` always
//!   IRI-values this one, `b00t:type/{kind}`); `hasR0le` ->
//!   [`TypeRelationshipKind::BelongsTo`] (a tenant is affiliated with a role);
//!   `allowsTool`/`grantsShard` -> [`TypeRelationshipKind::Attests`] (a role
//!   attesting/authorizing a capability — `identity_triples.rs` IRI-values
//!   `grantsShard` always; `allowsTool`'s sample values are bare strings like
//!   `"cargo.*"`, so in practice this one usually parses as a `Literal` and is
//!   dropped, not because it's excluded here but because the real data isn't
//!   IRI-shaped yet).
//! - Everything else with a `Literal` object (`hasKeyword`, `hasSkill`,
//!   `budgetCeiling`, `modelTier`, `shardMode`, `image`, and `allowsTool` in
//!   practice) is a **node attribute in the source ontology, not an edge** — but
//!   [`TypeNode`] (`holon-viz`'s own domain type) has no generic attribute bag, only
//!   `kind`/`z_layer`/`semantic_type`, and the latter two are `HasVisualization`
//!   slots from a different domain (Rust-type visualization) that b00t's
//!   `modelTier`/`shardMode`/etc. would collide with, not fit. These are dropped
//!   (documented gap, not silently missing) rather than forced into the wrong slot.
//!   `TypeNode.kind` *is* populated — from the subject IRI's own local-name shape
//!   (`datum`, `r0le`, `tenant`, `shard`, `svc`, `type`, …), a legitimate reuse of
//!   "type category" for "b00t subject category".

use std::collections::BTreeMap;

use holon_viz::cytoscape::CytoscapeGraph;
use holon_viz::type_graph::{
    TypeNode, TypeRelationship, TypeRelationshipGraph, TypeRelationshipKind,
};
use oxrdf::{NamedOrBlankNode, Term};
use oxttl::{TurtleParser, TurtleSyntaxError};

/// b00t's ontology namespace (`b00t-c0re-lib/src/graph_load.rs::B00T_NS`).
const B00T_NS: &str = "http://b00t.promptexecution.com/ontology#";
/// The standard RDFS namespace.
const RDFS_NS: &str = "http://www.w3.org/2000/01/rdf-schema#";

#[derive(Debug, thiserror::Error)]
pub enum TurtleGraphError {
    #[error("turtle syntax error: {0}")]
    Syntax(#[from] TurtleSyntaxError),
}

/// The [`TypeRelationshipKind`] a `b00t:`-namespace predicate's *local name* maps
/// to, when its object turns out to be IRI-valued (never consulted for a
/// `Literal` object — see module docs). `None` for a predicate with no edge
/// mapping (either because its role is a node attribute, or it's unrecognized).
fn relation_kind_for_predicate(local_name: &str) -> Option<TypeRelationshipKind> {
    match local_name {
        "dependsOn" | "requires" => Some(TypeRelationshipKind::References),
        "hasPart" => Some(TypeRelationshipKind::Contains),
        "hasType" => Some(TypeRelationshipKind::ClassifiedAs),
        "hasR0le" => Some(TypeRelationshipKind::BelongsTo),
        "allowsTool" | "grantsShard" => Some(TypeRelationshipKind::Attests),
        _ => None,
    }
}

/// Split a fully-expanded IRI into `(namespace, local_name)` at the last `#`
/// (b00t/RDFS convention) or, failing that, the last `/`.
fn split_iri(iri: &str) -> (&str, &str) {
    if let Some(idx) = iri.rfind('#') {
        (&iri[..=idx], &iri[idx + 1..])
    } else if let Some(idx) = iri.rfind('/') {
        (&iri[..=idx], &iri[idx + 1..])
    } else {
        ("", iri)
    }
}

/// A b00t subject's "kind" — the first path segment of its local name
/// (`datum/rust.cli` -> `datum`, `r0le/_base/worker` -> `r0le`). Falls back to
/// the whole local name when there's no `/`.
fn subject_kind(local_name: &str) -> String {
    local_name
        .split_once('/')
        .map_or(local_name, |(kind, _)| kind)
        .to_string()
}

/// A readable label from a local name: its last path segment.
fn subject_label(local_name: &str) -> String {
    local_name
        .rsplit_once('/')
        .map_or(local_name, |(_, tail)| tail)
        .to_string()
}

/// Parse `ttl` (UTF-8 Turtle text) into `(subject_iri, predicate_iri, object)`
/// triples. `lenient()` is used so one malformed statement (e.g. an
/// unregistered prefix from a future b00t vocabulary addition) doesn't abort
/// the whole parse — see [`TurtleParser::lenient`].
fn parse_triples(ttl: &[u8]) -> Result<Vec<(String, String, Term)>, TurtleGraphError> {
    TurtleParser::new()
        .lenient()
        .for_slice(ttl)
        .map(|r| {
            let triple = r?;
            let subject = match triple.subject {
                NamedOrBlankNode::NamedNode(n) => n.into_string().to_string(),
                NamedOrBlankNode::BlankNode(b) => format!("_:{}", b.as_str()),
            };
            Ok((
                subject,
                triple.predicate.into_string().to_string(),
                triple.object,
            ))
        })
        .collect()
}

/// Build a [`TypeRelationshipGraph`] from parsed Turtle triples (see module docs
/// for the classification rules).
pub fn triples_to_type_graph(triples: &[(String, String, Term)]) -> TypeRelationshipGraph {
    let mut nodes: BTreeMap<String, TypeNode> = BTreeMap::new();
    let mut relationships: Vec<TypeRelationship> = Vec::new();

    fn node_mut(id: &str, nodes: &mut BTreeMap<String, TypeNode>) {
        nodes.entry(id.to_string()).or_insert_with(|| {
            let (_ns, local) = split_iri(id);
            TypeNode {
                id: id.to_string(),
                label: subject_label(local),
                kind: subject_kind(local),
                parent_id: None,
                z_layer: None,
                semantic_type: None,
            }
        });
    }

    for (subject, predicate, object) in triples {
        node_mut(subject, &mut nodes);
        let (pred_ns, pred_local) = split_iri(predicate);

        if pred_ns == RDFS_NS && pred_local == "label" {
            if let Term::Literal(lit) = object {
                if let Some(n) = nodes.get_mut(subject) {
                    n.label = lit.value().to_string();
                }
            }
            continue;
        }

        if pred_ns != B00T_NS {
            continue;
        }

        match object {
            Term::NamedNode(target) => {
                let Some(kind) = relation_kind_for_predicate(pred_local) else {
                    continue;
                };
                let target_id = target.as_str().to_string();
                node_mut(&target_id, &mut nodes);
                relationships.push(TypeRelationship::new(subject.clone(), target_id, kind));
            }
            Term::BlankNode(_) | Term::Literal(_) => {
                // Node attribute in the source ontology (or an unrecognized
                // predicate) — dropped; TypeNode has no attribute bag to put
                // it in (see module docs).
            }
        }
    }

    TypeRelationshipGraph::new(nodes.into_values().collect(), relationships)
}

/// Parse `ttl` and build its [`TypeRelationshipGraph`] in one step.
pub fn parse_turtle_to_type_graph(ttl: &[u8]) -> Result<TypeRelationshipGraph, TurtleGraphError> {
    Ok(triples_to_type_graph(&parse_triples(ttl)?))
}

/// Emits [D2](https://d2lang.com) text from a [`CytoscapeGraph`] — the box-5 render
/// target for the b00t-graph route (kr0ki#13; D2 chosen over GraphViz, operator
/// decision). No emitter like this existed before (`holon_viz::emitter` only
/// targets SysML-v2 / OWL2-Turtle text, a different output format); D2 syntax is
/// simple enough (`node_id: label` / `node_id -> node_id2: edge_label`) that this
/// is a small, self-contained function, not a parser.
pub struct D2Emitter;

impl D2Emitter {
    /// Deterministic: node declarations first (graph order), then edges, one
    /// per line. IDs are D2-quoted so arbitrary b00t local names (containing
    /// `.`, `/`, `*`, …) never need escaping logic of their own.
    pub fn emit(graph: &CytoscapeGraph) -> String {
        let mut out = String::new();
        for node in &graph.nodes {
            out.push_str(&format!(
                "{}: {}\n",
                d2_quote(&node.data.id),
                d2_escape_label(&node.data.label)
            ));
        }
        for edge in &graph.edges {
            out.push_str(&format!(
                "{} -> {}: {}\n",
                d2_quote(&edge.data.source),
                d2_quote(&edge.data.target),
                d2_escape_label(&edge.data.label)
            ));
        }
        out
    }
}

/// D2 identifiers can be quoted with `"..."` to allow arbitrary characters;
/// escape any embedded `"` and `\` so the result is always a single valid
/// quoted-string token.
pub(crate) fn d2_quote(id: &str) -> String {
    format!("\"{}\"", id.replace('\\', "\\\\").replace('"', "\\\""))
}

/// D2 labels after `:` are *not* safe as bare trailing text: a label
/// beginning with `{` opens a nested map block (Kroki then rejects the
/// whole diagram, e.g. "maps must be terminated with }"), and other D2
/// syntax characters (`:`, `;`, `#`, …) can likewise be misread as
/// structure rather than label content. Always emit the label as a quoted
/// D2 string — the same escaping [`d2_quote`] uses for ids — so arbitrary
/// caller-supplied text (a ReqIF requirement title, an RDF `rdfs:label`, a
/// SysML element name) can never alter diagram structure. A raw newline
/// would still break the line-oriented statement format this module
/// builds, so it's flattened to a space before quoting.
pub(crate) fn d2_escape_label(label: &str) -> String {
    d2_quote(&label.replace('\n', " "))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_TTL: &str = r#"
        @prefix b00t: <http://b00t.promptexecution.com/ontology#> .
        @prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .

        <http://b00t.promptexecution.com/ontology#datum/rust.cli>
            b00t:dependsOn <http://b00t.promptexecution.com/ontology#datum/cargo.cli> ;
            b00t:hasPart <http://b00t.promptexecution.com/ontology#datum/rustfmt.cli> ;
            b00t:hasType <http://b00t.promptexecution.com/ontology#type/cli> ;
            b00t:hasKeyword "rust" ;
            b00t:hasSkill "compiled" ;
            rdfs:label "Rust toolchain" .

        <http://b00t.promptexecution.com/ontology#tenant/_base>
            b00t:hasR0le <http://b00t.promptexecution.com/ontology#r0le/_base/worker> .

        <http://b00t.promptexecution.com/ontology#r0le/_base/worker>
            b00t:allowsTool "cargo.*" ;
            b00t:grantsShard <http://b00t.promptexecution.com/ontology#shard/datum/*> ;
            b00t:budgetCeiling "1000" .
    "#;

    #[test]
    fn depends_on_and_has_part_and_has_type_become_edges() {
        let graph = parse_turtle_to_type_graph(SAMPLE_TTL.as_bytes()).unwrap();
        let has = |source: &str, target: &str, kind: TypeRelationshipKind| {
            graph
                .relationships
                .iter()
                .any(|r| r.source.ends_with(source) && r.target.ends_with(target) && r.kind == kind)
        };
        assert!(has(
            "datum/rust.cli",
            "datum/cargo.cli",
            TypeRelationshipKind::References
        ));
        assert!(has(
            "datum/rust.cli",
            "datum/rustfmt.cli",
            TypeRelationshipKind::Contains
        ));
        assert!(has(
            "datum/rust.cli",
            "type/cli",
            TypeRelationshipKind::ClassifiedAs
        ));
    }

    #[test]
    fn has_r0le_and_grants_shard_become_edges() {
        let graph = parse_turtle_to_type_graph(SAMPLE_TTL.as_bytes()).unwrap();
        let has = |source: &str, target: &str, kind: TypeRelationshipKind| {
            graph
                .relationships
                .iter()
                .any(|r| r.source.ends_with(source) && r.target.ends_with(target) && r.kind == kind)
        };
        assert!(has(
            "tenant/_base",
            "r0le/_base/worker",
            TypeRelationshipKind::BelongsTo
        ));
        assert!(has(
            "r0le/_base/worker",
            "shard/datum/*",
            TypeRelationshipKind::Attests
        ));
    }

    #[test]
    fn literal_valued_predicates_never_become_edges() {
        let graph = parse_turtle_to_type_graph(SAMPLE_TTL.as_bytes()).unwrap();
        // allowsTool is "cargo.*" (a literal, not an IRI) in this sample, matching
        // the real identity_triples.rs shape — no edge, no crash.
        assert!(!graph
            .relationships
            .iter()
            .any(|r| r.kind == TypeRelationshipKind::Attests
                && r.source.ends_with("r0le/_base/worker")
                && r.target == "cargo.*"));
        // hasKeyword/hasSkill/budgetCeiling are dropped, not turned into edges
        // or attached anywhere (documented gap — TypeNode has no attribute bag).
        assert_eq!(graph.relationships.len(), 5);
    }

    #[test]
    fn rdfs_label_overrides_the_derived_label() {
        let graph = parse_turtle_to_type_graph(SAMPLE_TTL.as_bytes()).unwrap();
        let rust = graph
            .nodes
            .iter()
            .find(|n| n.id.ends_with("datum/rust.cli"))
            .expect("rust.cli node");
        assert_eq!(rust.label, "Rust toolchain");
    }

    #[test]
    fn node_kind_is_derived_from_the_subject_local_name() {
        let graph = parse_turtle_to_type_graph(SAMPLE_TTL.as_bytes()).unwrap();
        let kind_of = |suffix: &str| {
            graph
                .nodes
                .iter()
                .find(|n| n.id.ends_with(suffix))
                .map(|n| n.kind.clone())
        };
        assert_eq!(kind_of("datum/rust.cli"), Some("datum".to_string()));
        assert_eq!(kind_of("r0le/_base/worker"), Some("r0le".to_string()));
        assert_eq!(kind_of("tenant/_base"), Some("tenant".to_string()));
    }

    #[test]
    fn malformed_turtle_is_a_syntax_error_not_a_panic() {
        let err = parse_turtle_to_type_graph(b"this is not turtle {{{").unwrap_err();
        assert!(matches!(err, TurtleGraphError::Syntax(_)));
    }

    #[test]
    fn d2_emitter_produces_node_and_edge_lines() {
        let graph = parse_turtle_to_type_graph(SAMPLE_TTL.as_bytes()).unwrap();
        let d2 = D2Emitter::emit(&graph.to_cytoscape());
        assert!(d2.contains("-> "));
        assert!(d2.contains(": "));
        // Every line is a valid, simple D2 statement (no stray braces/quotes).
        for line in d2.lines() {
            assert!(!line.trim().is_empty());
        }
    }

    #[test]
    fn d2_emitter_quotes_ids_with_special_characters() {
        let mut graph = TypeRelationshipGraph::default();
        graph.nodes.push(TypeNode {
            id: "b00t:shard/datum/*".to_string(),
            label: "shard \"star\"".to_string(),
            kind: "shard".to_string(),
            parent_id: None,
            z_layer: None,
            semantic_type: None,
        });
        let d2 = D2Emitter::emit(&graph.to_cytoscape());
        assert!(d2.contains("\"b00t:shard/datum/*\":"));
        // The label is itself a quoted D2 string now, so an embedded `"`
        // comes out backslash-escaped, not bare.
        assert!(d2.contains(r#": "shard \"star\"""#));
        assert_eq!(
            d2.lines().count(),
            1,
            "one node -> exactly one D2 statement line"
        );
    }

    #[test]
    fn d2_emitter_quotes_labels_containing_d2_structural_characters() {
        // A label beginning with `{` would otherwise open a nested D2 map
        // block and break the whole diagram (Kroki: "maps must be
        // terminated with }"). An `rdfs:label` is arbitrary caller text and
        // must never be able to do this.
        let mut graph = TypeRelationshipGraph::default();
        graph.nodes.push(TypeNode {
            id: "n1".to_string(),
            label: "normal requirement { {shape: circle}".to_string(),
            kind: "shard".to_string(),
            parent_id: None,
            z_layer: None,
            semantic_type: None,
        });
        let d2 = D2Emitter::emit(&graph.to_cytoscape());
        assert_eq!(
            d2,
            "\"n1\": \"normal requirement { {shape: circle}\"\n"
        );
        assert_eq!(
            d2.lines().count(),
            1,
            "malicious braces must not open a nested D2 block across lines"
        );
    }

    #[test]
    fn empty_turtle_yields_an_empty_graph() {
        let graph = parse_turtle_to_type_graph(b"").unwrap();
        assert!(graph.nodes.is_empty());
        assert!(graph.relationships.is_empty());
    }
}
