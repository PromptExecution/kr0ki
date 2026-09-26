//! Live acceptance test for the requirements-rules system (spec §1 item 6):
//! author real RuleDocument elements in a real project, recompute, assert
//! violations point at the right elements. `#[ignore]`d -- requires a live
//! OMG-API server, matching `crates/kr0ki-sysmlv2-client/tests/live.rs`'s
//! own gating convention exactly.

use kr0ki_core::recompute::recompute_and_evaluate;
use kr0ki_sysmlv2_client::{CommitRequest, DataVersion, SysmlV2Client};

fn live_client() -> Option<SysmlV2Client> {
    let base_url = std::env::var("KR0KI_SYSMLV2_BASE_URL").ok()?;
    let mut client = SysmlV2Client::new(base_url);
    if let Ok(token) = std::env::var("KR0KI_SYSMLV2_TOKEN") {
        client = client.with_token(token);
    }
    Some(client)
}

#[tokio::test]
#[ignore]
async fn recompute_flags_a_real_violation_on_a_live_project() {
    let Some(client) = live_client() else {
        eprintln!("KR0KI_SYSMLV2_BASE_URL not set, skipping live test");
        return;
    };

    let projects = client.projects().await.expect("list projects");
    let project = projects
        .first()
        .expect("at least one project on the live server");
    let project_id = project.at_id.clone();
    let commits = client.commits(&project_id).await.expect("list commits");
    let previous_commit = commits.first().map(|c| kr0ki_sysmlv2_client::Ref {
        at_id: c.at_id.clone(),
        extra: Default::default(),
    });

    // Author two rule docs directly (Playb00k UX is deferred -- this test
    // seeds them itself, exactly the write path that separate,
    // already-merged plan's own Task 1-9 (`docs/superpowers/plans/2026-09-20-flexo-write-path.md`)
    // already shipped for).
    let change = vec![
        DataVersion {
            type_: "DataVersion",
            payload: Some(serde_json::json!({
                "@type": "RuleDocument",
                "name": "No untitled parts",
                "rego": "package kr0ki\n\nviolations := [v |\n    some n\n    not input.nodes[n].label\n    v := {\"element_id\": input.nodes[n].id, \"reason\": \"element has no name\"}\n]\n"
            })),
            identity: None,
        },
        DataVersion {
            type_: "DataVersion",
            payload: Some(serde_json::json!({
                "@type": "RuleDocument",
                "name": "Always passes",
                "rego": "package kr0ki\n\nviolations := []\n"
            })),
            identity: None,
        },
    ];
    client
        .create_commit(
            &project_id,
            None,
            CommitRequest {
                type_: "Commit",
                change,
                previous_commit,
            },
        )
        .await
        .expect("seed rule docs");

    let result = recompute_and_evaluate(&client, &project_id)
        .await
        .expect("recompute");

    assert!(
        result.violations.len() >= 2,
        "expected at least the 2 freshly-seeded rule docs to be found and evaluated \
         (found {} — if this is 0, extract_rule_docs likely isn't matching the \
         server's actual element shape for these two)",
        result.violations.len()
    );
    assert!(
        result.violations.iter().any(|v| v.result.is_satisfied()),
        "expected the always-pass rule doc to evaluate as satisfied"
    );

    if let Some(violated) = result.violations.iter().find(|v| v.result.is_violated()) {
        let relation = result
            .requirements
            .relations
            .iter()
            .find(|r| r.source == violated.rule_id)
            .expect("a relation sourced from the violated rule doc");
        assert!(relation.target.starts_with("node:"));
    }
}
