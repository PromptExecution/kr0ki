//! KR-A02 acceptance case (VC-A02).
//!
//! "The traceability adapter shall resolve requirement links against a specified model
//! revision."
//!
//! Acceptance: link KR-A01 to its adapter and test; report a deliberately dangling reference.

use std::collections::BTreeSet;
use std::path::Path;

use kr0ki_core::assurance_baseline::load_baseline;
use kr0ki_core::assurance_trace::{
    resolve_links, DirResolver, LinkClass, LinkResolution, LinkStatus, ModelRevisionIndex,
};
use ufo_types::mbse::assurance::{analyze, CurrentRevisions, GapKind};
use ufo_types::mbse::requirements::{
    RelationAuthority, Requirement, RequirementGraph, RequirementRelation, RequirementRelationKind,
};

const BASELINE_TOML: &str = include_str!("../../../docs/assurance/kr0ki.assurance.toml");
const MODEL_SYSML: &str = include_str!("../../../docs/assurance/kr0ki-assurance.sysml");

fn graph() -> RequirementGraph {
    load_baseline(
        BASELINE_TOML,
        "docs/assurance/kr0ki.assurance.toml",
        "rev-1",
        None,
    )
    .expect("the checked-in baseline must load")
    .graph
}

fn repo() -> DirResolver {
    DirResolver::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

fn resolve(graph: &RequirementGraph, model: &str) -> LinkResolution {
    resolve_links(graph, &ModelRevisionIndex::from_sysml_text(model), &repo())
}

fn link<'a>(
    res: &'a LinkResolution,
    requirement: &str,
    class: LinkClass,
    locator: &str,
) -> &'a LinkStatus {
    &res.links
        .iter()
        .find(|l| l.requirement_id == requirement && l.class == class && l.locator == locator)
        .unwrap_or_else(|| panic!("{requirement} has no {class:?} link to {locator}"))
        .status
}

#[test]
fn kr_a01_links_to_its_element_its_adapter_and_its_test() {
    let res = resolve(&graph(), MODEL_SYSML);
    let a01: Vec<_> = res.for_requirement("KR-A01").collect();
    assert!(
        a01.iter().all(|l| l.status == LinkStatus::Resolved),
        "{a01:#?}"
    );
    assert_eq!(
        link(
            &res,
            "KR-A01",
            LinkClass::Element,
            "KrOKiAssurance::ModelService"
        ),
        &LinkStatus::Resolved
    );
    // The adapter: both files the control records.
    for path in [
        "crates/kr0ki-core/src/reqif_export.rs",
        "crates/kr0ki-core/src/reqif_roundtrip.rs",
    ] {
        assert_eq!(
            link(&res, "KR-A01", LinkClass::Implementation, path),
            &LinkStatus::Resolved
        );
    }
    // The test: the case's declared `cargo test --test assurance_a01` target.
    assert_eq!(
        link(
            &res,
            "KR-A01",
            LinkClass::VerificationTarget,
            "crates/kr0ki-core/tests/assurance_a01.rs"
        ),
        &LinkStatus::Resolved
    );
}

#[test]
fn kr_a02_links_to_itself_too() {
    let res = resolve(&graph(), MODEL_SYSML);
    assert_eq!(
        link(
            &res,
            "KR-A02",
            LinkClass::Implementation,
            "crates/kr0ki-core/src/assurance_trace.rs"
        ),
        &LinkStatus::Resolved
    );
    assert_eq!(
        link(
            &res,
            "KR-A02",
            LinkClass::VerificationTarget,
            "crates/kr0ki-core/tests/assurance_a02.rs"
        ),
        &LinkStatus::Resolved
    );
}

/// Adds an element (`Ghost`) that the model does not contain and records a non-existent
/// implementation path on KR-A01's control: two deliberately dangling references.
fn with_dangling_references(mut graph: RequirementGraph) -> RequirementGraph {
    let template: Requirement = graph
        .requirements
        .iter()
        .find(|n| n.id == "KrOKiAssurance::ModelService")
        .unwrap()
        .clone();
    let mut ghost = template;
    ghost.id = "KrOKiAssurance::Ghost".into();
    ghost.title = "Ghost".into();
    ghost
        .attributes
        .insert("qualified_name".into(), "KrOKiAssurance::Ghost".into());
    graph.requirements.push(ghost);
    let like = graph
        .relations
        .iter()
        .find(|r| r.kind == RequirementRelationKind::Satisfies)
        .unwrap()
        .clone();
    graph.relations.push(RequirementRelation {
        id: "satisfies:ghost->KR-A01".into(),
        source: "KrOKiAssurance::Ghost".into(),
        target: "KR-A01".into(),
        kind: RequirementRelationKind::Satisfies,
        authority: RelationAuthority::Asserted,
        provenance: like.provenance,
        promotion: None,
    });
    let control = graph
        .requirements
        .iter_mut()
        .find(|n| n.id == "CTL-A01")
        .unwrap();
    let implementation = control.attributes.get_mut("implementation").unwrap();
    implementation.push_str("; crates/kr0ki-core/src/does_not_exist.rs");
    graph
        .validate()
        .expect("the mutated graph must itself be valid");
    graph
}

