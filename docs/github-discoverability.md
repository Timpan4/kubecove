# GitHub discoverability

This is a maintainer checklist for KubeCove's GitHub landing surface. Repository metadata and the social-preview image are GitHub settings, so they cannot be applied through a normal repository file change.

## Repository description

In the repository's **About** settings, use:

> Fast local Kubernetes desktop GUI for inspecting clusters, workloads, Argo CD, Flux, and Helm with guarded operations.

This describes the current product without implying that KubeCove is a hosted service or a replacement for every Kubernetes tool.

## Topics

Use a focused set of accurate discovery terms:

- `kubernetes`
- `kubernetes-ui`
- `kubernetes-gui`
- `kubernetes-dashboard`
- `kubernetes-desktop`
- `cluster-management`
- `kubernetes-operations`
- `kubernetes-troubleshooting`
- `gitops`
- `argocd`
- `fluxcd`
- `helm`
- `devops`
- `kubeconfig`
- `tauri`
- `svelte`
- `rust`

Remove stale or inaccurate topics that are not supported by the current codebase. In particular, remove `react`: KubeCove's frontend is Svelte. The current repository topic list also contains generic or project-name topics; those can be omitted in favor of the focused set above.

## Social preview

In the repository's **Social preview** settings, create and upload a high-contrast image that combines:

- the product logo from [`public/kubecove-logo.svg`](../public/kubecove-logo.svg);
- a real product screenshot such as [`docs/assets/workspace-overview.png`](assets/workspace-overview.png); and
- this concise text:

```text
KubeCove
Fast Kubernetes Desktop UI
Kubernetes · Argo CD · Flux · Helm
```

Keep the screenshot and text readable at the preview size. This repository does not have a source-file workflow for social-preview binaries, so this change intentionally does not add a generated image. The maintainer should compose and upload the preview manually in GitHub.

## Maintenance

When the product's supported integrations or operation boundaries change, update the README and Wiki first, then review this description and topic list. Keep terms such as Argo CD, Flux, Helm, kubeconfig, Kubernetes troubleshooting, and guarded operations aligned with implemented behavior rather than roadmap items.
