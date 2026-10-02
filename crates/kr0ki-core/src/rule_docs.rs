//! Rule-document extraction: `@type: "RuleDocument"` elements in a
//! `ModelSnapshot`, carrying Rego source in a `"rego"` field
//! (this crate's own element-shape convention — see
//! `docs/superpowers/specs/2026-09-22-requirements-rules-system-design.md`
//! §3's revision note). ID format is not constrained — the OMG API assigns
//! server-generated IDs on create, so kr0ki cannot assume any prefix.

use kr0ki_sysmlv2_client::ModelSnapshot;
use ufo_types::sysml_model::ElementId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleBackendKind {
    Rego,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleDoc {
    pub id: ElementId,
    pub name: String,
    pub rego_source: String,
    pub backend: RuleBackendKind,
}

/// Extract every rule document from `snapshot`, in either of two shapes:
///
/// - `@type: "RuleDocument"` with a `rego` field (this crate's own convention; needs a server that accepts custom
///   element types), or
/// - the standard KerML `TextualRepresentation` with `language: "rego"` and the source in `body`. A strictly typed
///   server (the OMG pilot, which answers HTTP 500 to an unknown `@type`) accepts this one; verified live 2026-10-02.
///
/// An element of either shape missing its source is silently skipped — same "never abort the whole snapshot's graph
/// build" convention `ufo_graph.rs` already uses for malformed relationship elements.
pub fn extract_rule_docs(snapshot: &ModelSnapshot) -> Vec<RuleDoc> {
    snapshot
        .elements
        .iter()
        .filter(|el| {
            el.ty() == "RuleDocument"
                || (el.ty() == "TextualRepresentation"
                    && el
                        .get("language")
                        .and_then(|l| l.as_str())
                        .is_some_and(|l| l.eq_ignore_ascii_case("rego")))
        })
        .filter_map(|el| {
            let rego_source = el
                .get("rego")
                .or_else(|| el.get("body"))?
                .as_str()?
                .to_string();
            let name = el
                .name()
                .or_else(|| el.get("declaredName").and_then(|n| n.as_str()))
                .unwrap_or_else(|| el.id())
                .to_string();
            Some(RuleDoc {
                id: ElementId::new(el.id()),
                name,
                rego_source,
                backend: RuleBackendKind::Rego,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn element(json: serde_json::Value) -> kr0ki_sysmlv2_client::Element {
        serde_json::from_value(json).unwrap()
    }

    fn snapshot(elements: Vec<kr0ki_sysmlv2_client::Element>) -> ModelSnapshot {
        ModelSnapshot {
            project_id: "p1".into(),
            commit_id: "c1".into(),
            roots: Vec::new(),
            content_hash: "test".into(),
            elements,
        }
    }

    #[test]
    fn extracts_a_well_formed_rule_document() {
        let snap = snapshot(vec![element(json!({
            "@id": "rule:no-bad-parts",
            "@type": "RuleDocument",
            "name": "No BadPart allowed",
            "rego": "package kr0ki\n\nviolations := []\n"
        }))]);
        let docs = extract_rule_docs(&snap);
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].id, ElementId::new("rule:no-bad-parts"));
        assert_eq!(docs[0].name, "No BadPart allowed");
        assert_eq!(docs[0].backend, RuleBackendKind::Rego);
    }

    #[test]
    fn falls_back_to_id_when_name_is_absent() {
        let snap = snapshot(vec![element(json!({
            "@id": "rule:unnamed",
            "@type": "RuleDocument",
            "rego": "package kr0ki\n\nviolations := []\n"
        }))]);
        let docs = extract_rule_docs(&snap);
        assert_eq!(docs[0].name, "rule:unnamed");
    }

    #[test]
    fn extracts_a_rego_textual_representation_as_the_standard_encoding() {
        let snap = snapshot(vec![
            element(
                json!({"@id": "t1", "@type": "TextualRepresentation", "declaredName": "R", "language": "rego", "body": "package kr0ki\n"}),
            ),
            element(
                json!({"@id": "t2", "@type": "TextualRepresentation", "language": "ocl", "body": "not rego"}),
            ),
            element(json!({"@id": "t3", "@type": "TextualRepresentation", "language": "rego"})),
        ]);
        let docs = extract_rule_docs(&snap);
        assert_eq!(docs.len(), 1, "{docs:?}");
        assert_eq!(
            (docs[0].name.as_str(), docs[0].rego_source.as_str()),
            ("R", "package kr0ki\n")
        );
    }

    #[test]
    fn skips_non_rule_document_elements() {
        let snap = snapshot(vec![element(
            json!({"@id": "part-1", "@type": "PartUsage"}),
        )]);
        assert!(extract_rule_docs(&snap).is_empty());
    }

    #[test]
    fn skips_rule_document_missing_the_rego_field() {
        let snap = snapshot(vec![element(json!({
            "@id": "rule:broken",
            "@type": "RuleDocument"
        }))]);
        assert!(extract_rule_docs(&snap).is_empty());
    }

    #[test]
    fn extracts_rule_document_with_server_assigned_id() {
        let snap = snapshot(vec![element(json!({
            "@id": "elem-42",
            "@type": "RuleDocument",
            "name": "Server-assigned ID rule",
            "rego": "package kr0ki\n\nviolations := []\n"
        }))]);
        let docs = extract_rule_docs(&snap);
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].id, ElementId::new("elem-42"));
        assert_eq!(docs[0].name, "Server-assigned ID rule");
    }
}
