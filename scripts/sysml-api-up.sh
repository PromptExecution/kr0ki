#!/usr/bin/env bash
# SysML v2 pilot API server (Systems-Modeling/SysML-v2-API-Services) + PostgreSQL, loopback-only.
# Build first with ~/.local/share/kr0ki/sysml-api/build.sh (JDK 11, memory-capped; never alongside cargo builds).
# The server's shipped config expects postgres on localhost:5432 with a fixed dev password, hence the app joins postgres's
# network namespace (no pod: the node's b00t-limits hook rejects a pod's uncapped pause container). Published on 127.0.0.1 only. Data persists in $ROOT/pgdata.
set -euo pipefail
ROOT="${KR0KI_SYSML_API_HOME:-$HOME/.local/share/kr0ki/sysml-api}"
PORT="${KR0KI_SYSML_API_PORT:-9000}"
[ -x "$ROOT/src/target/universal/stage/bin/sysml-v2-api-services" ] || { echo "build the server first: $ROOT/build.sh" >&2; exit 1; }
mkdir -p "$ROOT/pgdata"
podman rm -f kr0ki-sysml-api-app kr0ki-sysml-pg >/dev/null 2>&1 || true
podman run -d --name kr0ki-sysml-pg --memory=512m --memory-swap=512m -p "127.0.0.1:$PORT:9000" \
  -e POSTGRES_PASSWORD=mysecretpassword -e POSTGRES_DB=sysml2 \
  -v "$ROOT/pgdata:/var/lib/postgresql/data:Z" docker.io/library/postgres:16-alpine >/dev/null
for _ in $(seq 1 30); do podman exec kr0ki-sysml-pg pg_isready -U postgres -d sysml2 >/dev/null 2>&1 && break; sleep 1; done
podman run -d --network container:kr0ki-sysml-pg --name kr0ki-sysml-api-app --memory=1536m --memory-swap=1536m \
  -v "$ROOT/src/target/universal/stage:/app:Z" -w /app \
  docker.io/library/eclipse-temurin:11-jre \
  /app/bin/sysml-v2-api-services -Dplay.http.secret.key=kr0ki-local-only-not-a-secret-0123456789 -Dhttp.port=9000 -J-Xmx1g >/dev/null
for _ in $(seq 1 90); do
  curl -fsS -m3 "http://127.0.0.1:$PORT/projects" >/dev/null 2>&1 && { echo "SysML v2 API up: http://127.0.0.1:$PORT"; exit 0; }
  sleep 2
done
echo "API did not come up; logs: podman logs kr0ki-sysml-api-app" >&2; exit 1
