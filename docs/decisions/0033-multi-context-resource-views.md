# ADR 0033: Multi-context resource views

## Status

Proposed on 2026-11-09. Accepted by the owner on 2026-11-09.

## Context

A workspace stores a primary context and optional cluster-group members. The
Workspace Overview reads every context and summarizes them together. The
Resources view read only the primary context, so objects in the other contexts
were invisible there and could not be opened, inspected or targeted by guarded
operations from that view.

## Decision

The Resources view reads every context of the workspace and shows their
objects in one table.

Reads:

- Each context is read with the existing per-context typed commands. No
  command accepts several contexts, and the Kubernetes access path, client
  intents and ADR 0009 client paths do not change.
- Contexts are read concurrently. Each context's result is kept separately in
  the client and merged only for display, in workspace context order: the
  primary context, then the cluster-group members.
- A context that fails contributes no rows on its first read, or keeps its
  last loaded rows on a refresh, and shows a warning that names the context.
  The other contexts stay visible. If every context fails, the view shows one
  error that names each context.
- Watches are started per context and per kind. A watch event updates only
  rows of its own context.
- ADR 0009 cancellation applies to every context's finite reads. Leaving the
  workspace or changing its contexts cancels them all.

Identity and display:

- A row's identity is its context, API version, kind, namespace and name.
  Objects with the same name in different contexts are different rows and are
  never merged.
- When the workspace has more than one context, the table shows a Context
  column, a Context filter, and includes the context in search. Ownership
  groups, Helm groups and inherited GitOps ownership never cross contexts.

Targets:

- Opening a row reads details, YAML, events, logs and metrics from that row's
  context.
- Every guarded operation, live session and YAML apply started from a row
  targets that row's context. ADR 0004's visible-target rule applies: the
  detail header shows the context when the workspace has more than one, and
  every review or confirmation step names it.
- No operation spans several contexts.

Scope limits:

- GitOps (Argo CD and Flux), Helm, RBAC, Incidents, Usage and the Cluster
  Overview stay primary-context only. Argo CD connected profiles keep ADR
  0013's single-context binding.
- Ownership map shows one context at a time. A "Map context" select in the map
  panel lists the workspace contexts in workspace order and defaults to the
  primary context. The select appears only when the workspace has more than one
  context. Ownership edges never cross clusters.
- Live sessions may target any workspace context. ADR 0003's scope rule,
  which already keeps sessions whose context is one of the workspace's
  contexts, is unchanged.

## Consequences

Users see and act on every workspace context from one table, with the target
context visible before any change. Read and watch load on the client grows
with the number of contexts; each API server sees the same load as before.
Client code that joins rows by namespace and name alone, such as metrics joins,
watch reconciliation and ownership inheritance, must include the context.
Both clients share acceptance scenarios for the merged view under ADR 0029.
