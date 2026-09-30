//! Fetch -> diff -> commit engine shared by every "reconcile managed elements into a
//! live SysML v2 project" sync (`digital_thread_sync`, `flexo_reqif_sync`).
//!
//! `DataVersion.payload` is a full replacement, so a diff computed against a stale head
//! is destructive: it would overwrite whatever another tool committed in between. This
//! engine therefore (1) resolves the head of the *target branch* rather than trusting
//! list order, (2) commits with `previousCommit` set to the head it actually diffed
//! against, (3) re-checks the head right before posting, and (4) re-runs the whole
//! fetch/diff on a conflict, giving up after [`MAX_SYNC_ATTEMPTS`].
//! Callers supply only the domain-specific parts: which elements they own and how to diff.

use kr0ki_sysmlv2_client::{
    ClientError, Commit, CommitRequest, DataVersion, Element, Ref, SysmlV2Client,
};

/// Attempts (fetch + diff + POST) before a persistent conflict is reported.
pub const MAX_SYNC_ATTEMPTS: usize = 3;

/// Which project (and optionally branch) to sync into.
#[derive(Debug, Clone)]
pub struct SyncConfig {
    pub project_id: String,
    pub branch_id: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error(transparent)]
    Client(#[from] kr0ki_sysmlv2_client::ClientError),

    /// Two managed elements (or two graph nodes) share one identifier, so a diff keyed by
    /// identifier would silently drop one of them. Fix the data; do not guess.
    #[error("duplicate managed identifier(s): {}", .0.join(", "))]
    DuplicateIdentifier(Vec<String>),

    /// Another writer kept moving the branch head; nothing was committed.
    #[error(
        "sync conflicted with concurrent commits on {attempts} attempt(s); nothing was committed"
    )]
    Conflict { attempts: usize },
}

fn is_conflict(e: &ClientError) -> bool {
    matches!(
        e,
        ClientError::Status {
            code: 409 | 412,
            ..
        }
    )
}

/// The commit the sync should diff against: the target branch's head (the explicit
/// `branch_id`, else the project's default branch). Servers that don't report a default
/// branch fall back to the newest commit by `created` (first in list order on ties) --
/// never blindly `commits[0]`, whose order the API does not define. `None` means the
/// target has no commits yet.
async fn resolve_head(
    client: &SysmlV2Client,
    config: &SyncConfig,
) -> Result<Option<Commit>, SyncError> {
    let project = &config.project_id;
    let explicit = config.branch_id.is_some();
    let branch_id = match &config.branch_id {
        Some(b) => Some(b.clone()),
        None => match client.project(project).await {
            Ok(p) => p.default_branch_id().map(str::to_owned),
            Err(ClientError::Status { code: 404, .. }) => None,
            Err(e) => return Err(e.into()),
        },
    };

    if let Some(branch_id) = branch_id {
        match client.branch(project, &branch_id).await {
            Ok(branch) => {
                return match branch.head_id() {
                    Some(head) => Ok(Some(client.commit(project, head).await?)),
                    None => Ok(None),
                }
            }
            // A branch the caller named must resolve; a guessed default may simply not exist.
            Err(ClientError::Status { code: 404, .. }) if !explicit => {}
            Err(e) => return Err(e.into()),
        }
    }

    let commits = client.commits(project).await?;
    let mut latest: Option<&Commit> = None;
    for c in &commits {
        if latest.is_none_or(|l| c.created > l.created) {
            latest = Some(c);
        }
    }
    Ok(latest.cloned())
}

/// Reconcile the elements `is_managed` selects, using `build` to turn their current
/// state into a changeset. Returns the new commit, the unchanged head when there was
/// nothing to change, or `None` when there is nothing to change and no commit exists
/// (an empty commit is never posted).
pub(crate) async fn sync_managed(
    client: &SysmlV2Client,
    config: &SyncConfig,
    is_managed: impl Fn(&Element) -> bool,
    build: impl Fn(&[Element]) -> Result<Vec<DataVersion>, SyncError>,
) -> Result<Option<Commit>, SyncError> {
    for _attempt in 0..MAX_SYNC_ATTEMPTS {
        let head = resolve_head(client, config).await?;
        let managed: Vec<Element> = match &head {
            Some(commit) => client
                .all_elements(&config.project_id, &commit.at_id)
                .await?
                .into_iter()
                .filter(&is_managed)
                .collect(),
            None => Vec::new(),
        };

        let changes = build(&managed)?;
        if changes.is_empty() {
            return Ok(head);
        }

        // Narrow (not close) the read-modify-write window: if the head moved while we were
        // diffing, the payloads we built are stale.
        let current = resolve_head(client, config).await?;
        if current.as_ref().map(|c| &c.at_id) != head.as_ref().map(|c| &c.at_id) {
            continue;
        }

        let request = CommitRequest {
            type_: "Commit",
            change: changes,
            previous_commit: head.as_ref().map(|c| Ref {
                at_id: c.at_id.clone(),
                extra: Default::default(),
            }),
        };
        match client
            .create_commit(&config.project_id, config.branch_id.as_deref(), request)
            .await
        {
            Ok(commit) => return Ok(Some(commit)),
            Err(e) if is_conflict(&e) => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(SyncError::Conflict {
        attempts: MAX_SYNC_ATTEMPTS,
    })
}
