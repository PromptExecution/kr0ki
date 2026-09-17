//! The Kubernetes pattern recognizer (kr0ki#12; PATTERNS-kubernetes.md).
//!
//! Normalizes raw Kubernetes API objects into the canonical UFO semantic
//! graph — the box-1→box-2 step for the Kubernetes source arm, which
//! `docs/PATTERNS-kubernetes.md` and `docs/PLAN-KR0KI-002.md` §2.1 both call
//! "the Kubernetes recognizer" (they fold the raw-verb-normalization and the
//! box-3 pattern-recognition concerns into one name; this module implements
//! only the former — normalizing overloaded k8s verbs into a canonical
//! [`UfoRelation`]). Lifting further into `ufo_types::sysml_model::Relation`
//! (box 4, per `PATTERNS-kubernetes.md` §4) is a separate, later stage, not
//! built here.
//!
//! ```text
//! Vec<k8s manifest JSON> ──▶ [THIS] ──▶ Vec<OntologicalEdge>
//!                                    └─▶ SysGraph (KubernetesRecognizer::recognize_to_sysgraph)
//! ```
//!
//! # Provenance: `philippemerle/KubeDiagrams`
//!
//! `docs/PATTERNS-kubernetes.md` directs porting `KubeDiagrams`'
//! (Apache-2.0) battle-tested GVK→relationship extraction catalogue
//! (`vendor/kubediagrams/bin/kube-diagrams.yaml`, vendored as a git
//! submodule) rather than re-deriving it from scratch. This module is that
//! port: every rule below traces to a specific `add_edge_to(...)` call site
//! or named helper (`add_owned_resources`, `add_service_account`, …) in that
//! file. It translates the *facts* the catalogue encodes (which field on
//! which kind references which other kind) into this crate's own static
//! Rust rules and its own `UfoRelation` classification — it does not call or
//! embed KubeDiagrams' Python. See `/NOTICE` for the required Apache-2.0
//! attribution.
//!
//! `docs/EVAL-kubediagrams.md` §3 already curated the JSONPath→relation
//! correspondence for the highest-value sites; this module's classification
//! choices follow that table directly where it applies, and extend it (with
//! the same UFO-category reasoning from `ontology.rs`) for sites EVAL didn't
//! spell out.
//!
//! # What's ported vs. deliberately deferred
//!
//! Covered: ownership (`ownerReferences`), Service label selection, HPA/VPA
//! scale targets, RBAC bindings, `serviceAccountName`, volume/env
//! ConfigMap+Secret references, `runtimeClassName`/`storageClassName`/
//! `ingressClassName`/`gatewayClassName` class references, `nodeName`,
//! PVC↔PV binding, StorageClass→CSIDriver, APIService→Service,
//! FlowSchema→PriorityLevelConfiguration.
//!
//! **Not yet ported** (tracked as kr0ki#12 follow-up, not silently missing):
//! admission-webhook `clientConfig.service`, `NetworkPolicy` ingress/egress
//! rules, `Endpoints`/`EndpointSlice` `targetRef`, and the Gateway API
//! surface (`Gateway`/`HTTPRoute`/`GRPCRoute`/`TLSRoute`/`BackendTLSPolicy`/
//! `ListenerSet`). Each needs either cross-manifest correlation this
//! module's simpler per-object model doesn't yet do, or GVK/field coverage
//! wide enough to deserve its own follow-up rather than a rushed subset.
//!
//! # Extension point (CRDs)
//!
//! [`KubernetesRecognizer::with_rule`] / [`KubernetesRecognizer::with_rules`]
//! append caller-supplied [`SimpleFieldRule`]s (e.g. for a CRD's own
//! reference fields) on top of [`builtin_rules`]. [`KubernetesRecognizer::rule_set_version`]
//! hashes the *active* rule set (built-in + extensions) — fold it into the
//! render-cache key (`cache::model_cache_key`'s `rule_set_version` field) so
//! a rule-set change invalidates cached derived diagrams (PLAN-KR0KI-002 §3).

use serde_json::Value;
use sha2::{Digest, Sha256};
use ufo_types::ontology::{OntologicalEdge, SourceAnchor, UfoRelation};
use ufo_types::sysml_model::ElementId;

/// A single-field, cluster-scoped-target reference rule: `(kind, api_group)`
/// at `field_path` names a target object of `target_kind`/`target_api_version`
/// by a bare `name` (no namespace — every built-in target kind here is
/// cluster-scoped: `RuntimeClass`, `Node`, `StorageClass`, `IngressClass`,
/// `GatewayClass`, `CSIDriver`, `PersistentVolume`,
/// `PriorityLevelConfiguration`). A namespaced-target rule would need this
/// struct extended; none of today's built-ins need it.
#[derive(Debug, Clone)]
pub struct SimpleFieldRule {
    pub kind: String,
    /// `""` for the core (`v1`) API group.
    pub api_group: String,
    /// Dot-separated path to the scalar field, e.g. `"spec.storageClassName"`.
    pub field_path: String,
    pub target_kind: String,
    pub target_api_version: String,
    pub relation: UfoRelation,
}

impl SimpleFieldRule {
    fn new(
        kind: &str,
        api_group: &str,
        field_path: &str,
        target_kind: &str,
        target_api_version: &str,
        relation: UfoRelation,
    ) -> Self {
        Self {
            kind: kind.to_string(),
            api_group: api_group.to_string(),
            field_path: field_path.to_string(),
            target_kind: target_kind.to_string(),
            target_api_version: target_api_version.to_string(),
            relation,
        }
    }
}

