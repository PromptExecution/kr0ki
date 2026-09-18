# Container-only local integrations

kr0ki's local MCP integration is intentionally container-only. Podman builds OCI
images; the local k0s cluster runs them and `kubectl --context Default` owns their
lifecycle. `b00t mcp install` registers **one** public stdio endpoint, `kr0ki-mcp`,
in both `.mcp.json` and the local Codex MCP configuration. Nothing installs Go,
Python packages, or an MCP binary on the host.

`kr0ki-mcp` is a stdio bridge to the `kr0ki-local` Pod. `kroki`, `kr0ki`, and the
long-lived bridge container share the Pod network namespace; Codex attaches to the
bridge with `kubectl exec`, not an untracked `podman run`:

```sh
just pod-up
b00t mcp register kr0ki-mcp --gate "command:kubectl --context Default -n kr0ki get pod kr0ki-local" --hint "Local kr0ki MCP facade (including KubeDiagrams worker)" -- kubectl --context Default -n kr0ki exec -i pod/kr0ki-local -c kr0ki-mcp -- python3 /opt/kr0ki-mcp/bridge.py
b00t mcp install kr0ki-mcp dotmcpjson
b00t mcp install kr0ki-mcp codex
```

Each workload has an explicit Kubernetes CPU and memory limit. `pod-up` imports the
locally-built OCI images into k0s via `sudo k0s ctr images import`, then applies the
manifests through the explicit local context. The host k0s controller must be running
first (`sudo k0s start`); the repository intentionally never changes the active kubectl
context or falls back to the expired cloud context.

The retired Podman Kube path needed a host-hook exception because its infra container
cannot receive a resource limit. The host hook should include the narrowly
scoped Podman-infra exemption tracked in b00t task #4; without it the pod is created
but cannot start.

If the service has FR7 bearer authentication enabled, add the already-provisioned
`KR0KI_AUTH_TOKEN` to the container environment at invocation time; do not place a
token in `.mcp.json` or repository files.

KubeDiagrams is an internal worker, because it is a quick Kubernetes-manifest renderer
—not a SysML/UFO pipeline stage. `containers/kr0ki-mcp/http_worker.py` is the
`kr0ki-mcp` container's own ENTRYPOINT: a persistent HTTP listener bound to
`0.0.0.0:8788` inside the pod (required so the kubelet readinessProbe — which
always targets the Pod IP, never `127.0.0.1` — can reach it; see
`deploy/kr0ki-local.pod.yaml`). It shells out to `kube-diagrams` without ever
passing the upstream `-c` option (that configuration can execute Python) and
parses manifests with `yaml.safe_load` throughout. Codex never registers a
second `kubediagram-mcp` server. Two callers reach the worker the same way:
`kr0ki-server`'s native `POST /render/kubediagram` HTTP route, and
`bridge.py`'s `render_kubernetes_manifest` MCP tool (same tool name and
interface as before, now dispatched generically from the `GET /mcp/tools`
manifest rather than a hardcoded branch). Neither path has a host mount or a
Linux capability, and arbitrary downstream commands remain unavailable.

```sh
just pod-up
```

The image is built from the vendored [PromptExecution/KubeDiagrams](https://github.com/PromptExecution/KubeDiagrams)
fork, not a mutable Docker Hub image. Its command and supported output types follow
the upstream project documentation; the fork is the controlled home for future
`-b00t` extensions.

## Provider-neutral model and state services

The default `just pod-up` footprint contains only kr0ki and its renderer/MCP
companions. It does not start PostgreSQL, a model server, or a contract
registry. This keeps the local UX testable without choosing a database provider
or creating state that disappears with a Pod.

To render a model snapshot, set `KR0KI_SYSMLV2_BASE_URL` (and, if required,
`KR0KI_SYSMLV2_TOKEN`) in the local `.env` to any Systems Modeling API server.
The server may use a local, air-gapped, or Hive-managed PostgreSQL deployment;
kr0ki only calls its HTTP model API.

Ledgrrr is the planned owner of state-machine contract definitions and their
registry. The integration boundary is a named contract lookup, not a kr0ki
database schema or PostgreSQL extension. See
[`DESIGN-NOTE-ledgrrr-state-contract-registry.md`](DESIGN-NOTE-ledgrrr-state-contract-registry.md).

<!-- b00t:map v1
summary: container-only local MCP bridges for kr0ki and sandboxed KubeDiagrams
tags: kr0ki, mcp, codex, podman, kubediagrams, security
tier: ch0nky
cmds: b00t mcp install kr0ki-mcp codex, just pod-up, kubectl --context Default -n kr0ki get pods
complexity: 6
-->
