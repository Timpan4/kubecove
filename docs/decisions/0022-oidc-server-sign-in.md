# ADR 0022: OIDC server sign-in

## Status

Accepted 2026-10-10. The product owner selected KubeCove sign-in only.

## Decision

The headless server supports one configured OpenID Connect issuer as an alternative
to startup-token or Cloudflare Access authentication. These modes are mutually
exclusive. OIDC authenticates browser users. Kubernetes requests continue to use
the server's configured credentials, including an operator-granted cluster-admin
service account. This first stage remains a shared backend with one active owner,
as described in [ADR 0020](0020-headless-server-mode.md). Concurrent user sessions
and both shared and per-user Kubernetes credentials are planned separately in
proposed [ADR 0023](0023-multi-user-sessions-and-persistence.md).

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

Local logout revokes the authentication credential, clears cookies and ends only
the matching owned session. It does not log the user out of the identity provider.
Callback errors never echo provider errors, authorization codes or credentials.
Redirect successful callbacks to `/` to remove protocol parameters from the page.
Keep Host/Origin checks, exact-origin CSP, no-referrer and no-store responses.

OIDC mode initially supports the browser client. Native desktop remote connections
continue to use token-mode servers. Native OIDC requires a separate external-browser
flow and is outside this decision.

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
