//! Output formatters for harvested symbols.
//!
//! Pattern derived from `b00t-cli/src/commands/docgen.rs`.
//! These formatters intentionally produce the same output conventions so that
//! downstream b00t tooling (tomllm parsers, mdbook preprocessors) can consume
//! kr0ki docgen without translation.

use super::Symbol;

const RUST_FLOW_D2: &str = include_str!("../../../../templates/kr0ki-render-flow.d2");
const RUST_FLOW_KERML: &str = include_str!("../../../../templates/kr0ki-render-flow.kerml");
const RUST_FLOW_SYSML: &str = include_str!("../../../../templates/kr0ki-render-flow.sysml");
/// JSON — machine-readable, lossless.
pub fn format_json(symbols: &[Symbol]) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(symbols)?)
}

/// Tomllm — b00t docgen convention.
///
/// Each symbol becomes a `[[qualified_name]]` table. A `b00t:map v1`
/// trailer carries metadata so b00t datum parsers can classify the export.
pub fn format_tomllm(symbols: &[Symbol]) -> String {
    let mut out = String::new();
    out.push_str("# kr0ki docgen export — .tomllm format\n");
    out.push_str("# schema: docgen-v1 | Auto-generated from kr0ki source\n\n");

    for s in symbols {
        out.push_str(&format!("[[{}]]\n", s.qualified_name));
        out.push_str(&format!("name = \"{}\"\n", s.name));
        out.push_str(&format!("kind = \"{}\"\n", s.kind));
        out.push_str(&format!("file_path = \"{}\"\n", s.file_path));
        out.push_str(&format!(
            "signature = \"{}\"\n",
            s.signature.escape_default()
        ));
        if !s.return_type.is_empty() {
            out.push_str(&format!("return_type = \"{}\"\n", s.return_type));
        }
        if let Some(line) = s.start_line {
            out.push_str(&format!("start_line = {line}\n"));
        }
        if let Some(c) = s.complexity {
            out.push_str(&format!("complexity = {c}\n"));
        }
        if !s.docstring.is_empty() {
            let oneline = s.docstring.replace('\n', " ");
            out.push_str(&format!("# @tribal: {oneline}\n"));
        }
        out.push('\n');
    }

    out.push_str("# b00t:map v1\n");
    out.push_str("# summary: kr0ki symbol documentation export\n");
    out.push_str("# tags: docgen, auto-export, kr0ki\n");
    out.push_str("# tier: sm0l\n");
    out.push_str("# cmds: kr0ki-server /docs/api.tomllm\n");
    out.push_str("# complexity: 3\n");
    out
}

/// Rustdoc — rustdoc-style comment blocks suitable for pasting into a .rs file
/// or for rendering by rustdoc viewers.
pub fn format_rustdoc(symbols: &[Symbol]) -> String {
    let mut out = String::new();
    out.push_str("// kr0ki docgen export — rustdoc style\n\n");

    for s in symbols {
        let sig_display = if s.signature.is_empty() {
            format!("{} {}", s.kind, s.name)
        } else {
            s.signature.clone()
        };
        out.push_str(&format!("/// `{sig_display}`\n"));
        for line in s.docstring.lines() {
            out.push_str(&format!("/// {line}\n"));
        }
        out.push_str("///\n");
        out.push_str("/// # Source\n");
        if let Some(line) = s.start_line {
            out.push_str(&format!("/// Review source at {}:{}\n", s.file_path, line));
        } else {
            out.push_str(&format!("/// Review source at {}\n", s.file_path));
        }
        out.push_str("/// # Kind\n");
        out.push_str(&format!("/// {}\n", s.kind));
        out.push('\n');
    }
    out
}

/// HTML — standalone page (MVP). No external CSS/JS frameworks.
/// Includes a diagram example rendered by kr0ki itself.
pub fn format_html(symbols: &[Symbol], title: &str, diagram_example_d2: &str) -> String {
    format_html_with_live_flow(symbols, title, diagram_example_d2, None)
}

