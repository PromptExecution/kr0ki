//! `RequirementGraph` -> ReqIF XML export. The missing direction:
//! `ufo_types::reqif` only lowers ReqIF -> `RequirementGraph`; this module
//! builds a `reqrs::model::ReqIfBundle` from a `RequirementGraph` and
//! serializes it back to XML. See
//! docs/superpowers/specs/2026-09-23-flexo-baseline-adapter-design.md.
//!
//! **Conformance scope.** This module's output is round-trip-grade for
//! kr0ki/reqrs, not full OMG ReqIF-1.2-schema-valid: it is proven to
//! round-trip through kr0ki's own import/export (`reqrs` parse +
//! `ufo_types::reqif` lowering -- see `tests/reqif_export.rs`), but the
//! emitted XML omits fields the OMG ReqIF 1.2 XSD requires on every
//! identifiable element (`LAST-CHANGE`), a `MAX-LENGTH` on the string
//! datatype, several `REQ-IF-HEADER` fields, and any `<SPECIFICATIONS>`
//! grouping. None of these are fabricated here (no `Utc::now()`, no
//! placeholder tool-id strings) -- doing so would make
//! `BaselineIdentity::exported_baseline_sha256` non-deterministic or
//! misleading. The output is therefore not guaranteed to be accepted by
//! third-party ReqIF tools (ReqIF Studio, DOORS, Polarion, etc.) without
//! further work.
//!
//! **Evidence endpoints are dropped, not dangling.** `RequirementGraph`
//! permits a `RequirementRelation` to point at an `EvidenceRef` id (e.g. a
//! `Verifies` edge from evidence to a requirement), but this module only
//! ever emits `SpecObject`s for `graph.requirements`, never for
//! `graph.evidence`. `export_bundle` therefore excludes any relation
//! touching a non-requirement endpoint from the exported bundle's
//! `spec_relations` -- matching the existing "no attachment
//! round-tripping" precedent (design doc §4). Exporting a
//! `SPEC-OBJECT-REF` to an object that was never written would produce
//! XML that `ufo_types::reqif::bundle_to_requirement_graph` rejects on
//! re-import ("relation has an unknown endpoint").
//!
//! **What is not preserved through export.** Only ids/titles/texts/
//! asserted relations round-trip (design doc §1/§4). Specifically dropped,
//! deliberately:
//! - `Requirement.attributes` -- all vendor/original attributes beyond
//!   title/text. Re-import always derives `subtypes = "requirement"` since
//!   only one `SpecObjectType` is synthesized here.
//! - `RequirementRelation.promotion` -- a promoted relation's audit trail.
//!   It re-imports looking like it was always `Asserted`.
//! - `BaselineIdentity.revision` and other provenance fields -- not part
//!   of ReqIF's own schema; there is no field to carry them in.

use crate::requirements::{Requirement, RequirementGraph, RequirementRelationKind};
use reqrs::model::{
    AttributeDefCommon, AttributeDefinition, AttributeDefinitionString, AttributeValue,
    AttributeValueString, CoreContent, DataType, DataTypeCommon, DataTypeString,
    DefaultValuePresence, ListForms, NamespaceInfo, ObjectLookup, ReqIfBundle, ReqIfContent,
    ReqIfHeader, SpecObject, SpecObjectType, SpecRelationType, SpecType, SpecTypeCommon,
};
use reqrs::{AttributeDefId, DataTypeId, SpecObjectId, SpecRelationId, SpecTypeId};

