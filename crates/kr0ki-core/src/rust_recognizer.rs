//! The Rust-source pattern recognizer (box 3, Rust arm; `docs/PATTERNS-rust-source.md`;
//! `docs/PLAN-KR0KI-003-rust-source-frontend.md` §4's D7, resolved here — this arm's
//! recognizer lives in kr0ki, mirroring [`crate::k8s_recognizer`]).
//!
//! Normalizes a workspace's Rust source into `ufo_types::iso_ir::{Node, Edge}` — the
//! box-1→box-2 step for the Rust source arm. **Not** `docgen`'s doc-symbol harvest
//! (`crate::docgen::harvest`) and **not** SysML constructs directly: this module's
//! only job is emitting the untyped `iso_ir` transport shape; lifting it into the
//! canonical UFO graph (`ufo_types::ontology`) and then `ufo_types::sysml_model`
//! (`crate::sysml_lift`) are later, separate stages (`PLAN-KR0KI-003.md` §2).
//!
//! ```text
//! Vec<syn::File> ──▶ [THIS] ──▶ (Vec<Node>, Vec<Edge>)
//! ```
//!
//! # Provenance: this crate's own `docgen/harvest.rs`
//!
//! `docs/PLAN-KR0KI-003.md` §4 (D7's resolution and the reuse-vs-clean-room spike)
//! directs generalizing `docgen::harvest`'s `syn::visit::Visit`-based `SymbolVisitor`
//! into a sibling relationship-collecting visitor, rather than reusing
//! `b00t-cli/src/dispatch_sysml.rs` (which turned out not to be a general AST walker
//! at all — see that section for why). This module is that sibling: same traversal
//! shape (walk every `.rs` file, track `module_path` through `ItemMod`), different
//! output (`iso_ir::{Node, Edge}` instead of `docgen::Symbol`). `docgen/` itself is
//! untouched by this module.
//!
//! # What's covered vs. deliberately deferred
//!
//! Per `docs/PATTERNS-rust-source.md` §5 — an intentional first increment, not
//! silently incomplete coverage, matching [`crate::k8s_recognizer`]'s own
//! ported-vs-deferred documentation convention:
//!
//! **Covered:** module containment (a module `has_part` every item declared directly
//! in it, including nested modules); struct/enum field types that resolve, by
//! simple name, to another struct/enum declared anywhere in the walked tree (also
//! `has_part` — see [`PATTERNS-rust-source.md` §2.1] for why a field is composition,
//! not a `requires` dependency); non-generic, non-blanket trait `impl` blocks
//! (`impl Trait for Type` → a `satisfies` edge — see
//! [`RelationshipVisitor::push_trait_impl_edge`] for the exact scope); and direct,
//! same-module, unqualified function calls (a `flows_to` edge per call site — see
//! [`RelationshipVisitor::push_call_edges`]/[`find_call_names`] for the exact scope,
//! and `PATTERNS-rust-source.md` §5 for why "same-module" specifically, not a
//! whole-tree lookup the way field types and trait impls get).
//!
//! Also covered, as two standalone functions rather than part of the
//! `Vec<syn::File> -> (Vec<Node>, Vec<Edge>)` shape above (crate-level, not
//! module/item-level — see their own doc comments): cross-crate `requires`
//! edges ([`cross_crate_requires`]) for a `use` of a workspace-sibling crate,
//! using [`workspace_member_crate_names`] to tell that apart from a use of a
//! genuine external dependency.
//!
//! **Deferred** (needs more than AST pattern-matching, or an unresolved design
//! choice — see `docs/PATTERNS-rust-source.md` §5 for each reason): cross-module/
//! method/trait-dispatch calls and blanket/generic trait `impl` blocks.
//!
//! **Inherited limitation, not new here:** like `SymbolVisitor`, each file's
//! `module_path` starts empty regardless of that file's real position in the crate
//! tree (a `mod pkg;` declaration pointing at `pkg.rs`/`pkg/mod.rs` is not resolved
//! back to a `pkg::` prefix for that file's own top-level items). Only *inline*
//! `mod pkg { ... }` blocks are tracked. Fixing this needs resolving `mod` file
//! declarations against the directory layout — a real capability gap, but the same
//! one `docgen`'s harvest already has, not a regression introduced by this module.
//!
//! # Local-type resolution is a name heuristic, not a type checker
//!
//! A field's type resolves to a local struct/enum by matching its **simple** (last
//! path segment) name against every struct/enum this pass collected across the whole
//! walked tree — not a real compiler type resolution (no `use`-aliasing, no
//! shadowing, no crate-boundary awareness). A field typed `Foo` where two unrelated
//! modules each declare their own `Foo` resolves to whichever this pass saw last;
//! this is a known, accepted limitation of a first slice, not a silent one. A field
//! type that doesn't match any collected local name is simply not lifted (no
//! `has_part` edge) — never an error, mirroring [`crate::ufo_graph`]'s own
//! "unrecognized element stays unlifted" philosophy.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Context;
use ufo_types::iso_ir::{Edge, Node};

/// Walk `root` recursively, parse every `.rs` file, and recognize module
/// containment + struct/enum field-type relationships across the whole tree.
pub fn walk_and_recognize(root: &Path) -> anyhow::Result<(Vec<Node>, Vec<Edge>)> {
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map(|ext| ext == "rs").unwrap_or(false))
    {
        let path = entry.path();
        let source =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let file =
            syn::parse_file(&source).with_context(|| format!("parsing {}", path.display()))?;
        files.push(file);
    }

    let local_types = collect_local_type_names(&files);
    let module_functions = collect_module_functions(&files);

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for file in &files {
        recognize_file(
            file,
            &local_types,
            &module_functions,
            &mut nodes,
            &mut edges,
        );
    }
    Ok((nodes, edges))
}

