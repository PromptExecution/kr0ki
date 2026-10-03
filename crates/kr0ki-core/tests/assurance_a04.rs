//! KR-A04 acceptance case (VC-A04).
//!
//! "The verification runner shall bind each result to the model, implementation and
//! configuration revisions used."
//!
//! Acceptance: run KR-A01's case; retrieve its evidence. Change a revision and display the
//! previous result as stale.
//!
//! This test runs VC-A01 for real — a child `cargo test -p kr0ki-core --test assurance_a01`
//! through the real process executor and the real git revision probe.

use std::path::{Path, PathBuf};
use std::time::Duration;

use kr0ki_core::assurance_baseline::load_baseline;
use kr0ki_core::assurance_trace::ModelRevisionIndex;
use kr0ki_core::assurance_view::{assurance_view, to_d2, to_table};
use kr0ki_core::evidence_store::{ArtifactCheck, FsEvidenceStore};
use kr0ki_core::verification_runner::{
    cases_from_graph, current_revisions, GitRevisionProbe, ProcessExecutor, RevisionProbe,
    RunReport, RunnerConfig, VerificationRunner,
};
use ufo_types::mbse::assurance::{Assurance, CurrentRevisions, VerificationResult};
use ufo_types::mbse::requirements::RequirementGraph;

const BASELINE_TOML: &str = include_str!("../../../docs/assurance/kr0ki.assurance.toml");
const MODEL_SYSML: &str = include_str!("../../../docs/assurance/kr0ki-assurance.sysml");

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

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

fn store(tag: &str) -> FsEvidenceStore {
    let d = std::env::temp_dir().join(format!("kr0ki-a04-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    FsEvidenceStore::new(d)
}

fn run_vc_a01(store: &FsEvidenceStore, model_revision: &str) -> RunReport {
    let graph = graph();
    let case = cases_from_graph(&graph)
        .into_iter()
        .find(|c| c.id == "VC-A01")
        .expect("the baseline declares VC-A01");
    let probe = GitRevisionProbe::new(repo_root());
    VerificationRunner {
        store,
        executor: &ProcessExecutor,
        probe: &probe,
        config: RunnerConfig {
            repo_root: repo_root(),
            // Short on purpose: a nested-cargo lock problem must fail fast, not hang the suite.
            timeout: Duration::from_secs(600),
            model_revision: model_revision.to_string(),
        },
    }
    .run_case(&case)
    .expect("VC-A01 must run")
}

#[test]
fn run_kr_a01s_case_retrieve_its_evidence_then_change_a_revision_and_see_it_stale() {
    let store = store("main");
    let index = ModelRevisionIndex::from_sysml_text(MODEL_SYSML);

    // 1. Run KR-A01's case for real.
    let report = run_vc_a01(&store, index.revision());
    assert_eq!(
        report.result,
        VerificationResult::Pass,
        "VC-A01 must pass: {}",
        report.detail
    );
    assert!(report.detail.contains("tests passed"), "{}", report.detail);
    assert_eq!(report.records.len(), 1, "VC-A01 verifies exactly KR-A01");

    // 2. Retrieve its evidence, bound to all three revisions.
    let evidence = store.for_requirement("KR-A01").unwrap();
    assert_eq!(evidence, report.records);
    let e = &evidence[0];
    assert_eq!(e.verification_id, "VC-A01");
    assert_eq!(e.model_revision, index.revision());
    assert_eq!(
        e.implementation_revision,
        GitRevisionProbe::new(repo_root())
            .implementation_revision()
            .unwrap(),
        "the implementation revision is the repository's, not a placeholder"
    );
    assert!(e.configuration_digest.starts_with("sha256:"));
    e.validate().unwrap();
    // The artifact is the real log and verifies against its digest.
    let (_, check) = &store.verify().unwrap()[0];
    assert_eq!(check, &ArtifactCheck::Intact);
    let log = std::fs::read_to_string(e.artifact_uri.strip_prefix("file://").unwrap()).unwrap();
    assert!(
        log.contains("verdict: Pass") && log.contains("test result: ok"),
        "{log}"
    );

    // 3. At the revisions it ran at, KR-A01 is verified.
    let graph = graph();
    let probe = GitRevisionProbe::new(repo_root());
    let now = current_revisions(&graph, &probe, index.revision()).unwrap();
    assert_eq!(report.revisions.model_revision, now.model_revision);
    assert_eq!(
        report.revisions.implementation_revision,
        now.implementation_revision
    );
    assert_eq!(
        Some(&report.revisions.configuration_digest),
        now.configuration_digests.get("VC-A01"),
        "the runner and the view derive the same per-case configuration digest"
    );
    let fresh = assurance_view(&graph, &evidence, &now, None).unwrap();
    assert_eq!(fresh.row("KR-A01").unwrap().assurance, Assurance::Verified);

    // 4. Change each revision in turn: the previous result is displayed as stale.
    let changed_model = {
        let later = MODEL_SYSML.replace("part def ToolGateway", "part def RenamedGateway");
        CurrentRevisions {
            model_revision: ModelRevisionIndex::from_sysml_text(&later)
                .revision()
                .into(),
            ..now.clone()
        }
    };
    let changed_implementation = CurrentRevisions {
        implementation_revision: format!("{}-edited", now.implementation_revision),
        ..now.clone()
    };
    let changed_configuration = {
        let mut edited = graph.clone();
        edited
            .requirements
            .iter_mut()
            .find(|n| n.id == "VC-A01")
            .unwrap()
            .text = "a different acceptance".into();
        let case = cases_from_graph(&edited)
            .into_iter()
            .find(|c| c.id == "VC-A01")
            .unwrap();
        now.clone()
            .with_case("VC-A01", case.configuration_digest(&probe.toolchain()))
    };
    for (what, current) in [
        ("model", changed_model),
        ("implementation", changed_implementation),
        ("configuration", changed_configuration),
    ] {
        assert_ne!(current, now, "{what}: the revision must actually differ");
        let view = assurance_view(&graph, &evidence, &current, None).unwrap();
        assert_eq!(
            view.row("KR-A01").unwrap().assurance,
            Assurance::Stale,
            "{what} change must make the result stale"
        );
        let d2 = to_d2(&graph, &view);
        assert!(d2.contains("VC-A01 · pass · STALE"), "{what}: {d2}");
        assert!(!d2.contains("[verified]"), "{what}");
        assert!(to_table(&view).contains("1 stale"), "{what}");
    }
}

#[test]
fn the_configuration_digest_is_stable_across_runs_of_the_same_case() {
    // Bound to what the case checks, not to timestamps or paths.
    let probe = GitRevisionProbe::new(repo_root());
    let case = cases_from_graph(&graph())
        .into_iter()
        .find(|c| c.id == "VC-A01")
        .unwrap();
    assert_eq!(
        case.configuration_digest(&probe.toolchain()),
        case.configuration_digest(&probe.toolchain())
    );
}

#[test]
fn every_declared_case_is_runnable_by_the_runner_or_explicitly_refused() {
    // The baseline's cases all use the one allowlisted shape.
    for case in cases_from_graph(&graph()) {
        assert!(
            kr0ki_core::assurance_trace::cargo_test_target(&case.argv).is_some(),
            "{} declares a command the runner would refuse: {:?}",
            case.id,
            case.argv
        );
        assert!(
            !case.requirements.is_empty(),
            "{} verifies nothing",
            case.id
        );
    }
}
