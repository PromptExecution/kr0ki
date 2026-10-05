use kr0ki_behavior::{EdgeKind, NodeKind, Resolution, RustBehaviorIr, Severity};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static RUN: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "kr0ki-compiler-{}-{nonce}-{}",
            std::process::id(),
            RUN.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/workspace");
        for path in ["Cargo.toml", "Cargo.lock", "src/lib.rs", "src/worker.rs"] {
            fs::copy(source.join(path), root.join(path)).unwrap();
        }
        Self(root)
    }
    fn extract(&self, output: &str) -> std::process::Output {
        self.extract_with(output, "Cargo.toml", &[])
    }
    fn extract_with(&self, output: &str, manifest: &str, extra: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_rust-behavior-extractor"))
            .arg("--manifest-path")
            .arg(self.0.join(manifest))
            .arg("--output")
            .arg(self.0.join(output))
            .args([
                "--revision",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "--tree-digest",
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "--offline",
            ])
            .args(extra)
            .output()
            .unwrap()
    }
    fn ir(&self, output: &str) -> RustBehaviorIr {
        let result = self.extract(output);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        RustBehaviorIr::from_json(&fs::read_to_string(self.0.join(output)).unwrap()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn duplicate_crate_names_preserve_shards_and_fail_closed_on_identity_conflicts() {
    let fixture = Fixture::new();
    fs::write(
        fixture.0.join("Cargo.toml"),
        "[workspace]\nmembers = [\"a\", \"b\"]\nresolver = \"2\"\n",
    )
    .unwrap();
    fs::write(fixture.0.join("Cargo.lock"), "version = 4\n\n[[package]]\nname = \"package-a\"\nversion = \"0.1.0\"\n\n[[package]]\nname = \"package-b\"\nversion = \"0.1.0\"\n").unwrap();
    for package in ["a", "b"] {
        let directory = fixture.0.join(package);
        fs::create_dir_all(directory.join("src")).unwrap();
        fs::write(directory.join("Cargo.toml"), format!("[package]\nname = \"package-{package}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[lib]\nname = \"shared_name\"\n")).unwrap();
        fs::write(
            directory.join("src/lib.rs"),
            "pub fn shared() -> u32 { 1 }\n",
        )
        .unwrap();
    }
    let result = fixture.extract("conflict.json");
    assert!(
        !result.status.success(),
        "same-named crates must not silently overwrite compiler shards"
    );
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("conflicting node shard"),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!fixture.0.join("conflict.json").exists());
}

#[test]
fn compiler_facts_preserve_dispatch_control_and_provenance_deterministically() {
    let fixture = Fixture::new();
    let ir = fixture.ir("first.json");
    let second = fixture.ir("second.json");
    assert_eq!(ir, second);
    // Absolute source checkout paths must not influence semantic output.
    let relocated = Fixture::new();
    assert_eq!(ir, relocated.ir("relocated.json"));
    assert!(!ir.validate().iter().any(|d| d.severity == Severity::Error));
    for (suffix, left, right) in [
        ("namespaces::Same", NodeKind::Type, NodeKind::Function),
        ("namespaces::Same::x", NodeKind::Field, NodeKind::Function),
        (
            "namespaces::Associated::item",
            NodeKind::AssociatedType,
            NodeKind::Function,
        ),
        ("namespaces::nested::bb0", NodeKind::Type, NodeKind::Exit),
    ] {
        let same_name: Vec<_> = ir
            .nodes
            .iter()
            .filter(|n| n.name.ends_with(suffix))
            .collect();
        let left_node = same_name
            .iter()
            .find(|n| n.kind == left)
            .unwrap_or_else(|| panic!("missing {left:?} {suffix}: {same_name:?}"));
        let right_node = same_name
            .iter()
            .find(|n| n.kind == right)
            .unwrap_or_else(|| panic!("missing {right:?} {suffix}: {same_name:?}"));
        assert_ne!(
            left_node.id, right_node.id,
            "distinct Rust namespaces must retain distinct facts"
        );
    }
    assert!(ir.edges.iter().any(|edge| edge.kind == EdgeKind::Calls
        && ir.nodes.iter().any(|node| node.id == edge.to
            && node.kind == NodeKind::Function
            && node.name == "behavior_fixture::namespaces::Same")));
    assert!(ir
        .nodes
        .iter()
        .find(|n| n.name == "behavior_fixture::Step")
        .unwrap()
        .annotations
        .iter()
        .any(|a| a.text == "#[doc = \"Reviewed Step stereotype\"]"));
    assert!(
        ir.nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Trait && n.name.starts_with("core::"))
            .all(|n| n.annotations.is_empty()),
        "external trait references must not inherit local declaration attributes"
    );
    let counter = ir
        .nodes
        .iter()
        .find(|n| n.name == "behavior_fixture::Counter")
        .unwrap();
    let definition = counter.anchor.as_ref().unwrap();
    let original = &ir
        .sources
        .iter()
        .find(|s| s.path == definition.file)
        .unwrap()
        .content;
    assert!(
        original[definition.start as usize..definition.end as usize]
            .starts_with("pub struct Counter"),
        "a type declaration must not retain an earlier impl/reference placeholder anchor"
    );
    assert!(counter
        .annotations
        .iter()
        .any(|a| a.path == "derive" && a.text == "#[derive(Debug)]"));
    assert!(counter
        .annotations
        .iter()
        .any(|a| a.path == "doc" && a.text.contains("π")));
    assert!(!ir.nodes.iter().any(|n| n.name.contains("ExcludedCounter")));
    let root = ir
        .nodes
        .iter()
        .find(|n| n.name == "behavior_fixture")
        .unwrap();
    assert_eq!(
        root.annotations
            .iter()
            .map(|a| a.text.as_str())
            .collect::<Vec<_>>(),
        vec!["#![allow(dead_code)]"]
    );
    let inline = ir
        .nodes
        .iter()
        .find(|n| n.name == "behavior_fixture::inline")
        .unwrap();
    assert_eq!(
        inline
            .annotations
            .iter()
            .map(|a| a.text.as_str())
            .collect::<Vec<_>>(),
        vec!["#[allow(dead_code)]"]
    );
    let inner = ir
        .nodes
        .iter()
        .find(|n| n.name == "behavior_fixture::inline::Inner")
        .unwrap();
    assert!(inner
        .annotations
        .iter()
        .any(|a| a.text == "#[derive(Debug)]"));
    for kind in [
        NodeKind::Field,
        NodeKind::AssociatedType,
        NodeKind::Decision,
        NodeKind::Merge,
        NodeKind::Loop,
        NodeKind::Exit,
    ] {
        assert!(ir.nodes.iter().any(|n| n.kind == kind), "missing {kind:?}");
    }
    let name = |id: &str| ir.nodes.iter().find(|n| n.id == id).unwrap().name.as_str();
    assert!(ir.edges.iter().any(|e| e.kind == EdgeKind::Calls
        && name(&e.from).starts_with("behavior_fixture::concrete::bb")
        && name(&e.to) == "behavior_fixture::<Counter as Step>::step"
        && e.resolution == Resolution::Resolved));
    assert!(ir.edges.iter().any(|e| e.kind == EdgeKind::Calls
        && name(&e.from).starts_with("behavior_fixture::cross_file::bb")
        && name(&e.to) == "behavior_fixture::worker::run"
        && e.resolution == Resolution::Resolved));
    for caller in ["dynamic", "generic", "callback"] {
        assert!(ir.edges.iter().any(|e| e.kind == EdgeKind::Calls
            && name(&e.from).starts_with(&format!("behavior_fixture::{caller}::bb"))
            && e.resolution == Resolution::Unresolved));
    }
    assert!(ir.edges.iter().any(|e| e.kind == EdgeKind::Requires
        && name(&e.from) == "behavior_fixture::Advanced"
        && name(&e.to) == "behavior_fixture::Step"));
    assert!(ir.edges.iter().any(|e| e.kind == EdgeKind::GovernedBy
        && name(&e.from) == "behavior_fixture::generic"
        && name(&e.to) == "behavior_fixture::Step"));
    assert!(ir.edges.iter().any(|e| e.kind == EdgeKind::Satisfies
        && name(&e.from) == "behavior_fixture::Generic"
        && e.guard.is_some()));
    for code in [
        "try_boundary",
        "async_await",
        "async_suspend",
        "control_break",
        "control_continue",
        "control_return",
        "recursive_call",
        "macro_expansion",
    ] {
        assert!(
            ir.diagnostics.iter().any(|d| d.code == code),
            "missing {code}"
        );
    }
    assert!(!ir.diagnostics.iter().any(|d| d.code == "try_boundary"
        && d.anchor
            .as_ref()
            .unwrap()
            .symbol
            .contains("literal_question")));
    assert!(ir.diagnostics.iter().any(|d| d.code == "recursive_call"
        && d.anchor
            .as_ref()
            .is_some_and(|a| a.symbol == "behavior_fixture::recursive")));
    assert!(ir.diagnostics.iter().any(|d| d.code == "macro_expansion"
        && d.anchor
            .as_ref()
            .is_some_and(|a| a.symbol == "behavior_fixture::generated")));
    assert!(ir.sources.iter().any(|s| s.path == "src/worker.rs"));
    let bounded = ir
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Loop && n.name.starts_with("behavior_fixture::worker::run"))
        .unwrap();
    assert!(ir
        .edges
        .iter()
        .any(|e| e.from == bounded.id && e.kind == EdgeKind::Exit));
    assert!(ir.validate().iter().any(|d| d.code == "unknown_loop_exit"
        && d.anchor.as_ref().unwrap().symbol == "behavior_fixture::infinite"));
}

