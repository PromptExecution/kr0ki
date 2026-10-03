//! KR-A01: export a `RequirementGraph` to ReqIF, reimport it, and compare **semantic
//! content** — not bytes — reporting exactly what did not survive.
//!
//! Three outcomes for anything in the original graph, kept apart on purpose:
//!
//! - **preserved** — present and equal after reimport (counted);
//! - **difference** — *should* have survived and did not: a defect. A lossless round trip has
//!   none ([`RoundTripReport::is_lossless`]);
//! - **unsupported** — content the ReqIF export deliberately does not carry (see
//!   `reqif_export`'s module doc). Listed explicitly with the ids affected, never dropped
//!   silently and never counted as a defect.
//!
//! A fourth list, **normalised**, records content the *importer* adds that is not in the
//! original (`text` as an attribute, a synthesised `subtypes`), so the comparison can ignore
//! it without hiding it.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use ufo_types::mbse::requirements::{RelationAuthority, Requirement, RequirementGraph};
use ufo_types::reqif::{parse_and_lower, ReqIfAdapterConfig, ReqIfAdapterError};

use crate::assurance_baseline::sha256_hex;
use crate::reqif_export::{export_bundle_to_xml, is_exportable_attribute_key, ReqIfExportError};

/// Why a round trip could not be run at all.
#[derive(Debug, thiserror::Error)]
pub enum RoundTripError {
    #[error("ReqIF export: {0}")]
    Export(#[from] ReqIfExportError),
    #[error("ReqIF reimport: {0}")]
    Import(#[from] ReqIfAdapterError),
}

/// Something that should have survived the round trip and did not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "difference", rename_all = "snake_case")]
pub enum Difference {
    MissingRequirement {
        id: String,
    },
    ExtraRequirement {
        id: String,
    },
    TitleChanged {
        id: String,
        before: String,
        after: String,
    },
    TextChanged {
        id: String,
        before: String,
        after: String,
    },
    AttributeLost {
        id: String,
        key: String,
    },
    AttributeAdded {
        id: String,
        key: String,
    },
    AttributeChanged {
        id: String,
        key: String,
        before: String,
        after: String,
    },
    RelationLost {
        id: String,
    },
    RelationAdded {
        id: String,
    },
    RelationChanged {
        id: String,
        field: &'static str,
        before: String,
        after: String,
    },
}

/// Kinds of content the ReqIF export deliberately does not carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnsupportedKind {
    /// `graph.evidence`: revision-bound, lives in the evidence store.
    EvidenceNodes,
    /// `Requirement.evidence` references.
    RequirementEvidenceRefs,
    /// Inferred or proposed relations (review-only, not authoritative).
    NonAssertedRelations,
    /// Asserted relations whose endpoint is not a requirement (e.g. evidence).
    RelationsToNonRequirements,
    /// The audit trail of a promoted relation.
    RelationPromotion,
    /// Per-node provenance; re-import regenerates it from its config.
    NodeProvenance,
    /// An attribute key ReqIF import would rewrite (upper case, `-`, space) or that is reserved.
    AttributeKeyNotRepresentable,
}

/// One kind of unsupported content and the ids it touches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Unsupported {
    pub kind: UnsupportedKind,
    pub note: &'static str,
    /// Requirement, relation or evidence ids; `id:key` for attribute keys.
    pub ids: Vec<String>,
}

/// Content added by the importer that is ignored in the comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Normalisation {
    pub attribute: String,
    pub note: &'static str,
    pub ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct RoundTripReport {
    pub requirements_compared: usize,
    pub attributes_compared: usize,
    pub relations_compared: usize,
    pub differences: Vec<Difference>,
    pub unsupported: Vec<Unsupported>,
    pub normalised: Vec<Normalisation>,
}

impl RoundTripReport {
    /// No defect: everything the export claims to carry came back equal.
    pub fn is_lossless(&self) -> bool {
        self.differences.is_empty()
    }

    pub fn unsupported_of(&self, kind: UnsupportedKind) -> Option<&Unsupported> {
        self.unsupported.iter().find(|u| u.kind == kind)
    }
}

