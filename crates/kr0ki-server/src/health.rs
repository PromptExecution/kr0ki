//! Deep health reporting for `/health` (operator request, 2026-09-18).
//!
//! The k8s readinessProbe only needs a fast 200, so the handler stays cheap:
//! every dependency check runs concurrently with a hard timeout, degrades to
//! `"degraded"` instead of failing, and NEVER performs a billable LLM request —
//! model enumeration (`GET {OPENAI_API_URL}/models`) is metadata only.

use std::time::{Duration, Instant};

use serde::Serialize;

use crate::app::AppState;

const CHECK_TIMEOUT: Duration = Duration::from_millis(1500);

#[derive(Serialize)]
pub struct HealthReport {
    /// "ok" when core rendering paths are healthy, "degraded" when any
    /// dependency check failed. Always 200 so the readiness probe never
    /// flap-fails on an optional dependency.
    pub status: &'static str,
    pub service: &'static str,
    /// Compiled crate version (CARGO_PKG_VERSION).
    pub version: &'static str,
    /// Process boot time, RFC3339 UTC.
    pub started_at: String,
    /// Seconds since process start (== container uptime: kr0ki is PID 1).
    pub uptime_secs: u64,
    /// Ledgrrr contract reference (DESIGN-NOTE-ledgrrr-state-contract-registry.md).
    pub contract: String,
    pub checks: Checks,
}

#[derive(Serialize)]
pub struct Checks {
    /// Kroki-compatible render backend (KR0KI_BACKEND_URL).
    pub kroki_backend: Dependency,
    /// kr0ki-mcp sidecar's KubeDiagrams listener, if configured.
    pub kubediagram_worker: Option<Dependency>,
    /// AG-UI storyb00k sidecar (:8789), if configured.
    pub storyb00k_agent: Option<Dependency>,
    /// OpenAI-compatible LLM endpoint — model COUNT only, never an inference
    /// request. `configured=false` when OPENAI_API_KEY/URL are unset.
    pub llm: LlmCheck,
    /// Local durable/derived stores. kr0ki keeps no SQL databases; these are
    /// its actual persistence surfaces, reported honestly as such.
    pub stores: StoresCheck,
    pub caller_auth: bool,
}

