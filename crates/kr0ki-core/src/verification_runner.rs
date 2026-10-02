//! KR-A04: run a declared verification case and **bind the result to the model,
//! implementation and configuration revisions it ran against**.
//!
//! A result is only evidence for the revisions it ran at. The runner therefore never produces
//! a bare pass/fail: every [`EvidenceRecord`] carries
//!
//! - `model_revision` — supplied by the caller (a model-server commit id, or the content
//!   address of a model file);
//! - `implementation_revision` — the repository's `HEAD`, plus a digest of any uncommitted
//!   change, so a dirty tree is a *different* revision from the clean commit;
//! - `configuration_digest` — a digest of what the case checks: its command, its acceptance
//!   text, the statement and acceptance of every requirement it verifies, and the toolchain.
//!
//! `ufo_types::mbse::assurance` then decides freshness by comparing those three with the
//! current ones; this module does not.
//!
//! # What may run
//!
//! A case's command is an argv, never a shell string, and only
//! `cargo test -p <package> --test <name>` is accepted (`assurance_trace::cargo_test_target`).
//! The child gets a cleared environment and a time limit. This is a guard against a malformed
//! baseline, **not a sandbox**: the test code itself is the repository's, and confining what it
//! may write is the execution sandbox's job (KR-A07), in its deployment component.
//!
//! # What counts as a pass
//!
//! Exit status 0 **and** at least one test passed **and** none failed. A run that executed zero
//! tests (a typo in a test filter, a renamed target) exits 0 and proves nothing, so it is an
//! `Error`, never a `Pass`. A build failure or timeout is an `Error`; tests that ran and failed
//! are a `Fail`.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use ufo_types::mbse::assurance::{
    CurrentRevisions, EvidenceRecord, NodeKind, RevisionSet, VerificationResult, NODE_KIND_ATTR,
};
use ufo_types::mbse::requirements::{RelationAuthority, RequirementGraph, RequirementRelationKind};

use crate::assurance_baseline::sha256_hex;
use crate::assurance_trace::cargo_test_target;
use crate::evidence_store::{EvidenceStoreError, FsEvidenceStore};

#[derive(Debug, thiserror::Error)]
pub enum RunnerError {
    #[error("case `{0}` is not in the baseline")]
    UnknownCase(String),
    #[error("case `{case}`: command {argv:?} is not `cargo test -p <package> --test <name>`")]
    NotAllowlisted { case: String, argv: Vec<String> },
    #[error("case `{0}` verifies no requirement, so its result would be evidence for nothing")]
    NoRequirement(String),
    #[error("revision probe: {0}")]
    Probe(String),
    #[error(transparent)]
    Store(#[from] EvidenceStoreError),
}

/// A verification case read from the baseline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationCase {
    pub id: String,
    pub argv: Vec<String>,
    pub acceptance: String,
    /// `(requirement id, statement, acceptance)` of each requirement it verifies, sorted by id.
    pub requirements: Vec<(String, String, String)>,
}

