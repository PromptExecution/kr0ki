//! End-to-end contracts for compiler evidence, authored views and local bundles.
use kr0ki_behavior::{
    Anchor, Edge, EdgeKind, Node, NodeKind, Resolution, RuntimeContext, RustBehaviorIr, SourceFile,
    StateMachine, StateMachineRuntime, TransitionTrace,
};
use kr0ki_core::rust_behavior::{build_graph, prepare, BehaviorRequest};
use std::{collections::BTreeMap, path::PathBuf, process::Command};
use ufo_types::{
    ontology::SourceAnchor,
    stereotype::UfoStereotype,
    sysml_model::{ElementId, ElementKind},
    view::SysmlViewKind,
    view_definition::{Expose, RenderingChoice, ViewDefinition, ViewFilter},
};

fn fixture() -> RustBehaviorIr {
    RustBehaviorIr::from_json(include_str!(
        "../../kr0ki-behavior/tests/fixtures/behavior.json"
    ))
    .unwrap()
}

fn request(model: RustBehaviorIr) -> BehaviorRequest {
    BehaviorRequest {
        model,
        view: None,
        expand_depth: 0,
        strict: false,
        stereotypes: BTreeMap::new(),
    }
}

fn authored(expose: Vec<Expose>, filter: Vec<ViewFilter>) -> ViewDefinition {
    ViewDefinition {
        id: ElementId::new("reviewed-view"),
        name: "Reviewed view".into(),
        kind: None,
        expose,
        filter,
        rendering: None,
        satisfies: vec![],
    }
}

#[test]
fn reordered_compiler_facts_generate_identical_artifacts_and_cache_hashes() {
    let first = fixture();
    let mut second = first.clone();
    second.nodes.reverse();
    second.edges.reverse();
    second.sources.reverse();
    let a = prepare(request(first)).unwrap();
    let b = prepare(request(second)).unwrap();
    assert_eq!(a.content_hash, b.content_hash);
    assert_eq!(a.d2, b.d2);
    assert_eq!(a.mermaid, b.mermaid);
    assert_eq!(a.sysml, b.sysml);
    assert_eq!(a.selected_nodes, b.selected_nodes);
    assert_eq!(
        serde_json::to_value(&a.graph).unwrap(),
        serde_json::to_value(&b.graph).unwrap()
    );
    assert_eq!(a.evidence, b.evidence);
}

#[test]
fn every_rendered_edge_retains_full_source_evidence_and_revision() {
    let ir = fixture();
    let view = prepare(request(ir.clone())).unwrap();
    assert!(!view.evidence.is_empty());
    for (id, evidence) in &view.evidence {
        assert_eq!(
            evidence,
            ir.edges.iter().find(|edge| &edge.id == id).unwrap()
        );
        let graph_edge = view.graph.edges.iter().find(|edge| &edge.id == id).unwrap();
        assert!(graph_edge.provenance.iter().any(|anchor| matches!(anchor,
            SourceAnchor::Vcs { commit, path: Some(path), .. } if commit == &ir.provenance.revision && path == &evidence.anchor.file)));
        assert!(graph_edge.provenance.iter().any(|anchor| matches!(anchor,
            SourceAnchor::Other(json) if serde_json::from_str::<Edge>(json).ok().as_ref() == Some(evidence))));
        let source = ir
            .sources
            .iter()
            .find(|source| source.path == evidence.anchor.file)
            .unwrap();
        assert!(source
            .content
            .get(evidence.anchor.start as usize..evidence.anchor.end as usize)
            .is_some());
    }
    let dynamic = &view.evidence["dynamic-call"];
    assert_eq!(dynamic.resolution, Resolution::Unresolved);
    assert!(view.selected_nodes.contains(&"dispatch".into()));
    assert!(view.d2.contains("unresolved"));
}

#[test]
fn authored_recursive_expose_and_upstream_kind_filter_select_the_model() {
    let mut ir = fixture();
    let anchor = ir.nodes[0].anchor.clone().unwrap();
    ir.nodes.push(Node {
        id: "namespace".into(),
        name: "Namespace".into(),
        kind: NodeKind::Module,
        anchor: Some(anchor.clone()),
    });
    for (idx, member) in ["job", "worker", "run", "dynamic"].into_iter().enumerate() {
        ir.edges.push(Edge {
            id: format!("namespace-member-{idx}"),
            from: "namespace".into(),
            to: member.into(),
            kind: EdgeKind::Contains,
            guard: None,
            resolution: Resolution::Resolved,
            anchor: anchor.clone(),
        });
    }
    let mut req = request(ir);
    req.view = Some(authored(
        vec![Expose::all_under(ElementId::new("namespace"))],
        vec![ViewFilter::of([ElementKind::ActionDefinition])],
    ));
    let selected = prepare(req).unwrap();
    assert_eq!(selected.selected_nodes, ["dynamic", "run"]);
    assert!(selected.evidence.is_empty());
    assert!(!selected.d2.contains("Namespace"));
}

