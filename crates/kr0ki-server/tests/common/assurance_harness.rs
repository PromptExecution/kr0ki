#![allow(dead_code)] // each acceptance test uses a different part of this shared harness

//! Shared harness for the KR-A06 / KR-A08 acceptance cases: the real router behind the real
//! gateway, listening on an ephemeral port, with an in-memory model server behind the change
//! service, three identities, and an audit log. Callers reach it either with plain HTTP or
//! through the actual Python MCP bridge, so both transports hit the same server.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kr0ki_core::audit_log::AuditLog;
use kr0ki_core::change_service::in_memory::InMemoryServer;
use kr0ki_core::change_service::ChangeService;
use kr0ki_core::verification_runner::{CommandExecutor, ExecRequest, ExecResult, RevisionProbe};
use kr0ki_core::{cache::FsCache, render::HttpKrokiBackend, RenderService};

use crate::app::assurance::{AssuranceRuntime, ChangeApi};
use crate::app::gateway::{AuthMode, Gateway, Identity};
use crate::app::{router_with_gateway, AppState};

/// Tokens exist only here; the gateway holds their SHA-256.
pub const READER: &str = "tok-reader-read-only";
pub const COMMITTER: &str = "tok-committer";
pub const AUDITOR: &str = "tok-auditor";

pub struct Probe;

impl RevisionProbe for Probe {
    fn implementation_revision(&self) -> Result<String, String> {
        Ok("impl-test".into())
    }

    fn toolchain(&self) -> String {
        "rustc-test".into()
    }
}

pub struct NoExec;

impl CommandExecutor for NoExec {
    fn run(&self, _: &ExecRequest) -> ExecResult {
        ExecResult {
            spawn_error: Some("not used by these cases".into()),
            ..Default::default()
        }
    }
}

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

pub struct Harness {
    pub base: String,
    pub audit: Arc<AuditLog>,
    pub tmp: PathBuf,
}

