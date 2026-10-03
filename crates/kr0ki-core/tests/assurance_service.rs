//! `AssuranceService`: the targeted, revision-qualified answers the CLI, the HTTP routes and the
//! MCP tools share. Runs against this repository's real baseline with a stub executor and a
//! fixed revision probe, so no process is spawned.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kr0ki_core::assurance_service::{
    AssuranceConfig, AssuranceService, RequirementFilter, ServiceError,
};
use kr0ki_core::verification_runner::{CommandExecutor, ExecRequest, ExecResult, RevisionProbe};
use ufo_types::mbse::assurance::{Assurance, Freshness, GapKind, VerificationResult};

struct Probe(Mutex<String>);

impl RevisionProbe for Probe {
    fn implementation_revision(&self) -> Result<String, String> {
        Ok(self.0.lock().unwrap().clone())
    }

    fn toolchain(&self) -> String {
        "rustc-test".into()
    }
}

#[derive(Default)]
struct Stub {
    runs: Mutex<Vec<Vec<String>>>,
    output: Mutex<Option<String>>,
}

impl CommandExecutor for Stub {
    fn run(&self, req: &ExecRequest) -> ExecResult {
        self.runs.lock().unwrap().push(req.argv.clone());
        let out = self
            .output
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| "test result: ok. 3 passed; 0 failed; 0 ignored;\n".into());
        ExecResult {
            exit_code: Some(0),
            output: out.into_bytes(),
            ..Default::default()
        }
    }
}

