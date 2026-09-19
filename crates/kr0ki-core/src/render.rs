//! The render backend: diagram text -> rendered bytes.
//!
//! P0 talks to a Kroki HTTP endpoint directly (`POST {base}/{slug}/{output}` with the
//! source as the request body) — exactly what `_b00t_/kroki.mcp.toml`'s wrapper does.
//! The vendored `vendor/kroki-mcp` (PRD NFR2) is not wired in yet: for raw Kroki-family
//! text the MCP hop adds nothing over the HTTP call. It becomes relevant when kr0ki
//! needs kroki-mcp's SVG-normalization / inline-embedding behaviour.
//!
//! `SECURE` mode is the backend's responsibility (PRD FR2): kr0ki renders LLM-authored
//! text, so the Kroki instance MUST run with `KROKI_SAFE_MODE=secure` to neutralise
//! PlantUML `!include`/`!includeurl`. kr0ki does not itself sanitise the source.

use std::time::Duration;

use crate::cache::OutputKind;
use crate::format::DiagramFormat;

/// A thing that can turn diagram text into rendered bytes.
#[allow(async_fn_in_trait)]
pub trait RenderBackend {
    async fn render(
        &self,
        format: DiagramFormat,
        output: OutputKind,
        source: &str,
    ) -> Result<Vec<u8>, RenderError>;

    /// Endpoint description for health/observability reporting. `None` for
    /// backends without a single HTTP endpoint (e.g. in-process test doubles).
    fn describe_endpoint(&self) -> Option<&str> {
        None
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// The backend rejected the diagram source (Kroki 400 — bad syntax).
    #[error("backend rejected the diagram source ({status}): {body}")]
    BadSource { status: u16, body: String },
    /// The backend was unreachable or returned 5xx / an unexpected status.
    #[error("backend unavailable: {0}")]
    Unavailable(String),
    /// The backend's own SVG couldn't be rasterized to PNG locally
    /// (`RenderService`'s SVG-to-PNG fallback, `crate::flatten`). The diagram source
    /// itself was fine — this is our downstream processing of a trusted SVG failing.
    #[error("SVG-to-PNG flatten failed: {0}")]
    Flatten(String),
}

/// Direct-HTTP Kroki client.
#[derive(Debug, Clone)]
pub struct HttpKrokiBackend {
    base_url: String,
    http: reqwest::Client,
}

impl HttpKrokiBackend {
    /// `base_url` e.g. `https://kroki.io` or `http://localhost:8000`. Trailing slash
    /// tolerated.
    pub fn new(base_url: impl Into<String>) -> Self {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(concat!("kr0ki/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("reqwest client builds with default config");
        Self { base_url, http }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

impl RenderBackend for HttpKrokiBackend {
    fn describe_endpoint(&self) -> Option<&str> {
        Some(&self.base_url)
    }

    async fn render(
        &self,
        format: DiagramFormat,
        output: OutputKind,
        source: &str,
    ) -> Result<Vec<u8>, RenderError> {
        let url = format!("{}/{}/{}", self.base_url, format.kroki_slug(), output.ext());
        let resp = self
            .http
            .post(&url)
            .header("Content-Type", "text/plain")
            .body(source.to_string())
            .send()
            .await
            .map_err(|e| RenderError::Unavailable(e.to_string()))?;

        let status = resp.status();
        if status.is_success() {
            return resp
                .bytes()
                .await
                .map(|b| b.to_vec())
                .map_err(|e| RenderError::Unavailable(e.to_string()));
        }

        let body = resp.text().await.unwrap_or_default();
        if status.as_u16() == 400 {
            Err(RenderError::BadSource {
                status: 400,
                body: body.chars().take(2000).collect(),
            })
        } else {
            Err(RenderError::Unavailable(format!(
                "{status}: {}",
                body.chars().take(500).collect::<String>()
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_url_trailing_slash_is_normalised() {
        let b = HttpKrokiBackend::new("https://kroki.io/");
        assert_eq!(b.base_url(), "https://kroki.io");
    }

    // A live render test against a real Kroki is an integration test, env-gated on
    // KR0KI_TEST_BACKEND (same pattern as the b00t nats integration tests) — see
    // tests/live_render.rs.
}