/// Start the server. `tag` keeps concurrent tests' files apart.
pub async fn start(tag: &str) -> Harness {
    let tmp = std::env::temp_dir().join(format!("kr0ki-gw-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let service = RenderService::new(
        HttpKrokiBackend::new("http://127.0.0.1:1"),
        FsCache::new(tmp.join("cache")),
    );
    let audit = Arc::new(AuditLog::open(tmp.join("audit").join("audit.jsonl")).unwrap());
    let assurance_service = Arc::new(kr0ki_core::assurance_service::AssuranceService::new(
        kr0ki_core::assurance_service::AssuranceConfig {
            baseline_path: repo_root().join("docs/assurance/kr0ki.assurance.toml"),
            repo_root: repo_root(),
            evidence_dir: tmp.join("evidence"),
            run_timeout: Duration::from_secs(5),
        },
        Arc::new(Probe),
        Arc::new(NoExec),
    ));
    let changes: Arc<dyn ChangeApi> = Arc::new(ChangeService::new(InMemoryServer::new(true)));
    let runtime = Arc::new(AssuranceRuntime {
        service: assurance_service.clone(),
        changes: Some(changes),
        audit: Some(audit.clone()),
        managed_prefix: "kr0ki:assurance:".into(),
    });
    let contract = Arc::new(crate::contract::ContractReference::default());
    let state = AppState {
        service: Arc::new(service),
        playbook_dir: tmp.join("no-playbook"),
        b00t_graph_artifacts_path: None,
        capabilities_path: None,
        kubediagram_worker_url: None,
        sysmlv2_client: None,
        model_graph: Arc::new(kr0ki_core::graph_store::GraphStore::new()),
        ui_bus: Arc::new(kr0ki_core::ui_bus::UiBus::new()),
        brand_dir: tmp.join("no-brands"),
        sysml_mcp: None,
        storyb00k_agent_url: None,
        llm_api_url: None,
        llm_api_key: None,
        started_at: std::time::Instant::now(),
        boot_wall_clock: std::time::SystemTime::now(),
        auth_token: None,
        contract: contract.clone(),
        assurance: Some(runtime),
    };
    let identities = vec![
        Identity::new("reader", &["assurance.read", "model.read"], READER),
        Identity::new(
            "committer",
            &[
                "assurance.read",
                "assurance.propose",
                "model.read",
                "model.commit",
            ],
            COMMITTER,
        ),
        Identity::new("auditor", &["audit.read"], AUDITOR),
    ];
    let svc = assurance_service.clone();
    let gateway = Arc::new(
        Gateway::new(AuthMode::Identities(identities), Some(audit.clone()))
            .with_model_revision(move || svc.model_revision()),
    );
    let app = router_with_gateway(state, gateway, contract);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    Harness {
        base: format!("http://{addr}"),
        audit,
        tmp,
    }
}

/// A running `containers/kr0ki-mcp/bridge.py`, speaking MCP over stdio, authenticated with one
/// token — exactly how an agent's MCP client would reach the server.
pub struct Bridge {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: Mutex<u64>,
}

impl Bridge {
    pub fn start(base: &str, token: &str) -> Bridge {
        let script = repo_root().join("containers/kr0ki-mcp/bridge.py");
        let mut child = Command::new("python3")
            .arg(&script)
            .current_dir(script.parent().unwrap())
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("KR0KI_URL", base)
            .env("KR0KI_AUTH_TOKEN", token)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("python3 is required to run the MCP bridge: this case cannot be verified without it");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Bridge {
            child,
            stdin,
            stdout,
            next_id: Mutex::new(1),
        }
    }

    fn rpc(&mut self, method: &str, params: serde_json::Value) -> serde_json::Value {
        let id = {
            let mut n = self.next_id.lock().unwrap();
            *n += 1;
            *n
        };
        let line =
            serde_json::json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writeln!(self.stdin, "{line}").unwrap();
        self.stdin.flush().unwrap();
        let mut out = String::new();
        self.stdout.read_line(&mut out).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&out).unwrap_or_else(|e| panic!("bridge said {out:?}: {e}"));
        assert_eq!(v["id"], id, "{v}");
        v
    }

    pub fn initialize(&mut self) {
        let v = self.rpc("initialize", serde_json::json!({}));
        assert_eq!(v["result"]["serverInfo"]["name"], "kr0ki-mcp");
    }

    /// `tools/call`; returns `(is_error, text)`.
    pub fn call(&mut self, tool: &str, arguments: serde_json::Value) -> (bool, String) {
        let v = self.rpc(
            "tools/call",
            serde_json::json!({"name": tool, "arguments": arguments}),
        );
        let result = &v["result"];
        (
            result["isError"].as_bool().unwrap_or(false),
            result["content"][0]["text"]
                .as_str()
                .unwrap_or("")
                .to_string(),
        )
    }

    /// Any MCP request; returns the whole JSON-RPC reply.
    pub fn request(&mut self, method: &str, params: serde_json::Value) -> serde_json::Value {
        self.rpc(method, params)
    }

    pub fn tool_names(&mut self) -> Vec<String> {
        let v = self.rpc("tools/list", serde_json::json!({}));
        v["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect()
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Plain HTTP with a bearer token and an optional correlation id.
pub async fn http(
    method: reqwest::Method,
    url: &str,
    token: &str,
    request_id: Option<&str>,
    body: Option<serde_json::Value>,
) -> (u16, serde_json::Value) {
    let mut req = reqwest::Client::new()
        .request(method, url)
        .bearer_auth(token);
    if let Some(id) = request_id {
        req = req.header("x-request-id", id);
    }
    if let Some(b) = body {
        req = req.header("content-type", "text/plain").body(b.to_string());
    }
    let resp = req.send().await.unwrap();
    let status = resp.status().as_u16();
    let text = resp.text().await.unwrap();
    (
        status,
        serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text)),
    )
}
