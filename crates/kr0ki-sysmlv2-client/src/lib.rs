//! `kr0ki-sysmlv2-client` — a thin async REST client for the OMG **Systems Modeling
//! API and Services** (the SysML v2 API).
//!
//! It targets the *generic* OMG PSM, not any one server. The endpoint subset here is
//! the intersection that Flexo's `flexo-mms-sysmlv2` (v0.2.0), the OMG Java pilot
//! `Systems-Modeling/SysML-v2-API-Services`, `Open-MBEE/OpenSysML`, and Eclipse SysON's
//! `/api/rest/` all implement: **read-only** Project / Branch / Tag / Commit / Element
//! / Relationship navigation, plus `roots`. No Diff/Merge, no Query POST, no mutation —
//! those are either unimplemented or inconsistent across the target servers (see
//! `docs/EVAL-flexo.md`, `docs/EVAL-syson.md`).
//!
//! ## Endpoints (all GET), paths per the OMG PSM
//!
//! | Method | Path |
//! |---|---|
//! | [`projects`](SysmlV2Client::projects) | `/projects` |
//! | [`project`](SysmlV2Client::project) | `/projects/{id}` |
//! | [`branches`](SysmlV2Client::branches) / [`branch`](SysmlV2Client::branch) | `/projects/{id}/branches[/{branchId}]` |
//! | [`tags`](SysmlV2Client::tags) / [`tag`](SysmlV2Client::tag) | `/projects/{id}/tags[/{tagId}]` |
//! | [`commits`](SysmlV2Client::commits) / [`commit`](SysmlV2Client::commit) | `/projects/{id}/commits[/{commitId}]` |
//! | [`elements`](SysmlV2Client::elements) | `/projects/{id}/commits/{cid}/elements?page-after=&page-before=&page-size=` |
//! | [`roots`](SysmlV2Client::roots) | `/projects/{id}/commits/{cid}/roots` |
//! | [`relationships`](SysmlV2Client::relationships) | `/projects/{id}/commits/{cid}/elements/{eid}/relationships?direction=in\|out\|both` |
//!
//! ## Paging heuristic
//!
//! `elements(..)` returns an [`ElementPage`] whose `next_after` is:
//! 1. the `page-after` value parsed out of a `Link: rel="next"` response header, if the
//!    server sent one (OMG pilot + Flexo do); else
//! 2. the last element's `@id`, *only if* the caller passed an explicit `page-size` and
//!    got a full page back (fallback for SysON, which sends no `Link` header); else
//! 3. `None` (exhausted).
//!
//! [`all_elements`](SysmlV2Client::all_elements) loops this until `next_after` is
//! `None`. See `paging::derive_next_after` for the full rationale.
//!
//! The `page-after` / `page-before` / `page-size` query-param spelling above is the
//! default [`PageParamStyle::Hyphenated`]; some JSON:API-flavoured OMG-pilot
//! deployments use bracket form (`page[after]=`) instead — set
//! [`SysmlV2Client::with_page_param_style`] to [`PageParamStyle::JsonApiBracket`] for
//! those.
//!
//! ## Content hashing
//!
//! No target server exposes a per-commit content hash / ETag, so
//! [`SysmlV2Client::snapshot`] builds a [`ModelSnapshot`] and hashes it client-side
//! (`compute_content_hash`) into a deterministic, order-independent SHA-256 — kr0ki's
//! model-side cache key for PRD-KR0KI-001 FR5.

mod error;
mod hash;
mod model;
mod paging;

use std::time::Duration;

use serde::de::DeserializeOwned;

pub use error::ClientError;
pub use hash::{canonical_json, compute_content_hash};
pub use model::{
    Branch, Commit, Direction, Element, ElementPage, ModelSnapshot, Page, Project, Ref, Tag,
};
pub use paging::PageParamStyle;

/// `User-Agent` sent on every request.
pub const USER_AGENT: &str = concat!("kr0ki-sysmlv2-client/", env!("CARGO_PKG_VERSION"));

/// Async client for one OMG Systems Modeling API server.
#[derive(Debug, Clone)]
pub struct SysmlV2Client {
    base_url: String,
    http: reqwest::Client,
    token: Option<String>,
    page_param_style: PageParamStyle,
}

