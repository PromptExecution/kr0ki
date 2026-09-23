# Flexo baseline adapter — design

**Status:** §1 (ReqIF export) implemented. §2 (Flexo storage) and §3 (round-trip contract test) blocked on PR #52 (`digital_thread_sync`) merging -- see docs/superpowers/plans/2026-09-23-flexo-baseline-adapter.md.

## 0. Context

`docs/TODO.md`'s ReqIF/Flexo stream names this as **M3**, the last
unbuilt prerequisite before the Playb00k requirements workspace (M4) can
be built against a real contract instead of mocks (per
`docs/HANDOFF-2026-09-19-reqif-flexo.md`: *"Bind Playb00k only after the
adapter exists"*). M1 (semantic model upstreamed to `ufo_types::mbse::
requirements`) and the import half of M2 (`reqif_import`, `reqif_fetch`)
are done. This spec covers M3: **import validated ReqIF into a Flexo
project as a commit, and prove commit ↔ graph ↔ export equivalence** —
i.e. round-trip a ReqIF baseline through a live SysML v2 project without
losing information.

## 1. The missing piece: ReqIF export

`ufo_types::reqif` only has the import direction
(`parse_and_lower`/`bundle_to_requirement_graph`: bytes → `ReqIfBundle` →
`RequirementGraph`). Nothing anywhere maps `RequirementGraph` back to a
`reqrs::model::ReqIfBundle` for re-serialization, even though `reqrs`
itself has the writer — `reqrs::ReqIfUnparser::unparse(&bundle, mode) ->
Result<String, ReqIfError>` (also re-exported at the crate root; the
free function lives at `unparse::driver::unparse_bundle` internally but
is *not* re-exported from `unparse::`, so the type's associated method
is the real public entry point — verified against `reqrs` 0.2.2 source
2026-09-23).

**Decision (approved):** build this export direction as a new
`kr0ki_core::reqif_export` module, using `reqrs` as a **direct**
`kr0ki-core` dependency (not routed through `ufo_types::reqif`, which
deliberately narrows its own scope to the import lowering — see that
module's own doc comment on why it exists only for parsing). This
mirrors the existing precedent: `kr0ki_core::requirements_render` already
owns the `RequirementGraph`-view → D2 adapter locally rather than pushing
rendering concerns into `ufo-types`. `reqrs` is already pulled
transitively into this workspace's dependency graph (via `ufo-types`'
`reqif` feature), so this adds no new supply-chain surface — only
promotes an existing transitive dependency to direct, so `kr0ki-core` can
`use reqrs::{model, unparse}` itself.

`kr0ki_core::reqif_export::export_bundle(graph: &RequirementGraph) ->
Result<ReqIfBundle, ReqIfExportError>` maps each `Requirement` to a
`reqrs::model::SpecObject` (title/text round-tripping through the same
attribute-name convention `ufo_types::reqif`'s import side already uses:
`title`/`name`/`key`, `text`/`description`) and each authoritative
`RequirementRelation` to a `SpecRelation`. Only asserted relations
(`RelationAuthority::Asserted`) round-trip — inferred/proposed edges are
a kr0ki/ufo-types concept ReqIF itself has no field for, and exporting
them would silently promote them, which is exactly what
`promote_relation()`'s explicit gate exists to prevent. A second function,
`export_bundle_to_xml(graph: &RequirementGraph) -> Result<String,
ReqIfExportError>`, calls `ReqIfUnparser::unparse` to finish the job.

### 1a. Corrections and missing steps (verified 2026-09-23 against real `reqrs` 0.2.2 source)

The paragraphs above are corrected in place above; four further things this
spec did not originally account for, found by reading `reqrs`' actual
source rather than guessing its shape:

1. **`ReqIfBundle` has no full constructor.** The only one is
   `ReqIfBundle::empty(namespace: Option<String>, configuration:
   Option<String>)`, which sets `header`/`core_content` to `None` and
   nothing else. `export_bundle` must build `core_content` itself, **then
   call `ObjectLookup::build(&content) -> ObjectLookup`** (single-argument,
   infallible, content-only) and assign it to the bundle's `lookup` field
   before unparsing — nothing wires this automatically.
2. **`AttributeValue` carries no name/key/title string.** Every variant
   (String/Boolean/Integer/Real/Date/Xhtml/Enumeration) only carries
   `definition_ref: AttributeDefId` — an opaque id pointing at an
   `AttributeDefinition`. The `title`/`name`/`key`, `text`/`description`
   attribute-name convention this spec describes cannot attach directly to
   an `AttributeValue`; `export_bundle` must first synthesize a
   `SpecObjectType` (with two `AttributeDefinition`s in its
   `spec_attributes`, one per convention name) and reference their ids via
   `definition_ref` on each `SpecObject`'s values. This is new scope this
   spec did not originally call out, not just a renamed field.
3. **`FormatMode` choice is an open decision, not a default.** `reqrs`
   offers `FormatMode::Passthrough` (relies on parser-captured
   self-closing/whitespace flags) and `FormatMode::Canonical`. A bundle
   built synthetically by `export_bundle` was never parsed, so it has no
   captured formatting flags — `export_bundle_to_xml` should pass
   `FormatMode::Canonical` explicitly. The implementation plan should
   confirm this against a real unparse call before relying on it.
4. **`children_order` fields are required, not `Option`.** Both
   `SpecObject.children_order` and `SpecRelation.children_order` need an
   explicit value — `Vec::new()` is valid (the unparser falls back to
   canonical order) but omitting the field is a compile error, not a
   silent default.

Everything else this spec assumes about `SpecObject`/`SpecRelation` field
names (`identifier`, `description`, `spec_object_type`/`relation_type`,
`source`/`target`) checked out correctly against the real source.

## 2. Storing a baseline in Flexo

A validated ReqIF import (`ReqIfImportResult`, already built) becomes one
new element per `Requirement`, `@type: "RequirementUsage"` (mirroring the
`RuleDocument`/`dbt:` precedent of storing kr0ki-domain data as ordinary
SysML v2 elements, not a bespoke store), written via the existing
`SysmlV2Client::create_commit` write path (`013e653`, already shipped).
Two new fields carry round-trip identity:
- `reqif_baseline_id`: `BaselineIdentity.id` (a plain string) — which
  baseline this requirement belongs to, so a later fetch can group
  requirements back by baseline (a Flexo project may hold requirements
  from more than one imported document over time).
- `reqif_source_sha256`: `BaselineIdentity.import_artifact_sha256`
  (already exists on the type — no new field needed there), preserved
  verbatim so the original bytes' identity survives even though the raw
  bytes themselves are never stored (matching `reqif_import`'s existing
  "never persists the source or attachments" boundary — Flexo owns
  retention of anything beyond this digest, kr0ki does not become a blob
  store).

`BaselineIdentity` already carries a fourth field for exactly §3's
purpose: `exported_baseline_sha256: Option<String>` — set it to the
SHA-256 of `export_bundle_to_xml`'s output once export produces it, so
the round-trip's own proof digest travels with the baseline identity
itself rather than needing a separate side-channel.

Following the already-established `digital_thread_sync` pattern (generic
diff/sync, not dbt-specific in practice): a new
`kr0ki_core::flexo_reqif_sync` module reuses
`digital_thread_sync::build_changeset`-shaped logic — but the existing
`build_changeset` is dbt-`SysGraph`-specific by its own field shape
(`OntologicalNode`/`ElementId`), not directly reusable for
`RequirementGraph`. **This spec chooses NOT to force a shared abstraction
across two different graph types just because the shapes rhyme** — YAGNI
per this project's own norms — and instead writes an analogous, smaller
diff function local to this module, scoped to `Requirement`s only.

## 3. Round-trip contract test

The M3 deliverable's actual proof: import a real ReqIF fixture →
`import_reqif_artifact` → write to a (test/OMG-pilot) Flexo project as a
commit → fetch that commit's elements back → reconstruct a
`RequirementGraph` from them → `export_bundle_to_xml` → re-parse that XML
with `reqrs::ReqIfParser` → assert the reconstructed `RequirementGraph`
is semantically equal to the original (same requirement ids/titles/
texts/asserted relations — not necessarily byte-identical XML, ReqIF's
own attribute ordering isn't meaningful). This is the "commit ↔ graph ↔
export equivalence" `docs/TODO.md` names explicitly.

## 4. Non-goals (explicit)

- No ReqIFz (zip) export — only plain ReqIF XML. Import already handles
  both; export starts narrower and can grow later.
- No inferred/proposed relation export (§1, already explained).
- No attachment round-tripping — `reqif_import` already only inventories
  attachment digests, never retains bytes; export has nothing to attach.
- Does not touch `Playb00k` (M4) — that stays the next, separate item.

## 5. Testing

- Unit tests for `reqif_export::export_bundle` (a hand-built
  `RequirementGraph` → expected `SpecObject`/`SpecRelation` shape;
  inferred relations correctly excluded).
- A round-trip unit test: parse a real fixture → `RequirementGraph` →
  `export_bundle_to_xml` → re-parse → assert graph equality (no network,
  no Flexo — this is the parser/exporter's own contract, independent of
  the write path).
- The full commit↔graph↔export contract test from §3 as an `#[ignore]`d
  live test against the OMG Java pilot, matching every other live test's
  existing convention in this repo (Flexo's own write endpoint is
  stubbed server-side, same reason `sync_dbt_graph`'s live test targets
  the pilot, not Flexo).
