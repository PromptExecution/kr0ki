//! ReqIF -> [`crate::requirements::RequirementGraph`] adapter (kr0ki M2,
//! `docs/TODO.md` "ReqIF / ReqIFz adapter").
//!
//! Parsing itself is [`reqrs`]'s job (decided 2026-09-20 — see
//! `docs/HANDOFF-2026-09-19-reqif-flexo.md`'s "Deliberate boundaries" and
//! [kr0ki#39](https://github.com/PromptExecution/kr0ki/issues/39)): a direct
//! Rust dependency, not a sidecar call to the vendored `reqif-opa-mcp`
//! submodule. This module's job is narrowed to one thing — lower a parsed
//! [`reqrs::model::ReqIfBundle`] onto [`crate::requirements::RequirementGraph`]
//! — mirroring the mapping already written once in Python for
//! `reqif-opa-mcp` (`reqif_mcp/reqif_parser.py` +
//! `reqif_mcp/normalization.py`, `PromptExecution/reqif-opa-mcp#25`), except
//! the target here is the shared semantic contract, not a bespoke OPA-input
//! schema.
//!
//! Attribute values are keyed by their `<ATTRIBUTE-DEFINITION-*>`
//! `LONG-NAME`, lowercased and snake-cased (`attribute_definition_names`),
//! the same convention `normalization.py::_normalize_spec_object` uses to
//! build its `attrs_map`. A requirement's `title`/`text` are then derived
//! from that map (falling back through `name`/`title`/`key` for `title`,
//! `text`/`description` for `text`) rather than from any ReqIF-standard
//! field, because ReqIF itself has no dedicated title/text elements —
//! everything beyond `IDENTIFIER`/`LONG-NAME` lives in vendor-defined
//! attributes.

use std::collections::BTreeMap;

use reqrs::ids::{AttributeDefId, EnumValueId, SpecTypeId};
use reqrs::model::{
    AttributeDefinition, AttributeValue, DataType, ReqIfBundle, SpecObject, SpecRelation, SpecType,
};

use crate::requirements::{
    BaselineIdentity, Provenance, RelationAuthority, Requirement, RequirementError,
    RequirementGraph, RequirementRelation, RequirementRelationKind,
};

/// Caller-supplied identity for the baseline a bundle represents. ReqIF has
/// no field that maps onto [`BaselineIdentity::revision`] (an "immutable
/// source revision" is a Flexo/kr0ki concept, not a ReqIF one) — the caller
/// supplies one, e.g. a git commit, an import timestamp, or a content hash
/// of the raw bytes.
#[derive(Debug, Clone)]
pub struct ReqIfAdapterConfig {
    /// Opaque locator for the source artifact (file path, URI, etc.) —
    /// becomes both [`Provenance::source_uri`] and, when the bundle's own
    /// `<REQ-IF-HEADER IDENTIFIER>` is absent, the baseline id.
    pub source_uri: String,
    pub revision: String,
    pub import_artifact_sha256: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ReqIfAdapterError {
    #[error("<REQ-IF-CONTENT> element not found")]
    MissingContent,
    #[error(transparent)]
    Graph(#[from] RequirementError),
}

/// Map a parsed [`ReqIfBundle`] onto a [`RequirementGraph`], validating the
/// result before returning it (see [`RequirementGraph::validate`]).
pub fn bundle_to_requirement_graph(
    bundle: &ReqIfBundle,
    config: &ReqIfAdapterConfig,
) -> Result<RequirementGraph, ReqIfAdapterError> {
    let content = bundle
        .core_content
        .as_ref()
        .and_then(|core| core.req_if_content.as_ref())
        .ok_or(ReqIfAdapterError::MissingContent)?;

    let baseline = BaselineIdentity {
        id: bundle
            .header
            .as_ref()
            .map(|header| header.identifier.clone())
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| config.source_uri.clone()),
        revision: config.revision.clone(),
        import_artifact_sha256: config.import_artifact_sha256.clone(),
        exported_baseline_sha256: None,
    };
    let provenance = Provenance {
        source_uri: config.source_uri.clone(),
        artifact_sha256: config.import_artifact_sha256.clone(),
        locator: None,
    };

    let spec_types = content.spec_types.as_deref().unwrap_or(&[]);
    let attr_def_names = attribute_definition_names(spec_types);
    let spec_type_names = spec_type_names(spec_types);
    let enum_value_names = enum_value_names(content.data_types.as_deref().unwrap_or(&[]));

    let requirements = content
        .spec_objects
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|spec_object| {
            spec_object_to_requirement(
                spec_object,
                &attr_def_names,
                &spec_type_names,
                &enum_value_names,
                &baseline,
                &provenance,
            )
        })
        .collect();

