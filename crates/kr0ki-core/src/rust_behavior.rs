//! Validated compiler facts → UFO graph → model-authored SysML views.
//!
//! Compiler execution belongs to the isolated extractor, never the HTTP route.
//! The source IR is retained alongside the semantic graph: UFO relations alone
//! cannot encode a dispatch's uncertainty or a control edge's guard.

use std::collections::{BTreeMap, BTreeSet};

use kr0ki_behavior::{EdgeKind, NodeKind, Resolution, RustBehaviorIr};
use oxigraph::{
    model::{GraphNameRef, Literal, NamedNode, QuadRef},
    sparql::{QueryResults, SparqlEvaluator},
    store::Store,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ufo_types::{
    ontology::{OntologicalEdge, SourceAnchor, UfoRelation},
    stereotype::UfoStereotype,
    sysgraph::{OntologicalNode, SysGraph},
    sysml_model::{ElementId, ElementKind},
    view_definition::{Expose, ModelIndex, ViewDefinition},
};

pub const RULE_VERSION: &str = "rust-behavior-v1";
pub const MAX_NODES: usize = 50_000;
pub const MAX_EDGES: usize = 200_000;

#[derive(Debug, thiserror::Error)]
pub enum BehaviorError {
    #[error("invalid behavior model: {0}")]
    Invalid(String),
    #[error("invalid view: {0}")]
    View(String),
    #[error("notation validation failed: {0}")]
    Notation(String),
    #[error("graph validation failed: {0}")]
    Graph(String),
}

/// JSON boundary: existing upstream view types, plus bounded body expansion.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BehaviorRequest {
    pub model: RustBehaviorIr,
    /// Explicit, reviewed ontology classifications; never inferred by a model.
    #[serde(default)]
    pub stereotypes: BTreeMap<String, UfoStereotype>,
    /// Reviewed exact source-attribute text → stereotype rules. Attribute names
    /// alone never establish an ontology classification.
    #[serde(default)]
    pub annotation_stereotypes: BTreeMap<String, UfoStereotype>,
    #[serde(default)]
    pub view: Option<ViewDefinition>,
    #[serde(default)]
    pub expand_depth: u8,
    #[serde(default)]
    pub strict: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotationValidation {
    pub notation: String,
    pub level: String,
    pub validator: String,
}

#[derive(Debug, Serialize)]
pub struct BehaviorView {
    pub graph: SysGraph,
    pub view: ViewDefinition,
    pub selected_nodes: Vec<String>,
    /// Stable rendered edge id → full source IR edge, including guard/status.
    pub evidence: BTreeMap<String, kr0ki_behavior::Edge>,
    pub d2: String,
    pub mermaid: String,
    pub sysml: String,
    /// Complete shared tables whose states and transition edges are all selected.
    /// A partial authored view never becomes a different executable machine.
    pub scxml: BTreeMap<String, String>,
    pub validation: Vec<NotationValidation>,
    pub content_hash: String,
}

fn slug<T: Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .expect("closed enum serialization")
        .as_str()
        .expect("closed enum string")
        .to_owned()
}

fn element_kind(kind: NodeKind) -> ElementKind {
    match kind {
        NodeKind::Module => ElementKind::Package,
        NodeKind::Type | NodeKind::Trait | NodeKind::External => ElementKind::PartDefinition,
        NodeKind::Field => ElementKind::AttributeUsage,
        NodeKind::AssociatedType => ElementKind::ItemDefinition,
        NodeKind::Function => ElementKind::ActionDefinition,
        NodeKind::State => ElementKind::StateUsage,
        _ => ElementKind::ActionUsage,
    }
}

fn relation(kind: EdgeKind) -> UfoRelation {
    match kind {
        EdgeKind::Contains => UfoRelation::HasPart,
        EdgeKind::Satisfies => UfoRelation::Satisfies,
        EdgeKind::Requires => UfoRelation::Requires,
        EdgeKind::GovernedBy => UfoRelation::GovernedBy,
        EdgeKind::Transition => UfoRelation::Transitions,
        // These are source control transfers, not assertions about execution.
        _ => UfoRelation::FlowsTo,
    }
}

