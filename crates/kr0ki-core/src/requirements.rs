//! Backend-neutral requirements semantics and induced views.
//!
//! This module is deliberately independent from ReqIF XML, OPA, Flexo, HTTP,
//! and diagram grammars.  Adapters lower their data into [`RequirementGraph`];
//! consumers ask it for a [`ViewRequest`] and receive an induced typed graph.
//! Rendering lives in [`crate::requirements_render`], keeping the semantic
//! contract reusable by a Flexo extension or an MCP server.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

/// An immutable ReqIF/Flexo baseline identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaselineIdentity {
    pub id: String,
    /// Flexo commit or other immutable source revision.
    pub revision: String,
    /// Digest of the import artifact, when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import_artifact_sha256: Option<String>,
    /// Digest of the deterministic exported ReqIF baseline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exported_baseline_sha256: Option<String>,
}

/// Where a fact came from.  The URI is deliberately opaque: it can name a
/// ReqIF object, a Flexo RDF resource, or an OPA input artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub source_uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_sha256: Option<String>,
    #[serde(default)]
    pub locator: Option<String>,
}

/// A test, review record, analysis report, or other verification evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRef {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
}

/// One normalized requirement. Attributes retain ReqIF's extensible fields
/// without making the common attributes stringly typed at every call site.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Requirement {
    pub id: String,
    pub title: String,
    pub text: String,
    pub baseline: BaselineIdentity,
    pub provenance: Provenance,
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
}

/// The asserted relation vocabulary shared by ReqIF, Flexo, and OPA adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementRelationKind {
    Contains,
    Derives,
    Refines,
    Requires,
    Satisfies,
    Verifies,
    Implements,
    Traces,
    AllocatedTo,
    Precedes,
}

impl RequirementRelationKind {
    pub const DECOMPOSITION: &'static [Self] = &[Self::Contains, Self::Derives, Self::Refines];
    pub const TRACEABILITY: &'static [Self] = &[
        Self::Satisfies,
        Self::Verifies,
        Self::Implements,
        Self::Traces,
        Self::AllocatedTo,
    ];
}

/// The model/version that produced an inferred or proposed relation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelIdentity {
    pub name: String,
    pub version: String,
}

/// Information attached to an edge which is not yet authoritative.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NonAuthoritativeRelation {
    /// A finite score in `[0, 1]`; validation rejects anything else.
    pub confidence: f64,
    pub rationale: String,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    pub model: ModelIdentity,
}

/// Whether an edge is authoritative. Inferred and proposed edges are visible
/// in review views only and can affect authoritative views only after explicit
/// promotion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RelationAuthority {
    Asserted,
    Inferred(NonAuthoritativeRelation),
    Proposed(NonAuthoritativeRelation),
}

impl RelationAuthority {
    pub const fn is_asserted(&self) -> bool {
        matches!(self, Self::Asserted)
    }
}

/// Audit record made when a human promotes an inferred/proposed relation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Promotion {
    pub actor: String,
    pub rationale: String,
    pub previous_status: String,
}

/// A directed relation. Endpoints may refer to a requirement or evidence; this
/// permits verification and implementation traces to leave the ReqIF tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequirementRelation {
    pub id: String,
    pub source: String,
    pub target: String,
    pub kind: RequirementRelationKind,
    pub authority: RelationAuthority,
    pub provenance: Provenance,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promotion: Option<Promotion>,
}

impl RequirementRelation {
    /// Promote a review-only relation after an explicit human decision.
    pub fn promote(
        &mut self,
        actor: impl Into<String>,
        rationale: impl Into<String>,
    ) -> Result<(), RequirementError> {
        let previous_status = match &self.authority {
            RelationAuthority::Asserted => {
                return Err(RequirementError::AlreadyAsserted(self.id.clone()))
            }
            RelationAuthority::Inferred(_) => "inferred",
            RelationAuthority::Proposed(_) => "proposed",
        };
        self.authority = RelationAuthority::Asserted;
        self.promotion = Some(Promotion {
            actor: actor.into(),
            rationale: rationale.into(),
            previous_status: previous_status.to_string(),
        });
        Ok(())
    }
}

/// The normalized requirements graph consumed by all views.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequirementGraph {
    pub baseline: BaselineIdentity,
    #[serde(default)]
    pub requirements: Vec<Requirement>,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub relations: Vec<RequirementRelation>,
}

