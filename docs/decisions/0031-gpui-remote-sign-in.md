# ADR 0031: Desktop remote sign-in for OIDC and Access servers

## Status

Accepted 2026-11-08 for OIDC servers. The Access section stays proposed until one manual check against a real Access application confirms the four facts it lists. The product owner's answers on 2026-11-08:

1. Both desktop clients, GPUI and Tauri, get the sign-in and the session notices, with shared parity scenarios.
2. OIDC ships first. Access follows after the manual check.
3. Desktop sign-ins are stored in native credential storage, so they survive an app restart.
4. Disconnect keeps the sign-in. Only Sign out revokes it.
5. Notice text is the browser's, with "This browser's" changed to "This app's".
6. Notices show in the workspace content area and in Settings > Remote.

Extends [ADR 0020](0020-headless-server-mode.md), [ADR 0022](0022-oidc-server-sign-in.md) and [ADR 0029](0029-gpui-native-client-and-feature-parity.md). It answers the client-flow decision that ADR 0022 ("Native OIDC requires a separate external-browser flow") and [ADR 0023](0023-multi-user-sessions-and-persistence.md) ("native OIDC/Access sign-in needs a separate client-flow decision") left open. On acceptance, those two sentences are amended to point here. Gap audit row N20 tracks the missing behavior.

## Context

The desktop remote client lives in `kubecove_host::remote` (`src-tauri/crates/host/src/remote/`). The GPUI client calls it through `DesktopBackend` in `src-tauri/gpui/src/backend/remote.rs`, and the Tauri desktop client calls the same crate through `src/lib/remote-runtime.ts`. It supports only startup-token servers:

- `connect` posts `/api/v1/session` with `Authorization: Bearer <startup token>`. Every later invoke, channel close, session close and WebSocket upgrade sends the same bearer plus the resume token.
- `RemoteCredentials` holds the server URL, bearer and resume token in Rust memory for the life of the connection. Nothing is persisted, so an app restart needs the token again.
- The client sends no Origin header. The server accepts that only because `SecurityPolicy::check` lets a request omit Origin when its bearer equals the startup token (`src-tauri/server/src/security.rs`).
- A `session_moved` frame or an expired reconnect window ends the connection. Settings > Remote then says the server "is selected but disconnected". There is no takeover action, no expiry notice and no sign-in path.

OIDC and Access servers refuse this client. In OIDC mode the guard authenticates only the `__Host-kubecove_oidc` cookie, and the cookie is issued only to the browser that completed the callback. In OIDC and Access modes the guard refuses any POST or WebSocket upgrade without an Origin header. In Access mode Cloudflare's edge redirects a request without an Access token to a browser login page before it reaches the server.

The browser client's gate (`src/app/svelte/BrowserSessionGate.svelte`, `src/lib/browser-runtime.ts`) shows these states: connecting, sign-in or token entry, "Session ended" with "Start a new session" after a takeover or a refused resume, and expiry messages that depend on the authentication mode.

## Decision

### Authenticated session for OIDC servers