/// HTML documentation with an optional service-rendered Rust flow image.
///
/// Static mdb00k exports deliberately pass `None`: GitHub Pages has no kr0ki
/// renderer behind it, so the portable structural SVG remains available. The
/// live server passes its same-origin endpoint, which exercises the complete
/// D2 -> kr0ki -> Kroki render path.
pub fn format_html_with_live_flow(
    symbols: &[Symbol],
    title: &str,
    diagram_example_d2: &str,
    live_flow_url: Option<&str>,
) -> String {
    let mut body = String::new();

    // Table of contents by kind
    let mut by_kind: std::collections::HashMap<String, Vec<&Symbol>> =
        std::collections::HashMap::new();
    for s in symbols {
        by_kind.entry(s.kind.to_string()).or_default().push(s);
    }

    for (kind, items) in by_kind {
        body.push_str(&format!(
            "<h2 id=\"kind-{kind}\">{kind} ({})</h2>\n",
            items.len()
        ));
        body.push_str("<ul class=\"symbol-list\">\n");
        for s in items {
            body.push_str("<li>\n");
            body.push_str(&format!(
                "  <code class=\"sig\">{}</code>\n",
                html_escape(&s.signature)
            ));
            if !s.docstring.is_empty() {
                body.push_str("  <div class=\"docs\">\n");
                for line in s.docstring.lines() {
                    body.push_str(&format!("    <p>{}</p>\n", html_escape(line)));
                }
                body.push_str("  </div>\n");
            }
            body.push_str(&format!(
                "  <span class=\"meta\">{}:{}</span>\n",
                html_escape(&s.file_path),
                s.start_line.unwrap_or(0)
            ));
            body.push_str("</li>\n");
        }
        body.push_str("</ul>\n");
    }

    // Diagram example section
    let diagram_section = format!(
        r#"<h2 id="diagram-example">Diagram Example</h2>
<p>The following D2 diagram is part of kr0ki's template suite and can be rendered via <code>POST /render/d2</code>:</p>
<pre class="d2-source"><code>{d2_source}</code></pre>
<p>Render it live:</p>
<pre><code>curl -X POST http://localhost:8787/render/d2 \
  -H "Content-Type: text/plain" \
  --data-binary @templates/b00t-stack-orchestration.d2 \
  --output diagram.svg</code></pre>
"#,
        d2_source = html_escape(diagram_example_d2),
    );

    let flow_visual = match live_flow_url {
        Some(url) => format!(
            r#"<img src="{url}" alt="Rendered kr0ki Rust code flow" loading="lazy">"#,
            url = html_escape(url)
        ),
        None => rendered_rust_flow_svg().to_string(),
    };
    let rust_flow_section = format!(
        r#"<h2 id="rust-flow">Rendered Rust flow → KerML / SysML v2</h2>
<p>This playb00k fixture models the implemented <strong>Box-5</strong> path: Axum route → <code>RenderService</code> → cache/backend → artifact. It is not a substitute for the deferred upstream <code>ModelSnapshot → UFO → recognizer → ViewDefinition</code> pipeline.</p>
<figure class="flow-view">{flow_visual}<figcaption>Cache hits bypass <code>HttpKrokiBackend</code>; misses render and atomically write the artifact.</figcaption></figure>
<details><summary>D2 source</summary><pre><code>{d2}</code></pre></details>
<details><summary>KerML representation</summary><pre><code>{kerml}</code></pre></details>
<details><summary>SysML v2 representation</summary><pre><code>{sysml}</code></pre></details>"#,
        flow_visual = flow_visual,
        d2 = html_escape(RUST_FLOW_D2),
        kerml = html_escape(RUST_FLOW_KERML),
        sysml = html_escape(RUST_FLOW_SYSML),
    );
    let capability_section = r#"<h2 id="capabilities">Capability examples and test evidence</h2>
<p>LAN documentation: <code id="live-origin"></code>. Start with <code>just run</code> or <code>just dev</code>; both bind <code>0.0.0.0:8787</code> by default.</p>
<pre><code>export KR0KI_URL=http://&lt;LAN-host&gt;:8787
curl "$KR0KI_URL/health"
curl "$KR0KI_URL/formats"
curl -X POST "$KR0KI_URL/render/graphviz?output=svg" --data-binary 'digraph {{ kr0ki -&gt; cache }}' --output flow.svg
curl "$KR0KI_URL/docs/api.tomllm"

just test       # unit + in-process HTTP
just check      # formatting + clippy
just test-live  # opt-in live render</code></pre>
<script>document.getElementById("live-origin").textContent = window.location.origin + "/docs";</script>"#
        .to_string();

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
:root {{ color-scheme: light dark; }}
body {{ font-family: system-ui, -apple-system, sans-serif; max-width: 80ch; margin: 2rem auto; padding: 0 1rem; line-height: 1.6; }}
h1, h2 {{ border-bottom: 1px solid #ccc; padding-bottom: 0.3rem; }}
code {{ background: #f4f4f4; padding: 0.15em 0.4em; border-radius: 4px; font-size: 0.9em; }}
pre {{ background: #f4f4f4; padding: 1rem; overflow-x: auto; border-radius: 6px; }}
pre code {{ background: none; padding: 0; }}
ul.symbol-list {{ list-style: none; padding: 0; }}
ul.symbol-list li {{ margin: 1.5rem 0; padding: 1rem; border: 1px solid #ddd; border-radius: 8px; }}
.sig {{ font-weight: bold; display: block; margin-bottom: 0.5rem; }}
.docs p {{ margin: 0.25rem 0; color: #333; }}
.meta {{ font-size: 0.8em; color: #666; }}
img {{ max-width: 100%; height: auto; border: 1px solid #ddd; border-radius: 6px; }}
@media (prefers-color-scheme: dark) {{
  body {{ background: #111; color: #eee; }}
  code {{ background: #222; }}
  pre {{ background: #222; }}
  ul.symbol-list li {{ border-color: #444; }}
  .docs p {{ color: #ccc; }}
}}
</style>
</head>
<body>
<h1>{title}</h1>
<p>Generated by kr0ki docgen. Pattern derived from <code>b00t-cli/src/commands/docgen.rs</code>.</p>
<nav>
  <a href="/docs">HTML</a> |
  <a href="/docs/api.json">JSON</a> |
  <a href="/docs/api.tomllm">tomllm</a> |
  <a href="/docs/api.rustdoc">rustdoc</a>
</nav>
<hr>
{body}
<hr>
{diagram_section}
<hr>
{rust_flow_section}
<hr>
{capability_section}
<footer><p>kr0ki v{version} — <a href="https://github.com/PromptExecution/kr0ki">source</a></p></footer>
</body>
</html>
"#,
        title = html_escape(title),
        body = body,
        diagram_section = diagram_section,
        rust_flow_section = rust_flow_section,
        capability_section = capability_section,
        version = env!("CARGO_PKG_VERSION"),
    )
}

fn rendered_rust_flow_svg() -> &'static str {
    r##"<svg viewBox="0 0 900 190" role="img" aria-label="kr0ki Rust render flow" xmlns="http://www.w3.org/2000/svg"><defs><marker id="a" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto"><path d="M0 0L10 5L0 10z" fill="currentColor"/></marker></defs><g fill="none" stroke="currentColor" stroke-width="2" marker-end="url(#a)"><path d="M125 70H180"/><path d="M305 70H360"/><path d="M485 70H540"/><path d="M665 70H720"/><path d="M422 105V150H600"/></g><g font-family="system-ui,sans-serif" text-anchor="middle" fill="currentColor"><rect x="20" y="35" width="105" height="70" rx="9" fill="#d8eaff" stroke="currentColor"/><text x="72" y="65">HTTP</text><text x="72" y="86">caller</text><rect x="180" y="35" width="125" height="70" rx="9" fill="#d8eaff" stroke="currentColor"/><text x="242" y="65">Axum</text><text x="242" y="86">route</text><rect x="360" y="35" width="125" height="70" rx="9" fill="#d8eaff" stroke="currentColor"/><text x="422" y="65">Render</text><text x="422" y="86">Service</text><rect x="540" y="35" width="125" height="70" rx="9" fill="#e9f7df" stroke="currentColor"/><text x="602" y="77">FsCache</text><rect x="720" y="35" width="155" height="70" rx="9" fill="#fff0cc" stroke="currentColor"/><text x="797" y="65">HttpKroki</text><text x="797" y="86">Backend</text><text x="510" y="158" font-size="13">cache hit returns artifact; miss calls backend</text></g></svg>"##
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docgen::{Symbol, SymbolKind};

    fn sample_symbol() -> Symbol {
        Symbol {
            name: "render".into(),
            qualified_name: "kr0ki_core::render::render".into(),
            file_path: "src/render.rs".into(),
            kind: SymbolKind::Function,
            signature: "pub async fn render()".into(),
            return_type: "Result<RenderOutput>".into(),
            start_line: Some(10),
            complexity: Some(2),
            docstring: "Render a diagram.".into(),
        }
    }

    #[test]
    fn tomllm_contains_boilerplate() {
        let s = sample_symbol();
        let out = format_tomllm(&[s]);
        assert!(out.contains("b00t:map v1"));
        assert!(out.contains("[[kr0ki_core::render::render]]"));
        assert!(out.contains("# @tribal:"));
    }

    #[test]
    fn rustdoc_contains_sections() {
        let s = sample_symbol();
        let out = format_rustdoc(&[s]);
        assert!(out.contains("/// `pub async fn render()`"));
        assert!(out.contains("/// # Source"));
        assert!(out.contains("/// Review source at src/render.rs:10"));
    }

    #[test]
    fn html_is_well_formed() {
        let s = sample_symbol();
        let out = format_html(&[s], "Test", "a -> b");
        assert!(out.contains("<!DOCTYPE html>"));
        assert!(out.contains("</html>"));
        assert!(out.contains("pub async fn render()"));
        assert!(out.contains("Diagram Example"));
        assert!(out.contains("Rendered Rust flow"));
        assert!(out.contains("SysML v2 representation"));
        assert!(out.contains("id=\"live-origin\""));
        assert!(!out.contains("192.168.1.137"));
    }

    #[test]
    fn html_escapes_special_chars() {
        let out = format_html(&[], "A < B", "x -> y");
        assert!(!out.contains("<title>A < B</title>"));
        assert!(out.contains("<title>A &lt; B</title>"));
    }
}
