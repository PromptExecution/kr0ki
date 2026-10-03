//! KR-A02: resolve the links of an assurance thread against a **specified model revision**.
//!
//! A link is only as good as the revision it was checked at: an element that exists at one
//! commit can be gone at the next. Resolution therefore always names the revision it ran
//! against ([`LinkResolution::revision`]) and reports every unresolved link as *dangling*
//! with the reason, instead of assuming it resolves.
//!
//! Three classes of link are resolved, each against the right authority:
//!
//! | class | what it links | resolved against |
//! |---|---|---|
//! | [`LinkClass::Element`] | requirement → satisfying system element | a [`ModelRevisionIndex`] (model revision) |
//! | [`LinkClass::Implementation`] | requirement → enforcing control's implementation path | the repository ([`ArtifactResolver`]) |
//! | [`LinkClass::VerificationTarget`] | requirement → its verification case's test target | the repository ([`ArtifactResolver`]) |
//!
//! A control that records *no* implementation is not a dangling link — it is an implementation
//! gap, reported by `ufo_types::mbse::assurance::analyze`. Dangling means a link was *made* and
//! does not resolve.

use std::collections::BTreeSet;
use std::path::PathBuf;

use kr0ki_sysmlv2_client::ModelSnapshot;
use serde::Serialize;
use ufo_types::mbse::assurance::{NodeKind, NODE_KIND_ATTR};
use ufo_types::mbse::requirements::{
    RelationAuthority, Requirement, RequirementGraph, RequirementRelationKind,
};

use crate::assurance_baseline::sha256_hex;

// ── Model revision index ────────────────────────────────────────────────────────

/// The element locators (qualified names and ids) that exist at one model revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRevisionIndex {
    revision: String,
    locators: BTreeSet<String>,
}

impl ModelRevisionIndex {
    pub fn new(revision: impl Into<String>, locators: impl IntoIterator<Item = String>) -> Self {
        Self {
            revision: revision.into(),
            locators: locators.into_iter().collect(),
        }
    }

    /// The index of a model-server snapshot. The revision is the commit id; locators are each
    /// element's id, `qualifiedName` and `name` (a name alone is only unambiguous for a
    /// root-level element, which is why qualified names are indexed too).
    pub fn from_snapshot(snapshot: &ModelSnapshot) -> Self {
        let mut locators = BTreeSet::new();
        for element in &snapshot.elements {
            locators.insert(element.id().to_string());
            if let Some(name) = element.name() {
                locators.insert(name.to_string());
            }
            if let Some(qualified) = element.get("qualifiedName").and_then(|v| v.as_str()) {
                locators.insert(qualified.to_string());
            }
        }
        Self {
            revision: snapshot.commit_id.clone(),
            locators,
        }
    }

    /// The index of a SysML v2 text, content-addressed: its revision is `sha256:<digest>`.
    ///
    /// This is a *declaration scanner*, not a parser: it reads the names of declarations
    /// (`package P`, `part def D`, `part u : T`, …) and their nesting. It is exact for the
    /// plain declaration forms the assurance model uses and is checked against the real
    /// parser in `tests/assurance_a02.rs`. For an authoritative index of a model that lives on
    /// a server, use [`Self::from_snapshot`].
    pub fn from_sysml_text(text: &str) -> Self {
        Self {
            revision: format!("sha256:{}", sha256_hex(text.as_bytes())),
            locators: scan_declarations(text).into_iter().collect(),
        }
    }

    pub fn revision(&self) -> &str {
        &self.revision
    }

    pub fn contains(&self, locator: &str) -> bool {
        self.locators.contains(locator)
    }

    pub fn locators(&self) -> &BTreeSet<String> {
        &self.locators
    }
}

/// Declaration keywords whose next identifier names an element.
const DECLARATION_KEYWORDS: &[&str] = &[
    "package",
    "part",
    "port",
    "item",
    "attribute",
    "action",
    "state",
    "requirement",
    "constraint",
    "connection",
    "interface",
    "view",
    "viewpoint",
    "rendering",
    "occurrence",
    "enum",
    "calc",
    "case",
    "analysis",
    "verification",
    "concern",
    "allocation",
    "flow",
    "metadata",
    "use",
    "individual",
    "snapshot",
    "timeslice",
];

