//! A small, in-memory RDF index over SysML v2 query results.
//!
//! This is deliberately neither a triplestore nor a SPARQL endpoint. It is a
//! disposable derived cache with two query shapes and one structural check.

use std::collections::BTreeSet;
use std::sync::Mutex;

use oxrdf::{Literal, NamedNode, NamedOrBlankNode, Term, Triple};

const KR0KI_NS: &str = "http://kr0ki.promptexecution.com/ontology#";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryShape {
    TriplesAbout,
    RelatedVia,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapeViolation {
    pub subject: String,
    pub rule: &'static str,
    pub message: String,
}

fn local_node(local_name: &str) -> NamedNode {
    NamedNode::new(format!("{KR0KI_NS}{local_name}"))
        .expect("SysML element ids must be valid IRI local-name segments")
}

fn short_name(iri: &str) -> String {
    iri.rsplit('#').next().unwrap_or(iri).to_string()
}

fn subject_name(subject: &NamedOrBlankNode) -> String {
    match subject {
        NamedOrBlankNode::NamedNode(node) => short_name(node.as_str()),
        NamedOrBlankNode::BlankNode(node) => node.to_string(),
    }
}

#[derive(Debug, Default)]
pub struct GraphStore {
    triples: Mutex<Vec<Triple>>,
}

fn insert_unique(triples: &mut Vec<Triple>, triple: Triple) {
    if !triples.contains(&triple) {
        triples.push(triple);
    }
}

impl GraphStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_element_triples(
        &self,
        project_id: &str,
        commit_id: &str,
        element: &kr0ki_sysmlv2_client::Element,
    ) {
        let subject = local_node(element.id());
        let mut triples = self.triples.lock().expect("graph_store mutex poisoned");
        insert_unique(
            &mut triples,
            Triple::new(
                subject.clone(),
                local_node("type"),
                Literal::new_simple_literal(element.ty()),
            ),
        );
        if let Some(name) = element.name() {
            insert_unique(
                &mut triples,
                Triple::new(
                    subject.clone(),
                    local_node("name"),
                    Literal::new_simple_literal(name),
                ),
            );
        }
        insert_unique(
            &mut triples,
            Triple::new(
                subject.clone(),
                local_node("sourceProject"),
                Literal::new_simple_literal(project_id),
            ),
        );
        insert_unique(
            &mut triples,
            Triple::new(
                subject,
                local_node("sourceCommit"),
                Literal::new_simple_literal(commit_id),
            ),
        );
    }

    pub fn insert_relationship_triple(
        &self,
        subject_id: &str,
        relation_kind: &str,
        object_id: &str,
    ) {
        let mut triples = self.triples.lock().expect("graph_store mutex poisoned");
        insert_unique(
            &mut triples,
            Triple::new(
                local_node(subject_id),
                local_node(relation_kind),
                Term::NamedNode(local_node(object_id)),
            ),
        );
    }

    pub fn query(&self, shape: QueryShape, subject: Option<&str>) -> Vec<(String, String, String)> {
        let Some(subject) = subject else {
            return Vec::new();
        };
        let subject = local_node(subject);
        self.triples
            .lock()
            .expect("graph_store mutex poisoned")
            .iter()
            .filter(|triple| triple.subject == NamedOrBlankNode::NamedNode(subject.clone()))
            .filter(|triple| match shape {
                QueryShape::TriplesAbout => true,
                QueryShape::RelatedVia => !matches!(
                    short_name(triple.predicate.as_str()).as_str(),
                    "type" | "name" | "sourceProject" | "sourceCommit"
                ),
            })
            .map(|triple| {
                let object = match &triple.object {
                    Term::NamedNode(node) => short_name(node.as_str()),
                    Term::Literal(literal) => literal.value().to_string(),
                    Term::BlankNode(node) => node.to_string(),
                };
                (
                    subject_name(&triple.subject),
                    short_name(triple.predicate.as_str()),
                    object,
                )
            })
            .collect()
    }

    pub fn check_shapes(&self) -> Vec<ShapeViolation> {
        let triples = self.triples.lock().expect("graph_store mutex poisoned");
        let typed: BTreeSet<String> = triples
            .iter()
            .filter(|triple| short_name(triple.predicate.as_str()) == "type")
            .map(|triple| subject_name(&triple.subject))
            .collect();

        triples
            .iter()
            .filter_map(|triple| match &triple.object {
                Term::NamedNode(node) => Some(short_name(node.as_str())),
                _ => None,
            })
            .filter(|object| !typed.contains(object))
            .map(|subject| ShapeViolation {
                message: format!(
                    "{subject} is the target of a relationship but was never materialized with a type triple"
                ),
                subject,
                rule: "must_have_type",
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_element() -> kr0ki_sysmlv2_client::Element {
        serde_json::from_value(serde_json::json!({
            "@id": "elem-1", "@type": "PartUsage", "name": "Engine"
        }))
        .unwrap()
    }

    #[test]
    fn inserting_an_element_materializes_type_and_name() {
        let store = GraphStore::new();
        store.insert_element_triples("proj-1", "commit-1", &sample_element());
        let triples = store.query(QueryShape::TriplesAbout, Some("elem-1"));
        assert!(triples
            .iter()
            .any(|(_, predicate, object)| predicate == "type" && object == "PartUsage"));
        assert!(triples
            .iter()
            .any(|(_, predicate, object)| predicate == "name" && object == "Engine"));
    }

    #[test]
    fn unknown_subject_is_empty() {
        assert!(GraphStore::new()
            .query(QueryShape::TriplesAbout, Some("missing"))
            .is_empty());
    }

    #[test]
    fn related_via_excludes_element_metadata() {
        let store = GraphStore::new();
        store.insert_relationship_triple("elem-1", "Satisfy", "elem-2");
        assert!(store
            .query(QueryShape::RelatedVia, Some("elem-1"))
            .iter()
            .any(|(_, predicate, object)| predicate == "Satisfy" && object == "elem-2"));
    }

    #[test]
    fn check_shapes_flags_untyped_relationship_target() {
        let store = GraphStore::new();
        store.insert_relationship_triple("elem-1", "Satisfy", "orphan-elem");
        assert!(store
            .check_shapes()
            .iter()
            .any(|violation| violation.subject == "orphan-elem"));
    }

    #[test]
    fn typed_element_has_no_shape_violation() {
        let store = GraphStore::new();
        store.insert_element_triples("proj-1", "commit-1", &sample_element());
        assert!(store
            .check_shapes()
            .iter()
            .all(|violation| violation.subject != "elem-1"));
    }
}
