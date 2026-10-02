//! The assurance thread as a service: the one place the CLI, the HTTP routes and the MCP tools
//! get their answers from.
//!
//! Every answer is **structured** and **revision-qualified**: it carries the model revision
//! and implementation revision it was computed at, so a caller can tell what it is looking at
//! and whether it has gone stale. Every answer is also **targeted**: a caller asks for one
//! requirement, one trace, one case's evidence, instead of loading the whole baseline into
//! context. [`AssuranceService::list_requirements`] returns one short row per requirement, and
//! [`AssuranceService::get_requirement`] returns one requirement in full.
//!
//! The baseline and the model are re-read on every call, so an edit is visible immediately and
//! there is no cache to go stale.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use ufo_types::mbse::assurance::{
    statement_issues, Assurance, CurrentRevisions, EvidenceRecord, Freshness, GapKind, NodeKind,
    ProfileRequirement, RequirementThread, ThreadError, VerificationResult, NODE_KIND_ATTR,
};
use ufo_types::mbse::requirements::RequirementGraph;

use crate::assurance_baseline::{load_baseline, BaselineError, LoadedBaseline};
use crate::assurance_trace::{
    resolve_links, ArtifactResolver, DirResolver, LinkResolution, ModelRevisionIndex,
};
use crate::assurance_view::{assurance_view, AssuranceView};
use crate::evidence_store::{ArtifactCheck, EvidenceStoreError, FsEvidenceStore};
use crate::verification_runner::{
    cases_from_graph, current_revisions, CommandExecutor, RevisionProbe, RunnerConfig, RunnerError,
    VerificationRunner,
};

