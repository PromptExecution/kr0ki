//! Behavior of the bounded loop, driven entirely by fakes (PLAN-KR0KI-007 Phase 0 acceptance).

mod common;
use common::*;
use kr0ki_raster_ingest::*;
use std::time::Duration;

fn cfg() -> LoopConfig {
    LoopConfig::new("d2")
}

#[tokio::test]
async fn accepts_when_the_judge_matches_and_labels_are_covered() {
    let proposer = ScriptedModel::new("p")
        .on(Purpose::Describe, vec![describe(&["Client", "API"])])
        .on(Purpose::Propose, vec![source("client -> api")]);
    // The judge tries to report its own label numbers; the loop must overwrite them with the deterministic ones.
    let judge = ScriptedModel::new("j").on(
        Purpose::Judge,
        vec![ok(
            r#"{"match":true,"score":0.95,"label_recall":0.0,"label_precision":0.0}"#,
        )],
    );
    let renderer = FakeRenderer::new(rendered(Some(&["client", "api"])));

    let r = run_loop(&image(), &cfg(), &proposer, &judge, &renderer)
        .await
        .unwrap();

    assert_eq!(
        r.outcome,
        Outcome::Accepted {
            attempt: 1,
            via: AcceptedVia::LabelsAndJudge
        }
    );
    assert_eq!(r.best_source(), Some("client -> api"));
    let v = r.attempts[0].verdict.as_ref().unwrap();
    assert_eq!(
        (v.label_recall, v.label_precision),
        (Some(1.0), Some(1.0)),
        "model-supplied label numbers are never trusted"
    );
    assert_eq!(
        (proposer.calls(), judge.calls(), renderer.calls()),
        (2, 1, 1)
    );
}

#[tokio::test]
async fn a_render_error_is_fed_back_and_costs_no_judge_call() {
    let proposer = ScriptedModel::new("p")
        .on(Purpose::Describe, vec![describe(&["A"])])
        .on(Purpose::Propose, vec![source("a ->"), source("a -> b")]);
    let judge = ScriptedModel::new("j").on(Purpose::Judge, vec![verdict(true, 0.9)]);
    let renderer = FakeRenderer::new(rendered(Some(&["a"])))
        .then(Err(RenderError::Rejected("syntax error on line 3".into())));

    let r = run_loop(&image(), &cfg(), &proposer, &judge, &renderer)
        .await
        .unwrap();

    assert_eq!(
        r.outcome,
        Outcome::Accepted {
            attempt: 2,
            via: AcceptedVia::LabelsAndJudge
        }
    );
    assert!(r.attempts[0]
        .render_error
        .as_deref()
        .unwrap()
        .contains("line 3"));
    assert_eq!(judge.calls(), 1, "the failed render must not be judged");
    let second_prompt = &proposer.prompts_for(Purpose::Propose)[1];
    assert!(
        second_prompt.contains("syntax error on line 3"),
        "{second_prompt}"
    );
}

#[tokio::test]
async fn judge_differences_are_fed_into_the_next_proposal() {
    let proposer = ScriptedModel::new("p")
        .on(Purpose::Describe, vec![describe(&["A"])])
        .on(Purpose::Propose, vec![source("a"), source("a -> cache")]);
    let judge = ScriptedModel::new("j").on(
        Purpose::Judge,
        vec![verdict_with_missing(0.4, "Cache"), verdict(true, 0.95)],
    );
    let renderer = FakeRenderer::new(rendered(Some(&["a"])));

    let r = run_loop(&image(), &cfg(), &proposer, &judge, &renderer)
        .await
        .unwrap();

    assert_eq!(
        r.outcome,
        Outcome::Accepted {
            attempt: 2,
            via: AcceptedVia::LabelsAndJudge
        }
    );
    assert!(proposer.prompts_for(Purpose::Propose)[1].contains("Missing nodes: Cache"));
    assert!(proposer.prompts_for(Purpose::Propose)[1].contains("The best source so far"));
}