    let relations = content
        .spec_relations
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|spec_relation| {
            spec_relation_to_requirement_relation(spec_relation, &spec_type_names, &provenance)
        })
        .collect();

    let graph = RequirementGraph {
        baseline,
        requirements,
        evidence: Vec::new(),
        relations,
    };
    graph.validate()?;
    Ok(graph)
}

fn attribute_definition_common(def: &AttributeDefinition) -> &reqrs::model::AttributeDefCommon {
    match def {
        AttributeDefinition::String(d) => &d.common,
        AttributeDefinition::Boolean(d) => &d.common,
        AttributeDefinition::Integer(d) => &d.common,
        AttributeDefinition::Real(d) => &d.common,
        AttributeDefinition::Date(d) => &d.common,
        AttributeDefinition::Xhtml(d) => &d.common,
        AttributeDefinition::Enumeration(d) => &d.common,
    }
}

fn spec_type_common(spec_type: &SpecType) -> &reqrs::model::SpecTypeCommon {
    match spec_type {
        SpecType::SpecObject(t) => &t.common,
        SpecType::Specification(t) => &t.common,
        SpecType::SpecRelation(t) => &t.common,
        SpecType::RelationGroup(t) => &t.common,
    }
}

/// `<ATTRIBUTE-DEFINITION-*>` id -> normalized `LONG-NAME`, flattened across
/// every `<SPEC-TYPES>` entry. Mirrors `attr_defs_map` in
/// `reqif_mcp/normalization.py`.
fn attribute_definition_names(spec_types: &[SpecType]) -> BTreeMap<AttributeDefId, String> {
    spec_types
        .iter()
        .flat_map(|spec_type| spec_type_common(spec_type).spec_attributes.iter().flatten())
        .filter_map(|def| {
            let long_name = attribute_definition_common(def).long_name.as_deref()?;
            Some((def.identifier().clone(), normalize_key(long_name)))
        })
        .collect()
}

/// `<SPEC-TYPES>` id -> normalized `LONG-NAME`. Used to recover a
/// requirement's subtype and a relation's [`RequirementRelationKind`] when
/// no more specific attribute carries that information.
fn spec_type_names(spec_types: &[SpecType]) -> BTreeMap<SpecTypeId, String> {
    spec_types
        .iter()
        .map(|spec_type| {
            let common = spec_type_common(spec_type);
            let name = common.long_name.as_deref().unwrap_or_default();
            (common.identifier.clone(), normalize_key(name))
        })
        .collect()
}

/// `<ENUM-VALUE>` id -> its human-readable value. `LONG-NAME` is the ReqIF
/// display value; `KEY` is a vendor-stable fallback when no long name exists.
fn enum_value_names(data_types: &[DataType]) -> BTreeMap<EnumValueId, String> {
    data_types
        .iter()
        .filter_map(|data_type| match data_type {
            DataType::Enumeration(data_type) => data_type.specified_values.as_deref(),
            _ => None,
        })
        .flatten()
        .filter_map(|value| {
            value
                .long_name
                .as_deref()
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
                .or_else(|| (!value.key.is_empty()).then(|| value.key.clone()))
                .map(|name| (value.identifier.clone(), name))
        })
        .collect()
}

fn normalize_key(s: &str) -> String {
    s.to_lowercase().replace([' ', '-'], "_")
}

fn attribute_value_string(
    value: &AttributeValue,
    enum_value_names: &BTreeMap<EnumValueId, String>,
) -> String {
    match value {
        AttributeValue::String(v) => v.value.clone(),
        AttributeValue::Boolean(v) => v.value.to_string(),
        AttributeValue::Integer(v) => v.value.clone(),
        AttributeValue::Real(v) => v.value.clone(),
        AttributeValue::Date(v) => v.value.clone(),
        AttributeValue::Xhtml(v) => v.the_value_raw.clone(),
        AttributeValue::Enumeration(v) => v
            .values
            .iter()
            .map(|id| {
                enum_value_names
                    .get(id)
                    .map(String::as_str)
                    .unwrap_or_else(|| id.as_str())
            })
            .collect::<Vec<_>>()
            .join(", "),
    }
}

