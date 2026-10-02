//! `kr0ki-assurance` — the assurance thread from a shell. A thin front end over
//! `kr0ki_core::assurance_service::AssuranceService`, the same code the HTTP routes and MCP
//! tools call, so a result here is the result there.
//!
//! ```text
//! kr0ki-assurance lint     [--strict]              load, profile + statement lint, links, ReqIF round trip, gaps
//! kr0ki-assurance verify   [CASE-ID ...]           run declared cases, store revision-bound evidence
//! kr0ki-assurance view     [--format table|d2|json]
//! kr0ki-assurance evidence [--requirement ID] [--case ID]
//! kr0ki-assurance gaps
//!
//! common: --baseline PATH   (default docs/assurance/kr0ki.assurance.toml)
//!         --repo-root PATH  (default .)
//!         --evidence-dir PATH (default .kr0ki-evidence)
//! ```
//!
//! Exit status: 0 success; 1 a defect (load error, ReqIF round trip lost content, a case that
//! did not pass, or under `--strict` a dangling link or lint finding); 2 usage.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use kr0ki_core::assurance_service::{AssuranceConfig, AssuranceService, RequirementFilter};
use kr0ki_core::assurance_trace::LinkStatus;
use kr0ki_core::assurance_view::{to_d2, to_table};
use kr0ki_core::reqif_roundtrip::round_trip;
use kr0ki_core::verification_runner::{GitRevisionProbe, ProcessExecutor};
use ufo_types::mbse::assurance::VerificationResult;

struct Args {
    command: String,
    positional: Vec<String>,
    baseline: PathBuf,
    repo_root: PathBuf,
    evidence_dir: PathBuf,
    format: String,
    strict: bool,
    requirement: Option<String>,
    case: Option<String>,
}

fn parse() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let command = it.next().ok_or("missing command")?;
    let mut a = Args {
        command,
        positional: Vec::new(),
        baseline: PathBuf::from("docs/assurance/kr0ki.assurance.toml"),
        repo_root: PathBuf::from("."),
        evidence_dir: PathBuf::from(".kr0ki-evidence"),
        format: "table".into(),
        strict: false,
        requirement: None,
        case: None,
    };
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "--baseline" => a.baseline = PathBuf::from(value("--baseline")?),
            "--repo-root" => a.repo_root = PathBuf::from(value("--repo-root")?),
            "--evidence-dir" => a.evidence_dir = PathBuf::from(value("--evidence-dir")?),
            "--format" => a.format = value("--format")?,
            "--requirement" => a.requirement = Some(value("--requirement")?),
            "--case" => a.case = Some(value("--case")?),
            "--strict" => a.strict = true,
            flag if flag.starts_with("--") => return Err(format!("unknown flag {flag}")),
            other => a.positional.push(other.to_string()),
        }
    }
    Ok(a)
}

fn service(a: &Args) -> AssuranceService {
    let root = a
        .repo_root
        .canonicalize()
        .unwrap_or_else(|_| a.repo_root.clone());
    AssuranceService::new(
        AssuranceConfig {
            baseline_path: if a.baseline.is_absolute() {
                a.baseline.clone()
            } else {
                root.join(&a.baseline)
            },
            repo_root: root.clone(),
            evidence_dir: if a.evidence_dir.is_absolute() {
                a.evidence_dir.clone()
            } else {
                root.join(&a.evidence_dir)
            },
            run_timeout: Duration::from_secs(1800),
        },
        Arc::new(GitRevisionProbe::new(root)),
        Arc::new(ProcessExecutor),
    )
}