impl RequirementGraph {
    /// Promote by id. This is intentionally an explicit mutation, never an
    /// incidental side effect of a view request.
    pub fn promote_relation(
        &mut self,
        relation_id: &str,
        actor: impl Into<String>,
        rationale: impl Into<String>,
    ) -> Result<(), RequirementError> {
        let edge = self
            .relations
            .iter_mut()
            .find(|edge| edge.id == relation_id)
            .ok_or_else(|| RequirementError::UnknownRelation(relation_id.to_string()))?;
        edge.promote(actor, rationale)
    }

    /// Validates graph references and guardrails before any adapter persists it.
    pub fn validate(&self) -> Result<(), RequirementError> {
        let mut ids = BTreeSet::new();
        for requirement in &self.requirements {
            if !ids.insert(requirement.id.as_str()) {
                return Err(RequirementError::DuplicateNode(requirement.id.clone()));
            }
        }
        for evidence in &self.evidence {
            if !ids.insert(evidence.id.as_str()) {
                return Err(RequirementError::DuplicateNode(evidence.id.clone()));
            }
        }
        let mut relation_ids = BTreeSet::new();
        for edge in &self.relations {
            if !relation_ids.insert(edge.id.as_str()) {
                return Err(RequirementError::DuplicateRelation(edge.id.clone()));
            }
            if !ids.contains(edge.source.as_str()) || !ids.contains(edge.target.as_str()) {
                return Err(RequirementError::UnknownEndpoint(edge.id.clone()));
            }
            if let RelationAuthority::Inferred(details) | RelationAuthority::Proposed(details) =
                &edge.authority
            {
                if !(0.0..=1.0).contains(&details.confidence) || !details.confidence.is_finite() {
                    return Err(RequirementError::InvalidConfidence(edge.id.clone()));
                }
            }
        }
        Ok(())
    }

    /// Compute an induced typed view. This never produces renderer source.
    pub fn view(&self, request: &ViewRequest) -> Result<ViewResult, RequirementError> {
        self.validate()?;
        let visible: Vec<&RequirementRelation> = self
            .relations
            .iter()
            .filter(|edge| request.scope == ViewScope::Review || edge.authority.is_asserted())
            .collect();

        match request.kind {
            ViewKind::Decomposition => Ok(self.filtered_view(
                &visible,
                RequirementRelationKind::DECOMPOSITION,
                ViewDiagnostics::default(),
                None,
                None,
            )),
            ViewKind::Traceability => Ok(self.filtered_view(
                &visible,
                RequirementRelationKind::TRACEABILITY,
                ViewDiagnostics::default(),
                None,
                None,
            )),
            ViewKind::Impact => self.impact_view(&visible, request),
            ViewKind::Behaviour => self.behaviour_view(&visible, request),
            ViewKind::VerificationCoverage => self.coverage_view(&visible),
        }
    }

    fn filtered_view(
        &self,
        visible: &[&RequirementRelation],
        kinds: &[RequirementRelationKind],
        diagnostics: ViewDiagnostics,
        behaviour: Option<BehaviourRecommendation>,
        coverage: Option<VerificationCoverage>,
    ) -> ViewResult {
        self.induce(
            visible
                .iter()
                .copied()
                .filter(|edge| kinds.contains(&edge.kind)),
            diagnostics,
            behaviour,
            coverage,
        )
    }

