# Product and architecture

KubeCove is a local Kubernetes desktop GUI for cluster inspection, GitOps context, and guarded troubleshooting workflows. This document records KubeCove's current product principles, system boundary, and module shape. User procedures belong in the [KubeCove Wiki](https://github.com/Timpan4/kubecove/wiki). Decisions with durable architectural consequences belong in [ADRs](decisions/).

## Product identity

KubeCove is a local desktop Kubernetes workspace for operators and application developers. It keeps cluster context and namespace scope visible while moving from broad health signals to resource evidence.

The product is inspection-first. Its governed exceptions are:

- Pod and selector-backed Service port forwarding;
- exact-Pod exec;
- selected-resource YAML apply;
- narrow scale, rollout-restart, and delete actions;
- explicit connected Argo CD refresh and sync operations.

These are typed workflows with visible targets. KubeCove is not a free-form mutation console or a wrapper around arbitrary shell commands.

## Product principles

### Keep scope explicit

A workspace stores local navigation scope: contexts, cluster-group members, namespaces, kinds, shortcuts, saved Service port-forward presets, and layout preferences. It does not copy kubeconfig credentials or persist selected Kubernetes objects as authoritative state.

```text
Cluster group -> Cluster/context -> Namespace -> App/owner -> Resource
```

Restoring a workspace fetches live state for its available scope. Missing saved contexts, namespaces, or kinds remain visible so the operator can correct the scope rather than silently inspecting something else.

### Start broad, preserve the path to evidence

The main workflow has three complementary surfaces:

- tables answer what exists and which objects need attention;
- details answer what evidence is available for one exact object;
- topology answers how loaded objects are connected by ownership or network relationships.

Health labels, incident signals, topology, and metrics are navigation evidence. They are not root-cause conclusions. Empty and partial states must remain explicit.

### Keep Kubernetes useful without integrations

The resource browser works without Argo CD, Flux, or Helm. GitOps and package information enrich Kubernetes inspection rather than replacing it.

- Argo CD and Flux discovery starts from installed Kubernetes Custom Resource Definitions.
- Flux remains Kubernetes-API-first and inspection-only.
- Helm reconciliation compares decoded release manifest references with live Kubernetes evidence and remains inspection-only.
- Connected Argo CD is an explicit HTTPS transport for bounded inspection and reviewed operations; it never becomes an automatic fallback.

### Guard every operation

An operation must expose one exact target and expected effect before execution. It should be narrow, permission-aware, reversible where practical, and separated from browsing.

New operation families require an ADR. Existing contracts are defined by:

- [ADR 0003](decisions/0003-guarded-live-sessions.md) for live sessions;
- [ADR 0004](decisions/0004-guarded-cluster-operations.md) for scale, restart, and delete;
- [ADR 0005](decisions/0005-guarded-pod-exec-sessions.md) for Pod exec;
- [ADR 0006](decisions/0006-guarded-selected-resource-yaml-apply.md) for YAML apply;
- [ADR 0013](decisions/0013-argocd-connected-inspection-and-operations.md) for connected Argo CD.

## Trust boundary

The Svelte frontend is untrusted relative to the Rust backend.

- Raw kubeconfig contents, tokens, client keys, and certificate data stay backend-side.
- The frontend cannot run arbitrary shell commands.
- Kubernetes access crosses typed Tauri commands and normally uses `kube-rs`.
- Connected Argo CD credentials, TLS handling, request limits, Secret redaction, preflight tokens, and execution stay backend-side.
- Frontend responses are bounded, serialized summaries, details, YAML, stream events, results, or typed errors.
- Secret disclosure is explicit and transient. Connected Argo Secret payloads are redacted before crossing the boundary.

`kubectl`, Helm CLI, Argo CD CLI, and Flux CLI are not core data paths. A new CLI-backed integration or broader filesystem boundary requires its own security review and ADR.

## Frontend shape

The Svelte application shell lives under `src/app/svelte/`. Feature code owns domain surfaces; shared components and pure logic remain separate.

```text
src/
  app/svelte/            app shell, surfaces, settings, shared frames
  components/            shared UI primitives and display components
  features/
    app-updates/
    argo/
    command-palette/
    gitops/
    helm/
    incidents/
    live-sessions/
    rbac/
    resource-detail/
    resources/
    workspaces/
  lib/                   stores, types, pure logic, typed Tauri wrappers
```

Frontend rules:

- components use typed wrappers in `src/lib/tauri.ts`, not raw `invoke()` calls;
- feature-specific UI stays in `src/features/<area>/`;
- reusable primitives stay in `src/components/`;
- selected context, namespace scope, kind scope, workspace, resource, detail view, and operation targets remain explicit state;
- credentials and broad raw Kubernetes objects do not become general frontend state.

Detailed placement and hygiene rules live in the [engineering handbook](handbook/).

## Backend shape

The Rust backend separates Tauri command handlers from serialized frontend contracts.

```text
src-tauri/src/
  commands/              Kubernetes, GitOps, Helm, RBAC, session, and support handlers
  models/                serde contracts returned across the Tauri boundary
```

Backend rules:

- `commands/` owns handlers and command-domain helpers;
- `models/` owns structured frontend contracts;
- commands return frontend-safe data and user-visible serialized errors;
- normal resource access uses typed or dynamic Kubernetes APIs, not shell commands;
- cancellation, stream/session cleanup, response bounds, and redaction are part of each applicable command contract;
- a separate shared Kubernetes service layer is justified only when commands are no longer the sole backend consumer.

Every structured command needs a Rust model, matching TypeScript type, typed wrapper, bounded error behavior, and review for secret or filesystem leakage.

## Resource and integration strategy

Common Kubernetes kinds remain typed or semi-typed when that produces clearer summaries. Custom resources use discovery and dynamic objects. Detail and YAML requests are explicit inspection paths rather than a reason to retain every raw object in frontend state.

Priority evidence surfaces include metadata, owner references, conditions, events, logs, metrics when available, deployment revisions, topology, GitOps status, Helm reconciliation, and RBAC provenance.

Argo CD, Flux, and Helm keep distinct capability boundaries:

- Argo Kubernetes transport reads CRDs and can submit only the reviewed Application operations supported by that transport.
- Connected Argo transport provides bounded API-backed inspection, comparison, refresh, and sync after explicit connection and preflight.
- Flux reads installed provider CRDs and does not mutate them.
- Helm reads release and live-object evidence and does not install, upgrade, rollback, or uninstall.

There is no automatic transport fallback and no inferred permission verdict. RBAC inventory is observed policy evidence; exact authorization uses an explicit live access review.

## Visual and interaction direction

KubeCove uses balanced IDE density: compact enough for operational tables, calm enough for incident work. Context and target identity must stay visible. Tables prioritize scanning, details use structured sections, and topology must clarify relationships rather than decorate the screen.

Future changes should preserve explicit workspace defaults. Adaptive behavior must not silently hide scope, choose credentials, or broaden an operation target.

## Direction and tracking

Future work is tracked through GitHub Issues and GitHub Milestones rather than checklists in documentation. Candidate areas require evidence and prioritization there; this document does not promise delivery dates.

Security-sensitive areas such as broader exec targets, Git-writing, Flux or Helm mutations, provider CLI integration, or AI-assisted operations require a fresh ADR before implementation.

Exact dependency and application versions live in `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, and release metadata. Kubernetes support policy lives in [ADR 0011](decisions/0011-rolling-kubernetes-support.md) and the release workflows, not duplicated version literals here.
