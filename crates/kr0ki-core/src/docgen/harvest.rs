use std::path::Path;

use anyhow::Context;
use syn::{visit::Visit, Attribute};

use super::{Symbol, SymbolKind};

/// Walk `root` recursively, parse every `.rs` file, and collect public symbols.
pub fn walk_and_harvest(root: &Path) -> anyhow::Result<Vec<Symbol>> {
    let mut symbols = Vec::new();
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
        let relative = path
            .strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string();
        harvest_file(&file, &relative, &source, &mut symbols);
    }
    Ok(symbols)
}

fn harvest_file(file: &syn::File, relative_path: &str, source: &str, out: &mut Vec<Symbol>) {
    let mut visitor = SymbolVisitor {
        module_path: Vec::new(),
        relative_path: relative_path.to_string(),
        source,
        symbols: out,
    };
    visitor.visit_file(file);
}

struct SymbolVisitor<'a> {
    module_path: Vec<String>,
    relative_path: String,
    #[allow(dead_code)]
    source: &'a str,
    symbols: &'a mut Vec<Symbol>,
}

impl<'a> Visit<'_> for SymbolVisitor<'a> {
    fn visit_item_mod(&mut self, node: &syn::ItemMod) {
        let name = node.ident.to_string();
        self.module_path.push(name.clone());
        if is_public(&node.vis) {
            self.push_symbol(
                &name,
                SymbolKind::Mod,
                format!("pub mod {}", name),
                "".to_string(),
                &node.attrs,
            );
        }
        syn::visit::visit_item_mod(self, node);
        self.module_path.pop();
    }

    fn visit_item_fn(&mut self, node: &syn::ItemFn) {
        let name = node.sig.ident.to_string();
        let mut sig = quote_snippet(&node.sig);
        if is_public(&node.vis) {
            sig = format!("pub {sig}");
        }
        let ret = quote_snippet(&node.sig.output);
        if is_public(&node.vis) {
            self.push_symbol(&name, SymbolKind::Function, sig, ret, &node.attrs);
        }
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_item_struct(&mut self, node: &syn::ItemStruct) {
        let name = node.ident.to_string();
        let sig = quote_snippet(node);
        if is_public(&node.vis) {
            self.push_symbol(&name, SymbolKind::Struct, sig, "".to_string(), &node.attrs);
        }
        syn::visit::visit_item_struct(self, node);
    }

    fn visit_item_enum(&mut self, node: &syn::ItemEnum) {
        let name = node.ident.to_string();
        let sig = quote_snippet(node);
        if is_public(&node.vis) {
            self.push_symbol(&name, SymbolKind::Enum, sig, "".to_string(), &node.attrs);
        }
        syn::visit::visit_item_enum(self, node);
    }

    fn visit_item_trait(&mut self, node: &syn::ItemTrait) {
        let name = node.ident.to_string();
        let sig = quote_snippet(node);
        if is_public(&node.vis) {
            self.push_symbol(&name, SymbolKind::Trait, sig, "".to_string(), &node.attrs);
        }
        syn::visit::visit_item_trait(self, node);
    }

    fn visit_item_type(&mut self, node: &syn::ItemType) {
        let name = node.ident.to_string();
        let sig = quote_snippet(node);
        if is_public(&node.vis) {
            self.push_symbol(&name, SymbolKind::Type, sig, "".to_string(), &node.attrs);
        }
        syn::visit::visit_item_type(self, node);
    }

    fn visit_item_const(&mut self, node: &syn::ItemConst) {
        let name = node.ident.to_string();
        let sig = quote_snippet(node);
        if is_public(&node.vis) {
            self.push_symbol(&name, SymbolKind::Const, sig, "".to_string(), &node.attrs);
        }
        syn::visit::visit_item_const(self, node);
    }

    fn visit_item_static(&mut self, node: &syn::ItemStatic) {
        let name = &node.ident.to_string();
        let sig = quote_snippet(node);
        if is_public(&node.vis) {
            self.push_symbol(name, SymbolKind::Static, sig, "".to_string(), &node.attrs);
        }
        syn::visit::visit_item_static(self, node);
    }
}