/// The cases of a graph, sorted by id.
pub fn cases_from_graph(graph: &RequirementGraph) -> Vec<VerificationCase> {
    let mut out: Vec<VerificationCase> = graph
        .requirements
        .iter()
        .filter(|n| {
            n.attributes.get(NODE_KIND_ATTR).map(String::as_str)
                == Some(NodeKind::VerificationCase.as_str())
        })
        .map(|n| {
            let mut requirements: Vec<(String, String, String)> = graph
                .relations
                .iter()
                .filter(|r| {
                    matches!(r.authority, RelationAuthority::Asserted)
                        && r.kind == RequirementRelationKind::Verifies
                        && r.source == n.id
                })
                .filter_map(|r| graph.requirements.iter().find(|q| q.id == r.target))
                .map(|q| {
                    (
                        q.id.clone(),
                        q.text.clone(),
                        q.attributes.get("acceptance").cloned().unwrap_or_default(),
                    )
                })
                .collect();
            requirements.sort();
            requirements.dedup();
            VerificationCase {
                id: n.id.clone(),
                argv: n
                    .attributes
                    .get("command")
                    .and_then(|c| serde_json::from_str(c).ok())
                    .unwrap_or_default(),
                acceptance: n.text.clone(),
                requirements,
            }
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

impl VerificationCase {
    /// Digest of what this case checks (see the module doc). Stable under unrelated edits to
    /// the baseline; changes when the command, its acceptance, or any verified requirement's
    /// statement or acceptance changes.
    pub fn configuration_digest(&self, toolchain: &str) -> String {
        let spec = serde_json::json!({
            "acceptance": self.acceptance,
            "argv": self.argv,
            "case": self.id,
            "requirements": self.requirements,
            "toolchain": toolchain,
        });
        // `serde_json::Value` objects are key-sorted (BTreeMap), so this is canonical.
        format!("sha256:{}", sha256_hex(spec.to_string().as_bytes()))
    }
}

/// What is current now: the supplied model revision, the probe's implementation revision, and
/// each declared case's own configuration digest.
pub fn current_revisions(
    graph: &RequirementGraph,
    probe: &dyn RevisionProbe,
    model_revision: &str,
) -> Result<CurrentRevisions, String> {
    let toolchain = probe.toolchain();
    let mut current = CurrentRevisions::new(model_revision, probe.implementation_revision()?);
    for case in cases_from_graph(graph) {
        let digest = case.configuration_digest(&toolchain);
        current = current.with_case(case.id, digest);
    }
    Ok(current)
}

// ── Execution ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ExecRequest {
    pub argv: Vec<String>,
    pub cwd: PathBuf,
    pub timeout: Duration,
    pub env: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default)]
pub struct ExecResult {
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    /// Combined stdout and stderr, capped to the last [`OUTPUT_CAP`] bytes.
    pub output: Vec<u8>,
    pub spawn_error: Option<String>,
}

pub const OUTPUT_CAP: u64 = 4 * 1024 * 1024;

pub trait CommandExecutor: Send + Sync {
    fn run(&self, req: &ExecRequest) -> ExecResult;
}

/// Runs the argv as a child process: no shell, cleared environment (only `req.env`), its own
/// process group so a timeout kills the whole `cargo` tree, output captured to a file so a
/// chatty child cannot block on a full pipe.
#[derive(Debug, Clone, Default)]
pub struct ProcessExecutor;

impl CommandExecutor for ProcessExecutor {
    fn run(&self, req: &ExecRequest) -> ExecResult {
        use std::os::unix::process::CommandExt;

        let Some((program, args)) = req.argv.split_first() else {
            return ExecResult {
                spawn_error: Some("empty argv".into()),
                ..Default::default()
            };
        };
        let log_path = std::env::temp_dir().join(format!(
            "kr0ki-run-{}-{}.log",
            std::process::id(),
            sha256_hex(format!("{:?}{:?}", req.argv, Instant::now()).as_bytes())
        ));
        let log = match fs::File::create(&log_path) {
            Ok(f) => f,
            Err(e) => {
                return ExecResult {
                    spawn_error: Some(format!("cannot create log: {e}")),
                    ..Default::default()
                };
            }
        };
        let log_err = match log.try_clone() {
            Ok(f) => f,
            Err(e) => {
                return ExecResult {
                    spawn_error: Some(format!("cannot clone log: {e}")),
                    ..Default::default()
                };
            }
        };
        let mut child = match Command::new(program)
            .args(args)
            .current_dir(&req.cwd)
            .env_clear()
            .envs(req.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(log_err))
            .process_group(0)
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let _ = fs::remove_file(&log_path);
                return ExecResult {
                    spawn_error: Some(format!("cannot spawn `{program}`: {e}")),
                    ..Default::default()
                };
            }
        };

        let deadline = Instant::now() + req.timeout;
        let (exit_code, timed_out) = loop {
            match child.try_wait() {
                Ok(Some(status)) => break (status.code(), false),
                Ok(None) if Instant::now() >= deadline => {
                    // Kill the whole group, then reap.
                    let _ = Command::new("kill")
                        .args(["-KILL", &format!("-{}", child.id())])
                        .status();
                    let _ = child.kill();
                    let _ = child.wait();
                    break (None, true);
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(e) => {
                    let _ = fs::remove_file(&log_path);
                    return ExecResult {
                        spawn_error: Some(format!("wait failed: {e}")),
                        ..Default::default()
                    };
                }
            }
        };

        let output = read_tail(&log_path, OUTPUT_CAP);
        let _ = fs::remove_file(&log_path);
        ExecResult {
            exit_code,
            timed_out,
            output,
            spawn_error: None,
        }
    }
}

