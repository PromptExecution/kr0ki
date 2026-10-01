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
    /// Total prompt+completion tokens across all calls. Checked before every call.
    pub max_tokens: u64,
    /// Minimum label recall to accept when labels are available.
    pub label_recall_threshold: f64,
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

#[derive(Debug, Clone, Serialize)]
pub struct AttemptRecord {
    pub n: u32,
    pub source: String,
    pub source_sha256: String,
    pub render_error: Option<String>,
    /// A model or parse failure in this attempt (proposal or judgement).
    pub model_error: Option<String>,
    pub verdict: Option<Verdict>,
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

    async fn call<M: VisionModel>(
        &mut self,
        model: &M,
        req: crate::model::VisionRequest,
    ) -> Result<String, ModelError> {
        let remaining = self.cfg.max_wall.saturating_sub(self.start.elapsed());
        let limit = self.cfg.call_timeout.min(remaining);
        if limit.is_zero() {
            return Err(ModelError::Timeout);
        }
        match timeout(limit, model.complete(req)).await {
            Err(_) => Err(ModelError::Timeout),
            Ok(Err(e)) => Err(e),
            Ok(Ok(resp)) => {
                self.usage.add(resp.usage);
                Ok(resp.text)
            }
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

    let reply = clock
        .call(proposer, prompts::describe_request(image))
        .await
        .map_err(LoopError::Describe)?;
    let description = prompts::parse_description(&reply).map_err(LoopError::Description)?;
    if description.diagram_kind == DiagramKind::None {
        return Ok(LoopResult {
            outcome: Outcome::NotADiagram,
            description,
            attempts: Vec::new(),
            usage: clock.usage,
        });
    }

    let mut attempts: Vec<AttemptRecord> = Vec::new();
    let mut scores: Vec<f64> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut best: Option<usize> = None; // index into `attempts`, rendered attempts only
    let mut feedback: Option<Feedback> = None;
    let mut consecutive_errors = 0u32;

    let outcome = 'run: {
        for n in 1..=cfg.max_iterations {
            if let Some(reason) = clock.exhausted() {
                break 'run Outcome::Exhausted {
                    reason,
                    best: best.map(|i| attempts[i].n),
                };
            }
            let before = clock.usage;
            let best_source = best.map(|i| attempts[i].source.clone());

            // ---- propose -------------------------------------------------------------------------
            let request = prompts::propose_request(
                image,
                &description,
                &cfg.format,
                best_source.as_deref(),
                feedback.as_ref(),
            );
            let proposed = match clock.call(proposer, request).await {
                Ok(text) => {
                    prompts::extract_source(&text, cfg.max_source_chars).map_err(|e| e.to_string())
                }
                Err(e) => Err(e.to_string()),
            };
            let source = match proposed {
                Ok(s) => s,
                Err(message) => {
                    consecutive_errors += 1;
                    attempts.push(failed(
                        n,
                        String::new(),
                        None,
                        Some(message),
                        usage_since(clock.usage, before),
                    ));
                    scores.push(0.0);
                    if consecutive_errors >= cfg.max_consecutive_model_errors {
                        break 'run Outcome::Exhausted {
                            reason: ExhaustReason::ModelErrors,
                            best: best.map(|i| attempts[i].n),
                        };
                    }
                    continue;
                }
            };
            if !seen.insert(sha256_hex(&source)) {
                break 'run Outcome::Exhausted {
                    reason: ExhaustReason::Stalled,
                    best: best.map(|i| attempts[i].n),
                };
            }

