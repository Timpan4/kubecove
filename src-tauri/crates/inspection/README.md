# Resource inspection

Typed/dynamic resource reads, discovery, details, YAML, scope, topology, namespaces, events, metrics, incidents, RBAC and the ADR-approved guarded Kubernetes operations. Operations and resource actions stay together because they share client/dry-run semantics.

Depends on models and Kubernetes infrastructure, not GitOps or live-session implementations. Deterministic benchmark APIs are exposed only through `bench-support`. Development kubeconform lookup still uses the desktop package's `bin/` directory; packaged executable-side lookup is unchanged.