/// Every behavior edge is lifted, with precise evidence retained in the IR.
pub fn build_graph(model: &RustBehaviorIr) -> Result<SysGraph, BehaviorError> {
    model
        .ensure_valid()
        .map_err(|e| BehaviorError::Invalid(e.to_string()))?;
    let prefix = model
        .provenance
        .config
        .get("source_prefix")
        .map(String::as_str)
        .unwrap_or("");
    if !prefix.is_empty()
        && prefix.split('/').any(|component| {
            component.is_empty() || matches!(component, "." | "..") || component.contains('\\')
        })
    {
        return Err(BehaviorError::Invalid(
            "invalid repository source prefix".into(),
        ));
    }
    let mut graph = SysGraph::new();
    for n in &model.nodes {
        // Classify source-language constructs; never infer a business stereotype
        // from a Rust trait's spelling.
        let stereotype = UfoStereotype::Abstract(format!("Rust::{}", slug(&n.kind)));
        graph.push_node(OntologicalNode::with_label(
            ElementId::new(&n.id),
            stereotype,
            &n.name,
        ));
    }
    for e in &model.edges {
        let mut edge = OntologicalEdge::new(
            &e.id,
            ElementId::new(&e.from),
            ElementId::new(&e.to),
            relation(e.kind),
        );
        let file = model
            .sources
            .iter()
            .find(|f| f.path == e.anchor.file)
            .ok_or_else(|| BehaviorError::Invalid("missing source file".into()))?;
        let preceding = &file.content[..e.anchor.start as usize];
        let line = preceding.bytes().filter(|b| *b == b'\n').count() as u32 + 1;
        let col = preceding.rsplit('\n').next().unwrap_or("").chars().count() as u32 + 1;
        edge.provenance = vec![
            SourceAnchor::SymbolPath(e.anchor.symbol.clone()),
            SourceAnchor::RustSpan {
                file: e.anchor.file.clone(),
                line,
                col: Some(col),
            },
            SourceAnchor::Other(
                serde_json::to_string(e).map_err(|e| BehaviorError::Invalid(e.to_string()))?,
            ),
        ];
        if model
            .provenance
            .config
            .get(&format!("source_origin:{}", e.anchor.file))
            .is_some_and(|origin| origin == "declared-state-machine")
        {
            edge.provenance.push(SourceAnchor::Other(format!(
                "declared-state-machine source sha256:{}",
                file.sha256
            )));
        } else {
            edge.provenance.push(SourceAnchor::Vcs {
                repo: None,
                commit: model.provenance.revision.clone(),
                path: Some(if prefix.is_empty() {
                    e.anchor.file.clone()
                } else {
                    format!("{prefix}/{}", e.anchor.file)
                }),
            });
        }
        graph.push_edge(edge);
    }
    if !graph.dangling_edges().is_empty() {
        return Err(BehaviorError::Graph("dangling semantic endpoints".into()));
    }
    validate_rdf(&graph)?;
    Ok(graph)
}

fn iri(value: &str) -> NamedNode {
    NamedNode::new_unchecked(format!(
        "urn:kr0ki:behavior:{}",
        hex_digest(value.as_bytes())
    ))
}

pub fn hex_digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Disposable RDF materialization: no second canonical database.
fn validate_rdf(graph: &SysGraph) -> Result<(), BehaviorError> {
    let store = Store::new().map_err(|e| BehaviorError::Graph(e.to_string()))?;
    let node = NamedNode::new_unchecked("urn:kr0ki:behavior:node");
    let from = NamedNode::new_unchecked("urn:kr0ki:behavior:from");
    let to = NamedNode::new_unchecked("urn:kr0ki:behavior:to");
    for n in &graph.nodes {
        store
            .insert(QuadRef::new(
                &iri(n.id.as_str()),
                &node,
                &Literal::from(true),
                GraphNameRef::DefaultGraph,
            ))
            .map_err(|e| BehaviorError::Graph(e.to_string()))?;
    }
    for e in &graph.edges {
        for (p, target) in [(&from, &e.source), (&to, &e.target)] {
            store
                .insert(QuadRef::new(
                    &iri(&e.id),
                    p,
                    &iri(target.as_str()),
                    GraphNameRef::DefaultGraph,
                ))
                .map_err(|e| BehaviorError::Graph(e.to_string()))?;
        }
    }
    let query = "ASK { ?e ?p ?target . VALUES ?p { <urn:kr0ki:behavior:from> <urn:kr0ki:behavior:to> } FILTER NOT EXISTS { ?target <urn:kr0ki:behavior:node> true } }";
    let result = SparqlEvaluator::new()
        .parse_query(query)
        .map_err(|e| BehaviorError::Graph(e.to_string()))?
        .on_store(&store)
        .execute()
        .map_err(|e| BehaviorError::Graph(e.to_string()))?;
    match result {
        QueryResults::Boolean(false) => Ok(()),
        _ => Err(BehaviorError::Graph("RDF endpoint shape violation".into())),
    }
}