    fn induce<'a>(
        &self,
        edges: impl IntoIterator<Item = &'a RequirementRelation>,
        diagnostics: ViewDiagnostics,
        behaviour: Option<BehaviourRecommendation>,
        coverage: Option<VerificationCoverage>,
    ) -> ViewResult {
        let relations: Vec<RequirementRelation> = edges.into_iter().cloned().collect();
        let ids: BTreeSet<&str> = relations
            .iter()
            .flat_map(|e| [e.source.as_str(), e.target.as_str()])
            .collect();
        ViewResult {
            graph: RequirementGraph {
                baseline: self.baseline.clone(),
                requirements: self
                    .requirements
                    .iter()
                    .filter(|n| ids.contains(n.id.as_str()))
                    .cloned()
                    .collect(),
                evidence: self
                    .evidence
                    .iter()
                    .filter(|n| ids.contains(n.id.as_str()))
                    .cloned()
                    .collect(),
                relations,
            },
            diagnostics,
            behaviour,
            coverage,
        }
    }

    fn impact_view(
        &self,
        visible: &[&RequirementRelation],
        request: &ViewRequest,
    ) -> Result<ViewResult, RequirementError> {
        let root = request
            .root_id
            .as_deref()
            .ok_or(RequirementError::ImpactNeedsRoot)?;
        if !self.has_node(root) {
            return Err(RequirementError::UnknownNode(root.to_string()));
        }
        let max_depth = request.max_depth.unwrap_or(3);
        let mut seen = BTreeSet::from([root.to_string()]);
        let mut queue = VecDeque::from([(root.to_string(), 0usize)]);
        let mut selected = BTreeSet::new();
        let mut cycles = BTreeSet::new();
        while let Some((node, depth)) = queue.pop_front() {
            if depth == max_depth {
                continue;
            }
            for edge in visible {
                let neighbor = match request.direction {
                    TraversalDirection::Downstream if edge.source == node => Some(&edge.target),
                    TraversalDirection::Upstream if edge.target == node => Some(&edge.source),
                    TraversalDirection::Both if edge.source == node => Some(&edge.target),
                    TraversalDirection::Both if edge.target == node => Some(&edge.source),
                    _ => None,
                };
                if let Some(neighbor) = neighbor {
                    selected.insert(edge.id.clone());
                    if !seen.insert(neighbor.clone()) {
                        cycles.insert(edge.id.clone());
                    } else {
                        queue.push_back((neighbor.clone(), depth + 1));
                    }
                }
            }
        }
        let diagnostics = ViewDiagnostics {
            cycles: cycles.into_iter().collect(),
            bounded_at_depth: Some(max_depth),
        };
        Ok(self.induce(
            visible
                .iter()
                .copied()
                .filter(|edge| selected.contains(&edge.id)),
            diagnostics,
            None,
            None,
        ))
    }

    fn behaviour_view(
        &self,
        visible: &[&RequirementRelation],
        request: &ViewRequest,
    ) -> Result<ViewResult, RequirementError> {
        let recommendation = self.recommend_behaviour(visible);
        if let Some(selected) = request.confirmed_behaviour {
            if selected != recommendation.recommended {
                return Err(RequirementError::BehaviourConfirmationMismatch {
                    recommended: recommendation.recommended,
                    selected,
                });
            }
        }
        let kinds: &[RequirementRelationKind] = match recommendation.recommended {
            BehaviourViewKind::Activity | BehaviourViewKind::Sequence => {
                &[RequirementRelationKind::Precedes]
            }
            BehaviourViewKind::State => &[
                RequirementRelationKind::Refines,
                RequirementRelationKind::Precedes,
            ],
        };
        Ok(self.filtered_view(
            visible,
            kinds,
            ViewDiagnostics::default(),
            Some(recommendation),
            None,
        ))
    }

    fn recommend_behaviour(&self, visible: &[&RequirementRelation]) -> BehaviourRecommendation {
        // Explicit semantic annotation wins. Otherwise a precedence relation
        // is a sequence; an unannotated model defaults to activity.
        let hint = self
            .requirements
            .iter()
            .find_map(|r| r.attributes.get("behaviour").map(String::as_str));
        let recommended = match hint {
            Some("state") => BehaviourViewKind::State,
            Some("sequence") => BehaviourViewKind::Sequence,
            Some("activity") => BehaviourViewKind::Activity,
            _ if visible
                .iter()
                .any(|edge| edge.kind == RequirementRelationKind::Precedes) =>
            {
                BehaviourViewKind::Sequence
            }
            _ => BehaviourViewKind::Activity,
        };
        BehaviourRecommendation {
            recommended,
            rationale: match hint {
                Some("state") => "explicit behaviour=state attribute".into(),
                Some("sequence") => "explicit behaviour=sequence attribute".into(),
                Some("activity") => "explicit behaviour=activity attribute".into(),
                _ if recommended == BehaviourViewKind::Sequence => {
                    "precedence relations imply an ordered interaction".into()
                }
                _ => {
                    "no state or precedence semantics; activity is the deterministic default".into()
                }
            },
        }
    }

    fn coverage_view(
        &self,
        visible: &[&RequirementRelation],
    ) -> Result<ViewResult, RequirementError> {
        let mut covered = BTreeMap::<String, Vec<String>>::new();
        let mut linked_evidence = BTreeSet::new();
        for edge in visible
            .iter()
            .filter(|e| e.kind == RequirementRelationKind::Verifies)
        {
            let requirement = if self.requirements.iter().any(|r| r.id == edge.target) {
                Some(&edge.target)
            } else if self.requirements.iter().any(|r| r.id == edge.source) {
                Some(&edge.source)
            } else {
                None
            };
            let evidence = if self.evidence.iter().any(|e| e.id == edge.source) {
                Some(&edge.source)
            } else if self.evidence.iter().any(|e| e.id == edge.target) {
                Some(&edge.target)
            } else {
                None
            };
            if let (Some(requirement), Some(evidence)) = (requirement, evidence) {
                covered
                    .entry(requirement.clone())
                    .or_default()
                    .push(evidence.clone());
                linked_evidence.insert(evidence.clone());
            }
        }
        for values in covered.values_mut() {
            values.sort();
            values.dedup();
        }
        let unverified = self
            .requirements
            .iter()
            .filter(|r| !covered.contains_key(&r.id))
            .map(|r| r.id.clone())
            .collect();
        let orphan_evidence = self
            .evidence
            .iter()
            .filter(|e| !linked_evidence.contains(&e.id))
            .map(|e| e.id.clone())
            .collect();
        let coverage = VerificationCoverage {
            covered,
            unverified,
            orphan_evidence,
        };
        Ok(self.induce(
            visible
                .iter()
                .copied()
                .filter(|edge| edge.kind == RequirementRelationKind::Verifies),
            ViewDiagnostics::default(),
            None,
            Some(coverage),
        ))
    }

    fn has_node(&self, id: &str) -> bool {
        self.requirements.iter().any(|n| n.id == id) || self.evidence.iter().any(|n| n.id == id)
    }
}

