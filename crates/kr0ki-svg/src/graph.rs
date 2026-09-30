//! Graph data structures for kr0ki-svg enrichment
//!
//! Provides Node, Edge, and Graph types for representing diagram relationships.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for graph elements
pub type NodeId = String;
pub type EdgeId = String;

/// Represents a node in the graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// Unique identifier
    pub id: NodeId,

    /// Display label
    pub label: String,

    /// Position in 2D space (x, y)
    pub position: (f64, f64),

    /// Size (width, height)
    pub size: (f64, f64),

    /// Visual properties
    pub style: NodeStyle,

    /// Custom metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Node {
    /// A node with default size and style at `position`.
    pub fn new(id: impl Into<NodeId>, label: impl Into<String>, position: (f64, f64)) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            position,
            size: (100.0, 50.0),
            style: NodeStyle::default(),
            metadata: HashMap::new(),
        }
    }
}

/// Visual styling for nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStyle {
    /// Fill color (hex)
    pub fill: String,

    /// Stroke color (hex)
    pub stroke: String,

    /// Stroke width
    pub stroke_width: f64,

    /// Corner radius
    pub corner_radius: f64,

    /// Font size
    pub font_size: f64,
}

impl Default for NodeStyle {
    fn default() -> Self {
        Self {
            fill: "#ffffff".to_string(),
            stroke: "#000000".to_string(),
            stroke_width: 1.0,
            corner_radius: 4.0,
            font_size: 12.0,
        }
    }
}

/// Represents an edge (connection) between nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// Unique identifier
    pub id: EdgeId,

    /// Source node ID
    pub source: NodeId,

    /// Target node ID
    pub target: NodeId,

    /// Display label
    pub label: Option<String>,

    /// Visual properties
    pub style: EdgeStyle,

    /// Custom metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Edge {
    /// An unlabelled edge with default style.
    pub fn new(
        id: impl Into<EdgeId>,
        source: impl Into<NodeId>,
        target: impl Into<NodeId>,
    ) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
            target: target.into(),
            label: None,
            style: EdgeStyle::default(),
            metadata: HashMap::new(),
        }
    }
}

/// Visual styling for edges
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeStyle {
    /// Stroke color (hex)
    pub stroke: String,

    /// Stroke width
    pub stroke_width: f64,

    /// Arrow style (none, start, end, both)
    pub arrow: ArrowStyle,

    /// Line style (solid, dashed, dotted)
    pub line_style: LineStyle,
}

impl Default for EdgeStyle {
    fn default() -> Self {
        Self {
            stroke: "#000000".to_string(),
            stroke_width: 1.0,
            arrow: ArrowStyle::End,
            line_style: LineStyle::Solid,
        }
    }
}

/// Arrow style for edges
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArrowStyle {
    None,
    Start,
    End,
    Both,
}

/// Line style for edges
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LineStyle {
    Solid,
    Dashed,
    Dotted,
}