/// Parse a single source string and recognize it in isolation (local-type
/// resolution only sees structs/enums declared in this same string). Mainly
/// for tests and single-file callers; [`walk_and_recognize`] is the
/// whole-tree entry point real callers want.
pub fn recognize_source(source: &str) -> syn::Result<(Vec<Node>, Vec<Edge>)> {
    let file = syn::parse_file(source)?;
    let files = std::slice::from_ref(&file);
    let local_types = collect_local_type_names(files);
    let module_functions = collect_module_functions(files);
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    recognize_file(
        &file,
        &local_types,
        &module_functions,
        &mut nodes,
        &mut edges,
    );
    Ok((nodes, edges))
}

/// Every `struct`/`enum`/`trait` name declared anywhere in `files`, mapped
/// from its simple (unqualified) name to its full `module::path::Name`
/// qualified name — the field-type resolution table §2.1's module docs
/// describe, and (kr0ki#31 follow-up) trait-impl resolution reuses the same
/// table: a trait or a struct/enum sharing one flat namespace here is the
/// same accepted "whichever this pass saw last" limitation already
/// documented for field types, not a new one.
fn collect_local_type_names(files: &[syn::File]) -> HashMap<String, String> {
    struct Collector<'a> {
        module_path: Vec<String>,
        out: &'a mut HashMap<String, String>,
    }

    impl syn::visit::Visit<'_> for Collector<'_> {
        fn visit_item_mod(&mut self, node: &syn::ItemMod) {
            self.module_path.push(node.ident.to_string());
            syn::visit::visit_item_mod(self, node);
            self.module_path.pop();
        }

        fn visit_item_struct(&mut self, node: &syn::ItemStruct) {
            self.record(&node.ident.to_string());
        }

        fn visit_item_enum(&mut self, node: &syn::ItemEnum) {
            self.record(&node.ident.to_string());
        }

        fn visit_item_trait(&mut self, node: &syn::ItemTrait) {
            self.record(&node.ident.to_string());
        }
    }

    impl Collector<'_> {
        fn record(&mut self, name: &str) {
            let mut path = self.module_path.clone();
            path.push(name.to_string());
            self.out.insert(name.to_string(), path.join("::"));
        }
    }

    let mut out = HashMap::new();
    for file in files {
        let mut collector = Collector {
            module_path: Vec::new(),
            out: &mut out,
        };
        syn::visit::visit_file(&mut collector, file);
    }
    out
}

/// The set of free-function names declared directly in each module
/// (`module qualified path -> {fn names}`) — deliberately *not* a single
/// global table like [`collect_local_type_names`]'s: a call's callee only
/// resolves against functions in the *same module* as the call site
/// (`PATTERNS-rust-source.md` §5's call-graph scope decision). Rust
/// guarantees at most one `fn` of a given name per module scope, so this
/// bound eliminates the cross-module name-collision risk a global lookup
/// would have for short, common function names (`new`, `parse`, `run`) —
/// unlike [`collect_local_type_names`]'s table, which accepts that risk for
/// type names because they collide far less often in practice.
fn collect_module_functions(files: &[syn::File]) -> HashMap<String, HashSet<String>> {
    struct Collector<'a> {
        module_path: Vec<String>,
        out: &'a mut HashMap<String, HashSet<String>>,
    }

    impl syn::visit::Visit<'_> for Collector<'_> {
        fn visit_item_mod(&mut self, node: &syn::ItemMod) {
            self.module_path.push(node.ident.to_string());
            syn::visit::visit_item_mod(self, node);
            self.module_path.pop();
        }

        fn visit_item_fn(&mut self, node: &syn::ItemFn) {
            // No recursion into the body: a fn nested inside another fn's
            // body isn't a call target this first slice resolves against.
            self.out
                .entry(self.module_path.join("::"))
                .or_default()
                .insert(node.sig.ident.to_string());
        }
    }

    let mut out = HashMap::new();
    for file in files {
        let mut collector = Collector {
            module_path: Vec::new(),
            out: &mut out,
        };
        syn::visit::visit_file(&mut collector, file);
    }
    out
}

fn recognize_file(
    file: &syn::File,
    local_types: &HashMap<String, String>,
    module_functions: &HashMap<String, HashSet<String>>,
    nodes: &mut Vec<Node>,
    edges: &mut Vec<Edge>,
) {
    let mut visitor = RelationshipVisitor {
        module_path: Vec::new(),
        local_types,
        module_functions,
        nodes,
        edges,
    };
    syn::visit::visit_file(&mut visitor, file);
}

struct RelationshipVisitor<'a> {
    module_path: Vec<String>,
    local_types: &'a HashMap<String, String>,
    module_functions: &'a HashMap<String, HashSet<String>>,
    nodes: &'a mut Vec<Node>,
    edges: &'a mut Vec<Edge>,
}

impl syn::visit::Visit<'_> for RelationshipVisitor<'_> {
    fn visit_item_mod(&mut self, node: &syn::ItemMod) {
        let name = node.ident.to_string();
        let parent = self.qualified(&self.module_path.clone());
        self.module_path.push(name.clone());
        let this = self.qualified(&self.module_path.clone());

        self.push_node(&this, "module");
        if !parent.is_empty() {
            self.push_has_part(&parent, &this);
        }

        syn::visit::visit_item_mod(self, node);
        self.module_path.pop();
    }

    fn visit_item_struct(&mut self, node: &syn::ItemStruct) {
        self.declare_item(&node.ident.to_string(), "struct");
        let qname = self.qualified_name(&node.ident.to_string());
        for field in &node.fields {
            self.push_field_edges(&qname, field);
        }
        syn::visit::visit_item_struct(self, node);
    }

