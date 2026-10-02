//! Client for the **SysML v2 MCP sidecar** (`containers/kr0ki-sysml-mcp`: the `sysml-v2-lsp` MCP server behind a small HTTP bridge).
//!
//! The sidecar is a read-only *language* service (parse / validate / symbols / summary over SysML v2 text). It is **not** the source
//! of truth for requirement graphs: it does not surface `satisfy` relations or `<'short names'>`, and it does not parse pure KerML
//! (verified 2026-10-01); kr0ki's own `sysml_lift` stays authoritative for those.
//!
//! One MCP session is shared and re-established transparently if the sidecar forgets it (restart, idle expiry). Only an allowlist of
//! read-only tools can be called, so a caller can never reach a tool this client was not written for.

use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::Mutex;

/// The tools kr0ki may call: pure functions of the supplied text.
pub const ALLOWED_TOOLS: &[&str] = &[
    "parse",
    "validate",
    "getDiagnostics",
    "getSymbols",
    "getModelSummary",
    "getComplexity",
];

/// SysML text larger than this is refused before it reaches the sidecar.
pub const MAX_SOURCE_BYTES: usize = 256 * 1024;
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum SysmlMcpError {
    #[error("tool '{0}' is not allowed")]
    NotAllowed(String),
    #[error("SysML source is larger than {MAX_SOURCE_BYTES} bytes")]
    TooLarge,
    #[error("the SysML MCP sidecar is unavailable: {0}")]
    Unavailable(String),
    #[error("MCP error {code}: {message}")]
    Rpc { code: i64, message: String },
    #[error("the tool reported an error: {0}")]
    Tool(String),
    #[error("unexpected response from the SysML MCP sidecar: {0}")]
    Protocol(String),
}

pub struct SysmlMcpClient {
    base: String,
    http: reqwest::Client,
    session: Mutex<Option<String>>,
    next_id: AtomicU64,
}

