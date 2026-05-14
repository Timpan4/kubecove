# `src-tauri/src/models/`

Serde contracts shared with the frontend. Frontend mirrors live in `src/lib/types.ts`.

While the module fits in one `mod.rs`, leave it. When it grows past the soft cap (500 lines), split per domain: `cluster.rs`, `namespace.rs`, `resource.rs`, `argo.rs`, `error.rs`. See [docs/handbook/code-organization.md](../../../docs/handbook/code-organization.md).

Models never embed raw Kubernetes objects. They are the trimmed, frontend-safe view. Anything that could leak kubeconfig contents, tokens, or certificate data does not belong here.