impl<'a> SymbolVisitor<'a> {
    fn qualified_name(&self, name: &str) -> String {
        let mut path = self.module_path.clone();
        path.push(name.to_string());
        path.join("::")
    }

    fn push_symbol(
        &mut self,
        name: &str,
        kind: SymbolKind,
        signature: String,
        return_type: String,
        attrs: &[Attribute],
    ) {
        let docstring = extract_docs(attrs);
        // Complexity heuristic: number of lines in the docstring + signature
        let complexity = Some(docstring.lines().count() as u64 + signature.lines().count() as u64);
        self.symbols.push(Symbol {
            name: name.to_string(),
            qualified_name: self.qualified_name(name),
            file_path: self.relative_path.clone(),
            kind,
            signature,
            return_type: return_type.to_string(),
            start_line: None,
            complexity,
            docstring,
        });
    }
}

fn is_public(vis: &syn::Visibility) -> bool {
    matches!(vis, syn::Visibility::Public(_))
}

fn extract_docs(attrs: &[syn::Attribute]) -> String {
    let mut lines = Vec::new();
    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let syn::Meta::NameValue(nv) = &attr.meta {
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(lit),
                    ..
                }) = &nv.value
                {
                    lines.push(lit.value());
                }
            }
        }
    }
    lines.join("\n")
}

/// Render a syn node back to source-like string for display.
fn quote_snippet<T: quote::ToTokens>(node: &T) -> String {
    let tokens = node.to_token_stream();
    let s = tokens.to_string();
    let cleaned = strip_outer_attrs(&s);
    // Collapse excessive whitespace for readability
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Strip `#[...]` and `# [...]` attribute noise from a token string.
fn strip_outer_attrs(s: &str) -> String {
    let mut out = s.to_string();
    while let Some(start) = out.find("# [") {
        let Some(end) = out[start..].find("]") else {
            break;
        };
        out.drain(start..start + end + 1);
    }
    // Also handle the no-space form: `#[...]`
    while let Some(start) = out.find("#[") {
        let Some(end) = out[start..].find("]") else {
            break;
        };
        out.drain(start..start + end + 1);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harvests_simple_function() {
        let source = r#"
/// Start the engine.
pub fn start() -> bool { true }
"#;
        let file = syn::parse_file(source).unwrap();
        let mut syms = Vec::new();
        harvest_file(&file, "test.rs", source, &mut syms);
        assert_eq!(syms.len(), 1);
        let s = &syms[0];
        assert_eq!(s.name, "start");
        assert_eq!(s.kind, SymbolKind::Function);
        assert!(s.docstring.contains("Start the engine"));
        assert!(
            s.signature.contains("pub fn start"),
            "signature was: {}",
            s.signature
        );
        assert_eq!(s.start_line, None);
    }

    #[test]
    fn skips_private_items() {
        let source = r#"
fn hidden() {}
pub fn visible() {}
"#;
        let file = syn::parse_file(source).unwrap();
        let mut syms = Vec::new();
        harvest_file(&file, "test.rs", source, &mut syms);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "visible");
    }

    #[test]
    fn harvest_workspace_returns_symbols() {
        let syms = crate::docgen::harvest_kr0ki_workspace().unwrap();
        assert!(!syms.is_empty(), "expected at least one symbol, got 0");
    }

    #[test]
    fn harvests_struct_with_fields() {
        let source = r#"
/// A point in 2D space.
pub struct Point { x: f64, y: f64 }
"#;
        let file = syn::parse_file(source).unwrap();
        let mut syms = Vec::new();
        harvest_file(&file, "test.rs", source, &mut syms);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, SymbolKind::Struct);
        assert!(syms[0].docstring.contains("point in 2D"));
    }
}
