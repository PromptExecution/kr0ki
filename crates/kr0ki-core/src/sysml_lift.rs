//! Box 3 → box 4 lift: `ufo_types::ontology::UfoRelation` (the canonical UFO
//! semantic graph) → `ufo_types::sysml_model::Relation` (KerML/SysML-v2
//! constructs).
//!
//! `docs/PATTERNS-kubernetes.md` §4 names this mapping but leaves it
//! "still design-only, no code" — this module is that code. It is the step
//! `docs/TODO.md` box 5 names as blocking both FR1 (the `iso_ir → Mermaid/D2`
//! adapter) and FR4 (per-`ViewDefinition` rendering): both consume
//! `ufo_types::sysml_model::Relation`, and neither [`crate::ufo_graph`] nor
//! [`crate::k8s_recognizer`] (box 1→2, box 3's recognizer stage) produces
//! that — they stop at `OntologicalEdge`.
//!
//! ```text
//! Vec<OntologicalEdge> ──▶ [THIS] ──▶ Vec<LiftedRelation>
//!   (box 2/3 output)                   (box 4 input, one per edge)
//! ```
//!
//! # Mapping
//!
//! Follows `PATTERNS-kubernetes.md` §4's groups exactly where it names one:
//!
//! | [`UfoRelation`] | [`Relation`] | [`SysmlViewKind`] |
//! |---|---|---|
//! | `HasPart` | `FeatureMembership { owner: source, member: target }` | `Tree` |
//! | `MemberOf` | `FeatureMembership { owner: target, member: source }` | `Tree` |
//! | `HostedBy` / `Binds` / `ResolvesTo` / `RoutesTo` | `Connection { ends: [source, target] }` | `Interconnection` |
//! | `ParticipatesIn` / `Initiates` / `Precedes` / `Causes` / `FlowsTo` | `Succession { source, target }` | `ActionFlow` |
//! | `Transitions` | `Succession { source, target }` | `StateTransition` |
//! | `Satisfies` | `Satisfy { subject: source, requirement: target }` | `General` |
//! | `Verifies` | `Verify { by: source, requirement: target }` | `General` |
//! | `Controls` / `Observes` / `GovernedBy` / `AuthorizedBy` | `Allocation { source, target }` | `General` |
//! | `TracesTo` | `Domain { source, target, kind: "traces_to" }` | `General` |
//!
//! `§4` groups `satisfies`/`verifies` under a "requirement-trace view" that
//! [`SysmlViewKind`] has no dedicated variant for (its ten variants are
//! `Tree`/`General`/`Interconnection`/`ActionFlow`/`StateTransition`/
//! `Sequence`/`Case`/`Geometry`/`Grid`/`Browser`); `General` — "the default
//! SysML v2 graphical view" — is the closest fit until/unless a
//! requirement-trace-specific kind is added upstream.
//!
//! Two direct 1:1 matches this module adds beyond `§4`'s explicit table,
//! because [`Relation`]'s own doc comments describe the identical concept
//! under the same name:
//!
//! | [`UfoRelation`] | [`Relation`] |
//! |---|---|
//! | `Specializes` ("is-a, extends, subtype") | `Specialization { specific: source, general: target }` |
//! | `Requires` ("consumes, depends-on, needs") | `Dependency { client: source, supplier: target }` |
//!
//! Every other [`UfoRelation`] variant (`Instantiates`, `ScopedBy`,
//! `Selects`, `Provides`, and any future `#[non_exhaustive]` addition) has
//! no KerML relationship this module is confident is the right fit, so it
//! falls through to [`Relation::Domain`] with `kind` set to the relation's
//! own [`UfoRelation::canonical_name`] — per
//! `DESIGN-NOTE-typed-model-layer.md` §2.1, nothing is silently dropped.
//! Unlike [`crate::ufo_graph`]'s box-1→box-2 direction (which skips a raw
//! element rather than guess at an ambiguous *reverse* mapping), this
//! direction's input relation is already fully known — there is no
//! ambiguity to duck, only an unmodeled one to fall through honestly.

use ufo_types::ontology::{OntologicalEdge, UfoRelation};
use ufo_types::sysml_model::{ElementId, Relation};
use ufo_types::view::SysmlViewKind;

/// One [`OntologicalEdge`] lifted into a KerML/SysML-v2 [`Relation`], paired
/// with the [`SysmlViewKind`] a renderer should place it in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiftedRelation {
    pub relation: Relation,
    pub view_kind: SysmlViewKind,
    /// The originating [`OntologicalEdge::id`], preserved for traceability
    /// from a rendered view back to the UFO graph it was lifted from.
    pub source_edge_id: String,
}

