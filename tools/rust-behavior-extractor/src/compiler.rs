use kr0ki_behavior::*;
#[path = "annotations.rs"]
mod annotations;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::{Compiler, Config};
extern crate rustc_data_structures;
use rustc_data_structures::steal::Steal;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::{
    mir::{BasicBlock, Body, TerminatorKind},
    ty::{self, TyCtxt},
};
use rustc_span::{FileName, Span};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

type MirProvider = for<'tcx> fn(TyCtxt<'tcx>, LocalDefId) -> &'tcx Steal<Body<'tcx>>;
static MIR_PROVIDER: OnceLock<MirProvider> = OnceLock::new();
static MIR_FACTS: Mutex<Vec<RustBehaviorIr>> = Mutex::new(Vec::new());

fn capture_mir(tcx: TyCtxt<'_>, local: LocalDefId) -> &Steal<Body<'_>> {
    let body = MIR_PROVIDER.get().expect("installed MIR provider")(tcx, local);
    let mut extractor = Extractor::new().expect("validated extractor environment");
    extractor.analyze_body(tcx, local, &body.borrow());
    MIR_FACTS
        .lock()
        .expect("MIR snapshots lock")
        .push(extractor.ir);
    body
}

fn symbol(tcx: TyCtxt<'_>, def: rustc_hir::def_id::DefId) -> String {
    let crate_name = tcx.crate_name(def.krate).to_string();
    let path = tcx.def_path_str(def);
    if path.is_empty() {
        crate_name
    } else if path.starts_with(&format!("{crate_name}::")) {
        path
    } else {
        format!("{crate_name}::{path}")
    }
}

pub struct Extractor {
    root: PathBuf,
    shards: PathBuf,
    ir: RustBehaviorIr,
    confirmed_definitions: BTreeSet<String>,
}

fn successors(kind: &TerminatorKind<'_>) -> Vec<BasicBlock> {
    match kind {
        TerminatorKind::FalseEdge { real_target, .. }
        | TerminatorKind::FalseUnwind { real_target, .. } => vec![*real_target],
        _ => kind.successors().collect(),
    }
}

struct Effects<'a, 'tcx> {
    extractor: &'a mut Extractor,
    tcx: TyCtxt<'tcx>,
    symbol: String,
    owner: String,
}

impl<'tcx> Visitor<'tcx> for Effects<'_, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx rustc_hir::Expr<'tcx>) {
        use rustc_hir::ExprKind;
        let code = match &expr.kind {
            ExprKind::Break(..) => Some("control_break"),
            ExprKind::Continue(..) => Some("control_continue"),
            ExprKind::Ret(..) => Some("control_return"),
            ExprKind::Match(_, _, rustc_hir::MatchSource::TryDesugar(_)) => Some("try_boundary"),
            ExprKind::Yield(_, rustc_hir::YieldSource::Await { .. }) => Some("async_await"),
            _ => None,
        };
        if let Some(code) = code {
            if let Some(anchor) = self.extractor.anchor(self.tcx, expr.span, &self.symbol) {
                let id = self.extractor.node(
                    &format!("{}::{code}@{}:{}", self.symbol, anchor.start, anchor.end),
                    NodeKind::Action,
                    Some(anchor.clone()),
                );
                self.extractor.edge(
                    &self.owner,
                    &id,
                    EdgeKind::Contains,
                    None,
                    Resolution::Resolved,
                    &anchor,
                );
                self.extractor.ir.diagnostics.push(Diagnostic { code: code.into(), severity: Severity::Info,
                    message: format!("typed HIR {code} source construct; execution successors are represented by MIR"), anchor: Some(anchor) });
            }
        }
        intravisit::walk_expr(self, expr);
    }
}

