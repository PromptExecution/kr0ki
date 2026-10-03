//! Stage 0: the brief's own requirement baseline loads, is profile-valid, and its thread is
//! analysable. Every later stage (`assurance_a01` …) runs against this same baseline.

use std::collections::BTreeSet;

use kr0ki_core::assurance_baseline::{load_baseline, BaselineError, LoadedBaseline};
use ufo_types::mbse::assurance::{
    analyze, Assurance, CurrentRevisions, GapKind, NodeKind, ProfileRequirement, RevisionSet,
};

pub const BASELINE_TOML: &str = include_str!("../../../docs/assurance/kr0ki.assurance.toml");
pub const MODEL_SYSML: &str = include_str!("../../../docs/assurance/kr0ki-assurance.sysml");

fn load() -> LoadedBaseline {
    load_baseline(
        BASELINE_TOML,
        "docs/assurance/kr0ki.assurance.toml",
        "test-revision",
        Some("model-1"),
    )
    .expect("the checked-in baseline must load")
}

fn revisions() -> CurrentRevisions {
    CurrentRevisions::for_cases(
        &RevisionSet {
            model_revision: "model-1".into(),
            implementation_revision: "impl-1".into(),
            configuration_digest: format!("sha256:{}", "0".repeat(64)),
        },
        ["VC-A01"],
    )
}

const IDS: [&str; 8] = [
    "KR-A01", "KR-A02", "KR-A03", "KR-A04", "KR-A05", "KR-A06", "KR-A07", "KR-A08",
];

#[test]
fn baseline_loads_eight_profile_valid_requirements_with_a_clean_statement_lint() {
    let loaded = load();
    assert_eq!(
        loaded.statement_lints,
        vec![],
        "statement lint must be clean"
    );
    let reqs: Vec<_> = loaded
        .graph
        .requirements
        .iter()
        .filter(|n| n.attributes.get("node_kind").map(String::as_str) == Some("requirement"))
        .collect();
    assert_eq!(reqs.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), IDS);
    for r in reqs {
        ProfileRequirement::from_node(r).unwrap_or_else(|e| panic!("{}: {e:?}", r.id));
    }
    assert_eq!(loaded.model.as_deref(), Some("kr0ki-assurance.sysml"));
}

#[test]
fn every_requirement_has_a_source_an_element_a_control_and_a_case() {
    let loaded = load();
    let report = analyze(&loaded.graph, &[], &revisions(), None).expect("analyzable");
    assert_eq!(report.requirements.len(), 8);
    for t in &report.requirements {
        assert!(!t.sources.is_empty(), "{} has no source", t.requirement_id);
        assert_eq!(t.satisfied_by.len(), 1, "{}", t.requirement_id);
        assert_eq!(t.enforced_by.len(), 1, "{}", t.requirement_id);
        assert_eq!(t.verification_cases.len(), 1, "{}", t.requirement_id);
    }
}

#[test]
fn the_three_source_kinds_stay_distinguishable() {
    let loaded = load();
    let kinds: BTreeSet<&str> = loaded
        .graph
        .requirements
        .iter()
        .filter(|n| n.attributes.get("node_kind").map(String::as_str) == Some("source_obligation"))
        .filter_map(|n| n.attributes.get("source_kind").map(String::as_str))
        .collect();
    assert_eq!(
        kinds,
        BTreeSet::from(["binding_obligation", "guidance", "organisational_policy"])
    );
}

#[test]
fn initial_gaps_are_exactly_the_unimplemented_controls_and_nothing_is_verified() {
    let loaded = load();
    let report = analyze(&loaded.graph, &[], &revisions(), None).expect("analyzable");

    // Every requirement is satisfied by design and untested: no evidence exists yet.
    for t in &report.requirements {
        assert_eq!(
            t.assurance,
            Assurance::SatisfiedUntested,
            "{}",
            t.requirement_id
        );
    }
    // The only structural gaps are controls the file records no implementation for.
    let unimplemented: BTreeSet<String> = loaded
        .graph
        .requirements
        .iter()
        .filter(|n| n.attributes.get("node_kind").map(String::as_str) == Some("control"))
        .filter(|n| {
            n.attributes
                .get("implementation")
                .is_none_or(|v| v.trim().is_empty())
        })
        .map(|n| n.id.clone())
        .collect();
    let reported: BTreeSet<String> = report
        .gaps
        .iter()
        .filter_map(|g| match &g.kind {
            GapKind::ControlNotImplemented { control_id } => Some(control_id.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(reported, unimplemented);
    for g in &report.gaps {
        assert!(
            matches!(
                g.kind,
                GapKind::ControlNotImplemented { .. } | GapKind::SatisfiedUntested
            ),
            "unexpected structural gap {g:?}"
        );
    }
    assert!(report.orphan_evidence.is_empty());
}

#[test]
fn control_nodes_record_their_enforcing_component() {
    let loaded = load();
    let component = |id: &str| {
        loaded
            .graph
            .requirements
            .iter()
            .find(|n| n.id == id)
            .and_then(|n| n.attributes.get("component").cloned())
    };
    // Sandbox enforcement lives in the deployment component, not in kr0ki.
    assert_eq!(component("CTL-A07").as_deref(), Some("deployment"));
    assert_eq!(component("CTL-A06").as_deref(), Some("kr0ki"));
}

#[test]
fn the_sysml_model_parses() {
    sysml_v2_parser::parse(MODEL_SYSML).expect("kr0ki-assurance.sysml must be valid SysML v2");
}

#[test]
fn node_kinds_cover_all_five_thread_stages() {
    let loaded = load();
    let kinds: BTreeSet<NodeKind> = loaded
        .graph
        .requirements
        .iter()
        .filter_map(|n| {
            n.attributes
                .get("node_kind")
                .and_then(|k| NodeKind::parse(k))
        })
        .collect();
    assert_eq!(kinds.len(), 5);
}

#[test]
fn a_typo_in_the_baseline_is_an_error_not_a_silently_dropped_field() {
    let typo = BASELINE_TOML.replacen("owner = ", "ownr = ", 1);
    assert!(matches!(
        load_baseline(&typo, "x", "r", None),
        Err(BaselineError::Toml(_))
    ));
}

#[test]
fn a_reference_to_an_undeclared_node_is_rejected() {
    let broken = BASELINE_TOML.replacen("control = \"CTL-A01\"", "control = \"CTL-NOPE\"", 1);
    match load_baseline(&broken, "x", "r", None) {
        Err(BaselineError::UnknownReference {
            owner,
            field,
            value,
        }) => {
            assert_eq!(
                (owner.as_str(), field, value.as_str()),
                ("KR-A01", "control", "CTL-NOPE")
            );
        }
        other => panic!("expected UnknownReference, got {other:?}"),
    }
}

#[test]
fn an_invalid_status_is_reported_by_the_profile() {
    let broken = BASELINE_TOML.replacen("status = \"gated\"", "status = \"done\"", 1);
    assert!(matches!(
        load_baseline(&broken, "x", "r", None),
        Err(BaselineError::Profile(_))
    ));
}

#[test]
fn the_baseline_digest_changes_when_the_file_changes() {
    let a = load();
    let edited = BASELINE_TOML.replacen("kr0ki-maintainers", "someone-else", 1);
    let b = load_baseline(
        &edited,
        "docs/assurance/kr0ki.assurance.toml",
        "test-revision",
        Some("model-1"),
    )
    .unwrap();
    assert_ne!(
        a.graph.baseline.import_artifact_sha256,
        b.graph.baseline.import_artifact_sha256
    );
}