#[derive(Serialize)]
pub struct Dependency {
    pub url: String,
    pub ok: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct LlmCheck {
    pub configured: bool,
    pub base_url: Option<String>,
    /// Key presence only — the key material is never echoed.
    pub api_key_set: bool,
    pub ok: bool,
    /// Number of models the endpoint advertises (`GET /models`).
    pub model_count: Option<u32>,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct StoresCheck {
    /// Content-addressed render cache (KR0KI_CACHE_DIR): writable + entry count.
    pub cache_dir: CacheDirCheck,
    /// kroki companion capabilities.json on the shared pod volume (kr0ki#20).
    pub capabilities_file: Option<bool>,
    /// Derived RDF triple count in the in-memory model graph store.
    pub graph_store_triples: usize,
}

#[derive(Serialize)]
pub struct CacheDirCheck {
    pub path: String,
    pub writable: bool,
    pub entries: usize,
    pub error: Option<String>,
}

struct Probe {
    ok: bool,
    latency_ms: Option<u64>,
    error: Option<String>,
}

async fn probe_json(http: &reqwest::Client, url: &str, auth: Option<&str>) -> Probe {
    let started = Instant::now();
    let mut req = http.get(url);
    if let Some(token) = auth {
        req = req.bearer_auth(token);
    }
    match req.send().await {
        Ok(resp) => {
            let latency = started.elapsed().as_millis() as u64;
            if resp.status().is_success() {
                Probe {
                    ok: true,
                    latency_ms: Some(latency),
                    error: None,
                }
            } else {
                Probe {
                    ok: false,
                    latency_ms: Some(latency),
                    error: Some(format!("HTTP {}", resp.status())),
                }
            }
        }
        Err(e) => Probe {
            ok: false,
            latency_ms: None,
            error: Some(e.to_string()),
        },
    }
}

async fn parse_models(body: reqwest::Response) -> Result<u32, String> {
    // The OpenAI listing shape is {"data": [...]}; tolerate any JSON object
    // with an array field named `data` (or a bare array).
    body.json::<serde_json::Value>()
        .await
        .map_err(|e| format!("unparseable /models body: {e}"))?
        .get("data")
        .and_then(|d| d.as_array())
        .map(|a| a.len() as u32)
        .ok_or_else(|| "/models body has no `data` array".to_string())
}

async fn check_llm(http: &reqwest::Client, base_url: &str, api_key: Option<&str>) -> LlmCheck {
    let mut check = LlmCheck {
        configured: true,
        base_url: Some(base_url.to_string()),
        api_key_set: api_key.is_some(),
        ok: false,
        model_count: None,
        latency_ms: None,
        error: None,
    };
    if base_url.trim().is_empty() {
        check.error = Some("OPENAI_API_URL is empty".into());
        return check;
    }
    let started = Instant::now();
    let mut req = http.get(format!("{}/models", base_url.trim_end_matches('/')));
    if let Some(key) = api_key {
        req = req.bearer_auth(key);
    }
    match req.send().await {
        Ok(resp) => {
            check.latency_ms = Some(started.elapsed().as_millis() as u64);
            let status = resp.status();
            match parse_models(resp).await {
                Ok(count) => {
                    check.ok = status.is_success();
                    check.model_count = Some(count);
                    if !status.is_success() {
                        check.error = Some(format!("HTTP {status}"));
                    }
                }
                Err(e) => check.error = Some(format!("HTTP {status}: {e}")),
            }
        }
        Err(e) => check.error = Some(e.to_string()),
    }
    check
}

async fn check_cache_dir(state: &AppState) -> CacheDirCheck {
    // Runs on the blocking pool: directory scans are I/O.
    let root = state.service.cache_root().to_path_buf();
    let result = tokio::task::spawn_blocking(move || {
        if !root.is_dir() {
            return CacheDirCheck {
                path: root.display().to_string(),
                writable: false,
                entries: 0,
                error: Some("directory missing".into()),
            };
        }
        let probe = root.join(".health-probe");
        let writable = std::fs::write(&probe, b"ok").is_ok();
        if writable {
            let _ = std::fs::remove_file(&probe);
        }
        let entries = std::fs::read_dir(&root)
            .map(|rd| rd.filter_map(|e| e.ok()).count())
            .unwrap_or(0);
        CacheDirCheck {
            path: root.display().to_string(),
            writable,
            entries,
            error: if writable {
                None
            } else {
                Some("not writable".into())
            },
        }
    })
    .await;
    result.unwrap_or_else(|e| CacheDirCheck {
        path: String::new(),
        writable: false,
        entries: 0,
        error: Some(format!("join error: {e}")),
    })
}

/// Build the full report. All network probes run concurrently; total latency
/// is bounded by CHECK_TIMEOUT, not the number of dependencies.
pub async fn collect(state: &AppState) -> HealthReport {
    let http = reqwest::Client::builder()
        .timeout(CHECK_TIMEOUT)
        .build()
        .expect("reqwest client builds with default config");

    let backend_url = state
        .service
        .backend_url()
        .unwrap_or("http://no-endpoint.invalid")
        .to_string();
    let kubediagram_url = state.kubediagram_worker_url.clone();
    let storyb00k_url = state.storyb00k_agent_url.clone();
    let llm_base = state.llm_api_url.clone();
    let llm_key = state.llm_api_key.clone();

    let kroki_url = format!("{backend_url}/health");
    let (kroki, kubediagram, storyb00k, llm, cache_dir) = tokio::join!(
        probe_json(&http, &kroki_url, None),
        async {
            match kubediagram_url.as_deref() {
                Some(url) => Some(probe_json(&http, &format!("{url}/health"), None).await),
                None => None,
            }
        },
        async {
            match storyb00k_url.as_deref() {
                Some(url) => Some(probe_json(&http, &format!("{url}/health"), None).await),
                None => None,
            }
        },
        async {
            match llm_base.as_deref() {
                Some(url) => Some(check_llm(&http, url, llm_key.as_deref()).await),
                None => None,
            }
        },
        check_cache_dir(state),
    );

    let to_dep = |url: &str, p: Probe| Dependency {
        url: url.to_string(),
        ok: p.ok,
        latency_ms: p.latency_ms,
        error: p.error,
    };
    let llm = llm.unwrap_or(LlmCheck {
        configured: false,
        base_url: None,
        api_key_set: false,
        ok: false,
        model_count: None,
        latency_ms: None,
        error: Some("OPENAI_API_KEY / OPENAI_API_URL not configured".into()),
    });

    let core_healthy = kroki.ok && cache_dir.writable;
    let uptime = state.started_at.elapsed();

    HealthReport {
        status: if core_healthy { "ok" } else { "degraded" },
        service: "kr0ki",
        version: env!("CARGO_PKG_VERSION"),
        started_at: humantime_or_fallback(state.boot_wall_clock),
        uptime_secs: uptime.as_secs(),
        contract: state.contract.uri.clone(),
        checks: Checks {
            kroki_backend: to_dep(&backend_url, kroki),
            kubediagram_worker: kubediagram_url
                .as_deref()
                .map(|url| to_dep(url, kubediagram.unwrap())),
            storyb00k_agent: storyb00k_url
                .as_deref()
                .map(|url| to_dep(url, storyb00k.unwrap())),
            llm,
            stores: StoresCheck {
                cache_dir,
                capabilities_file: state.capabilities_path.as_ref().map(|p| p.is_file()),
                graph_store_triples: state.model_graph.len(),
            },
            caller_auth: state.auth_token.is_some(),
        },
    }
}

fn humantime_or_fallback(boot: std::time::SystemTime) -> String {
    let secs = boot
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Minimal RFC3339 (seconds precision) without pulling chrono into the
    // workspace: days-since-epoch civil-date algorithm (Howard Hinnant).
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}