#[derive(Debug, thiserror::Error)]
pub enum ReqIfExportError {
    #[error(transparent)]
    Unparse(#[from] reqrs::ReqIfError),
}

pub fn export_bundle(graph: &RequirementGraph) -> Result<ReqIfBundle, ReqIfExportError> {
    let layer = type_layer();

    let spec_objects: Vec<SpecObject> = graph
        .requirements
        .iter()
        .map(|r| requirement_to_spec_object(r, &layer.text_attr_def_id))
        .collect();

    // Only `graph.requirements` become `SpecObject`s (never `graph.evidence`
    // -- see the module doc comment's "Evidence endpoints are dropped, not
    // dangling" section). A `RequirementRelation` may legally point at an
    // evidence id per `RequirementGraph::validate()`; exporting such a
    // relation would emit a `SPEC-OBJECT-REF` to an object this bundle never
    // writes, which `ufo_types::reqif::bundle_to_requirement_graph` rejects
    // on re-import. Drop any relation whose source or target isn't a known
    // requirement id.
    let requirement_ids: std::collections::BTreeSet<&str> =
        graph.requirements.iter().map(|r| r.id.as_str()).collect();
    let spec_relations: Vec<_> = asserted_relations_to_spec_relations(&graph.relations)
        .into_iter()
        .filter(|rel| {
            requirement_ids.contains(rel.source.as_str())
                && requirement_ids.contains(rel.target.as_str())
        })
        .collect();

    let mut spec_types = vec![layer.spec_object_type];
    spec_types.extend(layer.relation_types);

    let content = ReqIfContent {
        data_types: Some(vec![layer.data_type]),
        spec_types: Some(spec_types),
        spec_objects: Some(spec_objects),
        spec_relations: Some(spec_relations),
        specifications: None,
        relation_groups: None,
        list_forms: ListForms::default(),
        data_types_trailing_comments: Vec::new(),
        spec_types_trailing_comments: Vec::new(),
        spec_objects_trailing_comments: Vec::new(),
        spec_relations_trailing_comments: Vec::new(),
        specifications_trailing_comments: Vec::new(),
        relation_groups_trailing_comments: Vec::new(),
    };
    let lookup = ObjectLookup::build(&content);

    let header = ReqIfHeader {
        identifier: graph.baseline.id.clone(),
        comment: None,
        creation_time: None,
        repository_id: None,
        req_if_tool_id: None,
        req_if_version: None,
        source_tool_id: None,
        title: None,
    };

    Ok(ReqIfBundle {
        namespace_info: NamespaceInfo {
            doctype_is_present: true,
            encoding: Some("UTF-8".to_string()),
            namespace: Some("http://www.omg.org/spec/ReqIF/20110401/reqif.xsd".to_string()),
            ..Default::default()
        },
        header: Some(header),
        core_content: Some(CoreContent {
            req_if_content: Some(content),
        }),
        tool_extensions: Default::default(),
        lookup,
        exceptions: Vec::new(),
    })
}

pub fn export_bundle_to_xml(graph: &RequirementGraph) -> Result<String, ReqIfExportError> {
    let bundle = export_bundle(graph)?;
    Ok(reqrs::ReqIfUnparser::unparse(
        &bundle,
        reqrs::unparse::FormatMode::Canonical,
    )?)
}

const DATA_TYPE_STRING_ID: &str = "DT-STRING";
const SPEC_OBJECT_TYPE_ID: &str = "ST-REQUIREMENT";
const TEXT_ATTR_DEF_ID: &str = "AD-TEXT";

struct TypeLayer {
    data_type: DataType,
    text_attr_def_id: AttributeDefId,
    spec_object_type: SpecType,
    relation_types: Vec<SpecType>,
}

fn data_type_common() -> DataTypeCommon {
    DataTypeCommon {
        description: None,
        last_change: None,
        long_name: None,
        was_self_closing: true,
        comments_before: Vec::new(),
    }
}

fn attr_def_common() -> AttributeDefCommon {
    AttributeDefCommon {
        description: None,
        last_change: None,
        long_name: Some("Text".to_string()),
        is_editable: None,
        was_self_closing: false,
    }
}

fn spec_type_common(
    identifier: &str,
    long_name: &str,
    spec_attributes: Option<Vec<AttributeDefinition>>,
) -> SpecTypeCommon {
    SpecTypeCommon {
        identifier: SpecTypeId::new(identifier),
        description: None,
        last_change: None,
        long_name: Some(long_name.to_string()),
        was_self_closing: spec_attributes.is_none(),
        spec_attributes,
        comments_before: Vec::new(),
    }
}

/// Every `RequirementRelationKind` variant, purely for `type_layer()`'s
/// iteration -- `ufo_types::mbse::requirements::RequirementRelationKind` is a
/// foreign enum (from the `ufo-types` git dependency), so nothing here can
/// stop it from growing an 11th variant someday. This array is not the
/// source of truth for id/name mapping (see `relation_type_id` and
/// `relation_long_name` below, both exhaustive `match`es with no wildcard
/// arm) -- if a variant is ever added, the compiler fails at both match
/// sites *and* this array becomes stale-but-harmless (`type_layer()` would
/// simply omit the new kind's `SpecType` until this array is updated too).
const ALL_RELATION_KINDS: [RequirementRelationKind; 10] = {
    use RequirementRelationKind::*;
    [
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
    ]
};

/// `ST-*` spec-relation-type id for a `RequirementRelationKind`. Exhaustive
/// `match` with no `_ =>` wildcard arm on purpose: if
/// `RequirementRelationKind` (foreign, from the `ufo-types` git dependency)
/// ever gains an 11th variant, this must fail to compile rather than panic
/// at runtime the way a table-lookup-plus-`.expect()` would.
fn relation_type_id(kind: RequirementRelationKind) -> SpecTypeId {
    use RequirementRelationKind::*;
    SpecTypeId::new(match kind {
        Contains => "ST-CONTAINS",
        Derives => "ST-DERIVES",
        Refines => "ST-REFINES",
        Requires => "ST-REQUIRES",
        Satisfies => "ST-SATISFIES",
        Verifies => "ST-VERIFIES",
        Implements => "ST-IMPLEMENTS",
        Traces => "ST-TRACES",
        AllocatedTo => "ST-ALLOCATED-TO",
        Precedes => "ST-PRECEDES",
    })
}

/// `<SPEC-RELATION-TYPE LONG-NAME>` for a `RequirementRelationKind`. Must
/// exactly match one of `ufo_types::reqif::relation_kind_from_name`'s
/// recognized strings so re-import recovers the same kind. Exhaustive
/// `match`, same rationale as `relation_type_id`.
fn relation_long_name(kind: RequirementRelationKind) -> &'static str {
    use RequirementRelationKind::*;
    match kind {
        Contains => "contains",
        Derives => "derives",
        Refines => "refines",
        Requires => "requires",
        Satisfies => "satisfies",
        Verifies => "verifies",
        Implements => "implements",
        Traces => "traces",
        AllocatedTo => "allocated_to",
        Precedes => "precedes",
    }
}

