# Resource benchmark fixtures

Prepared Kubernetes objects, resource payloads, cache state, and captured schema
output for the resource pipeline timings. Each module owns the fixtures for the
matching benchmark module and calls the owning command's production helpers.

Use synthetic identities and credentials-free kubeconfig text. Preserve input
ownership and cache boundaries when exposing a production path to measurement.
