//! Constraint system for kr0ki-svg graph layout
//!
//! A [`Constraint`] is data (it round-trips through JSON from the browser). Each
//! solver iteration asks it for position *corrections* ([`Constraint::corrections`]);
//! the solver adds them after the force-directed step. Corrections are scaled by
//! `strength` (clamped to 0..=1), so strength 1.0 satisfies a constraint in one step.

use crate::graph::{Graph, NodeId};
use fdg_sim::glam::Vec2;
use serde::{Deserialize, Serialize};

/// Represents a constraint between graph elements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    /// Unique identifier
    pub id: String,

    /// Constraint type
    pub kind: ConstraintKind,

    /// Nodes involved in the constraint
    pub nodes: Vec<NodeId>,

    /// Constraint parameters
    pub params: ConstraintParams,

    /// Constraint strength (0.0 to 1.0)
    pub strength: f64,
}

/// Types of constraints
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConstraintKind {
    /// Maintain a specific distance between two nodes
    Distance,

    /// Maintain a specific angle between three nodes (vertex is the middle node)
    Angle,

    /// Align nodes horizontally
    AlignHorizontal,

    /// Align nodes vertically
    AlignVertical,

    /// Keep one node within bounds
    Boundary,

    /// Prevent two nodes from overlapping
    NonOverlap,

    /// Maintain relative position (offset) of the second node from the first
    RelativePosition,
}

impl ConstraintKind {
    /// Inclusive `(min, max)` number of nodes the kind operates on.
    fn arity(self) -> (usize, usize) {
        match self {
            Self::Boundary => (1, 1),
            Self::Distance | Self::NonOverlap | Self::RelativePosition => (2, 2),
            Self::Angle => (3, 3),
            Self::AlignHorizontal | Self::AlignVertical => (2, usize::MAX),
        }
    }
}

/// Parameters for constraints
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConstraintParams {
    /// Target distance (for Distance constraint, default 100)
    pub distance: Option<f64>,

    /// Target angle in radians, 0..=π (for Angle constraint, default π/2)
    pub angle: Option<f64>,

    /// Boundary rectangle (required for Boundary constraint)
    pub bounds: Option<Bounds>,

    /// Relative offset (required for RelativePosition constraint)
    pub offset: Option<(f64, f64)>,

    /// Minimum distance (for NonOverlap constraint, default 50)
    pub min_distance: Option<f64>,
}

/// Rectangular boundary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// A constraint that cannot be applied to the graph it was given.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("constraint '{id}': {reason}")]
pub struct ConstraintError {
    pub id: String,
    pub reason: String,
}

impl Constraint {
    fn new(id: &str, kind: ConstraintKind, nodes: Vec<NodeId>, params: ConstraintParams) -> Self {
        Self {
            id: id.to_string(),
            kind,
            nodes,
            params,
            strength: 1.0,
        }
    }

    /// Create a distance constraint
    pub fn distance(id: &str, node1: NodeId, node2: NodeId, distance: f64) -> Self {
        let params = ConstraintParams {
            distance: Some(distance),
            ..Default::default()
        };
        Self::new(id, ConstraintKind::Distance, vec![node1, node2], params)
    }

    /// Create an angle constraint (angle at `node2`)
    pub fn angle(id: &str, node1: NodeId, node2: NodeId, node3: NodeId, angle: f64) -> Self {
        let params = ConstraintParams {
            angle: Some(angle),
            ..Default::default()
        };
        Self::new(id, ConstraintKind::Angle, vec![node1, node2, node3], params)
    }

    /// Create a horizontal alignment constraint
    pub fn align_horizontal(id: &str, nodes: Vec<NodeId>) -> Self {
        Self::new(
            id,
            ConstraintKind::AlignHorizontal,
            nodes,
            Default::default(),
        )
    }

    /// Create a vertical alignment constraint
    pub fn align_vertical(id: &str, nodes: Vec<NodeId>) -> Self {
        Self::new(id, ConstraintKind::AlignVertical, nodes, Default::default())
    }

