//! Stores a ReqIF-derived `RequirementGraph` baseline in a live SysML v2 project and reads
//! it back (Flexo baseline adapter §2/§3, `docs/superpowers/specs/2026-09-23-flexo-baseline-adapter-design.md`).
//!
//! Each `Requirement` becomes one `RequirementUsage` element, following the
//! `digital_thread_sync` precedent of storing kr0ki-domain data as ordinary elements in a
//! managed identifier namespace (`reqif:<baseline>:<requirement>`) rather than a bespoke
//! store. The fetch -> diff -> commit mechanics (branch-head resolution, stale-head detection,
//! conflict retry, full-replace update payloads that preserve other tools' fields) are the
//! shared [`crate::sync_engine`]; only the mapping in this module is ReqIF-specific.
//!
//! Owned fields on each element: `name` (title), `text`, `reqif_baseline_id`,
//! `reqif_baseline_revision`, `reqif_source_sha256`, and the round-trip fields
//! `reqif_provenance`, `reqif_attributes`, `reqif_evidence`, and `reqif_relations` (the
//! *asserted* relations whose source is this requirement, as `{id, kind, target}`).
//! Inferred/proposed relations are not stored, matching `reqif_export`.
//!
//! Known limits: graph-level `evidence` is not stored (only per-requirement evidence); a
//! reconstructed relation takes its source requirement's provenance rather than its own; and
//! syncing an empty baseline deletes that baseline's previously stored requirements (that is
//! what reconciling to "no requirements" means) but never touches another baseline's.

use crate::requirements::{
    BaselineIdentity, EvidenceRef, Provenance, RelationAuthority, Requirement, RequirementGraph,
    RequirementRelation, RequirementRelationKind,
};
use crate::sync_engine::{
    diff_managed, element_identifier, resolve_head, sync_managed, DesiredElement, SyncConfig,
    SyncError,
};
use kr0ki_sysmlv2_client::{Commit, Element, SysmlV2Client};
use serde::de::DeserializeOwned;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

const ELEMENT_TYPE: &str = "RequirementUsage";

/// Escape `%` and `:` so a baseline/requirement id can never be confused with the `:`
/// separators of the identifier it is embedded in.
fn escape(s: &str) -> String {
    s.replace('%', "%25").replace(':', "%3A")
}

fn unescape(s: &str) -> String {
    s.replace("%3A", ":").replace("%25", "%")
}

/// Every element this module manages for `baseline_id` has an identifier with this prefix.
pub fn baseline_prefix(baseline_id: &str) -> String {
    format!("reqif:{}:", escape(baseline_id))
}

/// The stable server-side identity of a requirement within a baseline.
pub fn requirement_identifier(baseline_id: &str, requirement_id: &str) -> String {
    format!("{}{}", baseline_prefix(baseline_id), escape(requirement_id))
}

/// A stored baseline that cannot be turned back into a `RequirementGraph`.
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error(transparent)]
    Sync(#[from] SyncError),

    #[error("stored requirement '{identifier}' is malformed: {reason}")]
    Malformed { identifier: String, reason: String },
}

