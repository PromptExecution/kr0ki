//! KR-A05 against a real OMG Systems Modeling API server (the Java pilot).
//!
//! Skipped by default. `just sysml-api-up`, then:
//!
//! ```sh
//! KR0KI_SYSMLV2_BASE_URL=http://127.0.0.1:9000 \
//!   cargo test -p kr0ki-core --test assurance_a05_live -- --ignored --nocapture
//! ```
//!
//! Creates a throwaway project (as `just test-live-sysml` does). Records, without asserting,
//! whether the server itself enforces `previousCommit`: the service's own check is the
//! guarantee either way.

use std::sync::Arc;

use kr0ki_core::change_service::{ChangeError, ChangeService, SysmlBackend};
use kr0ki_core::sync_engine::DesiredElement;
use kr0ki_sysmlv2_client::{CommitRequest, DataVersion, Ref, SysmlV2Client};
use serde_json::{json, Map, Value};

const PREFIX: &str = "kr0ki:a05live:";

fn desired(names: &[(&str, &str)]) -> Vec<DesiredElement> {
    names
        .iter()
        .map(|(id, name)| {
            let mut fields = Map::new();
            fields.insert("name".into(), json!(name));
            DesiredElement {
                identifier: format!("{PREFIX}{id}"),
                type_: "PartUsage",
                fields,
            }
        })
        .collect()
}

/// identifier (from `aliasIds`, which the strictly-typed pilot keeps) -> name at `commit`.
async fn names_at(
    client: &SysmlV2Client,
    project: &str,
    commit: &str,
) -> std::collections::BTreeMap<String, String> {
    client
        .all_elements(project, commit)
        .await
        .expect("elements")
        .into_iter()
        .filter_map(|e| {
            let id = e
                .get("aliasIds")?
                .as_array()?
                .first()?
                .as_str()?
                .to_string();
            id.starts_with(PREFIX)
                .then(|| (id, e.name().unwrap_or_default().to_string()))
        })
        .collect()
}

#[tokio::test]
#[ignore = "needs KR0KI_SYSMLV2_BASE_URL pointing at the OMG pilot (just sysml-api-up)"]
async fn live_two_edits_from_one_baseline_second_is_rejected_first_is_kept() {
    let base = std::env::var("KR0KI_SYSMLV2_BASE_URL").expect("set KR0KI_SYSMLV2_BASE_URL");
    let created: Value = reqwest::Client::new()
        .post(format!("{}/projects", base.trim_end_matches('/')))
        .json(
            &json!({"@type": "Project", "name": format!("kr0ki a05 live {}", std::process::id())}),
        )
        .send()
        .await
        .expect("create project")
        .error_for_status()
        .expect("project created")
        .json()
        .await
        .unwrap();
    let project = created["@id"].as_str().expect("project id").to_string();

    let client = Arc::new(SysmlV2Client::new(base));
    let svc = ChangeService::new(SysmlBackend::new(client.clone()));

    let seed = svc
        .propose(
            &project,
            None,
            PREFIX,
            &desired(&[("a", "one"), ("b", "two")]),
        )
        .await
        .unwrap();
    assert_eq!(seed.base_revision, None, "a new project has no commits");
    let r0 = svc
        .commit(&seed.change_id, None)
        .await
        .expect("seed commit")
        .commit_id;

    let e1 = svc
        .propose(
            &project,
            None,
            PREFIX,
            &desired(&[("a", "one (edit 1)"), ("b", "two")]),
        )
        .await
        .unwrap();
    let e2 = svc
        .propose(
            &project,
            None,
            PREFIX,
            &desired(&[("a", "one"), ("b", "two (edit 2)")]),
        )
        .await
        .unwrap();
    assert_eq!(e1.base_revision.as_deref(), Some(r0.as_str()));
    assert_eq!(e2.base_revision.as_deref(), Some(r0.as_str()));

    let r1 = svc
        .commit(&e1.change_id, Some(&r0))
        .await
        .expect("first edit")
        .commit_id;
    let err = svc.commit(&e2.change_id, Some(&r0)).await.unwrap_err();
    assert_eq!(
        err,
        ChangeError::StaleBase {
            expected: Some(r0.clone()),
            current: Some(r1.clone())
        }
    );

    // Read it back from the real server: first edit kept, second absent, nothing lost.
    let names = names_at(&client, &project, &r1).await;
    assert_eq!(names[&format!("{PREFIX}a")], "one (edit 1)");
    assert_eq!(names[&format!("{PREFIX}b")], "two");
    assert_eq!(
        client.commits(&project).await.unwrap().len(),
        2,
        "seed + first edit only"
    );

    // Observation, not an assertion: does the pilot itself reject a stale `previousCommit`?
    let stale_direct = client
        .create_commit(
            &project,
            None,
            CommitRequest {
                type_: "Commit",
                change: vec![DataVersion {
                    type_: "DataVersion",
                    payload: Some(json!({"@type": "PartUsage", "name": "direct stale", "aliasIds": [format!("{PREFIX}c")]})),
                    identity: None,
                }],
                previous_commit: Some(Ref { at_id: r0.clone(), extra: Default::default() }),
            },
        )
        .await;
    eprintln!(
        "OBSERVED: pilot server on a stale previousCommit -> {}",
        match &stale_direct {
            Ok(_) => "ACCEPTED (the server does not enforce previousCommit; the service's own check is the guard)".to_string(),
            Err(e) => format!("REJECTED ({e})"),
        }
    );
}
