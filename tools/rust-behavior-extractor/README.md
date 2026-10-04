# Compiler behavior extractor

This standalone workspace pins `nightly-2026-06-16` and the matching `rustc-dev`
component. Stable KR0KI server builds do not link compiler internals.

```sh
cargo +nightly-2026-06-16 build --locked --manifest-path tools/rust-behavior-extractor/Cargo.toml
tools/rust-behavior-extractor/target/debug/rust-behavior-extractor \
  --manifest-path tools/rust-behavior-extractor/tests/fixtures/workspace/Cargo.toml \
  --workspace-root tools/rust-behavior-extractor/tests/fixtures/workspace \
  --revision "$(git rev-parse HEAD)" \
  --output .kr0ki-generated/fixture.ir.json --offline
```

Run the executable through `rustup run nightly-2026-06-16` if its dynamic compiler
library is not on the loader path. The CLI runs Cargo check using itself as
`RUSTC_WORKSPACE_WRAPPER`, merges per-crate facts, validates, and publishes the IR
atomically. Source revision and tree identity accompany embedded hash-checked source.
Temporary check targets/shards use `.extract-*` beside the chosen output and are
removed after successful or failed checks. An existing valid output is replaced
atomically only after the new compiler facts pass validation.

Compilation runs build scripts and procedural macros. Analyze trusted local
workspaces, or run in the resource-limited container. Never invoke this compiler
from the raw-source HTTP endpoint.

Facts use rustc definition paths, type-checked trait implementation predicates,
instance resolution, and a typed `mir_built` query capture before compiler
transformations can steal the body. Publication occurs only after analysis,
type checking, and borrow checking succeed.
CFG nodes represent compiler basic blocks; dominator back edges identify natural
loops and their normal exit edges. Unwind paths remain in the CFG but do not
count as evidence that a loop terminates normally. Concrete dispatch uses
compiler-selected implementations; virtual, generic
and pointer dispatch remain explicit unresolved nodes. Coroutine suspension is
reported from MIR yields. Typed HIR identifies `?`, break, continue, return, and
await constructs with original byte evidence; strings and comments do not
produce effects. Original BOM/CRLF/Unicode bytes are embedded and spans are
converted from rustc's normalized SourceMap back to those bytes.

The adapter emits fields, associated types, supertrait requirements, and generic
predicate constraints. A generic or blanket implementation retains its compiler
predicates on the `satisfies` edge. Implemented traits establish structural
satisfaction; they do not prove all runtime behavior.

Explicit bracketed attributes are parsed from original source and attached only
to compiler-confirmed item nodes. The syntax parser does not resolve calls or
types. Each annotation preserves its exact text, path and byte anchor, including
inner crate/module attributes and BOM/CRLF/Unicode evidence. Disabled cfg items
are not synthesized, while the embedded source file retains its original bytes.
Doc comments are not converted into attribute text. Source syntax unsupported by
the annotation parser produces a finding; it never becomes a guessed annotation.
Reviewed caller rules can map exact attribute text to stereotypes. No UFO helper
attribute macros are defined by the pinned `ufo-types` dependency. Compiler-proven
direct recursion and macro-generated bodies produce explicit warning findings.

Provenance hashes the Cargo invocation directory's configuration hierarchy,
Cargo-home configuration, recursive includes and compilation-affecting Cargo
environment overrides. Raw configuration values and host paths are not exported.
The invocation uses the supplied workspace root, following
[Cargo's configuration lookup rules](https://doc.rust-lang.org/cargo/reference/config.html#hierarchical-structure).
Compiler/wrapper overrides are rejected to preserve the pinned driver. The
extractor overrides `CARGO_TARGET_DIR`; its temporary path is excluded from
provenance so repeated runs remain identical. Arbitrary build-script environment
inputs require a controlled caller environment; these hashes do not make a host
execution hermetic.

Limitations: this first adapter exports whole compiler CFGs rather than recovering
high-level structured source expressions. MIR contains compiler-generated drop and
unwind paths. Guards describe typed MIR discriminants, not reconstructed source
expressions. Unsupported source locations remain findings; dependency source is
represented by external nodes. HIR desugaring can introduce control constructs,
and these are explicitly reported as typed HIR effects. State-machine transition tables enter through the
shared `kr0ki-behavior` runtime contract and are not guessed from arbitrary code.

The extractor choice is based on a successful local pinned-driver spike and the
need for typed compiler CFGs. The rust-analyzer candidate was inspected but was
not locally runnable (component and semantic dependency cache absent); no
comparative performance measurement is claimed. Compiler APIs are isolated in
this workspace because their ABI changes with the nightly compiler.

Acceptance: `cargo +nightly-2026-06-16 test --locked --manifest-path
tools/rust-behavior-extractor/Cargo.toml -- --test-threads=1` compiles a fixture
through the actual adapter, checks exact dispatch/control facts and deterministic
IR, validates BOM/CRLF/Unicode evidence, and rejects compiler type errors without
publishing a final IR.