#[tokio::test]
async fn exhausting_the_iterations_returns_the_best_attempt_not_the_last() {
    let scores = [0.2, 0.7, 0.4, 0.5];
    let proposer = ScriptedModel::new("p")
        .on(Purpose::Describe, vec![describe(&["A"])])
        .on(
            Purpose::Propose,
            (0..4).map(|i| source(&format!("v{i}"))).collect(),
        );
    let judge = ScriptedModel::new("j").on(
        Purpose::Judge,
        scores.iter().map(|s| verdict(false, *s)).collect(),
    );
    let renderer = FakeRenderer::new(rendered(None));
    let mut c = cfg();
    c.max_iterations = 4;
    c.stall_window = 10;

    let r = run_loop(&image(), &c, &proposer, &judge, &renderer)
        .await
        .unwrap();

    assert_eq!(
        r.outcome,
        Outcome::Exhausted {
            reason: ExhaustReason::MaxIterations,
            best: Some(2)
        }
    );
    assert_eq!(r.best_source(), Some("v1"));
    assert_eq!(r.attempts.len(), 4);
}

#[tokio::test]
async fn a_repeated_source_stops_the_run_before_it_is_rendered_again() {
    let proposer = ScriptedModel::new("p")
        .on(Purpose::Describe, vec![describe(&["A"])])
        .otherwise(Purpose::Propose, source("same"));
    let judge = ScriptedModel::new("j").otherwise(Purpose::Judge, verdict(false, 0.5));
    let renderer = FakeRenderer::new(rendered(None));

    let r = run_loop(&image(), &cfg(), &proposer, &judge, &renderer)
        .await
        .unwrap();

    assert_eq!(
        r.outcome,
        Outcome::Exhausted {
            reason: ExhaustReason::Stalled,
            best: Some(1)
        }
    );
    assert_eq!(renderer.calls(), 1);
}

#[tokio::test]
async fn a_best_score_that_stops_improving_stalls_the_run() {
    let proposer = ScriptedModel::new("p")
        .on(Purpose::Describe, vec![describe(&["A"])])
        .on(
            Purpose::Propose,
            (0..6).map(|i| source(&format!("v{i}"))).collect(),
        );
    let judge = ScriptedModel::new("j").on(
        Purpose::Judge,
        [0.5, 0.4, 0.3].iter().map(|s| verdict(false, *s)).collect(),
    );
    let renderer = FakeRenderer::new(rendered(None));

    let r = run_loop(&image(), &cfg(), &proposer, &judge, &renderer)
        .await
        .unwrap();

    assert_eq!(
        r.outcome,
        Outcome::Exhausted {
            reason: ExhaustReason::Stalled,
            best: Some(1)
        }
    );
    assert_eq!(
        r.attempts.len(),
        3,
        "stall_window=2 stops after the 3rd attempt"
    );
}

#[tokio::test]
async fn a_run_that_never_gets_past_the_renderer_is_still_bounded() {
    let proposer = ScriptedModel::new("p")
        .on(Purpose::Describe, vec![describe(&["A"])])
        .on(
            Purpose::Propose,
            (0..10).map(|i| source(&format!("bad{i}"))).collect(),
        );
    let judge = ScriptedModel::new("j");
    let renderer = FakeRenderer::new(Err(RenderError::Rejected("nope".into())));

    let r = run_loop(&image(), &cfg(), &proposer, &judge, &renderer)
        .await
        .unwrap();

    assert_eq!(
        r.outcome,
        Outcome::Exhausted {
            reason: ExhaustReason::MaxIterations,
            best: None
        }
    );
    assert_eq!((r.attempts.len(), judge.calls()), (6, 0));
    assert_eq!(r.best_source(), None);
}

#[tokio::test]
async fn the_token_budget_is_enforced_between_calls() {
    let proposer = ScriptedModel::new("p")
        .tokens_per_call(100)
        .on(Purpose::Describe, vec![describe(&["A"])])
        .on(
            Purpose::Propose,
            (0..9).map(|i| source(&format!("v{i}"))).collect(),
        );
    let judge = ScriptedModel::new("j").tokens_per_call(100).on(
        Purpose::Judge,
        vec![
            verdict(false, 0.1),
            verdict(false, 0.2),
            verdict(false, 0.3),
        ],
    );
    let renderer = FakeRenderer::new(rendered(None));
    let mut c = cfg();
    c.max_tokens = 450;
    c.stall_window = 10;

    let r = run_loop(&image(), &c, &proposer, &judge, &renderer)
        .await
        .unwrap();

    // describe 100, attempt1 propose+judge 200 -> 300, attempt2 propose+judge 200 -> 500 >= 450: stop before attempt 3.
    assert_eq!(
        r.outcome,
        Outcome::Exhausted {
            reason: ExhaustReason::TokenBudget,
            best: Some(2)
        }
    );
    assert_eq!(r.attempts.len(), 2);
    assert_eq!(r.usage.total(), 500);
}

