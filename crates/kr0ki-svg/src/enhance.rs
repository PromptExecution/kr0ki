//! Identifier-driven SVG enhancement: the overlay between a rendered diagram and its brand.
//!
//! The diagram source is only the version-controlled base layer. This module takes the SVG a renderer produced, finds
//! each element by its **well-known identifier**, **stamps** it with stable `data-kr0ki-*` attributes, and applies a
//! swappable [`Brand`] (classes, icons, restored display names, CSS) selected with plain CSS selectors over those stamps.
//! Design and findings: `docs/DESIGN-NOTE-skills-identity-and-svg-enhancement.md`.
//!
//! * **Index.** D2 puts an element's key on its `<g>` as an unpadded base64 class name, so for every known id we select
//!   `g.<base64(id)>`. (Other renderers expose ids differently; this is the only renderer-specific step.)
//! * **Edit in place.** Edits are byte-range splices on the original text, so everything the renderer wrote is
//!   preserved exactly and a second run over its own output changes nothing (idempotent).
//! * **Safe to inject.** Brand icons and CSS are operator configuration but are validated anyway: no scripts, event
//!   handlers, `foreignObject`, external references or stylesheet imports. Model-supplied names are XML-escaped.
//! * **Fail loud.** Unsupported selectors are an error, never silently ignored. Supported: an optional tag name,
//!   `.class`, `[attr]` and `[attr="value"]`, combined without combinators (e.g. `g[data-kr0ki-type="PartUsage"]`).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

const ICON_PREFIX: &str = "kr0ki-icon-";
const STYLE_ID: &str = "kr0ki-brand";
const MAX_ICON_BYTES: usize = 16 * 1024;
const MAX_CSS_BYTES: usize = 64 * 1024;

#[derive(Debug, Error, PartialEq)]
pub enum EnhanceError {
    #[error("input is not well-formed SVG/XML: {0}")]
    BadSvg(String),
    #[error("unsupported selector '{0}': supported are an optional tag, .class, [attr] and [attr=\"value\"] without combinators")]
    UnsupportedSelector(String),
    #[error("rule {index} references unknown icon '{icon}'")]
    UnknownIcon { index: usize, icon: String },
    #[error("icon '{name}' is not allowed: {reason}")]
    UnsafeIcon { name: String, reason: String },
    #[error("brand css is not allowed: {0}")]
    UnsafeCss(String),
    #[error("brand.version {0} is not supported (expected 1)")]
    UnsupportedVersion(u32),
}

/// An element the model knows, by its well-known identifier.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KnownElement {
    pub id: String,
    #[serde(rename = "type", default)]
    pub ty: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Icon {
    #[serde(default = "default_view_box", alias = "viewBox")]
    pub view_box: String,
    /// SVG fragment (shapes only) drawn inside the icon's `<symbol>`.
    pub svg: String,
}