/// The built-in rule set, ported from `vendor/kubediagrams/bin/kube-diagrams.yaml`
/// (see module docs for the exact call sites).
pub fn builtin_rules() -> Vec<SimpleFieldRule> {
    vec![
        // PersistentVolume(Claim).spec.storageClassName — EVAL §3: "...storageClassName... -> scoped_by".
        SimpleFieldRule::new(
            "PersistentVolume",
            "",
            "spec.storageClassName",
            "StorageClass",
            "storage.k8s.io/v1",
            UfoRelation::ScopedBy,
        ),
        SimpleFieldRule::new(
            "PersistentVolumeClaim",
            "",
            "spec.storageClassName",
            "StorageClass",
            "storage.k8s.io/v1",
            UfoRelation::ScopedBy,
        ),
        // PVC.spec.volumeName -> PV — EVAL §3 exact rule: "PVC spec.volumeName -> PV => binds".
        SimpleFieldRule::new(
            "PersistentVolumeClaim",
            "",
            "spec.volumeName",
            "PersistentVolume",
            "v1",
            UfoRelation::Binds,
        ),
        // CSIStorageCapacity.storageClassName -> StorageClass.
        SimpleFieldRule::new(
            "CSIStorageCapacity",
            "storage.k8s.io",
            "storageClassName",
            "StorageClass",
            "storage.k8s.io/v1",
            UfoRelation::ScopedBy,
        ),
        // Ingress.spec.ingressClassName — EVAL §3: "...ingressClassName... -> scoped_by".
        SimpleFieldRule::new(
            "Ingress",
            "networking.k8s.io",
            "spec.ingressClassName",
            "IngressClass",
            "networking.k8s.io/v1",
            UfoRelation::ScopedBy,
        ),
        // Gateway.spec.gatewayClassName — EVAL §3: "...gatewayClassName... -> scoped_by".
        SimpleFieldRule::new(
            "Gateway",
            "gateway.networking.k8s.io",
            "spec.gatewayClassName",
            "GatewayClass",
            "gateway.networking.k8s.io/v1",
            UfoRelation::ScopedBy,
        ),
        // StorageClass.provisioner -> CSIDriver: the class names the capability
        // provider it requires. Not in EVAL §3's table; "requires" is the
        // "consumer -> capability/interface" reading (ontology.rs).
        SimpleFieldRule::new(
            "StorageClass",
            "storage.k8s.io",
            "provisioner",
            "CSIDriver",
            "storage.k8s.io/v1",
            UfoRelation::Requires,
        ),
        // FlowSchema.spec.priorityLevelConfiguration.name -> PriorityLevelConfiguration:
        // a structural association, not in EVAL §3; `binds` is the generic
        // "relator mediating two endurants" reading.
        SimpleFieldRule::new(
            "FlowSchema",
            "flowcontrol.apiserver.k8s.io",
            "spec.priorityLevelConfiguration.name",
            "PriorityLevelConfiguration",
            "flowcontrol.apiserver.k8s.io/v1",
            UfoRelation::Binds,
        ),
    ]
}

/// A Kubernetes-manifest → UFO-graph recognizer: [`builtin_rules`] plus any
/// caller-supplied extension rules (the CRD extension point).
#[derive(Debug, Clone, Default)]
pub struct KubernetesRecognizer {
    rules: Vec<SimpleFieldRule>,
}

impl KubernetesRecognizer {
    /// A recognizer seeded with only the built-in rule set.
    pub fn new() -> Self {
        Self {
            rules: builtin_rules(),
        }
    }

    /// Append one extension rule (e.g. for a CRD's own reference field).
    #[must_use]
    pub fn with_rule(mut self, rule: SimpleFieldRule) -> Self {
        self.rules.push(rule);
        self
    }

    /// Append several extension rules at once.
    #[must_use]
    pub fn with_rules(mut self, rules: impl IntoIterator<Item = SimpleFieldRule>) -> Self {
        self.rules.extend(rules);
        self
    }

    /// A deterministic hash of the *active* rule set (built-ins + any
    /// extensions), lowercase hex SHA-256. Fold this into the render-cache
    /// key (PLAN-KR0KI-002 §3) so a rule-set change — built-in or CRD
    /// extension — invalidates every diagram derived from it.
    pub fn rule_set_version(&self) -> String {
        let mut h = Sha256::new();
        h.update(b"kr0ki-k8s-recognizer/v1");
        for r in &self.rules {
            h.update([0x1f]);
            h.update(r.kind.as_bytes());
            h.update([0x1f]);
            h.update(r.api_group.as_bytes());
            h.update([0x1f]);
            h.update(r.field_path.as_bytes());
            h.update([0x1f]);
            h.update(r.target_kind.as_bytes());
            h.update([0x1f]);
            h.update(r.target_api_version.as_bytes());
            h.update([0x1f]);
            h.update(r.relation.canonical_name().as_bytes());
        }
        let digest = h.finalize();
        let mut s = String::with_capacity(64);
        for b in digest {
            s.push_str(&format!("{b:02x}"));
        }
        s
    }

