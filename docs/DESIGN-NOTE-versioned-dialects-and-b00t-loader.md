# DESIGN NOTE — versioned dialects, the ReqIF/endurant bridge, and the `b00t://` loader

**Status:** pre-decision input. **Not approved. Not a plan.** Captures a same-session
direction-setting conversation with Brian H (2026-09-20) across `ufo-types`, kr0ki, and
b00t-cli, so the reasoning survives until a real `PLAN-KR0KI-0xx` is warranted. Several
items below are forward-looking scope notes, not specs — marked as such.
**Created:** 2026-09-20 · **Owner:** PromptExecution (@elasticdotventures)
**Reads against:** `docs/DESIGN-NOTE-typed-model-layer.md` (§2.4, §2.6, §2.8 — binding),
`docs/DESIGN-NOTE-ledgrrr-state-contract-registry.md` (binding precedent for the loader
shape), `crates/kr0ki-core/src/reqif_import.rs` / `reqif_adapter.rs` (PR #38/#40/#41,
merged), `ufo-types/src/mbse/requirements.rs`, `ufo-types/src/stereotype.rs`.

---

## 0. What this is about

Four things came up in sequence and turned out to be one design: (1) kr0ki/b00t need a
URI scheme for pulling bytes out of arbitrary registered sources (databases, files,
model servers) without kr0ki-core owning acquisition policy; (2) those bytes arrive at
many different minor/patch versions of the same schema and must decode forward, never
down; (3) `Requirement` should be a first-class citizen of the UFO ontology, not a bare
struct outside it; (4) the concrete ReqIF codec (`reqrs`) is young and must not become a
permanent, unconditional dependency of a foundational crate. This note ties those four
into one shape and flags where it touches already-merged code.

---

## 1. `DialectUrn` + `Upgrade` — versioned wire contracts, live in `ufo-types`

**Binding constraint, already decided:** `DESIGN-NOTE-typed-model-layer.md` §2.8 rejects
a `{major, minor}` field *on* a canonical typed model (`SysGraph`, and by extension
`RequirementGraph`/`Requirement`). Quote: *"Stability is enforced by golden fixtures +
sha256 content hashes + frozen wire constants... A version field invites the drift the
fixture discipline exists to prevent."* Nothing below adds such a field. Version
metadata lives on the **envelope that wraps a decode**, exactly the shape
`reqif_import.rs` already uses: `ImportedReqIfDocument` carries
`normalized_graph_sha256` and a `RequirementGraph` side by side; the graph itself stays
unversioned. This note generalizes that split, it does not revisit it.

### 1.1 Naming

kr0ki already has two ad hoc version-identity conventions:

- `b00t.type/reqif-import/v1` (`reqif_import.rs:45`) — interface/envelope identity,
  major-only.
- `ufo-types/0.15.0:mbse::requirements` (`reqif_import.rs:38`) — crate + full semver +
  module path.
- `ledgrrr://state-machines/sysml-render/v1` (`DESIGN-NOTE-ledgrrr-...md`) — the same
  `<scheme>://<namespace>/<name>/v<version>` shape, one level up, for a cross-service
  contract reference.

Unify the first two into one canonical form, reusing the third's `scheme://` framing so
all three "name a versioned contract" conventions in this codebase look the same:

```
urn:b00t:dialect:<crate>:<module::path>:<semver>
e.g. urn:b00t:dialect:ufo-types:mbse::requirements:0.15.0
```

### 1.2 The trait

```rust
// ufo-types::dialect (new module, no external dependencies)

pub struct DialectUrn {
    pub crate_name: &'static str,
    pub path: &'static str,
    pub version: semver::Version,
}

/// Forward-only decode: turn versioned wire bytes into the current in-process
/// value. There is deliberately no `downgrade()` on this trait.
pub trait Upgrade: Sized {
    /// This type's current dialect identity.
    const DIALECT: DialectUrn;

    /// `from_version`'s major component MUST equal `Self::DIALECT.version.major`
    /// or this returns `DialectError::MajorMismatch` — a major bump is a
    /// breaking wire change and gets an explicit, separately-versioned
    /// migration, never a silent `upgrade()` attempt. Every `from_version`
    /// this type has ever shipped at the current major MUST succeed.
    fn upgrade(bytes: &[u8], from_version: &semver::Version) -> Result<Self, DialectError>;
}
```

