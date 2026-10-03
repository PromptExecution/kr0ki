//! The assurance baseline's authoring format → [`RequirementGraph`].
//!
//! `docs/assurance/kr0ki.assurance.toml` is the diffable authoring source (what the
//! requirements agent proposes edits to). This module lowers it onto the existing
//! requirement graph using the `ufo_types::mbse::assurance` profile, so ReqIF export, the
//! requirements views, Flexo sync, MCP and Playb00k keep consuming one graph type.
//!
//! The loader is deterministic and strict: unknown keys are errors (a typo must not silently
//! drop a field), every reference inside the file must resolve, and every requirement must
//! carry the nine required profile fields. It never invents data.
//!
//! What is *not* checked here: whether a system element exists in a model revision
//! (`assurance_trace`, KR-A02) and whether a control's implementation actually works
//! (the verification cases).

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use sha2::{Digest, Sha256};
use ufo_types::mbse::assurance::{
    statement_issues, NodeKind, ProfileIssue, ProfileRequirement, SourceKind, StatementIssue,
    NODE_KIND_ATTR,
};
use ufo_types::mbse::requirements::{
    BaselineIdentity, Provenance, RelationAuthority, Requirement, RequirementError,
    RequirementGraph, RequirementRelation, RequirementRelationKind,
};

