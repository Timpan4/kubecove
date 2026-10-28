# ADR 0026: Cluster Access by Intent

## Status

Accepted on 2026-10-22.

## Context

Every Kubernetes command receives a cluster context and an optional `kubeconfigEnvVar` selector, and passes the pair down as loose arguments. Before this decision, 36 call sites built a `KubeconfigSource` from the selector, and 12 calls derived the source key separately through `kubeconfig_source_key`, often in the same command that built a client from the same selector again. About a dozen local wrappers turned the pair into a client, and the GitOps crate imported a live-session internal of the live crate only to get a fail-closed live client.

[ADR 0009](0009-workspace-request-cancellation.md) sets three client paths: rotating finite clients for reads, a non-rotating client for confirmed mutations, and a non-rotating client for long-lived streams and sessions. Cluster-changing paths also must not fall back to the default kubeconfig when configured sources fail to load. Each call site chose its path and its fail-closed rule by hand: 8 `fail_closed()` sites, 3 operation-client sites and 3 live-client sites. The rule had no single home, and no test checked that a confirmed write uses the non-rotating client.

In server mode the selector is a client-supplied host environment variable name. The server reads that variable as a kubeconfig path list and loads the files it names. [ADR 0020](0020-headless-server-mode.md) makes kubeconfig sources operator-owned, and the settings commands refuse changes from server sessions, but the per-request selector was not checked. [ADR 0023](0023-multi-user-sessions-and-persistence.md), still Proposed, requires that frontend-supplied paths or environment-variable selectors cannot bypass the allowed source set.

The frontend never sends an environment variable name as a selector. Its requests carry either no selector or the `sourceKey` returned by `get_kubeconfig_sources`, and its argument helpers remove source keys from most requests. Only the desktop settings form edits the variable name, through `set_kubeconfig_env_var`.

## Decision

### One cluster-access module

`ClusterAccess` in `kubecove-kube` is the only way commands reach Kubernetes credentials. A command resolves it once per request from the calling session's source policy and the request's selector. The resolved access exposes:

- the source key, and the source label when source labels are shown;
- the contexts of the resolved source;
- Kubernetes clients named by intent.

The intent decides the ADR 0009 client path and whether the request fails closed:

| Intent | Used for | Client path | Configured sources fail to load |
| --- | --- | --- | --- |
| Read | Finite list, get, discovery, topology, metrics, RBAC and integration reads | Rotating finite generation with the 30-second read timeout | Fall back to the default kubeconfig |
| Preview | Dry runs, preflights and action state read before a cluster-changing action | Rotating finite generation, as Read | Fail closed |
| Confirmed operation | Confirmed resource actions, YAML apply and Argo CD operations over Kubernetes transport | Non-rotating operation cache with the 30-second read timeout | Fail closed |
| Live session | Port-forwards, Pod exec and Argo CD Service tunnels | Non-rotating live cache | Fail closed |
| Stream | View-owned watches and log streams | Non-rotating live cache | Fall back to the default kubeconfig |

The module hides source resolution and merging, kubeconfig file fingerprints, generation rotation, the forwarded bearer layer of the [ADR 0022](0022-oidc-server-sign-in.md) amendment, and source-key derivation. Callers no longer build kubeconfig sources, derive source keys or choose cache paths. `SessionScope` and the client caches keep their behavior.

Long-lived work keeps its resolved access and asks it for a client again when it reconnects. Kubeconfig file rotation and renewed forwarded tokens therefore still apply to watches and port-forward connections.

The resolved access reports the key of the source it actually uses. A source-key selector selects the configured source. Some paths previously echoed the request's key unchanged; the two differ only when the request carries a stale key.

### Source policy and the server selector

The session's source policy decides which selectors a request may use:

- **Local**, for the desktop app outside any session scope: a selector may name any environment variable, as before. A source key or no selector selects the configured source. The isolated E2E source still overrides every selector in E2E builds.
- **Operator**, for every `kubecove-server` session: a request may send no selector, the operator's configured environment variable name, or the operator's configured source key. Any other selector fails with an `AppError` of kind `kubeconfig` before the named variable or any file it names is read. The check reads only the operator's source settings.

The same policy now decides that server sessions cannot change kubeconfig source settings, instead of the settings code checking whether any session scope is active. Forwarded-credential mode keeps its existing refusal.

Test harnesses that passed arbitrary environment variable names to the server configure their fixture source as an operator would.

### Seam for per-user sources

The source policy is the seam where ADR 0023's per-user sources would go. Today it has the local and operator adapters and a test adapter that hands every intent the same mock client and records the intents requested. A per-user adapter would resolve a selector to a source granted to the session's user and reject the rest. Client intents, cache paths and the source key interface would stay as they are.

This decision does not implement private sources, source grants, per-user source keys or credential storage. Those remain under ADR 0023.

### Tests

Tests reach Kubernetes through commands or through `ClusterAccess` with `tower_test` mock clients. The test adapter installs a session whose requests resolve to the mock client. Command tests replace the functions that existed only to inject a mock client.

Command tests assert that resource action execution and YAML apply request the confirmed-operation intent, and the existing Argo CD Kubernetes-transport test keeps proving that an operation survives rotation. A cluster-access test asserts that generation rotation cancels read and preview clients but not confirmed-operation, live-session or stream clients. Server tests assert that an arbitrary environment variable selector is rejected without being read, that the operator's configured selector and source key are accepted, and desktop tests assert that a local environment variable selector is still accepted.

## Consequences

Each command family picks an intent instead of a client path, so new reads, previews, confirmed writes and sessions follow ADR 0009 and the fail-closed rule from one place.

The wire contract is unchanged: command names and camelCase argument keys stay the same, including `kubeconfigEnvVar`. Desktop selector behavior is unchanged.

Server clients that sent their own environment variable names now receive a `kubeconfig` error. The bundled frontend is unaffected because it sends no such selector.
