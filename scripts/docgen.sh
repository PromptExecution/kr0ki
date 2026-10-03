#!/usr/bin/env bash
# Git archive is the sole source input: .env, caches, and working edits cannot leak.
set -euo pipefail
operation=${1:-image}
revision=${2:-HEAD}
repo=$(git -C "$(dirname "$0")/.." rev-parse --show-toplevel)
commit=$(git -C "$repo" rev-parse --verify --end-of-options "$revision^{commit}")
git_tree_id=$(git -C "$repo" rev-parse "$commit^{tree}")
memory=${KR0KI_DOCGEN_MEMORY:-8g}
cpus=${KR0KI_DOCGEN_CPUS:-1}
base=${KR0KI_DOCGEN_BASE_IMAGE:-docker.io/library/rust@sha256:4e4a7e7939c17991ab35f2b8c2e67593980f771d28f6b1254b1850f860fd0c7f}
manifest=${KR0KI_DOCGEN_MANIFEST:-tools/rust-behavior-extractor/tests/fixtures/workspace/Cargo.toml}
backend=${KR0KI_DOCGEN_BACKEND:-}
network=${KR0KI_DOCGEN_NETWORK:-none}
if [[ -n "$backend" && "$network" == none ]]; then
    echo 'A renderer requires an explicit KR0KI_DOCGEN_NETWORK; default is none.' >&2
    exit 2
fi
quota=$(python3 - "$cpus" "$manifest" <<'PY'
import math, pathlib, sys
cpus = float(sys.argv[1])
if not math.isfinite(cpus) or cpus <= 0:
    raise SystemExit('KR0KI_DOCGEN_CPUS must be a positive number')
path = pathlib.PurePosixPath(sys.argv[2])
if path.is_absolute() or '..' in path.parts or not path.parts:
    raise SystemExit('KR0KI_DOCGEN_MANIFEST must be relative to the selected tracked source tree')
print(max(1, int(cpus * 100000)))
PY
)
root="$repo/.kr0ki-generated/docgen/$commit"
mkdir -p "$root"
context=$(mktemp -d "$root/context.XXXXXXXX")
trap 'rm -rf "$context"' EXIT
mkdir "$context/source"
git -C "$repo" archive --format=tar --output="$context/source.tar" "$commit"
tree=$(sha256sum "$context/source.tar")
tree=${tree%% *}
tar -xf "$context/source.tar" -C "$context/source"
rm "$context/source.tar"
test -f "$context/source/$manifest" || { echo "manifest missing from revision: $manifest" >&2; exit 1; }
case "$operation" in image|self-test|artifacts) ;; *) echo 'usage: scripts/docgen.sh image|self-test|artifacts [revision]' >&2; exit 2 ;; esac
if [[ ! "$base" =~ @sha256:[[:xdigit:]]{64}$ ]]; then
    echo 'Set KR0KI_DOCGEN_BASE_IMAGE to an immutable rust:1.98.0-bookworm @sha256 reference.' >&2
    exit 2
fi
python3 - "$context" "$commit" "$tree" "$base" "$manifest" "$git_tree_id" "$backend" <<'PY'
import hashlib, json, pathlib, sys, urllib.parse
context = pathlib.Path(sys.argv[1])
source = context / 'source'
inputs = {'revision': sys.argv[2], 'source_tree': sys.argv[3], 'git_tree_id': sys.argv[6], 'builder_image': sys.argv[4],
          'source_manifest': sys.argv[5], 'stable_toolchain': '1.98.0',
          'extractor_toolchain': 'nightly-2026-06-16', 'debian_snapshot': '20260901T000000Z',
          'configuration': {'cargo_build_jobs': 1, 'source_date_epoch': 0}}
if sys.argv[7]:
    backend = urllib.parse.urlsplit(sys.argv[7])
    if backend.scheme not in {'http', 'https'} or not backend.hostname:
        raise SystemExit('KR0KI_DOCGEN_BACKEND must be an HTTP(S) renderer URL')
    hostname = backend.hostname
    if ':' in hostname:
        hostname = '[' + hostname + ']'
    if backend.port:
        hostname += ':' + str(backend.port)
    inputs['configuration']['backend'] = urllib.parse.urlunsplit((backend.scheme, hostname, backend.path, '', ''))
    inputs['configuration']['backend_configuration_sha256'] = hashlib.sha256(sys.argv[7].encode()).hexdigest()