    /// Recognize every edge this rule set + the built-in structural helpers
    /// can extract from `manifests` (one JSON k8s API object per entry — a
    /// caller with YAML input converts it first, e.g. via `serde_yaml`).
    ///
    /// Cross-manifest rules (Service selection, RBAC subject/ref
    /// resolution) see the whole batch; per-object rules see one manifest
    /// at a time. Order is not guaranteed to be stable across calls with a
    /// reordered `manifests` slice.
    pub fn recognize(&self, manifests: &[Value]) -> Vec<OntologicalEdge> {
        let mut edges = Vec::new();
        for m in manifests {
            edges.extend(self.simple_field_edges(m));
            edges.extend(owner_reference_edges(m));
            edges.extend(controller_scale_target_edges(m));
            edges.extend(rbac_binding_edges(m));
            edges.extend(service_account_edges(m));
            edges.extend(volume_reference_edges(m));
            edges.extend(container_env_edges(m));
            edges.extend(api_service_edges(m));
            edges.extend(runtime_class_edges(m));
            edges.extend(node_name_edges(m));
        }
        edges.extend(service_selector_edges(manifests));
        edges
    }

    /// [`Self::recognize`], wrapped in `ufo_types::sysgraph::SysGraph`
    /// (`docs/TODO.md` box 2's "Graph container type") — a node per
    /// manifest in the batch, `UfoStereotype::SubKind` of `"K8sObject"`
    /// exactly as `docs/PATTERNS-kubernetes.md` §3 already specifies
    /// ("Deployment, Service, Pod, ConfigMap, … → `SubKind` of
    /// `K8sObject`"), plus every edge `recognize` produces.
    ///
    /// Unlike [`crate::rust_recognizer::to_sysgraph`] (whose edges can never
    /// dangle — it builds both nodes and edges from one complete tree walk),
    /// an edge here legitimately can: a `SimpleFieldRule` target (a
    /// `RuntimeClass`, a `Node`, …) is only a node if its own manifest is
    /// also present in `manifests`. [`ufo_types::sysgraph::SysGraph::dangling_edges`]
    /// is the caller-invoked check for exactly this — expected to be
    /// non-empty for a partial manifest batch, not a bug.
    pub fn recognize_to_sysgraph(&self, manifests: &[Value]) -> ufo_types::sysgraph::SysGraph {
        use ufo_types::stereotype::UfoStereotype;
        use ufo_types::sysgraph::{OntologicalNode, SysGraph};

        let mut graph = SysGraph::new();
        for m in manifests {
            let (Some(id), Some(kind)) = (manifest_element_id(m), manifest_kind(m)) else {
                continue;
            };
            let label = manifest_name(m).unwrap_or(kind).to_string();
            let stereotype = UfoStereotype::SubKind {
                name: kind.to_string(),
                parent: "K8sObject".to_string(),
            };
            graph.push_node(OntologicalNode::with_label(id, stereotype, label));
        }
        for edge in self.recognize(manifests) {
            graph.push_edge(edge);
        }
        graph
    }

    fn simple_field_edges(&self, m: &Value) -> Vec<OntologicalEdge> {
        let Some(kind) = manifest_kind(m) else {
            return Vec::new();
        };
        let group = manifest_api_group(m);
        self.rules
            .iter()
            .filter(|r| r.kind == kind && r.api_group == group)
            .filter_map(|r| {
                let name = get_path(m, &r.field_path)?.as_str()?;
                let source = manifest_element_id(m)?;
                let target = element_id(&r.target_kind, None, name);
                Some(edge(m, source, target, r.relation))
            })
            .collect()
    }
}

// ---------------------------------------------------------------------
// Manifest helpers
// ---------------------------------------------------------------------

fn get_path<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(v, |cur, seg| cur.get(seg))
}

fn manifest_kind(m: &Value) -> Option<&str> {
    m.get("kind")?.as_str()
}

/// The API group from `apiVersion` (`"group/version"`, or just `"version"`
/// for the core group, which normalizes to `""`).
fn manifest_api_group(m: &Value) -> &str {
    m.get("apiVersion")
        .and_then(Value::as_str)
        .and_then(|av| av.split_once('/').map(|(g, _)| g))
        .unwrap_or("")
}

fn manifest_name(m: &Value) -> Option<&str> {
    get_path(m, "metadata.name")?.as_str()
}

fn manifest_namespace(m: &Value) -> Option<&str> {
    get_path(m, "metadata.namespace").and_then(Value::as_str)
}

fn manifest_uid(m: &Value) -> Option<String> {
    get_path(m, "metadata.uid")
        .and_then(Value::as_str)
        .map(String::from)
}

fn manifest_labels(m: &Value) -> Option<&serde_json::Map<String, Value>> {
    get_path(m, "metadata.labels")?.as_object()
}

/// This manifest's own [`ElementId`]: `k8s:{namespace|-}/{kind}/{name}`.
fn manifest_element_id(m: &Value) -> Option<ElementId> {
    let kind = manifest_kind(m)?;
    let name = manifest_name(m)?;
    Some(element_id(kind, manifest_namespace(m), name))
}

fn element_id(kind: &str, namespace: Option<&str>, name: &str) -> ElementId {
    ElementId::new(format!("k8s:{}/{kind}/{name}", namespace.unwrap_or("-")))
}

/// Build an edge with `source`'s own identity as the [`SourceAnchor::K8sObject`] provenance.
fn edge(
    source_manifest: &Value,
    source: ElementId,
    target: ElementId,
    relation: UfoRelation,
) -> OntologicalEdge {
    let id = format!("{source}~{}~{target}", relation.canonical_name());
    let mut e = OntologicalEdge::new(id, source, target, relation);
    if let (Some(api_version), Some(kind), Some(name)) = (
        source_manifest.get("apiVersion").and_then(Value::as_str),
        manifest_kind(source_manifest),
        manifest_name(source_manifest),
    ) {
        e.provenance.push(SourceAnchor::K8sObject {
            api_version: api_version.to_string(),
            kind: kind.to_string(),
            namespace: manifest_namespace(source_manifest).map(String::from),
            name: name.to_string(),
            uid: manifest_uid(source_manifest),
        });
    }
    e
}