/// Qualified names of the declarations in `text`, `::`-joined by nesting.
fn scan_declarations(text: &str) -> Vec<String> {
    #[derive(Debug, PartialEq)]
    enum Tok {
        Word(String),
        Open,
        Close,
        Semi,
        Other,
    }

    // Tokenise: drop comments and strings; keep identifiers (bare or 'quoted'), braces, `;`.
    let mut toks = Vec::new();
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
            }
            b'"' => {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i += 1;
                toks.push(Tok::Other);
            }
            b'\'' => {
                let start = i + 1;
                i += 1;
                while i < b.len() && b[i] != b'\'' {
                    i += 1;
                }
                toks.push(Tok::Word(text[start..i.min(b.len())].to_string()));
                i += 1;
            }
            b'{' => {
                toks.push(Tok::Open);
                i += 1;
            }
            b'}' => {
                toks.push(Tok::Close);
                i += 1;
            }
            b';' => {
                toks.push(Tok::Semi);
                i += 1;
            }
            c if c.is_ascii_whitespace() => i += 1,
            c if c.is_ascii_alphabetic() || c == b'_' => {
                let start = i;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                toks.push(Tok::Word(text[start..i].to_string()));
            }
            _ => {
                toks.push(Tok::Other);
                i += 1;
            }
        }
    }

    let mut names = Vec::new();
    let mut stack: Vec<Option<String>> = Vec::new();
    // The element whose body the next `{` opens, if any.
    let mut pending: Option<String> = None;
    let qualified = |stack: &[Option<String>], name: &str| {
        let mut parts: Vec<&str> = stack.iter().flatten().map(String::as_str).collect();
        parts.push(name);
        parts.join("::")
    };
    let mut k = 0;
    while k < toks.len() {
        match &toks[k] {
            Tok::Word(w) if DECLARATION_KEYWORDS.contains(&w.as_str()) => {
                let mut j = k + 1;
                if matches!(toks.get(j), Some(Tok::Word(d)) if d == "def") {
                    j += 1;
                }
                if let Some(Tok::Word(name)) = toks.get(j) {
                    // `part x`, not `part : T` or `part def :> X`; and not a following keyword.
                    if !DECLARATION_KEYWORDS.contains(&name.as_str()) && name != "def" {
                        let q = qualified(&stack, name);
                        names.push(q.clone());
                        pending = Some(name.clone());
                        k = j;
                    }
                }
            }
            Tok::Open => {
                stack.push(pending.take());
            }
            Tok::Close => {
                stack.pop();
                pending = None;
            }
            Tok::Semi => pending = None,
            _ => {}
        }
        k += 1;
    }
    names
}

// ── Artifact resolution ─────────────────────────────────────────────────────────

/// Resolves repository-relative paths.
pub trait ArtifactResolver {
    fn exists(&self, repo_path: &str) -> bool;
}

/// Resolves paths under a repository root. Absolute paths and `..` are refused: a link must
/// point inside the repository.
#[derive(Debug, Clone)]
pub struct DirResolver {
    root: PathBuf,
}

impl DirResolver {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl ArtifactResolver for DirResolver {
    fn exists(&self, repo_path: &str) -> bool {
        let p = std::path::Path::new(repo_path);
        if p.is_absolute()
            || p.components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return false;
        }
        self.root.join(p).is_file()
    }
}

/// `cargo test -p <package> --test <name>` → the package and test target.
/// The only command shape a verification case may declare (see `verification_runner`).
pub fn cargo_test_target(argv: &[String]) -> Option<(String, String)> {
    let a: Vec<&str> = argv.iter().map(String::as_str).collect();
    match a.as_slice() {
        ["cargo", "test", "-p", package, "--test", test] if is_ident(package) && is_ident(test) => {
            Some(((*package).to_string(), (*test).to_string()))
        }
        _ => None,
    }
}

