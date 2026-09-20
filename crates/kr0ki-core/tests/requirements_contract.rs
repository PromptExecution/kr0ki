//! Consumer-level contract tests for `kr0ki_core::requirements`.
//!
//! The semantic model (`RequirementGraph`, the five viewpoints, promotion of
//! inferred/proposed relations) lives in `ufo_types::mbse::requirements`;
//! `kr0ki_core::requirements` is a `pub use` re-export (kr0ki M1,
//! 2026-09-19), and `kr0ki_core::requirements_render::to_d2` is the only
//! graph-to-D2 adapter kr0ki-core owns. This file exercises both exactly as
//! an external consumer would — through kr0ki-core's public API only, an
//! integration test rather than a `src/` unit test — to prove the
//! re-export/adapter seam itself behaves as the HTTP routes in
//! `kr0ki-server` depend on, not just that the upstream crate's own test
//! suite passes in isolation.

use std::collections::BTreeMap;

use kr0ki_core::requirements::{
    BaselineIdentity, EvidenceRef, ModelIdentity, NonAuthoritativeRelation, Provenance,
    RelationAuthority, Requirement, RequirementError, RequirementGraph, RequirementRelation,
    RequirementRelationKind, TraversalDirection, ViewKind, ViewRequest, ViewScope,
};
use kr0ki_core::requirements_render::to_d2;

fn baseline() -> BaselineIdentity {
    BaselineIdentity {
        id: "BL-1".into(),
        revision: "commit-1".into(),
        import_artifact_sha256: None,
        exported_baseline_sha256: None,
    }
}

fn provenance() -> Provenance {
    Provenance {
        source_uri: "reqif://fixture".into(),
        artifact_sha256: None,
        locator: None,
    }
}

fn requirement(id: &str, title: &str) -> Requirement {
    Requirement {
        id: id.into(),
        title: title.into(),
        text: title.into(),
        baseline: baseline(),
        provenance: provenance(),
        attributes: BTreeMap::new(),
        evidence: vec![],
    }
}

#[test]
fn promotion_is_isolated_until_an_explicit_human_decision() {
    let mut graph = RequirementGraph {
        baseline: baseline(),
        requirements: vec![requirement("r1", "One"), requirement("r2", "Two")],
        evidence: vec![],
        relations: vec![RequirementRelation {
            id: "candidate".into(),
            source: "r1".into(),
            target: "r2".into(),
            kind: RequirementRelationKind::Derives,
            authority: RelationAuthority::Proposed(NonAuthoritativeRelation {
                confidence: 0.9,
                rationale: "candidate".into(),
                evidence: vec![],
                model: ModelIdentity {
                    name: "opa".into(),
                    version: "1".into(),
                },
            }),
            provenance: provenance(),
            promotion: None,
        }],
    };
    let authoritative = ViewRequest {
        kind: ViewKind::Decomposition,
        scope: ViewScope::Authoritative,
        root_id: None,
        max_depth: None,
        direction: TraversalDirection::Downstream,
        confirmed_behaviour: None,
    };
    assert_eq!(graph.view(&authoritative).unwrap().graph.relations.len(), 0);

    graph
        .promote_relation("candidate", "alice", "reviewed")
        .unwrap();
    assert_eq!(graph.view(&authoritative).unwrap().graph.relations.len(), 1);
}

