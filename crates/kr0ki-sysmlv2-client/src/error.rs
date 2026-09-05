//! Error type for the SysML v2 API client.

use thiserror::Error;

/// Anything that can go wrong talking to an OMG Systems Modeling API server.
#[derive(Debug, Error)]
pub enum ClientError {
    /// Transport-level failure (DNS, connect, TLS, timeout, body read).
    #[error("http transport error: {0}")]
    Http(#[from] reqwest::Error),

    /// The server answered, but with a non-2xx status. `body` is the response body,
    /// truncated to a sane length for logging.
    #[error("server returned status {code}: {body}")]
    Status { code: u16, body: String },

    /// The response was 2xx but its body did not deserialize into the expected shape.
    #[error("json decode error: {0}")]
    Json(#[from] serde_json::Error),
}