#[tokio::test(start_paused = true)]
async fn the_wall_clock_budget_ends_the_run() {
    let proposer = ScriptedModel::new("p")
        .delay(Duration::from_secs(40))
        .on(Purpose::Describe, vec![describe(&["A"])])
        .on(Purpose::Propose, vec![source("a")]);
    let judge = ScriptedModel::new("j")
        .delay(Duration::from_secs(40))
        .otherwise(Purpose::Judge, verdict(false, 0.5));
    let renderer = FakeRenderer::new(rendered(None));
    let mut c = cfg();
    c.max_wall = Duration::from_secs(100);

    let r = run_loop(&image(), &c, &proposer, &judge, &renderer)
        .await
        .unwrap();

    // describe 0-40s, propose 40-80s, judge clipped to the 20s left and times out; the next check sees 100s.
    assert_eq!(
        r.outcome,
        Outcome::Exhausted {
            reason: ExhaustReason::WallClock,
            best: Some(1)
        }
    );
    assert!(r.attempts[0]
        .model_error
        .as_deref()
        .unwrap()
        .contains("timed out"));
}

#[tokio::test(start_paused = true)]
async fn each_call_has_its_own_timeout() {
    let proposer = ScriptedModel::new("p")
        .delay(Duration::from_secs(200))
        .on(Purpose::Describe, vec![describe(&["A"])]);
    let r = run_loop(
        &image(),
        &cfg(),
        &proposer,
        &ScriptedModel::new("j"),
        &FakeRenderer::new(rendered(None)),
    )
    .await;
    assert_eq!(r.unwrap_err(), LoopError::Describe(ModelError::Timeout));
}