/// Lift a single edge. Total over every current and future (`#[non_exhaustive]`)
/// [`UfoRelation`] variant — see the module docs for the mapping.
pub fn lift_edge(edge: &OntologicalEdge) -> LiftedRelation {
    let (relation, view_kind) =
        lift_relation(edge.relation, edge.source.clone(), edge.target.clone());
    LiftedRelation {
        relation,
        view_kind,
        source_edge_id: edge.id.clone(),
    }
}

/// Lift every edge in a slice, preserving order.
pub fn lift_edges(edges: &[OntologicalEdge]) -> Vec<LiftedRelation> {
    edges.iter().map(lift_edge).collect()
}

fn lift_relation(
    relation: UfoRelation,
    source: ElementId,
    target: ElementId,
) -> (Relation, SysmlViewKind) {
    use UfoRelation::*;
    match relation {
        HasPart => (
            Relation::FeatureMembership {
                owner: source,
                member: target,
            },
            SysmlViewKind::Tree,
        ),
        MemberOf => (
            Relation::FeatureMembership {
                owner: target,
                member: source,
            },
            SysmlViewKind::Tree,
        ),
        Specializes => (
            Relation::Specialization {
                specific: source,
                general: target,
            },
            SysmlViewKind::Tree,
        ),

        HostedBy | Binds | ResolvesTo | RoutesTo => (
            Relation::Connection {
                ends: vec![source, target],
            },
            SysmlViewKind::Interconnection,
        ),
        Requires => (
            Relation::Dependency {
                client: source,
                supplier: target,
            },
            SysmlViewKind::Interconnection,
        ),

        ParticipatesIn | Initiates | Precedes | Causes | FlowsTo => (
            Relation::Succession { source, target },
            SysmlViewKind::ActionFlow,
        ),
        Transitions => (
            Relation::Succession { source, target },
            SysmlViewKind::StateTransition,
        ),

        Satisfies => (
            Relation::Satisfy {
                subject: source,
                requirement: target,
            },
            SysmlViewKind::General,
        ),
        Verifies => (
            Relation::Verify {
                by: source,
                requirement: target,
            },
            SysmlViewKind::General,
        ),

        Controls | Observes | GovernedBy | AuthorizedBy => (
            Relation::Allocation { source, target },
            SysmlViewKind::General,
        ),

        TracesTo => (domain(relation, source, target), SysmlViewKind::General),

        // Instantiates, ScopedBy, Selects, Provides, and any future
        // #[non_exhaustive] variant: no confident KerML fit (module docs).
        _ => (domain(relation, source, target), SysmlViewKind::General),
    }
}

