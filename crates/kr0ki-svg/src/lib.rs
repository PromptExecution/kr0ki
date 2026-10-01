//! kr0ki-svg: SVG enrichment and transformation layer
//!
//! This crate provides SVG parsing, enrichment, and transformation capabilities
//! for the kr0ki graph representation engine.

pub mod constraint;
pub mod enhance;
pub mod graph;
pub mod solver;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use wasm_bindgen::prelude::*;

/// Error types for kr0ki-svg operations
#[derive(Error, Debug)]
pub enum Kr0kiSvgError {
    #[error("Failed to parse SVG: {0}")]
    ParseError(String),

    #[error("Invalid SVG structure: {0}")]
    InvalidStructure(String),

    #[error("Element not found: {0}")]
    ElementNotFound(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Invalid JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Layout failed: {0}")]
    Solve(#[from] solver::SolveError),
}

impl From<Kr0kiSvgError> for JsValue {
    fn from(err: Kr0kiSvgError) -> Self {
        JsValue::from_str(&err.to_string())
    }
}

/// Represents an enriched SVG document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedSvg {
    /// Original SVG source
    pub source: String,

    /// Parsed tree structure
    pub tree: SvgTree,

    /// Metadata (JSON-LD)
    pub metadata: Option<serde_json::Value>,

    /// Interactive hooks
    pub hooks: Vec<Hook>,

    /// Animation definitions
    pub animations: Vec<Animation>,
}

/// Simplified SVG tree structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvgTree {
    /// Root element
    pub root: SvgElement,

    /// All elements by ID
    pub elements: std::collections::HashMap<String, SvgElement>,
}

/// Represents an SVG element
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvgElement {
    /// Element ID
    pub id: Option<String>,

    /// Element tag name
    pub tag: String,

    /// Attributes
    pub attributes: std::collections::HashMap<String, String>,

    /// Child elements
    pub children: Vec<SvgElement>,

    /// Text content (if any)
    pub text: Option<String>,
}

/// Interactive hook definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hook {
    /// Hook name
    pub name: String,

    /// Target element ID
    pub target: String,

    /// Action to perform
    pub action: String,
}

/// Animation definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Animation {
    /// Animation name
    pub name: String,

    /// Target element ID
    pub target: String,

    /// Animation kind
    pub kind: AnimationKind,

    /// Duration in milliseconds
    pub duration: u32,

    /// Delay in milliseconds
    pub delay: u32,
}

/// Types of animations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnimationKind {
    /// Transform animation (translate, rotate, scale)
    Transform,
    /// Opacity animation
    Opacity,
    /// Path animation
    Path,
    /// Color animation
    Color,
}

/// Configuration for SVG enrichment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentConfig {
    /// Add interactive hooks
    pub add_hooks: bool,

    /// Add animations
    pub add_animations: bool,

    /// Embed metadata
    pub metadata: Option<serde_json::Value>,
}

impl Default for EnrichmentConfig {
    fn default() -> Self {
        Self {
            add_hooks: true,
            add_animations: true,
            metadata: None,
        }
    }
}

/// Parse SVG string and create enriched SVG structure
pub fn parse_svg(input: &str) -> Result<EnrichedSvg, Kr0kiSvgError> {
    // Parse SVG using usvg to validate and simplify
    let opt = usvg::Options::default();
    let fontdb = usvg::fontdb::Database::new();
    let _tree = usvg::Tree::from_str(input, &opt, &fontdb)
        .map_err(|e| Kr0kiSvgError::ParseError(e.to_string()))?;

    // For now, just parse the raw XML to extract structure
    // This is a simplified approach that doesn't use all usvg features
    let doc =
        roxmltree::Document::parse(input).map_err(|e| Kr0kiSvgError::ParseError(e.to_string()))?;

    let root_node = doc.root_element();
    let mut elements = std::collections::HashMap::new();
    let root = parse_xml_node(&root_node, &mut elements)?;

    Ok(EnrichedSvg {
        source: input.to_string(),
        tree: SvgTree { root, elements },
        metadata: None,
        hooks: Vec::new(),
        animations: Vec::new(),
    })
}

/// Parse XML node to SvgElement
fn parse_xml_node(
    node: &roxmltree::Node,
    elements: &mut std::collections::HashMap<String, SvgElement>,
) -> Result<SvgElement, Kr0kiSvgError> {
    let tag = node.tag_name().name().to_string();

    // Extract attributes
    let mut attributes = std::collections::HashMap::new();
    for attr in node.attributes() {
        attributes.insert(attr.name().to_string(), attr.value().to_string());
    }

    // Extract ID
    let id = attributes.get("id").cloned();

    // Parse children
    let mut children = Vec::new();
    for child in node.children().filter(|n| n.is_element()) {
        children.push(parse_xml_node(&child, elements)?);
    }

    // Extract text content
    let text = node.text().map(|s| s.to_string());

    let element = SvgElement {
        id: id.clone(),
        tag,
        attributes,
        children,
        text,
    };

    // Store in elements map if it has an ID
    if let Some(id) = id {
        elements.insert(id, element.clone());
    }

    Ok(element)
}