fn spec_object_to_requirement(
    spec_object: &SpecObject,
    attr_def_names: &BTreeMap<AttributeDefId, String>,
    spec_type_names: &BTreeMap<SpecTypeId, String>,
    enum_value_names: &BTreeMap<EnumValueId, String>,
    baseline: &BaselineIdentity,
    provenance: &Provenance,
) -> Requirement {
    let mut attributes = BTreeMap::new();
    for value in &spec_object.attributes {
        if let Some(key) = attr_def_names.get(value.definition_ref()) {
            attributes.insert(key.clone(), attribute_value_string(value, enum_value_names));
        }
    }

    // ReqIF's required SPEC-OBJECT-TYPE-REF is the canonical subtype when a
    // vendor did not provide a populated `subtypes` or `type` attribute.
    if !["subtypes", "type"]
        .into_iter()
        .any(|key| attributes.get(key).is_some_and(|value| !value.is_empty()))
    {
        if let Some(name) = spec_type_names
            .get(&spec_object.spec_object_type)
            .filter(|name| !name.is_empty())
        {
            attributes.insert("subtypes".to_string(), name.clone());
        }
    }

    let title = spec_object
        .long_name
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| first_non_empty_attribute(&attributes, &["name", "title", "key"]))
        .unwrap_or_else(|| spec_object.identifier.as_str().to_string());

    let text = first_non_empty_attribute(&attributes, &["text", "description"]).unwrap_or_default();

    Requirement {
        id: spec_object.identifier.as_str().to_string(),
        title,
        text,
        baseline: baseline.clone(),
        provenance: provenance.clone(),
        attributes,
        evidence: Vec::new(),
    }
}

fn first_non_empty_attribute(
    attributes: &BTreeMap<String, String>,
    keys: &[&str],
) -> Option<String> {
    keys.iter()
        .filter_map(|key| attributes.get(*key))
        .find(|value| !value.is_empty())
        .cloned()
}

fn spec_relation_to_requirement_relation(
    spec_relation: &SpecRelation,
    spec_type_names: &BTreeMap<SpecTypeId, String>,
    provenance: &Provenance,
) -> RequirementRelation {
    let kind = spec_type_names
        .get(&spec_relation.relation_type)
        .and_then(|name| relation_kind_from_name(name))
        .unwrap_or(RequirementRelationKind::Traces);

    RequirementRelation {
        id: spec_relation.identifier.as_str().to_string(),
        source: spec_relation.source.as_str().to_string(),
        target: spec_relation.target.as_str().to_string(),
        kind,
        authority: RelationAuthority::Asserted,
        provenance: provenance.clone(),
        promotion: None,
    }
}