fn desired_elements(graph: &RequirementGraph) -> Result<Vec<DesiredElement>, SyncError> {
    let baseline_id = &graph.baseline.id;
    graph
        .requirements
        .iter()
        .map(|req| {
            let mut relations: Vec<&RequirementRelation> = graph
                .relations
                .iter()
                .filter(|r| r.source == req.id && r.authority == RelationAuthority::Asserted)
                .collect();
            relations.sort_by(|a, b| a.id.cmp(&b.id));
            let relations: Vec<Value> = relations
                .iter()
                .map(|r| {
                    Ok(json!({
                        "id": r.id,
                        "kind": serde_json::to_value(r.kind)?,
                        "target": r.target,
                    }))
                })
                .collect::<Result<_, SyncError>>()?;

            let mut fields = Map::new();
            fields.insert("name".into(), req.title.clone().into());
            // `text` is `String[0..*]` on a SysML v2 RequirementUsage; the pilot server rejects a bare string (HTTP 500, found live).
            fields.insert("text".into(), serde_json::json!([req.text]));
            fields.insert("reqif_baseline_id".into(), baseline_id.clone().into());
            fields.insert(
                "reqif_baseline_revision".into(),
                graph.baseline.revision.clone().into(),
            );
            if let Some(sha) = &graph.baseline.import_artifact_sha256 {
                fields.insert("reqif_source_sha256".into(), sha.clone().into());
            }
            fields.insert(
                "reqif_provenance".into(),
                serde_json::to_value(&req.provenance)?,
            );
            fields.insert(
                "reqif_attributes".into(),
                serde_json::to_value(&req.attributes)?,
            );
            fields.insert(
                "reqif_evidence".into(),
                serde_json::to_value(&req.evidence)?,
            );
            fields.insert("reqif_relations".into(), Value::Array(relations));
            Ok(DesiredElement {
                identifier: requirement_identifier(baseline_id, &req.id),
                type_: ELEMENT_TYPE,
                fields,
            })
        })
        .collect()
}

/// Reconcile `graph`'s requirements into the project as at most one commit. Only elements in
/// `graph.baseline`'s namespace are created, updated or deleted. Returns the new commit, the
/// unchanged head when the baseline already matches, or `None` for an empty project with
/// nothing to write. See [`crate::sync_engine::sync_managed`] for head/conflict semantics.
pub async fn sync_requirement_baseline(
    client: &SysmlV2Client,
    graph: &RequirementGraph,
    config: &SyncConfig,
) -> Result<Option<Commit>, SyncError> {
    let desired = desired_elements(graph)?;
    let prefix = baseline_prefix(&graph.baseline.id);
    sync_managed(
        client,
        config,
        |e| element_identifier(e).is_some_and(|id| id.starts_with(&prefix)),
        |managed| diff_managed(managed, &desired),
    )
    .await
}

fn field<'a>(e: &'a Element, identifier: &str, name: &str) -> Result<&'a Value, FetchError> {
    e.fields
        .get(name)
        .ok_or_else(|| malformed(identifier, format!("missing field '{name}'")))
}

fn malformed(identifier: &str, reason: String) -> FetchError {
    FetchError::Malformed {
        identifier: identifier.to_string(),
        reason,
    }
}

/// A required field, decoded as `T`.
fn required<T: DeserializeOwned>(
    e: &Element,
    identifier: &str,
    name: &str,
) -> Result<T, FetchError> {
    serde_json::from_value(field(e, identifier, name)?.clone())
        .map_err(|err| malformed(identifier, format!("field '{name}': {err}")))
}

/// `text` as stored by a SysML v2 server (a list of strings, joined by newlines). A bare string is also accepted so
/// elements written before the list form was fixed still read back.
fn requirement_text(e: &Element, identifier: &str) -> Result<String, FetchError> {
    let v = field(e, identifier, "text")?;
    match v {
        serde_json::Value::String(s) => Ok(s.clone()),
        serde_json::Value::Array(items) => items
            .iter()
            .map(|i| i.as_str().map(str::to_owned))
            .collect::<Option<Vec<_>>>()
            .map(|parts| parts.join("\n"))
            .ok_or_else(|| {
                malformed(
                    identifier,
                    "field 'text': list must contain only strings".into(),
                )
            }),
        _ => Err(malformed(
            identifier,
            "field 'text': expected a string or a list of strings".into(),
        )),
    }
}

/// An optional field: absent means `T::default()`, present-but-wrong is still an error.
fn optional<T: DeserializeOwned + Default>(
    e: &Element,
    identifier: &str,
    name: &str,
) -> Result<T, FetchError> {
    if e.fields.contains_key(name) {
        required(e, identifier, name)
    } else {
        Ok(T::default())
    }
}

