//! The bounded "does it match" loop (PLAN-KR0KI-007 §3).
//!
//! Every cap lives here, not in a prompt: a model can neither extend the run nor talk its way past a limit.
//! The loop always returns what it has: an exhausted run carries its best attempt and every attempt record.

use crate::{
    model::{ModelError, Purpose, RenderError, Renderer, Usage, VisionModel},
    normalize::NormalizedImage,
    prompts::{self, Feedback, ParseError},
    scoring::{composite_score, label_scores, LabelScore},
    types::{Description, DiagramKind, Verdict},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{collections::HashSet, time::Duration};
use tokio::time::{timeout, Instant};

#[derive(Debug, Clone)]
pub struct LoopConfig {
    /// Output format slug (validated by the caller against the renderer's formats).
    pub format: String,
    pub max_iterations: u32,
    /// Wall-clock budget for the whole run.
    pub max_wall: Duration,
    /// Per model call; also clipped to the wall-clock time remaining.
    pub call_timeout: Duration,
    /// Total prompt+completion tokens across all calls, as reported by the model. Checked before every model call,
    /// so one call can overshoot by its own usage. Calls that fail or time out report nothing, so the count is a
    /// lower bound on what was actually spent.
    pub max_tokens: u64,
    /// Minimum label recall to accept when labels are available.
    pub label_recall_threshold: f64,
    /// Minimum judge score for a `match: true` to count. A verdict that says match but scores below this, or lists
    /// differences, is treated as not matching.
    pub min_match_score: f64,
    /// Stop when the best score has not improved over this many attempts.
    pub stall_window: u32,
    /// Stop after this many consecutive model failures (transport, timeout, unparsable reply).
    pub max_consecutive_model_errors: u32,
    pub max_source_chars: usize,
}

impl LoopConfig {
    pub fn new(format: impl Into<String>) -> Self {
        Self {
            format: format.into(),
            max_iterations: 6,
            max_wall: Duration::from_secs(300),
            call_timeout: Duration::from_secs(90),
            max_tokens: 200_000,
            label_recall_threshold: 0.9,
            min_match_score: 0.7,
            stall_window: 2,
            max_consecutive_model_errors: 3,
            max_source_chars: 20_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExhaustReason {
    MaxIterations,
    WallClock,
    TokenBudget,
    /// A source repeated, or the best score stopped improving.
    Stalled,
    ModelErrors,
    RendererUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptedVia {
    /// Judge said match and label recall met the threshold.
    LabelsAndJudge,
    /// No labels were available; a second, differently-worded judge call agreed.
    JudgeConfirmed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Outcome {
    Accepted {
        attempt: u32,
        via: AcceptedVia,
    },
    /// `best` is the attempt number of the best *rendered* attempt, if any rendered at all.
    Exhausted {
        reason: ExhaustReason,
        best: Option<u32>,
    },
    NotADiagram,
}

/// What the second, edge-by-edge judge call said (only made when label text could not vouch for a match).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Confirmation {
    Agreed,
    Disagreed,
    Failed { error: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct AttemptRecord {
    pub n: u32,
    pub source: String,
    pub source_sha256: String,
    pub render_error: Option<String>,
    /// A model or parse failure in this attempt (proposal or judgement).
    pub model_error: Option<String>,
    pub verdict: Option<Verdict>,
    pub confirmation: Option<Confirmation>,
    pub score: f64,
    pub usage: Usage,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoopResult {
    pub outcome: Outcome,
    pub description: Description,
    pub attempts: Vec<AttemptRecord>,
    pub usage: Usage,
}

impl LoopResult {
    /// Whether this result is safe to cache by [`crate::result_cache_key`]. Only runs that reached a verdict on
    /// the content are: `Accepted` and `NotADiagram`. An `Exhausted` run may have been cut short by wall-clock
    /// time, a flaky model or an overloaded renderer, none of which are part of the key.
    pub fn is_cacheable(&self) -> bool {
        matches!(
            self.outcome,
            Outcome::Accepted { .. } | Outcome::NotADiagram
        )
    }

    /// The accepted source, or the best rendered attempt's source when exhausted.
    pub fn best_source(&self) -> Option<&str> {
        let n = match &self.outcome {
            Outcome::Accepted { attempt, .. } => Some(*attempt),
            Outcome::Exhausted { best, .. } => *best,
            Outcome::NotADiagram => None,
        }?;
        self.attempts
            .iter()
            .find(|a| a.n == n)
            .map(|a| a.source.as_str())
    }
}

/// Failures with nothing worth returning: the image could not even be described.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum LoopError {
    #[error("describing the image failed: {0}")]
    Describe(ModelError),
    #[error("the description could not be parsed: {0}")]
    Description(ParseError),
}

/// Why a model call did not produce text.
enum Stop {
    /// A budget ran out before the call was made; the run must end with this reason.
    Budget(ExhaustReason),
    Model(ModelError),
}

/// Why a render did not produce an image.
enum RenderFail {
    /// The source is wrong; fed back to the model.
    Rejected(String),
    Unavailable(String),
    TimedOut(Duration),
}

struct Clock<'a> {
    cfg: &'a LoopConfig,
    start: Instant,
    usage: Usage,
}

impl Clock<'_> {
    fn exhausted(&self) -> Option<ExhaustReason> {
        if self.start.elapsed() >= self.cfg.max_wall {
            Some(ExhaustReason::WallClock)
        } else if self.usage.total() >= self.cfg.max_tokens {
            Some(ExhaustReason::TokenBudget)
        } else {
            None
        }
    }

    /// Time a single call may take: the per-call timeout, clipped to the wall-clock time remaining.
    fn call_limit(&self) -> Duration {
        self.cfg
            .call_timeout
            .min(self.cfg.max_wall.saturating_sub(self.start.elapsed()))
    }

    /// One model call. Budgets are checked here, before every call, not only between iterations.
    async fn call<M: VisionModel>(
        &mut self,
        model: &M,
        req: crate::model::VisionRequest,
    ) -> Result<String, Stop> {
        if let Some(reason) = self.exhausted() {
            return Err(Stop::Budget(reason));
        }
        match timeout(self.call_limit(), model.complete(req)).await {
            Err(_) => Err(Stop::Model(ModelError::Timeout)),
            Ok(Err(e)) => Err(Stop::Model(e)),
            Ok(Ok(resp)) => {
                self.usage.add(resp.usage);
                Ok(resp.text)
            }
        }
    }

    /// One render, bounded by the same per-call limit so a hung renderer cannot outlive the wall-clock budget.
    async fn render<R: Renderer>(
        &self,
        renderer: &R,
        format: &str,
        source: &str,
    ) -> Result<crate::model::Rendered, RenderFail> {
        let limit = self.call_limit();
        match timeout(limit, renderer.render(format, source)).await {
            Err(_) => Err(RenderFail::TimedOut(limit)),
            Ok(Ok(r)) => Ok(r),
            Ok(Err(RenderError::Rejected(m))) => Err(RenderFail::Rejected(m)),
            Ok(Err(RenderError::Unavailable(m))) => Err(RenderFail::Unavailable(m)),
        }
    }
}

fn sha256_hex(s: &str) -> String {
    Sha256::digest(s.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn usage_since(now: Usage, before: Usage) -> Usage {
    Usage {
        prompt_tokens: now.prompt_tokens.saturating_sub(before.prompt_tokens),
        completion_tokens: now
            .completion_tokens
            .saturating_sub(before.completion_tokens),
    }
}

/// One extra attempt at a judge call that failed with a *transient* error (a dropped connection) or an unparsable
/// reply (re-prompted more strictly, since the identical request would repeat the reply) before the attempt is
/// recorded as a model error. Re-judging the same render is cheaper than a new
/// proposal, and a new proposal after a judge failure tends to repeat the same source and end the run as `Stalled`.
/// Timeouts and refusals are not retried: a timeout would double the time spent on one render, and a refusal
/// cannot succeed the second time.
const JUDGE_RETRIES: u32 = 1;
/// Consecutive render timeouts before the renderer is treated as unavailable instead of the source as too complex.
const RENDER_TIMEOUTS_BEFORE_UNAVAILABLE: u32 = 2;
/// One re-prompt for the describe step (see [`describe`]).
const DESCRIBE_RETRIES: u32 = 1;

/// Everything the loop remembers between attempts.
#[derive(Default)]
struct State {
    attempts: Vec<AttemptRecord>,
    /// Scores of *judged* attempts only, which is what the stall rule looks at: a run recovering from syntax errors
    /// has not had a chance to improve the content yet.
    judged_scores: Vec<f64>,
    seen: HashSet<String>,
    /// Index into `attempts` of the best *rendered* attempt.
    best: Option<usize>,
    feedback: Option<Feedback>,
    /// Consecutive proposal failures and consecutive judge failures are counted separately: each role's counter
    /// resets only on that role's own success, so an unrelated success cannot hide a role that is down.
    propose_errors: u32,
    judge_errors: u32,
    render_timeouts: u32,
}

enum Step {
    Next,
    Done(Outcome),
}

impl State {
    fn exhausted(&self, reason: ExhaustReason) -> Step {
        Step::Done(Outcome::Exhausted {
            reason,
            best: self.best.map(|i| self.attempts[i].n),
        })
    }

    /// Track the best *rendered* attempt: strictly better scores win, so ties keep the earlier attempt.
    fn consider_best(&mut self) {
        let idx = self.attempts.len() - 1;
        match self.best {
            Some(b) if self.attempts[b].score >= self.attempts[idx].score => {}
            _ => self.best = Some(idx),
        }
    }

    /// The best judged score has not improved over the last `stall_window` judged attempts.
    fn stalled(&self, window: u32) -> bool {
        let window = window as usize;
        if self.best.is_none() || window == 0 || self.judged_scores.len() <= window {
            return false;
        }
        let (earlier, recent) = self
            .judged_scores
            .split_at(self.judged_scores.len() - window);
        let max = |s: &[f64]| s.iter().copied().fold(f64::MIN, f64::max);
        max(recent) <= max(earlier)
    }

    /// Owned copy of the best attempt's source and score, for building the next prompt.
    fn best_so_far(&self) -> Option<(String, f64)> {
        self.best
            .map(|i| (self.attempts[i].source.clone(), self.attempts[i].score))
    }
}

struct Ctx<'a, M, J, R> {
    image: &'a NormalizedImage,
    cfg: &'a LoopConfig,
    description: &'a Description,
    proposer: &'a M,
    judge: &'a J,
    renderer: &'a R,
}

/// Run the loop. `proposer` describes the image and writes sources; `judge` compares renders with the original.
pub async fn run_loop<M, J, R>(
    image: &NormalizedImage,
    cfg: &LoopConfig,
    proposer: &M,
    judge: &J,
    renderer: &R,
) -> Result<LoopResult, LoopError>
where
    M: VisionModel,
    J: VisionModel,
    R: Renderer,
{
    let mut clock = Clock {
        cfg,
        start: Instant::now(),
        usage: Usage::default(),
    };

    let description = describe(&mut clock, proposer, image).await?;
    if description.diagram_kind == DiagramKind::None {
        return Ok(LoopResult {
            outcome: Outcome::NotADiagram,
            description,
            attempts: Vec::new(),
            usage: clock.usage,
        });
    }

    let ctx = Ctx {
        image,
        cfg,
        description: &description,
        proposer,
        judge,
        renderer,
    };
    let mut st = State::default();
    let mut outcome = None;
    for n in 1..=cfg.max_iterations {
        if let Some(reason) = clock.exhausted() {
            if let Step::Done(o) = st.exhausted(reason) {
                outcome = Some(o);
            }
            break;
        }
        if let Step::Done(o) = attempt(n, &ctx, &mut clock, &mut st).await {
            outcome = Some(o);
            break;
        }
    }
    // Ran out of iterations without any other verdict: report the best rendered attempt.
    let outcome = match outcome {
        Some(o) => o,
        None => match st.exhausted(ExhaustReason::MaxIterations) {
            Step::Done(o) => o,
            Step::Next => unreachable!("State::exhausted always returns Step::Done"),
        },
    };
    Ok(LoopResult {
        outcome,
        description,
        attempts: st.attempts,
        usage: clock.usage,
    })
}

/// Read the image into a [`Description`]. One re-prompt: after a dropped connection the same request, after an
/// unparsable reply a stricter one (the identical request at temperature 0 would just repeat the reply). A timeout
/// or refusal is final.
async fn describe<M: VisionModel>(
    clock: &mut Clock<'_>,
    model: &M,
    image: &NormalizedImage,
) -> Result<Description, LoopError> {
    let mut strict = false;
    let mut last = LoopError::Describe(ModelError::Rejected("describe was not attempted".into()));
    for _ in 0..=DESCRIBE_RETRIES {
        match clock
            .call(model, prompts::describe_request(image, strict))
            .await
        {
            Ok(text) => match prompts::parse_description(&text) {
                Ok(d) => return Ok(d),
                Err(e) => {
                    strict = true;
                    last = LoopError::Description(e);
                }
            },
            // Nothing has been spent before the first call, so a budget stop here means a zero budget.
            Err(Stop::Budget(_)) => {
                return Err(LoopError::Describe(ModelError::Rejected(
                    "budget is zero".into(),
                )))
            }
            Err(Stop::Model(e)) => {
                let retry = e.is_retryable();
                last = LoopError::Describe(e);
                if !retry {
                    break;
                }
            }
        }
    }
    Err(last)
}

/// Target labels that can actually be compared: the description may omit them, or they may all normalize to
/// nothing. With none, a render trivially "covers" them, so the deterministic check carries no information and
/// the confirming judge call must stand in for it.
fn usable_target_labels(description: &Description) -> bool {
    description
        .labels
        .iter()
        .any(|l| !crate::scoring::normalize_label(l).is_empty())
}

async fn attempt<M: VisionModel, J: VisionModel, R: Renderer>(
    n: u32,
    ctx: &Ctx<'_, M, J, R>,
    clock: &mut Clock<'_>,
    st: &mut State,
) -> Step {
    let cfg = ctx.cfg;
    let before = clock.usage;

    // ---- propose ---------------------------------------------------------------------------------
    let best = st.best_so_far();
    let request = prompts::propose_request(
        ctx.image,
        ctx.description,
        &cfg.format,
        best.as_ref().map(|(source, score)| prompts::BestSoFar {
            source,
            score: *score,
        }),
        st.feedback.as_ref(),
    );
    let proposed = match clock.call(ctx.proposer, request).await {
        Err(Stop::Budget(reason)) => return st.exhausted(reason),
        Err(Stop::Model(e)) => Err(e.to_string()),
        Ok(text) => prompts::extract_source(&text, cfg.max_source_chars).map_err(|e| e.to_string()),
    };
    let source = match proposed {
        Ok(s) => {
            st.propose_errors = 0;
            s
        }
        Err(message) => {
            st.propose_errors += 1;
            st.attempts.push(failed(
                n,
                String::new(),
                None,
                Some(message),
                usage_since(clock.usage, before),
            ));
            return if st.propose_errors >= cfg.max_consecutive_model_errors {
                st.exhausted(ExhaustReason::ModelErrors)
            } else {
                Step::Next
            };
        }
    };
    if !st.seen.insert(sha256_hex(&source)) {
        // Recorded, so the proposal's tokens and the offending source can be audited.
        st.attempts.push(failed(
            n,
            source,
            None,
            Some("repeated an earlier source".into()),
            usage_since(clock.usage, before),
        ));
        return st.exhausted(ExhaustReason::Stalled);
    }

    // ---- render ----------------------------------------------------------------------------------
    let rendered = match clock.render(ctx.renderer, &cfg.format, &source).await {
        Ok(r) => {
            st.render_timeouts = 0;
            r
        }
        Err(RenderFail::Unavailable(message)) => {
            st.attempts.push(failed(
                n,
                source,
                Some(message),
                None,
                usage_since(clock.usage, before),
            ));
            return st.exhausted(ExhaustReason::RendererUnavailable);
        }
        Err(RenderFail::TimedOut(limit)) => {
            // If the wall clock is what ran out, the renderer is not to blame and the source was never given a
            // fair chance: end the run for the real reason instead of counting a renderer timeout.
            if let Some(reason) = clock.exhausted() {
                st.attempts.push(failed(
                    n,
                    source,
                    Some(format!("render cut short: {reason:?} reached")),
                    None,
                    usage_since(clock.usage, before),
                ));
                return st.exhausted(reason);
            }
            st.render_timeouts += 1;
            let message = format!("rendering timed out after {}s", limit.as_secs());
            st.feedback = Some(Feedback {
                source: source.clone(),
                render_error: Some(format!(
                    "{message}; the diagram may be too complex, simplify it"
                )),
                ..Feedback::default()
            });
            st.attempts.push(failed(
                n,
                source,
                Some(message),
                None,
                usage_since(clock.usage, before),
            ));
            if st.render_timeouts >= RENDER_TIMEOUTS_BEFORE_UNAVAILABLE {
                return st.exhausted(ExhaustReason::RendererUnavailable);
            }
            return Step::Next;
        }
        Err(RenderFail::Rejected(message)) => {
            // The renderer answered, so it is alive: earlier timeouts were about the source, not an outage.
            st.render_timeouts = 0;
            st.feedback = Some(Feedback {
                source: source.clone(),
                render_error: Some(message.clone()),
                ..Feedback::default()
            });
            st.attempts.push(failed(
                n,
                source,
                Some(message),
                None,
                usage_since(clock.usage, before),
            ));
            return Step::Next;
        }
    };

    // ---- judge -----------------------------------------------------------------------------------
    let labels: Option<LabelScore> = match (&rendered.labels, usable_target_labels(ctx.description))
    {
        (Some(found), true) => Some(label_scores(&ctx.description.labels, found)),
        _ => None,
    };
    let mut last_error: Option<String> = None;
    let mut verdict: Option<Verdict> = None;
    let mut strict = false;
    for _ in 0..=JUDGE_RETRIES {
        let request = prompts::judge_request(Purpose::Judge, ctx.image, &rendered.png, strict);
        match clock.call(ctx.judge, request).await {
            Ok(text) => match prompts::parse_verdict(&text) {
                Ok(v) => {
                    verdict = Some(v);
                    break;
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                    strict = true;
                }
            },
            Err(Stop::Model(e)) => {
                let retry = e.is_retryable();
                last_error = Some(e.to_string());
                if !retry {
                    break;
                }
            }
            Err(Stop::Budget(reason)) => {
                // The render exists but cannot be judged: keep it as an unjudged attempt, then stop.
                let message = last_error.map_or_else(
                    || format!("{reason:?} reached before judging"),
                    |e| format!("{e}; then {reason:?} reached"),
                );
                record_unjudged(
                    n,
                    source,
                    message,
                    labels.as_ref(),
                    usage_since(clock.usage, before),
                    st,
                );
                return st.exhausted(reason);
            }
        }
    }
    let Some(mut verdict) = verdict else {
        st.judge_errors += 1;
        let message = last_error.unwrap_or_else(|| "judge failed".into());
        st.feedback = Some(Feedback {
            source: source.clone(),
            unjudged: true,
            ..Feedback::default()
        });
        record_unjudged(
            n,
            source,
            message,
            labels.as_ref(),
            usage_since(clock.usage, before),
            st,
        );
        return if st.judge_errors >= cfg.max_consecutive_model_errors {
            st.exhausted(ExhaustReason::ModelErrors)
        } else {
            Step::Next
        };
    };
    st.judge_errors = 0;
    verdict.label_recall = labels.as_ref().map(|l| l.recall);
    verdict.label_precision = labels.as_ref().map(|l| l.precision);
    let score = composite_score(true, Some(&verdict), labels.as_ref());

    // ---- decide ----------------------------------------------------------------------------------
    let mut confirmation: Option<Confirmation> = None;
    let mut feedback_verdict = verdict.clone();
    let accepted = if !verdict.is_consistent_match(cfg.min_match_score) {
        None
    } else {
        match &labels {
            Some(l) if l.recall >= cfg.label_recall_threshold => Some(AcceptedVia::LabelsAndJudge),
            Some(_) => None,
            None => match confirm(
                clock,
                ctx.judge,
                ctx.image,
                &rendered.png,
                cfg.min_match_score,
            )
            .await
            {
                Confirmed::Agreed => {
                    confirmation = Some(Confirmation::Agreed);
                    Some(AcceptedVia::JudgeConfirmed)
                }
                Confirmed::Disagreed(v) => {
                    // Its differences are what the next proposal needs to hear.
                    confirmation = Some(Confirmation::Disagreed);
                    feedback_verdict = *v;
                    None
                }
                Confirmed::Failed(error) => {
                    confirmation = Some(Confirmation::Failed { error });
                    None
                }
            },
        }
    };

    let below_threshold = labels
        .as_ref()
        .filter(|l| !l.missing.is_empty() && l.recall < cfg.label_recall_threshold);
    st.feedback = Some(Feedback {
        source: source.clone(),
        render_error: None,
        verdict: Some(feedback_verdict),
        label_recall: below_threshold.map(|l| l.recall),
        missing_labels: below_threshold.map_or_else(Vec::new, |l| l.missing.clone()),
        unjudged: false,
    });
    st.attempts.push(AttemptRecord {
        n,
        source_sha256: sha256_hex(&source),
        source,
        render_error: None,
        model_error: None,
        verdict: Some(verdict),
        confirmation,
        score,
        usage: usage_since(clock.usage, before),
    });
    st.judged_scores.push(score);
    st.consider_best();

    if let Some(via) = accepted {
        return Step::Done(Outcome::Accepted { attempt: n, via });
    }
    if st.stalled(cfg.stall_window) {
        return st.exhausted(ExhaustReason::Stalled);
    }
    Step::Next
}

fn failed(
    n: u32,
    source: String,
    render_error: Option<String>,
    model_error: Option<String>,
    usage: Usage,
) -> AttemptRecord {
    AttemptRecord {
        n,
        source_sha256: if source.is_empty() {
            String::new()
        } else {
            sha256_hex(&source)
        },
        source,
        render_error,
        model_error,
        verdict: None,
        confirmation: None,
        score: 0.0,
        usage,
    }
}

/// A rendered attempt that could not be judged. It still counts as a rendered candidate (scored from label
/// recall alone) so a run that ends here can return it.
fn record_unjudged(
    n: u32,
    source: String,
    message: String,
    labels: Option<&LabelScore>,
    usage: Usage,
    st: &mut State,
) {
    let mut record = failed(n, source, None, Some(message), usage);
    record.score = composite_score(true, None, labels);
    st.attempts.push(record);
    st.consider_best();
}

enum Confirmed {
    Agreed,
    /// The confirming judge disagreed; its verdict carries the differences it found.
    Disagreed(Box<Verdict>),
    Failed(String),
}

/// Second, differently-worded judge call used when label text cannot vouch for the match. Any failure means "not
/// confirmed"; the next call's budget check ends the run if the budget is gone.
async fn confirm<J: VisionModel>(
    clock: &mut Clock<'_>,
    judge: &J,
    image: &NormalizedImage,
    render_png: &bytes::Bytes,
    min_score: f64,
) -> Confirmed {
    let request = prompts::judge_request(Purpose::Confirm, image, render_png, false);
    match clock.call(judge, request).await {
        Ok(text) => match prompts::parse_verdict(&text) {
            Ok(v) if v.is_consistent_match(min_score) => Confirmed::Agreed,
            Ok(v) => Confirmed::Disagreed(Box::new(v)),
            Err(e) => Confirmed::Failed(e.to_string()),
        },
        Err(Stop::Model(e)) => Confirmed::Failed(e.to_string()),
        Err(Stop::Budget(reason)) => Confirmed::Failed(format!("{reason:?} reached")),
    }
}
