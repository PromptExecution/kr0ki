# PLAN-KR0KI-009 delivery evidence

## Tested source

Generator commit: `f0ab1ba1473ef72520a9e142fa4600c58d5e2594`.

The delivery documentation commit refers to this tested generator revision. Local
artifact images and bundles identify the generator revision, not the later
documentation commit. All generated files remain beneath ignored
`.kr0ki-generated/` directories.

## Implemented contracts

- A standalone compiler adapter pins `nightly-2026-06-16`. Successful type and
  borrow checking precede publication of typed dispatch, CFG and source evidence.
  Original UTF-8/BOM/CRLF bytes and byte spans are preserved.
  Explicit source attributes attach only to compiler-local definitions. Reviewed
  mapping rules use exact source text and reject conflicting classifications.
  Source namespaces keep same-named types, functions and getters distinct.
  Cargo configuration hierarchy/includes and compilation overrides contribute
  hashes to provenance without exporting raw configuration contents.
- A stable, versioned IR validates identities, references, provenance and shared
  state-machine tables. The runtime and SCXML exporter consume the same table;
  OODA execution traces are checked against the declared transitions.
- The semantic contract remains upstream `SysGraph`. Disposable Oxigraph queries
  validate graph shapes; there is no additional canonical database.
- Request-authored `ViewDefinition`/`Expose` selection precedes bounded expansion.
  Reviewed stereotype mappings are explicit. SysML uses its real grammar parser;
  SCXML uses XML and transition validation. D2 and Mermaid carry structural
  validation status; the configured D2 renderer provides the render parser gate.
- The CLI and `/render/rust-behavior` consume validated evidence. HTTP never
  invokes Cargo or rustc. Derived edge evidence retains its source facts.
- Podman builds from an exact tracked-source archive with locked dependencies,
  pinned toolchains/base/snapshot, and serial resource limits. Offline extraction
  is separated from the explicitly configured private renderer. Images and
  exports are checked against source inputs, labels, every file digest,
  normalized tar metadata and an independently computed semantic digest.

## Source gates

All gates below passed at the tested generator commit:

| Gate | Result |
|---|---|
| `just test` | 707 Rust passed, 18 ignored; Python suites passed with 2 skipped; freshly built Vue; 137 playbook and 9 LSP passed |
| `just check` | Formatting and workspace all-target Clippy with warnings denied passed |
| Standalone pinned compiler tests | 1 configuration unit and 4 integration tests passed: relocation determinism, annotation/namespace evidence, inherited cfg changes, duplicate crate-name rejection, original byte spans and type-error rejection without publication |
| OCI workflow tests | 8 passed, including rejection of all five stale labels and corrupt exports |
| Bundle packaging tests | 8 passed, including normalization, repeatability, semantic/render separation and unsafe-entry rejection |

Local source gate logs:

- `/tmp/kr0ki-behavioral-gate-f0ab1ba-just-test.log`
- `/tmp/kr0ki-behavioral-gate-f0ab1ba-just-check.log`
- `/tmp/kr0ki-final-compiler.log`

## Artifact gates

All required artifact gates passed using the exact tested generator revision.
Commands were run serially under Podman with CPU `1`, memory `8g`, a read-only
nonroot runner, no capabilities or privilege escalation, and offline extraction:

```sh
just docgen-artifacts f0ab1ba1473ef72520a9e142fa4600c58d5e2594
just docgen-self-test f0ab1ba1473ef72520a9e142fa4600c58d5e2594
KR0KI_DOCGEN_BACKEND=http://sm3lly.lan:8010 KR0KI_DOCGEN_NETWORK=host \
  just docgen-artifacts f0ab1ba1473ef72520a9e142fa4600c58d5e2594
KR0KI_DOCGEN_BACKEND=http://sm3lly.lan:8010 KR0KI_DOCGEN_NETWORK=host \
  just docgen-self-test f0ab1ba1473ef72520a9e142fa4600c58d5e2594
```

Both self-tests passed packaging, the 12 typed behavior contracts, shared runtime
trace and two fresh compiler extractions with byte-identical manifests and
compressed bundles. The configured self-test also passed private SVG/PNG rendering.
A further fresh private-render artifact run produced byte-identical complete
bundles, manifests and digest files; its inspected OCI image ID and labels also
matched. Offline and rendered artifacts share the same semantic digest.