fn read_tail(path: &Path, cap: u64) -> Vec<u8> {
    let Ok(mut f) = fs::File::open(path) else {
        return Vec::new();
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    if len > cap {
        let _ = f.seek(SeekFrom::Start(len - cap));
    }
    let mut buf = Vec::new();
    let _ = f.take(cap).read_to_end(&mut buf);
    buf
}

/// The environment a case may see: enough to find and run the toolchain, nothing else.
pub fn minimal_env() -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = [
        "PATH",
        "HOME",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "CARGO_TARGET_DIR",
        "TMPDIR",
    ]
    .iter()
    .filter_map(|k| std::env::var(k).ok().map(|v| ((*k).to_string(), v)))
    .collect();
    env.push(("CARGO_TERM_COLOR".into(), "never".into()));
    env.push(("RUST_BACKTRACE".into(), "0".into()));
    env
}

// ── Revisions ───────────────────────────────────────────────────────────────────

/// Where the implementation revision and toolchain come from.
pub trait RevisionProbe: Send + Sync {
    fn implementation_revision(&self) -> Result<String, String>;
    fn toolchain(&self) -> String;
}

/// `git`-backed: `HEAD`, plus a digest of uncommitted changes (tracked diff and the status of
/// untracked files), so editing the tree changes the revision.
#[derive(Debug, Clone)]
pub struct GitRevisionProbe {
    repo_root: PathBuf,
}

impl GitRevisionProbe {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
        }
    }

    fn git(&self, args: &[&str]) -> Result<Vec<u8>, String> {
        let out = Command::new("git")
            .args(args)
            .current_dir(&self.repo_root)
            .output()
            .map_err(|e| format!("cannot run git: {e}"))?;
        if out.status.success() {
            Ok(out.stdout)
        } else {
            Err(format!(
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        }
    }
}

impl RevisionProbe for GitRevisionProbe {
    fn implementation_revision(&self) -> Result<String, String> {
        let head = String::from_utf8_lossy(&self.git(&["rev-parse", "HEAD"])?)
            .trim()
            .to_string();
        let status = self.git(&["status", "--porcelain", "--untracked-files=all"])?;
        if status.is_empty() {
            return Ok(head);
        }
        let mut material = self.git(&["diff", "HEAD"])?;
        material.extend_from_slice(&status);
        // Untracked files' *content* matters too, not just their names.
        for line in String::from_utf8_lossy(&status)
            .lines()
            .filter(|l| l.starts_with("??"))
        {
            let path = line[3..].trim();
            if let Ok(bytes) = fs::read(self.repo_root.join(path)) {
                material.extend_from_slice(path.as_bytes());
                material.extend_from_slice(&bytes);
            }
        }
        Ok(format!("{head}+dirty.{}", &sha256_hex(&material)[..12]))
    }

    fn toolchain(&self) -> String {
        Command::new("rustc")
            .arg("--version")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "rustc-unknown".into())
    }
}

// ── Classification ──────────────────────────────────────────────────────────────

/// Sum the `N passed; M failed` of every `test result:` line in cargo's output.
pub fn parse_test_summary(output: &str) -> (usize, usize) {
    // In `test result: ok. 3 passed; 0 failed; ...` the count is the token before the word.
    fn count_before(line: &str, word: &str) -> usize {
        line.split(';')
            .find_map(|segment| {
                let tokens: Vec<&str> = segment.split_whitespace().collect();
                let at = tokens.iter().position(|t| *t == word)?;
                tokens.get(at.checked_sub(1)?)?.parse().ok()
            })
            .unwrap_or(0)
    }
    output
        .lines()
        .filter(|l| l.trim_start().starts_with("test result:"))
        .fold((0, 0), |(p, f), line| {
            (
                p + count_before(line, "passed"),
                f + count_before(line, "failed"),
            )
        })
}

