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
//! **What is preserved through export (KR-A01).** Identifiers, titles, texts, *typed
//! attributes* and asserted relations. Every `Requirement.attributes` entry becomes a ReqIF
//! attribute on the single `SpecObjectType`, named by its key. An attribute whose key has a
//! closed profile vocabulary (`ufo_types::mbse::assurance::attribute_vocabulary`: `node_kind`,
//! `source_kind`, `status`) is declared as an ENUMERATION when *every* value in the graph is in
//! that vocabulary, and as a STRING otherwise -- so an off-vocabulary value is carried, never
//! dropped or coerced. `tests/assurance_a01.rs` and `reqif_roundtrip` compare original and
//! reimported content semantically.
//!
//! **What is not preserved, deliberately and reported by `reqif_roundtrip`:**
//! - attribute keys ReqIF import would rewrite (`normalize_key`: upper case, `-`, space) or that
//!   collide with the reserved `text` attribute -- they are not exported;
//! - `RequirementRelation.promotion` -- a promoted relation's audit trail. It re-imports looking
//!   like it was always `Asserted`;
//! - non-asserted (inferred/proposed) relations, and any relation touching a non-requirement
//!   endpoint (evidence);
//! - `graph.evidence` and `Requirement.evidence` -- evidence is revision-bound and lives in the
//!   evidence store, not in the exchange file;
//! - per-node provenance and `BaselineIdentity.revision` -- not part of ReqIF's own schema;
//!   re-import regenerates provenance from its config.

use std::collections::{BTreeMap, BTreeSet};

use crate::requirements::{Requirement, RequirementGraph, RequirementRelationKind};
use reqrs::ids::EnumValueId;
use reqrs::model::{
    AttributeDefCommon, AttributeDefinition, AttributeDefinitionEnumeration,
    AttributeDefinitionString, AttributeValue, AttributeValueEnumeration, AttributeValueString,
    CoreContent, DataType, DataTypeCommon, DataTypeEnumeration, DataTypeString,
    DefaultValuePresence, EnumValue, ListForms, NamespaceInfo, ObjectLookup, ReqIfBundle,
    ReqIfContent, ReqIfHeader, SpecObject, SpecObjectType, SpecRelationType, SpecType,
    SpecTypeCommon,
};
use reqrs::{AttributeDefId, DataTypeId, SpecObjectId, SpecRelationId, SpecTypeId};
use ufo_types::mbse::assurance::attribute_vocabulary;

