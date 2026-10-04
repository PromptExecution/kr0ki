use crate::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("invalid behavior JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("behavior invariant violations: {0:?}")]
    Invariants(Vec<Diagnostic>),
}

pub(crate) fn finding(
    findings: &mut Vec<Diagnostic>,
    code: &str,
    severity: Severity,
    message: impl Into<String>,
    anchor: Option<&Anchor>,
) {
    findings.push(Diagnostic {
        code: code.into(),
        severity,
        message: message.into(),
        anchor: anchor.cloned(),
    });
}

fn is_hex(value: &str, lengths: &[usize]) -> bool {
    lengths.contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':'])
        && !path.chars().any(char::is_control)
        && path.split('/').all(|part| !matches!(part, "" | "." | ".."))
}

fn nonempty(value: &str) -> bool {
    !value.trim().is_empty()
}

/// Iterative Kosaraju traversal: each vertex and containment edge is visited
/// a bounded number of times, including in disconnected or invalid graphs.
fn containment_cycles<'a>(contains: &BTreeMap<&'a str, Vec<&'a str>>) -> Vec<&'a str> {
    let ids: BTreeSet<_> = contains
        .iter()
        .flat_map(|(from, targets)| std::iter::once(*from).chain(targets.iter().copied()))
        .collect();
    let ids: Vec<_> = ids.into_iter().collect();
    let index: BTreeMap<_, _> = ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let mut forward = vec![Vec::new(); ids.len()];
    let mut reverse = vec![Vec::new(); ids.len()];
    for (from, targets) in contains {
        let from = index[from];
        for to in targets {
            let to = index[to];
            forward[from].push(to);
            reverse[to].push(from);
        }
    }
    let mut visited = vec![false; ids.len()];
    let mut finished = Vec::with_capacity(ids.len());
    for start in 0..ids.len() {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        let mut pending = vec![(start, 0)];
        while let Some((node, next)) = pending.last_mut() {
            if let Some(&child) = forward[*node].get(*next) {
                *next += 1;
                if !visited[child] {
                    visited[child] = true;
                    pending.push((child, 0));
                }
            } else {
                finished.push(*node);
                pending.pop();
            }
        }
    }
    visited.fill(false);
    let mut cyclic = Vec::new();
    for start in finished.into_iter().rev() {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        let mut pending = vec![start];
        let mut component = Vec::new();
        while let Some(node) = pending.pop() {
            component.push(node);
            for &parent in &reverse[node] {
                if !visited[parent] {
                    visited[parent] = true;
                    pending.push(parent);
                }
            }
        }
        if component.len() > 1 || forward[start].contains(&start) {
            cyclic.extend(component.into_iter().map(|node| ids[node]));
        }
    }
    cyclic
}

fn check_anchor(
    anchor: &Anchor,
    sources: &BTreeMap<&str, &SourceFile>,
    findings: &mut Vec<Diagnostic>,
) {
    let Some(source) = sources.get(anchor.file.as_str()) else {
        finding(
            findings,
            "anchor_source",
            Severity::Error,
            format!("anchor source {} is absent", anchor.file),
            Some(anchor),
        );
        return;
    };
    let start = anchor.start as usize;
    let end = anchor.end as usize;
    if !nonempty(&anchor.symbol)
        || start > end
        || end > source.content.len()
        || !source.content.is_char_boundary(start)
        || !source.content.is_char_boundary(end)
    {
        finding(
            findings,
            "anchor_span",
            Severity::Error,
            "anchor must name a symbol and a valid UTF-8 byte range",
            Some(anchor),
        );
    }
}

