# ADR 0012: Production-Shaped E2E Lab

## Status

Accepted.

## Context

The deterministic Kind lab originally used one-node workloads, hand-written permissive Argo CD CRDs, and Argo objects with no controllers or reconciliation. That covered API parsing but could not prove GitOps status, controller ownership, networking policy, metrics, storage, ingress, or incident inspection against real implementations.

The lab must stay isolated, reproducible, and small enough for local development and CI. It is not a production availability or scale simulator.

## Decision

`e2e:real` and `dev:kind` use the same production-shaped, one-node lab.

- Kind disables its default CNI. Cilium is the CNI and policy engine; kube-proxy remains enabled for Docker and Podman portability.
- The runner bootstraps pinned Cilium and Argo CD manifests only until the cluster can reconcile itself. It creates no Helm releases for them.
- A cluster-internal, read-only Git daemon serves run-specific desired state. One root Argo CD Application creates restricted projects, self-manages Argo CD, adopts Cilium, installs metrics, dynamic local storage, and Traefik, and generates healthy tenant applications.
- A separate Helm release represents direct Helm ownership and contains one declared, deterministic CrashLoop incident. It is not presented as Argo-managed.
- Represented platform capabilities use real controllers and status. Source-controlled fake CRDs and fabricated controller status are not allowed.
- Charts, manifests, tools, node images, and workload images are version- or digest-pinned. Downloads used by the runner are checksum-verified.
- Readiness is condition-based. Platform applications must be Synced and Healthy; the exact declared incident is the only expected unhealthy workload.
- Diagnostics contain allowlisted metadata/status plus bounded logs from the credential-free lab controllers and declared incident. Redaction still runs over every text artifact. Diagnostics exclude Secrets, kubeconfig contents, credentials, environment dumps, repository contents, and unfiltered Kind logs.

Cilium 1.19.6 is the latest stable release selected for the initial lab. Its upstream tested matrix ends at Kubernetes 1.34. KubeCove still exercises 1.35 and 1.36 in its own matrix; those combinations are locally validated, not an upstream Cilium support claim.

## Consequences

The lab costs more startup time and memory than static fixtures, but it proves the Kubernetes states KubeCove displays. One fixture graph also prevents `dev:kind` and real E2E behavior from drifting.

The lab does not model multi-node scheduling, high availability, cloud load balancers, service mesh, certificate management, or production capacity. Add a controller only when it proves a KubeCove surface.

Argo CD and Cilium upgrades are deliberate maintenance changes. Each upgrade must refresh checksums and pass the Kubernetes 1.34–1.36 matrix before the documented pins change.
