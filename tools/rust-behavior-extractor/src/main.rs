#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_span;

mod compiler;
mod config;
mod shard;

struct RunDirectory(PathBuf);
impl Drop for RunDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

use kr0ki_behavior::{digest, Provenance, RustBehaviorIr, SCHEMA_VERSION};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, env, fs, path::PathBuf, process::Command};

const TOOLCHAIN: &str = "nightly-2026-06-16";
const COMPILER: &str = "01dfd79246f1b2d5f146616deff08223a840a9ae";

fn main() {
    if let Err(error) = run() {
        eprintln!("rust-behavior-extractor: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    // Cargo's wrapper protocol supplies the actual rustc executable first.
    if args.get(1).is_some_and(|a| a.ends_with("rustc")) {
        if env::var_os("KR0KI_BEHAVIOR_SHARDS").is_none()
            || !args.iter().any(|a| a == "--crate-name")
            || args
                .iter()
                .any(|a| a == "--print" || a.starts_with("--print="))
        {
            let status = Command::new(&args[1]).args(&args[2..]).status()?;
            std::process::exit(status.code().unwrap_or(1));
        }
        let mut callbacks = compiler::Extractor::new()?;
        let mut compiler_args = args[1..].to_vec();
        compiler_args.push("-Zmir-opt-level=0".into());
        compiler_args.push("--remap-path-prefix".into());
        compiler_args.push(format!("{}=workspace", env::var("KR0KI_BEHAVIOR_ROOT")?));
        rustc_driver::run_compiler(&compiler_args, &mut callbacks);
        return Ok(());
    }
    let mut options = BTreeMap::new();
    let mut iter = args[1..].iter();
    while let Some(flag) = iter.next() {
        if flag == "--help" {
            println!("rust-behavior-extractor --manifest-path PATH --output FILE --revision SHA [--tree-digest DIGEST] [--workspace-root DIR] [--features LIST] [--all-features] [--no-default-features] [--offline]");
            return Ok(());
        }
        if matches!(
            flag.as_str(),
            "--all-features" | "--no-default-features" | "--offline"
        ) {
            if options.insert(flag.clone(), "true".to_owned()).is_some() {
                return Err(format!("duplicate option {flag}").into());
            }
        } else if matches!(
            flag.as_str(),
            "--manifest-path"
                | "--output"
                | "--revision"
                | "--tree-digest"
                | "--workspace-root"
                | "--features"
                | "--target"
        ) {
            if options
                .insert(
                    flag.clone(),
                    iter.next().ok_or("missing option value")?.clone(),
                )
                .is_some()
            {
                return Err(format!("duplicate option {flag}").into());
            }
        } else {
            return Err(format!("unknown option {flag}").into());
        }
    }
    let manifest = fs::canonicalize(
        options
            .get("--manifest-path")
            .ok_or("--manifest-path required")?,
    )?;
    let revision = options
        .get("--revision")
        .ok_or("--revision required")?
        .clone();
    if !matches!(revision.len(), 40 | 64) || !revision.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("--revision must be a full git commit hash".into());
    }
    for key in [
        "RUSTC",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "CARGO_BUILD_RUSTC",
        "CARGO_BUILD_RUSTC_WRAPPER",
        "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    ] {
        if env::var_os(key).is_some() {
            return Err(
                format!("unset {key}: the extractor requires its pinned compiler wrapper").into(),
            );
        }
    }
    let output = PathBuf::from(options.get("--output").ok_or("--output required")?);
    let output = if output.is_absolute() {
        output
    } else {
        env::current_dir()?.join(output)
    };
    let mut metadata_command = Command::new("rustup");
    metadata_command
        .args([
            "run",
            TOOLCHAIN,
            "cargo",
            "metadata",
            "--locked",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(&manifest)
        .current_dir(manifest.parent().unwrap());
    if options.contains_key("--offline") {
        metadata_command.arg("--offline");
    }
    let metadata = metadata_command.output()?;
    if !metadata.status.success() {
        return Err(format!(
            "Cargo metadata failed: {}",
            String::from_utf8_lossy(&metadata.stderr)
        )
        .into());
    }
    let metadata: serde_json::Value = serde_json::from_slice(&metadata.stdout)?;
    let cargo_root = fs::canonicalize(PathBuf::from(
        metadata["workspace_root"]
            .as_str()
            .ok_or("Cargo metadata lacks workspace_root")?,
    ))?;
    let root = fs::canonicalize(
        options
            .get("--workspace-root")
            .map(PathBuf::from)
            .unwrap_or_else(|| cargo_root.clone()),
    )?;
    // Reject an incomplete source boundary before any compiler wrapper runs.
    for member in metadata["workspace_members"]
        .as_array()
        .ok_or("Cargo metadata lacks workspace_members")?
    {
        let package = metadata["packages"]
            .as_array()
            .ok_or("Cargo metadata lacks packages")?
            .iter()
            .find(|package| package["id"] == *member)
            .ok_or("workspace member package is absent")?;
        let mut paths = vec![package["manifest_path"]
            .as_str()
            .ok_or("workspace member lacks manifest_path")?];
        for target in package["targets"]
            .as_array()
            .ok_or("workspace member lacks targets")?
        {
            paths.push(
                target["src_path"]
                    .as_str()
                    .ok_or("workspace target lacks src_path")?,
            );
        }
        for path in paths {
            if !fs::canonicalize(path)?.starts_with(&root) {
                return Err(format!(
                    "workspace root {} excludes workspace member {} ({path}); no IR was published",
                    root.display(),
                    package["name"]
                )
                .into());
            }
        }
    }
    if !manifest.starts_with(&root) {
        return Err("workspace root excludes the selected manifest".into());
    }
    let tree_digest = match options.get("--tree-digest") {
        Some(v) => v.clone(),
        None => {
            let head = Command::new("git")
                .arg("-C")
                .arg(&root)
                .args(["rev-parse", "HEAD"])
                .output()?;
            let status = Command::new("git")
                .arg("-C")
                .arg(&root)
                .args(["status", "--porcelain", "--untracked-files=all"])
                .output()?;
            if !head.status.success()
                || !status.status.success()
                || String::from_utf8(head.stdout)?.trim() != revision
                || !status.stdout.is_empty()
            {
                return Err("source must be a clean checkout of --revision; use --tree-digest only for an externally verified source archive".into());
            }
            let repository = Command::new("git")
                .arg("-C")
                .arg(&root)
                .args(["rev-parse", "--show-toplevel"])
                .output()?;
            if !repository.status.success() {
                return Err("cannot determine repository archive root".into());
            }
            let repository = String::from_utf8(repository.stdout)?;
            let archive = Command::new("git")
                .arg("-C")
                .arg(repository.trim())
                .args(["archive", "--format=tar", &revision])
                .output()?;
            if !archive.status.success() {
                return Err("cannot archive source tree; pass --tree-digest for verified standalone fixtures".into());
            }
            format!("{:x}", Sha256::digest(&archive.stdout))
        }
    };
    if tree_digest.len() != 64 || !tree_digest.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("--tree-digest must be SHA-256".into());
    }
    let version = Command::new("rustup")
        .args(["run", TOOLCHAIN, "rustc", "-vV"])
        .output()?;
    if !version.status.success() {
        return Err("cannot query pinned compiler version".into());
    }
    let version = String::from_utf8(version.stdout)?;
    if !version.contains(COMPILER) {
        return Err("compiler commit differs from pinned extractor ABI".into());
    }
    let mut config = BTreeMap::new();
    let repository = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["rev-parse", "--show-toplevel"])
        .output();
    let source_prefix =
        if let Some(repository) = repository.ok().filter(|result| result.status.success()) {
            let repository = fs::canonicalize(String::from_utf8(repository.stdout)?.trim())?;
            root.strip_prefix(repository)?
                .to_string_lossy()
                .replace('\\', "/")
        } else {
            String::new()
        };
    config.insert("source_prefix".into(), source_prefix);
    config.insert(
        "compiler_host".into(),
        version
            .lines()
            .find_map(|line| line.strip_prefix("host: "))
            .ok_or("compiler version lacks host")?
            .to_owned(),
    );
    config.insert(
        "manifest".into(),
        manifest
            .strip_prefix(&root)?
            .to_string_lossy()
            .replace('\\', "/"),
    );
    config.insert(
        "target_selection".into(),
        "workspace-default-targets".into(),
    );
    config.insert(
        "cargo_lock_sha256".into(),
        format!(
            "{:x}",
            Sha256::digest(fs::read(cargo_root.join("Cargo.lock"))?)
        ),
    );
    let cargo_home = env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")));
    config.extend(config::capture(
        &root,
        cargo_home.as_deref(),
        &env::vars().collect(),
    )?);
    config.insert(
        "mir_phase".into(),
        "typed_mir_built_before_transforms_published_after_analysis".into(),
    );
    for key in [
        "--features",
        "--all-features",
        "--no-default-features",
        "--target",
    ] {
        if let Some(value) = options.get(key) {
            config.insert(key.trim_start_matches('-').to_owned(), value.clone());
        }
    }
    let provenance = Provenance {
        revision,
        tree_digest,
        toolchain: format!("{TOOLCHAIN}:{COMPILER}"),
        extractor: "rustc-driver-v1".into(),
        config,
    };
    fs::create_dir_all(output.parent().unwrap())?;
    // A fresh check directory ensures Cargo invokes every workspace wrapper even on a warm cache.
    // It remains derived data beneath the output's ignored artifact directory.
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let run_dir = output
        .parent()
        .unwrap()
        .join(format!(".extract-{}-{nonce}", std::process::id()));
    fs::create_dir_all(run_dir.join("shards"))?;
    let _cleanup = RunDirectory(run_dir.clone());
    let mut cargo = Command::new("rustup");
    cargo
        .args([
            "run",
            TOOLCHAIN,
            "cargo",
            "check",
            "--locked",
            "--jobs",
            "1",
            "--manifest-path",
        ])
        .arg(&manifest)
        .arg("--workspace")
        .current_dir(&root);
    for key in ["--features", "--target"] {
        if let Some(value) = options.get(key) {
            cargo.arg(key).arg(value);
        }
    }
    for key in ["--all-features", "--no-default-features", "--offline"] {
        if options.contains_key(key) {
            cargo.arg(key);
        }
    }
    cargo
        .env("RUSTC_WORKSPACE_WRAPPER", env::current_exe()?)
        .env("KR0KI_BEHAVIOR_ROOT", &root)
        .env(
            "KR0KI_BEHAVIOR_PROVENANCE",
            serde_json::to_string(&provenance)?,
        )
        .env("KR0KI_BEHAVIOR_SHARDS", run_dir.join("shards"))
        .env("CARGO_TARGET_DIR", run_dir.join("target"));
    if !cargo.status()?.success() {
        return Err("compiler analysis failed; no IR was published".into());
    }
    let mut merged = RustBehaviorIr {
        schema_version: SCHEMA_VERSION,
        provenance,
        sources: vec![],
        nodes: vec![],
        edges: vec![],
        diagnostics: vec![],
        machines: vec![],
    };
    let mut sources = BTreeMap::new();
    let mut nodes = BTreeMap::new();
    let mut edges = BTreeMap::new();
    let mut paths = fs::read_dir(run_dir.join("shards"))?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    for path in paths {
        let shard: shard::CompilerShard = serde_json::from_slice(&fs::read(path)?)?;
        shard.ir.ensure_valid()?;
        let ir = shard.ir;
        if ir.provenance != merged.provenance {
            return Err("compiler shard provenance differs".into());
        }
        for source in ir.sources {
            if let Some(old) = sources.get(&source.path) {
                if old != &source {
                    return Err(format!("conflicting source shard {}", source.path).into());
                }
            }
            sources.insert(source.path.clone(), source);
        }
        for node in ir.nodes {
            let confirmed = shard.confirmed_definitions.contains(&node.id);
            shard::merge_node(&mut nodes, node, confirmed)?;
        }
        for edge in ir.edges {
            if let Some(old) = edges.get(&edge.id) {
                if old != &edge {
                    return Err(format!("conflicting edge shard {}", edge.id).into());
                }
            }
            edges.insert(edge.id.clone(), edge);
        }
        merged.diagnostics.extend(ir.diagnostics);
    }
    merged.sources = sources.into_values().collect();
    merged.nodes = nodes.into_values().map(|(node, _)| node).collect();
    merged.edges = edges.into_values().collect();
    if merged.nodes.is_empty() {
        return Err("compiler produced no workspace facts".into());
    }
    merged.normalize();
    let json = merged.canonical_json()?;
    let temporary = run_dir.join("validated.json");
    fs::write(&temporary, &json)?;
    fs::rename(temporary, &output)?;
    eprintln!("validated IR {} ({})", output.display(), digest(&json));
    Ok(())
}
