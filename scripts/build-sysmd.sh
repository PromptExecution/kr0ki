#!/usr/bin/env bash
# Build the SysMD sidecar image: the PromptExecution/SysMD fork's `kr0ki/sidecar` branch (upstream tukcps/SysMD v4.3.0 plus our
# patches: toolchain resolver, distinct currencies, REST sessions with libraries, DELETE /session) -> bootJar -> podman image.
#
# Needs JDK 25 and podman. Run it ALONE: a gradle build next to a cargo build starves both of memory.
set -euo pipefail
cd "$(dirname "$0")/.."
src="${KR0KI_SYSMD_SRC:-$HOME/.local/share/kr0ki/sysmd}"
branch="${KR0KI_SYSMD_BRANCH:-kr0ki/sidecar}"
repo="${KR0KI_SYSMD_REPO:-https://github.com/PromptExecution/SysMD.git}"

if [ -d "$src/.git" ]; then
  git -C "$src" fetch --quiet origin "$branch"
  git -C "$src" checkout --quiet --detach FETCH_HEAD
else
  mkdir -p "$(dirname "$src")"
  git clone --quiet --branch "$branch" --depth 1 "$repo" "$src"
fi
rev="$(git -C "$src" rev-parse --short HEAD)"
echo "SysMD @ $rev ($branch)"

(cd "$src" && ./gradlew bootJar --no-daemon -x test)
jar="$(ls "$src"/build/libs/*.jar | grep -v -- '-plain' | head -1)"
install -m 644 "$jar" containers/kr0ki-sysmd/sysmd.jar
podman build --memory=2g --memory-swap=2g --label "org.opencontainers.image.revision=$rev" \
  -t localhost/kr0ki-sysmd:dev -f containers/kr0ki-sysmd/Containerfile containers/kr0ki-sysmd
echo "built localhost/kr0ki-sysmd:dev (SysMD $rev)"