    /// Create a boundary constraint
    pub fn boundary(id: &str, node: NodeId, bounds: Bounds) -> Self {
        let params = ConstraintParams {
            bounds: Some(bounds),
            ..Default::default()
        };
        Self::new(id, ConstraintKind::Boundary, vec![node], params)
    }

    /// Create a non-overlap constraint
    pub fn non_overlap(id: &str, node1: NodeId, node2: NodeId, min_distance: f64) -> Self {
        let params = ConstraintParams {
            min_distance: Some(min_distance),
            ..Default::default()
        };
        Self::new(id, ConstraintKind::NonOverlap, vec![node1, node2], params)
    }

    /// Create a relative position constraint
    pub fn relative_position(id: &str, node1: NodeId, node2: NodeId, offset: (f64, f64)) -> Self {
        let params = ConstraintParams {
            offset: Some(offset),
            ..Default::default()
        };
        Self::new(
            id,
            ConstraintKind::RelativePosition,
            vec![node1, node2],
            params,
        )
    }

    /// Check the constraint is well-formed for `graph`.
    ///
    /// Constraints arrive as untrusted JSON (serde bypasses the constructors above), so
    /// the solver validates before indexing `nodes` or reading required params.
    pub fn validate(&self, graph: &Graph) -> Result<(), ConstraintError> {
        let fail = |reason: String| ConstraintError {
            id: self.id.clone(),
            reason,
        };
        let (min, max) = self.kind.arity();
        if !(min..=max).contains(&self.nodes.len()) {
            return Err(fail(format!(
                "{:?} needs {} node(s), got {}",
                self.kind,
                if min == max {
                    min.to_string()
                } else {
                    format!("{min}+")
                },
                self.nodes.len()
            )));
        }
        if let Some(missing) = self.nodes.iter().find(|n| !graph.nodes.contains_key(*n)) {
            return Err(fail(format!("unknown node '{missing}'")));
        }
        if !self.strength.is_finite() {
            return Err(fail("strength must be finite".into()));
        }
        match (self.kind, &self.params) {
            (ConstraintKind::Boundary, ConstraintParams { bounds: None, .. }) => {
                Err(fail("Boundary requires params.bounds".into()))
            }
            (ConstraintKind::RelativePosition, ConstraintParams { offset: None, .. }) => {
                Err(fail("RelativePosition requires params.offset".into()))
            }
            _ => Ok(()),
        }
    }

    /// Position corrections (one per node, in `nodes` order) that move the layout toward
    /// satisfying this constraint. `at` resolves a node id to its current position; if any
    /// node is unresolvable (only possible if [`Constraint::validate`] was skipped) the
    /// constraint contributes nothing rather than guessing a position.
    pub fn corrections(&self, at: &impl Fn(&str) -> Option<Vec2>) -> Vec<(&str, Vec2)> {
        let Some(p) = self
            .nodes
            .iter()
            .map(|n| at(n))
            .collect::<Option<Vec<Vec2>>>()
        else {
            return Vec::new();
        };
        let s = self.strength.clamp(0.0, 1.0) as f32;
        let deltas = match self.kind {
            ConstraintKind::Distance => {
                let d = self.params.distance.unwrap_or(100.0) as f32;
                pair_toward(&p, Some(direction(p[1] - p[0]) * d), s)
            }
            ConstraintKind::NonOverlap => {
                let min = self.params.min_distance.unwrap_or(50.0) as f32;
                let delta = p[1] - p[0];
                let target = (delta.length() < min).then(|| direction(delta) * min);
                pair_toward(&p, target, s)
            }
            ConstraintKind::RelativePosition => {
                let offset = self
                    .params
                    .offset
                    .map(|(x, y)| Vec2::new(x as f32, y as f32));
                pair_toward(&p, offset, s)
            }
            ConstraintKind::AlignHorizontal => align(&p, 1, s),
            ConstraintKind::AlignVertical => align(&p, 0, s),
            ConstraintKind::Boundary => self.params.bounds.as_ref().map_or_else(
                || vec![Vec2::ZERO],
                |b| {
                    let min = Vec2::new(b.x as f32, b.y as f32);
                    let max = min + Vec2::new(b.width as f32, b.height as f32);
                    vec![(p[0].clamp(min, max) - p[0]) * s]
                },
            ),
            ConstraintKind::Angle => {
                let target = self.params.angle.unwrap_or(std::f64::consts::FRAC_PI_2) as f32;
                angle_at_vertex(&p, target, s)
            }
        };
        self.nodes.iter().map(String::as_str).zip(deltas).collect()
    }
}

