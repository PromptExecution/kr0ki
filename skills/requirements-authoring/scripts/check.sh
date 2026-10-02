#!/usr/bin/env bash
# The deterministic half of the authoring workflow, from a shell: load the baseline, check the
# nine profile fields and statement lint, resolve links, round-trip ReqIF, list gaps.
# Needs a built kr0ki-core; run from the repository root.
set -euo pipefail
exec cargo run -q -p kr0ki-core --bin kr0ki-assurance -- lint "$@"
