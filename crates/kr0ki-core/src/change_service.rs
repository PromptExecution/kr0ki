//! KR-A05: the model commit service — **propose, validate, commit with an expected base
//! revision, reject when it is stale**.
//!
//! `sync_engine::sync_managed` retries on a moved head: it re-reads, re-diffs and posts again.
//! That is right for reconciling *derived* state, where the desired content is a pure function
//! of the world. It is wrong for an agent's *proposed* edit, which was computed against a
//! specific base and may no longer mean what its author intended on a different one. So this
//! service never rebases:
//!
//! 1. [`ChangeService::propose`] reads the head, diffs the desired elements against it, and
//!    returns a [`ProposedChange`] naming that **base revision**. Nothing is written.
//! 2. [`ChangeService::validate`] reports whether the proposal is still current.
//! 3. [`ChangeService::commit`]`(change_id, expected_revision)` applies it only if the head
//!    **is** `expected_revision`. Otherwise it returns [`ChangeError::StaleBase`] naming both
//!    revisions and writes nothing, so a change committed in between is never overwritten
//!    (`DataVersion.payload` is a full replacement; a diff against a stale base is destructive).
//!
//! Commits to one branch are serialised in-process, so two concurrent callers with the same
//! expected revision cannot both pass the check.
//!
//! **Known limit (observed live, 2026-10-02): the OMG pilot server does not enforce
//! `previousCommit`.** A commit that names a stale previous commit is *accepted*
//! (`tests/assurance_a05_live.rs` records this on every run). So the server provides no
//! optimistic concurrency, and this service's check is the only guard: it holds for every writer
//! that goes through one `ChangeService`, but a writer that bypasses it (another process, a
//! direct API call) can still interleave between the head check and the write. Closing that
//! window needs either a server that enforces `previousCommit` (Flexo may) or a single writer in
//! front of the model. If a server does report a conflict, that is returned as a stale base too,
//! never retried.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::sync::{Arc, Mutex};

use kr0ki_sysmlv2_client::{CommitRequest, DataVersion, Element, Ref, SysmlV2Client};
use serde::Serialize;
use serde_json::Value;

use crate::assurance_baseline::sha256_hex;
use crate::sync_engine::{
    diff_managed, element_identifier, resolve_head, DesiredElement, SyncConfig, SyncError,
};

// ── Backend ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BackendError {
    /// The server refused the commit because its `previousCommit` is not the head (409/412).
    #[error("the model server reports a conflicting head")]
    Conflict,
    #[error("model backend: {0}")]
    Other(String),
}

/// The slice of a model server the commit service needs.
pub trait ModelBackend: Send + Sync {
    /// The head commit id of the branch (`None`: no commits yet).
    fn head(
        &self,
        project: &str,
        branch: Option<&str>,
    ) -> impl Future<Output = Result<Option<String>, BackendError>> + Send;

    fn elements_at(
        &self,
        project: &str,
        commit: &str,
    ) -> impl Future<Output = Result<Vec<Element>, BackendError>> + Send;

    /// Post a commit whose `previousCommit` is `previous`. Returns the new commit id.
    fn commit(
        &self,
        project: &str,
        branch: Option<&str>,
        previous: Option<&str>,
        changes: Vec<DataVersion>,
    ) -> impl Future<Output = Result<String, BackendError>> + Send;
}

/// The real thing: an OMG Systems Modeling API server through `kr0ki-sysmlv2-client`.
#[derive(Clone)]
pub struct SysmlBackend {
    client: Arc<SysmlV2Client>,
}

impl SysmlBackend {
    pub fn new(client: Arc<SysmlV2Client>) -> Self {
        Self { client }
    }
}

impl ModelBackend for SysmlBackend {
    async fn head(
        &self,
        project: &str,
        branch: Option<&str>,
    ) -> Result<Option<String>, BackendError> {
        let config = SyncConfig {
            project_id: project.into(),
            branch_id: branch.map(str::to_owned),
        };
        resolve_head(&self.client, &config)
            .await
            .map(|c| c.map(|c| c.at_id))
            .map_err(|e| BackendError::Other(e.to_string()))
    }

