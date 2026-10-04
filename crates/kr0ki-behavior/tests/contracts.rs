use kr0ki_behavior::*;
use quick_xml::{events::Event, Reader};
use std::collections::BTreeMap;

fn fixture() -> RustBehaviorIr {
    RustBehaviorIr::from_json(include_str!("fixtures/behavior.json")).unwrap()
}

fn machine() -> StateMachine {
    serde_json::from_str(include_str!("fixtures/ooda.json")).unwrap()
}

#[test]
fn transition_edges_require_two_state_endpoints() {
    let mut ir = fixture();
    ir.edges.clear();
    ir.edges.push(Edge {
        id: "typed-transition".into(),
        from: ir.nodes[0].id.clone(),
        to: ir.nodes[1].id.clone(),
        kind: EdgeKind::Transition,
        guard: None,
        resolution: Resolution::Resolved,
        anchor: ir.nodes[0].anchor.clone().unwrap(),
    });
    for (from, to) in [
        (NodeKind::Function, NodeKind::State),
        (NodeKind::State, NodeKind::Type),
        (NodeKind::Function, NodeKind::Type),
    ] {
        ir.nodes[0].kind = from;
        ir.nodes[1].kind = to;
        assert!(ir
            .validate()
            .iter()
            .any(|finding| finding.code == "edge_kind"));
        assert!(ir.ensure_valid().is_err());
    }
    ir.nodes[0].kind = NodeKind::State;
    ir.nodes[1].kind = NodeKind::State;
    ir.ensure_valid().unwrap();
    ir.edges[0].to = ir.edges[0].from.clone();
    ir.ensure_valid().unwrap();
}

fn annotated_fixture() -> RustBehaviorIr {
    let content = include_str!("fixtures/annotations.rs");
    let annotation = |path: &str, text: &str| {
        let start = content.find(text).unwrap() as u32;
        SourceAnnotation {
            path: path.into(),
            text: text.into(),
            anchor: Anchor {
                file: "src/annotations.rs".into(),
                symbol: "Worker".into(),
                start,
                end: start + text.len() as u32,
            },
        }
    };
    let mut ir = fixture();
    ir.sources = vec![SourceFile {
        path: "src/annotations.rs".into(),
        sha256: digest(content),
        content: content.into(),
    }];
    ir.nodes = vec![Node {
        id: "Worker".into(),
        name: "Worker".into(),
        kind: NodeKind::Type,
        anchor: Some(Anchor {
            file: "src/annotations.rs".into(),
            symbol: "Worker".into(),
            start: content.find("pub struct").unwrap() as u32,
            end: content.len() as u32,
        }),
        annotations: vec![
            annotation("allow", "#![allow(dead_code)]"),
            annotation("doc", "#[doc = \"café worker\"]"),
            annotation("derive", "#[derive(Debug, Clone)]"),
        ],
    }];
    ir.edges.clear();
    ir.diagnostics.clear();
    ir.machines.clear();
    ir
}

#[test]
fn annotation_bytes_are_proven_and_normalized_without_ontology_inference() {
    let first = annotated_fixture();
    first.ensure_valid().unwrap();
    for annotation in &first.nodes[0].annotations {
        assert_eq!(
            first.sources[0]
                .content
                .get(annotation.anchor.start as usize..annotation.anchor.end as usize),
            Some(annotation.text.as_str())
        );
    }
    let mut reordered = first.clone();
    reordered.nodes[0].annotations.reverse();
    let duplicate = reordered.nodes[0].annotations[0].clone();
    reordered.nodes[0].annotations.push(duplicate);
    assert_eq!(
        first.canonical_json().unwrap(),
        reordered.canonical_json().unwrap()
    );
    let reparsed = RustBehaviorIr::from_json(&first.canonical_json().unwrap()).unwrap();
    assert_eq!(reparsed.nodes[0].annotations.len(), 3);
    // Older documents omit this optional field and keep their serialized shape.
    assert!(fixture()
        .nodes
        .iter()
        .all(|node| node.annotations.is_empty()));
    assert!(!fixture()
        .canonical_json()
        .unwrap()
        .contains("\"annotations\""));
}

