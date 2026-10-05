# PLAN-KR0KI-009 delivery evidence

## Current revision: Copilot follow-up fixes

Tested generator commit: `68cc508e26459c142ea9a66c6488fcebe7ec3925`.
The Rust changes were locally tested at
`32f0b20af7577e1b2ad0b90dadd9778828438993`; the later source commit adds the
historical-archive wrapper regression without changing Rust sources.
The later documentation commit records this exact source revision. All results
under the historical headings below belong to their named revisions.

### Findings and regression coverage

- `Transition` edges require two `State` endpoints. Regressions reject invalid
  source, target and paired endpoint kinds, and accept valid transitions and
  state self-transitions.
- SCXML selection checks the selected edge's kind and exact machine-qualified
  endpoints in addition to its stable ID. Regressions retain mismatched graph
  evidence while withholding SCXML for wrong kinds, either wrong endpoint and
  transitions belonging to another machine. Complete matching tables still export.
- The OCI builder verifies its default `rustc` and Cargo releases are both
  `1.98.0`, before installing dependencies and again immediately before compiling
  the stable generator. The verification script's digest is recorded among the
  generator inputs. Tests reject older compiler/Cargo releases and nightly
  releases. Historical archives without this script retain their original
  generator-input list.
  A real artifact build with immutable Rust 1.91.1 base
  `docker.io/library/rust@sha256:8322627e69ba7780b54f39e9f4d3758c006a3ae0123ea01d63b91f0626169891`
  failed at the first version check and exported no bundle or manifest.

### Verification of this revision