    async fn elements_at(&self, project: &str, commit: &str) -> Result<Vec<Element>, BackendError> {
        self.client
            .all_elements(project, commit)
            .await
            .map_err(|e| BackendError::Other(e.to_string()))
    }

    async fn commit(
        &self,
        project: &str,
        branch: Option<&str>,
        previous: Option<&str>,
        changes: Vec<DataVersion>,
    ) -> Result<String, BackendError> {
        let request = CommitRequest {
            type_: "Commit",
            change: changes,
            previous_commit: previous.map(|p| Ref {
                at_id: p.to_string(),
                extra: Default::default(),
            }),
        };
        match self.client.create_commit(project, branch, request).await {
            Ok(c) => Ok(c.at_id),
            Err(kr0ki_sysmlv2_client::ClientError::Status {
                code: 409 | 412, ..
            }) => Err(BackendError::Conflict),
            Err(e) => Err(BackendError::Other(e.to_string())),
        }
    }
}

// ── Proposals ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Create,
    Update,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChangeSummary {
    pub operation: Operation,
    /// The managed identifier (or the element id for a delete of an unidentified element).
    pub identifier: String,
}

/// A computed, uncommitted change and the revision it was computed against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProposedChange {
    pub change_id: String,
    pub project_id: String,
    pub branch_id: Option<String>,
    /// The head the diff was computed against. `None`: the branch had no commits.
    pub base_revision: Option<String>,
    pub summary: Vec<ChangeSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Validation {
    pub change_id: String,
    pub base_revision: Option<String>,
    pub current_revision: Option<String>,
    /// `true` when the head is still the proposal's base.
    pub current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Committed {
    pub change_id: String,
    /// The revision the change was applied on top of.
    pub base_revision: Option<String>,
    pub commit_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChangeError {
    /// The head is not the revision the caller expected. Nothing was written. Re-read, re-propose.
    #[error("stale base: expected revision {expected:?} but the head is {current:?}; nothing was written")]
    StaleBase {
        expected: Option<String>,
        current: Option<String>,
    },
    /// The caller's expected revision is not the one the proposal was computed against.
    #[error(
        "expected revision {expected:?} is not the proposal's base {base:?}; nothing was written"
    )]
    ProposalMismatch {
        expected: Option<String>,
        base: Option<String>,
    },
    #[error("unknown or already-committed change `{0}`")]
    UnknownChange(String),
    #[error("the desired state already matches revision {0:?}: nothing to commit")]
    NoChanges(Option<String>),
    #[error(transparent)]
    Sync(SyncErrorText),
    #[error(transparent)]
    Backend(BackendError),
}

/// `SyncError` is not `Clone`/`Eq`; keep its message.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct SyncErrorText(String);

impl From<SyncError> for ChangeError {
    fn from(e: SyncError) -> Self {
        ChangeError::Sync(SyncErrorText(e.to_string()))
    }
}

struct Stored {
    proposal: ProposedChange,
    changes: Vec<DataVersion>,
}