/// The dotted path to this kind's embedded `PodSpec`, if it has one — unifies
/// the workload family (`Pod` itself, `PodTemplate`, and every controller
/// that wraps a pod template) so `runtimeClassName`/`serviceAccountName`/
/// `volumes`/containers rules need one lookup each, not one per kind.
fn pod_spec_path(kind: &str, group: &str) -> Option<&'static str> {
    match (kind, group) {
        ("Pod", "") => Some("spec"),
        ("PodTemplate", "") => Some("template.spec"),
        ("CronJob", "batch") => Some("spec.jobTemplate.spec.template.spec"),
        ("DaemonSet", "apps")
        | ("Deployment", "apps")
        | ("ReplicaSet", "apps")
        | ("StatefulSet", "apps")
        | ("Job", "batch") => Some("spec.template.spec"),
        ("ReplicationController", "") => Some("spec.template.spec"),
        _ => None,
    }
}

// ---------------------------------------------------------------------
// Structural helpers (mirror KubeDiagrams' named helper functions)
// ---------------------------------------------------------------------

/// `metadata.ownerReferences[]` — KubeDiagrams' `add_owned_resources()`
/// (OWNER edge kind). EVAL §3: "`OWNER` -> `has_part` (inverse `member_of`)".
/// Edge direction here: owner `has_part` owned (the owning controller "has as
/// part" the object it created), matching that reading.
fn owner_reference_edges(m: &Value) -> Vec<OntologicalEdge> {
    let Some(refs) = get_path(m, "metadata.ownerReferences").and_then(Value::as_array) else {
        return Vec::new();
    };
    let Some(target) = manifest_element_id(m) else {
        return Vec::new();
    };
    refs.iter()
        .filter_map(|r| {
            let kind = r.get("kind")?.as_str()?;
            let name = r.get("name")?.as_str()?;
            let source = element_id(kind, manifest_namespace(m), name);
            Some(edge(m, source, target.clone(), UfoRelation::HasPart))
        })
        .collect()
}

/// `HorizontalPodAutoscaler.spec.scaleTargetRef` /
/// `VerticalPodAutoscaler.spec.targetRef` — KubeDiagrams' `add_resource(...)`.
/// EVAL §3: "HPA/VPA `scaleTargetRef` -> `controls`".
fn controller_scale_target_edges(m: &Value) -> Vec<OntologicalEdge> {
    let kind = manifest_kind(m).unwrap_or_default();
    let group = manifest_api_group(m);
    let field = match (kind, group) {
        ("HorizontalPodAutoscaler", "autoscaling") => "spec.scaleTargetRef",
        ("VerticalPodAutoscaler", "autoscaling.k8s.io") => "spec.targetRef",
        _ => return Vec::new(),
    };
    let Some(target_ref) = get_path(m, field) else {
        return Vec::new();
    };
    let (Some(target_kind), Some(name), Some(source)) = (
        target_ref.get("kind").and_then(Value::as_str),
        target_ref.get("name").and_then(Value::as_str),
        manifest_element_id(m),
    ) else {
        return Vec::new();
    };
    // ObjectReference carries no namespace field; HPA/VPA are namespaced and
    // scale a target in their own namespace.
    let target = element_id(target_kind, manifest_namespace(m), name);
    vec![edge(m, source, target, UfoRelation::Controls)]
}

/// `RoleBinding`/`ClusterRoleBinding` `roleRef` + `subjects` — KubeDiagrams'
/// `add_role("roleRef")` + `add_subjects()`. EVAL §3: "RoleBinding/CRB
/// `roleRef` + `subjects` -> `authorized_by`".
fn rbac_binding_edges(m: &Value) -> Vec<OntologicalEdge> {
    let kind = manifest_kind(m).unwrap_or_default();
    if kind != "RoleBinding" && kind != "ClusterRoleBinding" {
        return Vec::new();
    }
    let Some(source) = manifest_element_id(m) else {
        return Vec::new();
    };
    let mut edges = Vec::new();
    if let Some(role_ref) = m.get("roleRef") {
        if let (Some(rk), Some(rn)) = (
            role_ref.get("kind").and_then(Value::as_str),
            role_ref.get("name").and_then(Value::as_str),
        ) {
            // ClusterRole is cluster-scoped; Role is namespaced (same namespace as the binding).
            let ns = if rk == "ClusterRole" {
                None
            } else {
                manifest_namespace(m)
            };
            let target = element_id(rk, ns, rn);
            edges.push(edge(m, source.clone(), target, UfoRelation::AuthorizedBy));
        }
    }
    if let Some(subjects) = m.get("subjects").and_then(Value::as_array) {
        for s in subjects {
            if let (Some(sk), Some(sn)) = (
                s.get("kind").and_then(Value::as_str),
                s.get("name").and_then(Value::as_str),
            ) {
                let ns = s
                    .get("namespace")
                    .and_then(Value::as_str)
                    .or_else(|| manifest_namespace(m));
                let target = element_id(sk, ns, sn);
                edges.push(edge(m, source.clone(), target, UfoRelation::AuthorizedBy));
            }
        }
    }
    edges
}