impl RustBehaviorIr {
    pub fn ensure_valid(&self) -> Result<(), ValidationError> {
        let errors: Vec<_> = self
            .validate()
            .into_iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(ValidationError::Invariants(errors))
        }
    }

    /// Include extractor diagnostics and deterministic invariant/lint findings.
    /// Warnings preserve unknown behavior without manufacturing runtime facts.
    pub fn validate(&self) -> Vec<Diagnostic> {
        let mut findings = self.diagnostics.clone();
        if self.schema_version != SCHEMA_VERSION {
            finding(
                &mut findings,
                "schema_version",
                Severity::Error,
                format!(
                    "expected schema version {SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
                None,
            );
        }
        if !is_hex(&self.provenance.revision, &[40, 64]) {
            finding(
                &mut findings,
                "source_revision",
                Severity::Error,
                "revision must be a full lowercase Git object id",
                None,
            );
        }
        if !is_hex(&self.provenance.tree_digest, &[64]) {
            finding(
                &mut findings,
                "tree_digest",
                Severity::Error,
                "tree_digest must be a lowercase SHA-256",
                None,
            );
        }
        if !nonempty(&self.provenance.extractor) || !nonempty(&self.provenance.toolchain) {
            finding(
                &mut findings,
                "analysis_provenance",
                Severity::Error,
                "extractor and toolchain must be recorded",
                None,
            );
        }
        let mut sources = BTreeMap::new();
        for source in &self.sources {
            if !relative_path(&source.path) {
                finding(
                    &mut findings,
                    "source_path",
                    Severity::Error,
                    format!("source path must be portable and relative: {}", source.path),
                    None,
                );
            }
            if sources.insert(source.path.as_str(), source).is_some() {
                finding(
                    &mut findings,
                    "duplicate_source",
                    Severity::Error,
                    format!("duplicate source {}", source.path),
                    None,
                );
            }
            if !is_hex(&source.sha256, &[64]) || digest(&source.content) != source.sha256 {
                finding(
                    &mut findings,
                    "source_digest",
                    Severity::Error,
                    format!("source digest mismatch for {}", source.path),
                    None,
                );
            }
        }
        for diagnostic in &self.diagnostics {
            if !nonempty(&diagnostic.code) || !nonempty(&diagnostic.message) {
                finding(
                    &mut findings,
                    "diagnostic_content",
                    Severity::Error,
                    "diagnostic code and message must be nonempty",
                    diagnostic.anchor.as_ref(),
                );
            }
            if let Some(anchor) = &diagnostic.anchor {
                check_anchor(anchor, &sources, &mut findings);
            }
        }
        let mut nodes = BTreeMap::new();
        for node in &self.nodes {
            if !nonempty(&node.id) || !nonempty(&node.name) {
                finding(
                    &mut findings,
                    "node_identifier",
                    Severity::Error,
                    "nodes must have nonempty ids and names",
                    node.anchor.as_ref(),
                );
            }
            if nodes.insert(node.id.as_str(), node).is_some() {
                finding(
                    &mut findings,
                    "duplicate_node",
                    Severity::Error,
                    format!("duplicate node {}", node.id),
                    node.anchor.as_ref(),
                );
            }
            match &node.anchor {
                Some(anchor) => check_anchor(anchor, &sources, &mut findings),
                None if node.kind != NodeKind::External => finding(
                    &mut findings,
                    "node_provenance",
                    Severity::Error,
                    format!("source-derived node {} has no anchor", node.id),
                    None,
                ),
                None => (),
            }
            for annotation in &node.annotations {
                check_anchor(&annotation.anchor, &sources, &mut findings);
                if !nonempty(&annotation.path) {
                    finding(
                        &mut findings,
                        "annotation_path",
                        Severity::Error,
                        "source annotation path must be nonempty",
                        Some(&annotation.anchor),
                    );
                }
                let source_text = sources
                    .get(annotation.anchor.file.as_str())
                    .and_then(|source| {
                        source
                            .content
                            .get(annotation.anchor.start as usize..annotation.anchor.end as usize)
                    });
                if !(annotation.text.starts_with("#[") || annotation.text.starts_with("#!["))
                    || source_text != Some(annotation.text.as_str())
                {
                    finding(
                        &mut findings,
                        "annotation_source",
                        Severity::Error,
                        "source annotation text must begin with #[ or #![ and exactly match its source span",
                        Some(&annotation.anchor),
                    );
                }
            }
        }
        let mut edges = BTreeSet::new();
        let mut contains: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        let mut owners: BTreeMap<&str, &str> = BTreeMap::new();
        for edge in &self.edges {
            check_anchor(&edge.anchor, &sources, &mut findings);
            if !nonempty(&edge.id) || !edges.insert(edge.id.as_str()) {
                finding(
                    &mut findings,
                    "duplicate_edge",
                    Severity::Error,
                    format!("edge id is empty or duplicated: {}", edge.id),
                    Some(&edge.anchor),
                );
            }
            let (Some(from), Some(to)) =
                (nodes.get(edge.from.as_str()), nodes.get(edge.to.as_str()))
            else {
                finding(
                    &mut findings,
                    "dangling_edge",
                    Severity::Error,
                    format!("edge {} references an absent endpoint", edge.id),
                    Some(&edge.anchor),
                );
                continue;
            };
            if edge.guard.as_deref().is_some_and(|guard| !nonempty(guard)) {
                finding(
                    &mut findings,
                    "empty_guard",
                    Severity::Error,
                    "guard must be absent or nonempty",
                    Some(&edge.anchor),
                );
            }
            let incompatible = match edge.kind {
                EdgeKind::Satisfies => from.kind != NodeKind::Type || to.kind != NodeKind::Trait,
                EdgeKind::Requires | EdgeKind::GovernedBy => to.kind != NodeKind::Trait,
                EdgeKind::Calls => !matches!(
                    to.kind,
                    NodeKind::Function | NodeKind::Dispatch | NodeKind::External
                ),
                EdgeKind::Back => to.kind != NodeKind::Loop,
                EdgeKind::Branch => {
                    !matches!(from.kind, NodeKind::Decision | NodeKind::Dispatch)
                        || edge.guard.is_none()
                }
                EdgeKind::Contains => {
                    if owners
                        .insert(edge.to.as_str(), edge.from.as_str())
                        .is_some_and(|owner| owner != edge.from)
                    {
                        finding(
                            &mut findings,
                            "ambiguous_ownership",
                            Severity::Error,
                            format!("node {} has multiple containment parents", edge.to),
                            Some(&edge.anchor),
                        );
                    }
                    contains
                        .entry(edge.from.as_str())
                        .or_default()
                        .push(edge.to.as_str());
                    edge.from == edge.to
                }
                _ => false,
            };
            if incompatible {
                finding(
                    &mut findings,
                    "edge_kind",
                    Severity::Error,
                    format!(
                        "edge {} is incompatible with endpoint kinds or guard",
                        edge.id
                    ),
                    Some(&edge.anchor),
                );
            }
            if edge.resolution != Resolution::Resolved {
                let code = if edge.resolution == Resolution::Unresolved {
                    "unresolved_fact"
                } else {
                    "inferred_fact"
                };
                finding(
                    &mut findings,
                    code,
                    Severity::Warning,
                    format!("edge {} is {:?}", edge.id, edge.resolution),
                    Some(&edge.anchor),
                );
            }
        }
        for start in containment_cycles(&contains) {
            finding(
                &mut findings,
                "containment_cycle",
                Severity::Error,
                format!("containment cycle through {start}"),
                None,
            );
        }
        for node in &self.nodes {
            if node.kind == NodeKind::Loop
                && !self
                    .edges
                    .iter()
                    .any(|e| e.from == node.id && e.kind == EdgeKind::Exit)
            {
                finding(
                    &mut findings,
                    "unknown_loop_exit",
                    Severity::Warning,
                    format!(
                        "loop {} has no known exit; termination is unproven",
                        node.id
                    ),
                    node.anchor.as_ref(),
                );
            }
            if node.kind == NodeKind::Dispatch
                && !self
                    .edges
                    .iter()
                    .any(|e| e.from == node.id && e.kind == EdgeKind::Calls)
            {
                finding(
                    &mut findings,
                    "unresolved_dispatch",
                    Severity::Warning,
                    format!("dispatch {} has no proven targets", node.id),
                    node.anchor.as_ref(),
                );
            }
        }
        let mut machines = BTreeSet::new();
        for machine in &self.machines {
            if !machines.insert(machine.id.as_str()) {
                finding(
                    &mut findings,
                    "duplicate_machine",
                    Severity::Error,
                    format!("duplicate machine {}", machine.id),
                    None,
                );
            }
            findings.extend(machine.validate());
            for state in &machine.states {
                if state.anchor.is_none() {
                    finding(
                        &mut findings,
                        "machine_provenance",
                        Severity::Error,
                        format!(
                            "machine {} state {} has no source anchor",
                            machine.id, state.id
                        ),
                        None,
                    );
                }
            }
            for transition in &machine.transitions {
                if transition.anchor.is_none() {
                    finding(
                        &mut findings,
                        "machine_provenance",
                        Severity::Error,
                        format!(
                            "machine {} transition {} has no source anchor",
                            machine.id, transition.id
                        ),
                        None,
                    );
                }
            }
            for anchor in machine
                .states
                .iter()
                .filter_map(|s| s.anchor.as_ref())
                .chain(machine.transitions.iter().filter_map(|t| t.anchor.as_ref()))
            {
                check_anchor(anchor, &sources, &mut findings);
            }
        }
        findings.sort();
        findings.dedup();
        findings
    }
}

