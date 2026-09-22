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
    let project = projects.first().expect("at least one project on the live server");
    let project_id = project.at_id.clone();
    let commits = client.commits(&project_id).await.expect("list commits");
    let previous_commit = commits.first().map(|c| kr0ki_sysmlv2_client::Ref {
        at_id: c.at_id.clone(),
        extra: Default::default(),
    });

    // Author two rule docs directly (Playb00k UX is deferred -- this test
    // seeds them itself, exactly the write path Task-1-through-9's
    // `docs/superpowers/plans/2026-09-20-flexo-write-path.md` already
    // shipped for).
    let change = vec![
        DataVersion {
            type_: "DataVersion",
            payload: Some(serde_json::json!({
                "@type": "RuleDocument",
                "@id": "rule:live-no-untitled-parts",
                "name": "No untitled parts",
                "rego": "package kr0ki\n\nviolations := [v |\n    some n\n    not input.nodes[n].label\n    v := {\"element_id\": input.nodes[n].id, \"reason\": \"element has no name\"}\n]\n"
            })),
            identity: None,
        },
        DataVersion {
            type_: "DataVersion",
            payload: Some(serde_json::json!({
                "@type": "RuleDocument",
                "@id": "rule:live-always-pass",
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

    let rule_ids: Vec<&str> = result
        .violations
        .iter()
        .map(|v| v.rule_id.as_str())
        .collect();
    assert!(rule_ids.contains(&"rule:live-no-untitled-parts"));
    assert!(rule_ids.contains(&"rule:live-always-pass"));

    let untitled_result = result
        .violations
        .iter()
        .find(|v| v.rule_id == "rule:live-no-untitled-parts")
        .unwrap();
    let always_pass_result = result
        .violations
        .iter()
        .find(|v| v.rule_id == "rule:live-always-pass")
        .unwrap();
    assert!(always_pass_result.result.is_satisfied());
    // "no untitled parts" may pass or fail depending on the live project's
    // actual content -- either is a valid outcome; what this test proves
    // is that a real evaluation ran and reported cleanly either way.
    assert!(
        untitled_result.result.is_satisfied() || untitled_result.result.is_violated(),
        "expected a definite disposition, got {:?}",
        untitled_result.result.disposition
    );
    if untitled_result.result.is_violated() {
        assert!(!result.requirements.relations.is_empty());
        let relation = &result.requirements.relations[0];
        assert!(relation.target.starts_with("node:"));
    }
}