pub struct ChangeService<B> {
    backend: B,
    proposals: Mutex<BTreeMap<String, Stored>>,
    branch_locks: Mutex<BTreeMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl<B: ModelBackend> ChangeService<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            proposals: Mutex::default(),
            branch_locks: Mutex::default(),
        }
    }

    /// The backend this service writes through (read access, for diagnostics and tests).
    pub fn backend(&self) -> &B {
        &self.backend
    }

    fn lock_for(&self, project: &str, branch: Option<&str>) -> Arc<tokio::sync::Mutex<()>> {
        let key = format!("{project}\u{0}{}", branch.unwrap_or(""));
        self.branch_locks
            .lock()
            .expect("lock map")
            .entry(key)
            .or_default()
            .clone()
    }

    /// Diff `desired` against the head, restricted to elements whose identifier starts with
    /// `managed_prefix`. Writes nothing.
    pub async fn propose(
        &self,
        project: &str,
        branch: Option<&str>,
        managed_prefix: &str,
        desired: &[DesiredElement],
    ) -> Result<ProposedChange, ChangeError> {
        let head = self
            .backend
            .head(project, branch)
            .await
            .map_err(ChangeError::Backend)?;
        let existing: Vec<Element> = match &head {
            Some(commit) => self
                .backend
                .elements_at(project, commit)
                .await
                .map_err(ChangeError::Backend)?
                .into_iter()
                .filter(|e| element_identifier(e).is_some_and(|i| i.starts_with(managed_prefix)))
                .collect(),
            None => Vec::new(),
        };
        let changes = diff_managed(&existing, desired)?;
        if changes.is_empty() {
            return Err(ChangeError::NoChanges(head));
        }

        let by_id: BTreeMap<&str, &str> = existing
            .iter()
            .filter_map(|e| element_identifier(e).map(|i| (e.id(), i)))
            .collect();
        let summary: Vec<ChangeSummary> = changes
            .iter()
            .map(|c| {
                let identity = c.identity.as_ref().map(|r| r.at_id.as_str());
                match (&c.payload, identity) {
                    (Some(p), None) => ChangeSummary {
                        operation: Operation::Create,
                        identifier: payload_identifier(p),
                    },
                    (Some(p), Some(id)) => ChangeSummary {
                        operation: Operation::Update,
                        identifier: by_id
                            .get(id)
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| payload_identifier(p)),
                    },
                    (None, id) => ChangeSummary {
                        operation: Operation::Delete,
                        identifier: id
                            .and_then(|i| by_id.get(i).map(|s| s.to_string()))
                            .or_else(|| id.map(str::to_owned))
                            .unwrap_or_default(),
                    },
                }
            })
            .collect();

        let change_id = {
            let material = serde_json::json!({
                "project": project,
                "branch": branch,
                "base": head,
                "changes": serde_json::to_value(&changes).unwrap_or(Value::Null),
            });
            format!("chg-{}", &sha256_hex(material.to_string().as_bytes())[..24])
        };
        let proposal = ProposedChange {
            change_id: change_id.clone(),
            project_id: project.into(),
            branch_id: branch.map(str::to_owned),
            base_revision: head,
            summary,
        };
        self.proposals.lock().expect("proposal map").insert(
            change_id,
            Stored {
                proposal: proposal.clone(),
                changes,
            },
        );
        Ok(proposal)
    }

    /// Is the proposal still based on the head?
    pub async fn validate(&self, change_id: &str) -> Result<Validation, ChangeError> {
        let proposal = self.proposal(change_id)?;
        let current = self
            .backend
            .head(&proposal.project_id, proposal.branch_id.as_deref())
            .await
            .map_err(ChangeError::Backend)?;
        Ok(Validation {
            change_id: change_id.into(),
            current: current == proposal.base_revision,
            base_revision: proposal.base_revision,
            current_revision: current,
        })
    }

    /// Apply `change_id` iff the head is `expected_revision`. Never rebases, never retries.
    pub async fn commit(
        &self,
        change_id: &str,
        expected_revision: Option<&str>,
    ) -> Result<Committed, ChangeError> {
        let (proposal, changes) = {
            let map = self.proposals.lock().expect("proposal map");
            let s = map
                .get(change_id)
                .ok_or_else(|| ChangeError::UnknownChange(change_id.into()))?;
            (s.proposal.clone(), s.changes.clone())
        };
        let expected = expected_revision.map(str::to_owned);
        if expected != proposal.base_revision {
            return Err(ChangeError::ProposalMismatch {
                expected,
                base: proposal.base_revision,
            });
        }

        // One committer per branch at a time: the check and the write are not interleaved
        // with another caller's check and write.
        let lock = self.lock_for(&proposal.project_id, proposal.branch_id.as_deref());
        let _guard = lock.lock().await;

        // A concurrent commit may already have consumed this proposal.
        if !self
            .proposals
            .lock()
            .expect("proposal map")
            .contains_key(change_id)
        {
            return Err(ChangeError::UnknownChange(change_id.into()));
        }
        let current = self
            .backend
            .head(&proposal.project_id, proposal.branch_id.as_deref())
            .await
            .map_err(ChangeError::Backend)?;
        if current != expected {
            return Err(ChangeError::StaleBase { expected, current });
        }
        match self
            .backend
            .commit(
                &proposal.project_id,
                proposal.branch_id.as_deref(),
                expected.as_deref(),
                changes,
            )
            .await
        {
            Ok(commit_id) => {
                self.proposals
                    .lock()
                    .expect("proposal map")
                    .remove(change_id);
                Ok(Committed {
                    change_id: change_id.into(),
                    base_revision: expected,
                    commit_id,
                })
            }
            Err(BackendError::Conflict) => {
                // The server saw a head we did not: report it as the stale base it is.
                let current = self
                    .backend
                    .head(&proposal.project_id, proposal.branch_id.as_deref())
                    .await
                    .map_err(ChangeError::Backend)?;
                Err(ChangeError::StaleBase { expected, current })
            }
            Err(e) => Err(ChangeError::Backend(e)),
        }
    }

    fn proposal(&self, change_id: &str) -> Result<ProposedChange, ChangeError> {
        self.proposals
            .lock()
            .expect("proposal map")
            .get(change_id)
            .map(|s| s.proposal.clone())
            .ok_or_else(|| ChangeError::UnknownChange(change_id.into()))
    }

    /// Identifiers of proposals still open (drafts).
    pub fn open_changes(&self) -> BTreeSet<String> {
        self.proposals
            .lock()
            .expect("proposal map")
            .keys()
            .cloned()
            .collect()
    }
}

