//! Wiremock-backed integration test for sync_dbt_graph's full fetch -> diff -> POST
//! sequence. See crates/kr0ki-core/src/digital_thread_sync.rs's own unit tests for
//! the diff logic itself -- this test is about orchestration, not diff correctness.

use kr0ki_core::digital_thread_sync::{sync_dbt_graph, SyncConfig, SyncError, MAX_SYNC_ATTEMPTS};
use kr0ki_sysmlv2_client::SysmlV2Client;
use serde_json::json;
use ufo_types::stereotype::UfoStereotype;
use ufo_types::sysgraph::{OntologicalNode, SysGraph};
use ufo_types::sysml_model::ElementId;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn syncs_a_new_node_as_a_create_commit() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/projects/p1/commits"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"@id": "c1", "@type": "Commit"}
        ])))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/projects/p1/commits/c1/elements"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/projects/p1/commits"))
        .and(body_partial_json(json!({
            "previousCommit": {"@id": "c1"},
            "change": [{"@type": "DataVersion"}]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"@id": "c2", "@type": "Commit"})),
        )
        .mount(&server)
        .await;

    let client = SysmlV2Client::new(server.uri());
    let mut graph = SysGraph::new();
    graph.push_node(OntologicalNode::with_label(
        ElementId::new("dbt:model.a"),
        UfoStereotype::Kind("DbtModel".into()),
        "A (marts)",
    ));
    let config = SyncConfig {
        project_id: "p1".to_string(),
        branch_id: None,
    };

    let commit = sync_dbt_graph(&client, &graph, &config)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(commit.at_id, "c2");
}

#[tokio::test]
async fn empty_changeset_skips_the_post_and_returns_the_latest_commit() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/projects/p1/commits"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"@id": "c1", "@type": "Commit"}
        ])))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/projects/p1/commits/c1/elements"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"@id": "srv-1", "@type": "PartUsage", "identifier": "dbt:model.a", "name": "A (marts)"}
        ])))
        .mount(&server)
        .await;
    // Deliberately no POST mock registered -- if sync_dbt_graph posts anyway, this
    // test fails with a connection/match error from wiremock, proving the skip.

    let client = SysmlV2Client::new(server.uri());
    let mut graph = SysGraph::new();
    graph.push_node(OntologicalNode::with_label(
        ElementId::new("dbt:model.a"),
        UfoStereotype::Kind("DbtModel".into()),
        "A (marts)",
    ));
    let config = SyncConfig {
        project_id: "p1".to_string(),
        branch_id: None,
    };

    let commit = sync_dbt_graph(&client, &graph, &config)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        commit.at_id, "c1",
        "should return the existing latest commit unchanged"
    );
}

#[tokio::test]
async fn empty_project_with_empty_graph_skips_the_post_and_returns_none() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/projects/p1/commits"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&server)
        .await;
    // No prior commit, so sync_dbt_graph must not call GET .../elements either --
    // deliberately no mock for it, and no POST mock: if the code posts an empty
    // first commit anyway, this test fails with a connection/match error from
    // wiremock, proving the "never post an empty commit" invariant.

    let client = SysmlV2Client::new(server.uri());
    let graph = SysGraph::new();
    let config = SyncConfig {
        project_id: "p1".to_string(),
        branch_id: None,
    };

    let commit = sync_dbt_graph(&client, &graph, &config).await.unwrap();

    assert!(
        commit.is_none(),
        "nothing to sync and no prior commit exists -- should return None"
    );
}

// ---- head resolution, staleness, and conflict handling -------------------------------

async fn get_json(server: &MockServer, p: &str, body: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path(p))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

fn one_dbt_node_graph() -> SysGraph {
    let mut graph = SysGraph::new();
    graph.push_node(OntologicalNode::with_label(
        ElementId::new("dbt:model.a"),
        UfoStereotype::Kind("DbtModel".into()),
        "A (marts)",
    ));
    graph
}

fn config() -> SyncConfig {
    SyncConfig {
        project_id: "p1".to_string(),
        branch_id: None,
    }
}

async fn post_ok(server: &MockServer, previous: &str) {
    Mock::given(method("POST"))
        .and(path("/projects/p1/commits"))
        .and(body_partial_json(
            json!({"previousCommit": {"@id": previous}}),
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"@id": "new", "@type": "Commit"})),
        )
        .mount(server)
        .await;
}

async fn post_count(server: &MockServer) -> usize {
    server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .filter(|r| r.method.as_str() == "POST")
        .count()
}

