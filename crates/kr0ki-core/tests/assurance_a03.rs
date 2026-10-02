//! KR-A03 acceptance case (VC-A03).
//!
//! "The requirements view shall distinguish satisfaction assertions from verification
//! results."
//!
//! Acceptance: show one satisfied but untested requirement and one verified requirement.

use kr0ki_core::assurance_baseline::load_baseline;
use kr0ki_core::assurance_view::{assurance_view, state_name, to_d2, to_table, VerificationAxis};
use ufo_types::mbse::assurance::{
    Assurance, CurrentRevisions, EvidenceRecord, RevisionSet, VerificationResult,
};
use ufo_types::mbse::requirements::RequirementGraph;

const BASELINE_TOML: &str = include_str!("../../../docs/assurance/kr0ki.assurance.toml");
const DIGEST: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000001";

fn graph() -> RequirementGraph {
    load_baseline(
        BASELINE_TOML,
        "docs/assurance/kr0ki.assurance.toml",
        "rev-1",
        None,
    )
    .expect("the checked-in baseline must load")
    .graph
}

fn now() -> RevisionSet {
    RevisionSet {
        model_revision: "model-1".into(),
        implementation_revision: "impl-1".into(),
        configuration_digest: DIGEST.into(),
    }
}

/// Every declared case is current at the same configuration digest in these fixtures.
fn current(r: &RevisionSet) -> CurrentRevisions {
    CurrentRevisions::for_cases(r, (1..=8).map(|n| format!("VC-A0{n}")))
}

fn evidence(
    requirement: &str,
    case: &str,
    at: &RevisionSet,
    result: VerificationResult,
) -> EvidenceRecord {
    EvidenceRecord {
        requirement_id: requirement.into(),
        verification_id: case.into(),
        model_revision: at.model_revision.clone(),
        implementation_revision: at.implementation_revision.clone(),
        configuration_digest: at.configuration_digest.clone(),
        result,
        artifact_uri: format!("file:///evidence/{requirement}.json"),
        artifact_digest: DIGEST.into(),
    }
}

/// KR-A01 verified (fresh pass); every other requirement is satisfied by design and untested.
fn the_acceptance_scenario() -> (RequirementGraph, Vec<EvidenceRecord>) {
    (
        graph(),
        vec![evidence(
            "KR-A01",
            "VC-A01",
            &now(),
            VerificationResult::Pass,
        )],
    )
}

#[test]
fn one_requirement_is_satisfied_but_untested_and_one_is_verified() {
    let (graph, ev) = the_acceptance_scenario();
    let view = assurance_view(&graph, &ev, &current(&now()), None).unwrap();

    let verified = view.row("KR-A01").unwrap();
    assert_eq!(verified.assurance, Assurance::Verified);
    assert_eq!(
        verified.verification,
        VerificationAxis::Evidence {
            fresh_pass: 1,
            fresh_fail: 0,
            stale: 0
        }
    );

    let untested = view.row("KR-A02").unwrap();
    assert_eq!(untested.assurance, Assurance::SatisfiedUntested);
    assert_eq!(untested.verification, VerificationAxis::NoEvidence);
    // It *is* satisfied by design — that axis is independent of the verification axis.
    assert_eq!(untested.satisfied_by, ["KrOKiAssurance::ModelService"]);

    let counts = view.counts();
    assert_eq!(counts.get("verified"), Some(&1));
    assert_eq!(counts.get("satisfied_untested"), Some(&7));
}

#[test]
fn the_diagram_draws_the_two_claims_differently() {
    let (graph, ev) = the_acceptance_scenario();
    let view = assurance_view(&graph, &ev, &current(&now()), None).unwrap();
    let d2 = to_d2(&graph, &view);

    // Each state has its own label and fill.
    assert!(d2.contains("KR-A01") && d2.contains("[verified]"), "{d2}");
    assert!(d2.contains("[satisfied · untested]"));
    assert!(d2.contains("#b7ebc6") && d2.contains("#ffe9a8"));
    // Satisfaction is an assertion: dashed. Verification is evidence: its own node and edge.
    assert!(d2.contains("satisfies (assertion) { style.stroke-dash: 4 }"));
    assert!(d2.contains("VC-A01 · pass · fresh"));
    assert!(d2.contains("verification result"));
    // Only the verified requirement has an evidence node attached.
    assert_eq!(d2.matches("verification result").count(), 1);
    // The dashed edge is never the evidence edge.
    let dashed_lines: Vec<_> = d2.lines().filter(|l| l.contains("stroke-dash")).collect();
    assert!(dashed_lines
        .iter()
        .all(|l| l.contains("satisfies (assertion)")));
}

#[test]
fn the_table_has_separate_columns_for_assertion_and_evidence() {
    let (graph, ev) = the_acceptance_scenario();
    let table = to_table(&assurance_view(&graph, &ev, &current(&now()), None).unwrap());
    assert!(table
        .starts_with("requirement | satisfaction (asserted) | verification (evidence) | state"));
    let line = |id: &str| {
        table
            .lines()
            .find(|l| l.starts_with(id))
            .unwrap()
            .to_string()
    };
    assert!(line("KR-A01").contains("1 fresh pass, 0 fresh fail, 0 stale"));
    assert!(line("KR-A01").ends_with("| verified"));
    assert!(line("KR-A02").contains("no evidence"));
    assert!(line("KR-A02").ends_with("| satisfied · untested"));
}

#[test]
fn a_satisfaction_edge_alone_can_never_make_a_requirement_verified() {
    let graph = graph();
    // No evidence at all: nothing is verified, however many satisfaction edges exist.
    let view = assurance_view(&graph, &[], &current(&now()), None).unwrap();
    assert!(view.rows.iter().all(|r| r.assurance != Assurance::Verified));
    assert!(view
        .rows
        .iter()
        .all(|r| r.verification == VerificationAxis::NoEvidence));
    assert!(view.rows.iter().all(|r| !r.satisfied_by.is_empty()));
}

