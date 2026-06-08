# ADR 0006: Guarded Selected-Resource YAML Apply

## Status

Accepted.

## Context

KubeCove needs a way to correct a currently inspected Kubernetes resource without turning the app into a generic manifest runner. ADR 0004 allows cluster-changing workflows when they stay typed, scoped, previewed, and Rust-owned.

The YAML inspector also needs two different read shapes:

- `kubectl` for familiar inspection output with server-side `metadata.managedFields` removed
- `applyClean` for edit/apply preparation with server-owned metadata and root `status` removed

Secret YAML is redacted in the UI. Applying redacted Secret values could destroy live data.

## Decision

KubeCove supports YAML apply only from the selected resource YAML panel.

The apply flow must:

- initialize the editor from `applyClean` YAML
- reject multi-document YAML
- validate `apiVersion`, `kind`, `metadata.name`, and `metadata.namespace` against the selected resource
- disable v1 Secret apply because displayed data is redacted
- run server-side apply dry-run with `fieldManager=kubecove`
- show a line diff between current `applyClean` YAML and the dry-run result
- require a second explicit Apply action after dry-run succeeds
- perform server-side apply with `force-conflicts` only when the global YAML
  force-conflicts setting or current selected-resource override allows it
- invalidate the selected resource details and YAML after success

The Rust side owns validation, dry-run, apply, and Kubernetes API access. React may edit YAML text and request the typed operation, but it does not receive kubeconfig material and does not run shell commands.

## Consequences

Apply v1 is deliberately narrow: no bulk manifest paste, no multi-document
apply, no namespace retargeting, and no arbitrary cluster apply.

Field ownership conflicts, RBAC denials, admission failures, and validation
failures are surfaced as user-visible command errors. If force-conflicts is
disabled globally, a field ownership conflict may be retried only through an
explicit current-resource override after the failed dry-run. Future broad apply,
Secret editing, Argo CD sync, Helm actions, delete, or scale workflows need
their own focused design under ADR 0004.