    fn visit_item_enum(&mut self, node: &syn::ItemEnum) {
        self.declare_item(&node.ident.to_string(), "enum");
        let qname = self.qualified_name(&node.ident.to_string());
        for variant in &node.variants {
            for field in &variant.fields {
                self.push_field_edges(&qname, field);
            }
        }
        syn::visit::visit_item_enum(self, node);
    }

    fn visit_item_trait(&mut self, node: &syn::ItemTrait) {
        self.declare_item(&node.ident.to_string(), "trait");
        syn::visit::visit_item_trait(self, node);
    }

    fn visit_item_fn(&mut self, node: &syn::ItemFn) {
        let name = node.sig.ident.to_string();
        self.declare_item(&name, "function");
        self.push_call_edges(&name, node);
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_item_impl(&mut self, node: &syn::ItemImpl) {
        self.push_trait_impl_edge(node);
        syn::visit::visit_item_impl(self, node);
    }
}

impl RelationshipVisitor<'_> {
    fn qualified(&self, path: &[String]) -> String {
        path.join("::")
    }

    fn qualified_name(&self, name: &str) -> String {
        let mut path = self.module_path.clone();
        path.push(name.to_string());
        path.join("::")
    }

    /// Register `name` (of `part_type`) as a node and a `has_part` edge from
    /// the enclosing module — every item declared inside a module is part
    /// of it, regardless of the item's own visibility (§2 covers
    /// containment universally; visibility is a separate, unrelated axis
    /// `docgen`'s `is_public` filter already owns for its own purpose).
    fn declare_item(&mut self, name: &str, part_type: &str) {
        let qname = self.qualified_name(name);
        self.push_node(&qname, part_type);
        let module = self.qualified(&self.module_path);
        if !module.is_empty() {
            self.push_has_part(&module, &qname);
        }
    }

    fn push_node(&mut self, id: &str, part_type: &str) {
        if self.nodes.iter().any(|n| n.id == id) {
            return;
        }
        self.nodes.push(Node {
            id: id.to_string(),
            label: id.to_string(),
            part_type: part_type.to_string(),
        });
    }

    fn push_edge(&mut self, from: &str, to: &str, edge_type: &str) {
        self.edges.push(Edge {
            id: format!("{from}_{edge_type}_{to}"),
            from: from.to_string(),
            to: to.to_string(),
            edge_type: edge_type.to_string(),
            kind: None,
        });
    }

    fn push_has_part(&mut self, owner: &str, member: &str) {
        self.push_edge(owner, member, "has_part");
    }

    /// A struct/enum-variant field whose type resolves (by simple name) to
    /// another locally-declared struct/enum becomes a `has_part` edge from
    /// `owner_qname` to that type's own qualified name. A field with no
    /// name (a tuple-struct/tuple-variant field) still participates —
    /// `syn::Fields` iteration yields its type either way, and this
    /// relationship doesn't need the field's own name, only its type.
    fn push_field_edges(&mut self, owner_qname: &str, field: &syn::Field) {
        let mut candidates = Vec::new();
        collect_type_names(&field.ty, &mut candidates);
        for candidate in candidates {
            if let Some(target_qname) = self.local_types.get(&candidate) {
                if target_qname != owner_qname {
                    self.push_has_part(owner_qname, target_qname);
                }
            }
        }
    }

    /// `impl Trait for Type` → a `satisfies` edge, Type → Trait
    /// (`PATTERNS-rust-source.md` §2: "the type meets the trait's
    /// contract"). Scoped to the unambiguous case only
    /// (`PATTERNS-rust-source.md` §5's stated reason for deferring this):
    /// the `impl` itself carries no generic parameters, and the
    /// implementing type is a plain named path — `impl<T> Trait for
    /// Foo<T>` and blanket impls like `impl<T: Bound> Trait for Vec<T>`
    /// have no decided edge shape yet and are silently skipped, not
    /// guessed at (mirrors `sysml_lift`'s own "no confident KerML fit"
    /// fallback philosophy, minus a `Domain`-style escape hatch this
    /// box-1→2 stage doesn't have). An inherent impl (`impl Type { .. }`,
    /// no trait) contributes nothing — there is no trait to satisfy.
    ///
    /// Both endpoints resolve through the same whole-tree `local_types`
    /// name table field-type resolution already uses (traits included,
    /// per `collect_local_type_names`'s doc comment); an endpoint with no
    /// local match falls back to its bare simple name, since a real
    /// external type/trait (`impl std::fmt::Debug for Foo`) is still worth
    /// recording as a node even though this module has no way to give it
    /// a qualified path of its own.
    fn push_trait_impl_edge(&mut self, node: &syn::ItemImpl) {
        if !node.generics.params.is_empty() {
            return;
        }
        let Some((_, trait_path, _)) = &node.trait_ else {
            return;
        };
        let Some(trait_name) = trait_path.segments.last().map(|s| s.ident.to_string()) else {
            return;
        };
        let syn::Type::Path(self_type_path) = node.self_ty.as_ref() else {
            return;
        };
        let Some(type_name) = self_type_path
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
        else {
            return;
        };

        let type_qname = self
            .local_types
            .get(&type_name)
            .cloned()
            .unwrap_or(type_name);
        let trait_qname = self
            .local_types
            .get(&trait_name)
            .cloned()
            .unwrap_or(trait_name);

        self.push_node(&type_qname, "type");
        self.push_node(&trait_qname, "trait");
        self.push_edge(&type_qname, &trait_qname, "satisfies");
    }