pub fn prepare(mut request: BehaviorRequest) -> Result<BehaviorView, BehaviorError> {
    if request.expand_depth > 8 {
        return Err(BehaviorError::View("expand_depth must be 0..=8".into()));
    }
    if request.model.nodes.len() > MAX_NODES || request.model.edges.len() > MAX_EDGES {
        return Err(BehaviorError::Invalid(
            "model exceeds graph size limits".into(),
        ));
    }
    let validated = RustBehaviorIr::from_json(
        &serde_json::to_string(&request.model)
            .map_err(|e| BehaviorError::Invalid(e.to_string()))?,
    )
    .map_err(|e| BehaviorError::Invalid(e.to_string()))?;
    request.model = validated;
    if request.strict
        && (request
            .model
            .validate()
            .iter()
            .any(|d| d.severity != kr0ki_behavior::Severity::Info)
            || request
                .model
                .edges
                .iter()
                .any(|e| e.resolution != Resolution::Resolved))
    {
        return Err(BehaviorError::Invalid(
            "strict mode rejects analysis findings".into(),
        ));
    }
    let mut graph = build_graph(&request.model)?;
    let mut classifications = request.stereotypes.clone();
    let mut annotations: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for source_node in &request.model.nodes {
        for annotation in &source_node.annotations {
            annotations
                .entry(annotation.text.as_str())
                .or_default()
                .insert(source_node.id.as_str());
        }
    }
    for (text, stereotype) in &request.annotation_stereotypes {
        let nodes = annotations.get(text.as_str()).ok_or_else(|| {
            BehaviorError::Invalid(format!(
                "ontology mapping names unknown source annotation {text}"
            ))
        })?;
        for id in nodes {
            if let Some(existing) = classifications.insert((*id).to_owned(), stereotype.clone()) {
                if existing != *stereotype {
                    return Err(BehaviorError::Invalid(format!(
                        "conflicting reviewed ontology mappings for {id}"
                    )));
                }
            }
        }
    }
    for (id, stereotype) in &classifications {
        let node = graph
            .nodes
            .iter_mut()
            .find(|n| n.id.as_str() == id)
            .ok_or_else(|| {
                BehaviorError::Invalid(format!("ontology mapping names unknown node {id}"))
            })?;
        node.stereotype = stereotype.clone();
    }
    let view = request.view.clone().unwrap_or_else(|| ViewDefinition {
        id: ElementId::new("rust-behavior"),
        name: "Rust behavior".into(),
        kind: None,
        expose: request
            .model
            .nodes
            .iter()
            .map(|n| Expose::membership(ElementId::new(&n.id)))
            .collect(),
        filter: vec![],
        rendering: None,
        satisfies: vec![],
    });
    view.validate()
        .map_err(|e| BehaviorError::View(e.to_string()))?;
    if view
        .rendering
        .as_ref()
        .is_some_and(|r| matches!(r, ufo_types::view_definition::RenderingChoice::Custom(_)))
    {
        return Err(BehaviorError::View(
            "custom rendering definitions are not supported".into(),
        ));
    }
    let lifted = crate::sysml_lift::lift_edges(&graph.edges);
    let lifted_kinds: BTreeMap<_, _> = lifted
        .iter()
        .map(|relation| (relation.source_edge_id.as_str(), relation.view_kind))
        .collect();
    let index = ModelIndex::new(
        request
            .model
            .nodes
            .iter()
            .map(|n| (ElementId::new(&n.id), element_kind(n.kind))),
        lifted.iter().map(|l| l.relation.clone()).collect(),
    );
    let selection = index.select(&view);
    if !selection.unresolved.is_empty() {
        return Err(BehaviorError::View(format!(
            "unresolved expose targets: {:?}",
            selection.unresolved
        )));
    }
    let mut chosen: BTreeSet<String> = selection
        .elements
        .iter()
        .map(|id| id.as_str().to_owned())
        .collect();
    let exposed = chosen.clone();
    let mut owner: BTreeMap<&str, &str> = BTreeMap::new();
    for e in &request.model.edges {
        if e.kind == EdgeKind::Contains {
            owner.insert(&e.to, &e.from);
        }
    }
    let by_id: BTreeMap<_, _> = request
        .model
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n))
        .collect();
    chosen.retain(|id| {
        let node = by_id[id.as_str()];
        if matches!(
            node.kind,
            NodeKind::Module
                | NodeKind::Type
                | NodeKind::Trait
                | NodeKind::Function
                | NodeKind::State
                | NodeKind::External
                | NodeKind::Dispatch
                | NodeKind::Field
                | NodeKind::AssociatedType
        ) {
            return true;
        }
        let mut current = id.as_str();
        let mut depth = 0u8;
        // Source ownership is acyclic (checked by the IR validator). Walk only
        // the requested bounded prefix instead of an unbounded body expansion.
        while let Some(parent) = owner.get(current) {
            depth = depth.saturating_add(1);
            if depth > request.expand_depth {
                return false;
            }
            if by_id
                .get(parent)
                .is_some_and(|n| n.kind == NodeKind::Function)
            {
                return true;
            }
            current = parent;
        }
        request.expand_depth > 0
    });
    let visible = |id: &str| -> String {
        let mut current = id;
        while !chosen.contains(current) {
            let Some(parent) = owner.get(current) else {
                break;
            };
            current = parent;
        }
        current.to_owned()
    };
    let mut evidence = BTreeMap::new();
    let mut rendered_edges = Vec::new();
    for e in &request.model.edges {
        // Expansion summarizes only authored content; it cannot pull an
        // unexposed or filtered element back into a view through its owner.
        if !exposed.contains(&e.from) || !exposed.contains(&e.to) {
            continue;
        }
        let (from, to) = (visible(&e.from), visible(&e.to));
        if !chosen.contains(&from) || !chosen.contains(&to) || (from == to && e.from != e.to) {
            continue;
        }
        if let Some(kind) = view.kind {
            let lifted_kind = lifted_kinds.get(e.id.as_str()).expect("total lift");
            if kind != ufo_types::view::SysmlViewKind::General && *lifted_kind != kind {
                continue;
            }
        }
        let mut display = e.clone();
        display.from = from;
        display.to = to;
        evidence.insert(e.id.clone(), e.clone());
        rendered_edges.push(display);
    }
    let names: BTreeMap<_, _> = request
        .model
        .nodes
        .iter()
        .filter(|n| chosen.contains(&n.id))
        .map(|n| (n.id.clone(), n.name.clone()))
        .collect();
    let d2 = emit_d2(&names, &rendered_edges);
    let mermaid = emit_mermaid(&names, &rendered_edges);
    let sysml = emit_sysml(&request.model, &chosen, &rendered_edges);
    sysml_v2_parser::parse(&sysml).map_err(|e| BehaviorError::Notation(format!("SysML: {e}")))?;
    let mut scxml = BTreeMap::new();
    for machine in &request.model.machines {
        // Shared-table graph identities match the local bundle adapter. Export
        // only complete tables: omitting a state or transition would change the
        // executable contract, while retaining it would reveal unselected data.
        if !machine
            .states
            .iter()
            .all(|state| chosen.contains(&format!("{}::{}", machine.id, state.id)))
            || !machine.transitions.iter().all(|transition| {
                evidence.contains_key(&kr0ki_behavior::stable_id(
                    "machine-transition",
                    &format!("{}::{}", machine.id, transition.id),
                ))
            })
        {
            continue;
        }
        let xml = machine
            .to_scxml()
            .map_err(|e| BehaviorError::Notation(e.to_string()))?;
        roxmltree::Document::parse(&xml)
            .map_err(|e| BehaviorError::Notation(format!("SCXML: {e}")))?;
        scxml.insert(machine.id.clone(), xml);
    }
    let hashed = serde_json::to_vec(&(
        RULE_VERSION,
        &request,
        &chosen,
        &d2,
        &mermaid,
        &sysml,
        &scxml,
    ))
    .map_err(|e| BehaviorError::Invalid(e.to_string()))?;
    Ok(BehaviorView {
        graph,
        view,
        selected_nodes: chosen.into_iter().collect(),
        evidence,
        d2,
        mermaid,
        sysml,
        scxml,
        validation: vec![
            NotationValidation {
                notation: "sysml".into(),
                level: "grammar".into(),
                validator: "sysml-v2-parser/0.55".into(),
            },
            NotationValidation {
                notation: "d2".into(),
                level: "structural-only".into(),
                validator: "typed-emitter; parser validation requires renderer".into(),
            },
            NotationValidation {
                notation: "mermaid".into(),
                level: "structural-only".into(),
                validator: "typed-emitter".into(),
            },
            NotationValidation {
                notation: "scxml".into(),
                level: "xml-and-transition-contract".into(),
                validator: "roxmltree/0.20 + StateMachine".into(),
            },
        ],
        content_hash: hex_digest(&hashed),
    })
}