fn default_view_box() -> String {
    "0 0 24 24".into()
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LabelMode {
    /// Replace a label that is just the identifier with the element's name.
    Name,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Rule {
    pub select: String,
    #[serde(default)]
    pub class: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub label: Option<LabelMode>,
}

/// A brand package: swap the file, not the diagram source.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Brand {
    #[serde(default = "one")]
    pub version: u32,
    #[serde(default)]
    pub icons: BTreeMap<String, Icon>,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub css: Option<String>,
}

fn one() -> u32 {
    1
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct Report {
    /// Known elements found in the SVG and stamped.
    pub indexed: usize,
    /// Known ids that were not drawn (not in this view, or not supported by the renderer).
    pub unindexed: Vec<String>,
    /// Number of edits made by brand rules; 0 on a second run.
    pub rewrites: usize,
}

// ---------------------------------------------------------------------------------------------------------------
// selectors

#[derive(Debug, PartialEq)]
struct Selector {
    tag: Option<String>,
    classes: Vec<String>,
    attrs: Vec<(String, Option<String>)>,
}

fn parse_selector(input: &str) -> Result<Selector, EnhanceError> {
    let bad = || EnhanceError::UnsupportedSelector(input.to_owned());
    let s = input.trim();
    if s.is_empty() {
        return Err(bad());
    }
    let b = s.as_bytes();
    let mut i = 0;
    let ident = |i: &mut usize| {
        let start = *i;
        while *i < b.len() && (b[*i].is_ascii_alphanumeric() || b[*i] == b'-' || b[*i] == b'_') {
            *i += 1;
        }
        s[start..*i].to_owned()
    };
    let mut sel = Selector {
        tag: None,
        classes: vec![],
        attrs: vec![],
    };
    if b[0].is_ascii_alphabetic() {
        sel.tag = Some(ident(&mut i));
    }
    while i < b.len() {
        match b[i] {
            b'.' => {
                i += 1;
                let c = ident(&mut i);
                if c.is_empty() {
                    return Err(bad());
                }
                sel.classes.push(c);
            }
            b'[' => {
                i += 1;
                let name = ident(&mut i);
                if name.is_empty() {
                    return Err(bad());
                }
                match b.get(i) {
                    Some(b']') => {
                        i += 1;
                        sel.attrs.push((name, None));
                    }
                    Some(b'=') => {
                        i += 1;
                        let value = if matches!(b.get(i), Some(b'"') | Some(b'\'')) {
                            let q = b[i];
                            i += 1;
                            let start = i;
                            while i < b.len() && b[i] != q {
                                i += 1;
                            }
                            if i >= b.len() {
                                return Err(bad());
                            }
                            let v = s[start..i].to_owned();
                            i += 1;
                            v
                        } else {
                            ident(&mut i)
                        };
                        if b.get(i) != Some(&b']') {
                            return Err(bad());
                        }
                        i += 1;
                        sel.attrs.push((name, Some(value)));
                    }
                    _ => return Err(bad()),
                }
            }
            _ => return Err(bad()), // whitespace, combinators, pseudo-classes, '#id', '*', ...
        }
    }
    Ok(sel)
}

/// What a selector is matched against: the element's tag and its attributes *including the stamps*.
struct Virtual<'a> {
    tag: &'a str,
    attrs: BTreeMap<String, String>,
}

impl Selector {
    fn matches(&self, v: &Virtual<'_>) -> bool {
        if self.tag.as_deref().is_some_and(|t| t != v.tag) {
            return false;
        }
        let classes: BTreeSet<&str> = v
            .attrs
            .get("class")
            .map(|c| c.split_whitespace().collect())
            .unwrap_or_default();
        self.classes.iter().all(|c| classes.contains(c.as_str()))
            && self
                .attrs
                .iter()
                .all(|(k, want)| match (v.attrs.get(k), want) {
                    (Some(_), None) => true,
                    (Some(have), Some(w)) => have == w,
                    _ => false,
                })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// safety

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {} // not legal in XML 1.0
            c => out.push(c),
        }
    }
    out
}

const FORBIDDEN_ELEMENTS: &[&str] = &[
    "script",
    "foreignObject",
    "iframe",
    "object",
    "embed",
    "style",
    "animate",
    "set",
    "animateTransform",
    "animateMotion",
];

fn check_icon(name: &str, icon: &Icon) -> Result<(), EnhanceError> {
    let bad = |reason: &str| EnhanceError::UnsafeIcon {
        name: name.to_owned(),
        reason: reason.to_owned(),
    };
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(bad("name must be letters, digits, '-' or '_'"));
    }
    if icon.svg.len() > MAX_ICON_BYTES {
        return Err(bad("larger than 16 KiB"));
    }
    if icon.view_box.split_whitespace().count() != 4
        || icon
            .view_box
            .split_whitespace()
            .any(|n| n.parse::<f64>().is_err())
    {
        return Err(bad("view_box must be four numbers"));
    }
    let wrapped = format!("<g xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\">{}</g>", icon.svg);
    let doc =
        roxmltree::Document::parse(&wrapped).map_err(|e| bad(&format!("not well-formed: {e}")))?;
    for n in doc.descendants().filter(|n| n.is_element()) {
        if FORBIDDEN_ELEMENTS
            .iter()
            .any(|f| n.tag_name().name().eq_ignore_ascii_case(f))
        {
            return Err(bad(&format!("<{}> is not allowed", n.tag_name().name())));
        }
        for a in n.attributes() {
            let key = a.name().to_ascii_lowercase();
            if key.starts_with("on") {
                return Err(bad("event handler attributes are not allowed"));
            }
            if key == "href"
                && !(a.value().starts_with('#') || a.value().starts_with("data:image/"))
            {
                return Err(bad("references must be '#fragment' or data:image/"));
            }
            if a.value().to_ascii_lowercase().contains("javascript:") {
                return Err(bad("javascript: is not allowed"));
            }
        }
    }
    Ok(())
}