    /// Direct, same-module, unqualified calls only
    /// (`PATTERNS-rust-source.md` §5's call-graph scope decision) → one
    /// `flows_to` edge per recognized call site, caller function → callee
    /// function. See [`find_call_names`] for exactly what counts as a
    /// recognized call (a bare `foo()`, not `self.foo()` / `Type::foo()` /
    /// `module::foo()`), and [`collect_module_functions`] for why the
    /// callee must be declared in the *same* module as the call site.
    fn push_call_edges(&mut self, fn_name: &str, node: &syn::ItemFn) {
        let module_key = self.qualified(&self.module_path);
        let Some(local_fns) = self.module_functions.get(&module_key) else {
            return;
        };
        let caller_qname = self.qualified_name(fn_name);
        for callee_name in find_call_names(&node.block) {
            if !local_fns.contains(&callee_name) {
                continue;
            }
            let callee_qname = self.qualified_name(&callee_name);
            self.push_edge(&caller_qname, &callee_qname, "flows_to");
        }
    }
}

/// Every unqualified, single-segment call-site callee name in `block` —
/// `foo(..)`, never `self.foo(..)` (that's a method call, a different
/// `syn::Expr` variant entirely), `Type::foo(..)` / `module::foo(..)`
/// (multi-segment path — excluded, since resolving *those* needs real name
/// resolution this module doesn't do), or a qualified-self call
/// (`<T as Trait>::foo()`, excluded via `qself`). Does not recurse into a
/// nested `fn` item's own body — a call inside a function nested within
/// `block` is that inner function's call, not this one's.
fn find_call_names(block: &syn::Block) -> Vec<String> {
    struct CallCollector<'a> {
        out: &'a mut Vec<String>,
    }

    impl syn::visit::Visit<'_> for CallCollector<'_> {
        fn visit_expr_call(&mut self, node: &syn::ExprCall) {
            if let syn::Expr::Path(p) = node.func.as_ref() {
                if p.qself.is_none() && p.path.segments.len() == 1 {
                    self.out.push(p.path.segments[0].ident.to_string());
                }
            }
            syn::visit::visit_expr_call(self, node);
        }

        fn visit_item_fn(&mut self, _node: &syn::ItemFn) {
            // Don't attribute a nested fn item's calls to the enclosing one.
        }
    }

    let mut out = Vec::new();
    let mut collector = CallCollector { out: &mut out };
    syn::visit::visit_block(&mut collector, block);
    out
}

/// Every path-type simple name reachable from `ty` without real type
/// resolution: the type's own last path segment, plus (recursively) any of
/// its angle-bracketed generic type arguments (`Option<Foo>` → `Foo`,
/// `Vec<Box<Bar>>` → `Bar`), and the element type of a reference or array.
/// Tuple types, function-pointer types, and anything else unhandled simply
/// contribute no candidates — a reduced-recall, never-wrong-answer default,
/// matching this module's stated "name heuristic, not a type checker" scope.
fn collect_type_names(ty: &syn::Type, out: &mut Vec<String>) {
    match ty {
        syn::Type::Path(type_path) => {
            if let Some(segment) = type_path.path.segments.last() {
                out.push(segment.ident.to_string());
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    for arg in &args.args {
                        if let syn::GenericArgument::Type(inner) = arg {
                            collect_type_names(inner, out);
                        }
                    }
                }
            }
        }
        syn::Type::Reference(r) => collect_type_names(&r.elem, out),
        syn::Type::Array(a) => collect_type_names(&a.elem, out),
        syn::Type::Paren(p) => collect_type_names(&p.elem, out),
        syn::Type::Group(g) => collect_type_names(&g.elem, out),
        _ => {}
    }
}

// ---------------------------------------------------------------------
// Cross-crate `requires` (PATTERNS-rust-source.md §5's last deferred item)
// ---------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct CargoTomlPackage {
    name: String,
}

#[derive(serde::Deserialize)]
struct CargoTomlWorkspace {
    members: Vec<String>,
}

#[derive(serde::Deserialize, Default)]
struct CargoTomlDoc {
    package: Option<CargoTomlPackage>,
    workspace: Option<CargoTomlWorkspace>,
}

/// Every crate name declared as a `[package] name` of a workspace member
/// listed in `workspace_root/Cargo.toml`'s `[workspace] members`, as the
/// Rust identifier form `use` paths need (hyphens replaced with
/// underscores — Cargo package names commonly use hyphens, `use` paths
/// never can).
///
/// Members are read as literal paths only, not glob patterns
/// (`kr0ki`'s own workspace lists every member explicitly; expanding a glob
/// pattern like `"crates/*"` is unimplemented — a member entry that isn't a
/// real directory with its own `Cargo.toml` is silently skipped, same
/// "never guess, never error on the unrecognized" posture as every other
/// heuristic in this module). Returns an empty set, not an error, for a
/// `Cargo.toml` with no `[workspace]` table (a single, non-workspace crate
/// has no siblings to distinguish from external dependencies).
pub fn workspace_member_crate_names(workspace_root: &Path) -> anyhow::Result<HashSet<String>> {
    let root_manifest = workspace_root.join("Cargo.toml");
    let root_text = std::fs::read_to_string(&root_manifest)
        .with_context(|| format!("reading {}", root_manifest.display()))?;
    let root_doc: CargoTomlDoc = toml::from_str(&root_text)
        .with_context(|| format!("parsing {}", root_manifest.display()))?;
    let Some(workspace) = root_doc.workspace else {
        return Ok(HashSet::new());
    };

    let mut names = HashSet::new();
    for member in &workspace.members {
        let member_manifest = workspace_root.join(member).join("Cargo.toml");
        let Ok(member_text) = std::fs::read_to_string(&member_manifest) else {
            continue;
        };
        let Ok(member_doc) = toml::from_str::<CargoTomlDoc>(&member_text) else {
            continue;
        };
        if let Some(package) = member_doc.package {
            names.insert(package.name.replace('-', "_"));
        }
    }
    Ok(names)
}