#[test]
fn ancestor_cargo_configuration_changes_real_compiler_cfg_and_provenance() {
    let parent = Fixture::new();
    let nested = parent.0.join("nested");
    fs::create_dir_all(nested.join("src")).unwrap();
    for path in ["Cargo.toml", "Cargo.lock", "src/lib.rs", "src/worker.rs"] {
        fs::copy(parent.0.join(path), nested.join(path)).unwrap();
    }
    let fixture = Fixture(nested);
    let before = fixture.ir("without-config.json");
    assert!(!before
        .nodes
        .iter()
        .any(|n| n.name == "behavior_fixture::ConfigSelected"));
    fs::create_dir_all(parent.0.join(".cargo")).unwrap();
    fs::write(parent.0.join(".cargo/config.toml"), "[build]\nrustflags = ['--cfg=annotation_fixture', '--check-cfg=cfg(annotation_fixture)']\n").unwrap();
    let after = fixture.ir("with-config.json");
    let selected = after
        .nodes
        .iter()
        .find(|n| n.name == "behavior_fixture::ConfigSelected")
        .unwrap();
    assert!(selected
        .annotations
        .iter()
        .any(|a| a.path == "derive" && a.text == "#[derive(Debug)]"));
    assert_ne!(before.provenance.config, after.provenance.config);
    assert!(after
        .provenance
        .config
        .values()
        .all(|value| !value.contains(parent.0.to_str().unwrap())));
    fs::write(
        parent.0.join(".cargo/config.toml"),
        "[build]\nrustflags = ['--check-cfg=cfg(annotation_fixture)']\n",
    )
    .unwrap();
    let changed = fixture.ir("changed-config.json");
    assert!(!changed
        .nodes
        .iter()
        .any(|n| n.name == "behavior_fixture::ConfigSelected"));
    assert_ne!(after.provenance.config, changed.provenance.config);
}