fn payload_identifier(payload: &Value) -> String {
    payload
        .get("identifier")
        .and_then(Value::as_str)
        .or_else(|| {
            payload
                .get("aliasIds")
                .and_then(Value::as_array)
                .and_then(|a| a.first())
                .and_then(Value::as_str)
        })
        .unwrap_or_default()
        .to_string()
}

/// A stateful in-memory [`ModelBackend`] with the semantics of the real server: commits form
/// a chain, and each commit's elements are the previous commit's with the changes applied.
/// For tests and demos (including `kr0ki-server`'s), where no real model server is available;
/// the real server is exercised by the `#[ignore]`d live tests.
pub mod in_memory {

    use super::*;
    use serde_json::json;

    #[derive(Default)]
    struct State {
        commits: Vec<(String, BTreeMap<String, Value>)>,
        next_element: u32,
        /// Conflict on the next commit even though `previous` was the head.
        conflict_once: bool,
    }

    pub struct InMemoryServer {
        state: Mutex<State>,
        /// Mimic a server that ignores `previousCommit` (the service's own check must catch it).
        pub enforces_previous: bool,
    }

    impl InMemoryServer {
        pub fn new(enforces_previous: bool) -> Self {
            Self {
                state: Mutex::default(),
                enforces_previous,
            }
        }

        pub fn inject_conflict_once(&self) {
            self.state.lock().unwrap().conflict_once = true;
        }

        pub fn head_id(&self) -> Option<String> {
            self.state
                .lock()
                .unwrap()
                .commits
                .last()
                .map(|(id, _)| id.clone())
        }

        pub fn commit_count(&self) -> usize {
            self.state.lock().unwrap().commits.len()
        }

