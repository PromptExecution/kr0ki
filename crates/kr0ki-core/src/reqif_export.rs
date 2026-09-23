//! `RequirementGraph` -> ReqIF XML export. The missing direction:
//! `ufo_types::reqif` only lowers ReqIF -> `RequirementGraph`; this module
//! builds a `reqrs::model::ReqIfBundle` from a `RequirementGraph` and
//! serializes it back to XML. See
//! docs/superpowers/specs/2026-09-23-flexo-baseline-adapter-design.md.

use crate::requirements::RequirementGraph;
use crate::requirements::RequirementRelationKind;
use reqrs::model::{
    AttributeDefCommon, AttributeDefinition, AttributeDefinitionString, DataType, DataTypeCommon,
    DataTypeString, DefaultValuePresence, SpecRelationType, SpecType, SpecTypeCommon,
};
use reqrs::{AttributeDefId, DataTypeId, SpecTypeId};

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
    let spec_object_type = SpecType::SpecObject(reqrs::model::SpecObjectType {
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

#[cfg(test)]
mod tests {
    use crate::requirements::RequirementRelationKind;

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