fn type_layer() -> TypeLayer {
    let data_type = DataType::String(DataTypeString {
        identifier: DataTypeId::new(DATA_TYPE_STRING_ID),
        common: data_type_common(),
        max_length: None,
    });
    let text_attr_def_id = AttributeDefId::new(TEXT_ATTR_DEF_ID);
    let text_attr_def = AttributeDefinition::String(AttributeDefinitionString {
        identifier: text_attr_def_id.clone(),
        common: attr_def_common(),
        type_ref: DataTypeId::new(DATA_TYPE_STRING_ID),
        default_value: DefaultValuePresence::Absent,
    });
    let spec_object_type = SpecType::SpecObject(SpecObjectType {
        common: spec_type_common(
            SPEC_OBJECT_TYPE_ID,
            "Requirement",
            Some(vec![text_attr_def]),
        ),
    });
    let relation_types = ALL_RELATION_KINDS
        .iter()
        .map(|kind| {
            SpecType::SpecRelation(SpecRelationType {
                common: spec_type_common(
                    relation_type_id(*kind).as_str(),
                    relation_long_name(*kind),
                    None,
                ),
            })
        })
        .collect();
    TypeLayer {
        data_type,
        text_attr_def_id,
        spec_object_type,
        relation_types,
    }
}

fn requirement_to_spec_object(req: &Requirement, text_attr_def_id: &AttributeDefId) -> SpecObject {
    SpecObject {
        identifier: SpecObjectId::new(req.id.as_str()),
        description: None,
        last_change: None,
        long_name: Some(req.title.clone()),
        spec_object_type: SpecTypeId::new(SPEC_OBJECT_TYPE_ID),
        attributes: vec![AttributeValue::String(AttributeValueString {
            definition_ref: text_attr_def_id.clone(),
            value: req.text.clone(),
            comments_before: Vec::new(),
        })],
        children_order: Vec::new(),
        comments_before: Vec::new(),
        values_trailing_comments: Vec::new(),
    }
}