fn domain(relation: UfoRelation, source: ElementId, target: ElementId) -> Relation {
    Relation::Domain {
        source,
        target,
        kind: relation.canonical_name().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(relation: UfoRelation) -> OntologicalEdge {
        OntologicalEdge::new("e1", ElementId::new("src"), ElementId::new("dst"), relation)
    }

    #[test]
    fn every_ufo_relation_lifts_without_panicking_and_preserves_edge_id() {
        for &relation in UfoRelation::ALL {
            let lifted = lift_edge(&edge(relation));
            assert_eq!(lifted.source_edge_id, "e1");
        }
    }

    #[test]
    fn has_part_lifts_to_feature_membership_owner_first() {
        let lifted = lift_edge(&edge(UfoRelation::HasPart));
        assert_eq!(
            lifted.relation,
            Relation::FeatureMembership {
                owner: ElementId::new("src"),
                member: ElementId::new("dst"),
            }
        );
        assert_eq!(lifted.view_kind, SysmlViewKind::Tree);
    }

    #[test]
    fn member_of_lifts_to_feature_membership_with_swapped_endpoints() {
        let lifted = lift_edge(&edge(UfoRelation::MemberOf));
        assert_eq!(
            lifted.relation,
            Relation::FeatureMembership {
                owner: ElementId::new("dst"),
                member: ElementId::new("src"),
            }
        );
    }

    #[test]
    fn specializes_lifts_to_specialization() {
        let lifted = lift_edge(&edge(UfoRelation::Specializes));
        assert_eq!(
            lifted.relation,
            Relation::Specialization {
                specific: ElementId::new("src"),
                general: ElementId::new("dst"),
            }
        );
    }

    #[test]
    fn requires_lifts_to_dependency() {
        let lifted = lift_edge(&edge(UfoRelation::Requires));
        assert_eq!(
            lifted.relation,
            Relation::Dependency {
                client: ElementId::new("src"),
                supplier: ElementId::new("dst"),
            }
        );
    }

    #[test]
    fn structural_group_lifts_to_connection_in_interconnection_view() {
        for relation in [
            UfoRelation::HostedBy,
            UfoRelation::Binds,
            UfoRelation::ResolvesTo,
            UfoRelation::RoutesTo,
        ] {
            let lifted = lift_edge(&edge(relation));
            assert_eq!(
                lifted.relation,
                Relation::Connection {
                    ends: vec![ElementId::new("src"), ElementId::new("dst")],
                },
                "{relation:?}"
            );
            assert_eq!(lifted.view_kind, SysmlViewKind::Interconnection);
        }
    }

    #[test]
    fn occurrence_group_lifts_to_succession_in_action_flow_view() {
        for relation in [
            UfoRelation::ParticipatesIn,
            UfoRelation::Initiates,
            UfoRelation::Precedes,
            UfoRelation::Causes,
            UfoRelation::FlowsTo,
        ] {
            let lifted = lift_edge(&edge(relation));
            assert_eq!(
                lifted.relation,
                Relation::Succession {
                    source: ElementId::new("src"),
                    target: ElementId::new("dst"),
                },
                "{relation:?}"
            );
            assert_eq!(lifted.view_kind, SysmlViewKind::ActionFlow);
        }
    }

    #[test]
    fn transitions_lifts_to_succession_in_state_transition_view() {
        let lifted = lift_edge(&edge(UfoRelation::Transitions));
        assert_eq!(
            lifted.relation,
            Relation::Succession {
                source: ElementId::new("src"),
                target: ElementId::new("dst"),
            }
        );
        assert_eq!(lifted.view_kind, SysmlViewKind::StateTransition);
    }

    #[test]
    fn satisfies_lifts_to_satisfy_with_subject_and_requirement() {
        let lifted = lift_edge(&edge(UfoRelation::Satisfies));
        assert_eq!(
            lifted.relation,
            Relation::Satisfy {
                subject: ElementId::new("src"),
                requirement: ElementId::new("dst"),
            }
        );
    }

    #[test]
    fn verifies_lifts_to_verify_with_by_and_requirement() {
        let lifted = lift_edge(&edge(UfoRelation::Verifies));
        assert_eq!(
            lifted.relation,
            Relation::Verify {
                by: ElementId::new("src"),
                requirement: ElementId::new("dst"),
            }
        );
    }

    #[test]
    fn governance_group_lifts_to_allocation() {
        for relation in [
            UfoRelation::Controls,
            UfoRelation::Observes,
            UfoRelation::GovernedBy,
            UfoRelation::AuthorizedBy,
        ] {
            let lifted = lift_edge(&edge(relation));
            assert_eq!(
                lifted.relation,
                Relation::Allocation {
                    source: ElementId::new("src"),
                    target: ElementId::new("dst"),
                },
                "{relation:?}"
            );
        }
    }

    #[test]
    fn traces_to_lifts_to_domain_with_its_own_canonical_name() {
        let lifted = lift_edge(&edge(UfoRelation::TracesTo));
        assert_eq!(
            lifted.relation,
            Relation::Domain {
                source: ElementId::new("src"),
                target: ElementId::new("dst"),
                kind: "traces_to".to_string(),
            }
        );
    }

    #[test]
    fn unmodeled_relations_fall_through_to_domain_and_preserve_the_canonical_name() {
        for relation in [
            UfoRelation::Instantiates,
            UfoRelation::ScopedBy,
            UfoRelation::Selects,
            UfoRelation::Provides,
        ] {
            let lifted = lift_edge(&edge(relation));
            assert_eq!(
                lifted.relation,
                Relation::Domain {
                    source: ElementId::new("src"),
                    target: ElementId::new("dst"),
                    kind: relation.canonical_name().to_string(),
                },
                "{relation:?}"
            );
        }
    }

    #[test]
    fn lift_edges_preserves_order() {
        let edges = vec![
            edge(UfoRelation::HasPart),
            edge(UfoRelation::Satisfies),
            edge(UfoRelation::TracesTo),
        ];
        let lifted = lift_edges(&edges);
        assert_eq!(lifted.len(), 3);
        assert!(matches!(
            lifted[0].relation,
            Relation::FeatureMembership { .. }
        ));
        assert!(matches!(lifted[1].relation, Relation::Satisfy { .. }));
        assert!(matches!(lifted[2].relation, Relation::Domain { .. }));
    }
}
