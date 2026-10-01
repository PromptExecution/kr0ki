# Skill: Kubernetes topology (`k8s-topology`, format `k8s-topology`)

## Choose it when
- The reader wants to know how workloads, services and config in a cluster relate.
- A manifest set needs a quick review: what selects what, who consumes which ConfigMap.
- You have actual manifests (YAML) to render; the diagram is derived from them, not drawn by hand.

## Not when
- You need IP segments and firewalls outside the cluster: use `network`.
- You need to show what artifact goes on which node in general: use `deployment`.
- You need call order between services at runtime: use `sequence`.

## Anatomy
- A Deployment (or StatefulSet, DaemonSet) is a workload that runs pods.
- A Service selects pods by label and gives them a stable address and ports.
- A ConfigMap or Secret is configuration a workload consumes (env or volume).
- A namespace groups resources; the same name in two namespaces is two different things.
- The edges come from the manifest itself: label selectors, `configMapRef`, volume references. Nothing is invented.

## What makes it good
- Feed it the complete related set (workload, Service, config) so the edges resolve; a lone Service or lone Deployment renders nothing at all.
- Keep to one application or namespace per diagram, roughly under 15 resources.
- Labels are the join key: make sure the Service selector matches the pod template labels exactly.
- Resources keep their namespace/kind/name identity; the diagram keys on that and labels with the name.
- Use a consistent `app` label so related resources are discoverable by humans too.

## What makes it bad
- Pasting a whole cluster dump, so the one story is lost.
- A Service selector that matches no pod labels, leaving a dangling Service.
- References to a ConfigMap that is not in the input, so the link is missing.
- Mixing several namespaces or environments (dev and prod) in one picture.
- Expecting runtime state (pod restarts, node placement) that a manifest does not contain.

## Questions to ask
- Which application or namespace should this cover, and can you paste its manifests?
- Which relationships matter most: exposure through Services, config consumption, or both?
- Is anything missing from the manifests (Ingress, Secret, another namespace) that the diagram should include?

## Contrast
Bad:
```k8s-topology
apiVersion: apps/v1
kind: Deployment
metadata:
  name: web
  namespace: demo
  labels:
    app: web
spec:
  replicas: 2
  selector:
    matchLabels:
      app: web
  template:
    metadata:
      labels:
        app: web
    spec:
      containers:
        - name: web
          image: web:latest
          envFrom:
            - configMapRef:
                name: app-config
---
apiVersion: v1
kind: Service
metadata:
  name: web
  namespace: demo
spec:
  selector:
    app: web
  ports:
    - port: 80
```
Good:
```k8s-topology
apiVersion: v1
kind: ConfigMap
metadata:
  name: app-config
  namespace: demo
data:
  LOG_LEVEL: info
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: web
  namespace: demo
  labels:
    app: web
spec:
  replicas: 2
  selector:
    matchLabels:
      app: web
  template:
    metadata:
      labels:
        app: web
    spec:
      containers:
        - name: web
          image: web:latest
          envFrom:
            - configMapRef:
                name: app-config
---
apiVersion: v1
kind: Service
metadata:
  name: web
  namespace: demo
spec:
  selector:
    app: web
  ports:
    - port: 80
```
The good version supplies the Service and ConfigMap alongside the Deployment, so the selector and config edges resolve; the bad one references `app-config` but never includes it, so that dependency is missing from the picture.