| Gate | Result |
|---|---|
| Workspace Rust tests | 712 passed, 19 ignored |
| Workspace formatting / all-target Clippy | `just check` passed with warnings denied |
| Pinned standalone extractor | 3 unit / 6 integration tests; formatting and Clippy passed |
| Packaging / OCI workflow tests | 8 / 10 passed |
| Rust-fix GitHub CI | `check`, `conformance`, `compiler-extractor` passed in [run 243](https://github.com/PromptExecution/kr0ki/actions/runs/37204710289) |
| Final source GitHub CI | All three jobs passed in [run 244](https://github.com/PromptExecution/kr0ki/actions/runs/37205618211) |

The Rust regressions failed before the fixes and passed afterward. Local builds
ran serially in the existing isolated worktree. The local Qwen/pi endpoint was
unavailable on the read-only review preflight; narrow review and verification
were completed directly. The earlier b00t identity and vendored Vue limitations
remain unchanged; no b00t/Rhai, infrastructure or vendored-package repair was attempted.
The NATS-registration waiver remains in effect. No deployment or registry publication.

### OCI evidence for this revision

Both offline and configured-private artifact builds and self-tests passed.
A fresh private-render repeat produced identical `bundle.tar.zst`, `bundle.sha256`,
`manifest.json` and artifact image ID. Private SVG/PNG rendering used the explicitly
configured `http://sm3lly.lan:8010` backend with host networking; compilation and
extraction remained offline. Builds and tests used the existing one-CPU / 8 GiB
limits and nonroot, read-only test runners.

| Evidence | Offline | Private rendering |
|---|---|---|
| Image | `localhost/kr0ki-docgen:d34d63cf018cd503` | `localhost/kr0ki-docgen:8d58cc1450c807b1` |
| Image ID | `8bfc8cc8a97ef18f4316177a11b3eac87d8173642ad5c669734cef871b21e884` | `6e83198e462305d5615bbe5b6ca91d0afb43290f10674066ed3afd0f7cc27351` |
| Configuration SHA-256 | `d34d63cf018cd503fb5cfd8685e92037ebcb5b64d78d6ae97ef45bfa1918e79e` | `8d58cc1450c807b10691f9ee4745cb9856c3d70c18b5769dbb9db444ce819a78` |
| Compressed bundle SHA-256 | `8778a8467b184d99ea7b95c1013f3e435b67badb348b9c19abbd60d6a04735ad` | `54c1b8c1699def7370fdc194b8f6da90ef499c6afd6da8074362787138829122` |

Source archive SHA-256: `fae7b7dcceef04e29fb953722602c8dd1a1f3197e4305558f241981257ac2ec4`.
Shared semantic SHA-256: `0c3cb7dbde9ecce7cf830bf941b8d4b71e9c1bc1660a997bb0328677ab67a857`.

Independent verification checked the exact Git archive and all generator input
hashes, Draft 2020-12 IR schema, embedded source bytes, all 635 anchor occurrences,
8 annotations, normalized tar metadata, all artifact hashes and all five dynamic
OCI labels. Each IR contains 3 sources, 212 nodes and 393 edges. The private
repeat was independently verified again and matched the original report.

Commands: `scripts/docgen.sh artifacts 68cc508e26459c142ea9a66c6488fcebe7ec3925`
and `scripts/docgen.sh self-test 68cc508e26459c142ea9a66c6488fcebe7ec3925`; private
runs also set `KR0KI_DOCGEN_BACKEND=http://sm3lly.lan:8010` and
`KR0KI_DOCGEN_NETWORK=host`. Logs, independent verification reports and the
consolidated narrow review are under ignored `.kr0ki-generated/pr90-review/copilot-*`.

All three Copilot findings were resolved after executable source verification,
and PR #90 was marked ready for review. No deployment or registry publication.

## Historical record: 8315fe2 review fixes

Tested generator commit: `8315fe24a635ff0a2618d860b16babda15b7a2ae`. The later documentation
commit records this exact revision; artifact labels and bundles identify the
generator commit. The historical `f0ab1ba` results below are retained separately.

### Reviewed changes

- Compiler-only shard metadata distinguishes declarations from references.
  Compatible shared trait references merge deterministically, declarations supply
  their canonical anchors and annotations, and every reference retains its edge
  evidence. Conflicting declarations remain errors. The public IR schema is unchanged.
- Cargo metadata's workspace root is the default source root. Explicit roots
  must include every workspace member manifest and target source before compilation.
  Source paths, configuration capture and provenance use the resolved root.
- An iterative graph-wide strongly connected component traversal replaces
  repeated descendant scans. Diagnostics still identify every cyclic node,
  including disconnected cycles and self-loops; ownership rules are preserved.
- A separate `compiler-extractor` CI job installs the pinned compiler development
  components and runs standalone formatting, Clippy and tests serially.

### Source verification

| Gate | Result at the tested generator commit |
|---|---|
| Workspace Rust tests | 710 passed, 19 ignored (including the manual benchmark) |
| Python portion of `just test` | 7 bridge, 8 HTTP worker, 81 agent tests completed; 2 agent tests skipped |
| `just check` | Workspace formatting and all-target Clippy with warnings denied passed |
| Standalone pinned extractor | 3 unit and 6 integration tests passed; formatting and all-target Clippy passed |
| Packaging and OCI workflow tests | 8 packaging and 8 workflow tests passed |
| CI workflow syntax | `wrkflw validate .github/workflows/ci.yml` passed |
| Playbook CI configuration / LSP bridge | 135 / 9 tests passed |
| GitHub CI | `check`, `conformance`, and `compiler-extractor` passed in [run 241](https://github.com/PromptExecution/kr0ki/actions/runs/37190430346) |

Compiler regressions cover shared implicit `Sized` and explicit `Debug`/workspace
trait bounds, cross-crate declaration/reference reconciliation, all six merge
orders, repeated and relocated extraction, duplicate declaration rejection,
sibling extraction through a member manifest, and incomplete-root rejection
before a build-script marker can run. Rejection preserves existing IR bytes.
Containment tests cover 30,000-node chains, branching graphs, disconnected cycles,
multiple owners and repeated edges from the same owner.

The manual chain-versus-star benchmark used five validations per measurement:

| Nodes | Chain (ms per validation) | Star (ms per validation) |
|---|---:|---:|
| 1,000 | 6.027 | 4.574 |
| 4,000 | 27.628 | 19.993 |
| 16,000 | 125.311 | 89.989 |
| 32,000 | 268.814 | 194.341 |

Both shapes have comparable near-linear scaling with indexed traversal. These are
local diagnostic measurements, not timing assertions in CI. Repeat with
`cargo test -p kr0ki-behavior --test containment chain_versus_star_benchmark -- --ignored --nocapture`.

The full fresh Vue build in `just test` is unavailable: the unmodified vendored
package requests pnpm 12.4.2, which the registry cannot supply. Disabling automatic
version management with `npm_config_manage_package_manager_versions=false`
reveals an upstream lockfile with multiple YAML documents. The workspace
Rust/Python portions passed and the existing playbook CI configuration passed;
no successful full `just test` or fresh Vue build is claimed for this revision.
An initial sandboxed test attempt could not bind HTTP mock sockets; the required
Rust/Python suite then passed outside that restriction. No vendored source or
package pins were changed.

### Revised OCI evidence

All commands ran serially with the existing one-CPU, 8 GiB, read-only nonroot
runner and offline compiler phase:

```sh
just docgen-artifacts 8315fe24a635ff0a2618d860b16babda15b7a2ae
just docgen-self-test 8315fe24a635ff0a2618d860b16babda15b7a2ae
KR0KI_DOCGEN_BACKEND=http://sm3lly.lan:8010 KR0KI_DOCGEN_NETWORK=host \
  just docgen-artifacts 8315fe24a635ff0a2618d860b16babda15b7a2ae
KR0KI_DOCGEN_BACKEND=http://sm3lly.lan:8010 KR0KI_DOCGEN_NETWORK=host \
  just docgen-self-test 8315fe24a635ff0a2618d860b16babda15b7a2ae
```

Both self-tests passed packaging, 15 behavior/containment tests, the shared
runtime trace and two fresh byte-identical compiler extractions and compressed
bundles. The configured self-test passed private SVG/PNG rendering. A further
fresh private render/export matched every exported file (`bundle.tar.zst`,
`manifest.json`, `bundle.sha256`) and the inspected OCI image ID byte-for-byte.

| Artifact | Image ID | Compressed bundle SHA-256 |
|---|---|---|
| Offline | `c6f7013947e631c711789f96d8c8feeb275f413ea8d7d8aa4ffee9ed84b4235b` | `0bebe776a46d2063618e77f824720ce8bc2aaeae3c4f3bd95027cbf0bd6fc47e` |
| Private SVG/PNG | `1481709d397ac863398fa19ab04fd2d58aa1876d2896a9780fce9253393ee797` | `6de5a983d121c5ae3c411df31e1720621aa1c31b9bf126fe52d37b2b5f2a3ea0` |

Shared semantic SHA-256: `30c4759825b147c08a35f1a66bcc7e9c462c32a97319f0f835e146111b7dcea3`.

Tracked source archive SHA-256: `96aa91e3345f8a8ac324a6f8ac2d0e3f29ae735c059a48ab909b98bc363d0904`.

Local artifact tags: `localhost/kr0ki-docgen:87cc7e577cca7127` and `localhost/kr0ki-docgen:ddb1ad65a0cdb9ca`.
Exports are under `.kr0ki-generated/docgen/8315fe24a635ff0a2618d860b16babda15b7a2ae/`, keyed by
configuration digests `87cc7e577cca712744284d7f147730a910cb42d3818bc434dc2cfd7b9e6722e4` (offline) and
`ddb1ad65a0cdb9ca848b7f278eea847b9383cbe7d497fbae13866e3bea32ff69` (private rendering).

Independent verification checked the exact Git archive, every recorded generator
input and output hash, normalized tar entries, all five dynamic image labels,
Draft 2020-12 IR schema, three embedded sources (including exact authored
state-machine JSON), 635 anchor occurrences across nodes, edges, diagnostics,
annotations and machine entries, and eight annotations. The model retains
212 nodes and 393 edges. Offline and private exports share the same independently
computed semantic digest. Historical image/PNG/Mermaid results below are not
new measurements for this revision.

### Handoff and unavailable gates

Logs, independent verification reports, repeat comparison and the consolidated
subsystem review are under ignored `.kr0ki-generated/pr90-review/`.
The existing isolated feature worktree was used; builds were serial. Source,
compiler, root/publication and containment review passes found no remaining
implementation finding after executable verification.

`b00t whoami` is unavailable in this worktree because `_b00t_/AGENT.md` is absent.
The Qwen/pi review preflight was attempted inside and outside the sandbox; its
local endpoint was unavailable. Reviews and verification were completed directly.
The recorded NATS-registration waiver remains in effect. No b00t/Rhai or
infrastructure repair was attempted. No merge, deployment or registry
publication was performed; PR #90 remains a draft.

## Historical record: f0ab1ba

Everything below describes the earlier tested generator revision
`f0ab1ba1473ef72520a9e142fa4600c58d5e2594`, not the current review fixes.

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