impl SysmlV2Client {
    /// New client for `base_url` (e.g. `https://sysml2.example.com` or
    /// `http://localhost:9000`). A trailing slash is trimmed. 30s request timeout.
    pub fn new(base_url: impl Into<String>) -> Self {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(USER_AGENT)
            .build()
            .expect("reqwest client builds with default config");
        Self {
            base_url,
            http,
            token: None,
            page_param_style: PageParamStyle::default(),
        }
    }

    /// Attach a bearer token, sent as `Authorization: Bearer <token>` on every request.
    #[must_use]
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Set which paging query-parameter convention this target server expects
    /// (default [`PageParamStyle::Hyphenated`] — Flexo and the OMG Java pilot). Some
    /// JSON:API-flavoured OMG-pilot deployments need [`PageParamStyle::JsonApiBracket`]
    /// instead; check the target server's own docs / `EVAL-*.md` before switching.
    #[must_use]
    pub fn with_page_param_style(mut self, style: PageParamStyle) -> Self {
        self.page_param_style = style;
        self
    }

    /// The normalized base URL (no trailing slash).
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    // ---- HTTP plumbing ----------------------------------------------------------

    async fn send_get(
        &self,
        url: &str,
        query: &[(&str, String)],
    ) -> Result<reqwest::Response, ClientError> {
        let mut req = self.http.get(url);
        if !query.is_empty() {
            req = req.query(query);
        }
        if let Some(token) = &self.token {
            req = req.bearer_auth(token);
        }
        Ok(req.send().await?)
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T, ClientError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.send_get(&url, query).await?;
        let status = resp.status();
        let bytes = resp.bytes().await?;
        if !status.is_success() {
            return Err(ClientError::Status {
                code: status.as_u16(),
                body: truncate_body(&bytes),
            });
        }
        Ok(serde_json::from_slice(&bytes)?)
    }

    // ---- Projects -------------------------------------------------------------

    /// `GET /projects`
    pub async fn projects(&self) -> Result<Vec<Project>, ClientError> {
        self.get_json("/projects", &[]).await
    }

    /// `GET /projects/{project_id}`
    pub async fn project(&self, project_id: &str) -> Result<Project, ClientError> {
        self.get_json(&format!("/projects/{project_id}"), &[]).await
    }

    // ---- Branches ----------------------------------------------------------

    /// `GET /projects/{project_id}/branches`
    pub async fn branches(&self, project_id: &str) -> Result<Vec<Branch>, ClientError> {
        self.get_json(&format!("/projects/{project_id}/branches"), &[])
            .await
    }

    /// `GET /projects/{project_id}/branches/{branch_id}`
    pub async fn branch(&self, project_id: &str, branch_id: &str) -> Result<Branch, ClientError> {
        self.get_json(&format!("/projects/{project_id}/branches/{branch_id}"), &[])
            .await
    }

    // ---- Tags ------------------------------------------------------------

    /// `GET /projects/{project_id}/tags`
    pub async fn tags(&self, project_id: &str) -> Result<Vec<Tag>, ClientError> {
        self.get_json(&format!("/projects/{project_id}/tags"), &[])
            .await
    }

    /// `GET /projects/{project_id}/tags/{tag_id}`
    pub async fn tag(&self, project_id: &str, tag_id: &str) -> Result<Tag, ClientError> {
        self.get_json(&format!("/projects/{project_id}/tags/{tag_id}"), &[])
            .await
    }

    // ---- Commits ---------------------------------------------------------

    /// `GET /projects/{project_id}/commits`
    pub async fn commits(&self, project_id: &str) -> Result<Vec<Commit>, ClientError> {
        self.get_json(&format!("/projects/{project_id}/commits"), &[])
            .await
    }

    /// `GET /projects/{project_id}/commits/{commit_id}`
    pub async fn commit(&self, project_id: &str, commit_id: &str) -> Result<Commit, ClientError> {
        self.get_json(&format!("/projects/{project_id}/commits/{commit_id}"), &[])
            .await
    }

    // ---- Elements ------------------------------------------------------------