/// The result of one export → reimport.
#[derive(Debug, Clone)]
pub struct RoundTrip {
    pub xml: String,
    pub xml_sha256: String,
    pub reimported: RequirementGraph,
    pub report: RoundTripReport,
}

/// Export `graph` to ReqIF, reimport it, and compare.
pub fn round_trip(graph: &RequirementGraph) -> Result<RoundTrip, RoundTripError> {
    let xml = export_bundle_to_xml(graph)?;
    let xml_sha256 = sha256_hex(xml.as_bytes());
    let config = ReqIfAdapterConfig {
        source_uri: "reqif-roundtrip".to_string(),
        revision: graph.baseline.revision.clone(),
        import_artifact_sha256: Some(xml_sha256.clone()),
    };
    let reimported = parse_and_lower(xml.as_bytes(), &config)?;
    let report = compare(graph, &reimported);
    Ok(RoundTrip {
        xml,
        xml_sha256,
        reimported,
        report,
    })
}

/// Compare an original graph with its reimported form.
pub fn compare(original: &RequirementGraph, reimported: &RequirementGraph) -> RoundTripReport {
    let mut report = RoundTripReport::default();
    let after: BTreeMap<&str, &Requirement> = reimported
        .requirements
        .iter()
        .map(|r| (r.id.as_str(), r))
        .collect();
    let original_ids: BTreeSet<&str> = original
        .requirements
        .iter()
        .map(|r| r.id.as_str())
        .collect();

    let mut unsupported: BTreeMap<UnsupportedKind, Vec<String>> = BTreeMap::new();
    let mut normalised: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();

    for before in &original.requirements {
        let Some(now) = after.get(before.id.as_str()) else {
            report.differences.push(Difference::MissingRequirement {
                id: before.id.clone(),
            });
            continue;
        };
        report.requirements_compared += 1;
        if before.title != now.title {
            report.differences.push(Difference::TitleChanged {
                id: before.id.clone(),
                before: before.title.clone(),
                after: now.title.clone(),
            });
        }
        if before.text != now.text {
            report.differences.push(Difference::TextChanged {
                id: before.id.clone(),
                before: before.text.clone(),
                after: now.text.clone(),
            });
        }

        // What the export is expected to carry, and what was never representable.
        let mut expected: BTreeMap<&str, &str> = BTreeMap::new();
        for (key, value) in &before.attributes {
            if is_exportable_attribute_key(key) {
                expected.insert(key, value);
            } else {
                unsupported
                    .entry(UnsupportedKind::AttributeKeyNotRepresentable)
                    .or_default()
                    .push(format!("{}:{key}", before.id));
            }
        }
        // What came back, less what the importer adds by itself.
        let mut actual: BTreeMap<&str, &str> = BTreeMap::new();
        for (key, value) in &now.attributes {
            if key == "text" && value == &before.text {
                normalised
                    .entry("text")
                    .or_default()
                    .push(before.id.clone());
            } else if key == "subtypes"
                && value == "requirement"
                && !before.attributes.contains_key("subtypes")
            {
                normalised
                    .entry("subtypes")
                    .or_default()
                    .push(before.id.clone());
            } else {
                actual.insert(key, value);
            }
        }
        for (key, want) in &expected {
            report.attributes_compared += 1;
            match actual.get(key) {
                None => report.differences.push(Difference::AttributeLost {
                    id: before.id.clone(),
                    key: (*key).to_string(),
                }),
                Some(got) if got != want => report.differences.push(Difference::AttributeChanged {
                    id: before.id.clone(),
                    key: (*key).to_string(),
                    before: (*want).to_string(),
                    after: (*got).to_string(),
                }),
                Some(_) => {}
            }
        }
        for key in actual.keys().filter(|k| !expected.contains_key(*k)) {
            report.differences.push(Difference::AttributeAdded {
                id: before.id.clone(),
                key: (*key).to_string(),
            });
        }

        if before.provenance != now.provenance {
            unsupported
                .entry(UnsupportedKind::NodeProvenance)
                .or_default()
                .push(before.id.clone());
        }
        if !before.evidence.is_empty() {
            unsupported
                .entry(UnsupportedKind::RequirementEvidenceRefs)
                .or_default()
                .push(before.id.clone());
        }
    }
    for id in after.keys().filter(|id| !original_ids.contains(*id)) {
        report.differences.push(Difference::ExtraRequirement {
            id: (*id).to_string(),
        });
    }

    if !original.evidence.is_empty() {
        unsupported
            .entry(UnsupportedKind::EvidenceNodes)
            .or_default()
            .extend(original.evidence.iter().map(|e| e.id.clone()));
    }

    // Relations: only asserted, requirement-to-requirement edges are carried.
    let reimported_relations: BTreeMap<&str, _> = reimported
        .relations
        .iter()
        .map(|r| (r.id.as_str(), r))
        .collect();
    let mut expected_relation_ids: BTreeSet<&str> = BTreeSet::new();
    for r in &original.relations {
        if !matches!(r.authority, RelationAuthority::Asserted) {
            unsupported
                .entry(UnsupportedKind::NonAssertedRelations)
                .or_default()
                .push(r.id.clone());
            continue;
        }
        if !original_ids.contains(r.source.as_str()) || !original_ids.contains(r.target.as_str()) {
            unsupported
                .entry(UnsupportedKind::RelationsToNonRequirements)
                .or_default()
                .push(r.id.clone());
            continue;
        }
        expected_relation_ids.insert(&r.id);
        report.relations_compared += 1;
        if r.promotion.is_some() {
            unsupported
                .entry(UnsupportedKind::RelationPromotion)
                .or_default()
                .push(r.id.clone());
        }
        match reimported_relations.get(r.id.as_str()) {
            None => report
                .differences
                .push(Difference::RelationLost { id: r.id.clone() }),
            Some(now) => {
                let mut changed = |field: &'static str, before: String, after: String| {
                    if before != after {
                        report.differences.push(Difference::RelationChanged {
                            id: r.id.clone(),
                            field,
                            before,
                            after,
                        });
                    }
                };
                changed("kind", format!("{:?}", r.kind), format!("{:?}", now.kind));
                changed("source", r.source.clone(), now.source.clone());
                changed("target", r.target.clone(), now.target.clone());
            }
        }
    }
    for id in reimported_relations
        .keys()
        .filter(|id| !expected_relation_ids.contains(*id))
    {
        report.differences.push(Difference::RelationAdded {
            id: (*id).to_string(),
        });
    }

    report.unsupported = unsupported
        .into_iter()
        .map(|(kind, mut ids)| {
            ids.sort();
            ids.dedup();
            Unsupported {
                kind,
                note: unsupported_note(kind),
                ids,
            }
        })
        .collect();
    report.normalised = normalised
        .into_iter()
        .map(|(attribute, mut ids)| {
            ids.sort();
            ids.dedup();
            Normalisation {
                attribute: attribute.to_string(),
                note: match attribute {
                    "text" => "the statement travels as the `text` attribute; the importer exposes it again",
                    _ => "the importer derives `subtypes` from the spec-object type name",
                },
                ids,
            }
        })
        .collect();
    report
}

fn unsupported_note(kind: UnsupportedKind) -> &'static str {
    match kind {
        UnsupportedKind::EvidenceNodes => {
            "evidence is revision-bound and lives in the evidence store, not the exchange file"
        }
        UnsupportedKind::RequirementEvidenceRefs => {
            "per-requirement evidence references are not exported"
        }
        UnsupportedKind::NonAssertedRelations => {
            "inferred and proposed relations are review-only and are not exported"
        }
        UnsupportedKind::RelationsToNonRequirements => {
            "a relation with a non-requirement endpoint would dangle in ReqIF and is not exported"
        }
        UnsupportedKind::RelationPromotion => {
            "the promotion audit trail has no ReqIF field; the relation reimports as plain asserted"
        }
        UnsupportedKind::NodeProvenance => {
            "provenance is not part of ReqIF; reimport regenerates it from its config"
        }
        UnsupportedKind::AttributeKeyNotRepresentable => {
            "the key would be rewritten by ReqIF import (upper case, '-', space) or is reserved"
        }
    }
}
