# PATTERNS — Kubernetes recognizer (STUB)

**Status:** stub. Follow-up to [`PLAN-KR0KI-002.md`](PLAN-KR0KI-002.md) §2.1.

The first **pattern recognizer** (box 3 of the five-box ingestion pipeline): the rule
set that lifts a canonical UFO-typed semantic graph derived from Kubernetes cluster
state / manifests into SysML v2 constructs.

kr0ki MUST NOT infer Kubernetes architecture from diagram syntax or from raw `iso_ir`
strings — the UFO semantic graph is the pivot, and this recognizer operates on it.

UFO 4-category split for the Kubernetes domain:

- **Endurants** — Cluster, Node, Namespace, Pod, Service, ConfigMap
- **Perdurants** — reconciliation, scheduling, rollout, request-handling, failure,
  recovery
- **Moments** — health, readiness, ownership-binding, policy-applicability
- **Abstracts** — selectors, constraints, quantities

**TODO:** fill in the full `k8s resource/relationship → UFO stereotype +
relation-normalization` table — see the external consultant table (operator-relayed).
Not written here yet.