fn edge_label(e: &kr0ki_behavior::Edge) -> String {
    let mut label = format!("{} [{}]", slug(&e.kind), slug(&e.resolution));
    if let Some(guard) = &e.guard {
        label.push_str(&format!(" / {guard}"));
    }
    label
}

fn quote(s: &str) -> String {
    serde_json::to_string(s).expect("string serialization")
}

fn emit_d2(names: &BTreeMap<String, String>, edges: &[kr0ki_behavior::Edge]) -> String {
    let mut out = String::new();
    for (id, name) in names {
        out.push_str(&format!("{}: {}\n", quote(id), quote(name)));
    }
    for e in edges {
        out.push_str(&format!(
            "{} -> {}: {}\n",
            quote(&e.from),
            quote(&e.to),
            quote(&edge_label(e))
        ));
    }
    out
}

fn emit_mermaid(names: &BTreeMap<String, String>, edges: &[kr0ki_behavior::Edge]) -> String {
    let escape = |s: &str| -> String {
        s.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == ' ' {
                    c.to_string()
                } else {
                    format!("#{};", c as u32)
                }
            })
            .collect()
    };
    let ids: BTreeMap<_, _> = names
        .keys()
        .enumerate()
        .map(|(i, id)| (id.clone(), format!("n{i}")))
        .collect();
    let mut out = String::from("flowchart TD\n");
    for (id, name) in names {
        out.push_str(&format!("    {}[\"{}\"]\n", ids[id], escape(name)));
    }
    for e in edges {
        out.push_str(&format!(
            "    {} -->|\"{}\"| {}\n",
            ids[&e.from],
            escape(&edge_label(e)),
            ids[&e.to]
        ));
    }
    out
}

