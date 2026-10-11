# ADR 0023: Multiple user sessions and persistent user state

## Status

Proposed 2026-10-11. Planning only; no runtime changes are implemented by this record.

The product owner confirmed multiple user sessions per backend, concurrent tabs and devices for the same user, and support for both shared and per-user Kubernetes credentials. Concurrent session isolation and opt-in OIDC credential forwarding are implemented. Both operator-assigned private sources and user-provided credentials remain part of the broader multi-user plan. Their provisioning, durable storage and the policies identified below remain proposed.

Extends the concurrent-session baseline in [ADR 0020](0020-headless-server-mode.md) and forwarded credentials in [ADR 0022](0022-oidc-server-sign-in.md). Before implementing the remaining proposal, accept its ownership and persistence boundary and amend the affected contracts in ADRs [0003](0003-guarded-live-sessions.md), [0009](0009-workspace-request-cancellation.md), [0013](0013-argocd-connected-inspection-and-operations.md), [0019](0019-reviewed-rbac-risk-context.md), [0021](0021-file-credential-store-without-keyring.md) and [0022](0022-oidc-server-sign-in.md) where needed.

## Outcome

One backend serves independent users without a new login displacing another client. The same user can work in concurrent tabs or devices. Saved workspaces and user settings survive server restarts and are available after that user signs in from another client. Each client retains independent navigation and live-session ownership.

The deployment can use shared server Kubernetes credentials, per-user credentials, or both according to operator policy. Application sign-in and Kubernetes authorization remain separate. A private workspace is private application data, not an independent Kubernetes permission boundary when its owner uses shared credentials.

## Current implementation

The merged session implementation provides the baseline below. It does not implement private kubeconfig provisioning or durable per-user workspace/settings storage:

- [`Sessions`](../../src-tauri/server/src/sessions.rs) keeps a registry of independent owners. Fresh authentication opens another session; a resume reattaches only its matching identity-bound session.
- [`BackendState`](../../src-tauri/server/src/backend.rs) groups live cache, cancellation, streams, Pod exec, port-forwards, usage and Argo connection state per session. Channels, tunnels, socket generation and reconnect expiry also belong to that session. [`SessionScope`](../../src-tauri/src/commands/helpers/session_scope.rs) isolates Kubernetes client caches, bearer slots and diagnostics.
- [`workspaceStore.ts`](../../src/features/workspaces/workspaceStore.ts) writes saved workspaces to `kubecove-workspaces` in client `localStorage`. [`settings.ts`](../../src/lib/settings.ts) persists UI preferences and Argo profile metadata to `kubecove-settings`. Neither key identifies a server or authenticated user. [`path-state.ts`](../../src/lib/path-state.ts) uses a URL hash and `sessionStorage` for navigation.
- [`kubeconfig.rs`](../../src-tauri/src/commands/kubeconfig.rs) keeps `kubeconfig-sources.json` as shared operator configuration. Server sessions cannot change its source registrations or labels in any authentication mode. Private-source assignment and self-service provisioning remain unimplemented.
- [`credential_store.rs`](../../src-tauri/src/commands/argo/credential_store.rs) shares one secure-store adapter, but namespaces remembered credentials and persisted operation reviews by the session's verified identity in OIDC and Access modes. Argo profile metadata remains client-persisted; it is not a durable per-user server profile.
- OIDC verifies issuer and subject, and Access verifies its configured issuer and subject. Resume credentials are bound to the verified identity. With `--oidc-kubernetes-credentials <AUD>`, each session forwards its user's verified ID token instead of kubeconfig credentials, with in-memory refresh and expiry handling. Without that opt-in, Kubernetes uses shared server credentials. Startup-token mode has no distinct authenticated user identities. Native desktop remote sign-in supports token mode only.

## Proposed boundaries

### Users, authentication and client sessions

Use a stable server-side user ID mapped from the verified authentication mode, issuer and subject. Never use email, a display name, a bearer token or a resume token as the durable user key. Do not merge users across providers without an explicit account-linking decision.

Preserve the implemented registry of client sessions. Each session already has its own opaque session/resume credential, socket generation, backend state, channels, tunnel and port-forward registries, and reconnect timer. A resume reattaches that exact session; a fresh login creates another without transferring resources. The remaining proposal associates persisted user records and private source grants with these authenticated sessions.

Resolve user and session from authenticated request context before dispatch. A request body may select an owned workspace or allowed credential source, but cannot select another user or override ownership. Validate HTTP invokes, WebSocket control and binary frames, stream subscriptions, exec input/resize/stop, forwards, cancellation and persisted-data access against that context.

Reconnect grace continues to apply independently to each session, using the existing configured policy. Ending or expiring one session cleans up only its resources. OIDC logout revokes the current sign-in, clears its bound bearer slots, and closes the session identified by a matching resume token. Other sockets using that credential close and follow the disconnect grace policy. Browser tabs sharing the same cookie share its revocation; a separate sign-in remains valid, including one for the same subject. Preserve this contract when adding persistence. Closing a workspace or switching scope cancels only that client's finite reads under ADR 0009. Confirmed mutations retain their existing non-cancellable execution semantics and visible outcomes.