impl Extractor {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            root: PathBuf::from(env::var("KR0KI_BEHAVIOR_ROOT")?),
            shards: PathBuf::from(env::var("KR0KI_BEHAVIOR_SHARDS")?),
            confirmed_definitions: BTreeSet::new(),
            ir: RustBehaviorIr {
                schema_version: SCHEMA_VERSION,
                provenance: serde_json::from_str(&env::var("KR0KI_BEHAVIOR_PROVENANCE")?)?,
                sources: vec![],
                nodes: vec![],
                edges: vec![],
                diagnostics: vec![],
                machines: vec![],
            },
        })
    }

    fn anchor(&mut self, tcx: TyCtxt<'_>, span: Span, symbol: &str) -> Option<Anchor> {
        if span.is_dummy() {
            return None;
        }
        let span = span.source_callsite();
        let file = tcx.sess.source_map().lookup_source_file(span.lo());
        let path = match &file.name {
            FileName::Real(real) => real.local_path()?,
            _ => return None,
        };
        let path = if path.is_absolute() {
            path.to_owned()
        } else {
            env::current_dir().ok()?.join(path)
        };
        let path = fs::canonicalize(path).ok()?;
        let relative = path
            .strip_prefix(&self.root)
            .ok()?
            .to_string_lossy()
            .replace('\\', "/");
        let content = fs::read_to_string(&path).ok()?;
        let start = file.original_relative_byte_pos(span.lo()).0;
        let end = file.original_relative_byte_pos(span.hi()).0;
        if end as usize > content.len() || start > end {
            return None;
        }
        if !self.ir.sources.iter().any(|s| s.path == relative) {
            self.ir.sources.push(SourceFile {
                path: relative.clone(),
                sha256: digest(&content),
                content,
            });
        }
        Some(Anchor {
            file: relative,
            symbol: symbol.to_owned(),
            start,
            end,
        })
    }

    fn node(&mut self, name: &str, kind: NodeKind, anchor: Option<Anchor>) -> String {
        let namespace = match kind {
            NodeKind::Function | NodeKind::External => "rust-function",
            NodeKind::Type => "rust-type",
            NodeKind::Trait => "rust-trait",
            NodeKind::Module => "rust-module",
            NodeKind::Field => "rust-field",
            NodeKind::AssociatedType => "rust-associated-type",
            _ => "rust",
        };
        let id = stable_id(namespace, name);
        if let Some(existing) = self.ir.nodes.iter_mut().find(|n| n.id == id) {
            if existing.kind == NodeKind::External && kind != NodeKind::External {
                existing.kind = kind;
                existing.anchor = anchor;
            }
        } else {
            self.ir.nodes.push(Node {
                id: id.clone(),
                name: name.to_owned(),
                kind,
                anchor,
                annotations: vec![],
            });
        }
        id
    }

    fn edge(
        &mut self,
        from: &str,
        to: &str,
        kind: EdgeKind,
        guard: Option<String>,
        resolution: Resolution,
        anchor: &Anchor,
    ) {
        let key = format!(
            "{from}|{to}|{kind:?}|{:?}|{}:{}:{}",
            guard, anchor.file, anchor.start, anchor.end
        );
        let id = stable_id("edge", &key);
        if !self.ir.edges.iter().any(|e| e.id == id) {
            self.ir.edges.push(Edge {
                id,
                from: from.to_owned(),
                to: to.to_owned(),
                kind,
                guard,
                resolution,
                anchor: anchor.clone(),
            });
        }
    }

    fn analyze(&mut self, tcx: TyCtxt<'_>) {
        // Def paths and source byte offsets come from rustc; no syntax name matching.
        for local in tcx.iter_local_def_id() {
            let def = local.to_def_id();
            let kind = match tcx.def_kind(def) {
                DefKind::Mod => NodeKind::Module,
                DefKind::Struct | DefKind::Enum | DefKind::Union | DefKind::TyAlias => {
                    NodeKind::Type
                }
                DefKind::Trait => NodeKind::Trait,
                DefKind::Field => NodeKind::Field,
                DefKind::AssocTy => NodeKind::AssociatedType,
                DefKind::Fn | DefKind::AssocFn | DefKind::Closure => NodeKind::Function,
                _ => continue,
            };
            let name = symbol(tcx, def);
            let Some(anchor) = self.anchor(tcx, tcx.def_span(def), &name) else {
                self.ir.diagnostics.push(Diagnostic {
                    code: "unsupported_source_anchor".into(),
                    severity: Severity::Warning,
                    message: format!("compiler symbol {name} has no portable source span"),
                    anchor: None,
                });
                continue;
            };
            let id = self.node(&name, kind, Some(anchor.clone()));
            self.confirmed_definitions.insert(id.clone());
            // Parent/trait-reference discovery can create a placeholder first.
            // A compiler definition always supplies its canonical declaration
            // span, rather than retaining an earlier impl/reference span.
            self.ir
                .nodes
                .iter_mut()
                .find(|node| node.id == id)
                .expect("node inserted")
                .anchor = Some(anchor.clone());
            let Some(parent) = tcx.opt_parent(def) else {
                continue;
            };
            let parent = if tcx.def_kind(parent) == DefKind::Variant {
                tcx.parent(parent)
            } else {
                parent
            };
            let owner = match tcx.def_kind(parent) {
                DefKind::Mod => Some((symbol(tcx, parent), NodeKind::Module)),
                DefKind::Trait => Some((symbol(tcx, parent), NodeKind::Trait)),
                DefKind::Struct | DefKind::Enum | DefKind::Union => {
                    Some((symbol(tcx, parent), NodeKind::Type))
                }
                DefKind::Fn | DefKind::AssocFn | DefKind::Closure => {
                    Some((symbol(tcx, parent), NodeKind::Function))
                }
                DefKind::Impl { .. } => {
                    let self_ty = tcx.type_of(parent).instantiate_identity().skip_norm_wip();
                    let owner_name = match self_ty.kind() {
                        ty::Adt(adt, _) => symbol(tcx, adt.did()),
                        _ => format!("{}::{self_ty}", tcx.crate_name(parent.krate)),
                    };
                    Some((owner_name, NodeKind::Type))
                }
                _ => None,
            };
            if let Some((parent_name, parent_kind)) = owner {
                if let Some(parent_anchor) = self.anchor(tcx, tcx.def_span(parent), &parent_name) {
                    let parent_id = self.node(&parent_name, parent_kind, Some(parent_anchor));
                    self.edge(
                        &parent_id,
                        &id,
                        EdgeKind::Contains,
                        None,
                        Resolution::Resolved,
                        &anchor,
                    );
                }
            }
            if matches!(
                kind,
                NodeKind::Type | NodeKind::Function | NodeKind::AssociatedType | NodeKind::Trait
            ) {
                let predicates = tcx.predicates_of(def);
                for (predicate, predicate_span) in predicates.predicates {
                    if let Some(bound) = predicate.as_trait_clause() {
                        let target_def = bound.skip_binder().trait_ref.def_id;
                        if target_def == def {
                            continue;
                        }
                        let target_name = symbol(tcx, target_def);
                        let evidence = self
                            .anchor(tcx, *predicate_span, &name)
                            .unwrap_or_else(|| anchor.clone());
                        let target =
                            self.node(&target_name, NodeKind::Trait, Some(evidence.clone()));
                        self.edge(
                            &id,
                            &target,
                            if kind == NodeKind::Trait {
                                EdgeKind::Requires
                            } else {
                                EdgeKind::GovernedBy
                            },
                            Some(format!("{predicate}")),
                            Resolution::Resolved,
                            &evidence,
                        );
                    }
                }
            }
        }
        for local in tcx.iter_local_def_id() {
            let def = local.to_def_id();
            if !matches!(tcx.def_kind(def), DefKind::Impl { of_trait: true }) {
                continue;
            }
            let trait_ref = tcx
                .impl_trait_ref(def)
                .instantiate_identity()
                .skip_norm_wip();
            let name = symbol(tcx, def);
            let Some(anchor) = self.anchor(tcx, tcx.def_span(def), &name) else {
                self.ir.diagnostics.push(Diagnostic {
                    code: "unsupported_source_anchor".into(),
                    severity: Severity::Warning,
                    message: format!("compiler symbol {name} has no portable source span"),
                    anchor: None,
                });
                continue;
            };
            let self_ty = trait_ref.self_ty();
            let type_name = match self_ty.kind() {
                ty::Adt(adt, _) => symbol(tcx, adt.did()),
                _ => format!("{}::{self_ty}", tcx.crate_name(def.krate)),
            };
            let ty_id = self.node(&type_name, NodeKind::Type, Some(anchor.clone()));
            let trait_name = symbol(tcx, trait_ref.def_id);
            let trait_id = self.node(&trait_name, NodeKind::Trait, Some(anchor.clone()));
            let predicates = tcx.predicates_of(def);
            let conditions = (!predicates.predicates.is_empty()).then(|| {
                format!(
                    "{:?}",
                    predicates
                        .predicates
                        .iter()
                        .map(|(clause, _)| clause)
                        .collect::<Vec<_>>()
                )
            });
            self.edge(
                &ty_id,
                &trait_id,
                EdgeKind::Satisfies,
                conditions,
                Resolution::Resolved,
                &anchor,
            );
            for (predicate, _) in predicates.predicates {
                if let Some(bound) = predicate.as_trait_clause() {
                    let bound_name = symbol(tcx, bound.skip_binder().trait_ref.def_id);
                    let bound_id = self.node(&bound_name, NodeKind::Trait, Some(anchor.clone()));
                    self.edge(
                        &ty_id,
                        &bound_id,
                        EdgeKind::GovernedBy,
                        Some(format!("{predicate}")),
                        Resolution::Resolved,
                        &anchor,
                    );
                }
            }
        }
    }

    fn analyze_hir(&mut self, tcx: TyCtxt<'_>) {
        for local in tcx.hir_body_owners() {
            let name = symbol(tcx, local.to_def_id());
            let Some(anchor) = self.anchor(tcx, tcx.def_span(local), &name) else {
                continue;
            };
            let owner = self.node(&name, NodeKind::Function, Some(anchor));
            let body = tcx.hir_body_owned_by(local);
            Effects {
                extractor: self,
                tcx,
                symbol: name,
                owner,
            }
            .visit_body(body);
        }
    }

    fn analyze_body<'tcx>(&mut self, tcx: TyCtxt<'tcx>, local: LocalDefId, body: &Body<'tcx>) {
        let def = local.to_def_id();
        let name = symbol(tcx, def);
        let Some(function_anchor) = self.anchor(tcx, tcx.def_span(def), &name) else {
            return;
        };
        let function = self.node(&name, NodeKind::Function, Some(function_anchor.clone()));
        if tcx.def_span(def).from_expansion() {
            self.ir.diagnostics.push(Diagnostic {
                code: "macro_expansion".into(), severity: Severity::Warning,
                message: "compiler-generated macro body is represented with source callsite evidence; it is not handwritten source behavior".into(),
                anchor: Some(function_anchor.clone()),
            });
        }
        // Captured from the mir_built provider before borrowck/optimization can
        // steal the body. Publication occurs only after compiler analysis succeeds.
        let dominators = body.basic_blocks.dominators();
        let mut loops = BTreeMap::<BasicBlock, Vec<BasicBlock>>::new();
        for (block, data) in body.basic_blocks.iter_enumerated() {
            for successor in successors(&data.terminator().kind) {
                if dominators.is_reachable(block) && dominators.dominates(successor, block) {
                    loops.entry(successor).or_default().push(block);
                }
            }
        }
        let mut predecessors = BTreeMap::<BasicBlock, Vec<BasicBlock>>::new();
        for (block, data) in body.basic_blocks.iter_enumerated() {
            for successor in successors(&data.terminator().kind) {
                predecessors.entry(successor).or_default().push(block);
            }
        }
        let mut loop_members = BTreeMap::new();
        for (head, tails) in &loops {
            let mut members = BTreeSet::from([*head]);
            let mut pending = tails.clone();
            while let Some(member) = pending.pop() {
                if members.insert(member) {
                    pending.extend(predecessors.get(&member).into_iter().flatten().copied());
                }
            }
            loop_members.insert(*head, members);
        }
        let block_name = |b: BasicBlock| format!("{name}::bb{}", b.index());
        for (block, data) in body.basic_blocks.iter_enumerated() {
            let terminator = data.terminator();
            let anchor = self
                .anchor(tcx, terminator.source_info.span, &name)
                .unwrap_or_else(|| function_anchor.clone());
            let kind = match &terminator.kind {
                TerminatorKind::SwitchInt { .. } => NodeKind::Decision,
                TerminatorKind::Return
                | TerminatorKind::Unreachable
                | TerminatorKind::UnwindResume
                | TerminatorKind::UnwindTerminate(_) => NodeKind::Exit,
                _ if predecessors.get(&block).is_some_and(|p| p.len() > 1) => NodeKind::Merge,
                _ => NodeKind::Action,
            };
            let id = self.node(&block_name(block), kind, Some(anchor.clone()));
            self.edge(
                &function,
                &id,
                EdgeKind::Contains,
                None,
                Resolution::Resolved,
                &anchor,
            );
        }
        // Separate loop nodes preserve decision typing for conditional heads.
        for head in loops.keys() {
            let loop_id = self.node(
                &format!("{name}::loop{}", head.index()),
                NodeKind::Loop,
                Some(function_anchor.clone()),
            );
            self.edge(
                &function,
                &loop_id,
                EdgeKind::Contains,
                None,
                Resolution::Resolved,
                &function_anchor,
            );
            self.edge(
                &loop_id,
                &stable_id("rust", &block_name(*head)),
                EdgeKind::Flow,
                None,
                Resolution::Resolved,
                &function_anchor,
            );
        }
        for (block, data) in body.basic_blocks.iter_enumerated() {
            let terminator = data.terminator();
            let anchor = self
                .anchor(tcx, terminator.source_info.span, &name)
                .unwrap_or_else(|| function_anchor.clone());
            let from = stable_id("rust", &block_name(block));
            for successor in successors(&terminator.kind) {
                let to = stable_id("rust", &block_name(successor));
                let guard = if let TerminatorKind::SwitchInt { discr, targets } = &terminator.kind {
                    Some(
                        targets
                            .iter()
                            .filter(|(_, target)| *target == successor)
                            .map(|(value, _)| format!("{discr:?} == {value}"))
                            .collect::<Vec<_>>()
                            .join(" or "),
                    )
                } else {
                    None
                };
                let guard = guard.map(|g| if g.is_empty() { "otherwise".into() } else { g });
                let kind = if guard.is_some() {
                    EdgeKind::Branch
                } else if matches!(
                    body.basic_blocks[successor].terminator().kind,
                    TerminatorKind::Return
                ) {
                    EdgeKind::Exit
                } else {
                    EdgeKind::Flow
                };
                self.edge(&from, &to, kind, guard, Resolution::Resolved, &anchor);
                for (head, members) in &loop_members {
                    if members.contains(&block)
                        && !members.contains(&successor)
                        && !body.basic_blocks[successor].is_cleanup
                    {
                        let loop_id = stable_id("rust", &format!("{name}::loop{}", head.index()));
                        self.edge(
                            &loop_id,
                            &to,
                            EdgeKind::Exit,
                            None,
                            Resolution::Resolved,
                            &anchor,
                        );
                    }
                }
                if loops
                    .get(&successor)
                    .is_some_and(|tails| tails.contains(&block))
                {
                    let loop_id = stable_id("rust", &format!("{name}::loop{}", successor.index()));
                    self.edge(
                        &from,
                        &loop_id,
                        EdgeKind::Back,
                        None,
                        Resolution::Resolved,
                        &anchor,
                    );
                }
            }
            if let TerminatorKind::Call { func, .. } | TerminatorKind::TailCall { func, .. } =
                &terminator.kind
            {
                let func_ty = func.ty(&body.local_decls, tcx);
                let typing_env = ty::TypingEnv::post_analysis(tcx, def);
                let (target_name, resolved) = if let ty::FnDef(callee, args) = func_ty.kind() {
                    match ty::Instance::try_resolve(tcx, typing_env, *callee, args) {
                        Ok(Some(instance))
                            if !matches!(instance.def, ty::InstanceKind::Virtual(..)) =>
                        {
                            (symbol(tcx, instance.def_id()), true)
                        }
                        _ => (
                            format!("{name}::dispatch@{}:{}", anchor.start, anchor.end),
                            false,
                        ),
                    }
                } else {
                    (
                        format!("{name}::dispatch@{}:{}", anchor.start, anchor.end),
                        false,
                    )
                };
                let target = self.node(
                    &target_name,
                    if resolved {
                        NodeKind::External
                    } else {
                        NodeKind::Dispatch
                    },
                    if resolved { None } else { Some(anchor.clone()) },
                );
                self.edge(
                    &from,
                    &target,
                    EdgeKind::Calls,
                    None,
                    if resolved {
                        Resolution::Resolved
                    } else {
                        Resolution::Unresolved
                    },
                    &anchor,
                );
                if resolved && target_name == name {
                    self.ir.diagnostics.push(Diagnostic {
                        code: "recursive_call".into(),
                        severity: Severity::Warning,
                        message: "compiler-resolved direct recursion; termination is unproven"
                            .into(),
                        anchor: Some(anchor.clone()),
                    });
                }
                if !resolved {
                    self.ir.diagnostics.push(Diagnostic {
                        code: "unresolved_dispatch".into(),
                        severity: Severity::Warning,
                        message: format!("compiler cannot select a concrete target for {func_ty}"),
                        anchor: Some(anchor.clone()),
                    });
                }
            }
            if matches!(terminator.kind, TerminatorKind::Yield { .. }) {
                self.ir.diagnostics.push(Diagnostic {
                    code: "async_suspend".into(),
                    severity: Severity::Info,
                    message: "compiler coroutine suspension and resume boundary".into(),
                    anchor: Some(anchor.clone()),
                });
            }
        }
    }
}