fn check_css(css: &str) -> Result<(), EnhanceError> {
    let lower = css.to_ascii_lowercase();
    let bad = |m: &str| Err(EnhanceError::UnsafeCss(m.to_owned()));
    if css.len() > MAX_CSS_BYTES {
        return bad("larger than 64 KiB");
    }
    for needle in [
        "]]>",
        "</",
        "@import",
        "expression(",
        "javascript:",
        "behavior:",
        "-moz-binding",
    ] {
        if lower.contains(needle) {
            return bad(&format!("contains '{needle}'"));
        }
    }
    if lower.contains("url(")
        && !lower.contains("url(#")
        && !lower.contains("url(data:image/")
        && !lower.contains("url(\"#")
        && !lower.contains("url('#")
    {
        return bad("url() may only reference '#fragment' or data:image/");
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// text-splice helpers

struct Edit {
    pos: usize,
    remove: usize,
    insert: String,
    seq: usize,
}

/// Index just past the end of the start tag that begins at `start` (the byte after its closing `>`), plus whether it
/// was self-closing. Honours quoted attribute values, which may legally contain `>`.
fn start_tag_end(text: &str, start: usize) -> Option<(usize, bool)> {
    let b = text.as_bytes();
    let mut i = start;
    let mut quote: Option<u8> = None;
    while i < b.len() {
        match (quote, b[i]) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, b'"') | (None, b'\'') => quote = Some(b[i]),
            (None, b'>') => return Some((i + 1, i > start && b[i - 1] == b'/')),
            _ => {}
        }
        i += 1;
    }
    None
}

/// Byte range of the *value* of attribute `name` inside the start tag `tag` (offsets relative to `tag`).
fn attr_value_range(tag: &str, name: &str) -> Option<(usize, usize)> {
    let b = tag.as_bytes();
    let mut i = tag.find(|c: char| c.is_whitespace())?; // after the tag name
    while i < b.len() {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/') {
            i += 1;
        }
        let ns = i;
        while i < b.len()
            && !b[i].is_ascii_whitespace()
            && b[i] != b'='
            && b[i] != b'>'
            && b[i] != b'/'
        {
            i += 1;
        }
        let attr = &tag[ns..i];
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if b.get(i) != Some(&b'=') {
            continue;
        }
        i += 1;
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let q = *b.get(i)?;
        if q != b'"' && q != b'\'' {
            return None;
        }
        let vs = i + 1;
        let ve = vs + tag[vs..].find(q as char)?;
        if attr == name {
            return Some((vs, ve));
        }
        i = ve + 1;
    }
    None
}

fn b64_class(id: &str) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = id.as_bytes();
    let mut out = String::with_capacity(bytes.len() * 4 / 3 + 3);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(T[(n >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            out.push(T[n as usize & 63] as char);
        }
    }
    out // unpadded, as D2 emits it
}

// ---------------------------------------------------------------------------------------------------------------