fn emit_sysml(
    model: &RustBehaviorIr,
    chosen: &BTreeSet<String>,
    edges: &[kr0ki_behavior::Edge],
) -> String {
    let ids: BTreeMap<_, _> = chosen
        .iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), format!("n{i}")))
        .collect();
    let mut out = String::from("package RustBehavior {\n");
    for n in model.nodes.iter().filter(|n| chosen.contains(&n.id)) {
        let keyword = match n.kind {
            NodeKind::Module => "package",
            NodeKind::Type | NodeKind::Trait | NodeKind::External => "part def",
            NodeKind::Field => "attribute",
            NodeKind::AssociatedType => "item def",
            NodeKind::State => "state",
            NodeKind::Function => "action def",
            _ => "action",
        };
        out.push_str(&format!("    {keyword} {};\n", ids[&n.id]));
    }
    // GuardedSuccessions belong to an action body in the real SysML grammar.
    // Source expressions remain opaque evidence: a symbolic Boolean parameter
    // preserves the conditional transfer without inventing executable SysML.
    out.push_str("    action def SourceControl {\n");
    for e in edges {
        // Stable IR id in a single-line comment provides the evidence key;
        // arbitrary source labels never enter the SysML grammar.
        out.push_str(&format!(
            "        // evidence {}\n",
            hex_digest(e.id.as_bytes())
        ));
        if matches!(
            e.kind,
            EdgeKind::Flow
                | EdgeKind::Branch
                | EdgeKind::Back
                | EdgeKind::Exit
                | EdgeKind::Calls
                | EdgeKind::Transition
        ) {
            if e.guard.is_some() {
                let guard = format!("g_{}", hex_digest(e.id.as_bytes()));
                out.push_str(&format!(
                    "        in attribute {guard} : ScalarValues::Boolean;\n        succession first {} if {guard} then {};\n",
                    ids[&e.from], ids[&e.to]
                ));
            } else {
                out.push_str(&format!(
                    "        succession first {} then {};\n",
                    ids[&e.from], ids[&e.to]
                ));
            }
        }
    }
    out.push_str("    }\n");
    out.push_str("}\n");
    out
}
