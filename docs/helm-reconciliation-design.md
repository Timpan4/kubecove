# Helm Reconciliation Design

## Goal

Helm Reconciliation compares decoded Helm release intent with live cluster state.
It helps operators answer which release resources are present, missing, unlabeled,
label-only, or unverifiable without adding Helm CLI, mutation, or shell behavior.

## Behavior

- Entry point: Helm release detail panel.
- Expected set: decoded Helm manifest resource references.
- Live evidence: Kubernetes API lookups for manifest references plus a bounded
  conservative scan for explicit `helm.sh/release=<release>` labels.
- Statuses:
  - `tracked`: in manifest, live object exists, explicit Helm release label matches.
  - `unlabeledLive`: in manifest and live, but explicit Helm release label is missing or different.
  - `missing`: in manifest, not found in the live cluster.
  - `labelOnly`: live object has explicit Helm release label but is absent from the decoded manifest.
  - `unavailable`: verification failed because discovery, RBAC, or API access did not allow a clear result.
- Namespace default: namespaced manifest resources without `metadata.namespace`
  use the release namespace after discovery confirms the kind is namespaced.
  Cluster-scoped resources keep no namespace.

## Safety Contract

- Kubernetes access stays behind Rust-side typed Tauri commands and `kube-rs`.
- The frontend receives frontend-safe summaries only.
- Raw Helm storage payloads, kubeconfig contents, tokens, certificates, and shell
  execution do not cross into the frontend.
- This does not implement Helm diff, install, upgrade, uninstall, rollback,
  sync, values diff, or manifest apply.

## Limits

Label-only scanning is intentionally conservative. It scans manifest kinds and
common namespaced resource kinds for the explicit Helm release label. It does not
scan every discovered CRD because that would be expensive and surprising under
limited RBAC.