- **Concurrency is structural, not incidental.** `upgrade()` is a plain function per
  call — no shared mutable dispatch state, no `static OnceLock<Mutex<...>>` registry.
  Many callers can decode many sources at many declared versions in parallel with zero
  contention. (Non-hypothetical: the `b00t learn` bug pi/ch0nky is diagnosing right now
  in a sibling worktree looks like exactly this class of bug — a warn-once
  `OnceLock<Mutex<HashSet<String>>>` in `b00t-cli/src/boot_datum.rs` producing surprising
  behavior. This trait shape rules that class out by construction.)
- **Not every dialect needs `.upgrade()`.** Formats kr0ki doesn't control the evolution
  of — the SysML v2 grammar itself — stay exact-pinned instead, per §2.7's existing
  precedent: *"pin `sysml-v2-parser` to exactly the version `ufo-types` tracks... treat
  the pin as a wire-format constant, not a tuning knob."* `Upgrade` is for contracts
  kr0ki/b00t own both ends of.
- **Testing discipline extends the existing bar, doesn't replace it.** Every
  `from_version` arm needs its own golden-fixture round-trip (old bytes in → current
  value out → matches the frozen hash) — the same bar `normalized_graph_sha256` already
  sets, applied per historical version instead of once.

New dependency: `semver` in `ufo-types/Cargo.toml` (not currently present anywhere in
the workspace).

---

## 2. `Requirement` as a first-class UFO Endurant

`ufo_types::mbse::requirements::Requirement` (`requirements.rs:54`) currently has no
`Stereotyped` impl — it sits outside the ontology `stereotype.rs` classifies everything
else under. Per `stereotype.rs`'s own table, a requirement is rigid and sortal (it
cannot lose its identity and remain a requirement) — `UfoCategory::Endurant`,
stereotype `Kind`:

```rust
impl Stereotyped for Requirement {
    fn ufo_stereotype(&self) -> UfoStereotype {
        UfoStereotype::Kind("Requirement".into())
    }
}
```

This is intentionally the one-line shape `mbse.rs`'s own doc comment prescribes ("For
agents extending this crate... Implementing `MbseExport` is one line, placed directly
beside the type's `impl Stereotyped` block"). It's additive to `requirements.rs`, not a
rewrite — the module's stated independence from ReqIF/OPA/Flexo/HTTP is untouched (see
§3). The payoff: `Requirement` gets `MbseExport::to_sysml_v2()` for free via
`mbse_field_dump`, and becomes addressable through whatever ontology-wide tooling
already walks `Stereotyped` values (DARE proposals, risk records, etc.) — a requirement
now composes with the rest of the ontology instead of living beside it.

---

## 3. The ReqIF bridge — default, but not unconditional

**Tension to resolve:** "make ReqIF (de)serialization of `Requirement` a first-class,
`#cfg` default data-type" pulls toward baking `reqrs` into `ufo-types`. But
`requirements.rs`'s own module doc already states a boundary: *"This module is
deliberately independent from ReqIF XML, OPA, Flexo, HTTP, and diagram grammars.
Adapters lower their data into `RequirementGraph`."* And `reqrs` is young — pre-1.0,
single maintainer (`kr0ki-reqif-adapter-uses-reqrs` memory) — not something a
foundational, widely-depended-on crate should carry as an unconditional dependency.
Brian's framing resolves this exactly: *"a more generic bridge... periodically ship
major subsystems when they are mature, so the rest of the ecosystem can stabilize."*
There's also a hard Rust constraint pushing the same direction: the orphan rule means
kr0ki-core, a *downstream* crate, cannot `impl Upgrade for RequirementGraph` itself —
both the trait and the type are foreign to it. The bridge has to live inside
`ufo-types`.

**Shape:**

- `ufo_types::mbse::requirements` (existing module) is untouched — stays adapter-agnostic,
  exactly as its doc comment already promises.
- New `ufo_types::reqif` module, gated behind a `reqif` Cargo feature, **on by default**
  (`default = [..., "reqif"]`) — satisfies "first class... default" for kr0ki's own use
  today, while any consumer that wants zero ReqIF-shaped dependencies (including
  `reqrs` itself) opts out with `default-features = false`. If `reqrs` proves unstable
  or gets replaced, exactly one feature module changes; the rest of the ecosystem, and
  every other `ufo-types` consumer, is insulated — the "ship when mature, let the rest
  stabilize" property.