#[test]
fn original_bom_crlf_unicode_bytes_are_preserved_and_type_errors_publish_nothing() {
    let fixture = Fixture::new();
    let path = fixture.0.join("src/lib.rs");
    let content = format!(
        "\u{feff}{}",
        fs::read_to_string(&path).unwrap().replace('\n', "\r\n")
    );
    fs::write(&path, &content).unwrap();
    let ir = fixture.ir("normalized.json");
    assert_eq!(
        ir.sources
            .iter()
            .find(|s| s.path == "src/lib.rs")
            .unwrap()
            .content,
        content
    );
    let function = ir
        .nodes
        .iter()
        .find(|n| n.name == "behavior_fixture::literal_question")
        .unwrap();
    let anchor = function.anchor.as_ref().unwrap();
    assert!(content[anchor.start as usize..anchor.end as usize].contains("literal_question"));
    let counter = ir
        .nodes
        .iter()
        .find(|n| n.name == "behavior_fixture::Counter")
        .unwrap();
    for annotation in &counter.annotations {
        assert_eq!(
            &content[annotation.anchor.start as usize..annotation.anchor.end as usize],
            annotation.text
        );
    }
    fs::write(
        &path,
        format!("{content}\npub fn invalid() {{ missing_compiler_symbol(); }}\n"),
    )
    .unwrap();
    let result = fixture.extract("invalid.json");
    assert!(!result.status.success());
    assert!(!fixture.0.join("invalid.json").exists());
}

