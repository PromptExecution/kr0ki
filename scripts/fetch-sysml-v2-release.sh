#!/usr/bin/env bash
#
# Fetch the pinned OMG SysML-v2-Release model corpus for the kr0ki conformance
# harness (crates/kr0ki-core/tests/conformance.rs).
#
# The tag is read from docs/conformance-target.toml (single source of truth).
# Only the model/grammar subtrees are extracted — never the OMG-copyright spec
# PDFs under the release's doc/ tree. The corpus is EPL-2.0 and is fetched on
# demand, never committed (see .gitignore: /.sysml-v2-release).
#
# Idempotent: if ./.sysml-v2-release/.fetched-<tag> already exists this is a
# no-op. Delete that stamp (or the whole ./.sysml-v2-release dir) to refetch.
#
# Optional override for offline/CI debugging:
#   KR0KI_SYSML_V2_RELEASE_TARBALL=/path/to/<tag>.tar.gz  — use this local
#   archive instead of downloading.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
TARGET_FILE="${REPO_ROOT}/docs/conformance-target.toml"
DEST_DIR="${REPO_ROOT}/.sysml-v2-release"

if [ ! -f "${TARGET_FILE}" ]; then
  echo "conformance target file not found: ${TARGET_FILE}" >&2
  exit 1
fi

# Thin TOML read (b00t: recipes/scripts stay thin — one grep/sed per key).
read_target() {
  key="$1"
  value="$(grep -E "^[[:space:]]*${key}[[:space:]]*=" "${TARGET_FILE}" \
    | head -n1 | sed -E 's/^[^=]*=[[:space:]]*"?([^"]*)"?[[:space:]]*$/\1/' | tr -d '\r')"
  if [ -z "${value}" ]; then
    echo "missing '${key}' in ${TARGET_FILE}" >&2
    exit 1
  fi
  printf '%s' "${value}"
}

TAG="$(read_target release_tag)"
STAMP="${DEST_DIR}/.fetched-${TAG}"

if [ -f "${STAMP}" ]; then
  echo "SysML-v2-Release ${TAG} already present at ${DEST_DIR} (stamp: ${STAMP##*/})"
  exit 0
fi

ARCHIVE_URL="https://github.com/Systems-Modeling/SysML-v2-Release/archive/refs/tags/${TAG}.tar.gz"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT
ARCHIVE="${TMP_DIR}/${TAG}.tar.gz"

if [ -n "${KR0KI_SYSML_V2_RELEASE_TARBALL:-}" ] && [ -f "${KR0KI_SYSML_V2_RELEASE_TARBALL}" ]; then
  echo "using local tarball: ${KR0KI_SYSML_V2_RELEASE_TARBALL}"
  cp "${KR0KI_SYSML_V2_RELEASE_TARBALL}" "${ARCHIVE}"
else
  echo "downloading ${ARCHIVE_URL}"
  curl -fSL --retry 3 -o "${ARCHIVE}" "${ARCHIVE_URL}"
fi

# Fresh tree each fetch so a retag can't leave stale files behind.
rm -rf "${DEST_DIR}"
mkdir -p "${DEST_DIR}"

# Extract ONLY the model + grammar subtrees, stripping the top-level
# SysML-v2-Release-<tag>/ component. GNU tar: --wildcards-match-slash lets the
# trailing /* span nested directories.
echo "extracting model + grammar subtrees to ${DEST_DIR}"
tar -xzf "${ARCHIVE}" -C "${DEST_DIR}" --strip-components=1 \
  --wildcards --wildcards-match-slash --no-anchored \
  '*/sysml/src/validation/*' \
  '*/sysml/src/examples/*' \
  '*/kerml/src/examples/*' \
  '*/sysml.library/*' \
  '*/bnf/*' \
  '*/LICENSE'

# Sanity-check the layout the tests depend on.
for p in "sysml/src/validation" "sysml/src/examples" "kerml/src/examples" "sysml.library" "bnf" "LICENSE"; do
  if [ ! -e "${DEST_DIR}/${p}" ]; then
    echo "extracted tree missing expected path: ${p}" >&2
    exit 1
  fi
done

VALIDATION_N="$(find "${DEST_DIR}/sysml/src/validation" -name '*.sysml' | wc -l | tr -d ' ')"
EXAMPLES_N="$(find "${DEST_DIR}/sysml/src/examples" -name '*.sysml' | wc -l | tr -d ' ')"
KERML_EX_N="$(find "${DEST_DIR}/kerml/src/examples" -name '*.kerml' | wc -l | tr -d ' ')"
LIB_N="$(find "${DEST_DIR}/sysml.library" \( -name '*.sysml' -o -name '*.kerml' \) | wc -l | tr -d ' ')"
BNF_N="$(find "${DEST_DIR}/bnf" -type f | wc -l | tr -d ' ')"

printf '%s\n' "# SysML-v2-Release ${TAG} — fetched by scripts/fetch-sysml-v2-release.sh" > "${STAMP}"
printf 'release_tag=%s\n' "${TAG}" >> "${STAMP}"

echo "fetched SysML-v2-Release ${TAG}:"
echo "  sysml/src/validation : ${VALIDATION_N} .sysml"
echo "  sysml/src/examples   : ${EXAMPLES_N} .sysml"
echo "  kerml/src/examples   : ${KERML_EX_N} .kerml"
echo "  sysml.library        : ${LIB_N} .sysml/.kerml"
echo "  bnf                  : ${BNF_N} files"
echo "  stamp                : ${STAMP}"
