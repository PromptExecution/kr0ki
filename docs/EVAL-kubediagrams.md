# EVAL — KubeDiagrams (`philippemerle/KubeDiagrams`) for kr0ki's k8s / IaC render path

**Status:** evaluation. Operator asked (2026-09-05) that kr0ki's render layer support
**IaC rendering of Kubernetes**, pointing at KubeDiagrams. This sizes three integration
postures against the [`PLAN-KR0KI-002`](PLAN-KR0KI-002.md) five-box pipeline.

---

## 1. What KubeDiagrams is

- **Apache-2.0**, Python 3.9+, ~2.7k stars, actively maintained (releases every ~4-6
  weeks, v0.8.0 → v0.9.0 dev head as of 2026-09). Academic lineage (INRIA / IEEE
  VISSOFT). Container `philippemerle/kubediagrams`, `kubectl-diagrams` plugin, GitHub
  Action, JetBrains plugin, web app.
- **Input contract:** a stream of Kubernetes API objects as YAML. `kube-diagrams`
  takes manifest files or stdin; `kubectl-diagrams` pipes `kubectl get … -o yaml`;
  `helm-diagrams` pipes `helm template`; kustomize / helmfile are `kubectl kustomize` /
  `helmfile template` piped in. Multi-doc YAML and `*List` objects handled.
- **Output:** `png` (default), `svg`, `pdf`, **`dot`**, **`dot_json`**, **`d2`**,
  **`mermaid`**, `drawio`, plus raster variants.
- **Engine:** `mingrammer/diagrams` → Graphviz (`pygraphviz`). `d2` / `mermaid` are
  KubeDiagrams' own DOT-AST converters; `drawio` shells `graphviz2drawio`.
- **The model** is one declarative file, `bin/kube-diagrams.yaml` (~1050 lines):
  a `nodes:` map keyed `Kind/apiGroup/version` → icon class + an embedded Python
  `edges:` snippet that calls a fixed helper library (`add_owned_resources()`,
  `add_edges_for_service()`, `add_all_volume_resources()`,
  `add_containers_env_value_from_and_env_from()`, `add_service_account()`,
  `add_role()` / `add_subjects()`, `add_ingress_and_egress_rules()`, `add_webhooks()`,
  `add_edge_to(jsonpath, …)`, …). ~51 built-in kinds plus the full Gateway API.
  CRDs are added by dropping a `-c` config with more `nodes:` entries.
- **Edge vocabulary is only ~6 semantic kinds:** `OWNER`, `CONTROLLED_BY`, `SELECTOR`,
  `REFERENCE` (an overloaded catch-all), `COMMUNICATION` (NetworkPolicy), `DEPENDENCE`
  (initContainer service gate). `*-UP` / `INVISIBLE` are Graphviz layout hints. The
  *specific* relation (volume mount vs env ref vs PVC↔PV bind vs ingress→service) only
  survives as the JSONPath string at each `add_edge_to` call site.
- **No typed / semantic export.** `dot` and `dot_json` are rendered-graph topology with
  Graphviz styling; the k8s kind/name/namespace lives in the visual label/tooltip and
  the relation type is encoded only as line style. No resource inventory, no RDF/graph
  model. (`kubectl-graph`, a different tool, is the only one in the landscape that
  emits a real typed property graph — Cypher/AQL.)
- **Determinism:** no internal sorting — output order follows input order. `dot` /
  `dot_json` / `svg` / `mermaid` / `d2` are reproducible run-to-run on a **pinned
  Graphviz**; `png` / `pdf` are not byte-stable across machines (libgd/cairo/pango/font
  versions). A Graphviz upgrade can shift layout.
- **`-c` configs are arbitrary Python** — `edges:` / `nodes:` snippets run via
  `exec()`. Accepting a user-supplied config in a render service is remote code
  execution.

Nothing in the whole `Awesome-Kubernetes-Architecture-Diagrams` landscape (CC0) maps
k8s to SysML / KerML / UFO / RDF. They are all the syntax-first "manifests → picture"
approach that `PLAN-KR0KI-002` §2 explicitly forbids as kr0ki's model path.

## 2. Three postures

### (a) Render backend — a `RenderBackend` that shells KubeDiagrams

`k8s YAML → kube-diagrams -f svg -o - → SVG`, sibling to `HttpKrokiBackend`.

- **Pro:** genuine P0 — one sandboxed subprocess, broad input coverage (manifests /
  kustomize / Helm / helmfile / live cluster) for free, permissive licence.
  [`Playable-first`](../CLAUDE.md-equivalent): a working k8s-YAML → SVG loop in days.
- **Con:** bypasses boxes 2-4 — no UFO graph, no SysML viewpoint, no recognizer. Only
  honest if shipped as an **explicitly separate "quick k8s render" feature**, not as
  part of the SysML pipeline. Heavy runtime deps (CPython + Graphviz + optionally Helm)
  in the service image. RCE risk if `-c` is ever exposed (it must never be). Input is
  multi-file YAML, not a single Kroki text blob — a new `DiagramFormat`/backend
  contract shape.
- **Content-addressing:** hash the normalized **`dot_json`** (not the PNG), pin
  Graphviz in the image, feed manifests in canonical sorted order. Then `FsCache` /
  the FR5 key work unchanged.

### (b) Recognizer prior art — port the ruleset into box 3