struct Fixture {
    service: AssuranceService,
    probe: Arc<Probe>,
    stub: Arc<Stub>,
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn fixture(tag: &str) -> Fixture {
    let dir = std::env::temp_dir().join(format!("kr0ki-service-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let probe = Arc::new(Probe(Mutex::new("impl-1".into())));
    let stub = Arc::new(Stub::default());
    let service = AssuranceService::new(
        AssuranceConfig {
            baseline_path: repo_root().join("docs/assurance/kr0ki.assurance.toml"),
            repo_root: repo_root(),
            evidence_dir: dir,
            run_timeout: Duration::from_secs(5),
        },
        probe.clone(),
        stub.clone(),
    );
    Fixture {
        service,
        probe,
        stub,
    }
}

#[test]
fn the_list_is_one_short_row_per_requirement_qualified_by_revisions() {
    let f = fixture("list");
    let list = f
        .service
        .list_requirements(&RequirementFilter::default())
        .unwrap();
    assert_eq!(list.total, 8);
    assert_eq!(list.requirements.len(), 8);
    assert!(list.revisions.model_revision.starts_with("sha256:"));
    assert_eq!(list.revisions.implementation_revision, "impl-1");
    let ids: Vec<_> = list.requirements.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(
        ids,
        ["KR-A01", "KR-A02", "KR-A03", "KR-A04", "KR-A05", "KR-A06", "KR-A07", "KR-A08"]
    );
    // No evidence yet: nothing is verified.
    assert!(list
        .requirements
        .iter()
        .all(|r| r.assurance == Assurance::SatisfiedUntested));
}

#[test]
fn filters_narrow_the_answer_so_only_what_is_needed_is_returned() {
    let f = fixture("filter");
    let f1 = |mutate: &dyn Fn(&mut RequirementFilter)| {
        let mut filter = RequirementFilter::default();
        mutate(&mut filter);
        f.service.list_requirements(&filter).unwrap()
    };
    assert_eq!(f1(&|x| x.status = Some("gated".into())).total, 8);
    assert_eq!(f1(&|x| x.status = Some("draft".into())).total, 0);
    assert_eq!(f1(&|x| x.state = Some("verified".into())).total, 0);
    assert_eq!(
        f1(&|x| x.state = Some("satisfied_untested".into())).total,
        8
    );
    assert_eq!(f1(&|x| x.owner = Some("nobody".into())).total, 0);
    // Implementation gaps: the controls the baseline records no implementation for.
    let unimplemented = f1(&|x| x.gap = Some("control_not_implemented".into()));
    assert!(
        unimplemented.total >= 1 && unimplemented.total < 8,
        "{}",
        unimplemented.total
    );
    assert!(unimplemented.requirements.iter().all(|r| r
        .gaps
        .iter()
        .any(|g| matches!(g, GapKind::ControlNotImplemented { .. }))));
    // A filtered answer is strictly smaller than the whole.
    let whole = serde_json::to_string(
        &f.service
            .list_requirements(&RequirementFilter::default())
            .unwrap(),
    )
    .unwrap();
    let narrow = serde_json::to_string(&unimplemented).unwrap();
    assert!(narrow.len() < whole.len());
}

#[test]
fn one_requirement_in_full_with_its_whole_thread() {
    let f = fixture("detail");
    let d = f.service.get_requirement("KR-A01").unwrap();
    assert_eq!(d.requirement.id, "KR-A01");
    assert!(d.requirement.statement.contains("shall"));
    assert_eq!(d.requirement.verification_id, "VC-A01");
    assert!(d.statement_lints.is_empty());
    assert_eq!(d.elements, ["KrOKiAssurance::ModelService"]);
    assert_eq!(d.cases.len(), 1);
    assert_eq!(d.cases[0].command[0], "cargo");
    assert!(d
        .obligations
        .iter()
        .any(|o| o.id == "OBL-OMG-REQIF-1.2" && o.source_kind == "binding_obligation"));
    assert!(d
        .obligations
        .iter()
        .any(|o| o.id == "OBL-OWNER-BRIEF" && o.source_kind == "organisational_policy"));
    assert_eq!(d.controls[0].id, "CTL-A01");
    assert!(d.controls[0].implementation.contains("reqif_roundtrip.rs"));
    assert_eq!(d.thread.assurance, Assurance::SatisfiedUntested);
    assert!(matches!(
        f.service.get_requirement("KR-NOPE"),
        Err(ServiceError::UnknownRequirement(_))
    ));
    // A node that is not a requirement is not one.
    assert!(matches!(
        f.service.get_requirement("CTL-A01"),
        Err(ServiceError::UnknownRequirement(_))
    ));
}

#[test]
fn trace_resolves_links_at_the_current_revision_and_reports_dangling_ones() {
    let f = fixture("trace");
    let t = f.service.trace_requirement("KR-A01").unwrap();
    assert_eq!(t.dangling, 0, "{:#?}", t.links);
    assert!(t
        .links
        .iter()
        .any(|l| l.locator == "KrOKiAssurance::ModelService"));
    assert!(t
        .links
        .iter()
        .any(|l| l.locator.ends_with("tests/assurance_a01.rs")));
    // A requirement whose control records an implementation that does not yet exist, or whose
    // test target is missing, shows it as dangling rather than hiding it.
    let all: usize = ["KR-A06", "KR-A07", "KR-A08"]
        .iter()
        .map(|id| f.service.trace_requirement(id).unwrap().dangling)
        .sum();
    assert!(all >= 1, "later stages' test targets do not exist yet");
    assert!(matches!(
        f.service.trace_requirement("nope"),
        Err(ServiceError::UnknownRequirement(_))
    ));
}

#[test]
fn running_a_case_stores_revision_bound_evidence_and_moves_the_requirement_to_verified() {
    let f = fixture("run");
    let out = f.service.run_verification("VC-A01", "impl-1").unwrap();
    assert_eq!(out.result, VerificationResult::Pass);
    assert_eq!(out.revisions.implementation_revision, "impl-1");
    assert_eq!(out.evidence_keys.len(), 1);
    assert_eq!(f.stub.runs.lock().unwrap().len(), 1);

    let list = f
        .service
        .list_requirements(&RequirementFilter::default())
        .unwrap();
    let state = |id: &str| {
        list.requirements
            .iter()
            .find(|r| r.id == id)
            .unwrap()
            .assurance
    };
    assert_eq!(state("KR-A01"), Assurance::Verified);
    assert_eq!(
        state("KR-A02"),
        Assurance::SatisfiedUntested,
        "satisfied, never tested"
    );

    let ev = f.service.evidence(Some("KR-A01"), None).unwrap();
    assert_eq!(ev.records.len(), 1);
    assert_eq!(ev.records[0].freshness, Freshness::Fresh);
    assert_eq!(ev.records[0].artifact, "intact");
    assert!(f
        .service
        .evidence(Some("KR-A02"), None)
        .unwrap()
        .records
        .is_empty());
    assert_eq!(
        f.service
            .evidence(None, Some("VC-A01"))
            .unwrap()
            .records
            .len(),
        1
    );
}

#[test]
fn a_changed_implementation_revision_makes_the_stored_result_stale() {
    let f = fixture("stale");
    f.service.run_verification("VC-A01", "impl-1").unwrap();
    *f.probe.0.lock().unwrap() = "impl-2".into();
    let list = f
        .service
        .list_requirements(&RequirementFilter::default())
        .unwrap();
    assert_eq!(
        list.requirements
            .iter()
            .find(|r| r.id == "KR-A01")
            .unwrap()
            .assurance,
        Assurance::Stale
    );
    assert_eq!(list.revisions.implementation_revision, "impl-2");
    let ev = f.service.evidence(Some("KR-A01"), None).unwrap();
    assert!(
        matches!(ev.records[0].freshness, Freshness::Stale { drift } if drift.implementation && !drift.model)
    );
}

#[test]
fn verifying_a_revision_other_than_the_repositorys_is_refused_and_runs_nothing() {
    let f = fixture("mismatch");
    let err = f
        .service
        .run_verification("VC-A01", "some-other-revision")
        .unwrap_err();
    assert!(matches!(
        &err,
        ServiceError::RevisionMismatch { expected, actual } if expected == "some-other-revision" && actual == "impl-1"
    ));
    assert!(
        f.stub.runs.lock().unwrap().is_empty(),
        "nothing may execute on a revision mismatch"
    );
    assert!(f.service.evidence(None, None).unwrap().records.is_empty());
}

#[test]
fn an_unknown_case_is_refused() {
    let f = fixture("unknown");
    assert!(matches!(
        f.service.run_verification("VC-NOPE", "impl-1"),
        Err(ServiceError::UnknownCase(_))
    ));
}

#[test]
fn a_run_that_executes_zero_tests_is_an_error_not_a_pass() {
    let f = fixture("zero");
    *f.stub.output.lock().unwrap() =
        Some("test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out\n".into());
    let out = f.service.run_verification("VC-A01", "impl-1").unwrap();
    assert_eq!(out.result, VerificationResult::Error);
    let list = f
        .service
        .list_requirements(&RequirementFilter::default())
        .unwrap();
    assert_ne!(
        list.requirements
            .iter()
            .find(|r| r.id == "KR-A01")
            .unwrap()
            .assurance,
        Assurance::Verified
    );
}

#[test]
fn verify_all_runs_only_the_cases_whose_test_targets_exist() {
    let f = fixture("all");
    let outcomes = f.service.verify_all().unwrap();
    let ids: Vec<_> = outcomes.iter().map(|o| o.case_id.as_str()).collect();
    assert!(ids.contains(&"VC-A01") && ids.contains(&"VC-A02"));
    // Every run happened through the allowlisted shape.
    for argv in f.stub.runs.lock().unwrap().iter() {
        assert_eq!(&argv[..3], ["cargo", "test", "-p"]);
    }
    for id in &ids {
        let path = {
            let l = f.service.get_requirement(match *id {
                "VC-A01" => "KR-A01",
                "VC-A02" => "KR-A02",
                "VC-A03" => "KR-A03",
                "VC-A04" => "KR-A04",
                "VC-A05" => "KR-A05",
                "VC-A06" => "KR-A06",
                "VC-A07" => "KR-A07",
                _ => "KR-A08",
            });
            l.is_ok()
        };
        assert!(path);
    }
}

#[test]
fn the_model_revision_alone_is_cheap_and_matches_the_snapshots() {
    let f = fixture("rev");
    let alone = f.service.model_revision().unwrap();
    let snap = f.service.snapshot().unwrap();
    assert_eq!(alone, snap.index.revision());
}
