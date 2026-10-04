use kr0ki_behavior::*;

fn graph(size: usize, parent: impl Fn(usize) -> usize) -> RustBehaviorIr {
    let mut ir = RustBehaviorIr::from_json(include_str!("fixtures/behavior.json")).unwrap();
    let anchor = ir.nodes[0].anchor.clone().unwrap();
    ir.nodes = (0..size)
        .map(|i| Node {
            id: format!("n{i:08}"),
            name: format!("node {i}"),
            kind: NodeKind::Module,
            anchor: Some(anchor.clone()),
            annotations: vec![],
        })
        .collect();
    ir.edges = (1..size)
        .map(|i| Edge {
            id: format!("e{i}"),
            from: ir.nodes[parent(i)].id.clone(),
            to: ir.nodes[i].id.clone(),
            kind: EdgeKind::Contains,
            guard: None,
            resolution: Resolution::Resolved,
            anchor: anchor.clone(),
        })
        .collect();
    ir.diagnostics.clear();
    ir.machines.clear();
    ir
}

fn add_edge(ir: &mut RustBehaviorIr, from: usize, to: usize) {
    ir.edges.push(Edge {
        id: format!("extra-{}", ir.edges.len()),
        from: ir.nodes[from].id.clone(),
        to: ir.nodes[to].id.clone(),
        kind: EdgeKind::Contains,
        guard: None,
        resolution: Resolution::Resolved,
        anchor: ir.nodes[from].anchor.clone().unwrap(),
    });
}

#[test]
fn deep_and_branching_valid_graphs_use_no_recursive_traversal() {
    // The test thread's small default stack cannot hold this many DFS frames.
    for ir in [
        graph(30_000, |i| i - 1),
        graph(30_000, |i| (i - 1) / 2),
        graph(30_000, |_| 0),
    ] {
        assert!(ir.validate().is_empty());
    }
}

#[test]
fn disconnected_cycles_report_only_cyclic_nodes_deterministically() {
    let mut ir = graph(8, |_| 0);
    ir.edges.clear();
    // A valid component, an upstream node, two cycles and a downstream leaf.
    for (from, to) in [(0, 1), (2, 3), (3, 4), (4, 3), (4, 5), (6, 6)] {
        add_edge(&mut ir, from, to);
    }
    let findings = ir.validate();
    let messages: Vec<_> = findings
        .iter()
        .filter(|d| d.code == "containment_cycle")
        .map(|d| d.message.as_str())
        .collect();
    assert_eq!(
        messages,
        [
            "containment cycle through n00000003",
            "containment cycle through n00000004",
            "containment cycle through n00000006"
        ]
    );
    assert!(ir.ensure_valid().is_err());
    ir.nodes.reverse();
    ir.edges.reverse();
    assert_eq!(findings, ir.validate());
}

#[test]
fn repeated_same_owner_is_allowed_but_multiple_owners_are_rejected() {
    let mut ir = graph(4, |_| 0);
    add_edge(&mut ir, 0, 1);
    assert!(ir.validate().is_empty());
    add_edge(&mut ir, 2, 1);
    assert!(ir
        .validate()
        .iter()
        .any(|d| d.code == "ambiguous_ownership"));
    assert!(ir.ensure_valid().is_err());
}

#[test]
#[ignore = "manual scaling evidence; no timing assertions"]
fn chain_versus_star_benchmark() {
    for size in [1_000, 2_000, 4_000, 8_000, 16_000, 32_000] {
        for (shape, ir) in [
            ("chain", graph(size, |i| i - 1)),
            ("star", graph(size, |_| 0)),
        ] {
            let started = std::time::Instant::now();
            for _ in 0..5 {
                assert!(ir.validate().is_empty());
            }
            println!(
                "{shape},{size},{:.3}ms",
                started.elapsed().as_secs_f64() * 200.0
            );
        }
    }
}