#[test]
fn authored_view_does_not_reintroduce_unexposed_edges_through_collapsing() {
    let mut ir = fixture();
    let anchor = ir.nodes[0].anchor.clone().unwrap();
    ir.edges.push(Edge {
        id: "method-membership".into(),
        from: "job".into(),
        to: "job-work".into(),
        kind: EdgeKind::Contains,
        guard: None,
        resolution: Resolution::Resolved,
        anchor: anchor.clone(),
    });
    ir.edges.push(Edge {
        id: "hidden-call".into(),
        from: "job-work".into(),
        to: "run".into(),
        kind: EdgeKind::Calls,
        guard: None,
        resolution: Resolution::Resolved,
        anchor,
    });
    let mut req = request(ir);
    req.view = Some(authored(
        vec![
            Expose::membership(ElementId::new("job")),
            Expose::membership(ElementId::new("run")),
        ],
        vec![],
    ));
    let view = prepare(req).unwrap();
    assert_eq!(view.selected_nodes, ["job", "run"]);
    assert!(!view.evidence.contains_key("hidden-call"));
    assert!(!view.evidence.contains_key("method-membership"));
}

#[test]
fn only_explicit_reviewed_stereotypes_change_semantic_classification() {
    let original = prepare(request(fixture())).unwrap();
    let mut req = request(fixture());
    let reviewed = UfoStereotype::Abstract("ReviewedDomainJob".into());
    req.stereotypes.insert("job".into(), reviewed.clone());
    let mapped = prepare(req).unwrap();
    assert_eq!(
        mapped
            .graph
            .nodes
            .iter()
            .find(|node| node.id.as_str() == "job")
            .unwrap()
            .stereotype,
        reviewed
    );
    assert_eq!(
        mapped
            .graph
            .nodes
            .iter()
            .find(|node| node.id.as_str() == "worker")
            .unwrap(),
        original
            .graph
            .nodes
            .iter()
            .find(|node| node.id.as_str() == "worker")
            .unwrap()
    );
    assert_ne!(mapped.content_hash, original.content_hash);
    let mut req = request(fixture());
    req.stereotypes
        .insert("missing".into(), UfoStereotype::Abstract("Invalid".into()));
    assert!(prepare(req).is_err());
}

#[test]
fn unknown_exposes_empty_filters_custom_renderers_and_depth_overflow_are_rejected() {
    let mut req = request(fixture());
    req.view = Some(authored(
        vec![Expose::membership(ElementId::new("missing"))],
        vec![],
    ));
    assert!(prepare(req)
        .unwrap_err()
        .to_string()
        .contains("unresolved expose"));
    let mut req = request(fixture());
    req.view = Some(authored(vec![], vec![ViewFilter { any_of: vec![] }]));
    assert!(prepare(req).is_err());
    let mut req = request(fixture());
    let mut view = authored(vec![], vec![]);
    view.rendering = Some(RenderingChoice::Custom(ElementId::new("unsupported")));
    req.view = Some(view);
    assert!(prepare(req).is_err());
    let mut req = request(fixture());
    req.expand_depth = 9;
    assert!(prepare(req).is_err());
}

fn nested_fixture() -> RustBehaviorIr {
    let mut model = fixture();
    let anchor: Anchor = model
        .nodes
        .iter()
        .find(|node| node.id == "run")
        .unwrap()
        .anchor
        .clone()
        .unwrap();
    for (id, kind) in [
        ("outer-loop", NodeKind::Loop),
        ("nested-action", NodeKind::Action),
        ("deep-action", NodeKind::Action),
    ] {
        model.nodes.push(Node {
            id: id.into(),
            name: id.into(),
            kind,
            anchor: Some(anchor.clone()),
        });
    }
    for (idx, (from, to, kind)) in [
        ("run", "outer-loop", EdgeKind::Contains),
        ("outer-loop", "nested-action", EdgeKind::Contains),
        ("nested-action", "deep-action", EdgeKind::Contains),
        ("outer-loop", "deep-action", EdgeKind::Exit),
        ("deep-action", "outer-loop", EdgeKind::Back),
    ]
    .into_iter()
    .enumerate()
    {
        model.edges.push(Edge {
            id: format!("nested-edge-{idx}"),
            from: from.into(),
            to: to.into(),
            kind,
            guard: None,
            resolution: Resolution::Resolved,
            anchor: anchor.clone(),
        });
    }
    model
}