#[test]
fn a_deliberately_dangling_reference_is_reported_with_its_revision_and_reason() {
    let res = resolve(&with_dangling_references(graph()), MODEL_SYSML);

    let element = link(&res, "KR-A01", LinkClass::Element, "KrOKiAssurance::Ghost");
    match element {
        LinkStatus::Dangling { reason } => {
            assert!(reason.contains("KrOKiAssurance::Ghost"), "{reason}");
            assert!(
                reason.contains(&res.revision),
                "the reason must name the revision: {reason}"
            );
        }
        other => panic!("expected dangling, got {other:?}"),
    }
    let file = link(
        &res,
        "KR-A01",
        LinkClass::Implementation,
        "crates/kr0ki-core/src/does_not_exist.rs",
    );
    assert!(
        matches!(file, LinkStatus::Dangling { reason } if reason.contains("does_not_exist.rs"))
    );

    // Exactly those two are new; everything KR-A01 really links to still resolves.
    let dangling: BTreeSet<(String, String)> = res
        .dangling()
        .filter(|l| l.requirement_id == "KR-A01")
        .map(|l| (l.requirement_id.clone(), l.locator.clone()))
        .collect();
    assert_eq!(
        dangling,
        BTreeSet::from([
            ("KR-A01".to_string(), "KrOKiAssurance::Ghost".to_string()),
            (
                "KR-A01".to_string(),
                "crates/kr0ki-core/src/does_not_exist.rs".to_string()
            ),
        ])
    );
}

#[test]
fn resolution_is_against_the_named_revision_and_changes_with_it() {
    let graph = graph();
    let r1 = resolve(&graph, MODEL_SYSML);
    // A later revision of the model in which the gateway definition was renamed away.
    let later = MODEL_SYSML.replace("part def ToolGateway", "part def RenamedGateway");
    assert_ne!(later, MODEL_SYSML, "the edit must change the model");
    let r2 = resolve(&graph, &later);

    assert_ne!(r1.revision, r2.revision);
    let gateway = "KrOKiAssurance::ToolGateway";
    assert_eq!(
        link(&r1, "KR-A06", LinkClass::Element, gateway),
        &LinkStatus::Resolved
    );
    assert!(matches!(
        link(&r2, "KR-A06", LinkClass::Element, gateway),
        LinkStatus::Dangling { .. }
    ));
    // Links to elements that still exist are unaffected.
    assert_eq!(
        link(
            &r2,
            "KR-A01",
            LinkClass::Element,
            "KrOKiAssurance::ModelService"
        ),
        &LinkStatus::Resolved
    );
}

#[test]
fn the_assurance_analysis_agrees_with_link_resolution() {
    let graph = graph();
    let revisions = CurrentRevisions::new("m", "i");
    let dangling_elements = |model: &str| -> BTreeSet<String> {
        let index = ModelRevisionIndex::from_sysml_text(model);
        analyze(&graph, &[], &revisions, Some(index.locators()))
            .unwrap()
            .gaps
            .into_iter()
            .filter_map(|g| match g.kind {
                GapKind::DanglingElement { element_id } => {
                    Some(format!("{}:{element_id}", g.requirement_id))
                }
                _ => None,
            })
            .collect()
    };
    assert_eq!(dangling_elements(MODEL_SYSML), BTreeSet::new());
    let later = MODEL_SYSML.replace("part def ToolGateway", "part def RenamedGateway");
    assert_eq!(
        dangling_elements(&later),
        BTreeSet::from(["KR-A06:KrOKiAssurance::ToolGateway".to_string()])
    );
}

#[test]
fn the_declaration_scanner_agrees_with_the_real_parser_on_the_checked_in_model() {
    use sysml_v2_parser::ast::{Body, PackageBodyElement, PartDefBodyElement, RootElement};

    let doc = sysml_v2_parser::parse(MODEL_SYSML).expect("the model must parse");
    let package = doc
        .root
        .elements
        .iter()
        .find_map(|e| match &e.value {
            RootElement::Package(p) => Some(&p.value),
            _ => None,
        })
        .expect("one root package");
    let Body::Brace { elements, .. } = &package.body else {
        panic!("package has a body")
    };
    let mut part_defs = 0;
    let mut nested_usages = 0;
    for member in elements {
        if let PackageBodyElement::PartDef(def) = &member.value {
            part_defs += 1;
            if let Body::Brace { elements, .. } = &def.value.body {
                nested_usages += elements
                    .iter()
                    .filter(|m| matches!(m.value, PartDefBodyElement::PartUsage(_)))
                    .count();
            }
        }
    }

    let index = ModelRevisionIndex::from_sysml_text(MODEL_SYSML);
    let depth = |n: usize| {
        index
            .locators()
            .iter()
            .filter(|l| l.matches("::").count() == n)
            .count()
    };
    assert_eq!(depth(1), part_defs, "part definitions: scanner vs parser");
    assert_eq!(
        depth(2),
        nested_usages,
        "nested part usages: scanner vs parser"
    );
    assert!(part_defs >= 8 && nested_usages >= 8);
}
