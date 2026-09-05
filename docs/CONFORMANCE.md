# SysML-v2 conformance harness

## Phase 1 (this) — consume `sysml-v2-parser`

kr0ki does **not** implement a SysML-v2 parser. Phase 1 gates kr0ki's SysML-v2
handling by **consuming** the [`sysml-v2-parser`](https://crates.io/crates/sysml-v2-parser)
crate (pure-Rust `nom` parser, pinned at `0.55`) against the OMG
[SysML-v2-Release](https://github.com/Systems-Modeling/SysML-v2-Release) model
corpus. If the parser we depend on regresses against the corpus kr0ki's lowering
layer will build on, CI fails.

### What is gated

`crates/kr0ki-core/tests/conformance.rs` (all `#[ignore]`, run on demand / in the
dedicated `conformance` CI job):

| Test | Corpus | Assertion |
| ---- | ------ | --------- |
| `validation_corpus_parses` | `sysml/src/validation/**/*.sysml` (56 curated positive fixtures, 17 topic folders — the primary gate) | `passed >= BASELINE` |
| `standard_library_parses` | `sysml.library/**/*.{kerml,sysml}` (94 files — the normative standard library, resolution prelude for anything past pure syntax) | `passed >= LIB_BASELINE` |
| `examples_scorecard_informational` | `sysml/src/examples/**/*.sysml` + `kerml/src/examples/**/*.kerml` (noisier, less curated) | none — printed scorecard only |

Each fixture is fed to `sysml_v2_parser::parse(&str) -> Result<ParsedDocument, ParseError>`
— the **strict** entry point (rejects unconsumed input), not the resilient
`parse_for_editor()`. A conformance gate wants the hard verdict. `ParseError`'s
`Display` carries `at line L, column C`, which the scorecard prints for each
failure.

Parsing runs on a 256 MiB worker thread: `sysml-v2-parser` 0.55 is
recursive-descent and a few deeply-nested `examples/` models overflow the default
2 MiB test-thread stack.

### Measured baselines (tag `2026-07`, `sysml-v2-parser` 0.55.0)

| Corpus | Result | Committed constant |
| ------ | ------ | ------------------ |
| `sysml/src/validation` | **56 / 56** | `BASELINE = 56` |
| `sysml.library` | **94 / 94** | `LIB_BASELINE = 94` |
| `sysml/src/examples` (informational) | 69 / 96 | — |
| `kerml/src/examples` (informational) | 38 / 58 | — |

The two gated corpora parse in full at this pin. The baseline constants act as a
ratchet, mirroring `sysml-v2-parser`'s own `ROUNDTRIP_PASS` discipline:

- a run **below** baseline fails CI — a regression in the pinned parser;
- a run **above** baseline prints a note prompting you to raise the constant.

### Pinned target and how to bump it

Single source of truth: [`docs/conformance-target.toml`](./conformance-target.toml)
(`release_tag`, `release_repo`, `sysml_v2_parser`).

To move to a newer monthly `YYYY-MM` release:

1. Edit `release_tag` in `docs/conformance-target.toml` (and bump the
   `sysml-v2-parser` dev-dependency / `sysml_v2_parser` pin together if a newer
   parser understands a newer grammar).
2. `just conformance` — re-fetches the corpus and re-runs the gate.
3. Adjust `BASELINE` / `LIB_BASELINE` in `crates/kr0ki-core/tests/conformance.rs`
   to the newly measured numbers (down only with a written reason; up is free).
4. The CI corpus cache key is `hashFiles('docs/conformance-target.toml')`, so it
   invalidates automatically.

### Corpus provenance (EPL-2.0, fetched not vendored)

`scripts/fetch-sysml-v2-release.sh` downloads the pinned tag's GitHub tarball to a
temp file and extracts **only** these subtrees into `./.sysml-v2-release/`
(git-ignored, never committed):

- `sysml/src/validation/`, `sysml/src/examples/`
- `kerml/src/examples/`
- `sysml.library/` (load order Kernel → Domain → Systems)
- `bnf/`
- `LICENSE`

The `Systems-Modeling/SysML-v2-Release` repo is **EPL-2.0** at the repo level (one
`LICENSE`, no per-file headers) covering all models and the standard library. The
spec PDFs under the release's `doc/` tree are **OMG-copyright** and are
deliberately **not** fetched or redistributed.

The script is idempotent: it writes `./.sysml-v2-release/.fetched-<tag>` on
success and no-ops if that stamp is present. The tests skip cleanly (compile,
no-op, "pass") when neither `KR0KI_SYSML_V2_RELEASE_DIR` is set nor the stamp is
present — so the fast CI `check` job (`cargo test --workspace`) is unaffected.

### No expected-error corpus (phase 1 scope note)

`Systems-Modeling/SysML-v2-Release` ships **positive fixtures only** — there is no
machine-checked "must fail to parse" corpus in it. The negative suite lives in the
OMG *Pilot Implementation*'s `org.omg.*.xpect.tests/**/*.xt` files (EPL, but
Xtext-coupled), which is out of scope for phase 1. Tracked as future work.

## Phase 2 (future) — upstream a kr0ki-adapter fixture set

Once kr0ki's FR1 SysML-v2 → `iso_ir` adapter exists, contribute a
kr0ki-adapter-level fixture set — golden `iso_ir` / Mermaid / D2 **lowering**
outputs for a subset of the shared corpus — **into `sysml-v2-parser`'s own
scorecard** (`tests/conformance_scorecard.rs` / `tests/roundtrip_validation.rs`),
rather than growing a parallel corpus here.

Rationale: the parser crate already owns the corpus fetch, the tag pin, the
`ROUNDTRIP_PASS` ratchet, and the scorecard printout. kr0ki should own **only its
lowering deltas** (the mapping from parsed SysML-v2 AST to kr0ki's intermediate
representation and diagram text), not a second copy of the parse/roundtrip
machinery. Phase 2 lands the lowering goldens where the shared corpus already
lives and leaves this crate depending on the published result.