/// Rebuild a `RequirementGraph` from a baseline's stored elements (`elements` may contain
/// other baselines' or unrelated elements; they are ignored). The baseline's own `revision`
/// (the import's source revision) is restored from the stored elements; `fallback_revision`
/// -- normally the commit they were read at -- is used only if none was stored.
/// Requirements and relations come back sorted by id.
pub fn requirement_graph_from_elements(
    baseline_id: &str,
    fallback_revision: &str,
    elements: &[Element],
) -> Result<RequirementGraph, FetchError> {
    let prefix = baseline_prefix(baseline_id);
    let managed: Vec<(&Element, &str)> = elements
        .iter()
        .filter_map(|e| element_identifier(e).map(|id| (e, id)))
        .filter(|(_, id)| id.starts_with(&prefix))
        .collect();

    let baseline = BaselineIdentity {
        id: baseline_id.to_string(),
        revision: managed
            .iter()
            .find_map(|(e, _)| e.fields.get("reqif_baseline_revision")?.as_str())
            .unwrap_or(fallback_revision)
            .to_string(),
        import_artifact_sha256: managed
            .iter()
            .find_map(|(e, _)| e.fields.get("reqif_source_sha256")?.as_str())
            .map(str::to_owned),
        exported_baseline_sha256: None,
    };

    let mut requirements = Vec::new();
    let mut relations = Vec::new();
    for (e, identifier) in managed {
        let id = unescape(&identifier[prefix.len()..]);
        if !e.fields.contains_key("identifier") && !e.fields.contains_key("reqif_provenance") {
            // Only the alias marker survived: a strictly typed server (the OMG pilot) drops extension fields. Provenance,
            // attributes, evidence and relations are not recoverable from it, and inventing them would be worse.
            return Err(malformed(
                identifier,
                "the server does not persist kr0ki's extension fields (reqif_provenance, relations, evidence); only id, name and text survive on a strictly typed SysML v2 server".into(),
            ));
        }
        let provenance: Provenance = required(e, identifier, "reqif_provenance")?;
        let attributes: BTreeMap<String, String> = optional(e, identifier, "reqif_attributes")?;
        let evidence: Vec<EvidenceRef> = optional(e, identifier, "reqif_evidence")?;

        #[derive(serde::Deserialize)]
        struct StoredRelation {
            id: String,
            kind: RequirementRelationKind,
            target: String,
        }
        let stored: Vec<StoredRelation> = optional(e, identifier, "reqif_relations")?;
        relations.extend(stored.into_iter().map(|r| RequirementRelation {
            id: r.id,
            source: id.clone(),
            target: r.target,
            kind: r.kind,
            authority: RelationAuthority::Asserted,
            provenance: provenance.clone(),
            promotion: None,
        }));

        requirements.push(Requirement {
            id,
            title: required(e, identifier, "name")?,
            text: requirement_text(e, identifier)?,
            baseline: baseline.clone(),
            provenance,
            attributes,
            evidence,
        });
    }
    requirements.sort_by(|a, b| a.id.cmp(&b.id));
    relations.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(RequirementGraph {
        baseline,
        requirements,
        evidence: Vec::new(),
        relations,
    })
}