#[test]
fn fabricated_annotation_paths_text_and_spans_fail_closed() {
    type Corruption = (&'static str, fn(&mut SourceAnnotation));
    let cases: &[Corruption] = &[
        ("annotation_path", |annotation| annotation.path.clear()),
        ("annotation_path", |annotation| {
            annotation.path = " \t".into()
        }),
        ("annotation_source", |annotation| {
            annotation.text = "#[doc = \"invented semantics\"]".into()
        }),
        ("annotation_source", |annotation| {
            annotation.text = "doc = \"café worker\"".into()
        }),
        ("anchor_source", |annotation| {
            annotation.anchor.file = "src/absent.rs".into()
        }),
        ("anchor_span", |annotation| annotation.anchor.end = u32::MAX),
        ("anchor_span", |annotation| {
            annotation.anchor.start = annotation.anchor.end + 1
        }),
    ];
    for (code, mutate) in cases {
        let mut ir = annotated_fixture();
        mutate(&mut ir.nodes[0].annotations[0]);
        assert!(ir
            .validate()
            .iter()
            .any(|finding| finding.code == *code && finding.severity == Severity::Error));
        assert!(ir.canonical_json().is_err(), "allowed {code}");
    }
    let mut ir = annotated_fixture();
    let accent = ir.sources[0].content.find('é').unwrap() as u32;
    ir.nodes[0].annotations[0].anchor.start = accent + 1;
    assert!(ir
        .validate()
        .iter()
        .any(|finding| finding.code == "anchor_span"));
}

#[test]
fn annotation_wire_shape_rejects_unknown_fields_and_missing_evidence() {
    let mut value = serde_json::to_value(annotated_fixture()).unwrap();
    value["nodes"][0]["annotations"][0]["ontology_guess"] = serde_json::json!("role");
    assert!(RustBehaviorIr::from_json(&value.to_string()).is_err());
    value["nodes"][0]["annotations"][0]
        .as_object_mut()
        .unwrap()
        .remove("ontology_guess");
    value["nodes"][0]["annotations"][0]
        .as_object_mut()
        .unwrap()
        .remove("anchor");
    assert!(RustBehaviorIr::from_json(&value.to_string()).is_err());
}

#[test]
fn reviewed_facts_keep_resolution_and_utf8_provenance() {
    let ir = fixture();
    let findings = ir.validate();
    assert!(!findings.iter().any(|d| d.severity == Severity::Error));
    assert!(findings.iter().any(|d| d.code == "unresolved_fact"));
    assert!(findings.iter().any(|d| d.code == "unresolved_dispatch"));
    let call = ir.edges.iter().find(|e| e.id == "concrete-call").unwrap();
    assert_eq!(call.resolution, Resolution::Resolved);
    let source = &ir.sources[0].content;
    assert_eq!(
        &source[call.anchor.start as usize..call.anchor.end as usize],
        "job.work()"
    );
}

#[test]
fn normalization_is_byte_identical_across_set_order() {
    let mut first = fixture();
    let mut table = machine();
    let anchor = first.nodes[0].anchor.clone();
    for state in &mut table.states {
        state.anchor.clone_from(&anchor);
    }
    for transition in &mut table.transitions {
        transition.anchor.clone_from(&anchor);
    }
    first.machines.push(table);
    let mut second = first.clone();
    second.sources.reverse();
    second.nodes.reverse();
    second.edges.reverse();
    second.machines[0].states.reverse();
    second.machines[0].transitions.reverse();
    assert_eq!(
        first.canonical_json().unwrap(),
        second.canonical_json().unwrap()
    );
    assert_ne!(stable_id("ab", "c"), stable_id("a", "bc"));
    assert_eq!(stable_id("node", "symbol"), stable_id("node", "symbol"));
}

#[test]
fn rejects_digest_revision_path_and_cross_field_corruption() {
    type Corruption = (&'static str, fn(&mut RustBehaviorIr));
    let cases: &[Corruption] = &[
        ("source_digest", |ir| ir.sources[0].content.push('!')),
        ("source_revision", |ir| {
            ir.provenance.revision = "main".into()
        }),
        ("tree_digest", |ir| {
            ir.provenance.tree_digest = "unknown".into()
        }),
        ("source_path", |ir| {
            ir.sources[0].path = "../src/lib.rs".into()
        }),
        ("source_path", |ir| {
            ir.sources[0].path = "/home/test/src/lib.rs".into()
        }),
        ("duplicate_node", |ir| ir.nodes.push(ir.nodes[0].clone())),
        ("duplicate_edge", |ir| ir.edges.push(ir.edges[0].clone())),
        ("dangling_edge", |ir| ir.edges[0].to = "absent".into()),
        ("anchor_span", |ir| ir.edges[0].anchor.end = u32::MAX),
        ("node_provenance", |ir| ir.nodes[0].anchor = None),
        ("edge_kind", |ir| {
            let edge = ir
                .edges
                .iter_mut()
                .find(|e| e.kind == EdgeKind::Satisfies)
                .unwrap();
            edge.to = "run".into();
        }),
    ];
    for (code, mutate) in cases {
        let mut ir = fixture();
        mutate(&mut ir);
        assert!(
            ir.validate()
                .iter()
                .any(|d| d.code == *code && d.severity == Severity::Error),
            "missing {code}"
        );
        assert!(
            ir.canonical_json().is_err(),
            "allowed {code} into an artifact"
        );
    }
    let mut ir = fixture();
    ir.sources[0].content = "é".into();
    ir.sources[0].sha256 = digest("é");
    ir.nodes.truncate(1);
    ir.edges.clear();
    ir.nodes[0].anchor = Some(Anchor {
        file: "src/lib.rs".into(),
        symbol: "x".into(),
        start: 1,
        end: 2,
    });
    assert!(ir.validate().iter().any(|d| d.code == "anchor_span"));
}

#[test]
fn strict_parser_rejects_unknown_vocabulary_and_fields() {
    let serialized = fixture().canonical_json().unwrap();
    assert!(RustBehaviorIr::from_json(
        &serialized.replace("\"satisfies\"", "\"invented_relation\"")
    )
    .is_err());
    let mut value: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    value["unreviewed_fact"] = serde_json::json!(true);
    assert!(RustBehaviorIr::from_json(&value.to_string()).is_err());
    value.as_object_mut().unwrap().remove("unreviewed_fact");
    value["schema_version"] = serde_json::json!(2);
    assert!(RustBehaviorIr::from_json(&value.to_string()).is_err());
}

#[test]
fn impossible_transitions_and_containment_cycles_fail_closed() {
    let mut table = machine();
    table.transitions[0].to = "missing".into();
    assert!(table.ensure_valid().is_err());
    table = machine();
    table
        .states
        .iter_mut()
        .find(|s| s.id == "idle")
        .unwrap()
        .terminal = true;
    assert!(StateMachineRuntime::new(&table).is_err());
    table = machine();
    table.initial = "unknown".into();
    assert!(table.ensure_valid().is_err());
    table = machine();
    table.states.push(State {
        id: "isolated".into(),
        terminal: false,
        anchor: None,
    });
    assert!(table
        .validate()
        .iter()
        .any(|d| d.code == "unreachable_state" && d.severity == Severity::Warning));
    let mut ir = fixture();
    let mut edge = ir.edges[0].clone();
    edge.id = "contains-a".into();
    edge.from = "job".into();
    edge.to = "run".into();
    edge.kind = EdgeKind::Contains;
    ir.edges.push(edge.clone());
    edge.id = "contains-b".into();
    edge.from = "run".into();
    edge.to = "job".into();
    ir.edges.push(edge);
    assert!(ir.validate().iter().any(|d| d.code == "containment_cycle"));
    assert!(ir.ensure_valid().is_err());
    let mut ir = fixture();
    let mut edge = ir.edges[0].clone();
    edge.id = "contains-parent-one".into();
    edge.from = "job".into();
    edge.to = "run".into();
    edge.kind = EdgeKind::Contains;
    ir.edges.push(edge.clone());
    edge.id = "contains-parent-two".into();
    edge.from = "worker".into();
    ir.edges.push(edge);
    assert!(ir
        .validate()
        .iter()
        .any(|d| d.code == "ambiguous_ownership"));
    assert!(ir.ensure_valid().is_err());
}

#[derive(Default)]
struct Context {
    effects: Vec<String>,
    enabled: bool,
    fail_effect: bool,
}
impl RuntimeContext for Context {
    fn guard(&self, name: &str) -> Result<bool, String> {
        match name {
            "observation_available" | "goal_met" => Ok(self.enabled),
            "goal_pending" => Ok(!self.enabled),
            _ => Err("unknown guard".into()),
        }
    }
    fn effect(&mut self, name: &str) -> Result<(), String> {
        if self.fail_effect {
            return Err("effect rejected".into());
        }
        self.effects.push(name.into());
        Ok(())
    }
}

#[test]
fn shared_runtime_trace_matches_reviewed_json_table() {
    let table = machine();
    let mut runtime = StateMachineRuntime::new(&table).unwrap();
    let mut context = Context {
        enabled: true,
        ..Default::default()
    };
    for event in ["observe", "orient", "decide", "act", "finish"] {
        runtime.step(event, &mut context).unwrap();
    }
    let expected: Vec<TransitionTrace> =
        serde_json::from_str(include_str!("fixtures/ooda-trace.json")).unwrap();
    assert_eq!(runtime.trace(), expected);
    assert_eq!(
        context.effects,
        [
            "read_observation",
            "update_context",
            "choose_action",
            "apply_action"
        ]
    );
    assert!(runtime.is_terminal());
    assert!(matches!(
        runtime.step("repeat", &mut context),
        Err(RuntimeError::Terminal(_))
    ));
}

#[test]
fn guards_failures_and_ambiguity_do_not_advance_state_or_run_effect() {
    let mut table = machine();
    let mut context = Context::default();
    let mut runtime = StateMachineRuntime::new(&table).unwrap();
    assert!(matches!(
        runtime.step("missing", &mut context),
        Err(RuntimeError::NoTransition { .. })
    ));
    runtime.step("observe", &mut context).unwrap();
    assert!(matches!(
        runtime.step("orient", &mut context),
        Err(RuntimeError::NoTransition { .. })
    ));
    assert_eq!(runtime.state(), "observe");
    assert_eq!(context.effects.len(), 1);
    table.transitions[0].guard = Some("observation_available".into());
    let mut alternative = table.transitions[0].clone();
    alternative.id = "alternative".into();
    table.transitions.push(alternative);
    context.enabled = true;
    let mut runtime = StateMachineRuntime::new(&table).unwrap();
    assert!(matches!(
        runtime.step("observe", &mut context),
        Err(RuntimeError::Ambiguous { .. })
    ));
    assert_eq!(runtime.state(), "idle");
    assert!(runtime.trace().is_empty());
    assert_eq!(context.effects.len(), 1);
    table.transitions.pop();
    context.fail_effect = true;
    let mut runtime = StateMachineRuntime::new(&table).unwrap();
    assert!(matches!(
        runtime.step("observe", &mut context),
        Err(RuntimeError::Effect { .. })
    ));
    assert_eq!(runtime.state(), "idle");
    table.transitions[0].guard = Some("unknown".into());
    let mut runtime = StateMachineRuntime::new(&table).unwrap();
    assert!(matches!(
        runtime.step("observe", &mut context),
        Err(RuntimeError::Guard { .. })
    ));
    assert_eq!(runtime.state(), "idle");
}

#[test]
fn scxml_xml_parser_preserves_every_table_transition_and_escapes_labels() {
    let mut table = machine();
    table.name = "<machine> & \"label\" 'single'\nline\ttab\rreturn".into();
    table.transitions[0].effect = Some("effect<&\"'\n\t\r".into());
    let first = table.to_scxml().unwrap();
    assert!(!first.contains("<script>"));
    table.states.reverse();
    table.transitions.reverse();
    assert_eq!(first, table.to_scxml().unwrap());
    let mut reader = Reader::from_str(&first);
    let mut transitions = BTreeMap::new();
    let mut targets = Vec::new();
    let mut states = Vec::new();
    loop {
        match reader.read_event().unwrap() {
            Event::Start(element) | Event::Empty(element) => {
                let attrs: BTreeMap<String, String> = element
                    .attributes()
                    .map(|attr| {
                        let attr = attr.unwrap();
                        (
                            String::from_utf8(attr.key.as_ref().to_vec()).unwrap(),
                            attr.unescape_value().unwrap().into_owned(),
                        )
                    })
                    .collect();
                match element.name().as_ref() {
                    b"state" | b"final" => states.push(attrs["id"].clone()),
                    b"transition" => {
                        targets.push(attrs["target"].clone());
                        transitions.insert(
                            attrs["kr0ki:id"].clone(),
                            (
                                attrs["event"].clone(),
                                attrs["target"].clone(),
                                attrs.get("cond").cloned(),
                            ),
                        );
                    }
                    b"kr0ki:effect" => assert!(table
                        .transitions
                        .iter()
                        .any(|t| t.effect.as_ref() == Some(&attrs["name"]))),
                    b"scxml" => assert_eq!(attrs["name"], table.name),
                    _ => (),
                }
            }
            Event::Eof => break,
            _ => (),
        }
    }
    assert_eq!(transitions.len(), table.transitions.len());
    assert!(targets.iter().all(|t| states.contains(t)));
    for transition in &table.transitions {
        assert_eq!(
            transitions[&transition.id],
            (
                transition.event.clone(),
                format!("s_{}", digest(&transition.to)),
                transition.guard.clone()
            )
        );
    }
    table.name.push('\u{0}');
    assert!(table.to_scxml().is_err());
}

#[test]
fn schema_defines_strict_versioned_wire_vocabulary() {
    let schema = json_schema();
    assert_eq!(
        schema["properties"]["schema_version"]["const"],
        SCHEMA_VERSION
    );
    for definition in [
        "Provenance",
        "SourceFile",
        "Node",
        "Edge",
        "Diagnostic",
        "StateMachine",
        "State",
        "Transition",
        "Anchor",
        "SourceAnnotation",
    ] {
        assert_eq!(schema["$defs"][definition]["additionalProperties"], false);
    }
    assert!(!schema["$defs"]["Node"]["required"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("annotations")));
    assert_eq!(
        schema["$defs"]["Node"]["properties"]["annotations"]["items"]["$ref"],
        "#/$defs/SourceAnnotation"
    );
    for kind in [
        NodeKind::Module,
        NodeKind::Type,
        NodeKind::Field,
        NodeKind::AssociatedType,
        NodeKind::Trait,
        NodeKind::Function,
        NodeKind::Action,
        NodeKind::Decision,
        NodeKind::Merge,
        NodeKind::Loop,
        NodeKind::Exit,
        NodeKind::Dispatch,
        NodeKind::State,
        NodeKind::External,
    ] {
        assert!(schema["$defs"]["NodeKind"]["enum"]
            .as_array()
            .unwrap()
            .contains(&serde_json::to_value(kind).unwrap()));
    }
}
