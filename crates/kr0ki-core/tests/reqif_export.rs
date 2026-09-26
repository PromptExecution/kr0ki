//! Round-trip contract for `reqif_export`, independent of Flexo/network:
//! parse a real fixture -> RequirementGraph -> export_bundle_to_xml ->
//! re-parse -> assert semantic equality (ids/titles/texts/asserted
//! relations -- not byte-identical XML; ReqIF's own attribute ordering
//! isn't meaningful, per design doc §3).

use kr0ki_core::reqif_export::export_bundle_to_xml;
use kr0ki_core::requirements::{
    BaselineIdentity, Provenance, RelationAuthority, Requirement, RequirementGraph,
    RequirementRelation, RequirementRelationKind,
};
use std::collections::BTreeMap;
use ufo_types::reqif::{parse_and_lower, ReqIfAdapterConfig};

const FIXTURE: &[u8] = include_bytes!("fixtures/reqif/roundtrip.reqif");

fn config() -> ReqIfAdapterConfig {
    ReqIfAdapterConfig {
        source_uri: "test:roundtrip".to_string(),
        revision: "r1".to_string(),
        import_artifact_sha256: None,
    }
}

fn baseline(id: &str) -> BaselineIdentity {
    BaselineIdentity {
        id: id.to_string(),
        revision: "r1".to_string(),
        import_artifact_sha256: None,
        exported_baseline_sha256: None,
    }
}

fn provenance() -> Provenance {
    Provenance {
        source_uri: "test:fixture".to_string(),
        artifact_sha256: None,
        locator: None,
    }
}

fn requirement(id: &str, title: &str, text: &str, baseline: &BaselineIdentity) -> Requirement {
    Requirement {
        id: id.to_string(),
        title: title.to_string(),
        text: text.to_string(),
        baseline: baseline.clone(),
        provenance: provenance(),
        attributes: BTreeMap::new(),
        evidence: Vec::new(),
    }
}

fn asserted_relation(
    id: &str,
    source: &str,
    target: &str,
    kind: RequirementRelationKind,
) -> RequirementRelation {
    RequirementRelation {
        id: id.to_string(),
        source: source.to_string(),
        target: target.to_string(),
        kind,
        authority: RelationAuthority::Asserted,
        provenance: provenance(),
        promotion: None,
    }
}

#[test]
fn roundtrip_preserves_ids_titles_texts_and_asserted_relations() {
    let original = parse_and_lower(FIXTURE, &config()).expect("fixture must parse and lower");

    let xml = export_bundle_to_xml(&original).expect("export must succeed");

    let reexported_bundle =
        reqrs::ReqIfParser::parse_str(&xml).expect("exported XML must re-parse");
    let reimported = ufo_types::reqif::bundle_to_requirement_graph(&reexported_bundle, &config())
        .expect("re-parsed bundle must lower");

    // The baseline/header id survives (export_bundle sets ReqIfHeader::identifier
    // to graph.baseline.id; re-import recovers BaselineIdentity::id from it when
    // the header identifier is non-empty).
    assert_eq!(reimported.baseline.id, original.baseline.id);

    let mut original_reqs: Vec<(String, String, String)> = original
        .requirements
        .iter()
        .map(|r| (r.id.clone(), r.title.clone(), r.text.clone()))
        .collect();
    let mut reimported_reqs: Vec<(String, String, String)> = reimported
        .requirements
        .iter()
        .map(|r| (r.id.clone(), r.title.clone(), r.text.clone()))
        .collect();
    original_reqs.sort();
    reimported_reqs.sort();
    assert_eq!(original_reqs, reimported_reqs);

    // Include the relation's own id (not just source/target/kind) -- ids must
    // survive the round trip too.
    let mut original_rels: Vec<(String, String, String, String)> = original
        .relations
        .iter()
        .map(|r| {
            (
                r.id.clone(),
                r.source.clone(),
                r.target.clone(),
                format!("{:?}", r.kind),
            )
        })
        .collect();
    let mut reimported_rels: Vec<(String, String, String, String)> = reimported
        .relations
        .iter()
        .map(|r| {
            (
                r.id.clone(),
                r.source.clone(),
                r.target.clone(),
                format!("{:?}", r.kind),
            )
        })
        .collect();
    original_rels.sort();
    reimported_rels.sort();
    assert_eq!(original_rels, reimported_rels);
}

#[test]
fn all_ten_relation_kinds_roundtrip_through_export_and_reimport() {
    use RequirementRelationKind::*;

    let bl = baseline("BL-ALL-KINDS");
    let kinds = [
        Contains,
        Derives,
        Refines,
        Requires,
        Satisfies,
        Verifies,
        Implements,
        Traces,
        AllocatedTo,
        Precedes,
    ];
    let relations: Vec<RequirementRelation> = kinds
        .iter()
        .enumerate()
        .map(|(i, kind)| asserted_relation(&format!("R{i}"), "REQ-A", "REQ-B", *kind))
        .collect();

    let graph = RequirementGraph {
        baseline: bl.clone(),
        requirements: vec![
            requirement("REQ-A", "Requirement A", "Text A.", &bl),
            requirement("REQ-B", "Requirement B", "Text B.", &bl),
        ],
        evidence: Vec::new(),
        relations,
    };

    let xml = export_bundle_to_xml(&graph).expect("export must succeed");
    let bundle = reqrs::ReqIfParser::parse_str(&xml).expect("exported XML must re-parse");
    let reimported = ufo_types::reqif::bundle_to_requirement_graph(&bundle, &config())
        .expect("re-parsed bundle must lower");

    assert_eq!(reimported.relations.len(), kinds.len());
    let mut reimported_by_id: BTreeMap<String, RequirementRelationKind> = reimported
        .relations
        .iter()
        .map(|r| (r.id.clone(), r.kind))
        .collect();
    for (i, kind) in kinds.iter().enumerate() {
        let id = format!("R{i}");
        let recovered = reimported_by_id
            .remove(&id)
            .unwrap_or_else(|| panic!("relation {id} missing after round-trip"));
        assert_eq!(
            recovered, *kind,
            "relation {id} round-tripped as {recovered:?}, expected {kind:?}"
        );
    }
}

#[test]
fn an_empty_requirement_graph_roundtrips_to_another_empty_graph() {
    let bl = baseline("BL-EMPTY");
    let graph = RequirementGraph {
        baseline: bl,
        requirements: Vec::new(),
        evidence: Vec::new(),
        relations: Vec::new(),
    };

    let xml = export_bundle_to_xml(&graph).expect("export of an empty graph must succeed");
    let bundle = reqrs::ReqIfParser::parse_str(&xml).expect("exported XML must re-parse");
    let reimported = ufo_types::reqif::bundle_to_requirement_graph(&bundle, &config())
        .expect("re-parsed bundle must lower");

    assert!(reimported.requirements.is_empty());
    assert!(reimported.relations.is_empty());
}