#[test]
fn bounded_expansion_changes_visible_depth_and_cache_fingerprint() {
    let model = nested_fixture();
    let summary = prepare(request(model.clone())).unwrap();
    assert!(!summary.selected_nodes.contains(&"outer-loop".into()));
    let mut req = request(model.clone());
    req.expand_depth = 1;
    let one = prepare(req).unwrap();
    assert!(one.selected_nodes.contains(&"outer-loop".into()));
    assert!(!one.selected_nodes.contains(&"nested-action".into()));
    let mut req = request(model);
    req.expand_depth = 2;
    let two = prepare(req).unwrap();
    assert!(two.selected_nodes.contains(&"nested-action".into()));
    assert!(!two.selected_nodes.contains(&"deep-action".into()));
    assert_ne!(summary.content_hash, one.content_hash);
    assert_ne!(one.content_hash, two.content_hash);
}

#[test]
fn malicious_labels_remain_data_and_sysml_passes_the_real_parser() {
    let attack = "\"] --> injected; <script>alert('x')</script>\npackage Breakout { /*";
    let mut model = fixture();
    model
        .nodes
        .iter_mut()
        .find(|node| node.id == "job")
        .unwrap()
        .name = attack.into();
    model
        .edges
        .iter_mut()
        .find(|edge| edge.id == "concrete-call")
        .unwrap()
        .guard = Some(attack.into());
    let view = prepare(request(model)).unwrap();
    assert!(view.d2.contains(&serde_json::to_string(attack).unwrap()));
    assert!(!view.mermaid.contains(attack));
    assert!(view.mermaid.contains("#60;script#62;"));
    assert!(!view.sysml.contains(attack));
    sysml_v2_parser::parse(&view.sysml).unwrap();
    assert!(view
        .validation
        .iter()
        .any(|v| v.notation == "sysml" && v.level == "grammar"));
    assert!(view
        .validation
        .iter()
        .any(|v| v.notation == "mermaid" && v.level == "structural-only"));
}

#[test]
fn guarded_sysml_successions_use_symbolic_conditions_and_retain_source_guard() {
    let mut model = fixture();
    let source_guard = "value < 7 && callback(\"*/ injected /*\")";
    model
        .edges
        .iter_mut()
        .find(|edge| edge.id == "concrete-call")
        .unwrap()
        .guard = Some(source_guard.into());
    let view = prepare(request(model)).unwrap();
    let guard_id = format!(
        "g_{}",
        kr0ki_core::rust_behavior::hex_digest(b"concrete-call")
    );
    assert!(view
        .sysml
        .contains(&format!("in attribute {guard_id} : ScalarValues::Boolean;")));
    assert!(view
        .sysml
        .lines()
        .any(|line| line.contains("succession first")
            && line.contains(&format!(" if {guard_id} then "))));
    assert!(!view.sysml.contains(source_guard));
    assert_eq!(
        view.evidence["concrete-call"].guard.as_deref(),
        Some(source_guard)
    );
    let parsed = sysml_v2_parser::parse(&view.sysml).unwrap();
    use sysml_v2_parser::ast::{
        ActionDefBody, ActionDefBodyElement, PackageBody, PackageBodyElement, RootElement,
    };
    let mut guards = Vec::new();
    for root in &parsed.root.elements {
        let RootElement::Package(package) = &root.value else {
            continue;
        };
        let PackageBody::Brace { elements, .. } = &package.value.body else {
            continue;
        };
        for member in elements {
            let PackageBodyElement::ActionDef(action) = &member.value else {
                continue;
            };
            let ActionDefBody::Brace { elements, .. } = &action.value.body else {
                continue;
            };
            for member in elements {
                if let ActionDefBodyElement::GuardedSuccession(succession) = &member.value {
                    guards.push(parsed.source.slice(&succession.value.guard.span).unwrap());
                }
            }
        }
    }
    assert!(
        guards.contains(&guard_id.as_str()),
        "real parser did not retain guarded succession AST"
    );
}

