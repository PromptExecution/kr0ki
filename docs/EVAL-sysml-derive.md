# EVAL — `sysml-derive`

**Evaluated:** 2026-09-17 · **For:** kr0ki's typed model layer authoring path
(`docs/DESIGN-NOTE-typed-model-layer.md` §2.7, PRD-KR0KI-001 §5 **D1**,
`ledgrrr#202`). **Verdict:** D1 is **resolved and implemented** — this crate is
production-appropriate for the structural half of SysML v2 text generation, used
in composition with `ufo-types::mbse`, not as a replacement for it.

---

## What it is

`PromptExecution/sysml-derive` — a small (~250 lines of tests, ~170 lines of
macro) `syn`-based proc-macro crate, extracted 2026-08-30 from `ledgrrr` into its
own repo. `Cargo.toml` still says `"Spike"`; **not published to crates.io**,
git-dependency only. 2 commits total as of this evaluation. No README.

`#[derive(SysmlBlock)]` walks a struct's **named fields** and generates
`sysml_block_def() -> &'static str`, a `part def` block:

```rust
#[derive(SysmlBlock)]
struct Transaction { tx_id: String, source_rows: Vec<NodeId> }
// Transaction::sysml_block_def() ==
// "part def Transaction {\n    attribute tx_id : String;\n    attribute source_rows : NodeId[*];\n}\n"
```

Field-type mapping: `Vec<T>` → `T[*]`, `Option<T>` → `T[0..1]`, Rust primitives →
`ScalarValues::{Boolean,Natural,Integer,Rational}`, `chrono::DateTime<Tz>` →
`ScalarValues::String` (SysML v2 has no angle-bracket generics — this was a real
bug caught and fixed, `ledgrrr#195`). Any other generic type is a **compile
error**, not silently-invalid text. It does **not** reference `UfoStereotype`
anywhere — it is a pure structural mapper, Part/containment-only.

## Validation discipline — matches the house bar

`tests/real_grammar_validation.rs` and `tests/bicep_module_digestion.rs` call
`ufo_types::sysml::validate_sysml_v2()` (the `sysml-v2-parser`-backed oracle) on
every generated block and assert `result.disposition.is_satisfied()` — not a
string/regex check. This is exactly `DESIGN-NOTE-typed-model-layer.md` §2.7's
"never visually inspected" bar, already met.

## The D1 decision — RESOLVED 2026-09-10

`ledgrrr#202` weighed three options for how `UfoStereotype`-tagged types get
their SysML v2 text: depend-as-is / extend-in-place / wrap-re-export (plus a
rejected fourth: reinvent in `systhread-core`). **Resolution: separation of
concerns across two independent, composable emitters, not a single macro:**

1. `sysml-derive` stays Part/containment-agnostic — structure-only, untouched.
2. `UfoStereotype → SysML v2 metadata` is emitted by **`ufo-types` itself**
   (`mbse::MbseExport`, extending the crate kr0ki already depends on) — **not**
   the macro.
3. Consumers (systhread FR8/FR9, kr0ki's authoring path) **compose both**: one
   `impl` block per type, no merged macro:

   ```rust
   impl Stereotyped for MyType { fn ufo_stereotype(&self) -> UfoStereotype { ... } }
   impl MbseExport for MyType {
       fn to_sysml_v2(&self) -> String {
           mbse_field_dump(&self.ufo_stereotype(), "MyType", self)
       }
   }
   ```
4. Extraction into the standalone `sysml-derive` repo — already done (this is
   that repo).

Point 2 is **already shipped**, not just decided: `ufo-types` (kr0ki pins
`v0.14.0` via git tag, `crates/kr0ki-core/Cargo.toml:30`; `v0.14.1` is
crates.io-latest) ships `src/mbse.rs` with a working `MbseExport` trait and
`mbse_field_dump()` — a **runtime, serde-reflection-based** emitter that walks
any `Stereotyped` value's `serde_json::Value` and emits a `part { ... }` block
(stereotype as a `//` comment, nested structs → nested `part` blocks,
`Vec<Struct>` → numbered sibling parts, `Option::None` → `[0..1]` unset
attribute). Composition for types containing other `Stereotyped` values is
`DaredProposal::to_sysml_v2` in `dare.rs`, which nests children's own
`to_sysml_v2()` rather than re-deriving.

**TRIZ framing of the resolved contradiction:** a domain type needs to stay a
plain, stereotype-agnostic Rust struct for `sysml-derive`'s structural mapping
to work on it, *and* it needs to carry stereotype/classification metadata for
full SysML v2 fidelity. Resolved by splitting the two concerns into independent
emitters composed at the call site — neither the macro nor the trait needs to
know about the other's mechanism.

## What this means for kr0ki

**`docs/DESIGN-NOTE-typed-model-layer.md` is stale on this point** — its status
line and §4 "Blocked on" table still list D1 as open/blocking. It has been
corrected as part of this evaluation (see that file's changelog note). kr0ki's
SysML v2 *authoring* path is **unblocked**: any future typed `Element` gets its
concrete text via `#[derive(SysmlBlock)]` (structure) + a small `MbseExport`
impl (stereotype metadata), both already-shipped dependencies, no new crate
work required on kr0ki's side.

**Gap this crate does not cover, and is not meant to:** neither `sysml-derive`
nor `ufo_types::mbse` performs *extraction* — turning unstructured or
semi-structured input (a document, a ReqIF file, natural language) into a typed
Rust value in the first place. Both only handle the second half of the
pipeline: **typed Rust value → validated SysML v2 text.** The first half
(unstructured input → typed value) is the genuinely open piece of kr0ki's
future authoring story — see
`docs/DESIGN-NOTE-agentic-mbse-generation.md` for where that fits.

**Maturity caveat:** `v0.1.0`, marked `"Spike"` in its own `Cargo.toml`, no
crates.io release, no README, 4 test files. Production-appropriate for the
mapping it does (validated against the real grammar oracle), but small and
young — track it for breaking changes same as any other git-pinned dependency,
and expect it to grow (a Bicep-module spike in the same repo already flags
SysML v2's `in`/`out` feature directionality as unmodeled — "a known modeling
gap worth a follow-up").