/// Maps a normalized `<SPEC-RELATION-TYPE LONG-NAME>` onto the shared
/// [`RequirementRelationKind`] vocabulary. Unrecognized names fall back to
/// [`RequirementRelationKind::Traces`] in the caller, the same "don't lose
/// the edge, don't overclaim its meaning" default
/// `normalization.py::_extract_subtypes` uses for an unknown requirement
/// subtype (`GENERAL`).
fn relation_kind_from_name(name: &str) -> Option<RequirementRelationKind> {
    use RequirementRelationKind::*;
    Some(match name {
        "contains" => Contains,
        "derives" | "derive" | "derived_from" => Derives,
        "refines" => Refines,
        "requires" => Requires,
        "satisfies" => Satisfies,
        "verifies" => Verifies,
        "implements" => Implements,
        "traces" => Traces,
        "allocated_to" | "allocatedto" => AllocatedTo,
        "precedes" => Precedes,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqrs::ReqIfParser;

    /// Two spec objects (one carrying `Name`/`Text`/`Key` attributes, one
    /// carrying only a `LONG-NAME`) plus one `derives` relation between
    /// them. Exercises the full parse -> map path, not a hand-built bundle.
    const FIXTURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<REQ-IF xmlns="http://www.omg.org/spec/ReqIF/20110401/reqif.xsd">
  <THE-HEADER>
    <REQ-IF-HEADER IDENTIFIER="BASELINE-TEST">
      <TITLE>Adapter test baseline</TITLE>
    </REQ-IF-HEADER>
  </THE-HEADER>
  <CORE-CONTENT>
    <REQ-IF-CONTENT>
      <DATATYPES>
        <DATATYPE-DEFINITION-STRING IDENTIFIER="DT-STRING" LONG-NAME="String"/>
        <DATATYPE-DEFINITION-ENUMERATION IDENTIFIER="DT-STATUS" LONG-NAME="Status">
          <SPECIFIED-VALUES>
            <ENUM-VALUE IDENTIFIER="EV-ACTIVE" LONG-NAME="Active">
              <PROPERTIES><EMBEDDED-VALUE KEY="active"/></PROPERTIES>
            </ENUM-VALUE>
          </SPECIFIED-VALUES>
        </DATATYPE-DEFINITION-ENUMERATION>
      </DATATYPES>
      <SPEC-TYPES>
        <SPEC-OBJECT-TYPE IDENTIFIER="ST-REQUIREMENT" LONG-NAME="Requirement">
          <SPEC-ATTRIBUTES>
            <ATTRIBUTE-DEFINITION-STRING IDENTIFIER="AD-KEY" LONG-NAME="Key">
              <TYPE><DATATYPE-DEFINITION-STRING-REF>DT-STRING</DATATYPE-DEFINITION-STRING-REF></TYPE>
            </ATTRIBUTE-DEFINITION-STRING>
            <ATTRIBUTE-DEFINITION-STRING IDENTIFIER="AD-TEXT" LONG-NAME="Text">
              <TYPE><DATATYPE-DEFINITION-STRING-REF>DT-STRING</DATATYPE-DEFINITION-STRING-REF></TYPE>
            </ATTRIBUTE-DEFINITION-STRING>
            <ATTRIBUTE-DEFINITION-STRING IDENTIFIER="AD-NAME" LONG-NAME="Name">
              <TYPE><DATATYPE-DEFINITION-STRING-REF>DT-STRING</DATATYPE-DEFINITION-STRING-REF></TYPE>
            </ATTRIBUTE-DEFINITION-STRING>
            <ATTRIBUTE-DEFINITION-STRING IDENTIFIER="AD-TITLE" LONG-NAME="Title">
              <TYPE><DATATYPE-DEFINITION-STRING-REF>DT-STRING</DATATYPE-DEFINITION-STRING-REF></TYPE>
            </ATTRIBUTE-DEFINITION-STRING>
            <ATTRIBUTE-DEFINITION-STRING IDENTIFIER="AD-DESCRIPTION" LONG-NAME="Description">
              <TYPE><DATATYPE-DEFINITION-STRING-REF>DT-STRING</DATATYPE-DEFINITION-STRING-REF></TYPE>
            </ATTRIBUTE-DEFINITION-STRING>
            <ATTRIBUTE-DEFINITION-ENUMERATION IDENTIFIER="AD-STATUS" LONG-NAME="Status">
              <TYPE><DATATYPE-DEFINITION-ENUMERATION-REF>DT-STATUS</DATATYPE-DEFINITION-ENUMERATION-REF></TYPE>
            </ATTRIBUTE-DEFINITION-ENUMERATION>
          </SPEC-ATTRIBUTES>
        </SPEC-OBJECT-TYPE>
        <SPEC-RELATION-TYPE IDENTIFIER="ST-DERIVES" LONG-NAME="Derives"/>
      </SPEC-TYPES>
      <SPEC-OBJECTS>
        <SPEC-OBJECT IDENTIFIER="REQ-1" LONG-NAME="Encryption at rest">
          <TYPE><SPEC-OBJECT-TYPE-REF>ST-REQUIREMENT</SPEC-OBJECT-TYPE-REF></TYPE>
          <VALUES>
            <ATTRIBUTE-VALUE-STRING THE-VALUE="ENC-001">
              <DEFINITION><ATTRIBUTE-DEFINITION-STRING-REF>AD-KEY</ATTRIBUTE-DEFINITION-STRING-REF></DEFINITION>
            </ATTRIBUTE-VALUE-STRING>
            <ATTRIBUTE-VALUE-STRING THE-VALUE="System shall encrypt data at rest.">
              <DEFINITION><ATTRIBUTE-DEFINITION-STRING-REF>AD-TEXT</ATTRIBUTE-DEFINITION-STRING-REF></DEFINITION>
            </ATTRIBUTE-VALUE-STRING>
            <ATTRIBUTE-VALUE-ENUMERATION>
              <VALUES><ENUM-VALUE-REF>EV-ACTIVE</ENUM-VALUE-REF></VALUES>
              <DEFINITION><ATTRIBUTE-DEFINITION-ENUMERATION-REF>AD-STATUS</ATTRIBUTE-DEFINITION-ENUMERATION-REF></DEFINITION>
            </ATTRIBUTE-VALUE-ENUMERATION>
          </VALUES>
        </SPEC-OBJECT>
        <SPEC-OBJECT IDENTIFIER="REQ-2">
          <TYPE><SPEC-OBJECT-TYPE-REF>ST-REQUIREMENT</SPEC-OBJECT-TYPE-REF></TYPE>
          <VALUES>
            <ATTRIBUTE-VALUE-STRING THE-VALUE="">
              <DEFINITION><ATTRIBUTE-DEFINITION-STRING-REF>AD-NAME</ATTRIBUTE-DEFINITION-STRING-REF></DEFINITION>
            </ATTRIBUTE-VALUE-STRING>
            <ATTRIBUTE-VALUE-STRING THE-VALUE="Derived encryption">
              <DEFINITION><ATTRIBUTE-DEFINITION-STRING-REF>AD-TITLE</ATTRIBUTE-DEFINITION-STRING-REF></DEFINITION>
            </ATTRIBUTE-VALUE-STRING>
            <ATTRIBUTE-VALUE-STRING THE-VALUE="ENC-001-DERIVED">
              <DEFINITION><ATTRIBUTE-DEFINITION-STRING-REF>AD-KEY</ATTRIBUTE-DEFINITION-STRING-REF></DEFINITION>
            </ATTRIBUTE-VALUE-STRING>
            <ATTRIBUTE-VALUE-STRING THE-VALUE="">
              <DEFINITION><ATTRIBUTE-DEFINITION-STRING-REF>AD-TEXT</ATTRIBUTE-DEFINITION-STRING-REF></DEFINITION>
            </ATTRIBUTE-VALUE-STRING>
            <ATTRIBUTE-VALUE-STRING THE-VALUE="Derived encryption must remain protected.">
              <DEFINITION><ATTRIBUTE-DEFINITION-STRING-REF>AD-DESCRIPTION</ATTRIBUTE-DEFINITION-STRING-REF></DEFINITION>
            </ATTRIBUTE-VALUE-STRING>
          </VALUES>
        </SPEC-OBJECT>
      </SPEC-OBJECTS>
      <SPEC-RELATIONS>
        <SPEC-RELATION IDENTIFIER="REL-1">
          <TYPE><SPEC-RELATION-TYPE-REF>ST-DERIVES</SPEC-RELATION-TYPE-REF></TYPE>
          <SOURCE><SPEC-OBJECT-REF>REQ-2</SPEC-OBJECT-REF></SOURCE>
          <TARGET><SPEC-OBJECT-REF>REQ-1</SPEC-OBJECT-REF></TARGET>
        </SPEC-RELATION>
      </SPEC-RELATIONS>
    </REQ-IF-CONTENT>
  </CORE-CONTENT>
</REQ-IF>"#;

    fn config() -> ReqIfAdapterConfig {
        ReqIfAdapterConfig {
            source_uri: "test://fixture.reqif".to_string(),
            revision: "rev-1".to_string(),
            import_artifact_sha256: Some("deadbeef".to_string()),
        }
    }

    #[test]
    fn maps_baseline_from_header_identifier() {
        let bundle = ReqIfParser::parse_str(FIXTURE).unwrap();
        let graph = bundle_to_requirement_graph(&bundle, &config()).unwrap();
        assert_eq!(graph.baseline.id, "BASELINE-TEST");
        assert_eq!(graph.baseline.revision, "rev-1");
        assert_eq!(
            graph.baseline.import_artifact_sha256.as_deref(),
            Some("deadbeef")
        );
    }

    #[test]
    fn maps_spec_object_long_name_and_attributes_to_title_and_text() {
        let bundle = ReqIfParser::parse_str(FIXTURE).unwrap();
        let graph = bundle_to_requirement_graph(&bundle, &config()).unwrap();

        let req1 = graph.requirements.iter().find(|r| r.id == "REQ-1").unwrap();
        assert_eq!(req1.title, "Encryption at rest");
        assert_eq!(req1.text, "System shall encrypt data at rest.");
        assert_eq!(
            req1.attributes.get("key").map(String::as_str),
            Some("ENC-001")
        );
        assert_eq!(
            req1.attributes.get("status").map(String::as_str),
            Some("Active")
        );
        assert_eq!(req1.provenance.source_uri, "test://fixture.reqif");
    }

    #[test]
    fn retains_object_type_as_subtype_when_vendor_attributes_are_absent() {
        let bundle = ReqIfParser::parse_str(FIXTURE).unwrap();
        let graph = bundle_to_requirement_graph(&bundle, &config()).unwrap();

        let req2 = graph.requirements.iter().find(|r| r.id == "REQ-2").unwrap();
        assert_eq!(
            req2.attributes.get("subtypes").map(String::as_str),
            Some("requirement")
        );
    }

    #[test]
    fn skips_empty_title_and_text_attributes_when_falling_back() {
        let bundle = ReqIfParser::parse_str(FIXTURE).unwrap();
        let graph = bundle_to_requirement_graph(&bundle, &config()).unwrap();

        let req2 = graph.requirements.iter().find(|r| r.id == "REQ-2").unwrap();
        assert_eq!(req2.title, "Derived encryption");
        assert_eq!(req2.text, "Derived encryption must remain protected.");
    }

    #[test]
    fn uses_enum_key_then_identifier_when_long_name_is_unavailable() {
        let bundle = ReqIfParser::parse_str(&FIXTURE.replace(r#"LONG-NAME="Active""#, "")).unwrap();
        let graph = bundle_to_requirement_graph(&bundle, &config()).unwrap();
        assert_eq!(
            graph.requirements[0]
                .attributes
                .get("status")
                .map(String::as_str),
            Some("active")
        );

        let bundle = ReqIfParser::parse_str(&FIXTURE.replace(
            ">EV-ACTIVE</ENUM-VALUE-REF>",
            ">EV-UNKNOWN</ENUM-VALUE-REF>",
        ))
        .unwrap();
        let graph = bundle_to_requirement_graph(&bundle, &config()).unwrap();
        assert_eq!(
            graph.requirements[0]
                .attributes
                .get("status")
                .map(String::as_str),
            Some("EV-UNKNOWN")
        );
    }

    #[test]
    fn maps_spec_relation_to_requirement_relation_with_kind_from_relation_type() {
        let bundle = ReqIfParser::parse_str(FIXTURE).unwrap();
        let graph = bundle_to_requirement_graph(&bundle, &config()).unwrap();

        assert_eq!(graph.relations.len(), 1);
        let rel = &graph.relations[0];
        assert_eq!(rel.id, "REL-1");
        assert_eq!(rel.source, "REQ-2");
        assert_eq!(rel.target, "REQ-1");
        assert_eq!(rel.kind, RequirementRelationKind::Derives);
        assert!(rel.authority.is_asserted());
    }

    #[test]
    fn returned_graph_passes_validation() {
        let bundle = ReqIfParser::parse_str(FIXTURE).unwrap();
        let graph = bundle_to_requirement_graph(&bundle, &config()).unwrap();
        assert!(graph.validate().is_ok());
    }

    #[test]
    fn missing_content_is_a_typed_error() {
        let bundle = ReqIfParser::parse_str(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<REQ-IF xmlns="http://www.omg.org/spec/ReqIF/20110401/reqif.xsd">
  <THE-HEADER><REQ-IF-HEADER IDENTIFIER="EMPTY"></REQ-IF-HEADER></THE-HEADER>
</REQ-IF>"#,
        )
        .unwrap();
        let err = bundle_to_requirement_graph(&bundle, &config()).unwrap_err();
        assert!(matches!(err, ReqIfAdapterError::MissingContent));
    }

    #[test]
    fn unrecognized_relation_type_name_falls_back_to_traces() {
        let xml = FIXTURE.replace(
            r#"LONG-NAME="Derives""#,
            r#"LONG-NAME="Custom Vendor Link""#,
        );
        let bundle = ReqIfParser::parse_str(&xml).unwrap();
        let graph = bundle_to_requirement_graph(&bundle, &config()).unwrap();
        assert_eq!(graph.relations[0].kind, RequirementRelationKind::Traces);
    }
}