#[tokio::test]
async fn a_picture_that_is_not_a_diagram_ends_after_one_model_call() {
    let proposer =
        ScriptedModel::new("p").on(Purpose::Describe, vec![ok(r#"{"diagram_kind":"none"}"#)]);
    let judge = ScriptedModel::new("j");
    let renderer = FakeRenderer::new(rendered(None));

    let r = run_loop(&image(), &cfg(), &proposer, &judge, &renderer)
        .await
        .unwrap();

    assert_eq!(r.outcome, Outcome::NotADiagram);
    assert_eq!(
        (proposer.calls(), judge.calls(), renderer.calls()),
        (1, 0, 0)
    );
    assert!(r.attempts.is_empty());
}

#[tokio::test]
async fn an_unparsable_description_is_an_error_not_a_loop() {
    let proposer =
        ScriptedModel::new("p").on(Purpose::Describe, vec![ok("I think it's a flowchart!")]);
    let r = run_loop(
        &image(),
        &cfg(),
        &proposer,
        &ScriptedModel::new("j"),
        &FakeRenderer::new(rendered(None)),
    )
    .await;
    assert!(matches!(r.unwrap_err(), LoopError::Description(_)));
    assert_eq!(proposer.calls(), 1);
}

#[tokio::test]
async fn consecutive_model_failures_stop_the_run_and_keep_the_best_attempt() {
    let proposer = ScriptedModel::new("p")
        .on(Purpose::Describe, vec![describe(&["A"])])
        .on(Purpose::Propose, vec![source("v1")])
        .otherwise(
            Purpose::Propose,
            Err(ModelError::Transport("connection reset".into())),
        );
    let judge = ScriptedModel::new("j").on(Purpose::Judge, vec![verdict(false, 0.6)]);
    let renderer = FakeRenderer::new(rendered(None));

    let r = run_loop(&image(), &cfg(), &proposer, &judge, &renderer)
        .await
        .unwrap();

    assert_eq!(
        r.outcome,
        Outcome::Exhausted {
            reason: ExhaustReason::ModelErrors,
            best: Some(1)
        }
    );
    assert_eq!(r.attempts.len(), 4, "1 good attempt + 3 failed proposals");
    assert!(r.attempts[3]
        .model_error
        .as_deref()
        .unwrap()
        .contains("connection reset"));
}

#[tokio::test]
async fn an_unparsable_judge_reply_is_recorded_and_bounded_never_a_panic() {
    let proposer = ScriptedModel::new("p")
        .on(Purpose::Describe, vec![describe(&["A"])])
        .on(
            Purpose::Propose,
            (0..5).map(|i| source(&format!("v{i}"))).collect(),
        );
    let judge = ScriptedModel::new("j").otherwise(Purpose::Judge, ok("looks good to me!"));
    let renderer = FakeRenderer::new(rendered(Some(&["a"])));

    let r = run_loop(&image(), &cfg(), &proposer, &judge, &renderer)
        .await
        .unwrap();

    assert_eq!(
        r.outcome,
        Outcome::Exhausted {
            reason: ExhaustReason::ModelErrors,
            best: Some(1)
        }
    );
    assert_eq!(r.attempts.len(), 3);
    assert!(r
        .attempts
        .iter()
        .all(|a| a.model_error.is_some() && a.verdict.is_none()));
}

#[tokio::test]
async fn a_judge_verdict_with_an_out_of_range_score_counts_as_a_model_error() {
    let proposer = ScriptedModel::new("p")
        .on(Purpose::Describe, vec![describe(&["A"])])
        .on(Purpose::Propose, vec![source("a")]);
    let judge =
        ScriptedModel::new("j").otherwise(Purpose::Judge, ok(r#"{"match":true,"score":7}"#));
    let r = run_loop(
        &image(),
        &cfg(),
        &proposer,
        &judge,
        &FakeRenderer::new(rendered(Some(&["a"]))),
    )
    .await
    .unwrap();
    assert!(r.attempts[0]
        .model_error
        .as_deref()
        .unwrap()
        .contains("score"));
    assert!(!matches!(r.outcome, Outcome::Accepted { .. }));
}

#[tokio::test]
async fn an_unavailable_renderer_stops_immediately_without_retrying() {
    let proposer = ScriptedModel::new("p")
        .on(Purpose::Describe, vec![describe(&["A"])])
        .on(Purpose::Propose, vec![source("a"), source("b")]);
    let renderer = FakeRenderer::new(Err(RenderError::Unavailable("connection refused".into())));

    let r = run_loop(
        &image(),
        &cfg(),
        &proposer,
        &ScriptedModel::new("j"),
        &renderer,
    )
    .await
    .unwrap();

    assert_eq!(
        r.outcome,
        Outcome::Exhausted {
            reason: ExhaustReason::RendererUnavailable,
            best: None
        }
    );
    assert_eq!((renderer.calls(), r.attempts.len()), (1, 1));
    assert!(r.attempts[0]
        .render_error
        .as_deref()
        .unwrap()
        .contains("refused"));
}

#[tokio::test]
async fn without_label_text_a_second_judge_call_must_confirm_the_match() {
    let proposer = ScriptedModel::new("p")
        .on(Purpose::Describe, vec![describe(&["A"])])
        .on(Purpose::Propose, vec![source("a")]);
    let judge = ScriptedModel::new("j")
        .on(Purpose::Judge, vec![verdict(true, 0.9)])
        .on(Purpose::Confirm, vec![verdict(true, 0.9)]);
    let renderer = FakeRenderer::new(rendered(None));

    let r = run_loop(&image(), &cfg(), &proposer, &judge, &renderer)
        .await
        .unwrap();

    assert_eq!(
        r.outcome,
        Outcome::Accepted {
            attempt: 1,
            via: AcceptedVia::JudgeConfirmed
        }
    );
    assert_eq!(
        (
            judge.calls_for(Purpose::Judge),
            judge.calls_for(Purpose::Confirm)
        ),
        (1, 1)
    );
}

#[tokio::test]
async fn a_refused_confirmation_keeps_the_run_going() {
    let proposer = ScriptedModel::new("p")
        .on(Purpose::Describe, vec![describe(&["A"])])
        .on(Purpose::Propose, vec![source("a"), source("b")]);
    let judge = ScriptedModel::new("j")
        .on(
            Purpose::Judge,
            vec![verdict(true, 0.9), verdict(true, 0.95)],
        )
        .on(
            Purpose::Confirm,
            vec![verdict(false, 0.3), verdict(true, 0.95)],
        );

    let r = run_loop(
        &image(),
        &cfg(),
        &proposer,
        &judge,
        &FakeRenderer::new(rendered(None)),
    )
    .await
    .unwrap();

    assert_eq!(
        r.outcome,
        Outcome::Accepted {
            attempt: 2,
            via: AcceptedVia::JudgeConfirmed
        }
    );
}

#[tokio::test]
async fn a_judge_match_is_not_enough_when_label_recall_is_below_the_threshold() {
    let proposer = ScriptedModel::new("p")
        .on(
            Purpose::Describe,
            vec![describe(&["Alpha", "Beta", "Gamma", "Delta"])],
        )
        .on(Purpose::Propose, vec![source("v1"), source("v2")]);
    let judge =
        ScriptedModel::new("j").on(Purpose::Judge, vec![verdict(true, 0.9), verdict(true, 0.9)]);
    let renderer = FakeRenderer::new(rendered(Some(&["Alpha", "Beta", "Gamma", "Delta"])))
        .then(rendered(Some(&["Alpha", "Beta"])));

    let r = run_loop(&image(), &cfg(), &proposer, &judge, &renderer)
        .await
        .unwrap();

    assert_eq!(
        r.attempts[0].verdict.as_ref().unwrap().label_recall,
        Some(0.5)
    );
    assert_eq!(
        r.outcome,
        Outcome::Accepted {
            attempt: 2,
            via: AcceptedVia::LabelsAndJudge
        }
    );
}

#[tokio::test]
async fn no_configuration_can_exceed_its_iteration_cap() {
    for max_iterations in 1..=8u32 {
        let proposer = ScriptedModel::new("p")
            .on(Purpose::Describe, vec![describe(&["A"])])
            .on(
                Purpose::Propose,
                (0..20).map(|i| source(&format!("v{i}"))).collect(),
            );
        let judge = ScriptedModel::new("j").otherwise(Purpose::Judge, verdict(false, 0.0));
        let renderer = FakeRenderer::new(rendered(Some(&["a"])));
        let mut c = cfg();
        c.max_iterations = max_iterations;
        c.stall_window = 0; // disabled, so only the cap can end the run

        let r = run_loop(&image(), &c, &proposer, &judge, &renderer)
            .await
            .unwrap();

        assert_eq!(r.attempts.len() as u32, max_iterations);
        assert!(
            proposer.calls() as u32 <= 1 + max_iterations,
            "describe + one propose per iteration"
        );
        assert!(judge.calls() as u32 <= max_iterations);
        assert_eq!(
            r.outcome,
            Outcome::Exhausted {
                reason: ExhaustReason::MaxIterations,
                best: Some(1)
            }
        );
    }
}

#[tokio::test]
async fn oversized_model_sources_are_rejected_as_a_model_error() {
    let proposer = ScriptedModel::new("p")
        .on(Purpose::Describe, vec![describe(&["A"])])
        .on(Purpose::Propose, vec![source(&"x".repeat(50))]);
    let mut c = cfg();
    c.max_source_chars = 10;
    c.max_consecutive_model_errors = 1;
    let renderer = FakeRenderer::new(rendered(None));

    let r = run_loop(&image(), &c, &proposer, &ScriptedModel::new("j"), &renderer)
        .await
        .unwrap();

    assert_eq!(renderer.calls(), 0);
    assert!(r.attempts[0]
        .model_error
        .as_deref()
        .unwrap()
        .contains("exceeds 10"));
}

#[test]
fn the_result_cache_key_depends_on_every_outcome_relevant_input_and_nothing_else() {
    let img = image();
    let base = cfg();
    let key = |c: &LoopConfig, p: &str, j: &str| result_cache_key(&img, c, p, j);
    let k0 = key(&base, "p", "j");
    assert_eq!(k0, key(&base, "p", "j"), "stable");
    assert_eq!(k0.len(), 64);

    let mut changed: Vec<(&str, String)> = vec![
        ("proposer", key(&base, "p2", "j")),
        ("judge", key(&base, "p", "j2")),
    ];
    let mut c = base.clone();
    c.format = "plantuml".into();
    changed.push(("format", key(&c, "p", "j")));
    let mut c = base.clone();
    c.max_iterations = 7;
    changed.push(("max_iterations", key(&c, "p", "j")));
    let mut c = base.clone();
    c.max_tokens = 1;
    changed.push(("max_tokens", key(&c, "p", "j")));
    let mut c = base.clone();
    c.label_recall_threshold = 0.5;
    changed.push(("threshold", key(&c, "p", "j")));
    let mut c = base.clone();
    c.stall_window = 5;
    changed.push(("stall_window", key(&c, "p", "j")));
    for (what, k) in &changed {
        assert_ne!(*k, k0, "{what} must change the key");
    }

    let mut c = base.clone();
    c.max_wall = Duration::from_secs(1);
    c.call_timeout = Duration::from_secs(1);
    assert_eq!(
        key(&c, "p", "j"),
        k0,
        "timeouts are operational, not part of the result identity"
    );

    // Length-prefixing: moving a boundary between two adjacent fields must not collide.
    assert_ne!(key(&base, "ab", "c"), key(&base, "a", "bc"));
}