/// Stamp known elements and apply `brand`. Returns the new SVG text and a report. Pure and idempotent.
pub fn enhance(
    svg: &str,
    known: &[KnownElement],
    brand: &Brand,
) -> Result<(String, Report), EnhanceError> {
    if brand.version != 1 {
        return Err(EnhanceError::UnsupportedVersion(brand.version));
    }
    // validate the whole brand before touching anything
    for (name, icon) in &brand.icons {
        check_icon(name, icon)?;
    }
    if let Some(css) = &brand.css {
        check_css(css)?;
    }
    let mut rules = Vec::with_capacity(brand.rules.len());
    for (index, r) in brand.rules.iter().enumerate() {
        if let Some(icon) = &r.icon {
            if !brand.icons.contains_key(icon) {
                return Err(EnhanceError::UnknownIcon {
                    index,
                    icon: icon.clone(),
                });
            }
        }
        if let Some(c) = &r.class {
            if c.is_empty()
                || !c
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            {
                return Err(EnhanceError::UnsupportedSelector(format!(
                    "class '{c}' must be letters, digits, '-' or '_'"
                )));
            }
        }
        rules.push((parse_selector(&r.select)?, r));
    }

    let doc = roxmltree::Document::parse(svg).map_err(|e| EnhanceError::BadSvg(e.to_string()))?;
    let by_class: BTreeMap<String, &KnownElement> =
        known.iter().map(|k| (b64_class(&k.id), k)).collect();

    let mut edits: Vec<Edit> = Vec::new();
    let mut seq = 0usize;
    let mut push = |edits: &mut Vec<Edit>, pos, remove, insert: String| {
        seq += 1;
        edits.push(Edit {
            pos,
            remove,
            insert,
            seq,
        });
    };
    let mut found = BTreeSet::new();
    let mut rewrites = 0usize;
    let mut icons_used: BTreeSet<&str> = BTreeSet::new();

    for g in doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "g")
    {
        let Some(known) = g
            .attribute("class")
            .and_then(|c| c.split_whitespace().find_map(|c| by_class.get(c)))
        else {
            continue;
        };
        found.insert(known.id.as_str());
        let range = g.range();
        let Some((tag_end, self_closing)) = start_tag_end(svg, range.start) else {
            continue;
        };
        let tag = &svg[range.start..tag_end];

        // 1. stamps (never overwrite an existing stamp)
        let mut virt = Virtual {
            tag: "g",
            attrs: g
                .attributes()
                .map(|a| (a.name().to_owned(), a.value().to_owned()))
                .collect(),
        };
        let mut stamp = String::new();
        let mut stamp_attr = |k: &str, v: &str, virt: &mut Virtual<'_>| {
            if !virt.attrs.contains_key(k) {
                stamp.push_str(&format!(" {k}=\"{}\"", xml_escape(v)));
                virt.attrs.insert(k.to_owned(), v.to_owned());
            }
        };
        stamp_attr("data-kr0ki-id", &known.id, &mut virt);
        if let Some(t) = &known.ty {
            stamp_attr("data-kr0ki-type", t, &mut virt);
        }
        if let Some(n) = &known.name {
            stamp_attr("data-kr0ki-name", n, &mut virt);
        }
        // 2. rules
        let mut add_classes: Vec<&str> = Vec::new();
        let mut icon: Option<&str> = None;
        let mut restore_label = false;
        for (sel, rule) in &rules {
            if !sel.matches(&virt) {
                continue;
            }
            if let Some(c) = &rule.class {
                let has = virt
                    .attrs
                    .get("class")
                    .is_some_and(|v| v.split_whitespace().any(|x| x == c))
                    || add_classes.contains(&c.as_str());
                if !has {
                    add_classes.push(c);
                }
            }
            if let Some(i) = &rule.icon {
                icon = Some(i.as_str());
            }
            restore_label |= rule.label == Some(LabelMode::Name);
        }
        // apply: attribute stamps go just before the closing '>' (or '/>')
        let insert_at = if self_closing {
            tag_end - 2
        } else {
            tag_end - 1
        };
        if !stamp.is_empty() {
            push(
                &mut edits,
                range.start + (insert_at - range.start),
                0,
                stamp,
            );
        }
        if !add_classes.is_empty() {
            rewrites += 1;
            let extra = add_classes.join(" ");
            match attr_value_range(tag, "class") {
                Some((_, ve)) => push(&mut edits, range.start + ve, 0, format!(" {extra}")),
                None => push(&mut edits, insert_at, 0, format!(" class=\"{extra}\"")),
            }
        }
        if let (Some(name), false) = (icon, self_closing) {
            let present = g.children().any(|c| {
                c.is_element()
                    && c.tag_name().name() == "use"
                    && c.attribute("data-kr0ki-icon").is_some()
            });
            if !present {
                if let Some(rect) = g
                    .descendants()
                    .find(|n| n.is_element() && n.tag_name().name() == "rect")
                {
                    let num = |k: &str| {
                        rect.attribute(k)
                            .and_then(|v| v.trim().parse::<f64>().ok())
                            .unwrap_or(0.0)
                    };
                    let end_tag = svg[range.start..range.end]
                        .rfind("</")
                        .map(|o| range.start + o);
                    if let Some(end_tag) = end_tag {
                        icons_used.insert(name);
                        rewrites += 1;
                        push(
                            &mut edits,
                            end_tag,
                            0,
                            format!(
                                "<use href=\"#{ICON_PREFIX}{name}\" data-kr0ki-icon=\"{name}\" x=\"{}\" y=\"{}\" width=\"20\" height=\"20\"/>",
                                num("x") + 6.0,
                                num("y") + 6.0
                            ),
                        );
                    }
                }
            } else {
                icons_used.insert(name);
            }
        }
        if restore_label {
            if let Some(display) = known.name.as_deref().filter(|n| !n.trim().is_empty()) {
                for t in g.descendants().filter(|n| n.is_text()) {
                    if t.text().is_some_and(|x| x.trim() == known.id)
                        && t.parent().is_some_and(|p| p.tag_name().name() == "text")
                    {
                        let r = t.range();
                        rewrites += 1;
                        push(&mut edits, r.start, r.end - r.start, xml_escape(display));
                    }
                }
            }
        }
    }

    // 3. symbols and style, once each
    let root = doc.root_element();
    let existing_symbols: BTreeSet<&str> = doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "symbol")
        .filter_map(|n| n.attribute("id"))
        .collect();
    let mut defs = String::new();
    for name in &icons_used {
        if !existing_symbols.contains(format!("{ICON_PREFIX}{name}").as_str()) {
            let icon = &brand.icons[*name];
            defs.push_str(&format!(
                "<symbol id=\"{ICON_PREFIX}{name}\" viewBox=\"{}\">{}</symbol>",
                xml_escape(&icon.view_box),
                icon.svg
            ));
        }
    }
    if !defs.is_empty() {
        if let Some((end, false)) = start_tag_end(svg, root.range().start) {
            rewrites += 1;
            push(&mut edits, end, 0, format!("<defs>{defs}</defs>"));
        }
    }
    if let Some(css) = brand.css.as_deref().filter(|c| !c.trim().is_empty()) {
        let present = doc.descendants().any(|n| {
            n.is_element() && n.tag_name().name() == "style" && n.attribute("id") == Some(STYLE_ID)
        });
        if !present {
            if let Some(o) = svg[root.range()].rfind("</") {
                rewrites += 1;
                push(
                    &mut edits,
                    root.range().start + o,
                    0,
                    format!("<style id=\"{STYLE_ID}\"><![CDATA[{css}]]></style>"),
                );
            }
        }
    }

    // apply back to front; equal positions keep insertion order
    edits.sort_by(|a, b| b.pos.cmp(&a.pos).then(b.seq.cmp(&a.seq)));
    let mut out = svg.to_owned();
    for e in &edits {
        out.replace_range(e.pos..e.pos + e.remove, &e.insert);
    }
    let unindexed = known
        .iter()
        .filter(|k| !found.contains(k.id.as_str()))
        .map(|k| k.id.clone())
        .collect();
    Ok((
        out,
        Report {
            indexed: found.len(),
            unindexed,
            rewrites,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known(id: &str, ty: &str, name: &str) -> KnownElement {
        KnownElement {
            id: id.into(),
            ty: Some(ty.into()),
            name: Some(name.into()),
        }
    }
    fn brand() -> Brand {
        serde_json::from_str(
            r##"{"version":1,
              "icons":{"part":{"svg":"<rect width=\"24\" height=\"24\" fill=\"#38bdf8\"/>"}},
              "rules":[{"select":"[data-kr0ki-type=\"PartUsage\"]","class":"k-part","icon":"part","label":"name"}],
              "css":".k-part rect { stroke: #0ea5e9 !important }"}"##,
        )
        .unwrap()
    }
    const ID: &str = "00000000-0000-4000-8000-000000000002";
    fn svg() -> String {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\"><g class=\"{}\"><rect x=\"10\" y=\"20\" width=\"50\" height=\"30\"/><text>{ID}</text></g><g class=\"other\"><rect x=\"0\" y=\"0\" width=\"1\" height=\"1\"/></g></svg>",
            b64_class(ID)
        )
    }

    #[test]
    fn base64_matches_d2s_unpadded_encoding() {
        assert_eq!(
            b64_class(ID),
            "MDAwMDAwMDAtMDAwMC00MDAwLTgwMDAtMDAwMDAwMDAwMDAy"
        );
        assert_eq!(b64_class("a"), "YQ");
        assert_eq!(b64_class("ab"), "YWI");
        assert_eq!(b64_class("abc"), "YWJj");
    }

    #[test]
    fn stamps_icons_names_and_style_only_on_known_elements() {
        let (out, rep) = enhance(&svg(), &[known(ID, "PartUsage", "engine")], &brand()).unwrap();
        assert_eq!(
            rep,
            Report {
                indexed: 1,
                unindexed: vec![],
                rewrites: rep.rewrites
            }
        );
        let doc = roxmltree::Document::parse(&out).expect("output is well-formed");
        let g = doc
            .descendants()
            .find(|n| n.attribute("data-kr0ki-id") == Some(ID))
            .unwrap();
        assert_eq!(g.attribute("data-kr0ki-type"), Some("PartUsage"));
        assert_eq!(g.attribute("data-kr0ki-name"), Some("engine"));
        assert!(g.attribute("class").unwrap().ends_with("k-part"));
        assert_eq!(
            g.descendants()
                .find(|n| n.tag_name().name() == "text")
                .unwrap()
                .text(),
            Some("engine")
        );
        assert_eq!(
            g.children()
                .filter(|c| c.tag_name().name() == "use")
                .count(),
            1
        );
        assert_eq!(
            doc.descendants()
                .filter(|n| n.tag_name().name() == "symbol")
                .count(),
            1
        );
        assert_eq!(
            doc.descendants()
                .filter(|n| n.attribute("id") == Some("kr0ki-brand"))
                .count(),
            1
        );
        let other = doc
            .descendants()
            .find(|n| n.attribute("class") == Some("other"))
            .unwrap();
        assert!(other.attribute("data-kr0ki-id").is_none());
    }

    #[test]
    fn a_second_run_changes_nothing() {
        let k = [known(ID, "PartUsage", "engine")];
        let (once, _) = enhance(&svg(), &k, &brand()).unwrap();
        let (twice, rep) = enhance(&once, &k, &brand()).unwrap();
        assert_eq!(once, twice);
        assert_eq!(rep.rewrites, 0);
    }

    #[test]
    fn unknown_and_undrawn_ids_are_reported_not_dropped() {
        let (_, rep) = enhance(
            &svg(),
            &[
                known(ID, "PartUsage", "e"),
                known("missing", "PartUsage", "x"),
            ],
            &brand(),
        )
        .unwrap();
        assert_eq!(rep.unindexed, vec!["missing".to_string()]);
    }

    #[test]
    fn rules_only_touch_elements_their_selector_matches() {
        let (out, _) = enhance(&svg(), &[known(ID, "PartDefinition", "Engine")], &brand()).unwrap();
        let doc = roxmltree::Document::parse(&out).unwrap();
        let g = doc
            .descendants()
            .find(|n| n.attribute("data-kr0ki-id") == Some(ID))
            .unwrap();
        assert_eq!(g.attribute("data-kr0ki-type"), Some("PartDefinition"));
        assert!(
            !g.attribute("class").unwrap().contains("k-part"),
            "PartUsage rule must not match a PartDefinition: {out}"
        );
        assert!(
            g.children().all(|c| c.tag_name().name() != "use"),
            "no icon for a non-matching element"
        );
    }

    #[test]
    fn names_from_the_model_are_escaped() {
        let (out, _) = enhance(&svg(), &[known(ID, "PartUsage", "a<b & \"c\"")], &brand()).unwrap();
        assert!(roxmltree::Document::parse(&out).is_ok(), "{out}");
        assert!(!out.contains("a<b"));
        assert!(out.contains("a&lt;b &amp; &quot;c&quot;"));
    }

    #[test]
    fn a_quoted_gt_in_an_attribute_does_not_confuse_the_start_tag_scan() {
        let s = format!("<svg xmlns=\"http://www.w3.org/2000/svg\"><g data-x=\"a>b\" class=\"{}\"><rect x=\"1\" y=\"1\"/></g></svg>", b64_class(ID));
        let (out, rep) = enhance(&s, &[known(ID, "PartUsage", "e")], &brand()).unwrap();
        assert_eq!(rep.indexed, 1);
        assert!(roxmltree::Document::parse(&out).is_ok(), "{out}");
    }

    #[test]
    fn selectors_are_a_strict_subset_and_fail_loudly() {
        assert!(parse_selector("g[data-kr0ki-type=\"PartUsage\"].x").is_ok());
        assert!(parse_selector("[data-kr0ki-id]").is_ok());
        for bad in [
            "g > rect",
            "a b",
            "g [data-x]",
            "g .x",
            ".a .b",
            "#id",
            "*",
            ":hover",
            "[a=",
            "",
            "g[",
            ".",
            "[a~=b]",
        ] {
            assert!(
                matches!(
                    parse_selector(bad),
                    Err(EnhanceError::UnsupportedSelector(_))
                ),
                "{bad}"
            );
        }
    }

    #[test]
    fn unsafe_icons_css_and_classes_are_rejected_before_anything_is_written() {
        let mk = |icon: &str| -> Brand {
            serde_json::from_value(serde_json::json!({"icons": {"i": {"svg": icon}}, "rules": []}))
                .unwrap()
        };
        for bad in [
            "<script>alert(1)</script>",
            "<rect onclick=\"x()\"/>",
            "<foreignObject/>",
            "<use href=\"http://evil/x.svg#a\"/>",
            "<rect fill=\"javascript:x\"/>",
            "<rect>",
            "<style>*{}</style>",
            "<animate attributeName=\"x\"/>",
        ] {
            assert!(
                matches!(
                    enhance(&svg(), &[], &mk(bad)),
                    Err(EnhanceError::UnsafeIcon { .. })
                ),
                "{bad}"
            );
        }
        assert!(enhance(&svg(), &[], &mk("<use href=\"#a\"/>")).is_ok());
        for css in [
            "a{} </style><script>",
            "@import url(x);",
            "@import \"x.css\";",
            "a{x:expression(alert(1))}",
            "a{behavior:url(#x)}",
            "a{background:url(http://evil/x)}",
            "x]]>y",
        ] {
            let b: Brand = serde_json::from_value(serde_json::json!({"css": css})).unwrap();
            assert!(
                matches!(enhance(&svg(), &[], &b), Err(EnhanceError::UnsafeCss(_))),
                "{css}"
            );
        }
        let b: Brand = serde_json::from_value(
            serde_json::json!({"icons": {}, "rules": [{"select": "g", "icon": "nope"}]}),
        )
        .unwrap();
        assert!(matches!(
            enhance(&svg(), &[], &b),
            Err(EnhanceError::UnknownIcon { .. })
        ));
        let b: Brand = serde_json::from_value(
            serde_json::json!({"rules": [{"select": "g", "class": "a\"b"}]}),
        )
        .unwrap();
        assert!(enhance(&svg(), &[], &b).is_err());
        let b: Brand = serde_json::from_value(serde_json::json!({"version": 2})).unwrap();
        assert_eq!(
            enhance(&svg(), &[], &b).unwrap_err(),
            EnhanceError::UnsupportedVersion(2)
        );
        assert!(matches!(
            enhance("<svg", &[], &brand()),
            Err(EnhanceError::BadSvg(_))
        ));
    }

    /// A real D2 render of a SysML v2 snapshot (names as labels, ids as keys): the contract the layer exists for.
    #[test]
    fn works_on_a_real_d2_render_of_a_sysml_snapshot() {
        let svg = include_str!("../tests/fixtures/sysml-d2.svg");
        let ids = |n: u32| format!("00000000-0000-4000-8000-{n:012}");
        let known = vec![
            known(&ids(1), "PartDefinition", "Vehicle"),
            known(&ids(2), "PartUsage", "engine"),
            known(&ids(3), "PartUsage", "transmission"),
            known(&ids(4), "PartUsage", "battery"),
            known(&ids(5), "RequirementUsage", "range"), // not drawn in this view
        ];
        let b: Brand = serde_json::from_value(serde_json::json!({
            "icons": {"part": {"svg": "<rect width=\"24\" height=\"24\"/>"}},
            "rules": [{"select": "g[data-kr0ki-type=\"PartUsage\"]", "class": "k-part", "icon": "part"}]
        }))
        .unwrap();
        let (out, rep) = enhance(svg, &known, &b).unwrap();
        assert_eq!(rep.indexed, 4);
        assert_eq!(rep.unindexed, vec![ids(5)]);
        let doc = roxmltree::Document::parse(&out).expect("well-formed");
        assert_eq!(
            doc.descendants()
                .filter(|n| n.attribute("data-kr0ki-type") == Some("PartUsage"))
                .count(),
            3
        );
        assert_eq!(
            doc.descendants()
                .filter(|n| n.tag_name().name() == "use")
                .count(),
            3
        );
        // everything the renderer wrote is still there: the original is the output minus our insertions
        assert!(out.len() > svg.len() && out.contains("data-d2-version"));
        let (again, r2) = enhance(&out, &known, &b).unwrap();
        assert_eq!((again == out, r2.rewrites), (true, 0));
    }
}