/// `{podSpecPath}.serviceAccountName` — KubeDiagrams' `add_service_account(path)`.
/// Not in EVAL §3's table as a single answer ("binds / authorized_by"); the
/// binding call site (`roleRef`/`subjects`) is what carries `authorized_by`
/// here, so this one is the structural "attaches, mounts, associates" reading.
fn service_account_edges(m: &Value) -> Vec<OntologicalEdge> {
    let kind = manifest_kind(m).unwrap_or_default();
    let group = manifest_api_group(m);
    let Some(prefix) = pod_spec_path(kind, group) else {
        return Vec::new();
    };
    let field = format!("{prefix}.serviceAccountName");
    let Some(name) = get_path(m, &field).and_then(Value::as_str) else {
        return Vec::new();
    };
    let Some(source) = manifest_element_id(m) else {
        return Vec::new();
    };
    let target = element_id("ServiceAccount", manifest_namespace(m), name);
    vec![edge(m, source, target, UfoRelation::Binds)]
}

/// `{podSpecPath}.volumes[].{configMap|secret|persistentVolumeClaim}` —
/// KubeDiagrams' `add_all_volume_resources(path)`. EVAL §3: "volume ->
/// ConfigMap/Secret/projected; ... -> `requires` (mount/consume)".
fn volume_reference_edges(m: &Value) -> Vec<OntologicalEdge> {
    let kind = manifest_kind(m).unwrap_or_default();
    let group = manifest_api_group(m);
    let Some(prefix) = pod_spec_path(kind, group) else {
        return Vec::new();
    };
    let Some(volumes) = get_path(m, &format!("{prefix}.volumes")).and_then(Value::as_array) else {
        return Vec::new();
    };
    let Some(source) = manifest_element_id(m) else {
        return Vec::new();
    };
    let ns = manifest_namespace(m);
    volumes
        .iter()
        .filter_map(|v| {
            let (target_kind, name) = if let Some(n) = v
                .get("configMap")
                .and_then(|c| c.get("name"))
                .and_then(Value::as_str)
            {
                ("ConfigMap", n)
            } else if let Some(n) = v
                .get("secret")
                .and_then(|s| s.get("secretName"))
                .and_then(Value::as_str)
            {
                ("Secret", n)
            } else {
                let n = v
                    .get("persistentVolumeClaim")
                    .and_then(|p| p.get("claimName"))
                    .and_then(Value::as_str)?;
                ("PersistentVolumeClaim", n)
            };
            let target = element_id(target_kind, ns, name);
            Some(edge(m, source.clone(), target, UfoRelation::Requires))
        })
        .collect()
}

/// `{podSpecPath}.{containers|initContainers|ephemeralContainers}[].{envFrom[].*Ref, env[].valueFrom.*KeyRef}`
/// — KubeDiagrams' `add_containers_env_value_from_and_env_from(path)`.
/// EVAL §3: "`envFrom` / `valueFrom` -> ConfigMap/Secret -> `requires`".
fn container_env_edges(m: &Value) -> Vec<OntologicalEdge> {
    let kind = manifest_kind(m).unwrap_or_default();
    let group = manifest_api_group(m);
    let Some(prefix) = pod_spec_path(kind, group) else {
        return Vec::new();
    };
    let Some(source) = manifest_element_id(m) else {
        return Vec::new();
    };
    let ns = manifest_namespace(m);
    let mut edges = Vec::new();
    for list_field in ["containers", "initContainers", "ephemeralContainers"] {
        let Some(containers) =
            get_path(m, &format!("{prefix}.{list_field}")).and_then(Value::as_array)
        else {
            continue;
        };
        for c in containers {
            if let Some(env_from) = c.get("envFrom").and_then(Value::as_array) {
                for ef in env_from {
                    if let Some(n) = ef
                        .get("configMapRef")
                        .and_then(|r| r.get("name"))
                        .and_then(Value::as_str)
                    {
                        edges.push(edge(
                            m,
                            source.clone(),
                            element_id("ConfigMap", ns, n),
                            UfoRelation::Requires,
                        ));
                    }
                    if let Some(n) = ef
                        .get("secretRef")
                        .and_then(|r| r.get("name"))
                        .and_then(Value::as_str)
                    {
                        edges.push(edge(
                            m,
                            source.clone(),
                            element_id("Secret", ns, n),
                            UfoRelation::Requires,
                        ));
                    }
                }
            }
            if let Some(env) = c.get("env").and_then(Value::as_array) {
                for e in env {
                    let Some(vf) = e.get("valueFrom") else {
                        continue;
                    };
                    if let Some(n) = vf
                        .get("configMapKeyRef")
                        .and_then(|r| r.get("name"))
                        .and_then(Value::as_str)
                    {
                        edges.push(edge(
                            m,
                            source.clone(),
                            element_id("ConfigMap", ns, n),
                            UfoRelation::Requires,
                        ));
                    }
                    if let Some(n) = vf
                        .get("secretKeyRef")
                        .and_then(|r| r.get("name"))
                        .and_then(Value::as_str)
                    {
                        edges.push(edge(
                            m,
                            source.clone(),
                            element_id("Secret", ns, n),
                            UfoRelation::Requires,
                        ));
                    }
                }
            }
        }
    }
    edges
}

