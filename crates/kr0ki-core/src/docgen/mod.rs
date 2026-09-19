//! Docgen — harvest Rust source symbols and render them as structured docs.
//!
//! Pattern derived from `b00t-cli/src/commands/docgen.rs` (elasticdotventures/_b00t_).
//! This is a clean-room implementation for kr0ki's Rust codebase — same pipeline
//! (harvest → format → emit) but using `syn` instead of `codebase-memory-mcp`.
//!
//! Output formats:
//!   - `json`   : machine-readable symbol list
//!   - `tomllm` : .tomllm format (b00t docgen convention)
//!   - `rustdoc`: rustdoc-style comment blocks
//!   - `html`   : standalone HTML page (MVP — no external CSS/JS frameworks)

pub mod format;
pub mod harvest;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A harvested code symbol (function, struct, enum, trait, etc.).
/// Field names align with the schema consumed by `b00t-cli docgen` formatters
/// so downstream tools can interoperate without translation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Symbol {
    pub name: String,
    pub qualified_name: String,
    pub file_path: String,
    pub kind: SymbolKind,
    pub signature: String,
    pub return_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_line: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complexity: Option<u64>,
    pub docstring: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
    Type,
    Macro,
    Mod,
    Const,
    Static,
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SymbolKind::Function => write!(f, "fn"),
            SymbolKind::Struct => write!(f, "struct"),
            SymbolKind::Enum => write!(f, "enum"),
            SymbolKind::Trait => write!(f, "trait"),
            SymbolKind::Type => write!(f, "type"),
            SymbolKind::Macro => write!(f, "macro"),
            SymbolKind::Mod => write!(f, "mod"),
            SymbolKind::Const => write!(f, "const"),
            SymbolKind::Static => write!(f, "static"),
        }
    }
}

/// Harvest all symbols from a crate source tree.
pub fn harvest_crate<P: AsRef<Path>>(root: P) -> anyhow::Result<Vec<Symbol>> {
    harvest::walk_and_harvest(root.as_ref())
}

/// Find the kr0ki workspace root by walking up from the current directory
/// looking for a `Cargo.toml` that declares `[workspace]`.
fn find_workspace_root() -> Option<std::path::PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let cargo_toml = dir.join("Cargo.toml");
        if cargo_toml.is_file() {
            if let Ok(contents) = std::fs::read_to_string(&cargo_toml) {
                if contents.contains("[workspace]") {
                    return Some(dir);
                }
            }
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

/// Convenience: harvest kr0ki's own workspace crates.
pub fn harvest_kr0ki_workspace() -> anyhow::Result<Vec<Symbol>> {
    let mut all = Vec::new();
    let root = find_workspace_root()
        .or_else(|| std::env::current_dir().ok())
        .context("cannot determine workspace root")?;

    // Map directory name → Rust crate name (hyphens to underscores)
    for (crate_dir, crate_rust) in [
        ("kr0ki-core", "kr0ki_core"),
        ("kr0ki-server", "kr0ki_server"),
        ("kr0ki-sysmlv2-client", "kr0ki_sysmlv2_client"),
    ] {
        let path = root.join(format!("crates/{crate_dir}/src"));
        if path.is_dir() {
            let mut syms = harvest_crate(&path)?;
            for sym in &mut syms {
                if !sym.qualified_name.starts_with(crate_rust) {
                    sym.qualified_name = format!("{crate_rust}::{}", sym.qualified_name);
                }
            }
            all.append(&mut syms);
        }
    }
    // Also harvest top-level src/ if present (for single-crate layout)
    let top_src = root.join("src");
    if top_src.is_dir() {
        all.append(&mut harvest_crate(&top_src)?);
    }
    Ok(all)
}

/// Emit a self-contained static mdb00k/playb00k bundle from the current workspace.
///
/// The bundle contains an HTML symbol index plus the machine-readable docgen
/// exports used by b00t-aware consumers. It is backend-free so it can be
/// published by GitHub Pages without a running kr0ki renderer.
pub fn export_static_mdb00k(output_dir: impl AsRef<Path>) -> anyhow::Result<()> {
    let output_dir = output_dir.as_ref();
    let symbols = harvest_kr0ki_workspace()?;
    let d2_source = find_workspace_root()
        .map(|root| root.join("templates/b00t-stack-orchestration.d2"))
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_else(|| "# D2 template not found\na -> b".into());
    write_static_mdb00k(output_dir, &symbols, &d2_source)
}

fn write_static_mdb00k(
    output_dir: &Path,
    symbols: &[Symbol],
    d2_source: &str,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(output_dir).with_context(|| {
        format!(
            "create static documentation directory {}",
            output_dir.display()
        )
    })?;
    std::fs::write(
        output_dir.join("index.html"),
        format::format_html(symbols, "kr0ki playb00k", d2_source),
    )
    .context("write static HTML playb00k")?;
    std::fs::write(output_dir.join("api.json"), format::format_json(symbols)?)
        .context("write static JSON export")?;
    std::fs::write(
        output_dir.join("api.tomllm"),
        format::format_tomllm(symbols),
    )
    .context("write static tomllm export")?;
    std::fs::write(
        output_dir.join("api.rustdoc"),
        format::format_rustdoc(symbols),
    )
    .context("write static rustdoc export")?;
    let playbook_api_dir = output_dir.join("playbook/api");
    std::fs::create_dir_all(&playbook_api_dir).context("create static playb00k API directory")?;
    std::fs::write(
        playbook_api_dir.join("examples.json"),
        serde_json::to_string_pretty(crate::examples::ALL)?,
    )
    .context("write static playb00k example catalog")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_serde_roundtrip() {
        let s = Symbol {
            name: "render".into(),
            qualified_name: "kr0ki_core::render::render".into(),
            file_path: "src/render.rs".into(),
            kind: SymbolKind::Function,
            signature: "pub async fn render() -> Result<RenderOutput>".into(),
            return_type: "Result<RenderOutput>".into(),
            start_line: Some(42),
            complexity: Some(5),
            docstring: "Render a diagram.".into(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Symbol = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn static_mdb00k_writes_all_exports() {
        let directory =
            std::env::temp_dir().join(format!("kr0ki-docgen-test-{}", std::process::id()));
        let symbol = Symbol {
            name: "render".into(),
            qualified_name: "kr0ki_core::render".into(),
            file_path: "src/render.rs".into(),
            kind: SymbolKind::Function,
            signature: "pub fn render()".into(),
            return_type: "()".into(),
            start_line: None,
            complexity: None,
            docstring: "Render a diagram.".into(),
        };

        write_static_mdb00k(&directory, &[symbol], "a -> b").unwrap();
        for file in ["index.html", "api.json", "api.tomllm", "api.rustdoc"] {
            assert!(directory.join(file).is_file(), "missing {file}");
        }
        let html = std::fs::read_to_string(directory.join("index.html")).unwrap();
        assert!(html.contains("Rendered Rust flow"));
        assert!(html.contains("KerML representation"));
        assert!(html.contains("SysML v2 representation"));
        assert!(html.contains("id=\"live-origin\""));
        assert!(!html.contains("192.168.1.137"));
        let examples =
            std::fs::read_to_string(directory.join("playbook/api/examples.json")).unwrap();
        assert!(examples.contains("d2-rust-flow"));
        std::fs::remove_dir_all(directory).unwrap();
    }
}
