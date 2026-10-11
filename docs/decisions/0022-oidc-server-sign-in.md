# ADR 0022: OIDC server sign-in

## Status

Accepted 2026-10-10. The product owner selected KubeCove sign-in only.

## Decision

The headless server supports one configured OpenID Connect issuer as an alternative
to startup-token or Cloudflare Access authentication. These modes are mutually
exclusive. OIDC authenticates browser users. By default, Kubernetes requests use
the server's configured credentials, including an operator-granted cluster-admin
service account. This is shared operator configuration with independent browser
sessions, as defined by ADR 0020's session amendment. The forwarded-credential
amendment below enables per-user Kubernetes identity. Private-source provisioning
and durable user state remain proposed in [ADR 0023](0023-multi-user-sessions-and-persistence.md).

Use authorization code flow with PKCE S256 and a nonce. The operator registers
`<public-origin>/api/v1/oidc/callback` as the exact redirect URI. OIDC requires native
TLS, an exact HTTPS public origin, issuer and client ID. An optional client secret
comes from a file, never a command argument. The identity provider's application
assignment policy determines who may sign in. Do not use email as identity or
implicitly grant Kubernetes permissions from OIDC claims.

The issuer must be HTTPS. Discovery must return that exact issuer and HTTPS
authorization, token and signing-key endpoints. Server HTTP requests do not follow
redirects. Use the maintained `openidconnect` implementation for signature,
issuer, audience, authorized-party, expiry and nonce verification. Reject missing
ID tokens, empty subjects and unsupported signing algorithms. Do not refresh or
retain access, ID or refresh tokens after establishing authentication.

Login starts with a same-origin fetch POST. The server returns the authorization
URL and the browser navigates to it. A native form POST would send a null Origin
under the retained no-referrer policy and fail the exact-origin check.
A signed, HttpOnly, Secure, SameSite=Lax
transaction cookie holds the state, nonce and PKCE verifier. Its signature and
expiry bind the callback to the initiating browser. Compare returned state before
exchanging a code. Authorization codes are single-use at the provider, as required
by OIDC Core. Clear the transaction cookie after the callback. The operator must
configure `--oidc-login-timeout`; there is no invented default login deadline.

After callback validation, issue an opaque HttpOnly, Secure, SameSite=Strict
authentication cookie. Keep its issuer/subject identity and ID-token expiration
in server memory. Prune expired credentials when accessing the registry. Restart
requires a new login. The existing resume credential remains separate and is
bound to the verified identity. Protected HTTP requests and WebSocket upgrades
require the authentication cookie. WebSockets close on expiry or local logout.
The existing owner grace and cleanup rules still apply on disconnect.

Local logout revokes the current sign-in's authentication credential, clears its
bound Kubernetes bearer slots and cookies, and ends the session identified by a
matching resume token. Other sockets using that credential close and follow the
disconnect grace policy. Separate sign-ins remain valid, including those for the
same subject. Logout does not log the user out of the identity provider.
Callback errors never echo provider errors, authorization codes or credentials.
Redirect successful callbacks to `/` to remove protocol parameters from the page.
Keep Host/Origin checks, exact-origin CSP, no-referrer and no-store responses.

OIDC mode initially supports the browser client. Native desktop remote connections
continue to use token-mode servers. Native OIDC requires a separate external-browser
flow and is outside this decision.

## Amendment: forwarded Kubernetes credentials

Proposed 2026-10-11. The opt-in flag below is implemented. This reverses two rules above:
"do not refresh or retain access, ID or refresh tokens" and "do not
implicitly grant Kubernetes permissions from OIDC claims". The default is
unchanged.

With `--oidc-kubernetes-credentials <AUD>`, the server calls the Kubernetes
API as the signed-in user. `AUD` is the API server's `--oidc-client-id`. The API server authenticates the user's ID token through
its own `--oidc-*` flags, and RBAC decides per user, as Headlamp does. The
kubeconfig still selects the cluster and its CA. Its credentials are never
sent: the server strips them, and a deployment should hold none.

- The server keeps the ID token and refresh token in memory only, beside the
  opaque cookie's identity. Nothing is written to the data directory.
- Each session has its own slot for its user's ID token (ADR 0020, session
  amendment of 2026-10-11). A client built for the session carries the slot
  and reads it when each request is sent, so a renewed token applies to
  cached clients and watch reconnects. An empty slot sends no credentials.
  A client cannot be built outside a session.
- The slot is set when a session starts or resumes, and cleared when that
  session ends, signs out, or its sign-in expires or is revoked.
- Kubeconfig source settings are shared by all users, so they are read-only
  in every server authentication mode. The operator sets them.
- Only sign-ins with an open session are renewed, 60 seconds before the
  token expires. A
  renewed ID token must keep the subject and authorized party, and carry no
  nonce or the login nonce (OIDC Core 12.2). If the provider refuses after
  the token expired, the sign-in is revoked.
- Transport errors, provider outages and unusable refresh responses remain
  retryable. Once the ID token expires, forwarding is suspended until a valid
  renewal succeeds; the sign-in remains available within its existing deadline.
- Check the sign-in deadline before renewal and again before accepting or
  publishing its result. Session activation after renewal holds the session's
  request gate and rejects an owner that has already ended.
- The sign-in then lasts until the provider refuses a refresh, capped at 12
  hours, instead of ending with the first ID token. Without a refresh token
  it ends with the ID token, as before.
- The provider must put `AUD` in the token's audience, or sign-in is refused.
  `AUD` is the only audience trusted beside KubeCove's client ID, and the
  existing multi-audience rule still requires `azp` to be KubeCove's.

## Verification

Before implementation, cover successful code exchange and browser-cookie login,
PKCE transmission, cross-browser and wrong state, nonce mismatch, callback replay,
provider denial, issuer/audience/signature/expiry rejection, HTTPS metadata and
issuer mismatch, protected-route and cross-identity denial, logout revocation and
WebSocket expiry. Exercise the built browser login interaction against a local
deterministic provider. Preserve token and Cloudflare Access regression checks.

## Sources

- [OIDC Core](https://openid.net/specs/openid-connect-core-1_0.html)
- [OIDC Discovery](https://openid.net/specs/openid-connect-discovery-1_0.html)
- [PKCE, RFC 7636](https://www.rfc-editor.org/rfc/rfc7636)
- [Fetch: request Origin header](https://fetch.spec.whatwg.org/#append-a-request-origin-header)
- [openidconnect 4.0.1](https://docs.rs/openidconnect/4.0.1/openidconnect/)
