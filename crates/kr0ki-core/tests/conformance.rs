//! Phase 1 SysML-v2 conformance harness.
//!
//! Gates kr0ki's SysML-v2 handling against the OMG
//! [SysML-v2-Release](https://github.com/Systems-Modeling/SysML-v2-Release)
//! model corpus by **consuming** the `sysml-v2-parser` crate (pure-Rust `nom`
//! parser). kr0ki does not reimplement a parser; this suite proves the parser
//! we depend on still accepts the corpus kr0ki's lowering layer will build on.
//!
//! Corpus provenance: EPL-2.0, fetched on demand by
//! `scripts/fetch-sysml-v2-release.sh`, never vendored. Pinned tag lives in
//! `docs/conformance-target.toml`. See `docs/CONFORMANCE.md`.
//!
//! Env-gated exactly like `live_render.rs`: with no corpus present these tests
//! compile, no-op, and "pass", so the fast CI `check` job is unaffected. The
//! dedicated `conformance` CI job fetches the corpus and runs them:
//!
//! ```text
//! bash scripts/fetch-sysml-v2-release.sh
//! cargo test -p kr0ki-core --test conformance -- --ignored --nocapture
//! ```
//!
//! `parse()` entry point (sysml-v2-parser 0.55):
//!   `sysml_v2_parser::parse(input: &str) -> Result<ParsedDocument, ParseError>`
//! strict / all-or-nothing (rejects unconsumed input). We deliberately use it
//! over the resilient `parse_for_editor()` — a conformance gate wants the hard
//! verdict. `ParseError`'s `Display` already carries "at line L, column C".
//!
//! ## Phase 2 (future, documented in docs/CONFORMANCE.md)
//! Upstream a kr0ki-adapter fixture set (iso_ir / Mermaid / D2 lowering golden
//! outputs) into `sysml-v2-parser`'s own scorecard once the FR1 adapter exists,
//! so the parser crate owns the shared corpus and kr0ki owns only its lowering
//! deltas.

use std::fs;
use std::path::{Path, PathBuf};

/// Committed pass-baseline for `sysml/src/validation/` (56 curated positive
/// fixtures, the primary gate). A run below this fails CI (regression in the
/// pinned `sysml-v2-parser`); a run above it is a prompt to raise the constant.
/// Mirrors `sysml-v2-parser`'s own `ROUNDTRIP_PASS` ratchet discipline.
const BASELINE: usize = 56;

/// Committed pass-baseline for the normative standard library (`sysml.library/`,
/// ~94 `.kerml`/`.sysml` files). Same ratchet discipline as [`BASELINE`].
const LIB_BASELINE: usize = 94;

// ---------------------------------------------------------------------------
// corpus discovery (env-gated, same skip pattern as live_render.rs)
// ---------------------------------------------------------------------------

/// Resolve the SysML-v2-Release corpus root, or `None` if the suite should skip.
///
/// - `KR0KI_SYSML_V2_RELEASE_DIR` (if set) wins unconditionally.
/// - otherwise the fetch script's default `<repo>/.sysml-v2-release`, but only
///   when its `.fetched-<tag>` stamp is present (task's skip contract:
///   "env unset AND stamp absent -> skip").
fn corpus_root() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("KR0KI_SYSML_V2_RELEASE_DIR") {
        return Some(PathBuf::from(dir));
    }
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    // `cargo test` CWD is the crate dir; the fetch script / justfile / CI all
    // run from the repo root, so the corpus lands two levels up.
    [
        manifest_dir.join("../../.sysml-v2-release"),
        PathBuf::from(".sysml-v2-release"),
    ]
    .into_iter()
    .find(|candidate| has_fetch_stamp(candidate))
}

fn has_fetch_stamp(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    entries
        .filter_map(Result::ok)
        .any(|e| e.file_name().to_string_lossy().starts_with(".fetched-"))
}

/// `eprintln!` a skip notice and hand back the resolved root, or `None`.
fn corpus_or_skip() -> Option<PathBuf> {
    match corpus_root() {
        Some(root) => Some(root),
        None => {
            eprintln!(
                "skipping: run scripts/fetch-sysml-v2-release.sh \
                 (or set KR0KI_SYSML_V2_RELEASE_DIR to a SysML-v2-Release checkout)"
            );
            None
        }
    }
}

fn find_files_with_ext(dir: &Path, exts: &[&str], out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            find_files_with_ext(&path, exts, out);
        } else if path
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|e| exts.contains(&e))
        {
            out.push(path);
        }
    }
}

fn collect(dir: &Path, exts: &[&str]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    find_files_with_ext(dir, exts, &mut out);
    out.sort();
    out
}

/// Run `f` on a worker thread with a large stack.
///
/// `sysml-v2-parser` 0.55 is a recursive-descent `nom` parser; a handful of
/// deeply-nested `sysml/src/examples/` models blow the default 2 MiB test-thread
/// stack (observed: SIGABRT "stack overflow"). Baking the large stack into the
/// test keeps `cargo test` working without a `RUST_MIN_STACK` incantation.
fn with_big_stack<T, F>(f: F) -> T
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    std::thread::Builder::new()
        .name("conformance-parse".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(f)
        .expect("spawn big-stack worker")
        .join()
        .expect("big-stack worker panicked")
}

// ---------------------------------------------------------------------------
// scorecard
// ---------------------------------------------------------------------------

