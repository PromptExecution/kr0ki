#!/usr/bin/env bash
#
# Vendor a pinned copy of yuzutech/kroki.io's example catalogue (kr0ki#19).
#
# `yuzutech/kroki` (the server repo) has NO clean, individually-extractable
# per-format fixture set — format-specific test data is scattered across
# each sub-service's own test harness (Java resources, .edn, ad hoc JS
# strings), entangled exactly the way the issue's spike step worried about.
# `yuzutech/kroki.io` (the *website* repo, MPL-2.0) does have one:
# `assets/examples/data.json`, one JSON file with a real, working example
# source per diagram type — confirmed to cover all 26 of
# `kr0ki_core::format::DiagramFormat::ALL`'s companion-free slugs.
#
# This fetches that one file from a PINNED commit (not `master` — determinism)
# and writes a decoded, trimmed copy to
# crates/kr0ki-core/fixtures/kroki-examples.json: only `type`/`name`/`href`/
# `categories` per format plus `source`/`lang`/`title` per example (the
# pre-rendered `svg` field is dropped — kr0ki renders its own). `source` is
# HTML-entity-decoded (data.json escapes it for the website's own display).
#
# Re-run this after bumping KROKI_IO_REV. Keep that pin roughly in lockstep
# with containers/kroki-compat/Containerfile's `docker.io/yuzutech/kroki@sha256:...`
# base image digest — different repos/release cadences, so this is a manual
# correspondence to re-check periodically, the same discipline already applied
# to sysml-v2-parser/ufo-types (DESIGN-NOTE-typed-model-layer.md §2.7), not
# something this script can derive automatically.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DEST="${REPO_ROOT}/crates/kr0ki-core/fixtures/kroki-examples.json"

KROKI_IO_REV="449c2e4eb74321ff3d2c13fc3da946e2cd87c2d3" # 2026-07-27, yuzutech/kroki.io master
SOURCE_URL="https://raw.githubusercontent.com/yuzutech/kroki.io/${KROKI_IO_REV}/assets/examples/data.json"

TMP="$(mktemp)"
trap 'rm -f "${TMP}"' EXIT

echo "downloading ${SOURCE_URL}"
curl -fSL --retry 3 -o "${TMP}" "${SOURCE_URL}"

python3 - "${TMP}" "${DEST}" "${KROKI_IO_REV}" <<'PY'
import html
import json
import sys

src_path, dest_path, rev = sys.argv[1], sys.argv[2], sys.argv[3]

with open(src_path, encoding="utf-8") as f:
    data = json.load(f)

types = []
for t in data["types"]:
    examples = [
        {
            "anchor": ex["anchor"],
            "title": ex["title"],
            "lang": ex["lang"],
            "source": html.unescape(ex["source"]),
        }
        for ex in t.get("examples", [])
    ]
    types.append(
        {
            "type": t["type"],
            "name": t["name"],
            "href": t.get("href"),
            "categories": t.get("categories", []),
            "examples": examples,
        }
    )

out = {
    "_source": "https://github.com/yuzutech/kroki.io (MPL-2.0)",
    "_source_path": "assets/examples/data.json",
    "_source_rev": rev,
    "_synced_by": "scripts/sync-kroki-examples.sh",
    "types": types,
}

with open(dest_path, "w", encoding="utf-8") as f:
    json.dump(out, f, indent=2, ensure_ascii=False)
    f.write("\n")

print(f"wrote {len(types)} diagram types to {dest_path}")
PY