/// Read a baseline back from the branch head. `None` when the target has no commits.
pub async fn fetch_requirement_baseline(
    client: &SysmlV2Client,
    config: &SyncConfig,
    baseline_id: &str,
) -> Result<Option<RequirementGraph>, FetchError> {
    let Some(head) = resolve_head(client, config).await? else {
        return Ok(None);
    };
    let elements = client
        .all_elements(&config.project_id, &head.at_id)
        .await
        .map_err(SyncError::from)?;
    requirement_graph_from_elements(baseline_id, &head.at_id, &elements).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reqif_export::export_bundle_to_xml;
    use std::collections::BTreeSet;
    use ufo_types::reqif::{parse_and_lower, ReqIfAdapterConfig};

    const FIXTURE: &[u8] = include_bytes!("../tests/fixtures/reqif/roundtrip.reqif");

    fn adapter_config() -> ReqIfAdapterConfig {
        ReqIfAdapterConfig {
            source_uri: "test:roundtrip".to_string(),
            revision: "r1".to_string(),
            import_artifact_sha256: Some("abc123".to_string()),
        }
    }

    /// Stand in for the server: turn create payloads into the elements a later fetch returns.
    fn as_server_elements(desired: Vec<DesiredElement>) -> Vec<Element> {
        desired
            .into_iter()
            .enumerate()
            .map(|(i, d)| {
                let mut fields = d.fields;
                fields.insert("identifier".into(), d.identifier.into());
                Element {
                    at_id: format!("srv-{i}"),
                    at_type: d.type_.to_string(),
                    fields,
                }
            })
            .collect()
    }

    fn asserted_triples(g: &RequirementGraph) -> BTreeSet<(String, String, String, String)> {
        g.relations
            .iter()
            .filter(|r| r.authority == RelationAuthority::Asserted)
            .map(|r| {
                (
                    r.id.clone(),
                    r.source.clone(),
                    r.target.clone(),
                    format!("{:?}", r.kind),
                )
            })
            .collect()
    }

    #[test]
    fn identifiers_escape_separators_and_round_trip() {
        for id in ["REQ-1", "a:b", "100%", "%3A", "x::y%25"] {
            assert_eq!(unescape(&escape(id)), id);
            let full = requirement_identifier("BL:1", id);
            assert!(full.starts_with(&baseline_prefix("BL:1")));
            assert_eq!(unescape(&full[baseline_prefix("BL:1").len()..]), id);
        }
        // "a" is not a prefix-neighbour of "a:b": distinct baselines never share a namespace.
        assert!(!requirement_identifier("a:b", "x").starts_with(&baseline_prefix("a")));
    }

    #[test]
    fn fixture_survives_graph_to_elements_to_graph_to_reqif() {
        let original = parse_and_lower(FIXTURE, &adapter_config()).expect("fixture lowers");
        assert!(!original.requirements.is_empty());

        // graph -> stored elements -> graph
        let elements = as_server_elements(desired_elements(&original).unwrap());
        let back = requirement_graph_from_elements(
            &original.baseline.id,
            &original.baseline.revision,
            &elements,
        )
        .unwrap();

        let mut expected = original.requirements.clone();
        expected.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(
            back.requirements, expected,
            "requirements must round-trip exactly"
        );
        assert_eq!(asserted_triples(&back), asserted_triples(&original));
        assert_eq!(
            back.baseline.import_artifact_sha256.as_deref(),
            Some("abc123")
        );

        // -> ReqIF XML -> re-parse: the stored baseline still exports to a semantically equal graph
        let xml = export_bundle_to_xml(&back).expect("export");
        let reparsed = ufo_types::reqif::bundle_to_requirement_graph(
            &reqrs::ReqIfParser::parse_str(&xml).expect("re-parse"),
            &adapter_config(),
        )
        .expect("lower");
        let key = |g: &RequirementGraph| {
            g.requirements
                .iter()
                .map(|r| (r.id.clone(), r.title.clone(), r.text.clone()))
                .collect::<BTreeSet<_>>()
        };
        assert_eq!(key(&reparsed), key(&original));
        assert_eq!(asserted_triples(&reparsed), asserted_triples(&original));
    }

    #[test]
    fn reconstruct_ignores_other_baselines_and_reports_malformed_elements() {
        let original = parse_and_lower(FIXTURE, &adapter_config()).unwrap();
        let mut elements = as_server_elements(desired_elements(&original).unwrap());
        // an unrelated element and another baseline's element must be ignored
        elements.push(
            serde_json::from_value(
                json!({"@id": "x", "@type": "PartUsage", "identifier": "dbt:m", "name": "M"}),
            )
            .unwrap(),
        );
        elements.push(
            serde_json::from_value(json!({"@id": "y", "@type": "RequirementUsage", "identifier": "reqif:OTHER:R1", "name": "Other"}))
                .unwrap(),
        );
        let ok = requirement_graph_from_elements(&original.baseline.id, "r1", &elements).unwrap();
        assert_eq!(ok.requirements.len(), original.requirements.len());

        // corrupt one stored requirement: missing `text`
        elements[0].fields.remove("text");
        assert!(matches!(
            requirement_graph_from_elements(&original.baseline.id, "r1", &elements),
            Err(FetchError::Malformed { .. })
        ));
    }
}