/// Decide the verdict for one execution.
pub fn classify(result: &ExecResult) -> (VerificationResult, String) {
    if let Some(e) = &result.spawn_error {
        return (VerificationResult::Error, format!("could not run: {e}"));
    }
    if result.timed_out {
        return (VerificationResult::Error, "timed out".into());
    }
    let text = String::from_utf8_lossy(&result.output);
    let (passed, failed) = parse_test_summary(&text);
    match result.exit_code {
        Some(0) if failed == 0 && passed >= 1 => {
            (VerificationResult::Pass, format!("{passed} tests passed"))
        }
        Some(0) if passed == 0 && failed == 0 => (
            VerificationResult::Error,
            "no tests ran: a run that exercised nothing is not evidence".into(),
        ),
        _ if failed > 0 => (
            VerificationResult::Fail,
            format!("{failed} tests failed, {passed} passed"),
        ),
        Some(code) => (
            VerificationResult::Error,
            format!("exit status {code} with no test verdict (build failure?)"),
        ),
        None => (VerificationResult::Error, "terminated by a signal".into()),
    }
}

// ── The runner ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RunnerConfig {
    pub repo_root: PathBuf,
    pub timeout: Duration,
    /// The model revision the run is bound to (supplied by the caller).
    pub model_revision: String,
}

/// What one run produced.
#[derive(Debug, Clone)]
pub struct RunReport {
    pub case_id: String,
    pub result: VerificationResult,
    pub detail: String,
    pub revisions: RevisionSet,
    /// One record per requirement the case verifies, all already stored.
    pub records: Vec<EvidenceRecord>,
}

pub struct VerificationRunner<'a> {
    pub store: &'a FsEvidenceStore,
    pub executor: &'a dyn CommandExecutor,
    pub probe: &'a dyn RevisionProbe,
    pub config: RunnerConfig,
}

impl VerificationRunner<'_> {
    /// Run `case`, store its log as an artifact and one evidence record per verified
    /// requirement, and return the report. The revisions are read *before* the run, so a
    /// change made while it runs shows up as stale next time, not as a mislabelled pass.
    pub fn run_case(&self, case: &VerificationCase) -> Result<RunReport, RunnerError> {
        if cargo_test_target(&case.argv).is_none() {
            return Err(RunnerError::NotAllowlisted {
                case: case.id.clone(),
                argv: case.argv.clone(),
            });
        }
        if case.requirements.is_empty() {
            return Err(RunnerError::NoRequirement(case.id.clone()));
        }
        let revisions = RevisionSet {
            model_revision: self.config.model_revision.clone(),
            implementation_revision: self
                .probe
                .implementation_revision()
                .map_err(RunnerError::Probe)?,
            configuration_digest: case.configuration_digest(&self.probe.toolchain()),
        };

        let result = self.executor.run(&ExecRequest {
            argv: case.argv.clone(),
            cwd: self.config.repo_root.clone(),
            timeout: self.config.timeout,
            env: minimal_env(),
        });
        let (verdict, detail) = classify(&result);

        let mut log = format!(
            "case: {}\nargv: {:?}\nmodel_revision: {}\nimplementation_revision: {}\nconfiguration_digest: {}\nverdict: {:?} ({})\nexit_code: {:?}\ntimed_out: {}\n--- output ---\n",
            case.id, case.argv, revisions.model_revision, revisions.implementation_revision,
            revisions.configuration_digest, verdict, detail, result.exit_code, result.timed_out
        )
        .into_bytes();
        log.extend_from_slice(&result.output);
        let (artifact_uri, artifact_digest) = self.store.put_artifact(&log)?;

        let mut records = Vec::new();
        for (requirement_id, _, _) in &case.requirements {
            let record = EvidenceRecord {
                requirement_id: requirement_id.clone(),
                verification_id: case.id.clone(),
                model_revision: revisions.model_revision.clone(),
                implementation_revision: revisions.implementation_revision.clone(),
                configuration_digest: revisions.configuration_digest.clone(),
                result: verdict,
                artifact_uri: artifact_uri.clone(),
                artifact_digest: artifact_digest.clone(),
            };
            self.store.put(&record)?;
            records.push(record);
        }
        Ok(RunReport {
            case_id: case.id.clone(),
            result: verdict,
            detail,
            revisions,
            records,
        })
    }
}

