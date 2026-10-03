//! KR-A03: the requirements view that **distinguishes satisfaction assertions from
//! verification results**.
//!
//! The pre-existing `VerificationCoverage` view counts any `verifies` edge to evidence as
//! covered. That conflates two different claims, so this view never collapses them:
//!
//! - **satisfaction** is an architectural assertion (`system_element —satisfies→ requirement`);
//!   it is drawn as a *dashed* edge and says nothing about whether a case ran;
//! - **verification** is revision-bound evidence; it is drawn as an evidence node carrying the
//!   result and whether it is still fresh at the current revisions.
//!
//! A requirement is therefore exactly one of [`Assurance::Unsatisfied`],
//! [`Assurance::SatisfiedUntested`], [`Assurance::Verified`], [`Assurance::Failing`],
//! [`Assurance::Stale`], and each has its own fill colour and label in the diagram and its own
//! column values in the table. The computation is `ufo_types::mbse::assurance::analyze`; this
//! module only presents it.

use std::collections::BTreeMap;

use serde::Serialize;
use ufo_types::mbse::assurance::{
    analyze, Assurance, CurrentRevisions, EvidenceRecord, Freshness, NodeKind, ThreadError,
    ThreadReport, VerificationResult, NODE_KIND_ATTR,
};
use ufo_types::mbse::requirements::{RelationAuthority, Requirement, RequirementGraph};

use crate::b00t_graph::{d2_escape_label, d2_quote};

/// One requirement's row: both axes, side by side.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Row {
    pub requirement_id: String,
    pub statement: String,
    /// Satisfaction axis: the elements asserted to satisfy it (empty = nothing asserts it).
    pub satisfied_by: Vec<String>,
    /// Verification axis, derived from evidence at the current revisions.
    pub verification: VerificationAxis,
    pub assurance: Assurance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum VerificationAxis {
    /// No evidence exists for any linked case.
    NoEvidence,
    /// Evidence exists; the newest fresh result decides, otherwise it is stale.
    Evidence {
        fresh_pass: usize,
        fresh_fail: usize,
        stale: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssuranceView {
    pub revisions: CurrentRevisions,
    pub rows: Vec<Row>,
    pub report: ThreadReport,
}

impl AssuranceView {
    pub fn row(&self, id: &str) -> Option<&Row> {
        self.rows.iter().find(|r| r.requirement_id == id)
    }

    /// Requirements per assurance state, for a one-line summary.
    pub fn counts(&self) -> BTreeMap<&'static str, usize> {
        let mut out = BTreeMap::new();
        for r in &self.rows {
            *out.entry(state_name(r.assurance)).or_insert(0) += 1;
        }
        out
    }
}

pub fn state_name(a: Assurance) -> &'static str {
    match a {
        Assurance::Unsatisfied => "unsatisfied",
        Assurance::SatisfiedUntested => "satisfied_untested",
        Assurance::Verified => "verified",
        Assurance::Failing => "failing",
        Assurance::Stale => "stale",
    }
}

fn label(a: Assurance) -> &'static str {
    match a {
        Assurance::Unsatisfied => "not satisfied",
        Assurance::SatisfiedUntested => "satisfied · untested",
        Assurance::Verified => "verified",
        Assurance::Failing => "FAILING",
        Assurance::Stale => "satisfied · result stale",
    }
}

fn fill(a: Assurance) -> &'static str {
    match a {
        Assurance::Unsatisfied => "#e0e0e0",
        Assurance::SatisfiedUntested => "#ffe9a8",
        Assurance::Verified => "#b7ebc6",
        Assurance::Failing => "#f6b3b3",
        Assurance::Stale => "#ffd2a1",
    }
}

/// Build the view: analyse the graph against `evidence` at the `current` revisions.
pub fn assurance_view(
    graph: &RequirementGraph,
    evidence: &[EvidenceRecord],
    current: &CurrentRevisions,
    known_elements: Option<&std::collections::BTreeSet<String>>,
) -> Result<AssuranceView, ThreadError> {
    let report = analyze(graph, evidence, current, known_elements)?;
    let statements: BTreeMap<&str, &str> = graph
        .requirements
        .iter()
        .map(|n| (n.id.as_str(), n.text.as_str()))
        .collect();
    let rows = report
        .requirements
        .iter()
        .map(|t| {
            let (mut fresh_pass, mut fresh_fail, mut stale) = (0, 0, 0);
            for e in &t.evidence {
                match (e.freshness, e.result) {
                    (Freshness::Stale { .. }, _) => stale += 1,
                    (Freshness::Fresh, VerificationResult::Pass) => fresh_pass += 1,
                    (Freshness::Fresh, _) => fresh_fail += 1,
                }
            }
            Row {
                requirement_id: t.requirement_id.clone(),
                statement: statements
                    .get(t.requirement_id.as_str())
                    .copied()
                    .unwrap_or("")
                    .to_string(),
                satisfied_by: t.satisfied_by.clone(),
                verification: if t.evidence.is_empty() {
                    VerificationAxis::NoEvidence
                } else {
                    VerificationAxis::Evidence {
                        fresh_pass,
                        fresh_fail,
                        stale,
                    }
                },
                assurance: t.assurance,
            }
        })
        .collect();
    Ok(AssuranceView {
        revisions: current.clone(),
        rows,
        report,
    })
}