#[test]
fn evidence_for_one_requirement_does_not_verify_another() {
    let graph = graph();
    let ev = [evidence(
        "KR-A01",
        "VC-A01",
        &now(),
        VerificationResult::Pass,
    )];
    let view = assurance_view(&graph, &ev, &current(&now()), None).unwrap();
    for r in view.rows.iter().filter(|r| r.requirement_id != "KR-A01") {
        assert_ne!(r.assurance, Assurance::Verified, "{}", r.requirement_id);
    }
    // Evidence that names a requirement with a case it is not linked to is orphaned, not counted.
    let stray = [evidence(
        "KR-A02",
        "VC-A01",
        &now(),
        VerificationResult::Pass,
    )];
    let view = assurance_view(&graph, &stray, &current(&now()), None).unwrap();
    assert_eq!(
        view.row("KR-A02").unwrap().assurance,
        Assurance::SatisfiedUntested
    );
    assert_eq!(view.report.orphan_evidence.len(), 1);
}

#[test]
fn a_result_for_other_revisions_is_shown_as_stale_not_as_verified_or_untested() {
    let graph = graph();
    let then = now();
    let ev = [evidence(
        "KR-A01",
        "VC-A01",
        &then,
        VerificationResult::Pass,
    )];
    let later = RevisionSet {
        implementation_revision: "impl-2".into(),
        ..then
    };
    let view = assurance_view(&graph, &ev, &current(&later), None).unwrap();
    assert_eq!(view.row("KR-A01").unwrap().assurance, Assurance::Stale);
    assert_eq!(state_name(Assurance::Stale), "stale");
    let d2 = to_d2(&graph, &view);
    assert!(d2.contains("VC-A01 · pass · STALE"));
    assert!(d2.contains("[satisfied · result stale]"));
    assert!(!d2.contains("[verified]"));
}

#[test]
fn a_fresh_failure_is_shown_as_failing() {
    let graph = graph();
    let ev = [evidence(
        "KR-A01",
        "VC-A01",
        &now(),
        VerificationResult::Fail,
    )];
    let view = assurance_view(&graph, &ev, &current(&now()), None).unwrap();
    assert_eq!(view.row("KR-A01").unwrap().assurance, Assurance::Failing);
    assert!(to_d2(&graph, &view).contains("[FAILING]"));
}

#[test]
fn the_view_serialises_for_mcp_and_http() {
    let (graph, ev) = the_acceptance_scenario();
    let json =
        serde_json::to_value(assurance_view(&graph, &ev, &current(&now()), None).unwrap()).unwrap();
    let rows = json["rows"].as_array().unwrap();
    let kr1 = rows
        .iter()
        .find(|r| r["requirement_id"] == "KR-A01")
        .unwrap();
    assert_eq!(kr1["assurance"], "verified");
    assert_eq!(kr1["verification"]["state"], "evidence");
    let kr2 = rows
        .iter()
        .find(|r| r["requirement_id"] == "KR-A02")
        .unwrap();
    assert_eq!(kr2["assurance"], "satisfied_untested");
    assert_eq!(kr2["verification"]["state"], "no_evidence");
}

#[test]
fn the_diagram_is_deterministic() {
    let (graph, ev) = the_acceptance_scenario();
    let a = to_d2(
        &graph,
        &assurance_view(&graph, &ev, &current(&now()), None).unwrap(),
    );
    let b = to_d2(
        &graph,
        &assurance_view(&graph, &ev, &current(&now()), None).unwrap(),
    );
    assert_eq!(a, b);
}

/// Live check: the diagram is valid D2 that a real Kroki-compatible backend renders, and the
/// rendered SVG still carries both claims. Env-gated like `template_render.rs`:
/// `KR0KI_TEST_BACKEND=http://127.0.0.1:8010 cargo test -p kr0ki-core --test assurance_a03 -- --ignored`
#[tokio::test]
#[ignore = "needs KR0KI_TEST_BACKEND pointing at a live Kroki"]
async fn the_assurance_diagram_renders_through_a_live_kroki_backend() {
    use kr0ki_core::cache::{FsCache, OutputKind};
    use kr0ki_core::format::DiagramFormat;
    use kr0ki_core::render::HttpKrokiBackend;
    use kr0ki_core::RenderService;

    let Ok(base) = std::env::var("KR0KI_TEST_BACKEND") else {
        eprintln!("KR0KI_TEST_BACKEND unset — skipping");
        return;
    };
    let (graph, ev) = the_acceptance_scenario();
    let d2 = to_d2(
        &graph,
        &assurance_view(&graph, &ev, &current(&now()), None).unwrap(),
    );

    let dir = std::env::temp_dir().join(format!("kr0ki-assurance-a03-{}", std::process::id()));
    let svc = RenderService::new(HttpKrokiBackend::new(base), FsCache::new(&dir));
    let rendered = svc
        .render(DiagramFormat::D2, OutputKind::Svg, &d2)
        .await
        .unwrap_or_else(|e| panic!("live render failed: {e}\n--- D2 ---\n{d2}"));
    let svg = String::from_utf8_lossy(&rendered.bytes);
    assert!(
        svg.contains("<svg"),
        "expected SVG, got: {}",
        &svg[..svg.len().min(200)]
    );
    assert!(
        svg.contains("verified"),
        "the verified state must survive rendering"
    );
    assert!(
        svg.contains("satisfied"),
        "the satisfied-untested state must survive rendering"
    );
    assert!(
        svg.contains("VC-A01"),
        "the evidence node must survive rendering"
    );
}