fn is_ident(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

// ── Resolution ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkClass {
    Element,
    Implementation,
    VerificationTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum LinkStatus {
    Resolved,
    Dangling { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedLink {
    pub requirement_id: String,
    pub class: LinkClass,
    /// The thread node the link goes through (element, control or case id).
    pub via: String,
    /// What was looked up: a qualified name or a repository path.
    pub locator: String,
    #[serde(flatten)]
    pub status: LinkStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LinkResolution {
    /// The model revision the element links were resolved against.
    pub revision: String,
    pub links: Vec<ResolvedLink>,
}

impl LinkResolution {
    pub fn dangling(&self) -> impl Iterator<Item = &ResolvedLink> {
        self.links
            .iter()
            .filter(|l| matches!(l.status, LinkStatus::Dangling { .. }))
    }

    pub fn for_requirement<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a ResolvedLink> {
        self.links.iter().filter(move |l| l.requirement_id == id)
    }
}

fn node<'a>(graph: &'a RequirementGraph, id: &str) -> Option<&'a Requirement> {
    graph.requirements.iter().find(|n| n.id == id)
}

fn kind_of(n: &Requirement) -> Option<NodeKind> {
    n.attributes
        .get(NODE_KIND_ATTR)
        .and_then(|k| NodeKind::parse(k))
}

/// Resolve every link of every requirement in `graph`.
///
/// Only asserted edges are followed. Output order is deterministic: by requirement id, then
/// class, then the node id.
pub fn resolve_links(
    graph: &RequirementGraph,
    index: &ModelRevisionIndex,
    artifacts: &dyn ArtifactResolver,
) -> LinkResolution {
    let asserted = |kind: RequirementRelationKind, target: &str, want: NodeKind| {
        let mut ids: Vec<&str> = graph
            .relations
            .iter()
            .filter(|r| {
                matches!(r.authority, RelationAuthority::Asserted)
                    && r.kind == kind
                    && r.target == target
            })
            .map(|r| r.source.as_str())
            .filter(|s| node(graph, s).and_then(kind_of) == Some(want))
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    };

    let mut requirement_ids: Vec<&str> = graph
        .requirements
        .iter()
        .filter(|n| kind_of(n) == Some(NodeKind::Requirement))
        .map(|n| n.id.as_str())
        .collect();
    requirement_ids.sort_unstable();

    let mut links = Vec::new();
    for rid in requirement_ids {
        // Element links, against the model revision.
        for eid in asserted(
            RequirementRelationKind::Satisfies,
            rid,
            NodeKind::SystemElement,
        ) {
            let n = node(graph, eid).expect("filtered to existing nodes");
            let locator = n
                .attributes
                .get("qualified_name")
                .cloned()
                .unwrap_or_else(|| eid.into());
            let status = if index.contains(&locator) {
                LinkStatus::Resolved
            } else {
                LinkStatus::Dangling {
                    reason: format!(
                        "`{locator}` does not exist at model revision {}",
                        index.revision()
                    ),
                }
            };
            links.push(ResolvedLink {
                requirement_id: rid.into(),
                class: LinkClass::Element,
                via: eid.into(),
                locator,
                status,
            });
        }
        // Implementation links, against the repository.
        for cid in asserted(RequirementRelationKind::Implements, rid, NodeKind::Control) {
            let n = node(graph, cid).expect("filtered to existing nodes");
            let recorded = n
                .attributes
                .get("implementation")
                .map(String::as_str)
                .unwrap_or("");
            for path in recorded.split(';').map(str::trim).filter(|p| !p.is_empty()) {
                let status = if artifacts.exists(path) {
                    LinkStatus::Resolved
                } else {
                    LinkStatus::Dangling {
                        reason: format!("`{path}` is not a file in the repository"),
                    }
                };
                links.push(ResolvedLink {
                    requirement_id: rid.into(),
                    class: LinkClass::Implementation,
                    via: cid.into(),
                    locator: path.into(),
                    status,
                });
            }
        }
        // Verification targets, against the repository.
        for vid in asserted(
            RequirementRelationKind::Verifies,
            rid,
            NodeKind::VerificationCase,
        ) {
            let n = node(graph, vid).expect("filtered to existing nodes");
            let argv: Vec<String> = n
                .attributes
                .get("command")
                .and_then(|c| serde_json::from_str(c).ok())
                .unwrap_or_default();
            let (locator, status) = match cargo_test_target(&argv) {
                Some((package, test)) => {
                    let path = format!("crates/{package}/tests/{test}.rs");
                    let status = if artifacts.exists(&path) {
                        LinkStatus::Resolved
                    } else {
                        LinkStatus::Dangling {
                            reason: format!("test target `{path}` does not exist"),
                        }
                    };
                    (path, status)
                }
                None => (
                    argv.join(" "),
                    LinkStatus::Dangling {
                        reason: "the command is not `cargo test -p <package> --test <name>`".into(),
                    },
                ),
            };
            links.push(ResolvedLink {
                requirement_id: rid.into(),
                class: LinkClass::VerificationTarget,
                via: vid.into(),
                locator,
                status,
            });
        }
    }
    LinkResolution {
        revision: index.revision().to_string(),
        links,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scanner_reads_nested_declarations_and_ignores_comments_and_strings() {
        let text = r#"
            // part def Commented;
            /* part def AlsoCommented; */
            package P {
                part def A;
                part def B {
                    part inner : A;
                    attribute label = "part def NotAThing";
                }
                part 'quoted name' : A;
            }
            part def TopLevel;
        "#;
        let names = scan_declarations(text);
        assert_eq!(
            names,
            vec![
                "P",
                "P::A",
                "P::B",
                "P::B::inner",
                "P::B::label",
                "P::quoted name",
                "TopLevel"
            ]
        );
    }

    #[test]
    fn scanner_does_not_name_anonymous_usages_but_still_tracks_their_scope() {
        let names = scan_declarations("package P { part : T { part x; } part y; }");
        assert_eq!(names, vec!["P", "P::x", "P::y"]);
    }

    #[test]
    fn revision_is_content_addressed() {
        let a = ModelRevisionIndex::from_sysml_text("package P { part def A; }");
        let b = ModelRevisionIndex::from_sysml_text("package P { part def A; }");
        let c = ModelRevisionIndex::from_sysml_text("package P { part def B; }");
        assert_eq!(a.revision(), b.revision());
        assert_ne!(a.revision(), c.revision());
        assert!(a.contains("P::A") && !a.contains("P::B"));
        assert!(a.revision().starts_with("sha256:"));
    }

    #[test]
    fn cargo_test_target_accepts_only_the_declared_shape() {
        let ok: Vec<String> = [
            "cargo",
            "test",
            "-p",
            "kr0ki-core",
            "--test",
            "assurance_a01",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(
            cargo_test_target(&ok),
            Some(("kr0ki-core".to_string(), "assurance_a01".to_string()))
        );
        for bad in [
            vec!["sh", "-c", "rm -rf /"],
            vec!["cargo", "test", "-p", "kr0ki-core", "--test", "a; rm -rf /"],
            vec!["cargo", "test", "-p", "../evil", "--test", "x"],
            vec!["cargo", "test", "--workspace"],
            vec![
                "cargo",
                "test",
                "-p",
                "kr0ki-core",
                "--test",
                "x",
                "--",
                "--nocapture",
            ],
        ] {
            let argv: Vec<String> = bad.iter().map(|s| s.to_string()).collect();
            assert_eq!(cargo_test_target(&argv), None, "{bad:?}");
        }
    }

    #[test]
    fn dir_resolver_refuses_paths_outside_the_repository() {
        let r = DirResolver::new(env!("CARGO_MANIFEST_DIR"));
        assert!(r.exists("Cargo.toml"));
        assert!(!r.exists("../../Cargo.toml"));
        assert!(!r.exists("/etc/passwd"));
        assert!(!r.exists("src"), "a directory is not a file artifact");
    }
}
