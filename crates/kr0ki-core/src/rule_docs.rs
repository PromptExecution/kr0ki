//! Rule-document extraction: `@type: "RuleDocument"` / `rule:`-prefixed
//! elements in a `ModelSnapshot`, carrying Rego source in a `"rego"` field
//! (this crate's own element-shape convention — see
//! `docs/superpowers/specs/2026-09-22-requirements-rules-system-design.md`
//! §3's revision note).

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

/// Extract every `RuleDocument` element from `snapshot`. An element whose
/// `@type` is `"RuleDocument"` but is missing its `rego` field, or whose id
/// doesn't carry the `rule:` prefix convention, is silently skipped — same
/// "never abort the whole snapshot's graph build" convention `ufo_graph.rs`
/// already uses for malformed relationship elements.
pub fn extract_rule_docs(snapshot: &ModelSnapshot) -> Vec<RuleDoc> {
    snapshot
        .elements
        .iter()
        .filter(|el| el.ty() == "RuleDocument" && el.id().starts_with("rule:"))
        .filter_map(|el| {
            let rego_source = el.get("rego")?.as_str()?.to_string();
            let name = el.name().unwrap_or_else(|| el.id()).to_string();
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
    fn skips_rule_typed_element_without_the_id_prefix_convention() {
        let snap = snapshot(vec![element(json!({
            "@id": "not-prefixed",
            "@type": "RuleDocument",
            "rego": "package kr0ki\n\nviolations := []\n"
        }))]);
        assert!(extract_rule_docs(&snap).is_empty());
    }
}