- That module provides the concrete `impl Upgrade for RequirementGraph` for the ReqIF
  dialect, backed by `reqrs`, living where the orphan rule allows it to exist at all.

**Migration (confirmed: now, not deferred):**

`crates/kr0ki-core/src/reqif_adapter.rs` (PR #40/#41) and the parse step of
`reqif_import.rs` (PR merged locally as `7fb20b5` on `task/reqif-intake`, not yet on a
PR) move into `ufo_types::reqif`. `kr0ki-core::reqif_import` keeps exactly its current
job — bounded-bytes acquisition, ZIP/entry-count limits, SHA-256 provenance, the
`ImportedReqIfDocument`/`ReqIfAttachment` envelope, the `/requirements/import` HTTP
route and `import_reqif` MCP tool — and becomes a thin caller of
`RequirementGraph::upgrade(bytes, &declared_version)` instead of owning the reqrs call
directly. The HTTP/MCP contract (`B00T_REQIF_INTAKE_INTERFACE`,
`POST /requirements/import`, `import_reqif` tool schema) does not change shape — this is
an internal move, not a breaking API change. The open `reqrs`-on-`reqif`-0.0.48
namespace-bug caveat (`kr0ki-reqif-adapter-uses-reqrs` memory) travels with the code,
unresolved either way.

---

## 4. `b00t://` — the loader scheme, sponsored by b00t

(Carried over from the earlier chat-only discussion this session, recorded here so it's
not lost.)

**Recommendation: b00t sponsors `b00t://`, kr0ki does not invent its own.** Grounded in
what already exists, not just the concept:

- `BootDatum` already has a `dsn: Option<String>` field and `DatumType::Database`.
  `datum_database.rs::DatabaseDatum::is_reachable` (`b00t-cli/src/datum_database.rs:30`)
  already dispatches on `db_type` (`postgres`/`mysql`/`sqlite`/`redis`) — today,
  reachability-only, shelling out to `psql`/`mysql`/`redis-cli`. The registry and
  DSN/credential model exist; a native data-moving client doesn't yet — that's the real
  gap and the real opportunity.
- Credentials are already solved once, generically, in b00t: encrypted `credential`
  datums, OS-keyring master key. A kr0ki-owned loader would duplicate that.
- b00t already owns kr0ki's cross-cutting concerns — DNS (`kr0ki.b00t.promptexecution.com`,
  open decision D4 in `docs/TODO.md`), CDN (R2/Workers, D5), and cross-repo task
  tracking (`docs/TODO.md` items filed as "b00t task #3"/"#4"). `b00t://` continues that
  pattern.
- kr0ki's own boundary discipline argues the same way twice over: `reqif_import.rs`'s
  doc comment ("the caller owns acquisition policy... intake owns format detection") and
  the Ledgrrr note's *"kr0ki must not require a particular provider, an extension, a
  proxy, or direct SQL access"* are the same principle applied to two different sources.
  A `b00t://` loader is acquisition policy; it belongs where credentials and provider
  choice already live, not in kr0ki-core.

**Mechanism:** `b00t://<datum-name>` resolves through the existing `BootDatum`/
`DatabaseDatum` registry to `{dsn, db_type, credentials}`. A `b00t-c0re-lib` addition
gives each `db_type` (postgres/pgwire, ClickHouse native, Arrow Flight SQL, Parquet
path) a real async client in place of today's CLI shell-out, reusing the DSN/credential
resolution already there. Since `DatabaseDatum` already treats `postgres`/`postgresql`
as first-class, a pgwire-speaking source (the `dbt2` case) is the natural first
protocol to implement — swap the native client in for the `psql` shell-out, nothing new
to invent at the registry layer.

