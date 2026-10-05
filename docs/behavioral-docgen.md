# Rust behavioral documentation and OCI artifacts

The compiler front end is isolated under `tools/rust-behavior-extractor` and uses
`nightly-2026-06-16` with `rustc-dev`, `rust-src`, and `llvm-tools-preview`. The
stable `kr0ki-behavior` contract carries compiler facts and declared executable
state machines; `kr0ki-docgen` validates and renders those contracts. Compiler
analysis runs locally or within Podman. JSON rendering does not compile submitted
source.

## Local interfaces

```sh
rust-behavior-extractor --manifest-path path/to/Cargo.toml --output ir.json \
  --revision SOURCE_COMMIT --tree-digest SOURCE_TREE --workspace-root path/to/workspace
cargo run -p kr0ki-core --bin kr0ki-docgen -- bundle \
  --input ir.json --output .kr0ki-generated/local \
  --state-machine crates/kr0ki-behavior/tests/fixtures/ooda.json
```

The bundle command writes validated IR, typed graph and selected view JSON,
notation, SCXML state machines, and lint/validation evidence. Optional `--backend`
requires the configured private renderer; available output formats and their
validation results are recorded by the CLI. Unresolved dispatch remains explicit
in compiler output and findings.

`POST /render/rust-behavior?format=json` accepts a `model` containing this IR,
an optional upstream `ViewDefinition` in `view`, `expand_depth` from 0 to 8,
`strict`, and reviewed `stereotypes` keyed by node ID. Other formats are `d2`,
`mermaid`, `sysml`, `svg` (default), and `png`. Unknown fields, invalid evidence,
unknown exposed elements, and unsupported custom rendering are rejected before
the renderer runs. The HTTP route never invokes Cargo or rustc.

Compiler-confirmed source items retain explicit bracketed Rust attributes in
`nodes[].annotations`: their path, exact source text and original byte anchor.
Inner `#![...]` attributes are included; doc comments are not synthesized into
attributes. Reviewed `annotation_stereotypes` rules are keyed by that exact raw
attribute text. The CLI accepts the same mapping JSON through
`--annotation-ontology MAPPINGS.json`. Unknown rules and conflicting node/attribute
classifications fail validation. Attribute names never classify a type by
themselves. This supports existing source attributes such as
`#[derive(SysmlBlock)]` without inventing UFO helper macros or executing a runtime
`Stereotyped` implementation.

The default view summarizes function bodies; expansion follows bounded source
ownership. Selection and semantic filters apply before summarization. A rendered
edge keeps its original IR edge in `evidence`, including its byte span, guard,
and resolution status. SysML guards are symbolic Boolean inputs linked to that
evidence; they do not claim that a MIR predicate is executable SysML. SysML text
passes the real grammar parser. D2 and Mermaid remain marked structural-only
until an available renderer parser validates them; SCXML passes XML and shared
transition-contract checks.

SCXML view exports require the complete declared machine: every state and every
transition must be selected. Partial selection omits the machine instead of
creating a different executable table. The CLI adds graph state IDs as
`machine_id::state_id` and transition evidence IDs with
`stable_id("machine-transition", "machine_id::transition_id")`; integrations
providing machine IR directly must use the same convention. The standalone
`StateMachine::to_scxml` API consumes the complete table independently of views.

## Container workflow

```sh
just docgen-image HEAD
just docgen-self-test HEAD
just docgen-artifacts HEAD
```

Commit the generator and fixtures before using these commands: the context comes
from `git archive` of the requested commit. Uncommitted edits, `.env`, Cargo caches,
and untracked host files are not source inputs. Git submodules are not implicitly
included; the docgen path uses the locked Cargo dependencies and tracked fixtures.
Builds run serially. `KR0KI_DOCGEN_MEMORY` defaults to `8g`, and
`KR0KI_DOCGEN_CPUS` defaults to `1`; runner temporary storage defaults to `6g` and
may be changed with `KR0KI_DOCGEN_TMP_SIZE`. The immutable Rust 1.98.0 Debian bookworm base
is pinned to its Linux amd64 manifest digest; a different immutable base may be
specified with `KR0KI_DOCGEN_BASE_IMAGE`. The builder checks that its actual default
`rustc` and Cargo both report `1.98.0` before installing dependencies and before
compiling the generator; a mismatched override fails without publishing artifacts.
Debian package sources are pinned to the
`20260901T000000Z` snapshot. The stable and standalone compiler Cargo lockfiles are
required and builds use `--locked`.