#[tokio::test]
async fn diffs_against_the_default_branch_head_not_the_first_listed_commit() {
    let server = MockServer::start().await;
    // The list leads with a stale commit; the branch head is the real latest.
    get_json(
        &server,
        "/projects/p1/commits",
        json!([{"@id": "c-stale"}, {"@id": "c-head"}]),
    )
    .await;
    get_json(
        &server,
        "/projects/p1",
        json!({"@id": "p1", "defaultBranch": {"@id": "b1"}}),
    )
    .await;
    get_json(
        &server,
        "/projects/p1/branches/b1",
        json!({"@id": "b1", "head": {"@id": "c-head"}}),
    )
    .await;
    get_json(
        &server,
        "/projects/p1/commits/c-head",
        json!({"@id": "c-head", "@type": "Commit"}),
    )
    .await;
    get_json(&server, "/projects/p1/commits/c-head/elements", json!([])).await;
    // No elements mock for c-stale: reading it would 404 and fail the sync.
    post_ok(&server, "c-head").await;

    let commit = sync_dbt_graph(
        &SysmlV2Client::new(server.uri()),
        &one_dbt_node_graph(),
        &config(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(commit.at_id, "new");
}

#[tokio::test]
async fn without_branch_info_the_newest_commit_by_created_wins_over_list_order() {
    let server = MockServer::start().await;
    get_json(
        &server,
        "/projects/p1/commits",
        json!([
            {"@id": "c-new", "created": "2026-03-01T00:00:00Z"},
            {"@id": "c-old", "created": "2026-01-01T00:00:00Z"}
        ]),
    )
    .await;
    // Ascending-list servers would put c-new last; either way created decides.
    get_json(&server, "/projects/p1/commits/c-new/elements", json!([])).await;
    post_ok(&server, "c-new").await;

    let commit = sync_dbt_graph(
        &SysmlV2Client::new(server.uri()),
        &one_dbt_node_graph(),
        &config(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(commit.at_id, "new");
}

#[tokio::test]
async fn a_head_that_moves_during_the_diff_is_re_read_before_committing() {
    let server = MockServer::start().await;
    get_json(
        &server,
        "/projects/p1",
        json!({"@id": "p1", "defaultBranch": {"@id": "b1"}}),
    )
    .await;
    // First read of the branch sees c1; every later read sees c2 (another writer committed).
    Mock::given(method("GET"))
        .and(path("/projects/p1/branches/b1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"@id": "b1", "head": {"@id": "c1"}})),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;
    get_json(
        &server,
        "/projects/p1/branches/b1",
        json!({"@id": "b1", "head": {"@id": "c2"}}),
    )
    .await;
    for c in ["c1", "c2"] {
        get_json(
            &server,
            &format!("/projects/p1/commits/{c}"),
            json!({"@id": c, "@type": "Commit"}),
        )
        .await;
        get_json(
            &server,
            &format!("/projects/p1/commits/{c}/elements"),
            json!([]),
        )
        .await;
    }
    // Only a POST built against c2 is accepted; a stale c1 POST would 404 and fail.
    post_ok(&server, "c2").await;

    let commit = sync_dbt_graph(
        &SysmlV2Client::new(server.uri()),
        &one_dbt_node_graph(),
        &config(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(commit.at_id, "new");
    assert_eq!(
        post_count(&server).await,
        1,
        "the stale attempt must not have posted"
    );
}

#[tokio::test]
async fn a_conflict_response_is_retried_with_a_fresh_read() {
    let server = MockServer::start().await;
    get_json(&server, "/projects/p1/commits", json!([{"@id": "c1"}])).await;
    get_json(&server, "/projects/p1/commits/c1/elements", json!([])).await;
    Mock::given(method("POST"))
        .and(path("/projects/p1/commits"))
        .respond_with(ResponseTemplate::new(409))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    post_ok(&server, "c1").await;

    let commit = sync_dbt_graph(
        &SysmlV2Client::new(server.uri()),
        &one_dbt_node_graph(),
        &config(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(commit.at_id, "new");
    assert_eq!(
        post_count(&server).await,
        2,
        "one rejected POST, one accepted"
    );
}

#[tokio::test]
async fn persistent_conflict_gives_up_after_the_attempt_budget() {
    let server = MockServer::start().await;
    get_json(&server, "/projects/p1/commits", json!([{"@id": "c1"}])).await;
    get_json(&server, "/projects/p1/commits/c1/elements", json!([])).await;
    Mock::given(method("POST"))
        .and(path("/projects/p1/commits"))
        .respond_with(ResponseTemplate::new(412))
        .mount(&server)
        .await;

    let err = sync_dbt_graph(
        &SysmlV2Client::new(server.uri()),
        &one_dbt_node_graph(),
        &config(),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, SyncError::Conflict { attempts } if attempts == MAX_SYNC_ATTEMPTS),
        "{err}"
    );
    assert_eq!(post_count(&server).await, MAX_SYNC_ATTEMPTS);
}

#[tokio::test]
async fn non_conflict_server_errors_are_not_retried() {
    let server = MockServer::start().await;
    get_json(&server, "/projects/p1/commits", json!([{"@id": "c1"}])).await;
    get_json(&server, "/projects/p1/commits/c1/elements", json!([])).await;
    Mock::given(method("POST"))
        .and(path("/projects/p1/commits"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let err = sync_dbt_graph(
        &SysmlV2Client::new(server.uri()),
        &one_dbt_node_graph(),
        &config(),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, SyncError::Client(_)), "{err}");
    assert_eq!(post_count(&server).await, 1);
}

#[tokio::test]
async fn duplicate_server_identifiers_abort_before_any_post() {
    let server = MockServer::start().await;
    get_json(&server, "/projects/p1/commits", json!([{"@id": "c1"}])).await;
    get_json(
        &server,
        "/projects/p1/commits/c1/elements",
        json!([
            {"@id": "s1", "@type": "PartUsage", "identifier": "dbt:model.a", "name": "A"},
            {"@id": "s2", "@type": "PartUsage", "identifier": "dbt:model.a", "name": "A"}
        ]),
    )
    .await;

    let err = sync_dbt_graph(
        &SysmlV2Client::new(server.uri()),
        &one_dbt_node_graph(),
        &config(),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, SyncError::DuplicateIdentifier(_)), "{err}");
    assert_eq!(post_count(&server).await, 0);
}
