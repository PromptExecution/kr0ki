//! `RequirementGraph` -> ReqIF XML export. The missing direction:
//! `ufo_types::reqif` only lowers ReqIF -> `RequirementGraph`; this module
//! builds a `reqrs::model::ReqIfBundle` from a `RequirementGraph` and
//! serializes it back to XML. See
//! docs/superpowers/specs/2026-09-23-flexo-baseline-adapter-design.md.

use crate::requirements::{Requirement, RequirementGraph, RequirementRelationKind};
use reqrs::model::{
    AttributeDefCommon, AttributeDefinition, AttributeDefinitionString, AttributeValue,
    AttributeValueString, DataType, DataTypeCommon, DataTypeString, DefaultValuePresence,
    SpecObject, SpecObjectType, SpecRelationType, SpecType, SpecTypeCommon,
};
use reqrs::{AttributeDefId, DataTypeId, SpecObjectId, SpecTypeId};

#[derive(Debug, thiserror::Error)]
pub enum ReqIfExportError {
    #[error(transparent)]
    Unparse(#[from] reqrs::ReqIfError),
}

pub fn export_bundle(
    _graph: &RequirementGraph,
) -> Result<reqrs::model::ReqIfBundle, ReqIfExportError> {
    todo!("Task 5")
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

/// `(RequirementRelationKind variant, spec-relation-type id, LONG-NAME)` --
/// LONG-NAME must exactly match one of `ufo_types::reqif::relation_kind_from_name`'s
/// recognized strings so re-import recovers the same kind.
const RELATION_KINDS: &[(RequirementRelationKind, &str, &str)] = {
    use RequirementRelationKind::*;
    &[
        (Contains, "ST-CONTAINS", "contains"),
        (Derives, "ST-DERIVES", "derives"),
        (Refines, "ST-REFINES", "refines"),
        (Requires, "ST-REQUIRES", "requires"),
        (Satisfies, "ST-SATISFIES", "satisfies"),
        (Verifies, "ST-VERIFIES", "verifies"),
        (Implements, "ST-IMPLEMENTS", "implements"),
        (Traces, "ST-TRACES", "traces"),
        (AllocatedTo, "ST-ALLOCATED-TO", "allocated_to"),
        (Precedes, "ST-PRECEDES", "precedes"),
    ]
};

fn relation_type_id(kind: RequirementRelationKind) -> SpecTypeId {
    RELATION_KINDS
        .iter()
        .find(|(k, _, _)| *k == kind)
        .map(|(_, id, _)| SpecTypeId::new(*id))
        .expect("RELATION_KINDS covers every RequirementRelationKind variant")
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
    let relation_types = RELATION_KINDS
        .iter()
        .map(|(_, id, long_name)| {
            SpecType::SpecRelation(SpecRelationType {
                common: spec_type_common(id, long_name, None),
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

#[cfg(test)]
mod tests {
    use crate::requirements::{BaselineIdentity, Provenance, Requirement, RequirementRelationKind};
    use std::collections::BTreeMap;
    use reqrs::SpecTypeId;

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

    #[test]
    fn requirement_maps_title_to_long_name_and_text_to_the_text_attribute() {
        let layer = super::type_layer();
        let req = requirement("REQ-1", "Encrypt at rest", "System shall encrypt data at rest.");
        let spec_object = super::requirement_to_spec_object(&req, &layer.text_attr_def_id);

        assert_eq!(spec_object.identifier.as_str(), "REQ-1");
        assert_eq!(spec_object.long_name.as_deref(), Some("Encrypt at rest"));
        assert_eq!(spec_object.spec_object_type, SpecTypeId::new(super::SPEC_OBJECT_TYPE_ID));
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
}
