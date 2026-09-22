//! Wiremock-backed integration test for sync_dbt_graph's full fetch -> diff -> POST
//! sequence. See crates/kr0ki-core/src/digital_thread_sync.rs's own unit tests for
//! the diff logic itself -- this test is about orchestration, not diff correctness.

use kr0ki_core::digital_thread_sync::{sync_dbt_graph, SyncConfig};
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
        .and(body_partial_json(json!({"previousCommit": {"@id": "c1"}})))
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

    let commit = sync_dbt_graph(&client, &graph, &config).await.unwrap();

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

    let commit = sync_dbt_graph(&client, &graph, &config).await.unwrap();

    assert_eq!(
        commit.at_id, "c1",
        "should return the existing latest commit unchanged"
    );
}