The KubeCove server brokers the desktop sign-in. The desktop is an OAuth 2.0 public native client of the KubeCove server, following [RFC 8252](https://www.rfc-editor.org/rfc/rfc8252): the system browser runs the authorization step, a loopback redirect returns the result, and PKCE S256 binds the result to the desktop process. The KubeCove server stays the only OIDC client of the identity provider. Identity-provider tokens never leave the server.

This keeps the identity provider registration unchanged: the operator's exact redirect URI `<public-origin>/api/v1/oidc/callback` and optional client secret stay as ADR 0022 defines them. Forwarded Kubernetes credentials under the ADR 0022 amendment keep working without change, because the server still holds the ID and refresh tokens.

Flow:

1. The desktop reads `GET /api/v1/auth`. It shows the token field only for `"mode":"token"`. For `"mode":"oidc"` it shows a "Sign in" action. A server that does not report `"nativeSignIn":true` gets the message "This server does not support desktop sign-in. Update the server." and no other attempt.
2. On "Sign in", Rust binds a listener to `127.0.0.1` on an OS-assigned port, falling back to `[::1]` when IPv4 loopback is unavailable (RFC 8252 sections 7.3 and 8.3; `localhost` is not used). It generates a PKCE verifier, its S256 challenge and a random state value, all from the OS CSPRNG.
3. The desktop opens the system browser at `<server>/api/v1/oidc/native/start?redirect_uri=http://127.0.0.1:<port>/kubecove/callback&code_challenge=<S256>&code_challenge_method=S256&state=<state>`. Embedded web views are not allowed (RFC 8252 section 8.12). The UI also offers "Copy sign-in link" for when no browser opens. The link carries no secret.
4. The server validates the parameters, then starts the same OIDC transaction as the browser flow: the signed `__Host-kubecove_oidc_tx` cookie in that browser holds the provider state, nonce and provider PKCE verifier, and also the desktop's redirect URI, challenge and state. The server answers with a redirect to the provider. The transaction deadline is the operator's `--oidc-login-timeout`.
5. The provider redirects to the existing `/api/v1/oidc/callback`. The server verifies exactly as ADR 0022 requires. For a native transaction it then creates a sign-in credential in the existing registry, without a cookie, and issues a single-use KubeCove authorization code bound to that credential, the desktop's challenge and its redirect URI. It redirects the browser to `http://127.0.0.1:<port>/kubecove/callback?code=<code>&state=<state>` and clears the transaction cookie.
6. The listener accepts only that path, compares `state` in constant time, answers with a static page that says sign-in finished and the browser tab can be closed, and stops listening. A provider denial arrives as `error=access_denied` and shows "Sign-in was not completed." Requests for other paths get a 404 and do not stop the listener.
7. The desktop posts `/api/v1/oidc/native/token` with the code, verifier and redirect URI. The server removes the code before checking it, so a code is used at most once even when the check fails. It then checks the deadline, the redirect URI and `BASE64URL(SHA256(verifier))` against the stored challenge. On success it returns the opaque credential. A second presentation of a consumed code revokes the credential that code produced ([RFC 9700](https://www.rfc-editor.org/rfc/rfc9700) section 4.2.4). The server keeps the link from a consumed code to its credential until that credential expires or is revoked, so a replay after the code's deadline still revokes it.
8. The desktop opens a session with `POST /api/v1/session` and `Authorization: Bearer <credential>`. From here the existing client code paths apply unchanged, with the sign-in credential in place of the startup token.

The KubeCove authorization code expires at the login transaction's deadline, which is the operator's `--oidc-login-timeout`. No new timeout is introduced. The desktop waits for the browser until the callback arrives or the user chooses Cancel or Disconnect; it sets no deadline of its own. Cancel, Disconnect and Sign out end the pending sign-in, including during the code exchange and while its session opens. The desktop checks that the sign-in is still pending before it stores the credential or keeps the session, and revokes a credential that arrives after the sign-in ended.

The sign-in credential is the same kind of entry as the browser cookie credential. It has the same identity, expiry, revocation token, forwarded-token slot and 12-hour forwarding cap. The server does not issue the desktop a refresh token. When the credential expires the user signs in again, as a browser user does.

The server cannot authenticate the desktop program (RFC 8252 sections 8.5 and 8.6). Any local program that the user lets complete a sign-in receives a session as that user, which is what that program could already reach through the user's browser. A KubeCove authorization code is useless without the verifier, so a code delivered to the wrong loopback listener or copied from browser history grants nothing.

### Authenticated session for Access servers

The desktop uses [Cloudflare Access Managed OAuth](https://developers.cloudflare.com/cloudflare-one/access-controls/applications/http-apps/managed-oauth/). This is Cloudflare's documented flow for a user-present non-browser client. Access acts as the authorization server, and the KubeCove server stays unchanged as the verifier of `Cf-Access-Jwt-Assertion`:

1. A request without an Access token gets Access's `401` with `WWW-Authenticate` pointing at the protected-resource metadata. The desktop reads that metadata and Access's authorization-server metadata.
2. The desktop registers itself as a public client (`token_endpoint_auth_method: "none"`) through dynamic client registration, then runs the same RFC 8252 loopback and PKCE S256 flow against Access's authorization endpoint in the system browser. Access applies the application's normal policies.
3. Access returns an access token and a refresh token. The desktop sends the access token on every request as Access requires, refreshes it through Access's token endpoint, and revokes it through Access's revocation endpoint on sign-out. Access's operator-configured lifetimes apply.
4. Access forwards a signed assertion to the KubeCove server. The server verifies it as today and opens or resumes the session for that subject.

The operator must enable Managed OAuth and "Allow loopback clients" on the Access application. Without them the desktop shows "This Cloudflare Access application does not allow desktop sign-in. Ask the operator to enable Managed OAuth." The startup-token and service-token paths are not substitutes, because a service token has no user identity and `cloudflared` is not a core data path.

This path depends on four facts that Cloudflare's documentation does not state. Verify them against an Access application before implementing (see "Remaining open item"):

- the access token header that Access expects, and whether Access forwards an `Authorization` header to the origin;
- that the forwarded assertion has the same `sub`, `type: "app"` and `aud` as a browser sign-in for the same user, so resume binding and identity-namespaced Argo CD credentials match;
- whether refresh tokens rotate, which RFC 9700 section 2.2.2 requires for public clients unless they are sender-constrained;
- that Managed OAuth is available on the plans KubeCove operators use.

### Origin rule for header credentials

The Origin check protects requests that a browser authenticates with a cookie. A browser cannot attach an `Authorization` header to a cross-site request or a WebSocket upgrade without a CORS preflight, and the server grants no CORS permissions. In OIDC and Access modes, a request may therefore omit Origin when it carries an `Authorization` header and no `Cookie` header. Every other POST and upgrade still needs an allowed Origin. Token mode keeps the existing rule that the bearer must equal the startup token. In OIDC mode the guard also refuses a request that carries both the authentication cookie and a bearer, so one request cannot mix two sign-ins. If Access does not forward `Authorization`, the desktop adds a fixed custom request header instead, which forces the same preflight.

### Where credentials live

The sign-in credential, the resume token, the Access access and refresh tokens, the PKCE verifier and the state value live only in Rust memory inside `kubecove_host::remote`, in the connection's `RemoteCredentials` and in the pending sign-in. They are never copied into GPUI view state, Svelte state, Tauri command results, preference documents (ADR 0030), logs, diagnostics or error messages. Error text keeps the existing redaction. Server error text reaches the desktop through `AppError` messages, so the host crate replaces the bearer and resume token values in it before the error leaves the crate. The code exchange and Sign out never forward server text. `RemoteConnectionSummary` gains only typed, non-secret fields: the authentication mode and the session state below.

The sign-in credential is persisted so it survives an app restart. It goes through the existing `SecureStore` adapter (`src-tauri/crates/gitops/src/commands/argo/credential_store.rs`), keyed by the normalized server URL under a new service name. The resume token, PKCE verifier and state value are never persisted. The desktop keeps ADR 0013's rule: native credential storage or nothing, with no file fallback. When native storage is unavailable, the credential stays in memory only and the client says so. ADR 0021's file store stays server-only.

The server keeps sign-ins only in memory, so a stored credential stops working at every server restart and at its server-side expiry. The client then shows "Sign-in expired" and deletes the stored credential. A stored credential is read only when the client connects to that server URL, and it is deleted on Sign out and on revocation.

### Logout and revocation

- Sign out posts `/api/v1/oidc/logout` with the bearer and `X-Kubecove-Session`. The server revokes that sign-in, clears its Kubernetes bearer slots, closes the session that the resume token names, and closes other sockets using the credential, exactly as it does for a browser logout. It does not log the user out of the identity provider.
- In Access mode, sign out revokes the Access token at Access's revocation endpoint, then closes the session with `/api/v1/session/close`.
- Disconnect closes the session but keeps the sign-in, in memory and in native storage. Reconnecting to the same server URL reuses it. Token-mode Disconnect stays as it is.
- On app exit the client closes the session within the existing session-close bound, which is the session grace window, and keeps the sign-in.
- Sign out and a received revocation drop the credential from memory and from native storage, whether or not the server call succeeds. Sign out deletes the local copies before it waits on the server, so a server that never answers, or an app closed during Sign out, cannot keep the sign-in. With an open session the server call is bounded by the session grace window. Without one it is bounded by the per-request stall limit the Kubernetes clients use (`REQUEST_READ_TIMEOUT` in `kubecove_kube`, 30 seconds).
- When native storage cannot delete the credential, the client does not report a finished sign-out. It does not read that stored entry again while it runs, and both clients show "This computer's credential store could not delete the sign-in, so it may still be stored. KubeCove will not use it while it runs. Remove any "KubeCove remote sign-in" entry for this server from the credential store." until a new sign-in for that server is stored over it.

### Session notices in the native UI

The host crate reports a typed session state, and both desktop clients render it. The state comes from the same server signals the browser uses:

| State | Server signal | Notice | Actions |
| --- | --- | --- | --- |
| Connecting | Session or socket being opened | "Connecting to KubeCove" / "Opening an authenticated session…" | Cancel |
| Waiting for browser | Pending loopback sign-in | "Finish signing in in your browser." | Copy sign-in link, Cancel |
| Reconnecting | Socket dropped within the grace window | "Connection interrupted. Reconnecting to the current session." | Disconnect |
| Session ended | `session_moved` frame, or HTTP 409 with kind `Session` | "Session ended" / "This app's server session closed or expired. Start a new session to continue. Other sessions on the server are not affected." | Start a new session, Disconnect |
| Sign-in expired | HTTP 401 on invoke or upgrade, or `/api/v1/auth` reporting `authenticated:false` after a socket close | "Your sign-in has expired. Sign in again." | Sign in, Disconnect |
| Session expired | Grace window elapsed without a reconnect | Token mode: "The server session expired. Reconnect with the server token." OIDC: "Your sign-in has expired. Sign in again." Access: "The server session expired. Sign in again through Cloudflare Access." | Token entry or Sign in, Disconnect |

"Start a new session" posts `/api/v1/session` without a resume token, using the credential the desktop still holds, as the browser's `takeOverBrowserSession` does. A 401 there moves to "Sign-in expired". The copy matches the browser's text, with "This browser's" changed to "This app's" and "Reload" replaced by an action, because the desktop has no page to reload.

While a selected remote is not connected, the workspace content area shows these notices in place of cluster views, as the browser gate does. Settings > Remote shows the same state. Disconnect stays available everywhere and is still the only way back to the local backend. Cluster commands never fall back to local credentials.

### Server changes

1. `/api/v1/auth` adds `"nativeSignIn":true`, and in OIDC mode reports `authenticated` for a bearer as well as for the cookie.
2. Add `GET /api/v1/oidc/native/start`. It requires no authentication and is exempt from the OIDC guard. It accepts only `redirect_uri` values of the form `http://127.0.0.1:<port>/kubecove/callback` or `http://[::1]:<port>/kubecove/callback`, with any non-zero port, and only `code_challenge_method=S256` with a well-formed challenge. It extends the signed transaction cookie with the native fields. It is a top-level navigation, so it needs no Origin.
3. Branch `/api/v1/oidc/callback` on the transaction kind. Browser transactions behave as today. Native transactions mint the KubeCove authorization code and redirect to the loopback URI.
4. Add `POST /api/v1/oidc/native/token`, exempt from authentication, with the single-use, PKCE and replay rules above. Codes live in server memory with their credential and expire at the transaction deadline. The record of a consumed code stays until its credential expires or is revoked. Both are pruned when the registry is accessed, as credentials are.
5. The OIDC guard, logout and WebSocket upgrade accept `Authorization: Bearer <credential>` as an alternative to the cookie. The guard refuses requests that carry both.
6. `SecurityPolicy::check` applies the Origin rule for header credentials above.
7. Access mode needs no new endpoint. It may need the custom-header variant of the Origin rule, depending on verification.

Callback errors, logs and observability events never include codes, verifiers, states, credentials or provider errors, as ADR 0022 already requires. Host and Origin checks for browsers, the exact-origin CSP, `no-referrer` and `no-store` stay unchanged.

### Client implementation notes

- The sign-in flow lives in `kubecove_host::remote`, so the GPUI client and the Tauri desktop client share it. Each client keeps its own view code.
- The loopback listener uses an HTTP server implementation already in the dependency tree. It does not parse HTTP by hand. Any new dependency follows the project's dependency rules.
- Opening the browser must not let the URL become part of a command line. The current `open_documentation` adapter runs `cmd /C start "" <url>` on Windows, where `&` in a query string is a command separator, so the sign-in URL does not use it. It uses the `open` crate 5.4.5 with default features, which never runs `cmd`. On macOS it runs `/usr/bin/open` with the URL as one argument. On Linux it tries `xdg-open` and then other desktop launchers, each with the URL as one argument. On Windows it runs `powershell.exe -NoProfile -NonInteractive -Command` with a fixed script that opens `$env:OPEN_RS_TARGET` with `Invoke-Item -LiteralPath` when it names an existing file and with `Start-Process -FilePath` otherwise, and passes the URL only in the `OPEN_RS_TARGET` environment variable, so PowerShell never parses it as script. If PowerShell fails it runs `explorer.exe` with the URL as one argument.
- The server redirects from an HTTPS page to an `http://127.0.0.1` top-level navigation. Verify in each supported browser that local-network access restrictions do not block this navigation.

## Consequences

The desktop clients can use every server authentication mode. OIDC needs no identity-provider change and keeps all provider tokens on the server. Access needs an operator setting on the Access application, and its path stays unverified until the facts listed above are checked. Desktop sign-ins appear in server logs and metrics as OIDC or Access sign-ins, like browser sign-ins. Desktop users keep their sign-in across app restarts and Disconnect, and sign in again after a server restart or the sign-in's server-side expiry. A stored desktop credential in the OS credential store is valid on the server until that expiry, so anyone who can read the user's credential store can use it until then. Sign out removes it.

## Verification

Server boundary tests, using the existing deterministic provider in `oidc_mock.rs`: native start parameter validation (non-loopback, `localhost`, HTTPS or other-path redirect URIs, missing or `plain` challenge), browser and native transactions on one callback, code single use, replay revocation, wrong verifier, wrong redirect URI, expired code, provider denial, bearer without Origin accepted, cookie without Origin refused, cookie and bearer together refused, logout and revocation by bearer, WebSocket close at bearer expiry, cross-identity resume refusal, forwarded-credential binding for a bearer session, and unchanged token and Access regressions.

Host crate tests against a local fake server: loopback listener state mismatch, wrong path, provider denial, cancel, IPv6 fallback, credential redaction in every error, memory cleanup on sign-out and disconnect, and each session-state transition in the table.

Shared parity scenarios in `e2e/parity/scenarios/` drive each notice and action through both drivers with mocked responses. GPUI native tests in `src-tauri/gpui/tests/native.rs` cover what the mocks cannot express, such as revocation arriving during a read. No test contacts a real identity provider, Cloudflare, cluster or server, or reads the user's settings, keyring or kubeconfig.

## Remaining open item

Access: one manual check against a real Access application, for the four facts listed under "Authenticated session for Access servers". Until it passes, desktop clients show "Desktop sign-in for Cloudflare Access is not available yet." for an Access server.

## Sources

- [RFC 8252, OAuth 2.0 for Native Apps](https://www.rfc-editor.org/rfc/rfc8252), sections 6, 7.3, 8.1, 8.3 to 8.6 and 8.12
- [RFC 7636, PKCE](https://www.rfc-editor.org/rfc/rfc7636)
- [RFC 9700, OAuth 2.0 Security Best Current Practice](https://www.rfc-editor.org/rfc/rfc9700), sections 2.1, 2.1.1, 2.2.2 and 4.2.4
- [RFC 6750, Bearer Token Usage](https://www.rfc-editor.org/rfc/rfc6750)
- [Cloudflare Access: Managed OAuth](https://developers.cloudflare.com/cloudflare-one/access-controls/applications/http-apps/managed-oauth/)
- [Cloudflare Access: Authenticate agents](https://developers.cloudflare.com/cloudflare-one/access-controls/authenticate-agents/)
- [Cloudflare Access: Validate JWTs](https://developers.cloudflare.com/cloudflare-one/access-controls/applications/http-apps/authorization-cookie/validating-json/)