#[derive(Debug, thiserror::Error)]
pub enum ReqIfExportError {
    #[error(transparent)]
    Unparse(#[from] reqrs::ReqIfError),
}

pub fn export_bundle(graph: &RequirementGraph) -> Result<ReqIfBundle, ReqIfExportError> {
    let layer = type_layer(graph);

    let spec_objects: Vec<SpecObject> = graph
        .requirements
        .iter()
        .map(|r| requirement_to_spec_object(r, &layer))
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
        data_types: Some(layer.data_types),
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
/// Reserved: the statement travels in this attribute (`reqif::spec_object_to_requirement`
/// reads `text`), so a requirement attribute of the same name cannot also be exported.
const TEXT_KEY: &str = "text";

/// How one attribute key is declared in the type layer.
enum AttrBinding {
    Text(AttributeDefId),
    Enumeration {
        def_id: AttributeDefId,
        values: BTreeMap<String, EnumValueId>,
    },
}

struct TypeLayer {
    data_types: Vec<DataType>,
    text_attr_def_id: AttributeDefId,
    /// Requirement attribute key -> its declaration.
    attributes: BTreeMap<String, AttrBinding>,
    spec_object_type: SpecType,
    relation_types: Vec<SpecType>,
}

fn data_type_common(was_self_closing: bool) -> DataTypeCommon {
    DataTypeCommon {
        description: None,
        last_change: None,
        long_name: None,
        was_self_closing,
        comments_before: Vec::new(),
    }
}

fn attr_def_common(long_name: &str) -> AttributeDefCommon {
    AttributeDefCommon {
        description: None,
        last_change: None,
        long_name: Some(long_name.to_string()),
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

/// `true` when `key` survives ReqIF import's `normalize_key` unchanged and is not reserved.
/// `pub(crate)` so `reqif_roundtrip` classifies the same keys as unsupported.
pub(crate) fn is_exportable_attribute_key(key: &str) -> bool {
    !key.is_empty() && key != TEXT_KEY && key == key.to_lowercase().replace([' ', '-'], "_")
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

fn type_layer(graph: &RequirementGraph) -> TypeLayer {
    // Which keys, with which values, appear in this graph?
    let mut values_by_key: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for req in &graph.requirements {
        for (key, value) in &req.attributes {
            if is_exportable_attribute_key(key) {
                values_by_key.entry(key).or_default().insert(value);
            }
        }
    }

    let mut data_types = vec![DataType::String(DataTypeString {
        identifier: DataTypeId::new(DATA_TYPE_STRING_ID),
        common: data_type_common(true),
        max_length: None,
    })];
    let mut attribute_defs = vec![AttributeDefinition::String(AttributeDefinitionString {
        identifier: AttributeDefId::new(TEXT_ATTR_DEF_ID),
        common: attr_def_common("Text"),
        type_ref: DataTypeId::new(DATA_TYPE_STRING_ID),
        default_value: DefaultValuePresence::Absent,
    })];
    let mut attributes = BTreeMap::new();

    for (key, seen) in &values_by_key {
        let def_id = AttributeDefId::new(format!("AD-ATTR-{key}"));
        let vocabulary =
            attribute_vocabulary(key).filter(|vocab| seen.iter().all(|v| vocab.contains(v)));
        match vocabulary {
            Some(vocab) => {
                let type_id = format!("DT-ENUM-{key}");
                let values: BTreeMap<String, EnumValueId> = vocab
                    .iter()
                    .map(|v| ((*v).to_string(), EnumValueId::new(format!("EV-{key}-{v}"))))
                    .collect();
                data_types.push(DataType::Enumeration(DataTypeEnumeration {
                    identifier: DataTypeId::new(type_id.as_str()),
                    common: data_type_common(false),
                    specified_values: Some(
                        vocab
                            .iter()
                            .map(|v| EnumValue {
                                identifier: values[*v].clone(),
                                long_name: Some((*v).to_string()),
                                description: None,
                                last_change: None,
                                key: (*v).to_string(),
                                other_content: None,
                            })
                            .collect(),
                    ),
                }));
                attribute_defs.push(AttributeDefinition::Enumeration(
                    AttributeDefinitionEnumeration {
                        identifier: def_id.clone(),
                        common: attr_def_common(key),
                        type_ref: DataTypeId::new(type_id.as_str()),
                        default_value: DefaultValuePresence::Absent,
                        multi_valued: Some(false),
                    },
                ));
                attributes.insert(
                    (*key).to_string(),
                    AttrBinding::Enumeration { def_id, values },
                );
            }
            None => {
                attribute_defs.push(AttributeDefinition::String(AttributeDefinitionString {
                    identifier: def_id.clone(),
                    common: attr_def_common(key),
                    type_ref: DataTypeId::new(DATA_TYPE_STRING_ID),
                    default_value: DefaultValuePresence::Absent,
                }));
                attributes.insert((*key).to_string(), AttrBinding::Text(def_id));
            }
        }
    }

    let spec_object_type = SpecType::SpecObject(SpecObjectType {
        common: spec_type_common(SPEC_OBJECT_TYPE_ID, "Requirement", Some(attribute_defs)),
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
        data_types,
        text_attr_def_id: AttributeDefId::new(TEXT_ATTR_DEF_ID),
        attributes,
        spec_object_type,
        relation_types,
    }
}

fn requirement_to_spec_object(req: &Requirement, layer: &TypeLayer) -> SpecObject {
    let mut attributes = vec![AttributeValue::String(AttributeValueString {
        definition_ref: layer.text_attr_def_id.clone(),
        value: req.text.clone(),
        comments_before: Vec::new(),
    })];
    for (key, value) in &req.attributes {
        match layer.attributes.get(key) {
            Some(AttrBinding::Text(def_id)) => {
                attributes.push(AttributeValue::String(AttributeValueString {
                    definition_ref: def_id.clone(),
                    value: value.clone(),
                    comments_before: Vec::new(),
                }));
            }
            Some(AttrBinding::Enumeration { def_id, values }) => {
                // `type_layer` only declares an enumeration when every value is in the
                // vocabulary, so this lookup cannot miss.
                if let Some(id) = values.get(value) {
                    attributes.push(AttributeValue::Enumeration(AttributeValueEnumeration {
                        definition_ref: def_id.clone(),
                        values: vec![id.clone()],
                        was_definition_first: true,
                        comments_before: Vec::new(),
                    }));
                }
            }
            // Not exportable (renamed by import, or reserved): reported by `reqif_roundtrip`.
            None => {}
        }
    }
    SpecObject {
        identifier: SpecObjectId::new(req.id.as_str()),
        description: None,
        last_change: None,
        long_name: Some(req.title.clone()),
        spec_object_type: SpecTypeId::new(SPEC_OBJECT_TYPE_ID),
        attributes,
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

    fn graph_of(requirements: Vec<Requirement>) -> RequirementGraph {
        RequirementGraph {
            baseline: baseline(),
            requirements,
            evidence: Vec::new(),
            relations: Vec::new(),
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
        let req = requirement(
            "REQ-1",
            "Encrypt at rest",
            "System shall encrypt data at rest.",
        );
        let layer = super::type_layer(&graph_of(vec![req.clone()]));
        let spec_object = super::requirement_to_spec_object(&req, &layer);

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
        let layer = super::type_layer(&graph_of(vec![]));
        assert_eq!(layer.relation_types.len(), 10);
        match &layer.spec_object_type {
            reqrs::model::SpecType::SpecObject(t) => {
                let attrs = t.common.spec_attributes.as_ref().expect("spec_attributes");
                assert_eq!(attrs.len(), 1);
            }
            other => panic!("expected SpecType::SpecObject, got {other:?}"),
        }
    }

    fn with_attrs(id: &str, attrs: &[(&str, &str)]) -> Requirement {
        let mut r = requirement(id, id, "The component shall behave.");
        for (k, v) in attrs {
            r.attributes.insert((*k).to_string(), (*v).to_string());
        }
        r
    }

    fn attr_binding<'a>(layer: &'a super::TypeLayer, key: &str) -> &'a super::AttrBinding {
        layer
            .attributes
            .get(key)
            .unwrap_or_else(|| panic!("no binding for {key}"))
    }

    #[test]
    fn attributes_become_declared_reqif_attributes_not_dropped() {
        let req = with_attrs("R1", &[("owner", "team-a"), ("rationale", "because")]);
        let layer = super::type_layer(&graph_of(vec![req.clone()]));
        let so = super::requirement_to_spec_object(&req, &layer);
        // text + owner + rationale
        assert_eq!(so.attributes.len(), 3);
        assert!(matches!(
            attr_binding(&layer, "owner"),
            super::AttrBinding::Text(_)
        ));
        match &layer.spec_object_type {
            reqrs::model::SpecType::SpecObject(t) => {
                // text + owner + rationale
                assert_eq!(t.common.spec_attributes.as_ref().unwrap().len(), 3);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_profile_vocabulary_attribute_is_declared_as_an_enumeration() {
        let req = with_attrs("R1", &[("status", "gated"), ("source_kind", "guidance")]);
        let layer = super::type_layer(&graph_of(vec![req.clone()]));
        assert!(matches!(
            attr_binding(&layer, "status"),
            super::AttrBinding::Enumeration { .. }
        ));
        assert!(matches!(
            attr_binding(&layer, "source_kind"),
            super::AttrBinding::Enumeration { .. }
        ));
        // string datatype + one enumeration datatype per enumerated key
        assert_eq!(layer.data_types.len(), 3);
        let so = super::requirement_to_spec_object(&req, &layer);
        let enums = so
            .attributes
            .iter()
            .filter(|a| matches!(a, reqrs::model::AttributeValue::Enumeration(_)))
            .count();
        assert_eq!(enums, 2);
    }

    #[test]
    fn one_off_vocabulary_value_makes_the_whole_key_a_string_so_nothing_is_lost() {
        let good = with_attrs("R1", &[("status", "gated")]);
        let odd = with_attrs("R2", &[("status", "on-hold-pending-legal")]);
        let layer = super::type_layer(&graph_of(vec![good.clone(), odd.clone()]));
        assert!(matches!(
            attr_binding(&layer, "status"),
            super::AttrBinding::Text(_)
        ));
        let so = super::requirement_to_spec_object(&odd, &layer);
        let carried = so.attributes.iter().any(|a| match a {
            reqrs::model::AttributeValue::String(v) => v.value == "on-hold-pending-legal",
            _ => false,
        });
        assert!(carried, "the off-vocabulary value must still be exported");
    }

    #[test]
    fn keys_that_import_would_rewrite_or_that_are_reserved_are_not_exported() {
        for key in ["Owner", "has-dash", "has space", "text", ""] {
            assert!(!super::is_exportable_attribute_key(key), "{key:?}");
        }
        for key in ["owner", "node_kind", "verification_id", "a1"] {
            assert!(super::is_exportable_attribute_key(key), "{key:?}");
        }
        let req = with_attrs("R1", &[("Owner", "x"), ("text", "y"), ("owner", "z")]);
        let layer = super::type_layer(&graph_of(vec![req.clone()]));
        let so = super::requirement_to_spec_object(&req, &layer);
        assert_eq!(so.attributes.len(), 2); // text (the statement) + owner
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
            let layer = super::type_layer(&graph_of(vec![]));
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