fn machine_fixture() -> RustBehaviorIr {
    let mut model = fixture();
    let content = include_str!("../../kr0ki-behavior/tests/fixtures/ooda.json");
    let mut machine: StateMachine = serde_json::from_str(content).unwrap();
    let anchor = Anchor {
        file: "state-machines/ooda.json".into(),
        symbol: machine.id.clone(),
        start: 0,
        end: content.len() as u32,
    };
    model.sources.push(SourceFile {
        path: anchor.file.clone(),
        sha256: kr0ki_behavior::digest(content),
        content: content.into(),
    });
    model.nodes.push(Node {
        id: machine.id.clone(),
        name: machine.name.clone(),
        kind: NodeKind::Module,
        anchor: Some(anchor.clone()),
    });
    for state in &mut machine.states {
        state.anchor = Some(anchor.clone());
        let id = format!("{}::{}", machine.id, state.id);
        model.nodes.push(Node {
            id: id.clone(),
            name: state.id.clone(),
            kind: NodeKind::State,
            anchor: Some(anchor.clone()),
        });
        model.edges.push(Edge {
            id: kr0ki_behavior::stable_id("machine-contains", &id),
            from: machine.id.clone(),
            to: id,
            kind: EdgeKind::Contains,
            guard: None,
            resolution: Resolution::Resolved,
            anchor: anchor.clone(),
        });
    }
    for transition in &mut machine.transitions {
        transition.anchor = Some(anchor.clone());
        model.edges.push(Edge {
            id: kr0ki_behavior::stable_id(
                "machine-transition",
                &format!("{}::{}", machine.id, transition.id),
            ),
            from: format!("{}::{}", machine.id, transition.from),
            to: format!("{}::{}", machine.id, transition.to),
            kind: EdgeKind::Transition,
            guard: Some(format!(
                "event={} guard={} effect={}",
                transition.event,
                transition.guard.as_deref().unwrap_or("true"),
                transition.effect.as_deref().unwrap_or("none")
            )),
            resolution: Resolution::Resolved,
            anchor: anchor.clone(),
        });
    }
    model.machines.push(machine);
    model
}

#[test]
fn scxml_exports_only_complete_machine_content_selected_by_authored_view() {
    let model = machine_fixture();
    assert!(prepare(request(model.clone()))
        .unwrap()
        .scxml
        .contains_key("ooda"));
    let mut req = request(model.clone());
    req.view = Some(authored(
        vec![Expose::membership(ElementId::new("run"))],
        vec![],
    ));
    assert!(prepare(req).unwrap().scxml.is_empty());
    let mut req = request(model.clone());
    req.view = Some(authored(
        vec![Expose::membership(ElementId::new("ooda::idle"))],
        vec![],
    ));
    let partial = prepare(req).unwrap();
    assert_eq!(partial.selected_nodes, ["ooda::idle"]);
    assert!(partial.scxml.is_empty());
    let mut req = request(model);
    req.view = Some(authored(
        vec![Expose::all_under(ElementId::new("ooda"))],
        vec![],
    ));
    let complete = prepare(req).unwrap();
    assert!(complete.scxml.contains_key("ooda"));
    assert_eq!(complete.selected_nodes.len(), 6);
    let xml = roxmltree::Document::parse(&complete.scxml["ooda"]).unwrap();
    for state in xml
        .descendants()
        .filter(|node| node.has_tag_name("state") || node.has_tag_name("final"))
    {
        let state_id = state.attribute(("urn:kr0ki:behavior:v1", "id")).unwrap();
        assert!(complete
            .selected_nodes
            .contains(&format!("ooda::{state_id}")));
    }
}

#[test]
fn view_kind_filters_edge_content_and_prevents_hidden_machine_transitions() {
    let mut req = request(fixture());
    let mut view = authored(
        req.model
            .nodes
            .iter()
            .map(|node| Expose::membership(ElementId::new(&node.id)))
            .collect(),
        vec![],
    );
    view.kind = Some(SysmlViewKind::ActionFlow);
    req.view = Some(view);
    let actions = prepare(req).unwrap();
    assert!(!actions.evidence.is_empty());
    assert!(actions
        .evidence
        .values()
        .all(|edge| edge.kind == EdgeKind::Calls));
    let model = machine_fixture();
    let mut req = request(model.clone());
    let mut view = authored(vec![Expose::all_under(ElementId::new("ooda"))], vec![]);
    view.kind = Some(SysmlViewKind::Tree);
    req.view = Some(view);
    let tree = prepare(req).unwrap();
    assert_eq!(tree.selected_nodes.len(), 6);
    assert!(tree.scxml.is_empty());
    let mut req = request(model);
    let mut view = authored(vec![Expose::all_under(ElementId::new("ooda"))], vec![]);
    view.kind = Some(SysmlViewKind::StateTransition);
    req.view = Some(view);
    let transitions = prepare(req).unwrap();
    assert!(transitions.scxml.contains_key("ooda"));
    assert_eq!(transitions.evidence.len(), 6);
}