Proposed token-mode compatibility: treat the shared startup token as one stable local application user, with concurrent sessions and a durable profile independent of the token value. Token holders share that user's data and cannot claim separate private profiles. OIDC or Access supplies distinct users. Keep native desktop token support; native OIDC/Access sign-in needs a separate client-flow decision before those clients can access identity-backed profiles.

### Kubernetes credentials and operator policy

The server already supports shared credentials and opt-in per-user OIDC forwarding. The remaining proposal adds explicit source assignment and selection:

| Model | Credential owner | Workspace behavior |
| --- | --- | --- |
| Shared | Server operator | Uses an operator-enabled shared source; Kubernetes RBAC evaluates that source's credentials. |
| Forwarded OIDC ID token, implemented | Signed-in user | With `--oidc-kubernetes-credentials <AUD>`, uses the user's verified ID token and Kubernetes RBAC. No kubeconfig upload or durable credential storage is required; refresh stays in server memory. |
| Private source, proposed | One application user | Uses a source assigned to that user; other users cannot select it or receive its summaries or credentials. |

Proposed operator policy can enable shared sources, per-user sources, or both. Each workspace records its selected source scope and stable source ID. If both are available, creation and any later switch show the credential model and target context explicitly. Changing a source invalidates affected reads, connections and operation reviews using the existing scope-change rules. A missing, expired, revoked or forbidden private source never silently falls back to shared credentials, including service-account or default kubeconfig discovery.

Resolve authorized sources in Rust before constructing a Kubernetes client. Include credential scope in client/cache keys, discovery, live-store lookups, Argo profile eligibility, operation reviews and RBAC review fingerprints. Identically named contexts or workspaces from different users or credential sources must not collide. Frontend-supplied paths or environment-variable selectors cannot bypass the allowed source set.

Keep listener, TLS, authentication configuration, data directory and shared kubeconfig registration operator-owned. User settings cannot change these global values. Separate presentation preferences such as source labels from source registration. The remaining provisioning scope includes both operator assignment and self-service credentials. Operator policy determines which methods a deployment permits. These paths are not prerequisites for the already implemented concurrent-session baseline.

### Credential provisioning

Operator assignment registers a server-managed source and grants it as shared access or private access for a specific user. Assignment and revocation are operator actions; knowing a source ID or server path never grants access. Existing host kubeconfig and mounted service-account sources remain supported through this path.

Self-service lets an authenticated user import, replace and remove their own Kubernetes credential source. The proposed browser flow uploads a user-selected file directly to a dedicated authenticated Rust endpoint, without parsing its raw contents into frontend application state. The desktop flow reads a selected file in desktop Rust and sends it through the typed remote path. Neither flow returns kubeconfig contents, tokens, private keys or certificate data to the frontend. Show a sanitized context/endpoint preview before committing the private source, and make its owner and credential model visible in workspace selection.

Parse and validate in Rust before storing or constructing a client. Enforce the existing transport body limits and define any additional format bounds from the supported credential contract before implementation. Resolve kubeconfig references deliberately: uploaded server-side file paths cannot read arbitrary host files, and uploaded environment settings cannot choose another user's credentials. Support self-contained token and certificate credentials; report missing external material or unsupported authentication explicitly rather than silently falling back. Store private credential material in user/source-namespaced secure storage, extending ADR 0021 where needed, with only sanitized source metadata in preference records. Replacement and removal invalidate all affected clients, caches, connections and reviews for that source while leaving unrelated sources and sessions intact.

Proposed exec-plugin policy: reject uploaded executable commands, arguments and environment overrides by default. If a deployment needs exec-based authentication, the operator supplies a trusted helper definition with its executable, invocation and allowed inputs. An uploaded source can reference only an authorized helper definition; it cannot supply arbitrary server commands or filesystem paths. Confirm the helper contract and supported authentication forms before implementation. This validation belongs to the proposed user-provided credential path.

### Persistent workspaces and settings

The server becomes authoritative for remote user data. Use versioned, typed records keyed by server-side user ID, with a revision checked on writes. Reuse the current workspace and persisted-settings schemas where compatible; validate persisted and imported fields in Rust and do not serialize store functions or runtime state. New frontend persistence and source commands need serde models, TypeScript types and wrappers in `src/lib/tauri.ts`, with typed application errors across both transports.

| Data | Owner and persistence |
| --- | --- |
| Saved workspace scope, layout, pinned/recent entries and port-forward presets | User-owned server records. Preserve existing workspace IDs within that user. |
| UI settings, Argo profile metadata and workspace connection preferences | User-owned server records. Keep safety defaults and explicit opt-ins. |
| RBAC review notes and fingerprints | Private user/workspace/source records under an amendment to ADR 0019; excluded from sharing exports. |
| Remembered Argo credentials and operation reviews | Preserve implemented identity-namespaced secure storage under ADRs 0013 and 0021. Extend review binding to private source scope while retaining reviewed-target binding, expiry, restart revalidation and single-use behavior. |
| Per-user Kubernetes credentials | Rust-only secure storage and both provisioning paths above, separate from general preference JSON and workspace exports. |
| Active navigation, selection, query results and revealed Secrets | Client-session state. No durable cluster snapshots or Secret values. |
| Running streams, logs, exec, tunnels and port forwards | Session memory only. Resume within grace; do not restore running handles after a server restart. Saved presets remain separate. |
| Listener, authentication and shared source configuration | Operator-owned server configuration. |

