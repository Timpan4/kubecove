# Run KubeCove on a remote host

`kubecove-server` runs KubeCove on a remote Linux or macOS host that already has kubeconfig files and network access to your clusters. You use it from a browser or from the KubeCove desktop app. The default setup is one OS user on a remote host, reached through an SSH local forward. Operators can also run the same binary as a container in Kubernetes, including k0s; see the [container build and Kubernetes example](https://github.com/Timpan4/kubecove/tree/main/deploy). KubeCove does not install an agent or deploy itself into a cluster.

## Before you start

- **Operator-managed sources.** Kubernetes uses the server's configured credentials by default. With `--oidc-kubernetes-credentials <AUD>`, requests use each signed-in user's verified ID token and Kubernetes RBAC evaluates that user. Client sessions cannot change the shared kubeconfig source settings.
- **Independent sessions.** A new sign-in opens another session without taking over existing clients. Each session owns its live streams, Pod exec sessions and port forwards. Closing or expiring one session leaves the others active. Saved workspaces and UI settings still use client storage; durable per-user server storage remains proposed.
- **One authentication mode per server.** Choose a per-start token, Cloudflare Access or OpenID Connect (OIDC) when starting the server. The modes cannot be combined.

## Choose a client

| Client | How it connects | Supported authentication | Where port forwards listen |
| --- | --- | --- | --- |
| Browser | Opens the web app served by `kubecove-server` | Token, Cloudflare Access, OIDC | Loopback on the server host |
| Desktop app | Settings → Remote server; desktop Rust sends HTTP and WebSocket requests | Token only | Loopback on the laptop, tunnelled through the server |

A browser port forward listens on `127.0.0.1` on the server. To reach it from a laptop, add a separate SSH forward to that server port. A desktop remote forward listens on the laptop and tunnels each connection through the server session.

The desktop app does not perform Cloudflare Access or OIDC sign-in. To use the desktop app against a server, run that server in token mode.

Releases provide full and slim variants of both archives and container images. Full includes the browser app. Slim includes only the server API, supports the desktop remote client in token mode, and serves no browser assets. A separate browser frontend must share the API's exact origin through a reverse proxy. See [Install the server](https://github.com/Timpan4/kubecove/wiki/Server-Installation#choose-full-or-slim).

## Choose an authentication mode

| Mode | Server options | Typical access path | Notes |
| --- | --- | --- | --- |
| Token (default) | none | SSH local forward, `kubectl port-forward` | A new random token on every start. Works with browser and desktop clients. |
| Cloudflare Access | `--cloudflare-access-team`, `--cloudflare-access-aud` | Cloudflare Tunnel with an Access application | Browser only. Requires native TLS and an HTTPS `--public-origin`. |
| OIDC | `--oidc-issuer`, `--oidc-client-id`, `--oidc-login-timeout`, optional `--oidc-client-secret-file` | Any HTTPS route to the server, such as Cloudflare Tunnel | Browser only. Requires native TLS and an HTTPS `--public-origin`. |

Cloudflare Access and OIDC modes create no startup token and ignore an old `server-token` file. `--print-token` cannot be used with them. Loopback listeners still require the selected mode, because other users on a shared host can reach loopback ports.

## Install and prepare the host

1. Install `kubecove-server` from a native archive, a container image or a systemd service. [Install the server](https://github.com/Timpan4/kubecove/wiki/Server-Installation) compares the methods and gives step-by-step instructions. The archive is not a desktop installer and does not self-update. To build from source, see the [server README](https://github.com/Timpan4/kubecove/tree/main/src-tauri/server).
2. Run it as the OS user whose kubeconfig and cluster access KubeCove should use. Kubeconfig discovery uses that user's `KUBECONFIG`, then the standard kubeconfig path. Kubeconfig paths entered in the UI refer to files on the server, not on the laptop.
3. Keep the listener on loopback. The default is `127.0.0.1:0`, where the OS picks a free port. For a predictable SSH target, set a port:

   ```sh
   ./kubecove-server --listen 127.0.0.1:8080
   ```

In token mode, startup prints a login URL that contains a fresh token. Treat that URL as a password. Keep it out of shared chat, tickets, shell history and persistent logs. The server also writes the token to `server-token` in its data directory with mode `0600`. On Unix, the data directory must be private (`0700`) and owned by the running user. Use `--data-dir PATH` to choose another location. Without that option, the server uses the platform config directory.

Run `kubecove-server --help` for the full option list.

## Connect from a browser through SSH

Keep the server on loopback. From the laptop, forward a local port to the server's loopback port:

```sh
ssh -N -L 8080:127.0.0.1:8080 user@remote-host
```

Start the server with the exact origin the browser will use. The server checks Host and Origin against this value, and the printed login URL uses it:

```sh
./kubecove-server --listen 127.0.0.1:8080 --public-origin http://localhost:8080
```

Open the printed login URL while the SSH tunnel is active. You can also open the origin and paste the token into the **Server token** field. The browser removes the token from the address bar after login and uses an HttpOnly, SameSite=Strict cookie. Close the browser session when finished.

If you use a different local port or hostname, put that exact origin in `--public-origin`. The server sends no CORS headers and does not support embedding from other origins.

Direct non-loopback TCP is an advanced alternative. It requires `--allow-remote`, `--tls-cert`, `--tls-key` and an exact HTTPS `--public-origin`. Never disable these checks to make a connection work.

## Connect from the desktop app

1. Start the server in token mode and make it reachable from the laptop through an SSH tunnel or a trusted TLS endpoint.
2. In KubeCove desktop, open Settings → Remote server.
3. Enter the **Server URL** as seen from the laptop, for example `http://localhost:8080`, and the **Server token**.

The token stays in desktop Rust memory for the active connection. It is not saved across app restarts or passed through frontend Kubernetes commands. HTTP is accepted only for loopback hosts; other hosts require HTTPS with a certificate the laptop trusts for that hostname.

The remote desktop session uses the server's kubeconfig and cluster connectivity. Workspace import and export stay on the laptop. **Disconnect** clears the active remote credentials and closes the owner's live sessions and forwards.

## Sign in through Cloudflare Access

The operator starts the server with `--cloudflare-access-team` and `--cloudflare-access-aud`, native TLS and an exact HTTPS public origin. Open the public KubeCove URL and complete Cloudflare Access sign-in. KubeCove opens without a token form. The Access application policy decides who can sign in. KubeCove verifies the signed Access application JWT and does not trust email headers.

Each client opens an independent session. An expired Access session may require reloading the page and signing in again. See the [container deployment guide](https://github.com/Timpan4/kubecove/tree/main/deploy#cloudflare-tunnel-and-access) for tunnel setup and signing-key refresh requirements.

## Sign in through OpenID Connect

Register a web application with your identity provider:

- Redirect URI: exactly `https://<kubecove-host>/api/v1/oidc/callback`.
- Grant type: authorization code. KubeCove uses PKCE S256, state and nonce.
- Assignment: restrict the application to the people allowed to sign in to this server.

Start the server with native TLS and the OIDC options:

```sh
./kubecove-server --listen 127.0.0.1:8443 \
  --tls-cert /path/to/certificate.pem --tls-key /path/to/private-key.pem \
  --public-origin https://<kubecove-host> \
  --oidc-issuer https://<provider-issuer> --oidc-client-id <registered-client-id> \
  --oidc-login-timeout <duration> \
  --oidc-client-secret-file /path/to/client-secret
```

`--oidc-login-timeout` is required and has no default. It bounds the sign-in transaction and the server's requests to the provider. Durations accept seconds, `m` or `h`, for example `15m`. Omit `--oidc-client-secret-file` for a public client. For a confidential client, mount the secret as a private file. Never pass the secret value as an argument or place it in chat, logs or frontend configuration.

The issuer must use HTTPS, and its discovery document must return the same issuer. Choose **Sign in**. The provider authenticates you and redirects back. The server validates the ID token and sets a private HttpOnly cookie. By default, the sign-in expires with that token. A server restart requires a new sign-in.

To forward each user's ID token to Kubernetes, add `--oidc-kubernetes-credentials <AUD>`. Kubernetes must accept the configured issuer and audience, and the ID token must contain that audience. Kubeconfig still selects the cluster and CA, but its authentication is removed from forwarded requests. Refresh tokens stay in server memory; renewal cannot extend the sign-in beyond 12 hours. An expired ID token stops credential forwarding during a provider outage while refresh remains retryable.

**Sign out** revokes the current sign-in, clears its bound bearer slots and closes the session identified by a matching resume token. Other sockets using that sign-in close and follow the disconnect grace policy. Tabs sharing the cookie share its revocation; separate sign-ins remain valid, including another sign-in for the same user. Sign-out does not sign you out of the identity provider.

For an Authentik example and Cloudflare Tunnel transport, see the [container deployment guide](https://github.com/Timpan4/kubecove/tree/main/deploy#openid-connect-with-authentik). The security contract is [ADR 0022](https://github.com/Timpan4/kubecove/blob/main/docs/decisions/0022-oidc-server-sign-in.md).

## Data and credential storage

The server's kubeconfig-source settings and token-mode startup token live in its data directory. Saved workspaces, UI settings and Argo profile metadata use client `localStorage`; navigation uses the URL hash and `sessionStorage`. Durable per-user server workspaces and settings remain proposed. Live Argo connections belong to each session. Kubeconfig responses contain summaries, not raw kubeconfig contents. On Unix, new data directories use mode `0700` and the startup token uses `0600`.

When native credential storage is unavailable on a Unix host, `kubecove-server` uses an encrypted file store for remembered Argo CD credentials and short-lived operation review sessions. Remembering a credential remains opt-in. The store uses `credentials.enc` and `credentials.key` with private permissions in the `0700` data directory. Keep both files on the host and protect backups. The store stops other OS users from reading credentials. It does not protect against processes running as the same user, root, or a backup that holds both files. The desktop app does not use this fallback.

## Sessions, reconnect and shutdown

Signing in from another browser or desktop client opens another session. Existing clients keep their streams, Pod exec sessions and port forwards. Both server-bound browser forwards and laptop-tunnelled desktop forwards remain owned by the session that created them.

A disconnected client can resume its matching session within the grace window, which defaults to five minutes. Set `--session-grace 0` to clean up immediately on disconnect, or a longer value such as `--session-grace 10m`. When the window expires, the server closes only that session's remaining live resources and forwards.

Stop the server with Ctrl+C or the host's service manager. Shutdown closes live sessions and forwards. To upgrade, stop the server, replace the executable from a reviewed release, and start it again. Keep the data directory.

## Security and support boundaries

- Non-loopback TCP requires `--allow-remote` and TLS. Unix sockets (`--socket`) use mode `0600`, cannot be combined with TLS or remote TCP, and need an exact `--public-origin` for browser access.
- In shared-credential mode, anyone allowed to sign in gets the server's Kubernetes permissions, subject to KubeCove's guarded workflows. The server's OS account and startup token must remain trusted.
- OIDC and Cloudflare Access control who can sign in to KubeCove. Kubernetes uses the server's configured credentials by default; grant that identity only what every permitted user should have. Opt-in OIDC forwarding uses each user's verified ID token and Kubernetes RBAC. Cloudflare Access does not forward its JWT to Kubernetes.
- Kubernetes and Argo CD access follows the [safety and data-handling rules](https://github.com/Timpan4/kubecove/wiki/Safety-Data-and-Architecture). KubeCove does not run arbitrary shell commands or deploy an in-cluster agent.

For engineering and security contracts, see [ADR 0020: Headless Server Mode](https://github.com/Timpan4/kubecove/blob/main/docs/decisions/0020-headless-server-mode.md), [ADR 0021: File Credential Store Without a Keyring](https://github.com/Timpan4/kubecove/blob/main/docs/decisions/0021-file-credential-store-without-keyring.md) and [ADR 0022: OIDC Server Sign-in](https://github.com/Timpan4/kubecove/blob/main/docs/decisions/0022-oidc-server-sign-in.md). Published release assets decide whether a server build is available for download.
