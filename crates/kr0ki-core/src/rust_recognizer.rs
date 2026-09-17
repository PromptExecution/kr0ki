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
//! in it, including nested modules) and struct/enum field types that resolve, by
//! simple name, to another struct/enum declared anywhere in the walked tree (also
//! `has_part` — see [`PATTERNS-rust-source.md` §2.1] for why a field is composition,
//! not a `requires` dependency).
//!
//! **Deferred** (needs more than AST pattern-matching, or an unresolved design
//! choice — see `docs/PATTERNS-rust-source.md` §5 for each reason): the call graph,
//! trait `impl` blocks, and cross-crate `requires` edges.
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

use std::collections::HashMap;
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

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for file in &files {
        recognize_file(file, &local_types, &mut nodes, &mut edges);
    }
    Ok((nodes, edges))
}

/// Parse a single source string and recognize it in isolation (local-type
/// resolution only sees structs/enums declared in this same string). Mainly
/// for tests and single-file callers; [`walk_and_recognize`] is the
/// whole-tree entry point real callers want.
pub fn recognize_source(source: &str) -> syn::Result<(Vec<Node>, Vec<Edge>)> {
    let file = syn::parse_file(source)?;
    let local_types = collect_local_type_names(std::slice::from_ref(&file));
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    recognize_file(&file, &local_types, &mut nodes, &mut edges);
    Ok((nodes, edges))
}

/// Every `struct`/`enum` name declared anywhere in `files`, mapped from its
/// simple (unqualified) name to its full `module::path::Name` qualified
/// name — the field-type resolution table §2.1's module docs describe.
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

fn recognize_file(
    file: &syn::File,
    local_types: &HashMap<String, String>,
    nodes: &mut Vec<Node>,
    edges: &mut Vec<Edge>,
) {
    let mut visitor = RelationshipVisitor {
        module_path: Vec::new(),
        local_types,
        nodes,
        edges,
    };
    syn::visit::visit_file(&mut visitor, file);
}

struct RelationshipVisitor<'a> {
    module_path: Vec<String>,
    local_types: &'a HashMap<String, String>,
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
        self.declare_item(&node.sig.ident.to_string(), "function");
        syn::visit::visit_item_fn(self, node);
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

    fn push_has_part(&mut self, owner: &str, member: &str) {
        self.edges.push(Edge {
            id: format!("{owner}_has_part_{member}"),
            from: owner.to_string(),
            to: member.to_string(),
            edge_type: "has_part".to_string(),
            kind: None,
        });
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
}