fn kind(n: &Requirement) -> Option<NodeKind> {
    n.attributes
        .get(NODE_KIND_ATTR)
        .and_then(|k| NodeKind::parse(k))
}

/// Deterministic D2 source. Satisfaction edges are dashed; evidence is its own node.
pub fn to_d2(graph: &RequirementGraph, view: &AssuranceView) -> String {
    let mut out = String::new();
    out.push_str("direction: right\n");
    out.push_str(&format!(
        "# model {} · implementation {} · {} case configuration(s)\n",
        view.revisions.model_revision,
        view.revisions.implementation_revision,
        view.revisions.configuration_digests.len()
    ));
    for node in &graph.requirements {
        let id = d2_quote(&node.id);
        match kind(node) {
            Some(NodeKind::Requirement) => {
                let a = view
                    .row(&node.id)
                    .map(|r| r.assurance)
                    .unwrap_or(Assurance::Unsatisfied);
                out.push_str(&format!(
                    "{id}: {} {{ shape: rectangle; style.fill: \"{}\" }}\n",
                    d2_escape_label(&format!("{} · [{}]", node.title, label(a))),
                    fill(a)
                ));
            }
            Some(NodeKind::SourceObligation) => out.push_str(&format!(
                "{id}: {} {{ shape: page }}\n",
                d2_escape_label(&node.title)
            )),
            Some(NodeKind::SystemElement) => out.push_str(&format!(
                "{id}: {} {{ shape: package }}\n",
                d2_escape_label(&node.title)
            )),
            Some(NodeKind::Control) => out.push_str(&format!(
                "{id}: {} {{ shape: hexagon }}\n",
                d2_escape_label(&node.title)
            )),
            Some(NodeKind::VerificationCase) => out.push_str(&format!(
                "{id}: {} {{ shape: diamond }}\n",
                d2_escape_label(&node.title)
            )),
            None => out.push_str(&format!("{id}: {}\n", d2_escape_label(&node.title))),
        }
    }
    // Evidence nodes: one per assessed record, carrying result and freshness.
    for t in &view.report.requirements {
        for e in &t.evidence {
            let ev = d2_quote(&e.key);
            let (result, colour) = match e.result {
                VerificationResult::Pass => ("pass", "#b7ebc6"),
                VerificationResult::Fail => ("fail", "#f6b3b3"),
                VerificationResult::Error => ("error", "#f6b3b3"),
            };
            let fresh = match e.freshness {
                Freshness::Fresh => "fresh",
                Freshness::Stale { .. } => "STALE",
            };
            let fill = if matches!(e.freshness, Freshness::Stale { .. }) {
                "#ffd2a1"
            } else {
                colour
            };
            out.push_str(&format!(
                "{ev}: {} {{ shape: cloud; style.fill: \"{fill}\" }}\n",
                d2_escape_label(&format!("{} · {result} · {fresh}", e.verification_id))
            ));
            out.push_str(&format!(
                "{} -> {ev}: produced\n{ev} -> {}: verification result\n",
                d2_quote(&e.verification_id),
                d2_quote(&t.requirement_id)
            ));
        }
    }
    for r in graph
        .relations
        .iter()
        .filter(|r| matches!(r.authority, RelationAuthority::Asserted))
    {
        let (text, dashed) = match r.kind {
            ufo_types::mbse::requirements::RequirementRelationKind::Satisfies => {
                ("satisfies (assertion)", true)
            }
            ufo_types::mbse::requirements::RequirementRelationKind::Derives => ("derives", false),
            ufo_types::mbse::requirements::RequirementRelationKind::Implements => {
                ("enforces", false)
            }
            ufo_types::mbse::requirements::RequirementRelationKind::AllocatedTo => {
                ("lives in", false)
            }
            ufo_types::mbse::requirements::RequirementRelationKind::Verifies => {
                ("acceptance case", false)
            }
            _ => continue,
        };
        if dashed {
            out.push_str(&format!(
                "{} -> {}: {text} {{ style.stroke-dash: 4 }}\n",
                d2_quote(&r.source),
                d2_quote(&r.target)
            ));
        } else {
            out.push_str(&format!(
                "{} -> {}: {text}\n",
                d2_quote(&r.source),
                d2_quote(&r.target)
            ));
        }
    }
    out
}

/// A plain-text table with the two axes in separate columns — for terminals, agents and logs.
pub fn to_table(view: &AssuranceView) -> String {
    let mut out =
        String::from("requirement | satisfaction (asserted) | verification (evidence) | state\n");
    for r in &view.rows {
        let satisfaction = if r.satisfied_by.is_empty() {
            "none".to_string()
        } else {
            r.satisfied_by.join(", ")
        };
        let verification = match &r.verification {
            VerificationAxis::NoEvidence => "no evidence".to_string(),
            VerificationAxis::Evidence {
                fresh_pass,
                fresh_fail,
                stale,
            } => {
                format!("{fresh_pass} fresh pass, {fresh_fail} fresh fail, {stale} stale")
            }
        };
        out.push_str(&format!(
            "{} | {} | {} | {}\n",
            r.requirement_id,
            satisfaction,
            verification,
            label(r.assurance)
        ));
    }
    out
}
