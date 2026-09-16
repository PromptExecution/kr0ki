# PATTERNS — Kubernetes recognizer

**Status:** design input (external ontology consultant, operator-relayed 2026-09-05).
Follow-up to [`PLAN-KR0KI-002.md`](PLAN-KR0KI-002.md) §2.1. **§1/§2 (the UFO-category
bridge + the 25-relation vocabulary, raw k8s -> `UfoRelation`) are implemented** in
`crates/kr0ki-core/src/k8s_recognizer.rs` (kr0ki#12) — ported from
`vendor/kubediagrams/bin/kube-diagrams.yaml`, differentially validated against a real
KubeDiagrams `dot_json` oracle (`crates/kr0ki-core/tests/kubediagrams_oracle.rs`).
Coverage is an intentional subset of KubeDiagrams' ~51-kind catalogue — see that
module's doc comment for what's ported vs. deferred. **§4 (`UfoRelation` ->
`ufo_types::sysml_model::Relation`, the box-3→box-4 lift the renderer adapters
actually consume) is still design-only, no code** — this is what still blocks FR1/FR4
rendering.

The first **pattern recognizer** — box 3 of the five-box ingestion pipeline: the rule
set that lifts a canonical UFO-typed semantic graph (derived from Kubernetes cluster
state / manifests) into SysML v2 constructs, which the kr0ki renderer adapters then
draw.

```
Kubernetes source → UFO semantic graph → [THIS: k8s recognizer] → SysML v2 viewpoints → kr0ki adapters
```

kr0ki MUST NOT infer Kubernetes architecture from diagram syntax or from raw
`iso_ir` classification strings. The UFO semantic graph
(`ufo_types::{UfoStereotype, UfoRelation, OntologicalEdge}`) is the pivot; this
recognizer operates on it, and everything downstream is derived, never re-inferred.

### Prior art to port: KubeDiagrams

`philippemerle/KubeDiagrams` (Apache-2.0) ships `bin/kube-diagrams.yaml` — a
battle-tested **GVK → relationship extraction catalogue** (~51 kinds + the full Gateway
API): for each `Kind/apiGroup/version`, an `edges:` snippet enumerates the JSONPaths at
which that kind references another (ownerReferences, `spec.selector`, volume / env /
serviceAccount refs, PVC↔PV↔StorageClass, ingress/route→service, NetworkPolicy,
webhooks, HPA/VPA `scaleTargetRef`, RBAC `roleRef`/`subjects`, …). KubeDiagrams
collapses all of that to ~6 coarse edge kinds at render time; **this recognizer keeps
the per-JSONPath distinction** and maps each call site to one of the canonical
relations below. Translate the ruleset into a static Rust table (their snippets run via
`exec()` — do not port that mechanism); attribute in a `NOTICE`. See
[`EVAL-kubediagrams.md`](EVAL-kubediagrams.md) §2(b), §3 for the site→relation table and
§4 for using KubeDiagrams as a CI topology-recall oracle.

---

## 1. The endurant / perdurant / moment / abstract bridge

The load-bearing distinction. Kubernetes concepts are sorted into the four UFO
top-level categories (`ufo_types::UfoCategory`) before any relationship is normalized.

| UFO category | What it captures | Kubernetes examples |
|---|---|---|
| **Endurants** | things that retain identity through time | Cluster, Node, Namespace, API object, Pod, Service, ConfigMap, Deployment, Secret, PVC |
| **Perdurants** | things occurring / unfolding through time | reconciliation, scheduling, rollout, request handling, failure, recovery |
| **Moments** | dependent qualities and mediators (exist only in a bearer) | health, readiness, ownership binding, policy applicability |
| **Abstracts** | selectors, constraints, quantities, propositions | label selectors, resource limits, quotas, affinity predicates |

## 2. Canonical relationship vocabulary

Kubernetes overloads verbs — "contains", "owns", "runs", "manages", "connects" all
appear for structurally different relations. Normalize to the canonical set
(`ufo_types::UfoRelation`) rather than treating the synonyms as equivalent.
`UfoRelation::from_synonym(...)` maps the middle column to the left.

| Canonical relation | Common Kubernetes synonyms | Ontological interpretation |
|---|---|---|
| `specializes` | is-a, extends, subtype | universal → universal |
| `instantiates` | instance-of, object-of-kind | endurant → type |
| `has_part` | contains, comprises, composed-of | strong endurant composition |
| `member_of` | belongs-to, grouped-in | endurant aggregation |
| `scoped_by` | in namespace, namespace-of | object → namespace |
| `hosted_by` | runs-on, placed-on, deployed-on | workload → node / cluster |
| `controls` | owns, manages, reconciles | controller → managed object |
| `observes` | watches, monitors, lists/watches | agent / controller → object |
| `selects` | matches, targets | selector → candidate object |
| `resolves_to` | discovers, finds endpoints | service / reference → endpoint |
| `binds` | attaches, mounts, associates | relator mediating two endurants |
| `provides` | offers, implements, exposes | provider → capability / interface |
| `requires` | consumes, depends-on, needs | consumer → capability / interface |
| `routes_to` | proxies-to, forwards-to | structural routing configuration |
| `participates_in` | performs, executes, handles | endurant → perdurant |
| `initiates` | triggers, starts, submits | endurant / event → perdurant |
| `precedes` | before, followed-by | perdurant → perdurant |
| `causes` | produces, results-in | perdurant → event / state |
| `transitions` | changes-to, enters-state | event connecting two states |
| `flows_to` | calls, sends, publishes | occurrence / data flow |
| `satisfies` | meets, complies-with | element → requirement |
| `verifies` | tests, proves, demonstrates | evidence / activity → requirement |
| `governed_by` | constrained-by, policy-applies | element / process → policy |
| `authorized_by` | approved-by, permitted-by | action → authority decision |
| `traces_to` | derived-from, corresponds-to | non-causal traceability |

### 2.1 Distinctions that MUST stay explicit

- `controls` is **not** `has_part` — a controller does not *contain* what it reconciles.
- `hosted_by` is **not** `member_of` — placement on a Node is not group membership.
- `requires` is **not** necessarily a temporal invocation — it is a capability
  dependency; the call that uses it is a separate `flows_to` occurrence.
- `routes_to` describes **configured topology**; `flows_to` describes an **occurrence**.
  A `Service` `routes_to` its endpoints structurally; a specific request `flows_to` one
  of them temporally.
- A controller endurant `controls` a `Deployment` object, but a **reconciliation
  process** (`participates_in` / `causes`) is what changes it.
- A `Service` `selects` `Pod`s structurally; a request occurrence `flows_to` one
  endpoint temporally — different relations, different layers.

## 3. Kubernetes concept → UFO stereotype

`ufo_types::UfoStereotype` variants. `SubKind<K8sObject>` = a rigid subtype of the
persistent API-object kind.

| Kubernetes concept | UFO stereotype | Reason |
|---|---|---|
| Cluster, Node | `Kind` | identity-bearing system objects |
| Kubernetes API object (generic) | `Kind` | persistent information object |
| Deployment, Service, Pod, ConfigMap, … | `SubKind` of `K8sObject` | distinct rigid API-object kinds |
| Leader, worker, primary | `Role` | contingent responsibility |
| Pending, Running, Terminating | `Phase` when classifying the Pod; `State` when modelling the interval | object classification vs. temporal occurrence |
| Controller capability | `Mixin` or `RoleMixin` | capability shared across kinds |
| Reconcile loop | `Process` | extended recurring occurrence |
| Rollout | `Process` or `Scenario` | temporally structured activity |
| Admission request | `Event` | punctual occurrence |
| Pod running interval | `State` | temporally extended condition |
| Failure / recovery sequence | `Scenario` | ordered heterogeneous occurrences |
| Owner binding (`ownerReferences`) | `Relator` | mediates controller and dependent object |
| Readiness / health | `Mode` | dependent quality of a bearer |
| Label selector | `Abstract` | formal predicate |
| Resource limit | `Abstract` + `constrains` | formal quantity / constraint |

## 4. Recognizer output → SysML v2

Once the k8s graph is UFO-typed and its edges normalized, the recognizer maps to
`ufo_types::sysml_model` (box 4):

- `has_part` / `member_of` → `Relation::FeatureMembership` + package/containment view
- `hosted_by` / `binds` / `resolves_to` / `routes_to` → `Relation::Connection` +
  interconnection view
- `participates_in` / `initiates` / `precedes` / `causes` / `transitions` / `flows_to`
  → `Relation::Succession` + action-flow / state-transition views
- `satisfies` / `verifies` → `Relation::Satisfy` / `Relation::Verify` +
  requirement-trace view
- `controls` / `observes` / `governed_by` / `authorized_by` → `Relation::Allocation`
  or `Relation::Dependency`, or `Relation::Domain { kind: "controls" | … }` where no
  KerML relationship fits
- `traces_to` → `Relation::Domain { kind: "traces_to" }`

The view kind is chosen from `ufo_types::SysmlViewKind`; `ViewpointDefinition` /
`ViewUsage.exposedElement` from the model (where present) scopes which elements land
in each view.
