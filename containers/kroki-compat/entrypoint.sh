#!/bin/sh
# kr0ki#20 — copy the build-time-computed capabilities.json (baked in at
# /opt/kr0ki/capabilities.json) to a shared pod volume so kr0ki-server (a
# separate container, separate filesystem) can read it, then hand off to the
# real Kroki process exactly as the base image's own entrypoint does.
#
# KR0KI_CAPABILITIES_PATH is optional: unset (e.g. plain `podman run`, `just
# dev-kroki-up`) just skips the copy — capabilities.json still exists at its
# baked-in path inside this container, only the cross-container hand-off is
# skipped.
set -eu

if [ -n "${KR0KI_CAPABILITIES_PATH:-}" ]; then
  cp /opt/kr0ki/capabilities.json "${KR0KI_CAPABILITIES_PATH}"
fi

exec java ${JAVA_OPTS:-} -jar /usr/local/kroki/kroki-server.jar
