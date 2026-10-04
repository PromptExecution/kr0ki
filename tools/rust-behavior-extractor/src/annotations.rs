//! Syntax is used only for original attribute bytes. Compiler-confirmed source
//! spans and kinds decide the target; this module never resolves Rust symbols.
use kr0ki_behavior::{Anchor, Diagnostic, NodeKind, RustBehaviorIr, Severity, SourceAnnotation};
use proc_macro2::Span;
use syn::{
    spanned::Spanned,
    visit::{self, Visit},
    Attribute,
};

struct ItemAttrs {
    kind: NodeKind,
    identity: std::ops::Range<usize>,
    attrs: Vec<Attribute>,
}

#[derive(Default)]
struct Collector(Vec<ItemAttrs>);
impl Collector {
    fn add(&mut self, kind: NodeKind, identity: Span, attrs: &[Attribute]) {
        self.0.push(ItemAttrs {
            kind,
            identity: identity.byte_range(),
            attrs: attrs.to_vec(),
        });
    }
}
impl<'ast> Visit<'ast> for Collector {
    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        self.add(NodeKind::Type, item.ident.span(), &item.attrs);
        visit::visit_item_struct(self, item);
    }
    fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
        self.add(NodeKind::Type, item.ident.span(), &item.attrs);
        visit::visit_item_enum(self, item);
    }
    fn visit_item_union(&mut self, item: &'ast syn::ItemUnion) {
        self.add(NodeKind::Type, item.ident.span(), &item.attrs);
        visit::visit_item_union(self, item);
    }
    fn visit_item_type(&mut self, item: &'ast syn::ItemType) {
        self.add(NodeKind::Type, item.ident.span(), &item.attrs);
        visit::visit_item_type(self, item);
    }
    fn visit_item_trait(&mut self, item: &'ast syn::ItemTrait) {
        self.add(NodeKind::Trait, item.ident.span(), &item.attrs);
        visit::visit_item_trait(self, item);
    }
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        self.add(NodeKind::Module, item.ident.span(), &item.attrs);
        visit::visit_item_mod(self, item);
    }
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        self.add(NodeKind::Function, item.sig.ident.span(), &item.attrs);
        visit::visit_item_fn(self, item);
    }
    fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
        self.add(NodeKind::Function, item.sig.ident.span(), &item.attrs);
        visit::visit_trait_item_fn(self, item);
    }
    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        self.add(NodeKind::Function, item.sig.ident.span(), &item.attrs);
        visit::visit_impl_item_fn(self, item);
    }
    fn visit_trait_item_type(&mut self, item: &'ast syn::TraitItemType) {
        self.add(NodeKind::AssociatedType, item.ident.span(), &item.attrs);
        visit::visit_trait_item_type(self, item);
    }
    fn visit_impl_item_type(&mut self, item: &'ast syn::ImplItemType) {
        self.add(NodeKind::AssociatedType, item.ident.span(), &item.attrs);
        visit::visit_impl_item_type(self, item);
    }
    fn visit_field(&mut self, field: &'ast syn::Field) {
        self.add(
            NodeKind::Field,
            field
                .ident
                .as_ref()
                .map(|id| id.span())
                .unwrap_or_else(|| field.ty.span()),
            &field.attrs,
        );
        visit::visit_field(self, field);
    }
}

fn parse_source(content: &str) -> Result<syn::File, syn::Error> {
    // syn::parse_file strips BOM/shebang and shifts offsets. Mask these prefixes
    // using equal-length ASCII bytes, keeping every following byte position.
    let mut bytes = content.as_bytes().to_vec();
    let bom = if content.starts_with('\u{feff}') {
        3
    } else {
        0
    };
    bytes[..bom].fill(b' ');
    let remainder = &content[bom..];
    if remainder.starts_with("#!") && !remainder[2..].trim_start().starts_with('[') {
        let end = content[bom..]
            .find('\n')
            .map(|i| bom + i)
            .unwrap_or(content.len());
        bytes[bom..end].fill(b' ');
    }
    syn::parse_str(&String::from_utf8(bytes).expect("masked prefix is UTF-8"))
}

pub fn attach(ir: &mut RustBehaviorIr, confirmed_definitions: &std::collections::BTreeSet<String>) {
    for source in &ir.sources {
        let ast = match parse_source(&source.content) {
            Ok(ast) => ast,
            Err(_) => {
                ir.diagnostics.push(Diagnostic { code: "unsupported_attribute_syntax".into(), severity: Severity::Warning,
                    message: format!("source annotation parser cannot parse {}; compiler facts are retained, annotation extraction is unsupported", source.path), anchor: None });
                continue;
            }
        };
        let mut collector = Collector::default();
        collector.visit_file(&ast);
        for node in &mut ir.nodes {
            // Imported type/trait references can carry a local use-site span.
            // They are not declarations at that span and cannot inherit attrs.
            if !confirmed_definitions.contains(&node.id) {
                continue;
            }
            let Some(node_anchor) = &node.anchor else {
                continue;
            };
            if node_anchor.file != source.path {
                continue;
            }
            let file_attrs = node.kind == NodeKind::Module
                && node_anchor.start <= 3
                && node_anchor.end as usize >= source.content.trim_end().len()
                && !node.name.contains("::");
            let header_end = source
                .content
                .get(node_anchor.start as usize..node_anchor.end as usize)
                .and_then(|text| text.find('{'))
                .map(|offset| node_anchor.start as usize + offset)
                .unwrap_or(node_anchor.end as usize);
            let matches: Vec<_> = collector
                .0
                .iter()
                .filter(|item| {
                    item.kind == node.kind
                        && !file_attrs
                        && node_anchor.start as usize <= item.identity.start
                        && item.identity.end <= header_end
                })
                .collect();
            if matches.len() > 1 {
                ir.diagnostics.push(Diagnostic {
                    code: "ambiguous_attribute_target".into(),
                    severity: Severity::Warning,
                    message: format!(
                        "original attributes cannot be attached uniquely to compiler symbol {}",
                        node.name
                    ),
                    anchor: Some(node_anchor.clone()),
                });
                continue;
            }
            let attrs = matches
                .first()
                .map(|item| item.attrs.as_slice())
                .unwrap_or(if file_attrs { &ast.attrs } else { &[] });
            for attribute in attrs {
                let range = attribute.span().byte_range();
                let Some(text) = source.content.get(range.clone()) else {
                    ir.diagnostics.push(Diagnostic {
                        code: "invalid_attribute_span".into(),
                        severity: Severity::Error,
                        message: "annotation parser returned an invalid original byte span".into(),
                        anchor: Some(node_anchor.clone()),
                    });
                    continue;
                };
                // Doc comments have a desugared AST attribute but are not an
                // explicit bracketed source attribute. Never synthesize bytes.
                if !text.starts_with("#[") && !text.starts_with("#![") {
                    continue;
                }
                let path = attribute
                    .path()
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::");
                node.annotations.push(SourceAnnotation {
                    path,
                    text: text.to_owned(),
                    anchor: Anchor {
                        file: source.path.clone(),
                        symbol: node.name.clone(),
                        start: range.start.try_into().expect("source fits u32"),
                        end: range.end.try_into().expect("source fits u32"),
                    },
                });
            }
        }
    }
}
