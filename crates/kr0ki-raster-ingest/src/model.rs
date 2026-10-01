//! The two seams the loop is written against, so it can be tested without a model or a renderer.
//!
//! Both traits return `impl Future + Send` so callers can use them from axum handlers. The loop is generic
//! over them (no `dyn`, no `async-trait`).

use std::future::Future;

/// Why the model is being called; fakes and adapters branch on this, prompts differ per purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Purpose {
    /// Read the original image into a [`crate::Description`].
    Describe,
    /// Produce diagram-as-code.
    Propose,
    /// Compare the original with a render.
    Judge,
    /// Re-check a match with a differently-worded, edge-by-edge prompt (used when labels are unavailable).
    Confirm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePart {
    pub mime: &'static str,
    /// Reference-counted, so the same image is shared across requests instead of copied into each.
    pub bytes: bytes::Bytes,
}

#[derive(Debug, Clone)]
pub struct VisionRequest {
    pub purpose: Purpose,
    pub system: String,
    pub prompt: String,
    pub images: Vec<ImagePart>,
    pub temperature: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

impl Usage {
    pub fn total(&self) -> u64 {
        self.prompt_tokens.saturating_add(self.completion_tokens)
    }
    pub(crate) fn add(&mut self, other: Usage) {
        self.prompt_tokens = self.prompt_tokens.saturating_add(other.prompt_tokens);
        self.completion_tokens = self
            .completion_tokens
            .saturating_add(other.completion_tokens);
    }
}

#[derive(Debug, Clone)]
pub struct VisionResponse {
    pub text: String,
    pub usage: Usage,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelError {
    #[error("model transport error: {0}")]
    Transport(String),
    #[error("model call timed out")]
    Timeout,
    #[error("model rejected the request: {0}")]
    Rejected(String),
}

impl ModelError {
    /// Worth repeating the identical request: a dropped connection can succeed next time. A refusal cannot, and
    /// repeating a timeout would double the time spent on one render.
    pub fn is_retryable(&self) -> bool {
        matches!(self, ModelError::Transport(_))
    }
}

pub trait VisionModel: Sync {
    /// Stable identifier (part of the result cache key).
    fn id(&self) -> &str;
    fn complete(
        &self,
        req: VisionRequest,
    ) -> impl Future<Output = Result<VisionResponse, ModelError>> + Send;
}

/// A successful render.
#[derive(Debug, Clone)]
pub struct Rendered {
    pub png: bytes::Bytes,
    /// Text found in the rendered SVG, or `None` when it cannot be extracted (text drawn as paths).
    pub labels: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RenderError {
    /// The source is wrong (syntax error, unknown construct). Fed back to the model.
    #[error("render rejected the source: {0}")]
    Rejected(String),
    /// The renderer itself is down. Not the model's fault: the loop stops instead of retrying.
    #[error("renderer unavailable: {0}")]
    Unavailable(String),
}

pub trait Renderer: Sync {
    fn render(
        &self,
        format: &str,
        source: &str,
    ) -> impl Future<Output = Result<Rendered, RenderError>> + Send;
}
