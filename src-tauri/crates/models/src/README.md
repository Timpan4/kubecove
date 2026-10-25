# `src-tauri/crates/models/src/`

Serde contracts shared with the frontend live here. Frontend mirrors live in `src/lib/types.ts` and the domain type files it re-exports, such as `src/lib/gitops-types.ts` for Argo CD, Flux and Helm.

Current domains include cluster, namespace, resource, discovery, events, streams, sessions, metrics, health, incidents, diagnostics, cancellation, resource actions, operations, Argo CD, Flux, Helm, RBAC, usage, the command contract, and error models.

Rules:

- Models are trimmed app contracts, not raw Kubernetes object dumps.
- Do not include kubeconfig contents, tokens, certificates, or broad filesystem data.
- Prefer domain files over a growing `lib.rs`.
- Keep field names stable with the TypeScript mirrors.
- Future operation models must include explicit target scope and frontend-safe results.

Caps: `.rs` soft 500 / hard 800. See [docs/handbook/file-size-and-split.md](../../../../docs/handbook/file-size-and-split.md).