| Artifact | Image ID | Compressed bundle SHA-256 |
|---|---|---|
| Offline | `8a648da36838135ceb94bba5d85ae00f0c1b08fa100415348548a74a1e1a6e29` | `eebad8609bc6e1275812802acaef03af2918cdd5be99492aee801233b71cca2e` |
| Private SVG/PNG | `ee8ce0ca0ab42098e5404e9c4fe055b6accc63afb7cbd3abaf2b286264a4cbd3` | `0a57f0160d113bafe601f11476fd21f9e1710c4f75729303aa3a7f3c2aff8f7c` |

Shared semantic SHA-256:
`98365ab7468abc0f9b524714866e8baf5e37a3d127f495f72513e48a7be1ae0a`.

Local tags: `localhost/kr0ki-docgen:3ad810cb0704de71` (offline) and
`localhost/kr0ki-docgen:9c419a133025a19b` (rendered). Their export directories are
beneath `.kr0ki-generated/docgen/f0ab1ba1473ef72520a9e142fa4600c58d5e2594/`, keyed
by full configuration digests `3ad810cb0704de71098bf082bcb8ec96280c26716e785a288070d287389d4294`
and `9c419a133025a19b1df151e9e0a3441615fcbac609e018e04973c5e5b195228a`.

Independent verification matched the exact Git source archive, every generator
input/output hash, Draft 2020-12 IR schema, source bytes, 604 anchors and eight
annotations; the model contains 212 nodes and 393 edges. All five dynamic image
labels match the selected revision, source archive, schema, rules and configuration.

Actual exported PNG inspection passed sampled label and glyph readability:
15,965 × 1,203 pixels, 31,483 dark pixels; its SVG has 166 text nodes. Dense full
views need zoom and some edge labels overlap. PNG SHA-256:
`c630824b16c03a8834aa317cf3099c82f5f9283d7ddcfb5e890155f208be3551`.

The D2 renderer parsed the generated source successfully. The additional exact
Mermaid-source request returned HTTP 503 because its backend dependency refused
the connection, before source parsing. Mermaid therefore remains structurally
validated; no grammar pass or source rejection is claimed. No service changes
were made to obtain these results.

Local artifact logs and reports are under
`.kr0ki-generated/docgen/f0ab1ba1-logs/`: `FINAL-RESULTS.md`,
`default-artifacts.log`, `default-self-test.log`, `private-artifacts.log`,
`private-self-test.log`, `private-repeat-artifacts.log`,
`private-repeat-comparison.json`, independent verification JSON files,
`FINAL-PNG-RESULT.md` and the Mermaid HTTP response.

## Process and limits

The work used isolated implementation and detached gate worktrees. The user
explicitly waived NATS registration while connectivity remained unavailable.
No registration or infrastructure repair was performed. The sm3lly development
configuration was inspected without exporting its secrets; its local inference
endpoint was used through the pi harness with Qwen3.8 NEO-CODER. Ralph draft,
review and correction cycles supplied proposals using selected b00t worker/reviewer
personas and authored worker-execution, Rust and GitHub review guidance. Source
and executable gates provided independent verification. The harness disabled the broken global LSP
extension for these read-only model calls.

Static evidence remains static evidence. Dynamic, generic and pointer targets
remain unresolved when the compiler cannot prove a concrete target. The first
adapter exports whole compiler CFGs, including generated drop/unwind paths;
guards are typed MIR discriminants rather than reconstructed source expressions.
State machines are declared tables, not inferred runtime behavior. The
rust-analyzer candidate was inspected but not locally runnable; no comparative
performance result is claimed. Model-server retrieval of authored view instances
remains outside this delivery; callers supply upstream view data directly.
Workspaces with conflicting fully qualified crate display names fail during
merging. Distinct compiler shards are retained; a partial workspace is never
published after an overwrite. Standalone build scripts require a controlled
caller environment; configuration hashes do not make arbitrary host execution
hermetic.

Local artifact creation does not publish a registry image, merge a PR, deploy a
service or modify the principal sm3lly development checkout.
