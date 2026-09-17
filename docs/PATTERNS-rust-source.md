# PATTERNS — Rust-source recognizer

**Status:** design input, prerequisite for `PLAN-KR0KI-003.md`'s implementation. Mirrors
[`PATTERNS-kubernetes.md`](PATTERNS-kubernetes.md)'s structure and role: the box-3
rule table for the Rust-source arm, the way that doc is for the Kubernetes arm.
**D7 (does this live in kr0ki or `ufo-types`?) is resolved** — kr0ki, per
`PLAN-KR0KI-003.md` §4 — so this table has somewhere to be implemented once written.

```
Rust source (syn AST) → UFO semantic graph → [THIS: rust recognizer] → SysML v2 viewpoints → kr0ki adapters
```

kr0ki MUST NOT infer Rust architecture from doc comments, symbol names alone, or any
other heuristic weaker than the actual parsed AST. `syn` is the pivot; this recognizer
operates on its output, and everything downstream is derived, never re-inferred.

### Prior art: `docgen/harvest.rs`'s `SymbolVisitor`

Per `PLAN-KR0KI-003.md` §3/§4 (D7's resolution), the reusable scaffold is this crate's
own `syn::visit::Visit`-based `SymbolVisitor`
(`crates/kr0ki-core/src/docgen/harvest.rs`), which already walks every `.rs` file in a
workspace and tracks `module_path` as it descends into `ItemMod`. It currently collects
`Symbol { name, qualified_name, kind, signature, docstring }` for `docgen`'s
documentation formats only — it does not, and per `PLAN-KR0KI-003.md` §1/§5 must not,
emit relationship edges itself. The rust-source recognizer generalizes the same
traversal into a **sibling** visitor that emits `iso_ir::{Node, Edge}` instead of (or
alongside) `Symbol` — `docgen/` stays unmodified; the new visitor lives in
`crates/kr0ki-core/src/rust_recognizer.rs`, matching `k8s_recognizer.rs`'s placement.

`b00t-cli/src/dispatch_sysml.rs` is **not** reusable here (`PLAN-KR0KI-003.md` §4's
resolved spike): it exports one hand-maintained runtime value (`Vec<dyn
DispatchMode>`), not a general AST walker. Its target shape
(`iso_ir::{Node, Edge}`, one edge per relationship, `edge_type` as a free-form
classifier string) is still the right precedent to follow for *what* this recognizer
produces, just not *how*.

---

## 1. The endurant / perdurant / moment / abstract bridge

Mirrors `PATTERNS-kubernetes.md` §1 for the Rust domain.

| UFO category | What it captures | Rust examples |
|---|---|---|
| **Endurants** | things that retain identity through time | crate, module, struct, enum, trait, a named function/const/static item |
| **Perdurants** | things occurring / unfolding through time | a function call at runtime, a build, a test run |
| **Moments** | dependent qualities and mediators | a trait implementation (mediates a type and a trait), visibility, a generic bound |
| **Abstracts** | selectors, constraints, quantities, propositions | a trait bound, a lifetime, a `where` clause |

Static analysis of source text captures **structural** (endurant/moment) relationships
only — module containment, field types, trait impls, and the call *graph as declared in
code* are all structural facts true regardless of whether the program ever runs. This
recognizer does not and cannot capture perdurants (an actual call at runtime); that
would need trace data (`ingest_traces`), a separate, unrelated capability.

## 2. Canonical relationship vocabulary

| Rust construct | `UfoRelation` | Ontological interpretation |
|---|---|---|
| module `mod m { ... item ... }` containment | `has_part` | strong endurant composition — an item cannot exist outside its declaring module |
| `struct`/`enum` field whose type is another local item | `has_part` | composition — the field's value is part of the containing struct's state (see §2.1 for why this is not `requires`) |
| `fn` body calling another local `fn` | `flows_to` | occurrence / data flow — mirrors `PATTERNS-kubernetes.md`'s own `calls, sends, publishes` reading of `flows_to`, applied to a static call *site* rather than a runtime call |
| `impl Trait for Type` | `satisfies` | the type meets the trait's contract — same reading `PATTERNS-kubernetes.md` §2 gives `satisfies` ("meets, complies-with") |
| `impl<T: Bound> ...` / a function's generic bound | `governed_by` | the implementation is constrained by the bound |
| `use other_crate::Item` / a field typed from an external crate | `requires` | a dependency on a capability this module does not itself define |

### 2.1 Distinctions that MUST stay explicit

- A struct field's type is `has_part`, **not** `requires` — the field's value is
  physically part of the containing struct's memory layout, not a standing capability
  dependency the way a Kubernetes `Requires` (ConfigMap/Secret mount) is. `requires` is
  reserved for cross-crate/external dependencies (§2's last row), where the referenced
  item is not the referencing item's own composition.
- A call site (`flows_to`) is a **static fact about the code** ("this function's body
  contains a call to that function"), not a modeled *occurrence* the way
  `PATTERNS-kubernetes.md` §2.1 distinguishes `routes_to` (configured topology) from
  `flows_to` (an occurrence that actually happened). Without trace data, this
  recognizer can only assert the call *site* exists — every edge it produces should
  carry `SourceAnchor::RustSpan`, never `TemporalExtent`, making that limitation
  visible in the data itself rather than asserted only in prose.
- `satisfies` here (trait impl) is unrelated to `PATTERNS-kubernetes.md`'s `satisfies`
  (element → requirement) beyond sharing a name — same relation, different domain
  reading, exactly as `UfoRelation`'s canonical vocabulary is designed to be reused
  across domains without forcing a domain-specific relation for each.

## 3. Rust concept → UFO stereotype

| Rust concept | UFO stereotype | Reason |
|---|---|---|
| Crate | `Kind` | identity-bearing compilation unit |
| Module | `Kind` | persistent namespacing/containment unit |
| `struct`, `enum` | `Kind` | rigid, identity-bearing type definitions |
| `trait` | `Role` or `RoleMixin` | a contingent capability contract a type may or may not implement — anti-rigid, matching `PATTERNS-kubernetes.md`'s own "controller capability → `Mixin`/`RoleMixin`" reasoning |
| `fn` (free function) | `Kind` | a named, persistent unit of behavior — its *definition*, not a call |
| A function call (call-graph edge) | not separately stereotyped | represented purely as a `flows_to` edge between two `fn` nodes, no node of its own |
| `impl Trait for Type` | `Relator` | mediates the type and the trait, exactly as `PATTERNS-kubernetes.md` treats `ownerReferences` — a binding between two endurants, not itself a third endurant |
| A generic bound / `where` clause | `Abstract` | formal predicate, same reading as a Kubernetes label selector |

## 4. Recognizer output → SysML v2

Once the Rust graph is UFO-typed and its edges normalized, `sysml_lift.rs` already
covers every one of this table's `UfoRelation`s (no new lift-side work needed):

- `has_part` → `Relation::FeatureMembership` + `SysmlViewKind::Tree`
- `flows_to` → `Relation::Succession` + `SysmlViewKind::ActionFlow`
- `satisfies` → `Relation::Satisfy` + `SysmlViewKind::General`
- `governed_by` → `Relation::Allocation` + `SysmlViewKind::General`
- `requires` → `Relation::Dependency` + `SysmlViewKind::Interconnection`

This is the payoff of `sysml_lift.rs` being written against the domain-neutral
`UfoRelation` vocabulary rather than per-arm: the Rust arm needs no box-3→4 lift work
of its own, only a box-1→2 producer (this recognizer) emitting the relations above.

## 5. What's in scope for a first implementation slice vs. deferred

Mirrors `k8s_recognizer.rs`'s own "what's ported vs. deliberately deferred" doc-comment
convention — an intentional first increment, not silently incomplete coverage.

**First slice:** module containment and struct/enum field types (§2, rows 1–2) —
both are purely structural, resolvable from a single file's AST with no cross-file or
type-resolution work (a field's type is a `syn::Type` printed as a path string; whether
it resolves to another *local* item is decided by string/qualified-name matching
against symbols the same harvest pass already collected, not a real type checker).

**Deferred, tracked not silently missing:**
- **Call graph** (§2 row 3) — needs resolving a call expression's callee to a concrete
  local `fn`, which `syn` alone cannot do for anything beyond a bare unqualified call
  in scope (method calls, trait dispatch, and re-exports all need more than AST
  pattern-matching). A real implementation needs either a much narrower "same-module
  direct calls only" first cut or a heavier dependency (`ra_ap_hir`-style resolution).
- **Trait impls** (§2 row 4) — straightforward to detect (`syn::ItemImpl`) but needs a
  decision on how to represent blanket / generic impls (`impl<T: Bound> Trait for
  Vec<T>`) before committing to an edge shape; deferred pending that decision rather
  than guessed at.
- **Cross-crate `requires`** (§2 row 5) — needs distinguishing a workspace-local crate
  from a genuine external dependency, which means reading `Cargo.toml` workspace
  membership, not just `syn::visit`.