/// Group evidence by requirement id (for display).
pub fn by_requirement(records: &[EvidenceRecord]) -> BTreeMap<&str, Vec<&EvidenceRecord>> {
    let mut out: BTreeMap<&str, Vec<&EvidenceRecord>> = BTreeMap::new();
    for r in records {
        out.entry(r.requirement_id.as_str()).or_default().push(r);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn ok_output(n: usize) -> String {
        format!("running {n} tests\ntest result: ok. {n} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n")
    }

    #[test]
    fn summary_sums_every_test_result_line() {
        let out = format!(
            "{}{}",
            ok_output(3),
            "test result: FAILED. 2 passed; 1 failed; 0 ignored;\n"
        );
        assert_eq!(parse_test_summary(&out), (5, 1));
        assert_eq!(parse_test_summary("nothing here"), (0, 0));
        assert_eq!(
            parse_test_summary("test result: ok. 0 passed; 0 failed; 9 filtered out"),
            (0, 0)
        );
    }

    fn exec(code: Option<i32>, output: &str) -> ExecResult {
        ExecResult {
            exit_code: code,
            output: output.as_bytes().to_vec(),
            ..Default::default()
        }
    }

    #[test]
    fn classification_is_strict_about_what_a_pass_is() {
        assert_eq!(
            classify(&exec(Some(0), &ok_output(4))).0,
            VerificationResult::Pass
        );
        // Exit 0 but nothing ran: NOT a pass.
        let none = "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out;\n";
        assert_eq!(classify(&exec(Some(0), none)).0, VerificationResult::Error);
        assert_eq!(classify(&exec(Some(0), "")).0, VerificationResult::Error);
        // Tests ran and failed.
        let failed = "test result: FAILED. 3 passed; 2 failed; 0 ignored;\n";
        assert_eq!(
            classify(&exec(Some(101), failed)).0,
            VerificationResult::Fail
        );
        // Exit 0 but a failure line anyway: still not a pass.
        assert_eq!(classify(&exec(Some(0), failed)).0, VerificationResult::Fail);
        // Build failure: no verdict at all.
        assert_eq!(
            classify(&exec(Some(101), "error[E0432]: unresolved import")).0,
            VerificationResult::Error
        );
        assert_eq!(classify(&exec(None, "")).0, VerificationResult::Error);
        assert_eq!(
            classify(&ExecResult {
                timed_out: true,
                ..Default::default()
            })
            .0,
            VerificationResult::Error
        );
        assert_eq!(
            classify(&ExecResult {
                spawn_error: Some("nope".into()),
                ..Default::default()
            })
            .0,
            VerificationResult::Error
        );
    }

    struct Fake {
        result: ExecResult,
        seen: Mutex<Vec<ExecRequest>>,
    }

    impl CommandExecutor for Fake {
        fn run(&self, req: &ExecRequest) -> ExecResult {
            self.seen.lock().unwrap().push(req.clone());
            self.result.clone()
        }
    }

    struct FixedProbe(&'static str);

    impl RevisionProbe for FixedProbe {
        fn implementation_revision(&self) -> Result<String, String> {
            Ok(self.0.into())
        }

        fn toolchain(&self) -> String {
            "rustc-test".into()
        }
    }

    fn case(argv: &[&str]) -> VerificationCase {
        VerificationCase {
            id: "VC-T".into(),
            argv: argv.iter().map(|s| s.to_string()).collect(),
            acceptance: "it works".into(),
            requirements: vec![(
                "KR-T".into(),
                "The thing shall work.".into(),
                "it works".into(),
            )],
        }
    }

    const OK_ARGV: [&str; 6] = ["cargo", "test", "-p", "kr0ki-core", "--test", "x"];

    fn store(tag: &str) -> FsEvidenceStore {
        let d = std::env::temp_dir().join(format!("kr0ki-runner-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        FsEvidenceStore::new(d)
    }

    fn run(
        fake: &Fake,
        store: &FsEvidenceStore,
        c: &VerificationCase,
        impl_rev: &'static str,
    ) -> Result<RunReport, RunnerError> {
        VerificationRunner {
            store,
            executor: fake,
            probe: &FixedProbe(impl_rev),
            config: RunnerConfig {
                repo_root: PathBuf::from("."),
                timeout: Duration::from_secs(5),
                model_revision: "model-1".into(),
            },
        }
        .run_case(c)
    }

    #[test]
    fn a_pass_is_bound_to_all_three_revisions_and_stored_with_its_log() {
        let store = store("bind");
        let fake = Fake {
            result: exec(Some(0), &ok_output(2)),
            seen: Mutex::default(),
        };
        let c = case(&OK_ARGV);
        let report = run(&fake, &store, &c, "impl-1").unwrap();
        assert_eq!(report.result, VerificationResult::Pass);
        let r = &report.records[0];
        assert_eq!(r.model_revision, "model-1");
        assert_eq!(r.implementation_revision, "impl-1");
        assert_eq!(r.configuration_digest, c.configuration_digest("rustc-test"));
        assert_eq!(store.list().unwrap(), report.records);
        // The stored artifact is the log and verifies.
        let checks = store.verify().unwrap();
        assert_eq!(checks[0].1, crate::evidence_store::ArtifactCheck::Intact);
        let log = fs::read_to_string(r.artifact_uri.strip_prefix("file://").unwrap()).unwrap();
        assert!(log.contains("verdict: Pass") && log.contains("2 passed"));
    }

    #[test]
    fn the_child_gets_a_cleared_environment_and_no_shell() {
        let store = store("env");
        let fake = Fake {
            result: exec(Some(0), &ok_output(1)),
            seen: Mutex::default(),
        };
        run(&fake, &store, &case(&OK_ARGV), "i").unwrap();
        let req = fake.seen.lock().unwrap()[0].clone();
        assert_eq!(
            req.argv[0], "cargo",
            "argv is passed through, not wrapped in `sh -c`"
        );
        let keys: Vec<_> = req.env.iter().map(|(k, _)| k.as_str()).collect();
        assert!(
            keys.iter().all(|k| [
                "PATH",
                "HOME",
                "CARGO_HOME",
                "RUSTUP_HOME",
                "CARGO_TARGET_DIR",
                "TMPDIR",
                "CARGO_TERM_COLOR",
                "RUST_BACKTRACE"
            ]
            .contains(k)),
            "{keys:?}"
        );
    }

    #[test]
    fn a_failure_and_an_error_are_recorded_as_such_never_as_a_pass() {
        let store = store("neg");
        let c = case(&OK_ARGV);
        let fail = Fake {
            result: exec(Some(101), "test result: FAILED. 1 passed; 1 failed;\n"),
            seen: Mutex::default(),
        };
        assert_eq!(
            run(&fail, &store, &c, "i1").unwrap().result,
            VerificationResult::Fail
        );
        let none = Fake {
            result: exec(
                Some(0),
                "test result: ok. 0 passed; 0 failed; 5 filtered out;\n",
            ),
            seen: Mutex::default(),
        };
        let r = run(&none, &store, &c, "i2").unwrap();
        assert_eq!(r.result, VerificationResult::Error);
        assert!(r.detail.contains("no tests ran"));
        assert!(store
            .list()
            .unwrap()
            .iter()
            .all(|e| e.result != VerificationResult::Pass));
    }

    #[test]
    fn a_command_outside_the_allowlist_is_refused_before_anything_runs() {
        let store = store("deny");
        let fake = Fake {
            result: exec(Some(0), &ok_output(1)),
            seen: Mutex::default(),
        };
        for argv in [
            vec!["sh", "-c", "echo pwned"],
            vec!["cargo", "test", "--workspace"],
            vec!["cargo", "test", "-p", "a", "--test", "b; id"],
            vec![],
        ] {
            assert!(
                matches!(
                    run(&fake, &store, &case(&argv), "i"),
                    Err(RunnerError::NotAllowlisted { .. })
                ),
                "{argv:?}"
            );
        }
        assert!(
            fake.seen.lock().unwrap().is_empty(),
            "nothing may execute for a refused case"
        );
        assert_eq!(store.list().unwrap(), vec![]);
    }

    #[test]
    fn a_case_that_verifies_nothing_is_refused() {
        let store = store("none");
        let fake = Fake {
            result: exec(Some(0), &ok_output(1)),
            seen: Mutex::default(),
        };
        let mut c = case(&OK_ARGV);
        c.requirements.clear();
        assert!(matches!(
            run(&fake, &store, &c, "i"),
            Err(RunnerError::NoRequirement(_))
        ));
    }

    #[test]
    fn the_configuration_digest_tracks_what_the_case_checks() {
        let base = case(&OK_ARGV);
        let d = base.configuration_digest("rustc-1");
        assert_eq!(d, base.configuration_digest("rustc-1"));
        assert_ne!(d, base.configuration_digest("rustc-2"), "toolchain");
        let mut statement = base.clone();
        statement.requirements[0].1 = "The thing shall work differently.".into();
        assert_ne!(
            d,
            statement.configuration_digest("rustc-1"),
            "requirement statement"
        );
        let mut acceptance = base.clone();
        acceptance.acceptance = "something else".into();
        assert_ne!(
            d,
            acceptance.configuration_digest("rustc-1"),
            "case acceptance"
        );
        let mut argv = base.clone();
        argv.argv[5] = "y".into();
        assert_ne!(d, argv.configuration_digest("rustc-1"), "command");
        assert!(d.starts_with("sha256:") && d.len() == 7 + 64);
    }

    fn git(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        assert!(ok, "git {args:?} failed");
    }

    #[test]
    fn the_git_probe_changes_with_any_edit_and_returns_when_it_is_undone() {
        if Command::new("git").arg("--version").output().is_err() {
            eprintln!("git not available - skipping");
            return;
        }
        let dir = std::env::temp_dir().join(format!("kr0ki-probe-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q"]);
        fs::write(dir.join("a.txt"), "one").unwrap();
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "-q", "-m", "init"]);

        let probe = GitRevisionProbe::new(&dir);
        let clean = probe.implementation_revision().unwrap();
        assert_eq!(clean.len(), 40, "a clean tree is exactly HEAD: {clean}");

        fs::write(dir.join("a.txt"), "two").unwrap();
        let edited = probe.implementation_revision().unwrap();
        assert!(
            edited.starts_with(&clean) && edited.contains("+dirty."),
            "{edited}"
        );
        assert_ne!(edited, clean);

        fs::write(dir.join("a.txt"), "three").unwrap();
        assert_ne!(
            probe.implementation_revision().unwrap(),
            edited,
            "a different edit is a different revision"
        );

        fs::write(dir.join("a.txt"), "one").unwrap();
        assert_eq!(
            probe.implementation_revision().unwrap(),
            clean,
            "undoing the edit restores the revision"
        );

        // An untracked file counts, and so does its content.
        fs::write(dir.join("new.txt"), "x").unwrap();
        let with_new = probe.implementation_revision().unwrap();
        assert_ne!(with_new, clean);
        fs::write(dir.join("new.txt"), "y").unwrap();
        assert_ne!(probe.implementation_revision().unwrap(), with_new);

        assert!(
            GitRevisionProbe::new(std::env::temp_dir().join("definitely-not-a-repo-xyz"))
                .implementation_revision()
                .is_err()
        );
    }

    #[test]
    fn the_process_executor_runs_without_a_shell_captures_output_and_enforces_the_timeout() {
        let exec = ProcessExecutor;
        let req = |argv: &[&str], secs: u64| ExecRequest {
            argv: argv.iter().map(|s| s.to_string()).collect(),
            cwd: std::env::temp_dir(),
            timeout: Duration::from_secs(secs),
            env: minimal_env(),
        };
        let r = exec.run(&req(&["echo", "hello; id"], 5));
        assert_eq!(r.exit_code, Some(0));
        assert_eq!(
            String::from_utf8_lossy(&r.output).trim(),
            "hello; id",
            "no shell interpretation"
        );

        let r = exec.run(&req(&["sleep", "30"], 1));
        assert!(r.timed_out && r.exit_code.is_none());

        let r = exec.run(&req(&["definitely-not-a-real-binary-xyz"], 5));
        assert!(r.spawn_error.is_some());

        // The environment really is cleared.
        let r = exec.run(&ExecRequest {
            env: vec![],
            ..req(&["/usr/bin/env"], 5)
        });
        assert_eq!(String::from_utf8_lossy(&r.output).trim(), "");
    }
}