#[test]
fn impact_view_is_bounded_and_reports_cycles() {
    let graph = RequirementGraph {
        baseline: baseline(),
        requirements: vec![
            requirement("r1", "One"),
            requirement("r2", "Two"),
            requirement("r3", "Three"),
        ],
        evidence: vec![],
        relations: vec![
            RequirementRelation {
                id: "a".into(),
                source: "r1".into(),
                target: "r2".into(),
                kind: RequirementRelationKind::Requires,
                authority: RelationAuthority::Asserted,
                provenance: provenance(),
                promotion: None,
            },
            RequirementRelation {
                id: "cycle".into(),
                source: "r2".into(),
                target: "r1".into(),
                kind: RequirementRelationKind::Requires,
                authority: RelationAuthority::Asserted,
                provenance: provenance(),
                promotion: None,
            },
        ],
    };
    let result = graph
        .view(&ViewRequest {
            kind: ViewKind::Impact,
            scope: ViewScope::Authoritative,
            root_id: Some("r1".into()),
            max_depth: Some(2),
            direction: TraversalDirection::Downstream,
            confirmed_behaviour: None,
        })
        .unwrap();
    assert_eq!(result.diagnostics.bounded_at_depth, Some(2));
    assert!(result.diagnostics.cycles.contains(&"cycle".to_string()));
}

#[test]
fn verification_coverage_reports_unverified_requirements_and_orphan_evidence() {
    let graph = RequirementGraph {
        baseline: baseline(),
        requirements: vec![requirement("r1", "One"), requirement("r2", "Two")],
        evidence: vec![EvidenceRef {
            id: "t1".into(),
            label: "orphan test".into(),
            uri: None,
            provenance: None,
        }],
        relations: vec![],
    };
    let result = graph
        .view(&ViewRequest {
            kind: ViewKind::VerificationCoverage,
            scope: ViewScope::Authoritative,
            root_id: None,
            max_depth: None,
            direction: TraversalDirection::Downstream,
            confirmed_behaviour: None,
        })
        .unwrap();
    let coverage = result.coverage.unwrap();
    assert!(coverage.unverified.contains(&"r1".to_string()));
    assert!(coverage.unverified.contains(&"r2".to_string()));
    assert_eq!(coverage.orphan_evidence, vec!["t1"]);
}

#[test]
fn validation_rejects_a_relation_with_an_unknown_endpoint() {
    let graph = RequirementGraph {
        baseline: baseline(),
        requirements: vec![requirement("r1", "One")],
        evidence: vec![],
        relations: vec![RequirementRelation {
            id: "dangling".into(),
            source: "r1".into(),
            target: "does-not-exist".into(),
            kind: RequirementRelationKind::Requires,
            authority: RelationAuthority::Asserted,
            provenance: provenance(),
            promotion: None,
        }],
    };
    let error = graph.validate().unwrap_err();
    assert_eq!(error, RequirementError::UnknownEndpoint("dangling".into()));
}

#[test]
fn an_induced_view_lowers_through_to_d2_with_no_renderer_source_leaking_into_the_view_itself() {
    let graph = RequirementGraph {
        baseline: baseline(),
        requirements: vec![requirement("r1", "One"), requirement("r2", "Two")],
        evidence: vec![],
        relations: vec![RequirementRelation {
            id: "e".into(),
            source: "r1".into(),
            target: "r2".into(),
            kind: RequirementRelationKind::Contains,
            authority: RelationAuthority::Asserted,
            provenance: provenance(),
            promotion: None,
        }],
    };
    let view = graph
        .view(&ViewRequest {
            kind: ViewKind::Decomposition,
            scope: ViewScope::Authoritative,
            root_id: None,
            max_depth: None,
            direction: TraversalDirection::Downstream,
            confirmed_behaviour: None,
        })
        .unwrap();
    // The view itself is typed data, not diagram source — this is the
    // contract kr0ki_server::requirements_view (POST /requirements/views)
    // depends on.
    assert_eq!(view.graph.relations.len(), 1);
    // Only requirements_render::to_d2, the one adapter kr0ki-core owns,
    // turns that typed view into D2 — exactly what
    // kr0ki_server::render_requirements_view (POST /render/requirements-view)
    // calls.
    let d2 = to_d2(&view);
    assert_eq!(
        d2,
        "\"r1\": \"One\"\n\"r2\": \"Two\"\n\"r1\" -> \"r2\": Contains (asserted)\n"
    );
}