A write acknowledges success only after durable commit. Reject a stale revision with a typed conflict containing the current safe record/revision; keep the client's unsaved edit available for explicit reload or reapply. Do not silently overwrite another tab's changes. Treat deletions as revisioned writes too. Notify other sessions of changed revisions so their saved-state views can refresh without moving their active navigation or live sessions. Network and storage errors remain visible; do not replace failed reads with an empty profile or silently save only to client storage.

Proposed initial storage is versioned per-user JSON under the private data directory, using a per-user write lock, revision comparison and atomic durable replacement. Use server-generated storage IDs rather than subjects or client IDs as filesystem paths. Keep one server process writing that directory, including the current single-replica deployment. A database and multi-replica storage remain unselected; introduce them only if measured requirements exceed this storage contract. Preserve a readable existing record on failed validation, interrupted writes or unsupported schema upgrades. Define platform-specific durability and permissions before accepting this storage mechanism.

Remote browser and desktop stores load only after authentication and server/user identification. Key any client cache by both server and user, and clear prior-user memory on logout or account/server changes before mounting another workspace. Keep local desktop persistence local. A remote connection failure cannot hydrate a different server's or local desktop's workspace as a fallback.

### Migration

Existing browser storage has no trustworthy user owner. Offer explicit, previewed import into the signed-in user's server profile; never assign it to the first person who logs in or upload it automatically. Reuse workspace import validation and conflict choices. Retain the original client data until the server confirms the write and the user chooses to remove it.

Existing server kubeconfig sources remain shared operator configuration unless explicitly assigned otherwise. Current OIDC/Access Argo credentials and operation reviews already use verified-identity namespaces. Legacy unnamespaced entries are not read through those namespaces and require credentials to be entered again. Any later migration of legacy entries or remapping to durable user IDs must require explicit ownership and invalidate unowned reviews. Do not expose them to every signed-in user. Preserve originals during migration and version the new storage so an older server cannot silently overwrite it.

## Delivery sequence and proof

This sequence is a design proposal, not a delivery-status checklist. Create or update GitHub Issues after the open policies are settled; keep ownership, release targets and progress there rather than in this ADR.

1. Accept identity, credential provisioning, operator policy, migration and persistence contracts. Inventory every dispatched command and stateful registry as operator, user or client-session scope, including indirect credential-store access. This inventory must cover the command table, not a sample.
2. Implement private user/source resolution, operator assignment and self-service import/replace/remove, extending the existing identity-namespaced credential and review storage. Preserve the existing ban on client changes to global source settings. Prove two users with identically named contexts cannot cross-read, connect or execute using each other's private credentials or review IDs. Cover malformed uploads, external file references, unapproved exec plugins, secure-store failures, credential replacement and removal.
3. Preserve the implemented concurrent-session lifecycle while adding those source and persistence paths. Prove another login, reconnect, scope switch, logout or grace expiry leaves unrelated sessions active. Cover same-user tabs, different users, guessed IDs and server-bound forward port conflicts. Derive any additional admission or backpressure limits from deployment requirements or measurements; the current session registry is uncapped.
4. Add durable user workspace/settings storage and typed remote wrappers, then integrate the existing Svelte stores and migration UI. Prove restart recovery, another-device loading, stale-write conflicts, deletion conflicts, failed-save handling and user/server cache separation. Verify saved auto-start preferences do not create unowned or duplicate forwards across concurrent clients; settle their concurrent-client behavior before enabling that path.
5. Run browser E2E with independent identities and clients across shared and private source policies and both credential provisioning methods, plus native desktop token-mode regressions. Record repeatable reports, redacted diagnostics and failure screenshots under `e2e/artifacts/`. Verify source revocation, interrupted persistence and server restart with active sessions as well as the happy path. Source or unit inspection alone cannot establish isolation.

For each implementation slice, use the project's frontend/backend checks and the nearest boundary tests before runtime E2E. Keep Host/Origin, TLS, authentication expiry, Secret redaction, guarded operations and cross-platform behavior unchanged unless separately amended. Accept the necessary ADR amendments before implementing their changed boundaries.

## Open owner decisions

1. Credential import policy: confirm supported authentication forms, treatment of external credential material, and the proposed operator-controlled exec-helper contract. Both operator assignment and user-provided credentials remain in the proposed provisioning scope.
2. Operator policy: choose which source models a deployment enables and how sources are assigned/revoked. Recommendation is explicit source grants with no implicit access to private sources or server configuration.
3. Persistence policy: confirm which user preferences should follow devices, any device-specific exclusions, migration ownership, and the proposed file-store durability contract. Navigation remains independent per client.
4. Saved-forward auto-start: define how the existing opt-in behaves across concurrent clients and server restart. Restoring a workspace must not transfer another session's live resources.

No release date, database dependency, session quota or new timeout is selected by this plan.