impl SysmlMcpClient {
    /// `base` is the sidecar's HTTP root, e.g. `http://127.0.0.1:8790` (the client posts to `{base}/mcp`).
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            base: base.into().trim_end_matches('/').to_owned(),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(35))
                .build()
                .expect("reqwest client builds"),
            session: Mutex::new(None),
            next_id: AtomicU64::new(1),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base
    }

    fn endpoint(&self) -> String {
        format!("{}/mcp", self.base)
    }

    fn id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    async fn post(
        &self,
        session: Option<&str>,
        body: &Value,
    ) -> Result<(reqwest::StatusCode, Option<String>, Vec<u8>), SysmlMcpError> {
        let mut req = self.http.post(self.endpoint()).json(body);
        if let Some(s) = session {
            req = req.header("mcp-session-id", s);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| SysmlMcpError::Unavailable(e.to_string()))?;
        let status = resp.status();
        let sid = resp
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| SysmlMcpError::Unavailable(e.to_string()))?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err(SysmlMcpError::Protocol("response is too large".into()));
        }
        Ok((status, sid, bytes.to_vec()))
    }

    async fn start_session(&self) -> Result<String, SysmlMcpError> {
        let init = json!({"jsonrpc": "2.0", "id": self.id(), "method": "initialize",
            "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "kr0ki", "version": env!("CARGO_PKG_VERSION")}}});
        let (status, sid, _) = self.post(None, &init).await?;
        if !status.is_success() {
            return Err(SysmlMcpError::Unavailable(format!(
                "initialize returned HTTP {status}"
            )));
        }
        let sid = sid.ok_or_else(|| {
            SysmlMcpError::Protocol("initialize returned no Mcp-Session-Id".into())
        })?;
        let (status, _, _) = self
            .post(
                Some(&sid),
                &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            )
            .await?;
        if !status.is_success() {
            return Err(SysmlMcpError::Unavailable(format!(
                "initialized notification returned HTTP {status}"
            )));
        }
        Ok(sid)
    }

    /// Call an allowlisted tool with `{ "code": <source> }` and return its structured result.
    pub async fn call_tool(&self, name: &str, source: &str) -> Result<Value, SysmlMcpError> {
        if !ALLOWED_TOOLS.contains(&name) {
            return Err(SysmlMcpError::NotAllowed(name.to_owned()));
        }
        if source.len() > MAX_SOURCE_BYTES {
            return Err(SysmlMcpError::TooLarge);
        }
        // Two attempts: the second after the sidecar has forgotten our session (404) or restarted.
        for attempt in 0..2 {
            let sid = {
                let mut guard = self.session.lock().await;
                if guard.is_none() {
                    *guard = Some(self.start_session().await?);
                }
                guard.clone().expect("session was just set")
            };
            let call = json!({"jsonrpc": "2.0", "id": self.id(), "method": "tools/call", "params": {"name": name, "arguments": {"code": source}}});
            let (status, _, body) = self.post(Some(&sid), &call).await?;
            if status == reqwest::StatusCode::NOT_FOUND && attempt == 0 {
                *self.session.lock().await = None;
                continue;
            }
            if !status.is_success() {
                return Err(SysmlMcpError::Unavailable(format!(
                    "tools/call returned HTTP {status}"
                )));
            }
            return parse_result(&body);
        }
        Err(SysmlMcpError::Unavailable(
            "could not establish a session".into(),
        ))
    }
}

/// Pull the tool's JSON out of a `tools/call` response: `structuredContent`, else the JSON text in `content[0]`, else the text.
fn parse_result(body: &[u8]) -> Result<Value, SysmlMcpError> {
    let v: Value = serde_json::from_slice(body)
        .map_err(|e| SysmlMcpError::Protocol(format!("not JSON: {e}")))?;
    if let Some(err) = v.get("error") {
        return Err(SysmlMcpError::Rpc {
            code: err.get("code").and_then(Value::as_i64).unwrap_or(0),
            message: err
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
        });
    }
    let result = v
        .get("result")
        .ok_or_else(|| SysmlMcpError::Protocol("response has neither result nor error".into()))?;
    let text = result.pointer("/content/0/text").and_then(Value::as_str);
    if result.get("isError").and_then(Value::as_bool) == Some(true) {
        return Err(SysmlMcpError::Tool(
            text.unwrap_or("unknown error").to_owned(),
        ));
    }
    if let Some(structured) = result.get("structuredContent") {
        return Ok(structured.clone());
    }
    match text {
        Some(t) => Ok(serde_json::from_str(t).unwrap_or_else(|_| Value::String(t.to_owned()))),
        None => Err(SysmlMcpError::Protocol("result has no content".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn rpc(id_hint: &str, result: Value) -> ResponseTemplate {
        let _ = id_hint;
        ResponseTemplate::new(200)
            .set_body_json(json!({"jsonrpc": "2.0", "id": 1, "result": result}))
    }

    async fn sidecar() -> MockServer {
        let s = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/mcp"))
            .and(body_partial_json(json!({"method": "initialize"})))
            .respond_with(
                rpc("init", json!({"serverInfo": {"name": "sysml-v2"}}))
                    .insert_header("mcp-session-id", "S1"),
            )
            .mount(&s)
            .await;
        Mock::given(method("POST"))
            .and(path("/mcp"))
            .and(body_partial_json(
                json!({"method": "notifications/initialized"}),
            ))
            .respond_with(ResponseTemplate::new(202))
            .mount(&s)
            .await;
        s
    }

    #[tokio::test]
    async fn calls_a_tool_after_initializing_one_session_and_returns_the_structured_result() {
        let s = sidecar().await;
        Mock::given(method("POST")).and(path("/mcp")).and(header("mcp-session-id", "S1"))
            .and(body_partial_json(json!({"method": "tools/call", "params": {"name": "validate", "arguments": {"code": "package P {}"}}})))
            .respond_with(rpc("call", json!({"content": [{"type": "text", "text": "{\"valid\":true}"}], "structuredContent": {"valid": true, "syntaxErrors": []}})))
            .expect(2).mount(&s).await;
        let c = SysmlMcpClient::new(s.uri());
        assert_eq!(
            c.call_tool("validate", "package P {}").await.unwrap(),
            json!({"valid": true, "syntaxErrors": []})
        );
        c.call_tool("validate", "package P {}").await.unwrap(); // the session is reused: initialize is not repeated
        let initializes = s
            .received_requests()
            .await
            .unwrap()
            .iter()
            .filter(|r| String::from_utf8_lossy(&r.body).contains("\"initialize\""))
            .count();
        assert_eq!(initializes, 1);
    }

    #[tokio::test]
    async fn re_establishes_the_session_when_the_sidecar_has_forgotten_it() {
        let s = sidecar().await;
        Mock::given(method("POST"))
            .and(path("/mcp"))
            .and(body_partial_json(json!({"method": "tools/call"})))
            .respond_with(ResponseTemplate::new(404))
            .up_to_n_times(1)
            .mount(&s)
            .await;
        Mock::given(method("POST"))
            .and(path("/mcp"))
            .and(body_partial_json(json!({"method": "tools/call"})))
            .respond_with(rpc("call", json!({"structuredContent": {"ok": 1}})))
            .mount(&s)
            .await;
        let c = SysmlMcpClient::new(s.uri());
        assert_eq!(c.call_tool("parse", "x").await.unwrap(), json!({"ok": 1}));
        let initializes = s
            .received_requests()
            .await
            .unwrap()
            .iter()
            .filter(|r| String::from_utf8_lossy(&r.body).contains("\"initialize\""))
            .count();
        assert_eq!(initializes, 2);
    }

    #[tokio::test]
    async fn falls_back_to_the_json_text_when_there_is_no_structured_content() {
        let s = sidecar().await;
        Mock::given(method("POST"))
            .and(path("/mcp"))
            .and(body_partial_json(json!({"method": "tools/call"})))
            .respond_with(rpc(
                "call",
                json!({"content": [{"type": "text", "text": "{\"symbolCount\":6}"}]}),
            ))
            .mount(&s)
            .await;
        assert_eq!(
            SysmlMcpClient::new(s.uri())
                .call_tool("parse", "x")
                .await
                .unwrap(),
            json!({"symbolCount": 6})
        );
    }

    #[tokio::test]
    async fn maps_protocol_and_tool_errors_distinctly() {
        let s = sidecar().await;
        Mock::given(method("POST")).and(path("/mcp")).and(body_partial_json(json!({"method": "tools/call", "params": {"name": "parse"}})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"jsonrpc": "2.0", "id": 1, "error": {"code": -32602, "message": "bad params"}}))).mount(&s).await;
        Mock::given(method("POST"))
            .and(path("/mcp"))
            .and(body_partial_json(
                json!({"method": "tools/call", "params": {"name": "validate"}}),
            ))
            .respond_with(rpc(
                "call",
                json!({"isError": true, "content": [{"type": "text", "text": "boom"}]}),
            ))
            .mount(&s)
            .await;
        Mock::given(method("POST"))
            .and(path("/mcp"))
            .and(body_partial_json(
                json!({"method": "tools/call", "params": {"name": "getSymbols"}}),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&s)
            .await;
        let c = SysmlMcpClient::new(s.uri());
        assert_eq!(
            c.call_tool("parse", "x").await.unwrap_err(),
            SysmlMcpError::Rpc {
                code: -32602,
                message: "bad params".into()
            }
        );
        assert_eq!(
            c.call_tool("validate", "x").await.unwrap_err(),
            SysmlMcpError::Tool("boom".into())
        );
        assert!(matches!(
            c.call_tool("getSymbols", "x").await.unwrap_err(),
            SysmlMcpError::Protocol(_)
        ));
    }

    #[tokio::test]
    async fn only_allowlisted_tools_and_bounded_sources_ever_reach_the_sidecar() {
        let s = sidecar().await;
        let c = SysmlMcpClient::new(s.uri());
        for bad in ["preview", "visualise", "deleteEverything", "", "validate "] {
            assert_eq!(
                c.call_tool(bad, "x").await.unwrap_err(),
                SysmlMcpError::NotAllowed(bad.into())
            );
        }
        assert_eq!(
            c.call_tool("parse", &"x".repeat(MAX_SOURCE_BYTES + 1))
                .await
                .unwrap_err(),
            SysmlMcpError::TooLarge
        );
        assert!(
            s.received_requests().await.unwrap().is_empty(),
            "nothing must have been sent"
        );
    }

    #[tokio::test]
    async fn reports_an_unreachable_sidecar_instead_of_panicking() {
        let c = SysmlMcpClient::new("http://127.0.0.1:1");
        assert!(matches!(
            c.call_tool("validate", "x").await.unwrap_err(),
            SysmlMcpError::Unavailable(_)
        ));
        let s = MockServer::start().await; // initialize answers without a session id
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"result": {}})))
            .mount(&s)
            .await;
        assert!(matches!(
            SysmlMcpClient::new(s.uri())
                .call_tool("validate", "x")
                .await
                .unwrap_err(),
            SysmlMcpError::Protocol(_)
        ));
    }
}