/// Why a baseline file could not be loaded.
#[derive(Debug, thiserror::Error)]
pub enum BaselineError {
    #[error("baseline TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("{field} `{value}` referenced by `{owner}` is not declared in the baseline")]
    UnknownReference {
        owner: String,
        field: &'static str,
        value: String,
    },
    #[error("`{id}`: invalid {field} `{value}`")]
    InvalidValue {
        id: String,
        field: &'static str,
        value: String,
    },
    #[error("requirement profile problems: {0:?}")]
    Profile(Vec<ProfileIssue>),
    #[error(transparent)]
    Graph(#[from] RequirementError),
}

/// A lowered baseline plus the findings that do not stop it loading.
#[derive(Debug, Clone)]
pub struct LoadedBaseline {
    pub graph: RequirementGraph,
    /// Deterministic statement lint, per requirement id. Empty for a clean baseline.
    pub statement_lints: Vec<(String, StatementIssue)>,
    /// Path of the SysML model the `element` locators resolve against, as written in the file.
    pub model: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Doc {
    baseline: BaselineDoc,
    #[serde(default)]
    obligation: Vec<ObligationDoc>,
    #[serde(default)]
    element: Vec<ElementDoc>,
    #[serde(default)]
    control: Vec<ControlDoc>,
    #[serde(default)]
    case: Vec<CaseDoc>,
    #[serde(default)]
    requirement: Vec<RequirementDoc>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BaselineDoc {
    id: String,
    #[allow(dead_code)]
    title: String,
    model: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ObligationDoc {
    id: String,
    title: String,
    source: String,
    version: String,
    kind: String,
    summary: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ElementDoc {
    id: String,
    title: String,
    description: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ControlDoc {
    id: String,
    title: String,
    component: String,
    #[serde(default)]
    implementation: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CaseDoc {
    id: String,
    title: String,
    command: Vec<String>,
    acceptance: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DerivesDoc {
    obligation: String,
    locator: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequirementDoc {
    id: String,
    title: String,
    statement: String,
    source: String,
    source_kind: String,
    owner: String,
    rationale: String,
    element: String,
    control: String,
    verification_id: String,
    acceptance: String,
    status: String,
    #[serde(default)]
    derives: Vec<DerivesDoc>,
}

/// Lowercase hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Digest::finalize(Sha256::new().chain_update(bytes))
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Lower the baseline file onto a [`RequirementGraph`].
///
/// - `toml_text`: the file's content.
/// - `source_uri`: where it lives (repo-relative path or URL), recorded as provenance.
/// - `revision`: the immutable revision the baseline is read at (a git commit id).
/// - `model_revision`: the model revision the `system_element` locators belong to, recorded
///   on each element node when known.
pub fn load_baseline(
    toml_text: &str,
    source_uri: &str,
    revision: &str,
    model_revision: Option<&str>,
) -> Result<LoadedBaseline, BaselineError> {
    let doc: Doc = toml::from_str(toml_text)?;
    let digest = sha256_hex(toml_text.as_bytes());
    let baseline = BaselineIdentity {
        id: doc.baseline.id.clone(),
        revision: revision.to_string(),
        import_artifact_sha256: Some(digest.clone()),
        exported_baseline_sha256: None,
    };
    let prov = |locator: String| Provenance {
        source_uri: source_uri.to_string(),
        artifact_sha256: Some(digest.clone()),
        locator: Some(locator),
    };
    let node = |id: &str,
                title: &str,
                text: &str,
                locator: String,
                attributes: BTreeMap<String, String>| Requirement {
        id: id.to_string(),
        title: title.to_string(),
        text: text.to_string(),
        baseline: baseline.clone(),
        provenance: prov(locator),
        attributes,
        evidence: Vec::new(),
    };
    let kind_attr = |kind: NodeKind| (NODE_KIND_ATTR.to_string(), kind.as_str().to_string());

    let mut nodes: Vec<Requirement> = Vec::new();
    let mut declared: BTreeMap<String, NodeKind> = BTreeMap::new();

    for o in &doc.obligation {
        let kind = SourceKind::parse(&o.kind).ok_or_else(|| BaselineError::InvalidValue {
            id: o.id.clone(),
            field: "kind",
            value: o.kind.clone(),
        })?;
        declared.insert(o.id.clone(), NodeKind::SourceObligation);
        nodes.push(node(
            &o.id,
            &o.title,
            &o.summary,
            format!("obligation[{}]", o.id),
            BTreeMap::from([
                kind_attr(NodeKind::SourceObligation),
                ("source".into(), o.source.clone()),
                ("source_kind".into(), kind.as_str().into()),
                ("version".into(), o.version.clone()),
            ]),
        ));
    }
    for e in &doc.element {
        declared.insert(e.id.clone(), NodeKind::SystemElement);
        let mut attrs = BTreeMap::from([
            kind_attr(NodeKind::SystemElement),
            ("qualified_name".into(), e.id.clone()),
        ]);
        if let Some(m) = model_revision {
            attrs.insert("model_revision".into(), m.to_string());
        }
        nodes.push(node(
            &e.id,
            &e.title,
            &e.description,
            format!("element[{}]", e.id),
            attrs,
        ));
    }
    for c in &doc.control {
        if c.component != "kr0ki" && c.component != "deployment" {
            return Err(BaselineError::InvalidValue {
                id: c.id.clone(),
                field: "component",
                value: c.component.clone(),
            });
        }
        declared.insert(c.id.clone(), NodeKind::Control);
        nodes.push(node(
            &c.id,
            &c.title,
            &c.title,
            format!("control[{}]", c.id),
            BTreeMap::from([
                kind_attr(NodeKind::Control),
                ("component".into(), c.component.clone()),
                ("implementation".into(), c.implementation.clone()),
            ]),
        ));
    }
    for c in &doc.case {
        declared.insert(c.id.clone(), NodeKind::VerificationCase);
        let command = serde_json::to_string(&c.command).expect("a Vec<String> serializes");
        nodes.push(node(
            &c.id,
            &c.title,
            &c.acceptance,
            format!("case[{}]", c.id),
            BTreeMap::from([
                kind_attr(NodeKind::VerificationCase),
                ("command".into(), command),
            ]),
        ));
    }

    let mut relations: Vec<RequirementRelation> = Vec::new();
    let mut seen_relations: BTreeSet<String> = BTreeSet::new();
    let mut relate = |kind: RequirementRelationKind,
                      source: &str,
                      target: &str,
                      locator: String,
                      relations: &mut Vec<RequirementRelation>| {
        let id = format!("{}:{source}->{target}", relation_slug(kind));
        if seen_relations.insert(id.clone()) {
            relations.push(RequirementRelation {
                id,
                source: source.to_string(),
                target: target.to_string(),
                kind,
                authority: RelationAuthority::Asserted,
                provenance: prov(locator),
                promotion: None,
            });
        }
    };

    let mut profile_issues = Vec::new();
    let mut statement_lints = Vec::new();
    for r in &doc.requirement {
        reference(
            &declared,
            &r.id,
            "element",
            &r.element,
            NodeKind::SystemElement,
        )?;
        reference(&declared, &r.id, "control", &r.control, NodeKind::Control)?;
        reference(
            &declared,
            &r.id,
            "verification_id",
            &r.verification_id,
            NodeKind::VerificationCase,
        )?;
        for d in &r.derives {
            reference(
                &declared,
                &r.id,
                "derives.obligation",
                &d.obligation,
                NodeKind::SourceObligation,
            )?;
        }
        if declared.contains_key(&r.id) {
            return Err(RequirementError::DuplicateNode(r.id.clone()).into());
        }
        declared.insert(r.id.clone(), NodeKind::Requirement);

        // Build the node, then read it back through the profile so a malformed field is
        // reported exactly as it would be for any other producer of these nodes.
        let attributes = BTreeMap::from([
            kind_attr(NodeKind::Requirement),
            ("source".into(), r.source.clone()),
            ("source_kind".into(), r.source_kind.clone()),
            ("owner".into(), r.owner.clone()),
            ("rationale".into(), r.rationale.clone()),
            ("verification_id".into(), r.verification_id.clone()),
            ("acceptance".into(), r.acceptance.clone()),
            ("status".into(), r.status.clone()),
        ]);
        let n = node(
            &r.id,
            &r.title,
            &r.statement,
            format!("requirement[{}]", r.id),
            attributes,
        );
        match ProfileRequirement::from_node(&n) {
            Ok(_) => {}
            Err(mut issues) => profile_issues.append(&mut issues),
        }
        for issue in statement_issues(&n.text) {
            statement_lints.push((n.id.clone(), issue));
        }
        nodes.push(n);

        for d in &r.derives {
            relate(
                RequirementRelationKind::Derives,
                &d.obligation,
                &r.id,
                d.locator.clone(),
                &mut relations,
            );
        }
        relate(
            RequirementRelationKind::Satisfies,
            &r.element,
            &r.id,
            format!("requirement[{}].element", r.id),
            &mut relations,
        );
        relate(
            RequirementRelationKind::Implements,
            &r.control,
            &r.id,
            format!("requirement[{}].control", r.id),
            &mut relations,
        );
        relate(
            RequirementRelationKind::AllocatedTo,
            &r.control,
            &r.element,
            format!("requirement[{}]", r.id),
            &mut relations,
        );
        relate(
            RequirementRelationKind::Verifies,
            &r.verification_id,
            &r.id,
            format!("requirement[{}].verification_id", r.id),
            &mut relations,
        );
    }
    if !profile_issues.is_empty() {
        return Err(BaselineError::Profile(profile_issues));
    }
    let graph = RequirementGraph {
        baseline,
        requirements: nodes,
        evidence: Vec::new(),
        relations,
    };
    graph.validate()?;
    Ok(LoadedBaseline {
        graph,
        statement_lints,
        model: doc.baseline.model,
    })
}

fn reference(
    declared: &BTreeMap<String, NodeKind>,
    owner: &str,
    field: &'static str,
    value: &str,
    want: NodeKind,
) -> Result<(), BaselineError> {
    if declared.get(value) == Some(&want) {
        Ok(())
    } else {
        Err(BaselineError::UnknownReference {
            owner: owner.to_string(),
            field,
            value: value.to_string(),
        })
    }
}

fn relation_slug(kind: RequirementRelationKind) -> &'static str {
    match kind {
        RequirementRelationKind::Contains => "contains",
        RequirementRelationKind::Derives => "derives",
        RequirementRelationKind::Refines => "refines",
        RequirementRelationKind::Requires => "requires",
        RequirementRelationKind::Satisfies => "satisfies",
        RequirementRelationKind::Verifies => "verifies",
        RequirementRelationKind::Implements => "implements",
        RequirementRelationKind::Traces => "traces",
        RequirementRelationKind::AllocatedTo => "allocated_to",
        RequirementRelationKind::Precedes => "precedes",
    }
}