        /// identifier -> `name` of every element at the head commit.
        pub fn names_at_head(&self) -> BTreeMap<String, String> {
            let s = self.state.lock().unwrap();
            s.commits
                .last()
                .map(|(_, els)| {
                    els.values()
                        .filter_map(|v| {
                            let id = v.get("identifier").and_then(Value::as_str)?;
                            Some((
                                id.to_string(),
                                v.get("name")
                                    .and_then(Value::as_str)
                                    .unwrap_or("")
                                    .to_string(),
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default()
        }

        pub fn identifiers_at_head(&self) -> BTreeSet<String> {
            let s = self.state.lock().unwrap();
            s.commits
                .last()
                .map(|(_, els)| {
                    els.values()
                        .filter_map(|v| {
                            v.get("identifier")
                                .and_then(Value::as_str)
                                .map(str::to_owned)
                        })
                        .collect()
                })
                .unwrap_or_default()
        }
    }

    impl ModelBackend for InMemoryServer {
        async fn head(&self, _p: &str, _b: Option<&str>) -> Result<Option<String>, BackendError> {
            Ok(self.head_id())
        }

        async fn elements_at(&self, _p: &str, commit: &str) -> Result<Vec<Element>, BackendError> {
            let s = self.state.lock().unwrap();
            let (_, els) = s
                .commits
                .iter()
                .find(|(id, _)| id == commit)
                .ok_or_else(|| BackendError::Other(format!("no commit {commit}")))?;
            Ok(els
                .iter()
                .map(|(id, v)| {
                    let mut v = v.clone();
                    v["@id"] = json!(id);
                    serde_json::from_value(v).unwrap()
                })
                .collect())
        }

        async fn commit(
            &self,
            _p: &str,
            _b: Option<&str>,
            previous: Option<&str>,
            changes: Vec<DataVersion>,
        ) -> Result<String, BackendError> {
            let mut s = self.state.lock().unwrap();
            if std::mem::take(&mut s.conflict_once) {
                return Err(BackendError::Conflict);
            }
            let head = s.commits.last().map(|(id, _)| id.clone());
            if self.enforces_previous && previous != head.as_deref() {
                return Err(BackendError::Conflict);
            }
            let mut elements = s.commits.last().map(|(_, e)| e.clone()).unwrap_or_default();
            for c in changes {
                match (c.payload, c.identity) {
                    (Some(p), None) => {
                        s.next_element += 1;
                        elements.insert(format!("el-{}", s.next_element), p);
                    }
                    (Some(p), Some(r)) => {
                        elements.insert(r.at_id, p);
                    }
                    (None, Some(r)) => {
                        elements.remove(&r.at_id);
                    }
                    (None, None) => {}
                }
            }
            let id = format!("commit-{}", s.commits.len() + 1);
            s.commits.push((id.clone(), elements));
            Ok(id)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::in_memory::InMemoryServer;
    use super::*;
    use serde_json::{json, Map};

    const PREFIX: &str = "kr0ki:assurance:";

    fn desired(id: &str, name: &str) -> DesiredElement {
        let mut fields = Map::new();
        fields.insert("name".into(), json!(name));
        fields.insert("custom".into(), json!(1));
        DesiredElement {
            identifier: format!("{PREFIX}{id}"),
            type_: "PartUsage",
            fields,
        }
    }

    fn id(s: &str) -> String {
        format!("{PREFIX}{s}")
    }

    async fn seeded(enforces: bool) -> (ChangeService<InMemoryServer>, String) {
        let svc = ChangeService::new(InMemoryServer::new(enforces));
        let base = svc
            .propose("p", None, PREFIX, &[desired("X", "X")])
            .await
            .unwrap();
        let committed = svc.commit(&base.change_id, None).await.unwrap();
        (svc, committed.commit_id)
    }

    #[tokio::test]
    async fn two_edits_from_one_baseline_second_is_rejected_first_is_kept() {
        let (svc, r0) = seeded(true).await;
        let both = [desired("X", "X")];
        let edit1 = svc
            .propose("p", None, PREFIX, &[both[0].clone(), desired("A", "A")])
            .await
            .unwrap();
        let edit2 = svc
            .propose("p", None, PREFIX, &[both[0].clone(), desired("B", "B")])
            .await
            .unwrap();
        assert_eq!(edit1.base_revision.as_deref(), Some(r0.as_str()));
        assert_eq!(
            edit2.base_revision, edit1.base_revision,
            "both edits start from one baseline"
        );
        assert_ne!(edit1.change_id, edit2.change_id);

        let r1 = svc
            .commit(&edit1.change_id, Some(&r0))
            .await
            .unwrap()
            .commit_id;
        let before = svc.backend.commit_count();

        let err = svc.commit(&edit2.change_id, Some(&r0)).await.unwrap_err();
        assert_eq!(
            err,
            ChangeError::StaleBase {
                expected: Some(r0.clone()),
                current: Some(r1.clone())
            }
        );
        // Nothing was written, and the first edit is intact.
        assert_eq!(svc.backend.commit_count(), before);
        assert_eq!(svc.backend.head_id().as_deref(), Some(r1.as_str()));
        let ids = svc.backend.identifiers_at_head();
        assert!(
            ids.contains(&id("A")),
            "the first edit must survive: {ids:?}"
        );
        assert!(
            !ids.contains(&id("B")),
            "the stale edit must not be applied: {ids:?}"
        );

        // Re-proposing from the new head works and keeps both.
        let redo = svc
            .propose(
                "p",
                None,
                PREFIX,
                &[both[0].clone(), desired("A", "A"), desired("B", "B")],
            )
            .await
            .unwrap();
        assert_eq!(redo.base_revision.as_deref(), Some(r1.as_str()));
        svc.commit(&redo.change_id, Some(&r1)).await.unwrap();
        let ids = svc.backend.identifiers_at_head();
        assert!(ids.contains(&id("A")) && ids.contains(&id("B")));
    }

    #[tokio::test]
    async fn the_service_rejects_a_stale_base_even_when_the_server_ignores_previous_commit() {
        let (svc, r0) = seeded(false).await;
        let x = || desired("X", "X");
        let e1 = svc
            .propose("p", None, PREFIX, &[x(), desired("A", "A")])
            .await
            .unwrap();
        let e2 = svc
            .propose("p", None, PREFIX, &[x(), desired("B", "B")])
            .await
            .unwrap();
        svc.commit(&e1.change_id, Some(&r0)).await.unwrap();
        assert!(matches!(
            svc.commit(&e2.change_id, Some(&r0)).await,
            Err(ChangeError::StaleBase { .. })
        ));
        assert!(!svc.backend.identifiers_at_head().contains(&id("B")));
    }

    #[tokio::test]
    async fn concurrent_commits_with_one_expected_revision_let_exactly_one_through() {
        let (svc, r0) = seeded(false).await;
        let svc = Arc::new(svc);
        let x = || desired("X", "X");
        let e1 = svc
            .propose("p", None, PREFIX, &[x(), desired("A", "A")])
            .await
            .unwrap();
        let e2 = svc
            .propose("p", None, PREFIX, &[x(), desired("B", "B")])
            .await
            .unwrap();
        let (a, b) = {
            let (s1, s2) = (svc.clone(), svc.clone());
            let (r1, r2) = (r0.clone(), r0.clone());
            tokio::join!(
                tokio::spawn(async move { s1.commit(&e1.change_id, Some(&r1)).await }),
                tokio::spawn(async move { s2.commit(&e2.change_id, Some(&r2)).await }),
            )
        };
        let results = [a.unwrap(), b.unwrap()];
        assert_eq!(
            results.iter().filter(|r| r.is_ok()).count(),
            1,
            "{results:?}"
        );
        assert_eq!(
            results
                .iter()
                .filter(|r| matches!(r, Err(ChangeError::StaleBase { .. })))
                .count(),
            1,
            "{results:?}"
        );
        assert_eq!(
            svc.backend.commit_count(),
            2,
            "the seed plus exactly one edit"
        );
    }

    #[tokio::test]
    async fn a_conflict_reported_by_the_server_is_a_stale_base_and_is_not_retried() {
        let (svc, r0) = seeded(true).await;
        let e = svc
            .propose("p", None, PREFIX, &[desired("X", "X"), desired("A", "A")])
            .await
            .unwrap();
        svc.backend.inject_conflict_once();
        let before = svc.backend.commit_count();
        let err = svc.commit(&e.change_id, Some(&r0)).await.unwrap_err();
        assert!(matches!(err, ChangeError::StaleBase { .. }), "{err:?}");
        assert_eq!(svc.backend.commit_count(), before, "no retry, no write");
        // The proposal is still open and can be re-validated.
        assert!(svc.open_changes().contains(&e.change_id));
    }

    #[tokio::test]
    async fn an_expected_revision_that_is_not_the_proposals_base_is_refused() {
        let (svc, r0) = seeded(true).await;
        let e = svc
            .propose("p", None, PREFIX, &[desired("X", "X"), desired("A", "A")])
            .await
            .unwrap();
        let err = svc
            .commit(&e.change_id, Some("commit-999"))
            .await
            .unwrap_err();
        assert_eq!(
            err,
            ChangeError::ProposalMismatch {
                expected: Some("commit-999".into()),
                base: Some(r0)
            }
        );
    }

    #[tokio::test]
    async fn validate_reports_whether_the_proposal_is_still_current() {
        let (svc, r0) = seeded(true).await;
        let x = || desired("X", "X");
        let e1 = svc
            .propose("p", None, PREFIX, &[x(), desired("A", "A")])
            .await
            .unwrap();
        let e2 = svc
            .propose("p", None, PREFIX, &[x(), desired("B", "B")])
            .await
            .unwrap();
        assert!(svc.validate(&e1.change_id).await.unwrap().current);
        svc.commit(&e1.change_id, Some(&r0)).await.unwrap();
        let v = svc.validate(&e2.change_id).await.unwrap();
        assert!(!v.current);
        assert_eq!(v.base_revision.as_deref(), Some(r0.as_str()));
        assert_ne!(v.current_revision, v.base_revision);
    }

    #[tokio::test]
    async fn a_committed_proposal_cannot_be_committed_twice() {
        let (svc, r0) = seeded(true).await;
        let e = svc
            .propose("p", None, PREFIX, &[desired("X", "X"), desired("A", "A")])
            .await
            .unwrap();
        svc.commit(&e.change_id, Some(&r0)).await.unwrap();
        assert_eq!(
            svc.commit(&e.change_id, Some(&r0)).await.unwrap_err(),
            ChangeError::UnknownChange(e.change_id.clone())
        );
    }

    #[tokio::test]
    async fn an_unknown_change_and_a_no_op_are_distinct_errors() {
        let (svc, r0) = seeded(true).await;
        assert!(matches!(
            svc.commit("chg-nope", Some(&r0)).await,
            Err(ChangeError::UnknownChange(_))
        ));
        assert_eq!(
            svc.propose("p", None, PREFIX, &[desired("X", "X")])
                .await
                .unwrap_err(),
            ChangeError::NoChanges(Some(r0))
        );
    }

    #[tokio::test]
    async fn the_proposal_names_what_it_would_do() {
        let (svc, _) = seeded(true).await;
        let e = svc
            .propose(
                "p",
                None,
                PREFIX,
                &[desired("X", "Renamed"), desired("A", "A")],
            )
            .await
            .unwrap();
        let ops: BTreeMap<_, _> = e
            .summary
            .iter()
            .map(|s| (s.identifier.clone(), s.operation))
            .collect();
        assert_eq!(ops[&id("X")], Operation::Update);
        assert_eq!(ops[&id("A")], Operation::Create);
        // Dropping X from the desired set deletes it.
        let d = svc
            .propose("p", None, PREFIX, &[desired("A", "A")])
            .await
            .unwrap();
        let ops: BTreeMap<_, _> = d
            .summary
            .iter()
            .map(|s| (s.identifier.clone(), s.operation))
            .collect();
        assert_eq!(ops[&id("X")], Operation::Delete);
    }

    #[tokio::test]
    async fn elements_outside_the_managed_prefix_are_never_touched() {
        let svc = ChangeService::new(InMemoryServer::new(true));
        // Someone else's element lives on the server.
        let mut other = Map::new();
        other.insert("name".into(), json!("theirs"));
        let theirs = DesiredElement {
            identifier: "other:thing".into(),
            type_: "PartUsage",
            fields: other,
        };
        let seed = svc.propose("p", None, "other:", &[theirs]).await.unwrap();
        let r0 = svc.commit(&seed.change_id, None).await.unwrap().commit_id;
        // Our prefix sees nothing of it, so it is not deleted.
        let e = svc
            .propose("p", None, PREFIX, &[desired("A", "A")])
            .await
            .unwrap();
        assert!(e.summary.iter().all(|s| s.operation == Operation::Create));
        svc.commit(&e.change_id, Some(&r0)).await.unwrap();
        assert!(svc.backend.identifiers_at_head().contains("other:thing"));
    }
}