struct Scorecard {
    label: String,
    total: usize,
    passed: usize,
    /// `(relative path, first diagnostic)` for each fixture `parse()` rejected.
    failures: Vec<(String, String)>,
}

impl Scorecard {
    fn run(label: &str, root: &Path, files: &[PathBuf]) -> Self {
        let label = label.to_string();
        let root = root.to_path_buf();
        let files = files.to_vec();
        with_big_stack(move || {
            let mut card = Scorecard {
                label,
                total: files.len(),
                passed: 0,
                failures: Vec::new(),
            };
            for file in &files {
                let rel = file
                    .strip_prefix(&root)
                    .unwrap_or(file)
                    .to_string_lossy()
                    .replace('\\', "/");
                let text = match fs::read_to_string(file) {
                    Ok(t) => t,
                    Err(e) => {
                        card.failures.push((rel, format!("read error: {e}")));
                        continue;
                    }
                };
                // `parse()` is strict: Ok == fully consumed, well-formed syntax.
                match sysml_v2_parser::parse(&text) {
                    Ok(_) => card.passed += 1,
                    Err(e) => card.failures.push((rel, e.to_string())),
                }
            }
            card
        })
    }

    fn print(&self) {
        eprintln!(
            "\n=== conformance scorecard: {} — {}/{} passed \
             (sysml-v2-parser {PARSER_PIN}) ===",
            self.label, self.passed, self.total,
        );
        for (rel, diag) in &self.failures {
            let diag = diag.lines().next().unwrap_or(diag);
            let diag: String = diag.chars().take(200).collect();
            eprintln!("  FAIL {rel}: {diag}");
        }
    }
}

/// The `sysml-v2-parser` version this gate is pinned against — kept in step with
/// the `crates/kr0ki-core/Cargo.toml` dev-dependency and `docs/conformance-target.toml`.
const PARSER_PIN: &str = "0.55";

// ---------------------------------------------------------------------------
// gated tests
// ---------------------------------------------------------------------------

/// Primary gate: every `.sysml` under `sysml/src/validation/` must parse.
#[test]
#[ignore = "needs SysML-v2-Release corpus; run scripts/fetch-sysml-v2-release.sh then --ignored"]
fn validation_corpus_parses() {
    let Some(root) = corpus_or_skip() else {
        return;
    };
    let dir = root.join("sysml/src/validation");
    let files = collect(&dir, &["sysml"]);
    assert!(
        !files.is_empty(),
        "no .sysml fixtures under {} — corpus layout changed?",
        dir.display()
    );

    let card = Scorecard::run("sysml/src/validation", &root, &files);
    card.print();

    assert!(
        card.passed >= BASELINE,
        "validation-corpus regression: {}/{} passed, below committed BASELINE = {}. \
         A pinned-parser regression — investigate before lowering the baseline.",
        card.passed,
        card.total,
        BASELINE
    );
    if card.passed > BASELINE {
        eprintln!(
            "note: {} validation fixtures now parse (BASELINE = {}). \
             Raise BASELINE in tests/conformance.rs to ratchet.",
            card.passed, BASELINE
        );
    }
}

/// The normative standard library (`sysml.library/`) is the resolution prelude
/// for anything past pure syntax; gate that it parses.
#[test]
#[ignore = "needs SysML-v2-Release corpus; run scripts/fetch-sysml-v2-release.sh then --ignored"]
fn standard_library_parses() {
    let Some(root) = corpus_or_skip() else {
        return;
    };
    let dir = root.join("sysml.library");
    let files = collect(&dir, &["kerml", "sysml"]);
    assert!(
        !files.is_empty(),
        "no .kerml/.sysml files under {} — corpus layout changed?",
        dir.display()
    );

    let card = Scorecard::run("sysml.library", &root, &files);
    card.print();

    assert!(
        card.passed >= LIB_BASELINE,
        "standard-library regression: {}/{} passed, below committed LIB_BASELINE = {}.",
        card.passed,
        card.total,
        LIB_BASELINE
    );
    if card.passed > LIB_BASELINE {
        eprintln!(
            "note: {} standard-library files now parse (LIB_BASELINE = {}). \
             Raise LIB_BASELINE in tests/conformance.rs to ratchet.",
            card.passed, LIB_BASELINE
        );
    }
}

/// Non-asserting informational scorecard for the noisier `sysml/src/examples/`
/// and `kerml/src/examples/` trees. Printed only — not a gate (yet). Phase 2
/// promotes the useful part of this into `sysml-v2-parser`'s own scorecard.
#[test]
#[ignore = "needs SysML-v2-Release corpus; run scripts/fetch-sysml-v2-release.sh then --ignored"]
fn examples_scorecard_informational() {
    let Some(root) = corpus_or_skip() else {
        return;
    };

    let sysml_examples = collect(&root.join("sysml/src/examples"), &["sysml"]);
    if !sysml_examples.is_empty() {
        Scorecard::run("sysml/src/examples (informational)", &root, &sysml_examples).print();
    }

    let kerml_examples = collect(&root.join("kerml/src/examples"), &["kerml"]);
    if !kerml_examples.is_empty() {
        Scorecard::run("kerml/src/examples (informational)", &root, &kerml_examples).print();
    }

    eprintln!("\n(examples/ scorecards are informational — no assertion)");
}