impl Callbacks for Extractor {
    fn config(&mut self, config: &mut Config) {
        config.override_queries = Some(|_session, providers| {
            let _ = MIR_PROVIDER.set(providers.queries.mir_built);
            providers.queries.mir_built = capture_mir;
        });
    }

    fn after_analysis(&mut self, _compiler: &Compiler, tcx: TyCtxt<'_>) -> Compilation {
        self.analyze(tcx);
        self.analyze_hir(tcx);
        // Ensure every local body has been queried; computed bodies already have
        // snapshots even when the compiler subsequently stole their storage.
        for local in tcx.mir_keys(()) {
            let _ = tcx.mir_built(*local);
        }
        for snapshot in MIR_FACTS.lock().expect("MIR snapshots lock").drain(..) {
            for source in snapshot.sources {
                if !self
                    .ir
                    .sources
                    .iter()
                    .any(|existing| existing.path == source.path)
                {
                    self.ir.sources.push(source);
                }
            }
            for node in snapshot.nodes {
                self.node(&node.name, node.kind, node.anchor);
            }
            self.ir.edges.extend(snapshot.edges);
            self.ir.diagnostics.extend(snapshot.diagnostics);
        }
        annotations::attach(&mut self.ir, &self.confirmed_definitions);
        self.ir.normalize();
        let file = self.shards.join(format!(
            "{}-{:?}.json",
            tcx.crate_name(rustc_hir::def_id::LOCAL_CRATE),
            tcx.stable_crate_id(rustc_hir::def_id::LOCAL_CRATE)
        ));
        let result: Result<(), Box<dyn std::error::Error>> = (|| {
            self.ir.ensure_valid()?;
            let shard = crate::shard::CompilerShard {
                ir: self.ir.clone(),
                confirmed_definitions: self.confirmed_definitions.clone(),
            };
            fs::write(file, serde_json::to_vec(&shard)?)?;
            Ok(())
        })();
        match result {
            Ok(()) => (),
            Err(error) => {
                eprintln!("invalid compiler IR: {error}");
                std::process::exit(1);
            }
        }
        Compilation::Continue
    }
}
