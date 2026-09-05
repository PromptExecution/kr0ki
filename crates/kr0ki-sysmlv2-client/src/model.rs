//! Data types for the subset of the OMG Systems Modeling API PSM that kr0ki reads.
//!
//! Every struct is *tolerant*: it names only the fields kr0ki depends on and captures
//! everything else in a `serde_json` map (`extra` / `fields`). A server adding fields,
//! or one omitting an optional one, never breaks deserialization. This matters because
//! the target servers (Flexo `flexo-mms-sysmlv2`, the OMG Java pilot, `Open-MBEE/OpenSysML`,
//! Eclipse SysON) each implement a slightly different slice of the same spec.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// An `@id` reference object, e.g. `Commit.owningProject`. The OMG PSM serializes
/// cross-element references as `{ "@id": "<uuid>", ... }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ref {
    #[serde(rename = "@id")]
    pub at_id: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A project on the model server. `POST {base}/projects/{id}` addresses it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    #[serde(rename = "@id")]
    pub at_id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A branch within a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    #[serde(rename = "@id")]
    pub at_id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A tag within a project (a named, immutable pointer at a commit).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    #[serde(rename = "@id")]
    pub at_id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A commit: an immutable snapshot of a project's model. `(projectId, commitId)` is
/// the stable, forever-cacheable coordinate kr0ki keys on (see `EVAL-flexo.md`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    #[serde(rename = "@id")]
    pub at_id: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub created: Option<String>,
    /// The project this commit belongs to, as an `@id` reference object. Servers that
    /// inline this as a bare string id instead are not supported for this field.
    #[serde(rename = "owningProject", default)]
    pub owning_project: Option<Ref>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A single model element. `@id` and `@type` are lifted out; everything else the
/// element carries stays in `fields`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Element {
    #[serde(rename = "@id")]
    pub at_id: String,
    #[serde(rename = "@type", default)]
    pub at_type: String,
    #[serde(flatten)]
    pub fields: Map<String, Value>,
}

impl Element {
    /// The element's `@id`.
    pub fn id(&self) -> &str {
        &self.at_id
    }

    /// The element's `@type` (KerML/SysML v2 metaclass name, e.g. `"PartUsage"`).
    pub fn ty(&self) -> &str {
        &self.at_type
    }

    /// A non-`@id`/`@type` field by name.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.fields.get(key)
    }

    /// The element's `name` field, when present and a string.
    pub fn name(&self) -> Option<&str> {
        self.fields.get("name").and_then(Value::as_str)
    }
}

/// Page request for `elements(..)`. Maps to the OMG PSM query params
/// `page-after` / `page-before` / `page-size`.
#[derive(Debug, Clone, Default)]
pub struct Page {
    pub after: Option<String>,
    pub before: Option<String>,
    pub size: Option<u32>,
}

impl Page {
    /// A first page of the given size.
    pub fn first(size: u32) -> Self {
        Self {
            size: Some(size),
            ..Self::default()
        }
    }

    /// A page starting after `cursor`.
    pub fn after(cursor: impl Into<String>) -> Self {
        Self {
            after: Some(cursor.into()),
            ..Self::default()
        }
    }
}

/// One page of elements plus the cursor to fetch the next page, if any.
///
/// `next_after` is derived from a `Link: rel="next"` header when the server sends one,
/// otherwise from a full-vs-short page heuristic — see [`crate`] docs and
/// `paging::derive_next_after`.
#[derive(Debug, Clone)]
pub struct ElementPage {
    pub items: Vec<Element>,
    pub next_after: Option<String>,
}

/// Direction filter for `relationships(..)` → `?direction=in|out|both`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    In,
    Out,
    Both,
}

impl Direction {
    pub(crate) fn as_query(self) -> &'static str {
        match self {
            Direction::In => "in",
            Direction::Out => "out",
            Direction::Both => "both",
        }
    }
}

/// A content-hashed snapshot of one project's model at one commit — the unit kr0ki's
/// model-ingestion path (PRD-KR0KI-001 FR1/FR4) consumes.
///
/// Flexo and SysON expose **no** server-side content hash or ETag, so `content_hash`
/// is derived client-side by `compute_content_hash` and becomes kr0ki's model-side
/// cache key (PRD FR5).
#[derive(Debug, Clone, Serialize)]
pub struct ModelSnapshot {
    pub project_id: String,
    pub commit_id: String,
    pub elements: Vec<Element>,
    pub roots: Vec<String>,
    pub content_hash: String,
}
