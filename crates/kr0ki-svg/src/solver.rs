//! Constraint-aware force-directed layout for kr0ki-svg
//!
//! The physics is `fdg-sim`'s Fruchterman-Reingold on a `petgraph` `StableGraph`; this
//! module adds what it lacks: input validation, deterministic starts, constraint
//! corrections after each step, and an incremental mode that keeps an existing layout.

use crate::constraint::{Constraint, ConstraintError};
use crate::graph::{Graph, GraphError, NodeId};
use fdg_sim::{
    force,
    glam::{Vec2, Vec3},
    petgraph::{graph::NodeIndex, Undirected},
    ForceGraph, ForceGraphHelper,
};
use serde::Serialize;
use std::collections::HashMap;

/// Nodes closer than this (in layout units) are treated as co-located at start.
const COINCIDENT_EPS: f32 = 0.5;
/// Golden angle: successive offsets never line up, so a spiral spreads a stack evenly.
const GOLDEN_ANGLE: f32 = 2.399_963;

/// Configuration for the layout solver
#[derive(Debug, Clone)]
pub struct SolverConfig {
    /// Maximum number of iterations
    pub max_iterations: usize,

    /// Stop when the mean per-node movement in one iteration drops below this
    pub convergence_threshold: f32,

    /// Fruchterman-Reingold ideal edge length; also the spacing used to spread
    /// co-located nodes
    pub scale: f32,

    /// Per-iteration velocity damping in 0..=1 (lower cools faster)
    pub cooloff_factor: f32,

    /// Integration time step
    pub time_step: f32,

    /// Fraction of `time_step` used by [`GraphSolver::solve_incremental`], so an
    /// existing layout is nudged rather than re-annealed
    pub incremental_step_factor: f32,
}

impl Default for SolverConfig {
    fn default() -> Self {
        Self {
            max_iterations: 500,
            convergence_threshold: 0.01,
            scale: 45.0,
            cooloff_factor: 0.975,
            time_step: 0.035,
            incremental_step_factor: 0.25,
        }
    }
}

/// Result of a solve operation
#[derive(Debug, Clone, Serialize)]
pub struct SolveResult {
    /// Final node positions
    pub positions: HashMap<NodeId, (f64, f64)>,

    /// Number of iterations performed
    pub iterations: usize,

    /// Mean per-node movement in the last iteration
    pub residual: f32,

    /// Whether `residual` fell below the convergence threshold
    pub converged: bool,
}

