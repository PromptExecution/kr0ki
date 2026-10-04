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
        Command::new(env!("CARGO_BIN_EXE_rust-behavior-extractor"))
            .arg("--manifest-path")
            .arg(self.0.join("Cargo.toml"))
            .arg("--output")
            .arg(self.0.join(output))
            .args([
                "--revision",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "--tree-digest",
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "--offline",
            ])
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
fn compiler_facts_preserve_dispatch_control_and_provenance_deterministically() {
    let fixture = Fixture::new();
    let ir = fixture.ir("first.json");
    let second = fixture.ir("second.json");
    assert_eq!(ir, second);
    // Absolute source checkout paths must not influence semantic output.
    let relocated = Fixture::new();
    assert_eq!(ir, relocated.ir("relocated.json"));
    assert!(!ir.validate().iter().any(|d| d.severity == Severity::Error));
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