fn asserted_relations_to_spec_relations(
    relations: &[crate::requirements::RequirementRelation],
) -> Vec<reqrs::model::SpecRelation> {
    use crate::requirements::RelationAuthority;
    use reqrs::model::SpecRelation;

    relations
        .iter()
        .filter(|r| matches!(r.authority, RelationAuthority::Asserted))
        .map(|r| SpecRelation {
            identifier: SpecRelationId::new(r.id.as_str()),
            description: None,
            last_change: None,
            long_name: None,
            relation_type: relation_type_id(r.kind),
            source: SpecObjectId::new(r.source.as_str()),
            target: SpecObjectId::new(r.target.as_str()),
            values: None,
            children_order: Vec::new(),
            comments_before: Vec::new(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::requirements::{
        BaselineIdentity, ModelIdentity, NonAuthoritativeRelation, Provenance, RelationAuthority,
        Requirement, RequirementGraph, RequirementRelation, RequirementRelationKind,
    };
    use reqrs::SpecTypeId;
    use std::collections::BTreeMap;

    fn baseline() -> BaselineIdentity {
        BaselineIdentity {
            id: "BL-1".to_string(),
            revision: "r1".to_string(),
            import_artifact_sha256: None,
            exported_baseline_sha256: None,
        }
    }

    fn provenance() -> Provenance {
        Provenance {
            source_uri: "test:fixture".to_string(),
            artifact_sha256: None,
            locator: None,
        }
    }

    fn requirement(id: &str, title: &str, text: &str) -> Requirement {
        Requirement {
            id: id.to_string(),
            title: title.to_string(),
            text: text.to_string(),
            baseline: baseline(),
            provenance: provenance(),
            attributes: BTreeMap::new(),
            evidence: Vec::new(),
        }
    }

    fn relation(
        id: &str,
        source: &str,
        target: &str,
        kind: RequirementRelationKind,
        authority: RelationAuthority,
    ) -> RequirementRelation {
        RequirementRelation {
            id: id.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            kind,
            authority,
            provenance: provenance(),
            promotion: None,
        }
    }

    #[test]
    fn requirement_maps_title_to_long_name_and_text_to_the_text_attribute() {
        let layer = super::type_layer();
        let req = requirement(
            "REQ-1",
            "Encrypt at rest",
            "System shall encrypt data at rest.",
        );
        let spec_object = super::requirement_to_spec_object(&req, &layer.text_attr_def_id);

        assert_eq!(spec_object.identifier.as_str(), "REQ-1");
        assert_eq!(spec_object.long_name.as_deref(), Some("Encrypt at rest"));
        assert_eq!(
            spec_object.spec_object_type,
            SpecTypeId::new(super::SPEC_OBJECT_TYPE_ID)
        );
        assert_eq!(spec_object.attributes.len(), 1);
        match &spec_object.attributes[0] {
            reqrs::model::AttributeValue::String(v) => {
                assert_eq!(v.definition_ref, layer.text_attr_def_id);
                assert_eq!(v.value, "System shall encrypt data at rest.");
            }
            other => panic!("expected AttributeValue::String, got {other:?}"),
        }
    }

    #[test]
    fn type_layer_has_one_spec_object_type_and_ten_relation_types() {
        let layer = super::type_layer();
        assert_eq!(layer.relation_types.len(), 10);
        match &layer.spec_object_type {
            reqrs::model::SpecType::SpecObject(t) => {
                let attrs = t.common.spec_attributes.as_ref().expect("spec_attributes");
                assert_eq!(attrs.len(), 1);
            }
            other => panic!("expected SpecType::SpecObject, got {other:?}"),
        }
    }

    #[test]
    fn relation_type_id_covers_every_kind_with_the_recognized_name() {
        use RequirementRelationKind::*;
        for kind in [
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
        ] {
            // Every kind must resolve to a SpecTypeId (no panic) -- and the
            // id must be present among type_layer()'s relation_types.
            let id = super::relation_type_id(kind);
            let layer = super::type_layer();
            let found = layer.relation_types.iter().any(|st| match st {
                reqrs::model::SpecType::SpecRelation(t) => t.common.identifier == id,
                _ => false,
            });
            assert!(
                found,
                "relation_type_id({kind:?}) = {id:?} not in type_layer()"
            );
        }
    }

    #[test]
    fn only_asserted_relations_are_exported() {
        let relations = vec![
            relation(
                "R1",
                "A",
                "B",
                RequirementRelationKind::Derives,
                RelationAuthority::Asserted,
            ),
            relation(
                "R2",
                "C",
                "D",
                RequirementRelationKind::Traces,
                RelationAuthority::Inferred(NonAuthoritativeRelation {
                    confidence: 0.8,
                    rationale: "test".into(),
                    evidence: vec![],
                    model: ModelIdentity {
                        name: "test_model".into(),
                        version: "1.0".into(),
                    },
                }),
            ),
        ];
        let spec_relations = super::asserted_relations_to_spec_relations(&relations);
        assert_eq!(spec_relations.len(), 1);
        assert_eq!(spec_relations[0].identifier.as_str(), "R1");
        assert_eq!(spec_relations[0].source.as_str(), "A");
        assert_eq!(spec_relations[0].target.as_str(), "B");
        assert_eq!(
            spec_relations[0].relation_type,
            super::relation_type_id(RequirementRelationKind::Derives)
        );
    }

    #[test]
    fn export_bundle_to_xml_produces_parseable_output_containing_the_requirement() {
        let graph = RequirementGraph {
            baseline: baseline(),
            requirements: vec![requirement("REQ-1", "Encrypt at rest", "Body text.")],
            evidence: Vec::new(),
            relations: Vec::new(),
        };
        let xml = super::export_bundle_to_xml(&graph).expect("export should succeed");
        assert!(xml.contains("REQ-1"));
        assert!(xml.contains("Encrypt at rest"));
        assert!(xml.contains("Body text."));

        // Must be valid enough for reqrs itself to parse back.
        let reparsed = reqrs::ReqIfParser::parse_str(&xml).expect("exported XML must re-parse");
        let content = reparsed
            .core_content
            .as_ref()
            .and_then(|c| c.req_if_content.as_ref())
            .expect("re-parsed bundle must have content");
        assert_eq!(content.spec_objects.as_ref().map(|v| v.len()), Some(1));
    }

    #[test]
    fn export_bundle_to_xml_emits_xml_declaration_and_reqif_namespace() {
        let graph = RequirementGraph {
            baseline: baseline(),
            requirements: vec![requirement("REQ-1", "Encrypt at rest", "Body text.")],
            evidence: Vec::new(),
            relations: Vec::new(),
        };
        let xml = super::export_bundle_to_xml(&graph).expect("export should succeed");
        assert!(
            xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"),
            "expected an XML declaration, got: {xml}"
        );
        assert!(
            xml.contains("xmlns=\"http://www.omg.org/spec/ReqIF/20110401/reqif.xsd\""),
            "expected the ReqIF namespace attribute, got: {xml}"
        );
    }

    #[test]
    fn a_relation_touching_a_non_requirement_endpoint_is_excluded_from_the_bundle() {
        // REQ-2's id is never present in `graph.requirements` (it stands in
        // for an evidence id, or any other non-requirement endpoint) --
        // `export_bundle` must drop R1 rather than emit a dangling
        // `SPEC-OBJECT-REF`.
        let graph = RequirementGraph {
            baseline: baseline(),
            requirements: vec![requirement("REQ-1", "Encrypt at rest", "Body text.")],
            evidence: Vec::new(),
            relations: vec![relation(
                "R1",
                "REQ-1",
                "REQ-2",
                RequirementRelationKind::Verifies,
                RelationAuthority::Asserted,
            )],
        };
        let bundle = super::export_bundle(&graph).expect("export should succeed");
        let spec_relations = bundle
            .core_content
            .as_ref()
            .and_then(|c| c.req_if_content.as_ref())
            .and_then(|c| c.spec_relations.as_ref())
            .expect("content must have a spec_relations field");
        assert!(
            spec_relations.is_empty(),
            "expected the evidence-touching relation to be excluded, got: {spec_relations:?}"
        );
    }
}
