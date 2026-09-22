//! Env-gated integration test against a real OMG Systems Modeling API server.
//!
//! Skipped by default. Run explicitly:
//!
//! ```sh
//! KR0KI_SYSMLV2_BASE_URL=https://sysml2.example.com \
//! KR0KI_SYSMLV2_TOKEN=... \
//! cargo test -p kr0ki-sysmlv2-client --test live -- --ignored --nocapture
//! ```
//!
//! No network traffic occurs in a normal `cargo test` run.

use kr0ki_sysmlv2_client::SysmlV2Client;

#[tokio::test]
#[ignore = "requires KR0KI_SYSMLV2_BASE_URL (+ optional KR0KI_SYSMLV2_TOKEN) and network"]
async fn lists_projects_and_snapshots_first_commit() {
    let base = std::env::var("KR0KI_SYSMLV2_BASE_URL")
        .expect("set KR0KI_SYSMLV2_BASE_URL to run this integration test");

    let mut client = SysmlV2Client::new(base);
    if let Ok(token) = std::env::var("KR0KI_SYSMLV2_TOKEN") {
        client = client.with_token(token);
    }

    let projects = client.projects().await.expect("list projects");
    eprintln!("found {} project(s)", projects.len());

    let Some(project) = projects.first() else {
        eprintln!("server has no projects; nothing to snapshot");
        return;
    };

    let commits = client.commits(&project.at_id).await.expect("list commits");
    let Some(commit) = commits.first() else {
        eprintln!("project {} has no commits", project.at_id);
        return;
    };

    let snap = client
        .snapshot(&project.at_id, &commit.at_id)
        .await
        .expect("snapshot first commit of first project");

    eprintln!(
        "snapshot {}@{}: {} elements, {} roots, content_hash={}",
        snap.project_id,
        snap.commit_id,
        snap.elements.len(),
        snap.roots.len(),
        snap.content_hash
    );
    assert_eq!(snap.content_hash.len(), 64);
}

#[tokio::test]
#[ignore = "requires KR0KI_SYSMLV2_BASE_URL pointing at a running OMG Java pilot \
            (Systems-Modeling/SysML-v2-API-Services) with commit-changes support -- \
            NOT Flexo, whose commit-changes endpoint is stubbed server-side. \
            Setup: clone the pilot repo, `docker run --name sysml2-postgres -p 5432:5432 \
            -e POSTGRES_PASSWORD=... -e POSTGRES_DB=sysml2 -d postgres`, then `sbt run` \
            (JDK 11 + sbt) per that repo's own README. Requires an existing project to \
            target -- set KR0KI_SYSMLV2_TEST_PROJECT_ID."]
async fn creates_a_commit_with_one_new_element() {
    let base = std::env::var("KR0KI_SYSMLV2_BASE_URL")
        .expect("set KR0KI_SYSMLV2_BASE_URL to run this integration test");
    let project_id = std::env::var("KR0KI_SYSMLV2_TEST_PROJECT_ID")
        .expect("set KR0KI_SYSMLV2_TEST_PROJECT_ID to an existing project's id");

    let mut client = SysmlV2Client::new(base);
    if let Ok(token) = std::env::var("KR0KI_SYSMLV2_TOKEN") {
        client = client.with_token(token);
    }

    let commits = client.commits(&project_id).await.expect("list commits");
    let previous_commit = commits.first().map(|c| kr0ki_sysmlv2_client::Ref {
        at_id: c.at_id.clone(),
        extra: Default::default(),
    });

    let request = kr0ki_sysmlv2_client::CommitRequest {
        type_: "Commit",
        change: vec![kr0ki_sysmlv2_client::DataVersion {
            type_: "DataVersion",
            payload: Some(serde_json::json!({
                "@type": "PartUsage",
                "name": "kr0ki live-test element",
                "identifier": "dbt:live-test-marker"
            })),
            identity: None,
        }],
        previous_commit,
    };

    let commit = client
        .create_commit(&project_id, None, request)
        .await
        .expect("create commit");
    eprintln!("created commit {}", commit.at_id);
}