paths = ['Cargo.lock', 'tools/rust-behavior-extractor/Cargo.lock', sys.argv[5],
         'crates/kr0ki-behavior/src/lib.rs', 'crates/kr0ki-behavior/src/schema.rs',
         'crates/kr0ki-core/src/rust_behavior.rs', 'scripts/docgen-bundle.py',
         'containers/kr0ki-docgen/Containerfile']
inputs['generator_inputs'] = {p: hashlib.sha256((source / p).read_bytes()).hexdigest() for p in paths}
(context / 'build-inputs.json').write_text(json.dumps(inputs, sort_keys=True, separators=(',', ':')) + '\n')
PY
config=$(sha256sum "$context/build-inputs.json")
config=${config%% *}
schema_digest=$(sha256sum "$context/source/crates/kr0ki-behavior/src/schema.rs")
schema_digest=${schema_digest%% *}
rule_digest=$(sha256sum "$context/source/crates/kr0ki-core/src/rust_behavior.rs")
rule_digest=${rule_digest%% *}
image="localhost/kr0ki-docgen:${config:0:16}"
runner="localhost/kr0ki-docgen-self-test:${config:0:16}"
renderer="localhost/kr0ki-docgen-renderer:${config:0:16}"
build=(podman build --jobs=1 --http-proxy=false --platform=linux/amd64 --memory="$memory" --memory-swap="$memory" \
    --cpu-period=100000 --cpu-quota="$quota" --timestamp=0 \
    --build-arg "BUILDER_IMAGE=$base" --build-arg "SOURCE_REVISION=$commit" \
    --build-arg "SOURCE_TREE=$tree" --build-arg "SOURCE_MANIFEST=$manifest" \
    --build-arg "SCHEMA_DIGEST=$schema_digest" --build-arg "RULE_DIGEST=$rule_digest" \
    --build-arg "CONFIG_DIGEST=$config" \
    -f "$context/source/containers/kr0ki-docgen/Containerfile")
run=(podman run --rm --read-only --cap-drop=ALL --security-opt=no-new-privileges \
    --memory="$memory" --memory-swap="$memory" --cpus="$cpus" --pids-limit=512 \
    --tmpfs "/tmp:rw,nosuid,nodev,exec,mode=1777,size=${KR0KI_DOCGEN_TMP_SIZE:-6g}")
if [[ "$operation" == self-test ]]; then
    "${build[@]}" --target self-test -t "$runner" "$context"
    "${run[@]}" --network=none "$runner"
fi
if [[ -n "$backend" ]]; then
    # The networked process can only render evidence already extracted offline.
    "${build[@]}" --target renderer -t "$renderer" "$context"
    mkdir "$context/rendered"
    "${run[@]}" --network="$network" --userns=keep-id --user="$(id -u):$(id -g)" \
        -v "$context/rendered:/export:Z" -e "KR0KI_DOCGEN_BACKEND=$backend" "$renderer"
    (cd "$context/rendered" && sha256sum -c bundle.sha256)
    if [[ "$operation" != self-test ]]; then
        "${build[@]}" --network=none --target rendered-artifact -t "$image" "$context"
    fi
elif [[ "$operation" == self-test ]]; then
    echo 'Private renderer gate not configured; set KR0KI_DOCGEN_BACKEND and KR0KI_DOCGEN_NETWORK.'
else
    "${build[@]}" --target artifact -t "$image" "$context"
fi
if [[ "$operation" != self-test ]]; then
    podman image inspect "$image" --format '{{.Id}}' > "$root/image-id.txt"
    printf 'Artifact image: %s\n' "$image"
    cat "$root/image-id.txt"
    if [[ "$operation" == artifacts ]]; then
        container=$(podman create --memory="$memory" --memory-swap="$memory" --cpus="$cpus" --network=none "$image" /unused)
        trap 'podman rm "$container" >/dev/null; rm -rf "$context"' EXIT
        export_dir="$root/$config"
        mkdir -p "$export_dir"
        podman cp "$container:/artifacts/." "$export_dir/"
        (cd "$export_dir" && sha256sum -c bundle.sha256)
        printf 'Bundle location: %s\n' "$export_dir"
        cat "$export_dir/bundle.sha256"
    fi
fi
