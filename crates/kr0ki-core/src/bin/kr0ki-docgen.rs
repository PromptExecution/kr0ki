//! Local artifact boundary. It consumes compiler evidence, never executes Rust.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use kr0ki_behavior::{
    Anchor, Edge, EdgeKind, Node, NodeKind, Resolution, RustBehaviorIr, SourceFile, StateMachine,
};
use kr0ki_core::{
    cache::{FsCache, OutputKind},
    format::DiagramFormat,
    render::HttpKrokiBackend,
    rust_behavior::{self, BehaviorRequest},
    RenderService,
};

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let command = args.next().context("usage: kr0ki-docgen validate|bundle --input IR.json [--output DIRECTORY] [--view VIEW.json] [--ontology MAPPINGS.json] [--expand-depth 0..8] [--strict] [--state-machine TABLE.json] [--backend URL]")?;
    let mut options = BTreeMap::new();
    let mut strict = false;
    while let Some(flag) = args.next() {
        if flag == "--strict" {
            if strict {
                bail!("duplicate option --strict");
            }
            strict = true;
            continue;
        }
        if ![
            "--input",
            "--output",
            "--view",
            "--expand-depth",
            "--state-machine",
            "--ontology",
            "--backend",
        ]
        .contains(&flag.as_str())
        {
            bail!("unknown option {flag}");
        }
        let value = args
            .next()
            .with_context(|| format!("missing value for {flag}"))?;
        if options.insert(flag.clone(), value).is_some() {
            bail!("duplicate option {flag}");
        }
    }
    let input = options.get("--input").context("--input is required")?;
    let mut model = RustBehaviorIr::from_json(&std::fs::read_to_string(input)?)?;
    if let Some(path) = options.get("--state-machine") {
        add_machine(&mut model, Path::new(path))?;
    }
    let expand_depth = options
        .get("--expand-depth")
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(0);
    let view = options
        .get("--view")
        .map(|p| -> Result<_> { Ok(serde_json::from_str(&std::fs::read_to_string(p)?)?) })
        .transpose()?;
    let prepared = rust_behavior::prepare(BehaviorRequest {
        model: model.clone(),
        stereotypes: options
            .get("--ontology")
            .map(|p| -> Result<_> { Ok(serde_json::from_str(&std::fs::read_to_string(p)?)?) })
            .transpose()?
            .unwrap_or_default(),
        view,
        expand_depth,
        strict,
    })?;
    if command == "validate" {
        println!("{}", serde_json::to_string_pretty(&prepared.validation)?);
        return Ok(());
    }
    if command != "bundle" {
        bail!("unknown command {command}");
    }
    let output = PathBuf::from(
        options
            .get("--output")
            .context("--output is required for bundle")?,
    );
    if output.exists() && std::fs::read_dir(&output)?.next().is_some() {
        bail!("bundle output must be an empty directory");
    }
    std::fs::create_dir_all(&output)?;
    write(&output, "ir.json", model.canonical_json()?.as_bytes())?;
    write(
        &output,
        "schema.json",
        serde_json::to_string_pretty(&kr0ki_behavior::json_schema())?.as_bytes(),
    )?;
    write(
        &output,
        "graph.json",
        &serde_json::to_vec_pretty(&prepared.graph)?,
    )?;
    write(&output, "view.json", &serde_json::to_vec_pretty(&prepared)?)?;
    write(&output, "diagram.d2", prepared.d2.as_bytes())?;
    write(&output, "diagram.mmd", prepared.mermaid.as_bytes())?;
    write(&output, "model.sysml", prepared.sysml.as_bytes())?;
    write(
        &output,
        "lint.json",
        &serde_json::to_vec_pretty(&model.validate())?,
    )?;
    let mut validation = prepared.validation.clone();
    for (id, text) in &prepared.scxml {
        write(
            &output,
            &format!("machine-{}.scxml", rust_behavior::hex_digest(id.as_bytes())),
            text.as_bytes(),
        )?;
    }
    if let Some(backend) = options.get("--backend") {
        let service = RenderService::new(
            HttpKrokiBackend::new(backend),
            FsCache::new(output.join("render-cache")),
        );
        for kind in [OutputKind::Svg, OutputKind::Png] {
            let rendered = service
                .render_model(
                    &format!("behavior-{}", kind.ext()),
                    DiagramFormat::D2,
                    kind,
                    &prepared.d2,
                    &prepared.content_hash,
                    rust_behavior::RULE_VERSION,
                )
                .await?;
            write(&output, &format!("diagram.{}", kind.ext()), &rendered.bytes)?;
        }
        for report in &mut validation {
            if report.notation == "d2" {
                report.level = "renderer-parser".into();
                report.validator = "private Kroki D2 renderer".into();
            }
        }
        // Cache is a build-stage working directory, not part of the bundle.
        std::fs::remove_dir_all(output.join("render-cache"))?;
    }
    write(
        &output,
        "validation.json",
        &serde_json::to_vec_pretty(&validation)?,
    )?;
    let mut digests = BTreeMap::new();
    for entry in std::fs::read_dir(&output)? {
        let entry = entry?;
        if entry.file_type()?.is_file() && entry.file_name() != "semantic-manifest.json" {
            digests.insert(
                entry.file_name().to_string_lossy().into_owned(),
                rust_behavior::hex_digest(&std::fs::read(entry.path())?),
            );
        }
    }
    let manifest = serde_json::json!({
        "schema_version": 1, "provenance": model.provenance,
        "content_hash": prepared.content_hash, "rule_version": rust_behavior::RULE_VERSION,
        "rule_digest": rust_behavior::hex_digest(include_bytes!("../rust_behavior.rs")),
        "schema_digest": rust_behavior::hex_digest(&serde_json::to_vec(&kr0ki_behavior::json_schema())?),
        "files": digests,
    });
    write(
        &output,
        "semantic-manifest.json",
        &serde_json::to_vec_pretty(&manifest)?,
    )?;
    println!("{}", prepared.content_hash);
    Ok(())
}

fn write(output: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    std::fs::write(output.join(name), bytes).with_context(|| format!("write {name}"))
}

fn add_machine(model: &mut RustBehaviorIr, path: &Path) -> Result<()> {
    let content = std::fs::read_to_string(path)?;
    let mut machine: StateMachine = serde_json::from_str(&content)?;
    let file = format!(
        "state-machines/{}.json",
        rust_behavior::hex_digest(machine.id.as_bytes())
    );
    let anchor = Anchor {
        file: file.clone(),
        symbol: machine.id.clone(),
        start: 0,
        end: content.len().try_into()?,
    };
    model.provenance.config.insert(
        format!("source_origin:{file}"),
        "declared-state-machine".into(),
    );
    model.sources.push(SourceFile {
        path: file,
        sha256: kr0ki_behavior::digest(&content),
        content,
    });
    model.nodes.push(Node {
        id: machine.id.clone(),
        name: machine.name.clone(),
        kind: NodeKind::Module,
        anchor: Some(anchor.clone()),
    });
    for state in &mut machine.states {
        state.anchor.get_or_insert_with(|| anchor.clone());
        let id = format!("{}::{}", machine.id, state.id);
        model.nodes.push(Node {
            id: id.clone(),
            name: state.id.clone(),
            kind: NodeKind::State,
            anchor: state.anchor.clone(),
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
        transition.anchor.get_or_insert_with(|| anchor.clone());
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
            anchor: transition.anchor.clone().expect("anchor inserted"),
        });
    }
    model.machines.push(machine);
    model.normalize();
    model.ensure_valid()?;
    Ok(())
}