/// `use other_crate::Item` importing from a workspace-sibling crate becomes
/// a `requires` edge from `this_crate` to `other_crate`
/// (`PATTERNS-rust-source.md` §2 row 5). An import from a genuine external
/// dependency (anything not in `workspace_crates`) is not recognized — this
/// module has no source for it to place a meaningful node, and telling
/// "interesting to this workspace's own architecture" apart from "just a
/// library dependency" is exactly what `workspace_crates` (from
/// [`workspace_member_crate_names`]) answers. `self`/`super`/`crate` leading
/// segments and a `use` of `this_crate`'s own name are excluded — neither
/// denotes a *different* crate. One edge per distinct target crate, however
/// many `use` sites reference it.
pub fn cross_crate_requires(
    files: &[syn::File],
    this_crate: &str,
    workspace_crates: &HashSet<String>,
) -> Vec<Edge> {
    let mut required = HashSet::new();
    for file in files {
        for name in collect_use_crate_names(file) {
            if name == "self" || name == "super" || name == "crate" || name == this_crate {
                continue;
            }
            if workspace_crates.contains(&name) {
                required.insert(name);
            }
        }
    }
    let mut names: Vec<String> = required.into_iter().collect();
    names.sort_unstable(); // deterministic edge order
    names
        .into_iter()
        .map(|name| Edge {
            id: format!("{this_crate}_requires_{name}"),
            from: this_crate.to_string(),
            to: name,
            edge_type: "requires".to_string(),
            kind: None,
        })
        .collect()
}

/// The leading path segment of every top-level `use` tree in `file` — the
/// crate name a `use` statement imports from, before any real name
/// resolution (`self`/`super`/`crate`/an aliased re-export are not filtered
/// here; [`cross_crate_requires`] does that, since what counts as "not a
/// different crate" is that function's concern, not this collector's).
fn collect_use_crate_names(file: &syn::File) -> Vec<String> {
    fn walk(tree: &syn::UseTree, out: &mut Vec<String>) {
        match tree {
            syn::UseTree::Path(p) => out.push(p.ident.to_string()),
            // `use { foo::A, bar::B };` — a top-level group has no single
            // leading segment of its own; recurse into each branch.
            syn::UseTree::Group(g) => {
                for item in &g.items {
                    walk(item, out);
                }
            }
            // `use Foo;` / `use Foo as Bar;` / `use *;` with no leading
            // path segment at all — nothing to extract.
            syn::UseTree::Name(_) | syn::UseTree::Rename(_) | syn::UseTree::Glob(_) => {}
        }
    }

    let mut out = Vec::new();
    for item in &file.items {
        if let syn::Item::Use(item_use) = item {
            walk(&item_use.tree, &mut out);
        }
    }
    out
}

// ---------------------------------------------------------------------
// Box 1 -> box 2: iso_ir -> the canonical UFO semantic graph (SysGraph)
// ---------------------------------------------------------------------

