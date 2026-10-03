//! KR-A05 acceptance case (VC-A05).
//!
//! "The model commit service shall reject a proposed mutation whose expected base revision
//! differs from the current revision."
//!
//! Acceptance: submit two edits from one baseline; reject the second stale edit without losing
//! the first.
//!
//! The model content is this repository's own requirement baseline. An in-memory model server
//! stands in for the real one here; `assurance_a05_live.rs` runs the same scenario against the
//! OMG pilot server.

use std::collections::BTreeMap;

use kr0ki_core::assurance_baseline::load_baseline;
use kr0ki_core::change_service::in_memory::InMemoryServer;
use kr0ki_core::change_service::{ChangeError, ChangeService, Operation};
use kr0ki_core::sync_engine::DesiredElement;
use serde_json::{json, Map};
use ufo_types::mbse::assurance::NODE_KIND_ATTR;

const BASELINE_TOML: &str = include_str!("../../../docs/assurance/kr0ki.assurance.toml");
const PREFIX: &str = "kr0ki:assurance:";

/// The baseline's requirements as managed model elements, with title overrides applied.
fn desired(overrides: &BTreeMap<&str, &str>) -> Vec<DesiredElement> {
    let graph = load_baseline(
        BASELINE_TOML,
        "docs/assurance/kr0ki.assurance.toml",
        "rev-1",
        None,
    )
    .expect("the checked-in baseline must load")
    .graph;
    graph
        .requirements
        .iter()
        .filter(|n| n.attributes.get(NODE_KIND_ATTR).map(String::as_str) == Some("requirement"))
        .map(|n| {
            let mut fields = Map::new();
            let title = overrides.get(n.id.as_str()).copied().unwrap_or(&n.title);
            fields.insert("name".into(), json!(title));
            fields.insert("text".into(), json!([n.text]));
            DesiredElement {
                identifier: format!("{PREFIX}{}", n.id),
                type_: "RequirementUsage",
                fields,
            }
        })
        .collect()
}

fn names_at_head(server: &InMemoryServer) -> BTreeMap<String, String> {
    // identifier -> name, read back from the server's head commit.
    server.names_at_head()
}

#[tokio::test]
async fn two_edits_from_one_baseline_the_second_stale_edit_is_rejected_and_the_first_is_kept() {
    let svc = ChangeService::new(InMemoryServer::new(true));

    // The baseline: all eight requirements committed.
    let seed = svc
        .propose("p", None, PREFIX, &desired(&BTreeMap::new()))
        .await
        .unwrap();
    assert_eq!(seed.base_revision, None);
    assert_eq!(seed.summary.len(), 8);
    assert!(seed
        .summary
        .iter()
        .all(|s| s.operation == Operation::Create));
    let r0 = svc.commit(&seed.change_id, None).await.unwrap().commit_id;

    // Two edits, both proposed from the same baseline r0.
    let edit1 = svc
        .propose(
            "p",
            None,
            PREFIX,
            &desired(&BTreeMap::from([("KR-A01", "KR-A01 (edited first)")])),
        )
        .await
        .unwrap();
    let edit2 = svc
        .propose(
            "p",
            None,
            PREFIX,
            &desired(&BTreeMap::from([("KR-A02", "KR-A02 (edited second)")])),
        )
        .await
        .unwrap();
    assert_eq!(edit1.base_revision.as_deref(), Some(r0.as_str()));
    assert_eq!(edit2.base_revision, edit1.base_revision);
    assert_eq!(edit1.summary.len(), 1);
    assert_eq!(edit1.summary[0].identifier, format!("{PREFIX}KR-A01"));
    assert_eq!(edit2.summary[0].identifier, format!("{PREFIX}KR-A02"));

    // The first lands.
    let r1 = svc
        .commit(&edit1.change_id, Some(&r0))
        .await
        .unwrap()
        .commit_id;
    let commits_after_first = svc.backend().commit_count();

    // The second, still expecting r0, is rejected: it names both revisions.
    let err = svc.commit(&edit2.change_id, Some(&r0)).await.unwrap_err();
    assert_eq!(
        err,
        ChangeError::StaleBase {
            expected: Some(r0.clone()),
            current: Some(r1.clone())
        }
    );
    assert!(err.to_string().contains("nothing was written"));

    // Nothing was lost and nothing was added.
    assert_eq!(svc.backend().commit_count(), commits_after_first);
    assert_eq!(svc.backend().head_id().as_deref(), Some(r1.as_str()));
    let names = names_at_head(svc.backend());
    assert_eq!(
        names[&format!("{PREFIX}KR-A01")],
        "KR-A01 (edited first)",
        "the first edit is kept"
    );
    assert_ne!(
        names[&format!("{PREFIX}KR-A02")],
        "KR-A02 (edited second)",
        "the stale edit is not applied"
    );
    assert_eq!(names.len(), 8, "no requirement was lost");

    // Re-proposing from the new head keeps both, and commits.
    let redo = svc
        .propose(
            "p",
            None,
            PREFIX,
            &desired(&BTreeMap::from([
                ("KR-A01", "KR-A01 (edited first)"),
                ("KR-A02", "KR-A02 (edited second)"),
            ])),
        )
        .await
        .unwrap();
    assert_eq!(redo.base_revision.as_deref(), Some(r1.as_str()));
    svc.commit(&redo.change_id, Some(&r1)).await.unwrap();
    let names = names_at_head(svc.backend());
    assert_eq!(names[&format!("{PREFIX}KR-A01")], "KR-A01 (edited first)");
    assert_eq!(names[&format!("{PREFIX}KR-A02")], "KR-A02 (edited second)");
}

#[tokio::test]
async fn the_rejection_does_not_depend_on_the_server_checking_previous_commit() {
    // A server that ignores `previousCommit` would silently overwrite; the service must not.
    let svc = ChangeService::new(InMemoryServer::new(false));
    let seed = svc
        .propose("p", None, PREFIX, &desired(&BTreeMap::new()))
        .await
        .unwrap();
    let r0 = svc.commit(&seed.change_id, None).await.unwrap().commit_id;
    let e1 = svc
        .propose(
            "p",
            None,
            PREFIX,
            &desired(&BTreeMap::from([("KR-A01", "one")])),
        )
        .await
        .unwrap();
    let e2 = svc
        .propose(
            "p",
            None,
            PREFIX,
            &desired(&BTreeMap::from([("KR-A02", "two")])),
        )
        .await
        .unwrap();
    svc.commit(&e1.change_id, Some(&r0)).await.unwrap();
    assert!(matches!(
        svc.commit(&e2.change_id, Some(&r0)).await,
        Err(ChangeError::StaleBase { .. })
    ));
    assert_eq!(
        names_at_head(svc.backend())[&format!("{PREFIX}KR-A01")],
        "one"
    );
}