/// Represents a complete graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Graph {
    /// All nodes in the graph
    pub nodes: HashMap<NodeId, Node>,

    /// All edges in the graph
    pub edges: HashMap<EdgeId, Edge>,

    /// Graph metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Graph {
    /// Create a new empty graph
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            metadata: HashMap::new(),
        }
    }

    /// Add a node to the graph
    pub fn add_node(&mut self, node: Node) {
        self.nodes.insert(node.id.clone(), node);
    }

    /// Add an edge to the graph, rejecting dangling endpoints
    pub fn add_edge(&mut self, edge: Edge) -> Result<(), GraphError> {
        self.check_edge(&edge)?;
        self.edges.insert(edge.id.clone(), edge);
        Ok(())
    }

    /// Check every edge endpoint exists.
    ///
    /// `Deserialize` builds a `Graph` field-by-field and bypasses [`Graph::add_edge`],
    /// so anything that accepts untrusted JSON must call this before using the graph.
    pub fn validate(&self) -> Result<(), GraphError> {
        self.edges.values().try_for_each(|e| self.check_edge(e))
    }

    fn check_edge(&self, edge: &Edge) -> Result<(), GraphError> {
        [&edge.source, &edge.target]
            .into_iter()
            .find(|id| !self.nodes.contains_key(*id))
            .map_or(Ok(()), |id| Err(GraphError::NodeNotFound(id.clone())))
    }

    /// Current node positions keyed by id
    pub fn positions(&self) -> HashMap<NodeId, (f64, f64)> {
        self.nodes
            .iter()
            .map(|(id, n)| (id.clone(), n.position))
            .collect()
    }

    /// Get a node by ID
    pub fn get_node(&self, id: &str) -> Option<&Node> {
        self.nodes.get(id)
    }

    /// Get a mutable reference to a node
    pub fn get_node_mut(&mut self, id: &str) -> Option<&mut Node> {
        self.nodes.get_mut(id)
    }

    /// Get an edge by ID
    pub fn get_edge(&self, id: &str) -> Option<&Edge> {
        self.edges.get(id)
    }

    /// Get all edges connected to a node
    pub fn get_connected_edges(&self, node_id: &str) -> Vec<&Edge> {
        self.edges
            .values()
            .filter(|e| e.source == node_id || e.target == node_id)
            .collect()
    }

    /// Get all neighbors of a node
    pub fn get_neighbors(&self, node_id: &str) -> Vec<NodeId> {
        let mut neighbors = Vec::new();
        for edge in self.edges.values() {
            if edge.source == node_id {
                neighbors.push(edge.target.clone());
            } else if edge.target == node_id {
                neighbors.push(edge.source.clone());
            }
        }
        neighbors
    }

    /// Remove a node and all connected edges
    pub fn remove_node(&mut self, id: &str) -> Result<(), GraphError> {
        if !self.nodes.contains_key(id) {
            return Err(GraphError::NodeNotFound(id.to_string()));
        }

        // Remove all connected edges
        let edges_to_remove: Vec<EdgeId> = self
            .edges
            .values()
            .filter(|e| e.source == id || e.target == id)
            .map(|e| e.id.clone())
            .collect();

        for edge_id in edges_to_remove {
            self.edges.remove(&edge_id);
        }

        // Remove the node
        self.nodes.remove(id);
        Ok(())
    }

    /// Remove an edge
    pub fn remove_edge(&mut self, id: &str) -> Result<(), GraphError> {
        if self.edges.remove(id).is_none() {
            return Err(GraphError::EdgeNotFound(id.to_string()));
        }
        Ok(())
    }

    /// Get the number of nodes
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get the number of edges
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
}

impl Default for Graph {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur during graph operations
#[derive(Debug, Clone, thiserror::Error)]
pub enum GraphError {
    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Edge not found: {0}")]
    EdgeNotFound(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}

/// Test fixture: nodes spaced 200 apart on the x axis, edges named `e0`, `e1`, ...
#[cfg(test)]
pub(crate) fn test_graph(ids: &[&str], edges: &[(&str, &str)]) -> Graph {
    let mut g = Graph::new();
    for (i, id) in ids.iter().enumerate() {
        g.add_node(Node::new(*id, *id, (i as f64 * 200.0, 0.0)));
    }
    for (i, (s, t)) in edges.iter().enumerate() {
        g.add_edge(Edge::new(format!("e{i}"), *s, *t)).unwrap();
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_graph() {
        let g = test_graph(&["n1", "n2"], &[]);
        assert_eq!(g.node_count(), 2);
        assert!(g.get_node("n1").is_some() && g.get_node("n2").is_some());
    }

    #[test]
    fn test_add_edge() {
        let mut g = test_graph(&["n1", "n2"], &[]);
        assert!(g.add_edge(Edge::new("e1", "n1", "n2")).is_ok());
        assert_eq!(g.edge_count(), 1);
    }

    #[test]
    fn test_add_edge_invalid_node() {
        let mut g = Graph::new();
        assert!(g.add_edge(Edge::new("e1", "n1", "n2")).is_err());
    }

    #[test]
    fn test_validate_catches_deserialized_dangling_edge() {
        let mut g = test_graph(&["a"], &[]);
        g.edges.insert("e1".into(), Edge::new("e1", "a", "ghost"));
        assert!(matches!(g.validate(), Err(GraphError::NodeNotFound(id)) if id == "ghost"));
    }

    #[test]
    fn test_get_neighbors() {
        let mut g = test_graph(&["n1", "n2", "n3"], &[]);
        g.add_edge(Edge::new("e1", "n1", "n2")).unwrap();
        g.add_edge(Edge::new("e2", "n1", "n3")).unwrap();
        let mut neighbors = g.get_neighbors("n1");
        neighbors.sort();
        assert_eq!(neighbors, ["n2", "n3"]);
    }
}