#[derive(Debug, Clone)]
pub struct AssuranceConfig {
    /// The baseline authoring file. Its `[baseline] model` is resolved relative to its directory.
    pub baseline_path: PathBuf,
    /// The repository: implementation paths, test targets and `git` revisions resolve here.
    pub repo_root: PathBuf,
    pub evidence_dir: PathBuf,
    pub run_timeout: Duration,
}

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("the baseline declares no `[baseline] model`, so links cannot be resolved against a model revision")]
    NoModel,
    #[error(transparent)]
    Baseline(#[from] BaselineError),
    #[error("implementation revision: {0}")]
    Probe(String),
    #[error(transparent)]
    Evidence(#[from] EvidenceStoreError),
    #[error(transparent)]
    Thread(#[from] ThreadError),
    #[error("unknown requirement `{0}`")]
    UnknownRequirement(String),
    #[error("unknown verification case `{0}`")]
    UnknownCase(String),
    /// The caller asked to verify one implementation revision, the repository is at another.
    #[error("revision mismatch: you asked to verify {expected}, the repository is at {actual}")]
    RevisionMismatch { expected: String, actual: String },
    #[error(transparent)]
    Runner(#[from] RunnerError),
}

/// The revisions every answer is qualified by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Revisions {
    pub model_revision: String,
    pub implementation_revision: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequirementSummary {
    pub id: String,
    pub title: String,
    pub status: String,
    pub owner: String,
    pub source_kind: String,
    pub assurance: Assurance,
    pub gaps: Vec<GapKind>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequirementList {
    pub revisions: Revisions,
    /// How many requirements matched; the list is exactly these.
    pub total: usize,
    pub requirements: Vec<RequirementSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ObligationRef {
    pub id: String,
    pub title: String,
    pub source: String,
    pub source_kind: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ControlRef {
    pub id: String,
    pub title: String,
    pub component: String,
    /// Empty = no implementation recorded (an implementation gap).
    pub implementation: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaseRef {
    pub id: String,
    pub title: String,
    pub command: Vec<String>,
    pub acceptance: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequirementDetail {
    pub revisions: Revisions,
    pub requirement: ProfileRequirement,
    pub statement_lints: Vec<String>,
    pub obligations: Vec<ObligationRef>,
    pub elements: Vec<String>,
    pub controls: Vec<ControlRef>,
    pub cases: Vec<CaseRef>,
    pub thread: RequirementThread,
    pub gaps: Vec<GapKind>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceReport {
    pub revisions: Revisions,
    pub requirement_id: String,
    pub links: Vec<crate::assurance_trace::ResolvedLink>,
    pub dangling: usize,
    pub gaps: Vec<GapKind>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvidenceItem {
    pub record: EvidenceRecord,
    pub evidence_key: String,
    pub freshness: Freshness,
    /// `intact`, `missing`, `digest_mismatch` or `external`.
    pub artifact: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvidenceList {
    pub revisions: Revisions,
    pub records: Vec<EvidenceItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VerificationOutcome {
    pub revisions: Revisions,
    pub case_id: String,
    pub result: VerificationResult,
    pub detail: String,
    pub evidence_keys: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct RequirementFilter {
    pub status: Option<String>,
    pub owner: Option<String>,
    /// `unsatisfied`, `satisfied_untested`, `verified`, `failing`, `stale`.
    pub state: Option<String>,
    /// A gap kind as serialised (e.g. `control_not_implemented`).
    pub gap: Option<String>,
}

/// Everything computed at one moment from the files and the store.
pub struct Snapshot {
    pub loaded: LoadedBaseline,
    pub index: ModelRevisionIndex,
    pub current: CurrentRevisions,
    pub evidence: Vec<EvidenceRecord>,
    pub links: LinkResolution,
    pub view: AssuranceView,
}

impl Snapshot {
    pub fn graph(&self) -> &RequirementGraph {
        &self.loaded.graph
    }

    pub fn revisions(&self) -> Revisions {
        Revisions {
            model_revision: self.current.model_revision.clone(),
            implementation_revision: self.current.implementation_revision.clone(),
        }
    }
}

pub struct AssuranceService {
    cfg: AssuranceConfig,
    store: FsEvidenceStore,
    probe: Arc<dyn RevisionProbe>,
    executor: Arc<dyn CommandExecutor>,
}

fn gap_name(g: &GapKind) -> String {
    serde_json::to_value(g)
        .ok()
        .and_then(|v| v.get("gap").and_then(|n| n.as_str().map(str::to_owned)))
        .unwrap_or_default()
}

fn attr<'a>(graph: &'a RequirementGraph, id: &str, key: &str) -> &'a str {
    graph
        .requirements
        .iter()
        .find(|n| n.id == id)
        .and_then(|n| n.attributes.get(key))
        .map(String::as_str)
        .unwrap_or("")
}

impl AssuranceService {
    pub fn new(
        cfg: AssuranceConfig,
        probe: Arc<dyn RevisionProbe>,
        executor: Arc<dyn CommandExecutor>,
    ) -> Self {
        let store = FsEvidenceStore::new(&cfg.evidence_dir);
        Self {
            cfg,
            store,
            probe,
            executor,
        }
    }

    pub fn config(&self) -> &AssuranceConfig {
        &self.cfg
    }

    pub fn store(&self) -> &FsEvidenceStore {
        &self.store
    }

    fn read(path: &Path) -> Result<String, ServiceError> {
        std::fs::read_to_string(path).map_err(|source| ServiceError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    /// The model revision alone (content address of the model file), without the rest.
    pub fn model_revision(&self) -> Option<String> {
        let text = Self::read(&self.cfg.baseline_path).ok()?;
        let loaded = load_baseline(&text, "baseline", "-", None).ok()?;
        let model = self.cfg.baseline_path.parent()?.join(loaded.model?);
        Some(
            ModelRevisionIndex::from_sysml_text(&Self::read(&model).ok()?)
                .revision()
                .to_string(),
        )
    }

    /// Re-read everything and compute the thread at the current revisions.
    pub fn snapshot(&self) -> Result<Snapshot, ServiceError> {
        let text = Self::read(&self.cfg.baseline_path)?;
        let implementation = self
            .probe
            .implementation_revision()
            .map_err(ServiceError::Probe)?;
        let preliminary = load_baseline(
            &text,
            &self.cfg.baseline_path.to_string_lossy(),
            &implementation,
            None,
        )?;
        let model_rel = preliminary.model.clone().ok_or(ServiceError::NoModel)?;
        let model_text = Self::read(
            &self
                .cfg
                .baseline_path
                .parent()
                .unwrap_or(Path::new("."))
                .join(model_rel),
        )?;
        let index = ModelRevisionIndex::from_sysml_text(&model_text);
        let loaded = load_baseline(
            &text,
            &self.cfg.baseline_path.to_string_lossy(),
            &implementation,
            Some(index.revision()),
        )?;
        let current = current_revisions(&loaded.graph, self.probe.as_ref(), index.revision())
            .map_err(ServiceError::Probe)?;
        let evidence = self.store.list()?;
        let artifacts = DirResolver::new(&self.cfg.repo_root);
        let links = resolve_links(&loaded.graph, &index, &artifacts as &dyn ArtifactResolver);
        let view = assurance_view(&loaded.graph, &evidence, &current, Some(index.locators()))?;
        Ok(Snapshot {
            loaded,
            index,
            current,
            evidence,
            links,
            view,
        })
    }

    fn gaps_of(snapshot: &Snapshot, id: &str) -> Vec<GapKind> {
        snapshot.view.report.gaps_for(id).cloned().collect()
    }

    pub fn list_requirements(
        &self,
        filter: &RequirementFilter,
    ) -> Result<RequirementList, ServiceError> {
        let snap = self.snapshot()?;
        let graph = snap.graph();
        let mut rows = Vec::new();
        for row in &snap.view.rows {
            let gaps = Self::gaps_of(&snap, &row.requirement_id);
            let summary = RequirementSummary {
                id: row.requirement_id.clone(),
                title: graph
                    .requirements
                    .iter()
                    .find(|n| n.id == row.requirement_id)
                    .map(|n| n.title.clone())
                    .unwrap_or_default(),
                status: attr(graph, &row.requirement_id, "status").to_string(),
                owner: attr(graph, &row.requirement_id, "owner").to_string(),
                source_kind: attr(graph, &row.requirement_id, "source_kind").to_string(),
                assurance: row.assurance,
                gaps,
            };
            let keep = filter.status.as_ref().is_none_or(|s| &summary.status == s)
                && filter.owner.as_ref().is_none_or(|o| &summary.owner == o)
                && filter
                    .state
                    .as_ref()
                    .is_none_or(|s| crate::assurance_view::state_name(summary.assurance) == s)
                && filter
                    .gap
                    .as_ref()
                    .is_none_or(|g| summary.gaps.iter().any(|k| &gap_name(k) == g));
            if keep {
                rows.push(summary);
            }
        }
        Ok(RequirementList {
            revisions: snap.revisions(),
            total: rows.len(),
            requirements: rows,
        })
    }

    pub fn get_requirement(&self, id: &str) -> Result<RequirementDetail, ServiceError> {
        let snap = self.snapshot()?;
        let graph = snap.graph();
        let node = graph
            .requirements
            .iter()
            .find(|n| {
                n.id == id
                    && n.attributes.get(NODE_KIND_ATTR).map(String::as_str) == Some("requirement")
            })
            .ok_or_else(|| ServiceError::UnknownRequirement(id.to_string()))?;
        let requirement = ProfileRequirement::from_node(node)
            .map_err(|issues| ServiceError::Baseline(BaselineError::Profile(issues)))?;
        let thread = snap
            .view
            .report
            .requirement(id)
            .cloned()
            .ok_or_else(|| ServiceError::UnknownRequirement(id.to_string()))?;
        let by_id = |i: &str| graph.requirements.iter().find(|n| n.id == i);
        let obligations = thread
            .sources
            .iter()
            .filter_map(|i| by_id(i))
            .map(|n| ObligationRef {
                id: n.id.clone(),
                title: n.title.clone(),
                source: n.attributes.get("source").cloned().unwrap_or_default(),
                source_kind: n.attributes.get("source_kind").cloned().unwrap_or_default(),
                version: n.attributes.get("version").cloned().unwrap_or_default(),
            })
            .collect();
        let controls = thread
            .enforced_by
            .iter()
            .filter_map(|i| by_id(i))
            .map(|n| ControlRef {
                id: n.id.clone(),
                title: n.title.clone(),
                component: n.attributes.get("component").cloned().unwrap_or_default(),
                implementation: n
                    .attributes
                    .get("implementation")
                    .cloned()
                    .unwrap_or_default(),
            })
            .collect();
        let cases = thread
            .verification_cases
            .iter()
            .filter_map(|i| by_id(i))
            .map(|n| CaseRef {
                id: n.id.clone(),
                title: n.title.clone(),
                command: n
                    .attributes
                    .get("command")
                    .and_then(|c| serde_json::from_str(c).ok())
                    .unwrap_or_default(),
                acceptance: n.text.clone(),
            })
            .collect();
        Ok(RequirementDetail {
            revisions: snap.revisions(),
            statement_lints: statement_issues(&requirement.statement)
                .iter()
                .map(|i| i.to_string())
                .collect(),
            requirement,
            obligations,
            elements: thread.satisfied_by.clone(),
            controls,
            cases,
            gaps: Self::gaps_of(&snap, id),
            thread,
        })
    }

    pub fn trace_requirement(&self, id: &str) -> Result<TraceReport, ServiceError> {
        let snap = self.snapshot()?;
        if !snap.view.rows.iter().any(|r| r.requirement_id == id) {
            return Err(ServiceError::UnknownRequirement(id.to_string()));
        }
        let links: Vec<_> = snap.links.for_requirement(id).cloned().collect();
        let dangling = links
            .iter()
            .filter(|l| {
                matches!(
                    l.status,
                    crate::assurance_trace::LinkStatus::Dangling { .. }
                )
            })
            .count();
        Ok(TraceReport {
            revisions: snap.revisions(),
            requirement_id: id.to_string(),
            links,
            dangling,
            gaps: Self::gaps_of(&snap, id),
        })
    }

    pub fn evidence(
        &self,
        requirement: Option<&str>,
        case: Option<&str>,
    ) -> Result<EvidenceList, ServiceError> {
        let snap = self.snapshot()?;
        let checks = self.store.verify()?;
        let records = checks
            .into_iter()
            .filter(|(r, _)| requirement.is_none_or(|q| r.requirement_id == q))
            .filter(|(r, _)| case.is_none_or(|c| r.verification_id == c))
            .map(|(record, check)| EvidenceItem {
                evidence_key: record.key(),
                freshness: record.freshness(&snap.current),
                artifact: match check {
                    ArtifactCheck::Intact => "intact",
                    ArtifactCheck::Missing => "missing",
                    ArtifactCheck::DigestMismatch { .. } => "digest_mismatch",
                    ArtifactCheck::External => "external",
                },
                record,
            })
            .collect();
        Ok(EvidenceList {
            revisions: snap.revisions(),
            records,
        })
    }

    /// Run one declared case (blocking). `expected_implementation_revision` is what the caller
    /// believes it is verifying; a different repository revision refuses the run.
    pub fn run_verification(
        &self,
        case_id: &str,
        expected_implementation_revision: &str,
    ) -> Result<VerificationOutcome, ServiceError> {
        let snap = self.snapshot()?;
        let case = cases_from_graph(snap.graph())
            .into_iter()
            .find(|c| c.id == case_id)
            .ok_or_else(|| ServiceError::UnknownCase(case_id.to_string()))?;
        let actual = self
            .probe
            .implementation_revision()
            .map_err(ServiceError::Probe)?;
        if actual != expected_implementation_revision {
            return Err(ServiceError::RevisionMismatch {
                expected: expected_implementation_revision.to_string(),
                actual,
            });
        }
        let runner = VerificationRunner {
            store: &self.store,
            executor: self.executor.as_ref(),
            probe: self.probe.as_ref(),
            config: RunnerConfig {
                repo_root: self.cfg.repo_root.clone(),
                timeout: self.cfg.run_timeout,
                model_revision: snap.index.revision().to_string(),
            },
        };
        let report = runner.run_case(&case)?;
        Ok(VerificationOutcome {
            revisions: Revisions {
                model_revision: report.revisions.model_revision.clone(),
                implementation_revision: report.revisions.implementation_revision.clone(),
            },
            case_id: report.case_id,
            result: report.result,
            detail: report.detail,
            evidence_keys: report.records.iter().map(|r| r.key()).collect(),
        })
    }

    /// Run every declared case whose test target exists, in id order.
    pub fn verify_all(&self) -> Result<Vec<VerificationOutcome>, ServiceError> {
        let snap = self.snapshot()?;
        let implementation = snap.current.implementation_revision.clone();
        let artifacts = DirResolver::new(&self.cfg.repo_root);
        let mut out = Vec::new();
        for case in cases_from_graph(snap.graph()) {
            let runnable =
                crate::assurance_trace::cargo_test_target(&case.argv).is_some_and(|(pkg, test)| {
                    artifacts.exists(&format!("crates/{pkg}/tests/{test}.rs"))
                });
            if runnable {
                out.push(self.run_verification(&case.id, &implementation)?);
            }
        }
        Ok(out)
    }

    /// Which node kinds the baseline defines, for diagnostics.
    pub fn node_kinds(snapshot: &Snapshot) -> Vec<(NodeKind, usize)> {
        NodeKind::ALL
            .iter()
            .map(|k| {
                let n = snapshot
                    .graph()
                    .requirements
                    .iter()
                    .filter(|r| {
                        r.attributes.get(NODE_KIND_ATTR).map(String::as_str) == Some(k.as_str())
                    })
                    .count();
                (*k, n)
            })
            .collect()
    }
}