fn shared_workspace() -> Fixture {
    let fixture = Fixture::new();
    fs::write(
        fixture.0.join("Cargo.toml"),
        "[workspace]\nmembers = [\"a\", \"b\"]\nresolver = \"2\"\n",
    )
    .unwrap();
    fs::write(fixture.0.join("Cargo.lock"), "version = 4\n\n[[package]]\nname = \"a\"\nversion = \"0.1.0\"\n\n[[package]]\nname = \"b\"\nversion = \"0.1.0\"\ndependencies = [\"a\"]\n").unwrap();
    for package in ["a", "b"] {
        let directory = fixture.0.join(package);
        fs::create_dir_all(directory.join("src")).unwrap();
        let dependency = if package == "b" {
            "[dependencies]\na = { path = \"../a\" }\n"
        } else {
            ""
        };
        fs::write(directory.join("Cargo.toml"), format!("[package]\nname = \"{package}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n{dependency}")).unwrap();
    }
    fs::write(fixture.0.join("a/src/lib.rs"), "#[doc = \"Reviewed shared trait\"]\npub trait Shared {}\npub fn first<T: Shared + core::fmt::Debug>() {}\n").unwrap();
    fs::write(
        fixture.0.join("b/src/lib.rs"),
        "pub fn second<T: a::Shared + core::fmt::Debug>() {}\n",
    )
    .unwrap();
    fixture
}

#[test]
fn shared_traits_merge_declarations_and_keep_all_reference_evidence() {
    let fixture = shared_workspace();
    let ir = fixture.ir("first.json");
    assert_eq!(ir, fixture.ir("second.json"));
    assert_eq!(ir, shared_workspace().ir("relocated.json"));
    for name in [
        "core::std::marker::Sized",
        "core::std::fmt::Debug",
        "a::Shared",
    ] {
        let traits: Vec<_> = ir
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Trait && n.name == name)
            .collect();
        assert_eq!(
            traits.len(),
            1,
            "{name}: {:?}",
            ir.nodes
                .iter()
                .filter(|n| n.kind == NodeKind::Trait)
                .map(|n| &n.name)
                .collect::<Vec<_>>()
        );
        let trait_node = traits[0];
        let evidence: Vec<_> = ir
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::GovernedBy && e.to == trait_node.id)
            .collect();
        for file in ["a/src/lib.rs", "b/src/lib.rs"] {
            assert!(
                evidence.iter().any(|e| e.anchor.file == file),
                "lost {name} reference in {file}"
            );
        }
        if name == "a::Shared" {
            let anchor = trait_node.anchor.as_ref().unwrap();
            assert_eq!(anchor.file, "a/src/lib.rs");
            let source = ir.sources.iter().find(|s| s.path == anchor.file).unwrap();
            assert!(source.content[anchor.start as usize..anchor.end as usize]
                .starts_with("pub trait Shared"));
            assert!(trait_node
                .annotations
                .iter()
                .any(|a| a.text == "#[doc = \"Reviewed shared trait\"]"));
        } else {
            assert!(trait_node.annotations.is_empty());
        }
    }
    let wire = ir.canonical_json().unwrap();
    assert!(!wire.contains("confirmed_definitions"));
}

#[test]
fn member_manifest_extracts_siblings_and_incomplete_root_preserves_output() {
    let fixture = shared_workspace();
    let result = fixture.extract_with("member.json", "a/Cargo.toml", &[]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let bytes = fs::read(fixture.0.join("member.json")).unwrap();
    let ir = RustBehaviorIr::from_json(std::str::from_utf8(&bytes).unwrap()).unwrap();
    for file in ["a/src/lib.rs", "b/src/lib.rs"] {
        assert!(ir.sources.iter().any(|s| s.path == file));
    }
    assert_eq!(ir.provenance.config["manifest"], "a/Cargo.toml");
    // A build script makes the before-compilation rejection observable.
    fs::write(fixture.0.join("a/build.rs"), "fn main() { std::fs::write(std::env::var(\"CARGO_MANIFEST_DIR\").unwrap() + \"/compiled\", \"ran\").unwrap(); }\n").unwrap();
    let root = fixture.0.join("a");
    let result = fixture.extract_with(
        "member.json",
        "a/Cargo.toml",
        &["--workspace-root", root.to_str().unwrap()],
    );
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("excludes workspace member"),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(bytes, fs::read(fixture.0.join("member.json")).unwrap());
    assert!(
        !root.join("compiled").exists(),
        "incomplete root must fail before compilation"
    );
    assert!(!fs::read_dir(&fixture.0).unwrap().any(|e| e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".extract-")));
}