/// Five supported requirement viewpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewKind {
    Decomposition,
    Traceability,
    Impact,
    Behaviour,
    VerificationCoverage,
}

/// Authoritative views deliberately exclude inferred/proposed relations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ViewScope {
    #[default]
    Authoritative,
    Review,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TraversalDirection {
    Upstream,
    #[default]
    Downstream,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BehaviourViewKind {
    Activity,
    Sequence,
    State,
}

/// Request contract reusable by HTTP, MCP, Playb00k, and a Flexo extension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewRequest {
    pub kind: ViewKind,
    #[serde(default)]
    pub scope: ViewScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_depth: Option<usize>,
    #[serde(default)]
    pub direction: TraversalDirection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmed_behaviour: Option<BehaviourViewKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ViewDiagnostics {
    #[serde(default)]
    pub cycles: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounded_at_depth: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BehaviourRecommendation {
    pub recommended: BehaviourViewKind,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationCoverage {
    pub covered: BTreeMap<String, Vec<String>>,
    pub unverified: Vec<String>,
    pub orphan_evidence: Vec<String>,
}

/// A semantic view result. The `graph` is the only diagram input a renderer
/// receives; it has no knowledge of ReqIF XML or Flexo's storage layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewResult {
    pub graph: RequirementGraph,
    pub diagnostics: ViewDiagnostics,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behaviour: Option<BehaviourRecommendation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<VerificationCoverage>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RequirementError {
    #[error("duplicate graph node: {0}")]
    DuplicateNode(String),
    #[error("duplicate relation: {0}")]
    DuplicateRelation(String),
    #[error("relation has an unknown endpoint: {0}")]
    UnknownEndpoint(String),
    #[error("relation confidence must be finite and between zero and one: {0}")]
    InvalidConfidence(String),
    #[error("unknown relation: {0}")]
    UnknownRelation(String),
    #[error("relation is already asserted: {0}")]
    AlreadyAsserted(String),
    #[error("unknown graph node: {0}")]
    UnknownNode(String),
    #[error("impact viewpoint needs root_id")]
    ImpactNeedsRoot,
    #[error("behaviour selection ({selected:?}) disagrees with deterministic recommendation ({recommended:?})")]
    BehaviourConfirmationMismatch {
        recommended: BehaviourViewKind,
        selected: BehaviourViewKind,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn baseline() -> BaselineIdentity {
        BaselineIdentity {
            id: "BL-1".into(),
            revision: "commit-1".into(),
            import_artifact_sha256: None,
            exported_baseline_sha256: None,
        }
    }
    fn provenance() -> Provenance {
        Provenance {
            source_uri: "reqif://fixture".into(),
            artifact_sha256: None,
            locator: None,
        }
    }
    fn requirement(id: &str) -> Requirement {
        Requirement {
            id: id.into(),
            title: id.into(),
            text: id.into(),
            baseline: baseline(),
            provenance: provenance(),
            attributes: BTreeMap::new(),
            evidence: vec![],
        }
    }
    fn edge(
        id: &str,
        source: &str,
        target: &str,
        kind: RequirementRelationKind,
        authority: RelationAuthority,
    ) -> RequirementRelation {
        RequirementRelation {
            id: id.into(),
            source: source.into(),
            target: target.into(),
            kind,
            authority,
            provenance: provenance(),
            promotion: None,
        }
    }
    fn graph() -> RequirementGraph {
        RequirementGraph {
            baseline: baseline(),
            requirements: vec![requirement("r1"), requirement("r2"), requirement("r3")],
            evidence: vec![
                EvidenceRef {
                    id: "t1".into(),
                    label: "test 1".into(),
                    uri: None,
                    provenance: None,
                },
                EvidenceRef {
                    id: "t2".into(),
                    label: "orphan test".into(),
                    uri: None,
                    provenance: None,
                },
            ],
            relations: vec![
                edge(
                    "contains",
                    "r1",
                    "r2",
                    RequirementRelationKind::Contains,
                    RelationAuthority::Asserted,
                ),
                edge(
                    "precedes",
                    "r2",
                    "r3",
                    RequirementRelationKind::Precedes,
                    RelationAuthority::Asserted,
                ),
                edge(
                    "verify",
                    "t1",
                    "r2",
                    RequirementRelationKind::Verifies,
                    RelationAuthority::Asserted,
                ),
                edge(
                    "candidate",
                    "r1",
                    "r3",
                    RequirementRelationKind::Derives,
                    RelationAuthority::Proposed(NonAuthoritativeRelation {
                        confidence: 0.8,
                        rationale: "candidate".into(),
                        evidence: vec![],
                        model: ModelIdentity {
                            name: "opa".into(),
                            version: "1".into(),
                        },
                    }),
                ),
            ],
        }
    }

    #[test]
    fn authoritative_views_exclude_proposed_edges_until_explicitly_promoted() {
        let mut g = graph();
        let request = ViewRequest {
            kind: ViewKind::Decomposition,
            scope: ViewScope::Authoritative,
            root_id: None,
            max_depth: None,
            direction: TraversalDirection::Downstream,
            confirmed_behaviour: None,
        };
        assert_eq!(g.view(&request).unwrap().graph.relations.len(), 1);
        g.promote_relation("candidate", "alice", "reviewed")
            .unwrap();
        let result = g.view(&request).unwrap();
        assert_eq!(result.graph.relations.len(), 2);
        assert_eq!(
            result.graph.relations[1]
                .promotion
                .as_ref()
                .unwrap()
                .previous_status,
            "proposed"
        );
    }

    #[test]
    fn impact_is_bounded_and_reports_revisited_edges() {
        let mut g = graph();
        g.relations.push(edge(
            "cycle",
            "r3",
            "r2",
            RequirementRelationKind::Requires,
            RelationAuthority::Asserted,
        ));
        let result = g
            .view(&ViewRequest {
                kind: ViewKind::Impact,
                scope: ViewScope::Authoritative,
                root_id: Some("r2".into()),
                max_depth: Some(3),
                direction: TraversalDirection::Downstream,
                confirmed_behaviour: None,
            })
            .unwrap();
        assert_eq!(result.diagnostics.bounded_at_depth, Some(3));
        assert!(result.diagnostics.cycles.contains(&"cycle".to_string()));
    }

    #[test]
    fn behaviour_is_deterministic_and_requires_matching_confirmation() {
        let g = graph();
        let ok = g
            .view(&ViewRequest {
                kind: ViewKind::Behaviour,
                scope: ViewScope::Authoritative,
                root_id: None,
                max_depth: None,
                direction: TraversalDirection::Downstream,
                confirmed_behaviour: Some(BehaviourViewKind::Sequence),
            })
            .unwrap();
        assert_eq!(
            ok.behaviour.unwrap().recommended,
            BehaviourViewKind::Sequence
        );
        let err = g
            .view(&ViewRequest {
                kind: ViewKind::Behaviour,
                scope: ViewScope::Authoritative,
                root_id: None,
                max_depth: None,
                direction: TraversalDirection::Downstream,
                confirmed_behaviour: Some(BehaviourViewKind::State),
            })
            .unwrap_err();
        assert!(matches!(
            err,
            RequirementError::BehaviourConfirmationMismatch { .. }
        ));
    }

    #[test]
    fn coverage_returns_unverified_and_orphan_findings() {
        let result = graph()
            .view(&ViewRequest {
                kind: ViewKind::VerificationCoverage,
                scope: ViewScope::Authoritative,
                root_id: None,
                max_depth: None,
                direction: TraversalDirection::Downstream,
                confirmed_behaviour: None,
            })
            .unwrap();
        let coverage = result.coverage.unwrap();
        assert_eq!(coverage.covered["r2"], vec!["t1"]);
        assert!(coverage.unverified.contains(&"r1".to_string()));
        assert_eq!(coverage.orphan_evidence, vec!["t2"]);
    }
}