            // ---- render --------------------------------------------------------------------------
            let rendered = match renderer.render(&cfg.format, &source).await {
                Ok(r) => r,
                Err(RenderError::Unavailable(message)) => {
                    attempts.push(failed(
                        n,
                        source,
                        Some(message),
                        None,
                        usage_since(clock.usage, before),
                    ));
                    break 'run Outcome::Exhausted {
                        reason: ExhaustReason::RendererUnavailable,
                        best: best.map(|i| attempts[i].n),
                    };
                }
                Err(RenderError::Rejected(message)) => {
                    feedback = Some(Feedback {
                        render_error: Some(message.clone()),
                        verdict: None,
                    });
                    attempts.push(failed(
                        n,
                        source,
                        Some(message),
                        None,
                        usage_since(clock.usage, before),
                    ));
                    scores.push(0.0);
                    continue;
                }
            };

            // ---- judge ---------------------------------------------------------------------------
            let labels: Option<LabelScore> = rendered
                .labels
                .as_ref()
                .map(|l| label_scores(&description.labels, l));
            let judged = match clock
                .call(
                    judge,
                    prompts::judge_request(
                        Purpose::Judge,
                        image,
                        &rendered.png,
                        &description,
                        &source,
                    ),
                )
                .await
            {
                Ok(text) => prompts::parse_verdict(&text).map_err(|e| e.to_string()),
                Err(e) => Err(e.to_string()),
            };
            let mut verdict = match judged {
                Ok(v) => v,
                Err(message) => {
                    consecutive_errors += 1;
                    let score = composite_score(true, None, labels.as_ref());
                    let mut record = failed(
                        n,
                        source,
                        None,
                        Some(message),
                        usage_since(clock.usage, before),
                    );
                    record.score = score;
                    attempts.push(record);
                    scores.push(score);
                    keep_best(&mut best, &attempts, score);
                    if consecutive_errors >= cfg.max_consecutive_model_errors {
                        break 'run Outcome::Exhausted {
                            reason: ExhaustReason::ModelErrors,
                            best: best.map(|i| attempts[i].n),
                        };
                    }
                    continue;
                }
            };
            consecutive_errors = 0;
            verdict.label_recall = labels.map(|l| l.recall);
            verdict.label_precision = labels.map(|l| l.precision);
            let score = composite_score(true, Some(&verdict), labels.as_ref());

            // ---- decide --------------------------------------------------------------------------
            let accepted = if !verdict.matches {
                None
            } else {
                match labels {
                    Some(l) if l.recall >= cfg.label_recall_threshold => {
                        Some(AcceptedVia::LabelsAndJudge)
                    }
                    Some(_) => None,
                    None => confirm(
                        &mut clock,
                        judge,
                        image,
                        &rendered.png,
                        &description,
                        &source,
                    )
                    .await
                    .then_some(AcceptedVia::JudgeConfirmed),
                }
            };

            attempts.push(AttemptRecord {
                n,
                source_sha256: sha256_hex(&source),
                source,
                render_error: None,
                model_error: None,
                verdict: Some(verdict.clone()),
                score,
                usage: usage_since(clock.usage, before),
            });
            scores.push(score);
            keep_best(&mut best, &attempts, score);

            if let Some(via) = accepted {
                break 'run Outcome::Accepted { attempt: n, via };
            }
            feedback = Some(Feedback {
                render_error: None,
                verdict: Some(verdict),
            });

            // Score stall only counts once something has rendered; before that the repeat-source check and
            // the consecutive-error cap bound a run that cannot get past the renderer.
            let window = cfg.stall_window as usize;
            if best.is_some() && window > 0 && scores.len() > window {
                let (earlier, recent) = scores.split_at(scores.len() - window);
                let max = |s: &[f64]| s.iter().copied().fold(f64::MIN, f64::max);
                if max(recent) <= max(earlier) {
                    break 'run Outcome::Exhausted {
                        reason: ExhaustReason::Stalled,
                        best: best.map(|i| attempts[i].n),
                    };
                }
            }
        }
        Outcome::Exhausted {
            reason: ExhaustReason::MaxIterations,
            best: best.map(|i| attempts[i].n),
        }
    };

    Ok(LoopResult {
        outcome,
        description,
        attempts,
        usage: clock.usage,
    })
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
        score: 0.0,
        usage,
    }
}

/// Track the best *rendered* attempt: strictly better scores win, so ties keep the earlier attempt.
fn keep_best(best: &mut Option<usize>, attempts: &[AttemptRecord], score: f64) {
    let idx = attempts.len() - 1;
    match best {
        Some(b) if attempts[*b].score >= score => {}
        _ => *best = Some(idx),
    }
}

/// Second, differently-worded judge call used when label text is unavailable. Any failure means "not confirmed";
/// the next iteration's budget check will stop the run if the budget is gone.
async fn confirm<J: VisionModel>(
    clock: &mut Clock<'_>,
    judge: &J,
    image: &NormalizedImage,
    render_png: &[u8],
    description: &Description,
    source: &str,
) -> bool {
    if clock.exhausted().is_some() {
        return false;
    }
    let request = prompts::judge_request(Purpose::Confirm, image, render_png, description, source);
    match clock.call(judge, request).await {
        Ok(text) => prompts::parse_verdict(&text).is_ok_and(|v| v.matches),
        Err(_) => false,
    }
}