Treat `bin/kube-diagrams.yaml` as a battle-tested **GVK → relationship extraction
catalogue** and translate its `nodes:` / `edges:` logic into kr0ki's Kubernetes
pattern recognizer, emitting **UFO-typed** `OntologicalEdge`s into box 2.

- **Pro:** ~51 kinds + Gateway API of ownerRef / selector / volume / env / SA / PVC↔PV↔
  StorageClass / ingress→service / netpol / webhook / HPA-VPA / RBAC wiring, already
  catalogued — weeks of k8s-schema archaeology done. Their per-call JSONPath preserves
  the *specific* relation, so each call site maps to one of kr0ki's finer 25 canonical
  relations (see §3) even though KubeDiagrams itself collapses them to `REFERENCE` at
  render time. **Respects the five-box pipeline.** Apache-2.0 + the rules are facts
  about k8s → translate with attribution (NOTICE).
- **Con:** you port logic, you don't call code — inherit their gaps and re-express
  their runtime-`exec()` snippets as static Rust match arms / a safe declarative table.

### (c) Oracle — differential validation in CI

Render the same manifests through kr0ki's k8s path and through KubeDiagrams; diff the
node/edge sets against KubeDiagrams' machine-readable `dot_json`.

- **Pro:** cheap ongoing recall signal ("did kr0ki drop a Service→Pod edge?"). Their
  `examples/` corpus (argo, istio, cert-manager, kube-prometheus-stack, online-boutique,
  5G stacks) + the `wordpress-manifest.yaml` benchmark = a ready test suite. No runtime
  coupling, CI-only, no prod deps.
- **Con:** validates topology *recall* only, not UFO typing or SysML correctness; their
  coarse edge kinds make the relation-*type* diff fuzzy.

## 3. Edge-vocabulary alignment with `PATTERNS-kubernetes.md`

`OWNER` → `has_part` (inverse `member_of`), `CONTROLLED_BY` → `controls`, `SELECTOR` →
`selects` line up **1:1**. Everything else is inside KubeDiagrams' single `REFERENCE`
kind, recoverable only from the JSONPath:

| KubeDiagrams site (JSONPath) | kr0ki relation |
|---|---|
| Service → EndpointSlice / Endpoints | `resolves_to` |
| PVC `spec.volumeName` → PV | `binds` |
| PVC/PV `spec.storageClassName`, `runtimeClassName`, `priorityClassName`, `ingressClassName`, `gatewayClassName` | `scoped_by` |
| Ingress rule / HTTPRoute `backendRefs` / Gateway `parentRefs` / webhook `clientConfig.service` → Service | `routes_to` |
| volume → ConfigMap/Secret/projected; `envFrom` / `valueFrom` → ConfigMap/Secret | `requires` (mount/consume) |
| `serviceAccountName` → ServiceAccount | `binds` / `authorized_by` |
| RoleBinding/CRB `roleRef` + `subjects` | `authorized_by` |
| Pod `spec.nodeName` → Node; VolumeAttachment → Node | `hosted_by` |
| HPA/VPA `scaleTargetRef` | `controls` |
| `COMMUNICATION` (NetworkPolicy ingress/egress) | `routes_to` (structural) — a request is a separate `flows_to` |
| `DEPENDENCE` (initContainer waits on a Service) | `requires` |
| `clusters:` by namespace / app-instance / chart / component label | `scoped_by` (namespace), `member_of` (labels) |

No KubeDiagrams concept is *incompatible* with the canonical set — it is strictly
coarser. Which is exactly why (b) works: their `edges:` snippets enumerate the call
sites, so porting them yields the finer kr0ki relations for free.

## 4. Recommendation

**(b) + (c), with (a) as an explicitly-separate fast-path — not as the SysML pipeline.**

1. **Now / P0 (optional):** a sandboxed `KubeDiagramsBackend` (`RenderBackend`), gated
   behind a distinct route/format (`POST /render/k8s`, body = a YAML bundle), producing
   SVG for the "I just want a picture of my cluster" use case. Never expose `-c`. Run
   the subprocess rootless, no network, read-only FS. Hash `dot_json` for the cache
   key. Ship KubeDiagrams via its own container as a sidecar rather than fattening the
   Rust image. **This is a leaf feature, not box 5 of the pipeline** — label it as such.
2. **The real path:** port `kube-diagrams.yaml`'s GVK→relationship rules into kr0ki's
   Kubernetes recognizer (box 3, `PATTERNS-kubernetes.md`), emitting UFO `OntologicalEdge`s.
3. **CI:** keep KubeDiagrams as the topology-recall oracle over its `examples/` corpus.

Vendoring note (if the config or scripts are copied into the tree next to
`vendor/kroki-mcp`): Apache-2.0 — retain `LICENSE`, add a `NOTICE`, state changes. MIT
kr0ki may include Apache-2.0 components without relicensing.

## 5. References

- KubeDiagrams — https://github.com/philippemerle/KubeDiagrams (Apache-2.0), homepage
  https://kubediagrams.lille.inria.fr/
- Awesome-Kubernetes-Architecture-Diagrams — https://github.com/philippemerle/Awesome-Kubernetes-Architecture-Diagrams (CC0)
- `kubectl-graph` (nearest prior art for k8s → typed property graph) —
  https://github.com/steveteuber/kubectl-graph