    /// `GET /projects/{project_id}/commits/{commit_id}/elements` — one page.
    ///
    /// `page` maps to `page-after` / `page-before` / `page-size`. The returned
    /// [`ElementPage::next_after`] is derived per the crate-level paging heuristic.
    pub async fn elements(
        &self,
        project_id: &str,
        commit_id: &str,
        page: Page,
    ) -> Result<ElementPage, ClientError> {
        let url = format!(
            "{}/projects/{}/commits/{}/elements",
            self.base_url, project_id, commit_id
        );
        let mut query: Vec<(&str, String)> = Vec::new();
        if let Some(after) = &page.after {
            query.push((self.page_param_style.after_key(), after.clone()));
        }
        if let Some(before) = &page.before {
            query.push((self.page_param_style.before_key(), before.clone()));
        }
        if let Some(size) = page.size {
            query.push((self.page_param_style.size_key(), size.to_string()));
        }

        let resp = self.send_get(&url, &query).await?;
        let status = resp.status();
        let link = resp
            .headers()
            .get(reqwest::header::LINK)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let bytes = resp.bytes().await?;
        if !status.is_success() {
            return Err(ClientError::Status {
                code: status.as_u16(),
                body: truncate_body(&bytes),
            });
        }
        let items: Vec<Element> = serde_json::from_slice(&bytes)?;
        let next_after =
            paging::derive_next_after(link.as_deref(), &items, page.size, self.page_param_style);
        Ok(ElementPage { items, next_after })
    }

    /// Every element at `(project_id, commit_id)`, following `next_after` until the
    /// collection is exhausted.
    pub async fn all_elements(
        &self,
        project_id: &str,
        commit_id: &str,
    ) -> Result<Vec<Element>, ClientError> {
        let mut out: Vec<Element> = Vec::new();
        let mut after: Option<String> = None;
        loop {
            let page = Page {
                after: after.clone(),
                before: None,
                size: None,
            };
            let mut res = self.elements(project_id, commit_id, page).await?;
            out.append(&mut res.items);
            match res.next_after {
                // Stop if the server hands back the same cursor again (guards against a
                // server that echoes `page-after` but does not advance).
                Some(next) if Some(next.as_str()) != after.as_deref() => after = Some(next),
                _ => break,
            }
        }
        Ok(out)
    }

    /// `GET /projects/{project_id}/commits/{commit_id}/roots` → the `@id`s of the
    /// commit's root elements. Tolerates a body of either id strings or `{"@id": ...}`
    /// objects.
    pub async fn roots(
        &self,
        project_id: &str,
        commit_id: &str,
    ) -> Result<Vec<String>, ClientError> {
        let values: Vec<serde_json::Value> = self
            .get_json(
                &format!("/projects/{project_id}/commits/{commit_id}/roots"),
                &[],
            )
            .await?;
        Ok(values.iter().filter_map(value_to_id).collect())
    }

    /// `GET /projects/{p}/commits/{c}/elements/{element_id}/relationships?direction=…`
    pub async fn relationships(
        &self,
        project_id: &str,
        commit_id: &str,
        element_id: &str,
        dir: Direction,
    ) -> Result<Vec<Element>, ClientError> {
        self.get_json(
            &format!(
                "/projects/{project_id}/commits/{commit_id}/elements/{element_id}/relationships"
            ),
            &[("direction", dir.as_query().to_string())],
        )
        .await
    }

    // ---- Snapshot ----------------------------------------------------------

    /// Pull `all_elements` + `roots` for `(project_id, commit_id)` and wrap them in a
    /// content-hashed [`ModelSnapshot`].
    pub async fn snapshot(
        &self,
        project_id: &str,
        commit_id: &str,
    ) -> Result<ModelSnapshot, ClientError> {
        let elements = self.all_elements(project_id, commit_id).await?;
        let roots = self.roots(project_id, commit_id).await?;
        let content_hash = compute_content_hash(&elements, &roots);
        Ok(ModelSnapshot {
            project_id: project_id.to_string(),
            commit_id: commit_id.to_string(),
            elements,
            roots,
            content_hash,
        })
    }
}

fn value_to_id(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(m) => m.get("@id").and_then(|x| x.as_str()).map(str::to_string),
        _ => None,
    }
}

fn truncate_body(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).chars().take(2000).collect()
}