fn lint(svc: &AssuranceService, strict: bool) -> Result<bool, String> {
    let snap = svc.snapshot().map_err(|e| e.to_string())?;
    let mut defects = 0usize;
    let mut warnings = 0usize;

    println!(
        "baseline  {} (model {}, implementation {})",
        snap.graph().baseline.id,
        snap.current.model_revision,
        snap.current.implementation_revision
    );
    for (kind, n) in AssuranceService::node_kinds(&snap) {
        println!("  {:>2} {}", n, kind.as_str());
    }

    for (id, lint) in &snap.loaded.statement_lints {
        println!("lint      {id}: {lint}");
        warnings += 1;
    }

    let rt = round_trip(snap.graph()).map_err(|e| e.to_string())?;
    println!(
        "reqif     {} nodes, {} attributes, {} relations compared; {}",
        rt.report.requirements_compared,
        rt.report.attributes_compared,
        rt.report.relations_compared,
        if rt.report.is_lossless() {
            "lossless".to_string()
        } else {
            format!("LOST {}", rt.report.differences.len())
        }
    );
    for d in &rt.report.differences {
        println!("          defect: {d:?}");
        defects += 1;
    }
    for u in &rt.report.unsupported {
        println!(
            "          unsupported ({:?}): {} id(s) - {}",
            u.kind,
            u.ids.len(),
            u.note
        );
    }

    let dangling: Vec<_> = snap.links.dangling().collect();
    println!(
        "links     {} resolved, {} dangling at model revision {}",
        snap.links.links.len() - dangling.len(),
        dangling.len(),
        snap.links.revision
    );
    for l in &dangling {
        if let LinkStatus::Dangling { reason } = &l.status {
            println!(
                "          dangling: {} {:?} {} - {reason}",
                l.requirement_id, l.class, l.locator
            );
        }
    }
    if strict {
        warnings += dangling.len();
    }

    println!(
        "gaps      {} across {} requirements",
        snap.view.report.gaps.len(),
        snap.view.rows.len()
    );
    let mut by_kind: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for g in &snap.view.report.gaps {
        let name = serde_json::to_value(&g.kind)
            .ok()
            .and_then(|v| v["gap"].as_str().map(str::to_owned))
            .unwrap_or_default();
        *by_kind.entry(name).or_default() += 1;
    }
    for (k, n) in by_kind {
        println!("          {n:>2} {k}");
    }
    println!("state     {:?}", snap.view.counts());

    println!(
        "result    {defects} defect(s), {warnings} warning(s){}",
        if strict { " (strict)" } else { "" }
    );
    Ok(defects == 0 && !(strict && warnings > 0))
}

fn main() -> ExitCode {
    let args = match parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("kr0ki-assurance: {e}\nusage: kr0ki-assurance lint|verify|view|evidence|gaps [flags]  (see the file header)");
            return ExitCode::from(2);
        }
    };
    let svc = service(&args);
    let outcome: Result<bool, String> = (|| match args.command.as_str() {
        "lint" => lint(&svc, args.strict),
        "verify" => {
            let outcomes = if args.positional.is_empty() {
                svc.verify_all().map_err(|e| e.to_string())?
            } else {
                let snap = svc.snapshot().map_err(|e| e.to_string())?;
                let mut v = Vec::new();
                for case in &args.positional {
                    v.push(
                        svc.run_verification(case, &snap.current.implementation_revision)
                            .map_err(|e| e.to_string())?,
                    );
                }
                v
            };
            let mut all_pass = true;
            for o in &outcomes {
                println!(
                    "{:<8} {:?}  {}  (implementation {})",
                    o.case_id, o.result, o.detail, o.revisions.implementation_revision
                );
                all_pass &= o.result == VerificationResult::Pass;
            }
            println!("ran {} case(s)", outcomes.len());
            Ok(all_pass)
        }
        "view" => {
            let snap = svc.snapshot().map_err(|e| e.to_string())?;
            match args.format.as_str() {
                "table" => print!("{}", to_table(&snap.view)),
                "d2" => print!("{}", to_d2(snap.graph(), &snap.view)),
                "json" => println!(
                    "{}",
                    serde_json::to_string_pretty(&snap.view).map_err(|e| e.to_string())?
                ),
                other => return Err(format!("unknown --format {other} (table|d2|json)")),
            }
            Ok(true)
        }
        "evidence" => {
            let list = svc
                .evidence(args.requirement.as_deref(), args.case.as_deref())
                .map_err(|e| e.to_string())?;
            println!(
                "at model {} / implementation {}",
                list.revisions.model_revision, list.revisions.implementation_revision
            );
            for r in &list.records {
                println!(
                    "{:<8} {:<8} {:?}  {:?}  artifact {}  {}",
                    r.record.requirement_id,
                    r.record.verification_id,
                    r.record.result,
                    r.freshness,
                    r.artifact,
                    r.evidence_key
                );
            }
            println!("{} record(s)", list.records.len());
            Ok(true)
        }
        "gaps" => {
            let list = svc
                .list_requirements(&RequirementFilter::default())
                .map_err(|e| e.to_string())?;
            for r in &list.requirements {
                let gaps: Vec<String> = r
                    .gaps
                    .iter()
                    .filter_map(|g| {
                        serde_json::to_value(g)
                            .ok()
                            .and_then(|v| v["gap"].as_str().map(str::to_owned))
                    })
                    .collect();
                println!(
                    "{:<8} {:<20} {}",
                    r.id,
                    crate_state(&r.assurance),
                    gaps.join(", ")
                );
            }
            Ok(true)
        }
        other => Err(format!("unknown command {other}")),
    })();
    match outcome {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("kr0ki-assurance: {e}");
            ExitCode::from(1)
        }
    }
}

fn crate_state(a: &ufo_types::mbse::assurance::Assurance) -> &'static str {
    kr0ki_core::assurance_view::state_name(*a)
}
