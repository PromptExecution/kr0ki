//! KR-A01 acceptance case (VC-A01).
//!
//! "The requirement adapter shall preserve supported requirement identifiers, typed attributes
//! and asserted relations across ReqIF export and reimport."
//!
//! Acceptance: round-trip this baseline; compare semantic content. Report unsupported content
//! explicitly.

use std::collections::BTreeSet;

use kr0ki_core::assurance_baseline::load_baseline;
use kr0ki_core::reqif_roundtrip::{compare, round_trip, Difference, UnsupportedKind};
use ufo_types::mbse::requirements::{
    EvidenceRef, ModelIdentity, NonAuthoritativeRelation, Promotion, RelationAuthority,
    RequirementGraph, RequirementRelation, RequirementRelationKind,
};

const BASELINE_TOML: &str = include_str!("../../../docs/assurance/kr0ki.assurance.toml");

fn baseline() -> RequirementGraph {
    load_baseline(
        BASELINE_TOML,
        "docs/assurance/kr0ki.assurance.toml",
        "rev-1",
        Some("model-1"),
    )
    .expect("the checked-in baseline must load")
    .graph
}

fn relation(
    id: &str,
    source: &str,
    target: &str,
    authority: RelationAuthority,
    like: &RequirementRelation,
) -> RequirementRelation {
    RequirementRelation {
        id: id.into(),
        source: source.into(),
        target: target.into(),
        kind: RequirementRelationKind::Traces,
        authority,
        provenance: like.provenance.clone(),
        promotion: None,
    }
}

#[test]
fn this_baseline_round_trips_losslessly() {
    let graph = baseline();
    let rt = round_trip(&graph).expect("export and reimport must succeed");
    assert!(
        rt.report.is_lossless(),
        "round trip lost content: {:#?}",
        rt.report.differences
    );
    // Counted, not assumed: every node, attribute and asserted relation was compared.
    assert_eq!(rt.report.requirements_compared, graph.requirements.len());
    assert_eq!(rt.report.relations_compared, graph.relations.len());
    assert_eq!(
        rt.report.attributes_compared,
        graph
            .requirements
            .iter()
            .map(|r| r.attributes.len())
            .sum::<usize>()
    );
    assert!(rt.report.attributes_compared > 0);
}

#[test]
fn identifiers_and_relation_ids_survive_exactly() {
    let graph = baseline();
    let rt = round_trip(&graph).unwrap();
    let ids = |g: &RequirementGraph| -> BTreeSet<String> {
        g.requirements.iter().map(|r| r.id.clone()).collect()
    };
    assert_eq!(ids(&graph), ids(&rt.reimported));
    let rel_ids = |g: &RequirementGraph| -> BTreeSet<String> {
        g.relations.iter().map(|r| r.id.clone()).collect()
    };
    assert_eq!(rel_ids(&graph), rel_ids(&rt.reimported));
    assert_eq!(rt.reimported.baseline.id, graph.baseline.id);
    assert_eq!(rt.reimported.baseline.revision, graph.baseline.revision);
}

#[test]
fn profile_vocabularies_are_carried_as_typed_reqif_enumerations() {
    let rt = round_trip(&baseline()).unwrap();
    for needle in [
        "DATATYPE-DEFINITION-ENUMERATION",
        "ATTRIBUTE-DEFINITION-ENUMERATION",
        "ATTRIBUTE-VALUE-ENUMERATION",
        "LONG-NAME=\"status\"",
        "LONG-NAME=\"source_kind\"",
        "LONG-NAME=\"node_kind\"",
    ] {
        assert!(rt.xml.contains(needle), "exported ReqIF lacks {needle}");
    }
    // Free-text fields stay strings.
    assert!(rt.xml.contains("LONG-NAME=\"rationale\""));
    // The typed value comes back as the same string.
    let kr = rt
        .reimported
        .requirements
        .iter()
        .find(|r| r.id == "KR-A01")
        .unwrap();
    assert_eq!(
        kr.attributes.get("status").map(String::as_str),
        Some("gated")
    );
    assert_eq!(
        kr.attributes.get("node_kind").map(String::as_str),
        Some("requirement")
    );
}

#[test]
fn an_off_vocabulary_value_is_carried_as_text_not_lost_or_coerced() {
    let mut graph = baseline();
    graph
        .requirements
        .iter_mut()
        .find(|r| r.id == "KR-A03")
        .unwrap()
        .attributes
        .insert("status".into(), "on-hold-pending-legal".into());
    let rt = round_trip(&graph).unwrap();
    assert!(rt.report.is_lossless(), "{:#?}", rt.report.differences);
    let kr = rt
        .reimported
        .requirements
        .iter()
        .find(|r| r.id == "KR-A03")
        .unwrap();
    assert_eq!(kr.attributes["status"], "on-hold-pending-legal");
}

#[test]
fn for_this_baseline_the_only_unsupported_content_is_node_provenance() {
    let graph = baseline();
    let rt = round_trip(&graph).unwrap();
    let kinds: Vec<_> = rt.report.unsupported.iter().map(|u| u.kind).collect();
    assert_eq!(kinds, vec![UnsupportedKind::NodeProvenance]);
    let u = rt
        .report
        .unsupported_of(UnsupportedKind::NodeProvenance)
        .unwrap();
    assert_eq!(u.ids.len(), graph.requirements.len());
    // The importer's own additions are listed, not hidden.
    let normalised: Vec<_> = rt
        .report
        .normalised
        .iter()
        .map(|n| n.attribute.as_str())
        .collect();
    assert!(normalised.contains(&"text"));
}