/// Unit vector along `v`; a fixed axis when `v` is ~zero so coincident nodes still separate
/// deterministically instead of producing NaN.
fn direction(v: Vec2) -> Vec2 {
    v.try_normalize().unwrap_or(Vec2::X)
}

/// Move a node pair so that `p[1] - p[0]` approaches `target`, splitting the correction
/// equally. `None` means already satisfied. Shared by Distance/NonOverlap/RelativePosition.
fn pair_toward(p: &[Vec2], target: Option<Vec2>, s: f32) -> Vec<Vec2> {
    let correction = target.map_or(Vec2::ZERO, |t| ((p[1] - p[0]) - t) * 0.5 * s);
    vec![correction, -correction]
}

/// Pull every node's coordinate on `axis` (0 = x, 1 = y) toward the group mean.
fn align(p: &[Vec2], axis: usize, s: f32) -> Vec<Vec2> {
    let mean = p.iter().map(|v| v[axis]).sum::<f32>() / p.len() as f32;
    p.iter()
        .map(|v| {
            let mut d = Vec2::ZERO;
            d[axis] = (mean - v[axis]) * s;
            d
        })
        .collect()
}

/// Rotate the outer nodes about the vertex `p[1]` so the angle between them approaches
/// `target` (unsigned, 0..=π). The vertex itself does not move.
fn angle_at_vertex(p: &[Vec2], target: f32, s: f32) -> Vec<Vec2> {
    let (v1, v2) = (p[0] - p[1], p[2] - p[1]);
    if v1.length_squared() < f32::EPSILON || v2.length_squared() < f32::EPSILON {
        return vec![Vec2::ZERO; 3];
    }
    let current = v1.angle_between(v2); // signed, in [-π, π]
    let wanted = if current < 0.0 { -target } else { target };
    let half = (wanted - current) * 0.5 * s;
    let rotate = |v: Vec2, by: f32| Vec2::from_angle(by).rotate(v);
    vec![rotate(v1, -half) - v1, Vec2::ZERO, rotate(v2, half) - v2]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::test_graph;
    use std::collections::HashMap;

    /// Apply corrections once and return new positions.
    fn step(c: &Constraint, pos: &HashMap<&str, Vec2>) -> HashMap<String, Vec2> {
        let at = |id: &str| pos.get(id).copied();
        let mut out: HashMap<String, Vec2> = pos.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        for (id, d) in c.corrections(&at) {
            *out.get_mut(id).unwrap() += d;
        }
        out
    }

    fn pts<'a>(v: &[(&'a str, f32, f32)]) -> HashMap<&'a str, Vec2> {
        v.iter().map(|(k, x, y)| (*k, Vec2::new(*x, *y))).collect()
    }

    #[test]
    fn distance_converges_in_one_step_at_full_strength() {
        let c = Constraint::distance("d", "a".into(), "b".into(), 100.0);
        let out = step(&c, &pts(&[("a", 0.0, 0.0), ("b", 300.0, 0.0)]));
        assert!((out["a"].distance(out["b"]) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn coincident_nodes_do_not_produce_nan() {
        for c in [
            Constraint::distance("d", "a".into(), "b".into(), 100.0),
            Constraint::non_overlap("n", "a".into(), "b".into(), 50.0),
            Constraint::angle("g", "a".into(), "b".into(), "c".into(), 1.0),
        ] {
            let out = step(
                &c,
                &pts(&[("a", 5.0, 5.0), ("b", 5.0, 5.0), ("c", 5.0, 5.0)]),
            );
            assert!(out.values().all(|v| v.is_finite()), "{:?}", c.kind);
        }
        let out = step(
            &Constraint::non_overlap("n", "a".into(), "b".into(), 50.0),
            &pts(&[("a", 5.0, 5.0), ("b", 5.0, 5.0)]),
        );
        assert!(
            out["a"].distance(out["b"]) > 1.0,
            "co-located pair must separate"
        );
    }

    #[test]
    fn non_overlap_ignores_pairs_already_far_enough() {
        let c = Constraint::non_overlap("n", "a".into(), "b".into(), 50.0);
        let out = step(&c, &pts(&[("a", 0.0, 0.0), ("b", 80.0, 0.0)]));
        assert_eq!(out["b"], Vec2::new(80.0, 0.0));
    }

    #[test]
    fn right_angle_is_a_fixed_point_and_other_angles_move_toward_it() {
        let c = Constraint::angle(
            "g",
            "a".into(),
            "b".into(),
            "c".into(),
            std::f64::consts::FRAC_PI_2,
        );
        let right = pts(&[("a", 10.0, 0.0), ("b", 0.0, 0.0), ("c", 0.0, 10.0)]);
        let out = step(&c, &right);
        assert!(out
            .iter()
            .all(|(k, v)| v.distance(right[k.as_str()]) < 1e-4));

        let acute = pts(&[("a", 10.0, 0.0), ("b", 0.0, 0.0), ("c", 10.0, 10.0)]);
        let out = step(&c, &acute);
        let after = (out["a"] - out["b"])
            .angle_between(out["c"] - out["b"])
            .abs();
        assert!((after - std::f32::consts::FRAC_PI_2).abs() < 1e-3);
        // mirrored (negative-orientation) triangle corrects the same way
        let mirrored = pts(&[("a", 10.0, 0.0), ("b", 0.0, 0.0), ("c", 10.0, -10.0)]);
        let out = step(&c, &mirrored);
        let after = (out["a"] - out["b"])
            .angle_between(out["c"] - out["b"])
            .abs();
        assert!((after - std::f32::consts::FRAC_PI_2).abs() < 1e-3);
    }

    #[test]
    fn align_and_boundary_and_relative() {
        let out = step(
            &Constraint::align_horizontal("h", vec!["a".into(), "b".into()]),
            &pts(&[("a", 0.0, 0.0), ("b", 10.0, 20.0)]),
        );
        assert_eq!((out["a"].y, out["b"].y), (10.0, 10.0));

        let c = Constraint::boundary(
            "b",
            "a".into(),
            Bounds {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
        );
        assert_eq!(
            step(&c, &pts(&[("a", 50.0, -5.0)]))["a"],
            Vec2::new(10.0, 0.0)
        );

        let c = Constraint::relative_position("r", "a".into(), "b".into(), (30.0, 0.0));
        let out = step(&c, &pts(&[("a", 0.0, 0.0), ("b", 0.0, 40.0)]));
        assert!(((out["b"] - out["a"]) - Vec2::new(30.0, 0.0)).length() < 1e-4);
    }

    #[test]
    fn validate_rejects_untrusted_shapes() {
        let g = test_graph(&["a", "b"], &[]);
        let ok = Constraint::distance("d", "a".into(), "b".into(), 10.0);
        assert!(ok.validate(&g).is_ok());

        let mut bad_arity = ok.clone();
        bad_arity.nodes = vec!["a".into()];
        assert!(bad_arity
            .validate(&g)
            .unwrap_err()
            .reason
            .contains("needs 2"));

        let mut empty = ok.clone();
        empty.nodes.clear();
        assert!(empty.validate(&g).is_err());

        let unknown = Constraint::distance("d", "a".into(), "ghost".into(), 10.0);
        assert!(unknown.validate(&g).unwrap_err().reason.contains("ghost"));

        let mut no_bounds = Constraint::boundary(
            "b",
            "a".into(),
            Bounds {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
        );
        no_bounds.params.bounds = None;
        assert!(no_bounds.validate(&g).is_err());

        let mut nan = ok;
        nan.strength = f64::NAN;
        assert!(nan.validate(&g).is_err());
    }
}
