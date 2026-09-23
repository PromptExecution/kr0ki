//! Round-trip contract for `reqif_export`, independent of Flexo/network:
//! parse a real fixture -> RequirementGraph -> export_bundle_to_xml ->
//! re-parse -> assert semantic equality (ids/titles/texts/asserted
//! relations -- not byte-identical XML; ReqIF's own attribute ordering
//! isn't meaningful, per design doc §3).

use kr0ki_core::reqif_export::export_bundle_to_xml;
use ufo_types::reqif::{parse_and_lower, ReqIfAdapterConfig};

const FIXTURE: &[u8] = include_bytes!("fixtures/reqif/roundtrip.reqif");

fn config() -> ReqIfAdapterConfig {
    ReqIfAdapterConfig {
        source_uri: "test:roundtrip".to_string(),
        revision: "r1".to_string(),
        import_artifact_sha256: None,
    }
}

#[test]
fn roundtrip_preserves_ids_titles_texts_and_asserted_relations() {
    let original = parse_and_lower(FIXTURE, &config()).expect("fixture must parse and lower");

    let xml = export_bundle_to_xml(&original).expect("export must succeed");

    let reexported_bundle = reqrs::ReqIfParser::parse_str(&xml).expect("exported XML must re-parse");
    let reimported = ufo_types::reqif::bundle_to_requirement_graph(&reexported_bundle, &config())
        .expect("re-parsed bundle must lower");

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

    let mut original_rels: Vec<(String, String, String)> = original
        .relations
        .iter()
        .map(|r| (r.source.clone(), r.target.clone(), format!("{:?}", r.kind)))
        .collect();
    let mut reimported_rels: Vec<(String, String, String)> = reimported
        .relations
        .iter()
        .map(|r| (r.source.clone(), r.target.clone(), format!("{:?}", r.kind)))
        .collect();
    original_rels.sort();
    reimported_rels.sort();
    assert_eq!(original_rels, reimported_rels);
}