/// Why a solve could not run (all are bad input, none are bugs worth a panic)
#[derive(Debug, Clone, thiserror::Error)]
pub enum SolveError {
    #[error(transparent)]
    Graph(#[from] GraphError),

    #[error(transparent)]
    Constraint(#[from] ConstraintError),

    #[error("non-finite start position for node '{0}'")]
    NonFiniteInput(NodeId),

    #[error("layout diverged to a non-finite position at iteration {0}")]
    Diverged(usize),
}

/// Force-directed layout solver
#[derive(Default)]
pub struct GraphSolver {
    config: SolverConfig,
    constraints: Vec<Constraint>,
}

impl GraphSolver {
    /// Create a new solver with default configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new solver with custom configuration
    pub fn with_config(config: SolverConfig) -> Self {
        Self {
            config,
            constraints: Vec::new(),
        }
    }

    /// Create a solver that enforces `constraints`
    pub fn with_constraints(constraints: Vec<Constraint>) -> Self {
        Self {
            constraints,
            ..Self::default()
        }
    }

    /// Add a constraint to the solver
    pub fn add_constraint(&mut self, constraint: Constraint) {
        self.constraints.push(constraint);
    }

    /// Clear all constraints
    pub fn clear_constraints(&mut self) {
        self.constraints.clear();
    }

    /// Solve the graph layout starting from each node's own `position`
    pub fn solve(&self, graph: &Graph) -> Result<SolveResult, SolveError> {
        self.run(graph, graph.positions(), self.config.time_step)
    }

    /// Continue from `previous` positions (for example after adding a node).
    ///
    /// Nodes missing from `previous` start at the centroid of their placed neighbours;
    /// entries for nodes no longer in the graph are ignored.
    pub fn solve_incremental(
        &self,
        graph: &Graph,
        previous: HashMap<NodeId, (f64, f64)>,
    ) -> Result<SolveResult, SolveError> {
        graph.validate()?;
        let start = seed_missing(graph, previous);
        let dt = self.config.time_step * self.config.incremental_step_factor;
        self.run(graph, start, dt)
    }

    fn run(
        &self,
        graph: &Graph,
        start: HashMap<NodeId, (f64, f64)>,
        dt: f32,
    ) -> Result<SolveResult, SolveError> {
        graph.validate()?;
        self.constraints
            .iter()
            .try_for_each(|c| c.validate(graph))?;

        // Sorted so the result is a pure function of the input (the cache is content-addressed).
        let mut ids: Vec<&NodeId> = graph.nodes.keys().collect();
        ids.sort();
        let mut fg: ForceGraph<(), ()> = ForceGraph::default();
        let mut index: HashMap<&str, NodeIndex> = HashMap::with_capacity(ids.len());
        for id in &ids {
            let (x, y) = start.get(*id).copied().unwrap_or(graph.nodes[*id].position);
            let at = Vec3::new(x as f32, y as f32, 0.0);
            if !at.is_finite() {
                return Err(SolveError::NonFiniteInput((*id).clone()));
            }
            index.insert(
                id.as_str(),
                fg.add_force_node_with_coords(id.as_str(), (), at),
            );
        }
        let mut edges: Vec<_> = graph.edges.values().collect();
        edges.sort_by(|a, b| a.id.cmp(&b.id));
        for e in edges {
            let endpoint = |id: &NodeId| {
                index
                    .get(id.as_str())
                    .copied()
                    .ok_or_else(|| GraphError::NodeNotFound(id.clone()))
            };
            fg.add_edge(endpoint(&e.source)?, endpoint(&e.target)?, ());
        }
        let order: Vec<NodeIndex> = ids.iter().map(|id| index[id.as_str()]).collect();
        spread_coincident(&mut fg, &order, self.config.scale);

        let fr = force::fruchterman_reingold::<(), (), Undirected>(
            self.config.scale,
            self.config.cooloff_factor,
        );
        let mut residual = f32::INFINITY;
        let mut iterations = 0;
        let mut before = vec![Vec3::ZERO; order.len()];
        while iterations < self.config.max_iterations {
            iterations += 1;
            for (slot, i) in before.iter_mut().zip(&order) {
                *slot = fg[*i].location;
            }
            fr.update(&mut fg, dt);
            self.apply_constraints(&mut fg, &index);

            if order.iter().any(|i| !fg[*i].location.is_finite()) {
                return Err(SolveError::Diverged(iterations));
            }
            residual = order
                .iter()
                .zip(&before)
                .map(|(i, b)| fg[*i].location.distance(*b))
                .sum::<f32>()
                / order.len().max(1) as f32;
            if residual < self.config.convergence_threshold {
                break;
            }
        }

        Ok(SolveResult {
            positions: ids
                .iter()
                .map(|id| {
                    let l = fg[index[id.as_str()]].location;
                    ((*id).clone(), (f64::from(l.x), f64::from(l.y)))
                })
                .collect(),
            iterations,
            residual,
            converged: residual < self.config.convergence_threshold,
        })
    }

    /// Add every constraint's correction (computed against the same snapshot) to the layout.
    fn apply_constraints(&self, fg: &mut ForceGraph<(), ()>, index: &HashMap<&str, NodeIndex>) {
        let deltas: Vec<(&str, Vec2)> = {
            let at = |id: &str| index.get(id).map(|i| fg[*i].location.truncate());
            self.constraints
                .iter()
                .flat_map(|c| c.corrections(&at))
                .collect()
        };
        for (id, d) in deltas {
            if let Some(i) = index.get(id) {
                fg[*i].location += d.extend(0.0);
            }
        }
    }
}

/// Offset all but the first of each group of coincident nodes along a golden-angle
/// spiral. The force model divides by node distance, so nodes stacked on one point
/// would otherwise never separate (or go NaN). `order` fixes who counts as "first".
fn spread_coincident(fg: &mut ForceGraph<(), ()>, order: &[NodeIndex], spacing: f32) {
    let mut groups: HashMap<(i64, i64), Vec<NodeIndex>> = HashMap::new();
    for i in order {
        let l = fg[*i].location;
        let cell = |v: f32| (v / COINCIDENT_EPS).round() as i64;
        groups.entry((cell(l.x), cell(l.y))).or_default().push(*i);
    }
    for members in groups.values().filter(|m| m.len() > 1) {
        for (k, i) in members.iter().enumerate().skip(1) {
            let angle = k as f32 * GOLDEN_ANGLE;
            let offset = Vec3::new(angle.cos(), angle.sin(), 0.0) * spacing * (k as f32).sqrt();
            fg[*i].location += offset;
            fg[*i].old_location = fg[*i].location;
        }
    }
}

/// `previous` restricted to `graph`'s nodes, with gaps filled: a missing node takes the
/// centroid of its already-placed neighbours, else of all placed nodes, else its own
/// `position`. Placements cascade, so a chain of new nodes settles outward.
fn seed_missing(
    graph: &Graph,
    mut previous: HashMap<NodeId, (f64, f64)>,
) -> HashMap<NodeId, (f64, f64)> {
    previous.retain(|id, _| graph.nodes.contains_key(id));
    let centroid = |pts: Vec<(f64, f64)>| {
        (!pts.is_empty()).then(|| {
            let n = pts.len() as f64;
            let (sx, sy) = pts.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
            (sx / n, sy / n)
        })
    };
    let mut missing: Vec<&NodeId> = graph
        .nodes
        .keys()
        .filter(|id| !previous.contains_key(*id))
        .collect();
    missing.sort();
    for id in missing {
        let near = graph
            .get_neighbors(id)
            .iter()
            .filter_map(|n| previous.get(n).copied())
            .collect();
        let all = previous.values().copied().collect();
        let at = centroid(near)
            .or_else(|| centroid(all))
            .unwrap_or(graph.nodes[id].position);
        previous.insert(id.clone(), at);
    }
    previous
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{test_graph, Edge, Node};

    fn dist(r: &SolveResult, a: &str, b: &str) -> f64 {
        let (pa, pb) = (r.positions[a], r.positions[b]);
        ((pa.0 - pb.0).powi(2) + (pa.1 - pb.1).powi(2)).sqrt()
    }

    #[test]
    fn basic_layout_converges_and_is_finite() {
        let g = test_graph(&["a", "b", "c"], &[("a", "b"), ("b", "c")]);
        let r = GraphSolver::new().solve(&g).unwrap();
        assert!(
            r.converged,
            "residual {} after {} iterations",
            r.residual, r.iterations
        );
        assert!(r
            .positions
            .values()
            .all(|p| p.0.is_finite() && p.1.is_finite()));
        assert!(
            dist(&r, "a", "c") > dist(&r, "a", "b"),
            "path layout should spread out"
        );
    }

    #[test]
    fn solve_is_deterministic() {
        let g = test_graph(&["a", "b", "c", "d"], &[("a", "b"), ("b", "c"), ("c", "d")]);
        let s = GraphSolver::new();
        let (r1, r2) = (s.solve(&g).unwrap(), s.solve(&g).unwrap());
        assert_eq!(r1.positions, r2.positions);
    }

    #[test]
    fn dangling_edge_is_an_error_not_a_panic() {
        let mut g = test_graph(&["a"], &[]);
        g.edges.insert("e".into(), Edge::new("e", "a", "ghost"));
        assert!(matches!(
            GraphSolver::new().solve(&g),
            Err(SolveError::Graph(GraphError::NodeNotFound(_)))
        ));
    }

    #[test]
    fn malformed_constraint_is_an_error_not_a_panic() {
        let g = test_graph(&["a", "b"], &[]);
        let mut bad = Constraint::distance("d", "a".into(), "b".into(), 10.0);
        bad.nodes.truncate(1);
        let s = GraphSolver::with_constraints(vec![bad]);
        assert!(matches!(s.solve(&g), Err(SolveError::Constraint(_))));
    }

    #[test]
    fn non_finite_input_is_rejected() {
        let mut g = test_graph(&["a", "b"], &[("a", "b")]);
        g.nodes.get_mut("a").unwrap().position = (f64::NAN, 0.0);
        assert!(
            matches!(GraphSolver::new().solve(&g), Err(SolveError::NonFiniteInput(id)) if id == "a")
        );
    }

    #[test]
    fn stacked_nodes_separate() {
        let mut g = Graph::new();
        for id in ["a", "b", "c"] {
            g.add_node(Node::new(id, id, (0.0, 0.0)));
        }
        g.add_edge(Edge::new("e1", "a", "b")).unwrap();
        let r = GraphSolver::new().solve(&g).unwrap();
        for (a, b) in [("a", "b"), ("a", "c"), ("b", "c")] {
            assert!(
                dist(&r, a, b) > 5.0,
                "{a}-{b} still stacked: {}",
                dist(&r, a, b)
            );
        }
    }

    #[test]
    fn constraints_are_satisfied_by_the_layout() {
        let g = test_graph(&["a", "b", "c"], &[("a", "b"), ("b", "c")]);
        let s = GraphSolver::with_constraints(vec![
            Constraint::align_horizontal("h", vec!["a".into(), "b".into(), "c".into()]),
            Constraint::distance("d", "a".into(), "b".into(), 120.0),
        ]);
        let r = s.solve(&g).unwrap();
        let ys: Vec<f64> = ["a", "b", "c"].iter().map(|n| r.positions[*n].1).collect();
        assert!(
            ys.iter().all(|y| (y - ys[0]).abs() < 1.0),
            "not aligned: {ys:?}"
        );
        assert!(
            (dist(&r, "a", "b") - 120.0).abs() < 5.0,
            "{}",
            dist(&r, "a", "b")
        );
    }

    #[test]
    fn incremental_handles_new_and_removed_nodes_without_scrambling() {
        let g = test_graph(&["a", "b", "c"], &[("a", "b"), ("b", "c")]);
        let solver = GraphSolver::new();
        let settled = solver.solve(&g).unwrap();

        // add "d", drop "c"; previous map still has c and lacks d
        let mut g2 = test_graph(&["a", "b", "d"], &[("a", "b"), ("b", "d")]);
        for id in ["a", "b"] {
            g2.nodes.get_mut(id).unwrap().position = settled.positions[id];
        }
        let r = solver
            .solve_incremental(&g2, settled.positions.clone())
            .unwrap();

        assert_eq!(r.positions.len(), 3);
        assert!(r.positions.contains_key("d") && !r.positions.contains_key("c"));
        let drift = |id: &str| {
            let (p, q) = (settled.positions[id], r.positions[id]);
            ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt()
        };
        assert!(
            drift("a") < 60.0 && drift("b") < 60.0,
            "{} {}",
            drift("a"),
            drift("b")
        );
    }

    #[test]
    fn seed_missing_uses_neighbour_centroid() {
        let g = test_graph(&["a", "b", "n"], &[("a", "n"), ("b", "n")]);
        let prev = HashMap::from([
            ("a".to_string(), (0.0, 0.0)),
            ("b".to_string(), (10.0, 20.0)),
        ]);
        assert_eq!(seed_missing(&g, prev)["n"], (5.0, 10.0));
    }
}
