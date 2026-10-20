# KubeCove threat model

## Product and assets

KubeCove is a Kubernetes workspace with a Tauri desktop app and an authenticated headless HTTP/WebSocket server. Both hosts use shared Rust command handlers. The Svelte frontend is untrusted relative to Rust. The [product and architecture](../docs/product-and-architecture.md) and accepted [ADRs](../docs/decisions/) define intended behavior; proposed ADR sections are not implemented guarantees.

Protect Kubernetes and connected Argo CD credentials, server authentication and session credentials, cluster operation integrity, and isolation of session-owned streams, exec sessions, tunnels and port forwards. Explicit Secret inspection is supported, but raw kubeconfig, private keys, authentication tokens and certificate data must not become general frontend responses or logs.

## Adversaries and entry points

- Unauthenticated network clients and hostile web origins reaching the headless server, including through loopback and DNS rebinding.
- An authenticated client forging command arguments, resource identity, review handles, session identifiers, channel IDs or tunnel frames.
- Hostile Kubernetes objects, annotations, CRDs, Helm release payloads, events, logs and terminal output rendered or decoded by KubeCove.
- Malicious or compromised Connected Argo CD responses, endpoints and tunnel targets.
- Another unprivileged OS user attempting to read or replace protected credential files.

Prioritize these areas:

1. `src-tauri/server/src/`: token, Cloudflare Access and OIDC authentication; Host and Origin checks; cookies, logout, expiry and refresh; command dispatch and WebSocket admission; session ownership and cleanup. With OIDC Kubernetes credential forwarding enabled, a user's requests must not fall back to the operator's credentials.
2. `src-tauri/crates/` and `src-tauri/src/`: frontend command boundaries and secret redaction; bounded YAML/base64/gzip and API payload processing; connected Argo TLS, destinations and credential use; local credential files; desktop remote adapters; cancellation and retained sessions.
3. Guarded Kubernetes and Argo operations: exact target scope, permission checks, dry-run and confirmation requirements, UID/resourceVersion preconditions, and single-use transport-bound Argo operation reviews. Inspect current contracts in ADRs 0003, 0004, 0005, 0006, 0013, 0014, 0017, 0021 and 0024.
4. `src/` and `src-tauri/capabilities/`: cluster-data injection into Svelte, YAML and terminal surfaces; navigation or link handling; unintended command or filesystem capability exposure.

The frontend must not acquire arbitrary host shell execution or unrestricted filesystem access. Typed Pod exec intentionally runs a command in the selected Pod; this must not become execution on the KubeCove host. Invoking kubeconform for YAML validation is also intentional and must retain its constrained arguments and input handling.

## Trust assumptions and report boundaries

The operator controls kubeconfig sources, their credentials and configured exec plugins. A report that requires an operator to install an attacker-supplied kubeconfig or executable is not itself a trust-boundary bypass. Client sessions must not gain authority to change that operator configuration. Kubernetes RBAC and the API server are external authorities; assess how KubeCove uses them rather than reporting intended API access as privilege escalation.

Server sign-in authorizes use of KubeCove, and sessions use the server's configured Kubernetes credentials by default. Shared kubeconfig sources and access to the same cluster resources are intentional. Report cross-session access to session-owned streams, credentials, exec sessions, forwards or tunnels with evidence of the ownership boundary crossed. OIDC credential forwarding is a separate mode with per-user Kubernetes authorization.

The encrypted file credential store protects against other OS users, not root, processes running as the same OS user, or backups containing both encrypted data and its key. That limitation does not exclude file ownership, permissions, symlink or path-handling defects exploitable by another OS user.

The `e2e` Cargo feature and development browser mocks are not production interfaces. Scripts, fixtures and docs support reproduction; findings confined to those tools need a demonstrated production or maintainer security impact. Third-party library and bundled kubeconform vulnerabilities belong to their upstream projects unless a KubeCove integration or local patch creates the defect. Proposed persistent per-user server workspaces are not a shipped security boundary.

## Exercise the code offline

The image starts in `/src`. It retains `node_modules`, Cargo registries, desktop `src-tauri/target/` and server `src-tauri/server/target/` artifacts. `CARGO_NET_OFFLINE=true` makes missing dependencies explicit.

```sh
bun test
cargo test --locked --workspace --manifest-path src-tauri/Cargo.toml
cargo test --locked --manifest-path src-tauri/server/Cargo.toml
```

The server tests include local HTTP/WebSocket and OIDC fixtures. Network isolation still permits loopback. Extend these fixtures or create synthetic Kubernetes/Argo responses to reproduce defects without a real cluster. The debug server binary is `src-tauri/server/target/debug/kubecove-server`; `--help` documents startup flags. Use only temporary data directories and synthetic credentials. Desktop code is compiled, but this environment does not establish GUI or other operating-system behavior.

Do not run the Kind lab, deploy Kubernetes resources, read a host kubeconfig, contact a real Argo instance, or mount credentials. Live-cluster and native GUI E2E need the repository's separate isolated workflows. kubeconform's remote schema downloads are unavailable offline; supply local synthetic schemas when needed. Dependency auditing and release/update downloads also need a network and are not offline checks.

## Assess and report findings

Rate severity from demonstrated attacker access, affected assets, required configuration and the authority gained. Prioritize unauthenticated host execution or credential theft, authentication bypass, access to another session's private state, hostile cluster-data execution, and bypass of guarded operation scope. Distinguish those outcomes from denial of service or intended actions available to an already authorized operator. Do not infer cluster takeover from a crash or assign severity solely from a vulnerability class.

Each report should identify the commit, affected host/mode, attacker prerequisites, exact path across the trust boundary, expected and observed behavior, and a repeatable local proof of concept using synthetic data. Include a minimal patch and regression test where possible. Group duplicates by root cause, distinguish hypotheses from reproduced findings, and redact secrets from all evidence. Send reports privately to the configured maintainer contact in [SECURITY.md](../SECURITY.md).