**Composes orthogonally with §1's `DialectUrn`:** the loader resolves *where*
(transport + credentials); the dialect URN resolves *what shape*:
`b00t://<datum-name>#dialect=urn:b00t:dialect:ufo-types:mbse::requirements:0.15.0`.
Self-describing sources (a ReqIF file's own header) use their in-band version instead
of the fragment; the fragment is a pin for sources that don't self-describe.

**Explicit non-goal, inherited from the Ledgrrr note:** no PostgreSQL extension, no SQL
parsing inside a pgwire proxy, no PostgreSQL sidecar in kr0ki's default Pod. A `b00t://`
postgres/pgwire loader is a *client* b00t uses to pull bytes/rows on request — it is not
a proxy, and it does not change kr0ki's own deployment footprint.

---

## 5. Rust-source frontend — reuse `b00t learn rust`'s vocabulary

Forward-looking scope note, not a spec. `PLAN-KR0KI-003`'s box-1/box-3 Rust-source
recognizer (AST → typed graph, per the `docgen-diagram-pipeline-correction` memory:
introspection → typed graph → diagram-as-code emitter → kroki render, kept as separate
stages) should draw its structure-oriented vocabulary from b00t's existing `rust`
learn topic (`_b00t_/rust.🦀/.md`, reachable via `b00t learn rust`) rather than kr0ki
inventing its own idiom for "what counts as a structural relationship" in Rust source.
Not yet scoped into a task; needs its own pass once box-1/box-3 is actually picked up.

---

## 6. Render-target expansion — isometric + procedural SVG, major/minor abstraction

Forward-looking scope note. Diagram output should eventually reach 3D-isometric and
procedurally-enhanced SVG views of the SysML v2 logical model of executable code,
navigable at both subsystem (major) and component (minor) abstraction levels — the
level SysML v2 is designed to represent well. `b00t-isometric-wasm` is the likely
renderer seam. This stays behind the existing `RenderBackend` trait boundary (P0,
already shipped) and the diagram-pipeline memory's stage separation — it's a new
renderer implementation and new `ViewDefinition`/`ViewpointDefinition` authoring for
major/minor scope, not a new pipeline stage. No sizing done yet.

---

## 7. `transm0grify` / `m0grph` — an authoring agent interface

Forward-looking scope note, least defined of the set. A specialized, MCP-exposed agent
interface — distinct from the ingestion-only `import_reqif` tool — whose job is
*authoring* requirements and their typed relations (graph/cloud linkages between types,
via §1's trait-driven `Upgrade`/dialect system) rather than editing serialized text by
hand. Needs its own design pass before anything is built: at minimum, what MCP tool
surface it exposes, how it validates a proposed `RequirementRelation` before it's
`Asserted` (the existing `RelationAuthority::Proposed`/`Inferred` → `promote()` flow in
`requirements.rs` already models exactly this human-gate shape and should be reused,
not reinvented), and whether it's kr0ki-hosted or b00t-hosted given §4's loader
precedent.

---

## Non-goals

- No `{major, minor}` field added to any canonical typed model — §2.8 stands.
- No unconditional `reqrs` dependency in `ufo-types`'s required dependency set — always
  behind the `reqif` feature, default-on today, revisitable independently.
- No change to the shipped `POST /requirements/import` / `import_reqif` HTTP-MCP
  contract shape during the §3 migration — callers don't notice.
- No PostgreSQL extension, SQL-parsing proxy, or PostgreSQL sidecar in kr0ki's default
  Pod, inherited unchanged from `DESIGN-NOTE-ledgrrr-state-contract-registry.md`.
- §5–§7 are not scoped work — do not start implementation from this note alone.

## Blocked on / open questions

- §3's migration touches `ufo-types` (a separate repo/tag dependency,
  `git = "...ufo-types", tag = "v0.15.0"`) — needs its own PR there before kr0ki's side
  can drop `reqrs` as a direct `kr0ki-core` dependency; version-bump kr0ki's pin once
  landed, same lockstep discipline §2.7 already applies to `sysml-v2-parser`.
- §4's native pgwire/ClickHouse/Arrow clients are new `b00t-c0re-lib` surface, not yet
  scoped as a task.
- §7 needs a real design pass (brainstorm-first, per this repo's own process
  convention) before any code.

<!-- b00t:map v1
summary: versioned Upgrade/DialectUrn contract in ufo-types, Requirement as a UFO Endurant, ReqIF as a default-but-feature-gated bridge, and b00t:// as the sponsored loader scheme
tags: kr0ki, ufo-types, b00t, reqif, dialect, semver, loader, pgwire, mbse
tier: frontier
cmds: cargo test -p kr0ki-core reqif_adapter, cargo test -p ufo-types
complexity: 4
-->