The default source manifest is the tracked compiler conformance workspace.
`KR0KI_DOCGEN_MANIFEST` selects another tracked manifest relative to the selected
source tree. Dependencies must be available in the pinned image's Cargo cache;
offline extraction fails if they are absent. Compilation can execute Rust build
scripts and procedural macros inside the container. Do not compile untrusted
source directly on a host.

The artifact image contains `/artifacts/bundle.tar.zst`, `manifest.json`, and
`bundle.sha256`, with no executable entrypoint. `docgen-artifacts` creates a stopped
container and copies these files beneath `.kr0ki-generated/docgen/`. The output
reports the image ID, bundle path, and SHA-256. `docgen-self-test` uses a separate
runner image, validates contracts/runtime transition tests, extracts twice, and
compares both manifests and compressed bundles.

The runner has a read-only root, no capabilities, no privilege escalation, explicit
memory/CPU/PID limits, and a writable temporary directory. Network is disabled by
default. A live render gate needs both an explicit private backend and a chosen
Podman network:

```sh
KR0KI_DOCGEN_BACKEND=http://PRIVATE_RENDERER:8010 \
KR0KI_DOCGEN_NETWORK=YOUR_PODMAN_NETWORK just docgen-self-test HEAD
KR0KI_DOCGEN_BACKEND=http://PRIVATE_RENDERER:8010 \
KR0KI_DOCGEN_NETWORK=YOUR_PODMAN_NETWORK just docgen-artifacts HEAD
```

Compiler extraction and runtime tests always run without network access. A
separate restricted renderer container consumes the resulting validated IR and
never invokes Cargo or rustc. Its only writable host mount is a fresh output
directory beneath `.kr0ki-generated/`. When configured, the final scratch image
is assembled offline from that rendered bundle, including SVG and PNG. Backend
identity without credentials/query parameters and its configuration digest are
recorded in build inputs and tag selection. Without this configuration, the
self-test reports the missing renderer gate explicitly. Dependency downloads
occur only in the builder stage.

## Digest contract

The packaging manifest records source revision, the Git tree object ID and a
SHA-256 of the deterministic tracked-source archive, immutable base, toolchains,
snapshot, source manifest, configuration, and generator input hashes. It lists
SHA-256 for each output file. `semantic_sha256` hashes the sorted `semantic_files`
map: IR, schema, graph, view and lint JSON plus SysML, D2, Mermaid and SCXML notation.
Renderer bytes, renderer validation evidence, and manifests containing rendered
file hashes are excluded from this semantic map; `bundle.sha256` hashes the entire compressed archive,
including renders when supplied. The packaging manifest itself is inside the
archive as `bundle-manifest.json`; it has no circular reference to the bundle hash.
The artifact image labels include revision, source archive, schema source, mapping
rules source, configuration digests, and both toolchain versions.

Tar entries are sorted regular files with uid/gid/mtime zero and mode `0644`.
Symlinks and special files are rejected. Zstd compression runs at level 19 with
one worker. Wall-clock timestamps, host permissions, and source file mtimes do not
affect the archive. Changes in compiler facts, validation findings, notation,
toolchain inputs, or renderer bytes do affect the applicable digests.

```sh
python3 scripts/docgen-bundle-test.py
python3 scripts/docgen-workflow-test.py
bash -n scripts/docgen.sh scripts/docgen-build.sh scripts/docgen-self-test.sh scripts/docgen-render.sh
```

Packaging tests cover repeatability after metadata changes, archive metadata,
manifest contents, semantic changes, render exclusion, and rejection of unsafe
input entries. Workflow tests use a disposable Git repository and a mock Podman
to check committed-source isolation, container invocation policy, offline
compilation followed by networked rendering, and offline rendered-image assembly. Successful
packaging or workflow tests alone do not prove that container or
live renderer gates passed; record those separately with source revision and
digests.