/// `APIService.spec.service` — KubeDiagrams' explicit `add_edge_to` call.
/// Same family as the other "-> Service" routing sites EVAL §3 classifies
/// `routes_to` (Ingress backends, Gateway `parentRefs`, webhook
/// `clientConfig.service`).
fn api_service_edges(m: &Value) -> Vec<OntologicalEdge> {
    if manifest_kind(m) != Some("APIService") {
        return Vec::new();
    }
    let Some(svc) = get_path(m, "spec.service") else {
        return Vec::new();
    };
    let Some(name) = svc.get("name").and_then(Value::as_str) else {
        return Vec::new();
    };
    let Some(source) = manifest_element_id(m) else {
        return Vec::new();
    };
    let ns = svc.get("namespace").and_then(Value::as_str);
    let target = element_id("Service", ns, name);
    vec![edge(m, source, target, UfoRelation::RoutesTo)]
}

/// `{podSpecPath}.runtimeClassName` on every pod-spec-bearing kind —
/// KubeDiagrams' per-kind explicit `add_edge_to(".../runtimeClassName", ...)`
/// call sites. EVAL §3: "...`runtimeClassName`... -> `scoped_by`".
fn runtime_class_edges(m: &Value) -> Vec<OntologicalEdge> {
    let kind = manifest_kind(m).unwrap_or_default();
    let group = manifest_api_group(m);
    let Some(prefix) = pod_spec_path(kind, group) else {
        return Vec::new();
    };
    let Some(name) = get_path(m, &format!("{prefix}.runtimeClassName")).and_then(Value::as_str)
    else {
        return Vec::new();
    };
    let Some(source) = manifest_element_id(m) else {
        return Vec::new();
    };
    let target = element_id("RuntimeClass", None, name);
    vec![edge(m, source, target, UfoRelation::ScopedBy)]
}

/// `Pod.spec.nodeName` / `PodTemplate.template.spec.nodeName` —
/// KubeDiagrams scopes this to `Pod` and `PodTemplate` only (not the
/// controller-wrapped templates). EVAL §3 exact rule: "Pod `spec.nodeName`
/// -> Node; ... -> `hosted_by`".
fn node_name_edges(m: &Value) -> Vec<OntologicalEdge> {
    let field = match manifest_kind(m) {
        Some("Pod") if manifest_api_group(m).is_empty() => "spec.nodeName",
        Some("PodTemplate") if manifest_api_group(m).is_empty() => "template.spec.nodeName",
        _ => return Vec::new(),
    };
    let Some(name) = get_path(m, field).and_then(Value::as_str) else {
        return Vec::new();
    };
    let Some(source) = manifest_element_id(m) else {
        return Vec::new();
    };
    let target = element_id("Node", None, name);
    vec![edge(m, source, target, UfoRelation::HostedBy)]
}