#[test]
fn unsupported_content_is_reported_explicitly_with_the_ids_affected() {
    let mut graph = baseline();
    let like = graph.relations[0].clone();

    // Evidence node + an asserted relation that ends at it.
    graph.evidence.push(EvidenceRef {
        id: "EV-1".into(),
        label: "a test run".into(),
        uri: Some("file:///evidence/ev-1.json".into()),
        provenance: None,
    });
    graph.relations.push(relation(
        "rel-to-evidence",
        "VC-A01",
        "EV-1",
        RelationAuthority::Asserted,
        &like,
    ));
    // A proposed (review-only) relation between two requirements.
    graph.relations.push(relation(
        "rel-proposed",
        "KR-A01",
        "KR-A02",
        RelationAuthority::Proposed(NonAuthoritativeRelation {
            confidence: 0.6,
            rationale: "model guess".into(),
            evidence: vec![],
            model: ModelIdentity {
                name: "m".into(),
                version: "1".into(),
            },
        }),
        &like,
    ));
    // A promoted relation: carried as asserted, audit trail not carried.
    let mut promoted = relation(
        "rel-promoted",
        "KR-A01",
        "KR-A03",
        RelationAuthority::Asserted,
        &like,
    );
    promoted.promotion = Some(Promotion {
        actor: "reviewer".into(),
        rationale: "confirmed".into(),
        previous_status: "proposed".into(),
    });
    graph.relations.push(promoted);
    // A requirement-level evidence reference and an attribute key ReqIF import would rewrite.
    let kr = graph
        .requirements
        .iter_mut()
        .find(|r| r.id == "KR-A01")
        .unwrap();
    kr.evidence.push(EvidenceRef {
        id: "EV-2".into(),
        label: "x".into(),
        uri: None,
        provenance: None,
    });
    kr.attributes.insert("Bad-Key".into(), "v".into());
    graph.validate().expect("test graph must itself be valid");

    let rt = round_trip(&graph).unwrap();
    assert!(rt.report.is_lossless(), "{:#?}", rt.report.differences);

    let ids = |k: UnsupportedKind| rt.report.unsupported_of(k).map(|u| u.ids.clone());
    assert_eq!(
        ids(UnsupportedKind::EvidenceNodes),
        Some(vec!["EV-1".to_string()])
    );
    assert_eq!(
        ids(UnsupportedKind::NonAssertedRelations),
        Some(vec!["rel-proposed".to_string()])
    );
    assert_eq!(
        ids(UnsupportedKind::RelationsToNonRequirements),
        Some(vec!["rel-to-evidence".to_string()])
    );
    assert_eq!(
        ids(UnsupportedKind::RelationPromotion),
        Some(vec!["rel-promoted".to_string()])
    );
    assert_eq!(
        ids(UnsupportedKind::RequirementEvidenceRefs),
        Some(vec!["KR-A01".to_string()])
    );
    assert_eq!(
        ids(UnsupportedKind::AttributeKeyNotRepresentable),
        Some(vec!["KR-A01:Bad-Key".to_string()])
    );
}

#[test]
fn the_comparator_flags_real_loss_as_a_difference_not_as_unsupported() {
    let original = baseline();

    let mut dropped_attr = original.clone();
    dropped_attr
        .requirements
        .iter_mut()
        .find(|r| r.id == "KR-A01")
        .unwrap()
        .attributes
        .remove("owner");
    let report = compare(&original, &dropped_attr);
    assert!(!report.is_lossless());
    assert!(report.differences.contains(&Difference::AttributeLost {
        id: "KR-A01".into(),
        key: "owner".into()
    }));

    let mut changed_text = original.clone();
    changed_text
        .requirements
        .iter_mut()
        .find(|r| r.id == "KR-A02")
        .unwrap()
        .text = "different".into();
    assert!(compare(&original, &changed_text)
        .differences
        .iter()
        .any(|d| matches!(d, Difference::TextChanged { id, .. } if id == "KR-A02")));

    let mut lost_relation = original.clone();
    let removed = lost_relation.relations.remove(0);
    assert!(compare(&original, &lost_relation)
        .differences
        .contains(&Difference::RelationLost { id: removed.id }));

    let mut missing_node = original.clone();
    missing_node.requirements.retain(|r| r.id != "KR-A08");
    assert!(compare(&original, &missing_node).differences.contains(
        &Difference::MissingRequirement {
            id: "KR-A08".into()
        }
    ));

    let mut added = original.clone();
    let mut extra = added.requirements[0].clone();
    extra.id = "EXTRA".into();
    added.requirements.push(extra);
    assert!(compare(&original, &added)
        .differences
        .contains(&Difference::ExtraRequirement { id: "EXTRA".into() }));
}

#[test]
fn the_report_serialises_for_mcp_and_http() {
    let rt = round_trip(&baseline()).unwrap();
    let json = serde_json::to_value(&rt.report).unwrap();
    assert_eq!(json["differences"], serde_json::json!([]));
    assert_eq!(json["unsupported"][0]["kind"], "node_provenance");
    assert!(json["requirements_compared"].as_u64().unwrap() > 0);
}

#[test]
fn export_is_deterministic_so_the_baseline_digest_is_reproducible() {
    let a = round_trip(&baseline()).unwrap();
    let b = round_trip(&baseline()).unwrap();
    assert_eq!(a.xml_sha256, b.xml_sha256);
}
