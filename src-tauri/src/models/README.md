# `src-tauri/src/models/`

Serde contracts shared with the frontend live here. Frontend mirrors live in `src/lib/types.ts`.

Current domains include cluster, namespace, resource, discovery, events, streams, metrics, Argo CD, Helm, RBAC, usage, and error models.

Rules:

- Models are trimmed app contracts, not raw Kubernetes object dumps.
- Do not include kubeconfig contents, tokens, certificates, or broad filesystem data.
- Prefer domain files over a growing `mod.rs`.
- Keep field names stable with the TypeScript mirrors.

Caps: `.rs` soft 500 / hard 800. See [docs/handbook/file-size-and-split.md](../../../docs/handbook/file-size-and-split.md).