#[test]
fn strict_findings_and_invalid_public_graph_inputs_fail_closed() {
    let mut req = request(fixture());
    req.strict = true;
    assert!(prepare(req).is_err());
    let mut model = fixture();
    model
        .edges
        .retain(|edge| edge.resolution == Resolution::Resolved);
    model.nodes.retain(|node| node.kind != NodeKind::Dispatch);
    let mut req = request(model);
    req.strict = true;
    prepare(req).unwrap();
    let mut invalid = fixture();
    invalid.edges[0].anchor.start = u32::MAX;
    let result = std::panic::catch_unwind(|| build_graph(&invalid));
    assert!(
        result.is_ok(),
        "public graph builder panicked on invalid source offsets"
    );
    assert!(result.unwrap().is_err());
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "kr0ki-behavior-test-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct FixtureContext;
impl RuntimeContext for FixtureContext {
    fn guard(&self, name: &str) -> Result<bool, String> {
        match name {
            "observation_available" | "goal_met" => Ok(true),
            "goal_pending" => Ok(false),
            _ => Err(format!("unknown {name}")),
        }
    }
    fn effect(&mut self, name: &str) -> Result<(), String> {
        if [
            "read_observation",
            "update_context",
            "choose_action",
            "apply_action",
        ]
        .contains(&name)
        {
            Ok(())
        } else {
            Err(format!("unknown {name}"))
        }
    }
}

#[test]
fn cli_bundle_machine_graph_scxml_and_runtime_trace_share_one_table() {
    let scratch = Scratch::new();
    let input = scratch.0.join("input.json");
    std::fs::write(&input, fixture().canonical_json().unwrap()).unwrap();
    let table_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../kr0ki-behavior/tests/fixtures/ooda.json");
    let output = scratch.0.join("bundle");
    let run = Command::new(env!("CARGO_BIN_EXE_kr0ki-docgen"))
        .args(["bundle", "--input"])
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .arg("--state-machine")
        .arg(&table_path)
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let model =
        RustBehaviorIr::from_json(&std::fs::read_to_string(output.join("ir.json")).unwrap())
            .unwrap();
    let table = &model.machines[0];
    assert!(table.states.iter().all(|state| state.anchor.is_some()));
    assert!(table
        .transitions
        .iter()
        .all(|transition| transition.anchor.is_some()));
    assert_eq!(
        model
            .edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Transition)
            .count(),
        table.transitions.len()
    );
    let mut runtime = StateMachineRuntime::new(table).unwrap();
    let mut context = FixtureContext;
    for event in ["observe", "orient", "decide", "act", "finish"] {
        runtime.step(event, &mut context).unwrap();
    }
    let expected: Vec<TransitionTrace> = serde_json::from_str(include_str!(
        "../../kr0ki-behavior/tests/fixtures/ooda-trace.json"
    ))
    .unwrap();
    assert_eq!(runtime.trace(), expected);
    let scxml_path = output.join(format!(
        "machine-{}.scxml",
        kr0ki_behavior::digest(&table.id)
    ));
    let xml = std::fs::read_to_string(scxml_path).unwrap();
    let document = roxmltree::Document::parse(&xml).unwrap();
    let transitions: BTreeMap<_, _> = document
        .descendants()
        .filter(|node| node.has_tag_name("transition"))
        .map(|node| {
            (
                node.attribute(("urn:kr0ki:behavior:v1", "id")).unwrap(),
                node.attribute("event").unwrap(),
            )
        })
        .collect();
    for trace in runtime.trace() {
        assert_eq!(transitions[trace.transition.as_str()], trace.event);
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output.join("semantic-manifest.json")).unwrap())
            .unwrap();
    for (name, hash) in manifest["files"].as_object().unwrap() {
        let bytes = std::fs::read(output.join(name)).unwrap();
        assert_eq!(
            hash.as_str().unwrap(),
            kr0ki_core::rust_behavior::hex_digest(&bytes)
        );
    }
    let rerun = Command::new(env!("CARGO_BIN_EXE_kr0ki-docgen"))
        .args(["bundle", "--input"])
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .arg("--state-machine")
        .arg(&table_path)
        .output()
        .unwrap();
    assert!(
        !rerun.status.success(),
        "bundle silently reused a nonempty directory"
    );
    let preserved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output.join("semantic-manifest.json")).unwrap())
            .unwrap();
    assert_eq!(preserved, manifest);
}