/// Lift this recognizer's own `iso_ir::{Node, Edge}` output into
/// `ufo_types::sysgraph::SysGraph` — the box-2 typed envelope
/// (`docs/TODO.md` box 2's "Graph container type", `ufo-types` `SysGraph`).
/// The Kubernetes arm (`k8s_recognizer.rs`) and SysML-v2 arm (`ufo_graph.rs`)
/// go straight from their own raw source to `OntologicalEdge` with no
/// `iso_ir` step in between (their sources already speak a structured
/// vocabulary `iso_ir`'s free-form strings don't add anything over); this
/// arm's `Node`/`Edge` *are* the free-form transport shape `iso_ir` exists
/// for, so it needs this explicit lift where the other two arms don't.
///
/// `part_type`/`edge_type` are the exact strings this module's own
/// `RelationshipVisitor` emits (`"module"`/`"struct"`/`"enum"`/`"trait"`/
/// `"function"`/`"type"`, and `"has_part"`/`"satisfies"`/`"flows_to"`) — the
/// stereotype table is `PATTERNS-rust-source.md` §3 verbatim (this function
/// is that table's implementation, previously undone); the relation parse
/// reuses `UfoRelation`'s own `FromStr`, since every edge type this module
/// emits already *is* one of `UfoRelation`'s canonical names.
///
/// Total: an unrecognized `part_type`/`edge_type` (only possible if this
/// module starts emitting a new one without updating this function) falls
/// back to [`UfoStereotype::Kind`] / is silently dropped respectively,
/// rather than panicking — this is a lift, not a parser with a failure mode
/// callers need to handle.
pub fn to_sysgraph(nodes: &[Node], edges: &[Edge]) -> ufo_types::sysgraph::SysGraph {
    use ufo_types::ontology::OntologicalEdge;
    use ufo_types::stereotype::UfoStereotype;
    use ufo_types::sysgraph::{OntologicalNode, SysGraph};
    use ufo_types::sysml_model::ElementId;

    let mut graph = SysGraph::new();
    for node in nodes {
        let stereotype = match node.part_type.as_str() {
            "module" | "struct" | "enum" | "function" | "type" => {
                UfoStereotype::Kind(node.part_type.clone())
            }
            "trait" => UfoStereotype::Role(node.part_type.clone()),
            other => UfoStereotype::Kind(other.to_string()),
        };
        graph.push_node(OntologicalNode::with_label(
            ElementId::new(node.id.clone()),
            stereotype,
            node.label.clone(),
        ));
    }
    for edge in edges {
        let Ok(relation) = edge.edge_type.parse() else {
            continue;
        };
        graph.push_edge(OntologicalEdge::new(
            edge.id.clone(),
            ElementId::new(edge.from.clone()),
            ElementId::new(edge.to.clone()),
            relation,
        ));
    }
    graph
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_edge(edges: &[Edge], from: &str, to: &str, edge_type: &str) -> bool {
        edges
            .iter()
            .any(|e| e.from == from && e.to == to && e.edge_type == edge_type)
    }

    #[test]
    fn module_declares_has_part_edges_to_its_direct_items() {
        let src = r#"
            mod pkg {
                struct A;
                fn f() {}
            }
        "#;
        let (nodes, edges) = recognize_source(src).unwrap();
        assert!(nodes
            .iter()
            .any(|n| n.id == "pkg" && n.part_type == "module"));
        assert!(nodes
            .iter()
            .any(|n| n.id == "pkg::A" && n.part_type == "struct"));
        assert!(nodes
            .iter()
            .any(|n| n.id == "pkg::f" && n.part_type == "function"));
        assert!(has_edge(&edges, "pkg", "pkg::A", "has_part"));
        assert!(has_edge(&edges, "pkg", "pkg::f", "has_part"));
    }

    #[test]
    fn nested_modules_chain_has_part_from_outer_to_inner() {
        let src = r#"
            mod outer {
                mod inner {
                    struct Leaf;
                }
            }
        "#;
        let (_, edges) = recognize_source(src).unwrap();
        assert!(has_edge(&edges, "outer", "outer::inner", "has_part"));
        assert!(has_edge(
            &edges,
            "outer::inner",
            "outer::inner::Leaf",
            "has_part"
        ));
    }

    #[test]
    fn top_level_items_outside_any_module_get_no_containment_edge() {
        let src = "struct A;";
        let (nodes, edges) = recognize_source(src).unwrap();
        assert!(nodes.iter().any(|n| n.id == "A"));
        assert!(edges.is_empty());
    }

    #[test]
    fn struct_field_of_a_local_struct_type_becomes_has_part() {
        let src = r#"
            struct Engine;
            struct Car { engine: Engine }
        "#;
        let (_, edges) = recognize_source(src).unwrap();
        assert!(has_edge(&edges, "Car", "Engine", "has_part"));
    }

    #[test]
    fn field_type_wrapped_in_option_or_vec_still_resolves_to_the_inner_local_type() {
        let src = r#"
            struct Wheel;
            struct Car {
                spare: Option<Wheel>,
                wheels: Vec<Wheel>,
            }
        "#;
        let (_, edges) = recognize_source(src).unwrap();
        let count = edges
            .iter()
            .filter(|e| e.from == "Car" && e.to == "Wheel" && e.edge_type == "has_part")
            .count();
        // one has_part edge per field that resolved, not deduplicated across
        // fields -- each field is its own relationship, even to the same type.
        assert_eq!(count, 2);
    }

    #[test]
    fn field_of_a_std_or_external_type_produces_no_edge() {
        let src = r#"
            struct Car {
                name: String,
                weight: u32,
            }
        "#;
        let (_, edges) = recognize_source(src).unwrap();
        assert!(edges.is_empty());
    }

    #[test]
    fn enum_variant_fields_are_also_recognized() {
        let src = r#"
            struct Payload;
            enum Message {
                Data(Payload),
                Ping,
            }
        "#;
        let (nodes, edges) = recognize_source(src).unwrap();
        assert!(nodes
            .iter()
            .any(|n| n.id == "Message" && n.part_type == "enum"));
        assert!(has_edge(&edges, "Message", "Payload", "has_part"));
    }

    #[test]
    fn trait_and_function_items_are_registered_as_nodes() {
        let src = r#"
            trait Drive {}
            fn go() {}
        "#;
        let (nodes, _) = recognize_source(src).unwrap();
        assert!(nodes
            .iter()
            .any(|n| n.id == "Drive" && n.part_type == "trait"));
        assert!(nodes
            .iter()
            .any(|n| n.id == "go" && n.part_type == "function"));
    }

    #[test]
    fn nodes_are_deduplicated_by_id() {
        let src = r#"
            mod pkg {
                struct A;
            }
        "#;
        let (nodes, _) = recognize_source(src).unwrap();
        let count = nodes.iter().filter(|n| n.id == "pkg::A").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn trait_impl_for_a_local_type_becomes_satisfies() {
        let src = r#"
            trait Drive {}
            struct Car;
            impl Drive for Car {}
        "#;
        let (_, edges) = recognize_source(src).unwrap();
        assert!(has_edge(&edges, "Car", "Drive", "satisfies"));
    }

    #[test]
    fn trait_impl_nodes_reuse_the_already_declared_node_not_a_duplicate() {
        let src = r#"
            trait Drive {}
            struct Car;
            impl Drive for Car {}
        "#;
        let (nodes, _) = recognize_source(src).unwrap();
        let car_nodes: Vec<_> = nodes.iter().filter(|n| n.id == "Car").collect();
        assert_eq!(car_nodes.len(), 1);
        assert_eq!(car_nodes[0].part_type, "struct");
        let drive_nodes: Vec<_> = nodes.iter().filter(|n| n.id == "Drive").collect();
        assert_eq!(drive_nodes.len(), 1);
        assert_eq!(drive_nodes[0].part_type, "trait");
    }

    #[test]
    fn inherent_impl_with_no_trait_produces_no_satisfies_edge() {
        let src = r#"
            struct Car;
            impl Car {
                fn honk(&self) {}
            }
        "#;
        let (_, edges) = recognize_source(src).unwrap();
        assert!(!edges.iter().any(|e| e.edge_type == "satisfies"));
    }

    #[test]
    fn generic_impl_is_skipped_pending_a_decided_edge_shape() {
        let src = r#"
            trait Wrap {}
            struct Box2<T> { inner: T }
            impl<T> Wrap for Box2<T> {}
        "#;
        let (_, edges) = recognize_source(src).unwrap();
        assert!(!edges.iter().any(|e| e.edge_type == "satisfies"));
    }

    #[test]
    fn blanket_impl_over_an_external_generic_type_is_skipped() {
        let src = r#"
            trait Describe {}
            impl<T: std::fmt::Debug> Describe for Vec<T> {}
        "#;
        let (_, edges) = recognize_source(src).unwrap();
        assert!(!edges.iter().any(|e| e.edge_type == "satisfies"));
    }

    #[test]
    fn trait_impl_for_an_external_type_falls_back_to_its_bare_simple_name() {
        // Debug and String are neither declared in this source -- both
        // endpoints fall back to their unqualified names rather than being
        // dropped, per push_trait_impl_edge's own doc comment.
        let src = "impl std::fmt::Debug for String {}";
        let (nodes, edges) = recognize_source(src).unwrap();
        assert!(has_edge(&edges, "String", "Debug", "satisfies"));
        assert!(nodes
            .iter()
            .any(|n| n.id == "String" && n.part_type == "type"));
        assert!(nodes
            .iter()
            .any(|n| n.id == "Debug" && n.part_type == "trait"));
    }

    #[test]
    fn trait_impl_inside_a_module_resolves_to_qualified_names() {
        let src = r#"
            mod shapes {
                trait Area {}
                struct Circle;
                impl Area for Circle {}
            }
        "#;
        let (_, edges) = recognize_source(src).unwrap();
        assert!(has_edge(
            &edges,
            "shapes::Circle",
            "shapes::Area",
            "satisfies"
        ));
    }

    #[test]
    fn direct_same_module_call_becomes_flows_to() {
        let src = r#"
            fn helper() {}
            fn run() {
                helper();
            }
        "#;
        let (_, edges) = recognize_source(src).unwrap();
        assert!(has_edge(&edges, "run", "helper", "flows_to"));
    }

    #[test]
    fn method_calls_and_qualified_calls_are_not_recognized() {
        let src = r#"
            struct Car;
            impl Car {
                fn honk(&self) {}
                fn start(&self) {
                    self.honk();
                    Car::honk(self);
                }
            }
            fn run() {
                other::helper();
            }
            mod other {
                pub fn helper() {}
            }
        "#;
        let (_, edges) = recognize_source(src).unwrap();
        assert!(!edges.iter().any(|e| e.edge_type == "flows_to"));
    }

    #[test]
    fn calls_across_modules_are_not_recognized_even_with_the_same_simple_name() {
        // The whole point of the same-module scope: two unrelated `parse`
        // functions in different modules must never cross-link, even
        // though a whole-tree name lookup (like local_types uses for
        // field/trait resolution) would have collided them.
        let src = r#"
            mod a {
                pub fn parse() {}
                pub fn run() {
                    parse();
                }
            }
            mod b {
                pub fn parse() {}
            }
        "#;
        let (_, edges) = recognize_source(src).unwrap();
        assert!(has_edge(&edges, "a::run", "a::parse", "flows_to"));
        assert!(!has_edge(&edges, "a::run", "b::parse", "flows_to"));
        assert_eq!(
            edges.iter().filter(|e| e.edge_type == "flows_to").count(),
            1
        );
    }

    #[test]
    fn calls_inside_a_nested_fn_item_are_not_attributed_to_the_outer_function() {
        let src = r#"
            fn helper() {}
            fn outer() {
                fn inner() {
                    helper();
                }
            }
        "#;
        let (_, edges) = recognize_source(src).unwrap();
        assert!(!has_edge(&edges, "outer", "helper", "flows_to"));
        assert!(has_edge(&edges, "inner", "helper", "flows_to"));
    }

    #[test]
    fn calls_inside_a_closure_are_attributed_to_the_enclosing_function() {
        let src = r#"
            fn helper() {}
            fn run() {
                let f = || helper();
                f();
            }
        "#;
        let (_, edges) = recognize_source(src).unwrap();
        assert!(has_edge(&edges, "run", "helper", "flows_to"));
    }

    #[test]
    fn a_call_to_an_unrecognized_name_is_silently_skipped() {
        let src = r#"
            fn run() {
                std::process::exit(0);
                println!("hi");
            }
        "#;
        let (_, edges) = recognize_source(src).unwrap();
        assert!(!edges.iter().any(|e| e.edge_type == "flows_to"));
    }

    fn parse_all(sources: &[&str]) -> Vec<syn::File> {
        sources
            .iter()
            .map(|s| syn::parse_file(s).unwrap())
            .collect()
    }

    #[test]
    fn use_of_a_workspace_sibling_crate_becomes_requires() {
        let files = parse_all(&["use other_crate::Thing;"]);
        let workspace_crates: HashSet<String> = ["other_crate".to_string()].into_iter().collect();
        let edges = cross_crate_requires(&files, "this_crate", &workspace_crates);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].from, "this_crate");
        assert_eq!(edges[0].to, "other_crate");
        assert_eq!(edges[0].edge_type, "requires");
    }

    #[test]
    fn use_of_a_genuine_external_dependency_is_not_recognized() {
        let files = parse_all(&["use serde::Serialize;", "use tokio::spawn;"]);
        let workspace_crates: HashSet<String> = ["other_crate".to_string()].into_iter().collect();
        let edges = cross_crate_requires(&files, "this_crate", &workspace_crates);
        assert!(edges.is_empty());
    }

    #[test]
    fn use_of_self_super_crate_and_the_current_crate_itself_are_excluded() {
        let files = parse_all(&[
            "use self::inner::Thing;",
            "use super::Other;",
            "use crate::local::Item;",
            "use this_crate::AlsoLocal;",
        ]);
        let workspace_crates: HashSet<String> = ["this_crate".to_string()].into_iter().collect();
        let edges = cross_crate_requires(&files, "this_crate", &workspace_crates);
        assert!(edges.is_empty());
    }

    #[test]
    fn grouped_use_statements_are_all_recognized() {
        let files = parse_all(&["use {other_crate::A, another_crate::B};"]);
        let workspace_crates: HashSet<String> =
            ["other_crate".to_string(), "another_crate".to_string()]
                .into_iter()
                .collect();
        let mut edges = cross_crate_requires(&files, "this_crate", &workspace_crates);
        edges.sort_by(|a, b| a.to.cmp(&b.to));
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].to, "another_crate");
        assert_eq!(edges[1].to, "other_crate");
    }

    #[test]
    fn multiple_use_sites_of_the_same_crate_produce_one_edge() {
        let files = parse_all(&[
            "use other_crate::A;",
            "use other_crate::B;",
            "use other_crate::c::D;",
        ]);
        let workspace_crates: HashSet<String> = ["other_crate".to_string()].into_iter().collect();
        let edges = cross_crate_requires(&files, "this_crate", &workspace_crates);
        assert_eq!(edges.len(), 1);
    }

    #[test]
    fn workspace_member_crate_names_reads_kr0kis_own_workspace() {
        // Dogfoods this function against the real repository: proves it
        // reads an actual Cargo workspace correctly, not just a synthetic
        // fixture. CARGO_MANIFEST_DIR is crates/kr0ki-core; the workspace
        // root is one level up.
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();
        let names = workspace_member_crate_names(workspace_root).unwrap();
        assert!(names.contains("kr0ki_core"));
        assert!(names.contains("kr0ki_server"));
        assert!(names.contains("kr0ki_sysmlv2_client"));
    }

    #[test]
    fn cross_crate_requires_dogfoods_against_kr0kis_own_ufo_graph_rs() {
        // ufo_graph.rs genuinely does `use kr0ki_sysmlv2_client::{...}` --
        // proves the whole pipeline (real Cargo.toml -> real source file)
        // end to end, not just synthetic snippets.
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();
        let workspace_crates = workspace_member_crate_names(workspace_root).unwrap();

        let ufo_graph_source =
            std::fs::read_to_string(manifest_dir.join("src/ufo_graph.rs")).unwrap();
        let files = parse_all(&[&ufo_graph_source]);
        let edges = cross_crate_requires(&files, "kr0ki_core", &workspace_crates);
        assert!(edges
            .iter()
            .any(|e| e.from == "kr0ki_core" && e.to == "kr0ki_sysmlv2_client"));
    }

    #[test]
    fn to_sysgraph_round_trips_nodes_and_edges_with_no_dangling_endpoints() {
        let src = r#"
            trait Drive {}
            struct Engine;
            struct Car { engine: Engine }
            impl Drive for Car {}
            fn start() {}
            fn run() {
                start();
            }
        "#;
        let (nodes, edges) = recognize_source(src).unwrap();
        let graph = to_sysgraph(&nodes, &edges);

        assert_eq!(graph.nodes.len(), nodes.len());
        assert_eq!(graph.edges.len(), edges.len());
        assert!(
            graph.dangling_edges().is_empty(),
            "every edge endpoint should resolve to a node: {:?}",
            graph.dangling_edges()
        );
    }

    #[test]
    fn to_sysgraph_relation_kinds_match_the_recognized_edge_types() {
        use ufo_types::ontology::UfoRelation;

        let src = r#"
            trait Drive {}
            struct Engine;
            struct Car { engine: Engine }
            impl Drive for Car {}
        "#;
        let (nodes, edges) = recognize_source(src).unwrap();
        let graph = to_sysgraph(&nodes, &edges);

        let has_part_count = graph
            .edges
            .iter()
            .filter(|e| e.relation == UfoRelation::HasPart)
            .count();
        let satisfies_count = graph
            .edges
            .iter()
            .filter(|e| e.relation == UfoRelation::Satisfies)
            .count();
        assert!(has_part_count >= 1, "expected at least one has_part edge");
        assert_eq!(satisfies_count, 1, "expected exactly one satisfies edge");
    }

    #[test]
    fn to_sysgraph_gives_every_node_a_stereotype_matching_patterns_rust_source_md() {
        use ufo_types::stereotype::UfoStereotype;

        let src = r#"
            mod pkg {
                trait Drive {}
                struct Car;
                fn go() {}
            }
        "#;
        let (nodes, edges) = recognize_source(src).unwrap();
        let graph = to_sysgraph(&nodes, &edges);

        let stereotype_of = |id: &str| {
            graph
                .node(&ufo_types::sysml_model::ElementId::new(id))
                .map(|n| n.stereotype.clone())
                .unwrap_or_else(|| panic!("no node for {id}"))
        };
        assert!(matches!(stereotype_of("pkg"), UfoStereotype::Kind(_)));
        assert!(matches!(stereotype_of("pkg::Car"), UfoStereotype::Kind(_)));
        assert!(matches!(stereotype_of("pkg::go"), UfoStereotype::Kind(_)));
        assert!(matches!(
            stereotype_of("pkg::Drive"),
            UfoStereotype::Role(_)
        ));
    }
}
