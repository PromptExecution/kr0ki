//! Source evidence contract for behavioral documentation.
//!
//! This IR stores extractor facts, not a replacement semantic ontology. Callers
//! must validate before lifting into `ufo_types::SysGraph` or rendering. Byte
//! offsets always refer to the embedded, hash-checked UTF-8 source text.

mod machine;
mod schema;
mod validation;

pub use machine::{RuntimeContext, RuntimeError, StateMachineRuntime, TransitionTrace};
pub use schema::json_schema;
pub use validation::ValidationError;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RustBehaviorIr {
    pub schema_version: u32,
    pub provenance: Provenance,
    pub sources: Vec<SourceFile>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub diagnostics: Vec<Diagnostic>,
    pub machines: Vec<StateMachine>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub revision: String,
    pub tree_digest: String,
    pub toolchain: String,
    pub extractor: String,
    pub config: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceFile {
    pub path: String,
    pub sha256: String,
    pub content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct Anchor {
    pub file: String,
    pub symbol: String,
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub id: String,
    pub name: String,
    pub kind: NodeKind,
    pub anchor: Option<Anchor>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Module,
    Type,
    Field,
    AssociatedType,
    Trait,
    Function,
    Action,
    Decision,
    Merge,
    Loop,
    Exit,
    Dispatch,
    State,
    External,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Edge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
    pub guard: Option<String>,
    pub resolution: Resolution,
    pub anchor: Anchor,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Contains,
    Satisfies,
    Requires,
    GovernedBy,
    Calls,
    Flow,
    Branch,
    Back,
    Exit,
    Transition,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    Resolved,
    Inferred,
    Unresolved,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub anchor: Option<Anchor>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StateMachine {
    pub id: String,
    pub name: String,
    pub initial: String,
    pub states: Vec<State>,
    pub transitions: Vec<Transition>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub id: String,
    pub terminal: bool,
    pub anchor: Option<Anchor>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    pub id: String,
    pub from: String,
    pub to: String,
    pub event: String,
    pub guard: Option<String>,
    pub effect: Option<String>,
    pub anchor: Option<Anchor>,
}

/// SHA-256 in lowercase hex, for source and semantic content addressing.
pub fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

/// Stable identifier with length-delimited fields to avoid ambiguous joins.
pub fn stable_id(namespace: &str, key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update((namespace.len() as u64).to_be_bytes());
    hasher.update(namespace.as_bytes());
    hasher.update((key.len() as u64).to_be_bytes());
    hasher.update(key.as_bytes());
    format!("{namespace}:{:x}", hasher.finalize())
}

impl RustBehaviorIr {
    /// Parse a strictly typed document, then enforce all cross-field invariants.
    pub fn from_json(source: &str) -> Result<Self, ValidationError> {
        let mut ir: Self = serde_json::from_str(source)?;
        ir.ensure_valid()?;
        ir.normalize();
        Ok(ir)
    }

    /// Produce deterministic JSON; invalid evidence never becomes an artifact.
    pub fn canonical_json(&self) -> Result<String, ValidationError> {
        self.ensure_valid()?;
        let mut ir = self.clone();
        ir.normalize();
        Ok(serde_json::to_string_pretty(&ir)?)
    }

    /// Sort only sets; this never rewrites transition priority or behavior.
    /// Ambiguous transitions are rejected by the runtime rather than prioritized.
    pub fn normalize(&mut self) {
        self.sources.sort_by(|a, b| a.path.cmp(&b.path));
        self.nodes.sort_by(|a, b| a.id.cmp(&b.id));
        self.edges.sort_by(|a, b| a.id.cmp(&b.id));
        self.diagnostics.sort();
        self.diagnostics.dedup();
        self.machines.sort_by(|a, b| a.id.cmp(&b.id));
        for machine in &mut self.machines {
            machine.states.sort_by(|a, b| a.id.cmp(&b.id));
            machine.transitions.sort_by(|a, b| a.id.cmp(&b.id));
        }
    }
}