impl StateMachine {
    /// Structural transition-table validation; callback meanings are supplied by
    /// the runtime application, never evaluated as embedded source code.
    pub fn validate(&self) -> Vec<Diagnostic> {
        let mut findings = Vec::new();
        if !nonempty(&self.id) || !nonempty(&self.name) {
            finding(
                &mut findings,
                "machine_identifier",
                Severity::Error,
                "machine id and name must be nonempty",
                None,
            );
        }
        let mut states = BTreeMap::new();
        for state in &self.states {
            if !nonempty(&state.id) || states.insert(state.id.as_str(), state).is_some() {
                finding(
                    &mut findings,
                    "duplicate_state",
                    Severity::Error,
                    format!("state id is empty or duplicated: {}", state.id),
                    state.anchor.as_ref(),
                );
            }
        }
        if !states.contains_key(self.initial.as_str()) {
            finding(
                &mut findings,
                "initial_state",
                Severity::Error,
                "initial state must exist",
                None,
            );
        }
        let mut transitions = BTreeSet::new();
        let mut unguarded = BTreeSet::new();
        for transition in &self.transitions {
            if !nonempty(&transition.id) || !transitions.insert(transition.id.as_str()) {
                finding(
                    &mut findings,
                    "duplicate_transition",
                    Severity::Error,
                    format!("transition id is empty or duplicated: {}", transition.id),
                    transition.anchor.as_ref(),
                );
            }
            if !states.contains_key(transition.from.as_str())
                || !states.contains_key(transition.to.as_str())
            {
                finding(
                    &mut findings,
                    "transition_target",
                    Severity::Error,
                    format!("transition {} references absent state", transition.id),
                    transition.anchor.as_ref(),
                );
            }
            if states
                .get(transition.from.as_str())
                .is_some_and(|state| state.terminal)
            {
                finding(
                    &mut findings,
                    "terminal_transition",
                    Severity::Error,
                    format!("terminal state {} has outgoing transition", transition.from),
                    transition.anchor.as_ref(),
                );
            }
            if !nonempty(&transition.event)
                || transition.guard.as_deref().is_some_and(|s| !nonempty(s))
                || transition.effect.as_deref().is_some_and(|s| !nonempty(s))
            {
                finding(
                    &mut findings,
                    "transition_content",
                    Severity::Error,
                    "event, guard and effect identifiers must be nonempty",
                    transition.anchor.as_ref(),
                );
            }
            if transition.guard.is_none()
                && !unguarded.insert((&transition.from, &transition.event))
            {
                finding(
                    &mut findings,
                    "ambiguous_transition",
                    Severity::Error,
                    format!(
                        "multiple unguarded transitions for {} / {}",
                        transition.from, transition.event
                    ),
                    transition.anchor.as_ref(),
                );
            }
        }
        let mut reachable = BTreeSet::new();
        let mut pending = vec![self.initial.as_str()];
        while let Some(state) = pending.pop() {
            if reachable.insert(state) {
                pending.extend(
                    self.transitions
                        .iter()
                        .filter(|t| t.from == state)
                        .map(|t| t.to.as_str()),
                );
            }
        }
        for state in &self.states {
            if !reachable.contains(state.id.as_str()) {
                finding(
                    &mut findings,
                    "unreachable_state",
                    Severity::Warning,
                    format!(
                        "state {} is unreachable from initial state {}",
                        state.id, self.initial
                    ),
                    state.anchor.as_ref(),
                );
            }
        }
        findings.sort();
        findings
    }

    pub fn ensure_valid(&self) -> Result<(), ValidationError> {
        let errors: Vec<_> = self
            .validate()
            .into_iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(ValidationError::Invariants(errors))
        }
    }
}
