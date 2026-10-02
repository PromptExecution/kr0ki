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
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

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

    /// A value the sync had to write could not be encoded as JSON.
    #[error("could not encode element payload: {0}")]
    Encode(#[from] serde_json::Error),

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

/// The sync identifier of an element: its `identifier` field, else the first `aliasIds` entry.
///
/// Both live in `Element`'s flattened `fields` map. Writes put the identifier in both places because the two kinds of
/// server disagree: the OMG pilot implementation is strictly typed and *silently drops unknown fields* (found live
/// against it, 2026-10-02: `identifier` and every custom field vanished, so the next sync could not recognise its own
/// elements and committed again), while `aliasIds` is a standard SysML v2 field it persists.
pub(crate) fn element_identifier(element: &Element) -> Option<&str> {
    element
        .fields
        .get("identifier")
        .and_then(Value::as_str)
        .or_else(|| {
            element
                .fields
                .get("aliasIds")
                .and_then(Value::as_array)
                .and_then(|a| a.iter().find_map(Value::as_str))
        })
}

/// An element a sync wants to exist, identified by `identifier`, together with the
/// writable `fields` the sync owns (anything else on the server element is not ours and
/// is preserved on update).
pub(crate) struct DesiredElement {
    pub identifier: String,
    pub type_: &'static str,
    pub fields: Map<String, Value>,
}

fn data_version(payload: Option<Value>, existing: Option<&Element>) -> DataVersion {
    DataVersion {
        type_: "DataVersion",
        payload,
        identity: existing.map(|e| Ref {
            at_id: e.id().to_string(),
            extra: Default::default(),
        }),
    }
}

/// Create / update / delete changeset that makes `existing` (already restricted to the
/// caller's managed namespace) match `desired`.
///
/// - **create**: an identifier with no server element.
/// - **update**: some owned field differs. `DataVersion.payload` is a full replacement --
///   the OMG reference implementation never merges it with the prior version -- so the
///   payload starts from the element's *current* fields and only overlays the owned ones.
///   A sparse payload would wipe whatever another tool set.
/// - **delete**: a server element whose identifier is no longer desired.
/// - Repeated identifiers on either side are an error, never a silent collapse.
pub(crate) fn diff_managed(
    existing: &[Element],
    desired: &[DesiredElement],
) -> Result<Vec<DataVersion>, SyncError> {
    let mut duplicates = BTreeSet::new();
    let mut by_identifier: BTreeMap<&str, &Element> = BTreeMap::new();
    for e in existing {
        if let Some(id) = element_identifier(e) {
            if by_identifier.insert(id, e).is_some() {
                duplicates.insert(id.to_string());
            }
        }
    }
    let mut wanted = BTreeSet::new();
    for d in desired {
        if !wanted.insert(d.identifier.as_str()) {
            duplicates.insert(d.identifier.clone());
        }
    }
    if !duplicates.is_empty() {
        return Err(SyncError::DuplicateIdentifier(
            duplicates.into_iter().collect(),
        ));
    }

    let mut changes = Vec::new();
    for d in desired {
        match by_identifier.get(d.identifier.as_str()) {
            None => {
                let mut payload = d.fields.clone();
                payload.insert("@type".into(), d.type_.into());
                payload.insert("identifier".into(), d.identifier.clone().into());
                payload.insert("aliasIds".into(), serde_json::json!([d.identifier]));
                changes.push(data_version(Some(Value::Object(payload)), None));
            }
            Some(&current) => {
                // A server that dropped `identifier` is strictly typed and drops every other unknown field too, so an
                // owned field it does not return is unknowable, not changed. Compare only what came back; otherwise
                // identical data would be re-committed on every run.
                let strict_schema = !current.fields.contains_key("identifier");
                let owned_field_differs =
                    d.fields.iter().any(|(k, v)| match current.fields.get(k) {
                        Some(have) => have != v,
                        None => !strict_schema,
                    });
                if owned_field_differs {
                    let mut payload = current.fields.clone();
                    payload.insert("@type".into(), current.ty().into());
                    payload.extend(d.fields.clone());
                    changes.push(data_version(Some(Value::Object(payload)), Some(current)));
                }
            }
        }
    }
    for (identifier, current) in &by_identifier {
        if !wanted.contains(identifier) {
            changes.push(data_version(None, Some(current)));
        }
    }
    Ok(changes)
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
pub(crate) async fn resolve_head(
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn element(v: Value) -> Element {
        serde_json::from_value(v).unwrap()
    }
    fn desired(identifier: &str, name: &str, extra: Value) -> DesiredElement {
        let mut fields = Map::new();
        fields.insert("name".into(), name.into());
        fields.insert("custom".into(), extra);
        DesiredElement {
            identifier: identifier.into(),
            type_: "PartUsage",
            fields,
        }
    }

    #[test]
    fn a_new_element_is_marked_in_both_identifier_and_alias_ids() {
        let changes = diff_managed(&[], &[desired("dbt:a", "A", json!(1))]).unwrap();
        let payload = changes[0].payload.as_ref().unwrap();
        assert_eq!(payload["identifier"], "dbt:a");
        assert_eq!(payload["aliasIds"], json!(["dbt:a"]));
    }

    #[test]
    fn identifier_falls_back_to_alias_ids_for_servers_that_drop_unknown_fields() {
        let e =
            element(json!({"@id": "1", "@type": "PartUsage", "aliasIds": ["dbt:a"], "name": "A"}));
        assert_eq!(element_identifier(&e), Some("dbt:a"));
        let both = element(
            json!({"@id": "1", "@type": "PartUsage", "identifier": "dbt:x", "aliasIds": ["dbt:a"]}),
        );
        assert_eq!(element_identifier(&both), Some("dbt:x"));
    }

    /// The OMG pilot returns only the fields its schema knows. Re-syncing identical data must not commit.
    #[test]
    fn a_strictly_typed_server_that_drops_custom_fields_does_not_cause_a_commit_every_run() {
        let stored =
            element(json!({"@id": "1", "@type": "PartUsage", "aliasIds": ["dbt:a"], "name": "A"}));
        let changes = diff_managed(
            std::slice::from_ref(&stored),
            &[desired("dbt:a", "A", json!(1))],
        )
        .unwrap();
        assert!(changes.is_empty(), "{changes:?}");
        // ...but a change to a field it *does* return still syncs.
        let changes = diff_managed(
            std::slice::from_ref(&stored),
            &[desired("dbt:a", "Renamed", json!(1))],
        )
        .unwrap();
        assert_eq!(changes.len(), 1);
    }

    /// A server that keeps `identifier` keeps custom fields too, so an absent owned field there is a real difference.
    #[test]
    fn a_server_that_keeps_identifier_treats_a_missing_owned_field_as_a_change() {
        let stored =
            element(json!({"@id": "1", "@type": "PartUsage", "identifier": "dbt:a", "name": "A"}));
        let changes = diff_managed(
            std::slice::from_ref(&stored),
            &[desired("dbt:a", "A", json!(1))],
        )
        .unwrap();
        assert_eq!(changes.len(), 1);
    }
}
