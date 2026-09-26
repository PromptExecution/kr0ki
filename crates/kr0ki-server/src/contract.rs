//! Ledgrrr contract-reference boundary.
//!
//! kr0ki is stateless with respect to procedural state machines. It attaches
//! a contract reference to responses as metadata, but stores nothing and
//! executes no transitions. Ledgrrr owns the contract registry.
//!
//! See `docs/DESIGN-NOTE-ledgrrr-state-contract-registry.md`.

use axum::http::Request;
use std::sync::Arc;
use uuid::Uuid;

/// Contract reference attached to responses.
///
/// The contract URI is a label, not a live reference. When Ledgrrr publishes
/// its contract registry, this value becomes configurable or discoverable —
/// but the header shape does not change.
#[derive(Debug, Clone)]
pub struct ContractReference {
    /// The contract URI (e.g., `ledgrrr://state-machines/sysml-render/v1`).
    pub uri: String,
    /// The subject (e.g., `kr0ki.b00t.promptexecution.com` or `localhost`).
    pub subject: String,
}

impl Default for ContractReference {
    fn default() -> Self {
        Self {
            uri: "ledgrrr://state-machines/sysml-render/v1".to_string(),
            subject: "localhost".to_string(),
        }
    }
}

impl ContractReference {
    /// Create from environment variables.
    ///
    /// - `KR0KI_CONTRACT_URI` (default: `ledgrrr://state-machines/sysml-render/v1`)
    /// - `KR0KI_CONTRACT_SUBJECT` (default: `localhost`)
    #[allow(dead_code)] // used by main.rs binary, not by test harnesses
    pub fn from_env() -> Self {
        Self {
            uri: std::env::var("KR0KI_CONTRACT_URI")
                .unwrap_or_else(|_| "ledgrrr://state-machines/sysml-render/v1".to_string()),
            subject: std::env::var("KR0KI_CONTRACT_SUBJECT")
                .unwrap_or_else(|_| "localhost".to_string()),
        }
    }
}

/// Request ID extracted from or generated for a request.
///
/// If the incoming request carries `X-Request-ID`, adopt it (caller-propagated
/// correlation). If absent, generate one server-side (UUID v7 for time-sortability).
#[derive(Debug, Clone)]
pub struct RequestId(pub String);

impl RequestId {
    /// Extract from request headers or generate a new one.
    pub fn from_request<B>(req: &Request<B>) -> Self {
        req.headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .map(|s| Self(s.to_string()))
            .unwrap_or_else(|| Self(Uuid::now_v7().to_string()))
    }
}

/// Middleware that attaches contract reference and request ID to responses.
///
/// - Generates or adopts `X-Request-ID` from the request.
/// - Adds `X-Kr0ki-Contract` header with the contract URI.
/// - Adds `X-Kr0ki-Request-Id` header with the request ID.
/// - Stores the request ID in request extensions for use by handlers.
pub async fn contract_middleware(
    req: axum::extract::Request,
    next: axum::middleware::Next,
    contract: Arc<ContractReference>,
) -> axum::response::Response {
    let request_id = RequestId::from_request(&req);

    // Store request ID in extensions for handlers to use
    let mut req = req;
    req.extensions_mut().insert(request_id.clone());

    let mut response = next.run(req).await;

    // Attach contract reference headers
    response.headers_mut().insert(
        "x-kr0ki-contract",
        contract.uri.parse().expect("valid header value"),
    );
    response.headers_mut().insert(
        "x-kr0ki-request-id",
        request_id.0.parse().expect("valid header value"),
    );

    response
}

/// Add `request_id` to a JSON error response.
///
/// Error responses already return JSON with `error` and `detail` fields.
/// This adds `request_id` to the envelope for correlation.
pub fn error_with_request_id(
    error: &str,
    detail: &str,
    request_id: &RequestId,
) -> serde_json::Value {
    serde_json::json!({
        "error": error,
        "detail": detail,
        "request_id": request_id.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_reference_default() {
        let c = ContractReference::default();
        assert_eq!(c.uri, "ledgrrr://state-machines/sysml-render/v1");
        assert_eq!(c.subject, "localhost");
    }

    #[test]
    fn request_id_generated_when_absent() {
        let req = Request::builder().uri("/health").body(()).unwrap();
        let id = RequestId::from_request(&req);
        assert!(!id.0.is_empty());
        // UUID v7 is 36 chars with hyphens
        assert_eq!(id.0.len(), 36);
    }

    #[test]
    fn request_id_adopted_when_present() {
        let req = Request::builder()
            .uri("/health")
            .header("x-request-id", "my-correlation-123")
            .body(())
            .unwrap();
        let id = RequestId::from_request(&req);
        assert_eq!(id.0, "my-correlation-123");
    }

    #[test]
    fn error_with_request_id_includes_all_fields() {
        let req_id = RequestId("test-id-456".to_string());
        let err = error_with_request_id("bad_request", "something went wrong", &req_id);
        assert_eq!(err["error"], "bad_request");
        assert_eq!(err["detail"], "something went wrong");
        assert_eq!(err["request_id"], "test-id-456");
    }
}