/// WASM export: Parse SVG and return enriched structure
#[wasm_bindgen]
pub fn parse_svg_wasm(input: &str) -> Result<JsValue, JsValue> {
    let enriched = parse_svg(input)?;
    serde_wasm_bindgen::to_value(&enriched).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// WASM export: Parse SVG and return simplified JSON
#[wasm_bindgen]
pub fn parse_svg_json(input: &str) -> Result<String, JsValue> {
    let enriched = parse_svg(input)?;
    serde_json::to_string(&enriched).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Parse the JSON inputs, solve, and serialize the full [`solver::SolveResult`]
/// (`positions`, `iterations`, `residual`, `converged`). `previous` switches to the
/// incremental solve. Shared by both WASM exports so they cannot drift apart.
fn solve_json(
    graph_json: &str,
    constraints_json: &str,
    previous: Option<&str>,
) -> Result<String, Kr0kiSvgError> {
    let graph: graph::Graph = serde_json::from_str(graph_json)?;
    let constraints: Vec<constraint::Constraint> = serde_json::from_str(constraints_json)?;
    let solver = solver::GraphSolver::with_constraints(constraints);
    let result = match previous {
        Some(json) => {
            let positions: HashMap<String, (f64, f64)> = serde_json::from_str(json)?;
            solver.solve_incremental(&graph, positions)?
        }
        None => solver.solve(&graph)?,
    };
    Ok(serde_json::to_string(&result)?)
}

/// WASM export: Solve graph layout
#[wasm_bindgen]
pub fn solve_graph_layout(graph_json: &str, constraints_json: &str) -> Result<String, JsValue> {
    Ok(solve_json(graph_json, constraints_json, None)?)
}

/// WASM export: Solve graph layout incrementally from `positions_json`
#[wasm_bindgen]
pub fn solve_graph_layout_incremental(
    graph_json: &str,
    constraints_json: &str,
    positions_json: &str,
) -> Result<String, JsValue> {
    Ok(solve_json(
        graph_json,
        constraints_json,
        Some(positions_json),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_svg() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <rect id="rect1" x="10" y="10" width="80" height="80" fill="red"/>
        </svg>"#;

        let result = parse_svg(svg);
        assert!(result.is_ok());

        let enriched = result.unwrap();
        assert_eq!(enriched.source, svg);
        assert!(enriched.tree.elements.contains_key("rect1"));
    }

    #[test]
    fn test_parse_svg_with_path() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <path id="path1" d="M 10 10 L 90 90" stroke="black" stroke-width="2"/>
        </svg>"#;

        let result = parse_svg(svg);
        assert!(result.is_ok());

        let enriched = result.unwrap();
        assert!(enriched.tree.elements.contains_key("path1"));
    }

    #[test]
    fn test_parse_invalid_svg() {
        let svg = "not an svg";
        let result = parse_svg(svg);
        assert!(result.is_err());
    }

    #[test]
    fn solve_json_returns_full_result_and_rejects_bad_input() {
        let graph = serde_json::to_string(&graph::test_graph(&["a", "b"], &[])).unwrap();
        let graph = graph.as_str();
        let out: serde_json::Value =
            serde_json::from_str(&solve_json(graph, "[]", None).unwrap()).unwrap();
        assert!(out["converged"].is_boolean() && out["positions"]["a"].is_array());
        assert!(out["iterations"].as_u64().unwrap() > 0);

        let arity = r#"[{"id":"c","kind":"Distance","nodes":["a"],"params":{},"strength":1.0}]"#;
        assert!(matches!(
            solve_json(graph, arity, None),
            Err(Kr0kiSvgError::Solve(_))
        ));
        assert!(matches!(
            solve_json("not json", "[]", None),
            Err(Kr0kiSvgError::Json(_))
        ));
    }
}

/// WASM/JS entry point for the enhancement layer. `known_json`: `[{"id","type","name"}]`; `brand_json`: a brand package.
/// Returns `{"svg": "...", "report": {"indexed", "unindexed", "rewrites"}}`.
#[wasm_bindgen]
pub fn enhance_svg_json(svg: &str, known_json: &str, brand_json: &str) -> Result<String, JsValue> {
    let known: Vec<enhance::KnownElement> =
        serde_json::from_str(known_json).map_err(Kr0kiSvgError::from)?;
    let brand: enhance::Brand = serde_json::from_str(brand_json).map_err(Kr0kiSvgError::from)?;
    let (svg, report) =
        enhance::enhance(svg, &known, &brand).map_err(|e| JsValue::from_str(&e.to_string()))?;
    serde_json::to_string(&serde_json::json!({ "svg": svg, "report": report }))
        .map_err(|e| Kr0kiSvgError::from(e).into())
}