/// `Service.spec.selector` matched by equality against every other
/// pod-spec-bearing manifest's `metadata.labels` in the batch — KubeDiagrams'
/// `add_edges_for_service()` (its `SELECTOR` edge kind). EVAL intro: "`SELECTOR`
/// -> `selects` line up 1:1". No live cluster is available to resolve which
/// Pods a Service actually fronts, so this matches against whichever
/// pod-spec-bearing manifest (`Pod` or a pod-template-wrapping controller;
/// see [`pod_spec_path`]) is in the batch — the same static-manifest
/// simplification KubeDiagrams itself makes when not pointed at a live
/// cluster. Candidates are restricted to pod-spec-bearing kinds, not "any
/// kind with matching labels": the KubeDiagrams oracle test caught a
/// Service->Service false edge (a LoadBalancer `Service` mirroring its
/// target's `app` label) before this restriction was added.
fn service_selector_edges(manifests: &[Value]) -> Vec<OntologicalEdge> {
    let mut edges = Vec::new();
    for svc in manifests {
        if manifest_kind(svc) != Some("Service") {
            continue;
        }
        let Some(selector) = get_path(svc, "spec.selector").and_then(Value::as_object) else {
            continue;
        };
        if selector.is_empty() {
            continue;
        }
        let Some(source) = manifest_element_id(svc) else {
            continue;
        };
        let svc_ns = manifest_namespace(svc);
        for candidate in manifests {
            if std::ptr::eq(candidate, svc) {
                continue;
            }
            // Restrict candidates to pod-spec-bearing kinds (Pod, and every
            // controller wrapping a pod template) — a Service's selector
            // targets Pods, never another Service. Confirmed by the
            // KubeDiagrams oracle test: without this, a LoadBalancer Service
            // mirroring its target's `app` label (e.g. `frontend-external`
            // alongside `frontend`) produced a false Service->Service edge.
            let candidate_kind = manifest_kind(candidate).unwrap_or_default();
            if pod_spec_path(candidate_kind, manifest_api_group(candidate)).is_none() {
                continue;
            }
            if manifest_namespace(candidate) != svc_ns {
                continue;
            }
            let Some(labels) = manifest_labels(candidate) else {
                continue;
            };
            let matches = selector
                .iter()
                .all(|(k, v)| labels.get(k).and_then(Value::as_str) == v.as_str());
            if !matches {
                continue;
            }
            if let Some(target) = manifest_element_id(candidate) {
                edges.push(edge(svc, source.clone(), target, UfoRelation::Selects));
            }
        }
    }
    edges
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn storage_class_reference_lifts_to_scoped_by() {
        let pvc = json!({
            "apiVersion": "v1", "kind": "PersistentVolumeClaim",
            "metadata": { "name": "data", "namespace": "ns1" },
            "spec": { "storageClassName": "fast", "volumeName": "pv-1" }
        });
        let rec = KubernetesRecognizer::new();
        let edges = rec.recognize(std::slice::from_ref(&pvc));
        let sc = edges
            .iter()
            .find(|e| e.relation == UfoRelation::ScopedBy)
            .expect("scoped_by edge");
        assert_eq!(sc.target, ElementId::new("k8s:-/StorageClass/fast"));
        let bind = edges
            .iter()
            .find(|e| e.relation == UfoRelation::Binds)
            .expect("binds edge");
        assert_eq!(bind.target, ElementId::new("k8s:-/PersistentVolume/pv-1"));
    }

    #[test]
    fn owner_reference_lifts_to_has_part_from_owner_to_owned() {
        let rs = json!({
            "apiVersion": "apps/v1", "kind": "ReplicaSet",
            "metadata": {
                "name": "web-abc123", "namespace": "ns1",
                "ownerReferences": [{ "kind": "Deployment", "name": "web", "apiVersion": "apps/v1" }]
            }
        });
        let edges = KubernetesRecognizer::new().recognize(std::slice::from_ref(&rs));
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].relation, UfoRelation::HasPart);
        assert_eq!(edges[0].source, ElementId::new("k8s:ns1/Deployment/web"));
        assert_eq!(
            edges[0].target,
            ElementId::new("k8s:ns1/ReplicaSet/web-abc123")
        );
    }

    #[test]
    fn hpa_scale_target_lifts_to_controls() {
        let hpa = json!({
            "apiVersion": "autoscaling/v2", "kind": "HorizontalPodAutoscaler",
            "metadata": { "name": "web-hpa", "namespace": "ns1" },
            "spec": { "scaleTargetRef": { "apiVersion": "apps/v1", "kind": "Deployment", "name": "web" } }
        });
        let edges = KubernetesRecognizer::new().recognize(std::slice::from_ref(&hpa));
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].relation, UfoRelation::Controls);
        assert_eq!(edges[0].target, ElementId::new("k8s:ns1/Deployment/web"));
    }

    #[test]
    fn role_binding_lifts_role_ref_and_subjects_to_authorized_by() {
        let rb = json!({
            "apiVersion": "rbac.authorization.k8s.io/v1", "kind": "RoleBinding",
            "metadata": { "name": "rb1", "namespace": "ns1" },
            "roleRef": { "kind": "Role", "name": "viewer", "apiGroup": "rbac.authorization.k8s.io" },
            "subjects": [{ "kind": "ServiceAccount", "name": "sa1" }]
        });
        let edges = KubernetesRecognizer::new().recognize(std::slice::from_ref(&rb));
        assert_eq!(edges.len(), 2);
        assert!(edges
            .iter()
            .all(|e| e.relation == UfoRelation::AuthorizedBy));
        assert!(edges
            .iter()
            .any(|e| e.target == ElementId::new("k8s:ns1/Role/viewer")));
        assert!(edges
            .iter()
            .any(|e| e.target == ElementId::new("k8s:ns1/ServiceAccount/sa1")));
    }

    #[test]
    fn pod_service_account_and_volumes_and_env_and_runtime_class_and_node() {
        let pod = json!({
            "apiVersion": "v1", "kind": "Pod",
            "metadata": { "name": "web-1", "namespace": "ns1" },
            "spec": {
                "serviceAccountName": "sa1",
                "runtimeClassName": "gvisor",
                "nodeName": "node-7",
                "volumes": [
                    { "name": "cfg", "configMap": { "name": "app-cfg" } },
                    { "name": "sec", "secret": { "secretName": "app-secret" } }
                ],
                "containers": [{
                    "name": "app",
                    "envFrom": [{ "configMapRef": { "name": "env-cfg" } }],
                    "env": [{ "name": "X", "valueFrom": { "secretKeyRef": { "name": "x-secret" } } }]
                }]
            }
        });
        let edges = KubernetesRecognizer::new().recognize(std::slice::from_ref(&pod));

        let has = |relation: UfoRelation, target: &str| {
            edges
                .iter()
                .any(|e| e.relation == relation && e.target == ElementId::new(target))
        };
        assert!(has(UfoRelation::Binds, "k8s:ns1/ServiceAccount/sa1"));
        assert!(has(UfoRelation::ScopedBy, "k8s:-/RuntimeClass/gvisor"));
        assert!(has(UfoRelation::HostedBy, "k8s:-/Node/node-7"));
        assert!(has(UfoRelation::Requires, "k8s:ns1/ConfigMap/app-cfg"));
        assert!(has(UfoRelation::Requires, "k8s:ns1/Secret/app-secret"));
        assert!(has(UfoRelation::Requires, "k8s:ns1/ConfigMap/env-cfg"));
        assert!(has(UfoRelation::Requires, "k8s:ns1/Secret/x-secret"));
    }

    #[test]
    fn service_selector_matches_pods_by_label_in_same_namespace() {
        let svc = json!({
            "apiVersion": "v1", "kind": "Service",
            "metadata": { "name": "web", "namespace": "ns1" },
            "spec": { "selector": { "app": "web" } }
        });
        let pod_match = json!({
            "apiVersion": "v1", "kind": "Pod",
            "metadata": { "name": "web-1", "namespace": "ns1", "labels": { "app": "web", "extra": "x" } }
        });
        let pod_other_ns = json!({
            "apiVersion": "v1", "kind": "Pod",
            "metadata": { "name": "web-2", "namespace": "ns2", "labels": { "app": "web" } }
        });
        let pod_no_match = json!({
            "apiVersion": "v1", "kind": "Pod",
            "metadata": { "name": "db-1", "namespace": "ns1", "labels": { "app": "db" } }
        });
        let manifests = vec![svc, pod_match, pod_other_ns, pod_no_match];
        let edges = KubernetesRecognizer::new().recognize(&manifests);
        let selects: Vec<_> = edges
            .iter()
            .filter(|e| e.relation == UfoRelation::Selects)
            .collect();
        assert_eq!(selects.len(), 1);
        assert_eq!(selects[0].target, ElementId::new("k8s:ns1/Pod/web-1"));
    }

    #[test]
    fn crd_extension_point_adds_a_rule_without_touching_builtins() {
        let rec = KubernetesRecognizer::new().with_rule(SimpleFieldRule::new(
            "MyCrd",
            "example.com",
            "spec.clusterRef",
            "Cluster",
            "example.com/v1",
            UfoRelation::ScopedBy,
        ));
        let crd = json!({
            "apiVersion": "example.com/v1", "kind": "MyCrd",
            "metadata": { "name": "thing1", "namespace": "ns1" },
            "spec": { "clusterRef": "prod" }
        });
        let edges = rec.recognize(std::slice::from_ref(&crd));
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].relation, UfoRelation::ScopedBy);
        assert_eq!(edges[0].target, ElementId::new("k8s:-/Cluster/prod"));
    }

    #[test]
    fn rule_set_version_changes_when_rules_change_and_is_deterministic() {
        let base = KubernetesRecognizer::new();
        let v1a = base.rule_set_version();
        let v1b = KubernetesRecognizer::new().rule_set_version();
        assert_eq!(v1a, v1b, "same rule set hashes identically every time");
        assert_eq!(v1a.len(), 64);
        assert!(v1a.bytes().all(|c| c.is_ascii_hexdigit()));

        let extended = base.with_rule(SimpleFieldRule::new(
            "MyCrd",
            "example.com",
            "spec.clusterRef",
            "Cluster",
            "example.com/v1",
            UfoRelation::ScopedBy,
        ));
        assert_ne!(
            v1a,
            extended.rule_set_version(),
            "adding a CRD rule must change the version"
        );
    }

    #[test]
    fn manifest_with_no_recognizable_field_yields_no_edges() {
        let cm = json!({
            "apiVersion": "v1", "kind": "ConfigMap",
            "metadata": { "name": "plain", "namespace": "ns1" },
            "data": { "k": "v" }
        });
        let edges = KubernetesRecognizer::new().recognize(std::slice::from_ref(&cm));
        assert!(edges.is_empty());
    }

    fn playbook_demo_manifests() -> Vec<Value> {
        // The exact three-object bundle kr0ki_core::examples' k8s-topology-
        // web-service fixture uses, live-verified end to end (kr0ki PR #31).
        serde_yaml::Deserializer::from_str(
            "apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: app-config\n  namespace: demo\n\
             data:\n  LOG_LEVEL: info\n---\napiVersion: apps/v1\nkind: Deployment\nmetadata:\n  \
             name: web\n  namespace: demo\n  labels:\n    app: web\nspec:\n  replicas: 2\n  \
             selector:\n    matchLabels:\n      app: web\n  template:\n    metadata:\n      \
             labels:\n        app: web\n    spec:\n      containers:\n        - name: web\n          \
             image: web:latest\n          envFrom:\n            - configMapRef:\n                \
             name: app-config\n---\napiVersion: v1\nkind: Service\nmetadata:\n  name: web\n  \
             namespace: demo\nspec:\n  selector:\n    app: web\n  ports:\n    - port: 80\n",
        )
        .map(|doc| serde::Deserialize::deserialize(doc).expect("valid manifest doc"))
        .collect()
    }

    #[test]
    fn recognize_to_sysgraph_has_no_dangling_edges_for_a_self_contained_batch() {
        let manifests = playbook_demo_manifests();
        let graph = KubernetesRecognizer::new().recognize_to_sysgraph(&manifests);
        assert_eq!(graph.nodes.len(), 3);
        assert!(
            graph.dangling_edges().is_empty(),
            "every referenced object is in this batch: {:?}",
            graph.dangling_edges()
        );
    }

    #[test]
    fn recognize_to_sysgraph_nodes_are_subkind_of_k8sobject() {
        use ufo_types::stereotype::UfoStereotype;

        let manifests = playbook_demo_manifests();
        let graph = KubernetesRecognizer::new().recognize_to_sysgraph(&manifests);
        let deployment = graph
            .node(&ElementId::new("k8s:demo/Deployment/web"))
            .expect("deployment node");
        assert_eq!(deployment.label.as_deref(), Some("web"));
        assert_eq!(
            deployment.stereotype,
            UfoStereotype::SubKind {
                name: "Deployment".to_string(),
                parent: "K8sObject".to_string(),
            }
        );
    }

    #[test]
    fn recognize_to_sysgraph_reports_a_dangling_edge_for_a_reference_outside_the_batch() {
        let pvc = json!({
            "apiVersion": "v1", "kind": "PersistentVolumeClaim",
            "metadata": { "name": "data", "namespace": "ns1" },
            "spec": { "storageClassName": "fast", "volumeName": "pv-1" }
        });
        // Neither the StorageClass nor the PersistentVolume this PVC
        // references is included in the batch -- both referenced edges
        // must dangle, not silently vanish or panic.
        let graph = KubernetesRecognizer::new().recognize_to_sysgraph(std::slice::from_ref(&pvc));
        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(graph.dangling_edges().len(), 2);
    }
}
