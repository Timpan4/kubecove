# ADR 0009: Workspace-Scoped Kubernetes Request Cancellation

## Status

Accepted.

## Context

KubeCove renders one active workspace at a time, but switching workspaces previously changed frontend state without ending Kubernetes work started by the old workspace. TanStack Query cancellation stopped frontend consumers from awaiting those results, but it could not abort an in-progress Tauri command. The Rust live store could also retain a shared loader future after its original caller stopped awaiting it.

An unavailable cluster could therefore leave HTTP requests, Tauri commands, shared loaders, and loading state alive while the replacement workspace tried to load. Old work could delay recovery or complete after the active context had changed.

Long-lived watches, logs, port-forwards, and Pod exec sessions have different lifecycles from finite list, get, discovery, topology, metrics, and integration reads. Port-forward and Pod exec sessions must continue to honor the user setting that controls whether live sessions survive a workspace switch.

## Decision

Workspace and selected-context changes use one ordered cancellation boundary owned by the application root:

1. Suspend and unmount the current workspace UI, then flush Svelte cleanup so view-owned watches and log streams stop.
2. Cancel finite Kubernetes TanStack queries.
3. Invoke the typed `cancel_workspace_requests` Tauri command.
4. Apply only the newest requested workspace or context destination.
5. Mount the replacement workspace and allow it to load.

Rapid transitions are coalesced while cancellation is in progress. A transition from A to B to C applies C; an older transition cannot rotate clients after C starts loading.

Finite Kubernetes clients belong to a rotatable generation. Every generation has one cancellation token shared by its cached client clones. Rotating the generation clears the finite client cache, installs a fresh generation, and cancels the previous token. Client construction captures its generation and may enter the cache only if that generation is still current.

Each finite client request races its transport future against the generation token. Finite clients also use a 30-second read timeout as a fallback for stalled endpoints. Workspace switching does not wait for this timeout because generation cancellation is immediate.

The backend cancellation command also cancels registered finite command guards and removes loading entries retained by the shared live store. A prior ready value is restored as dirty; completed ready values remain cached. Existing load identifiers prevent a stale loader completion from replacing a newer result.

Long-lived operations use a separate, non-rotating client cache:

- Port-forward and Pod exec sessions remain governed by their explicit session lifecycle and `keepLiveSessionsOnWorkspaceSwitch`.
- View-owned watches and log streams stop when their owning Svelte view unmounts.
- Stream and live-session clients are not cancelled by finite request generation rotation.

Intentional request cancellation is represented as `AppError` kind `cancelled`. It is lifecycle control, not a Kubernetes authorization or availability failure. Real API status, permission, transport, and timeout errors retain their existing classifications.

The cancellation result exposes only diagnostic counts and the new generation identifier. It does not expose kubeconfig contents, credentials, or Kubernetes resource data.

## Consequences

A healthy workspace can begin loading promptly after an unreachable workspace is left, without waiting for old finite Kubernetes work. Old shared loaders cannot repopulate the active cache, and rapid transitions have latest-destination semantics.

Finite and long-lived Kubernetes access now use deliberately separate client paths. New finite command families must use the rotating client path and be included in the frontend finite-query predicate. New live streams or explicit sessions must use the live client path and define their own cleanup policy.

The 30-second timeout bounds finite reads even when cancellation is not triggered. It does not replace explicit workspace cancellation and does not apply to long-lived streams or sessions.
