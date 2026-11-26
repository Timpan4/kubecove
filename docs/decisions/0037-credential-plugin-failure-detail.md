# ADR 0037: Credential plugin failure detail

## Status

Accepted on 2026-11-26.

## Context

Kubeconfig users can sign in through an exec credential plugin, such as `kubectl oidc-login`, `aws eks get-token` or `gke-gcloud-auth-plugin`. When the plugin fails, `kube-rs` returns the exit status, stdout, stderr and the command with its arguments and environment. Before this decision, `AppError::from(kube::Error)` kept all of that in Rust and showed only "The kubeconfig credential plugin exited with status 1." The UI did not say which context failed or why, so a user had to rerun the plugin by hand to learn the cause.

In the case that prompted this decision, the identity provider had moved to a new host name. kubelogin's stderr said so in one line: the issuer URL in the kubeconfig did not match the issuer URL the provider returned. KubeCove showed only the exit status.

The plugin's output is not safe to forward as-is:

- stdout carries the token on success and can carry partial credentials on failure.
- The arguments and environment can hold client secrets, tokens or file paths to keys.
- A verbose or debug plugin can print headers, OAuth redirects with authorization codes, or raw tokens on stderr.

[ADR 0014](0014-runtime-secret-disclosure.md) keeps tokens and kubeconfig credentials backend-only. [ADR 0020](0020-headless-server-mode.md) keeps error shapes transport-neutral. [ADR 0027](0027-server-observability.md) records server errors by kind only. [ADR 0028](0028-read-only-mcp-server.md) returns errors to agents as message and kind.

## Decision

`AppError` gains an optional `credential` field. Rust sets it only when all of these hold:

- The error is a Kubernetes authentication failure.
- The client was built for the desktop user's own kubeconfig, which is `SourcePolicy::Local`. Server sessions never build the data, so server users and remote desktops never receive it.
- The context's user signs in through an exec plugin.

The field holds:

| Field | Source | Rule |
| --- | --- | --- |
| `context` | The requested context name | As given. |
| `user` | The context's kubeconfig user entry name | Read from the kubeconfig. |
| `command` | The plugin program and its leading subcommand words | The program's file name, then each following argument that is a lowercase word of letters, digits and hyphens. The first other argument ends it, because flags, values and paths can carry secrets. |
| `kubeconfig` | The kubeconfig files KubeCove read | File paths only, never contents. |
| `reproduce` | A terminal command for the user to copy | `kubectl --context <context> auth whoami`, with `KUBECONFIG` set when KubeCove read files other than the default kubeconfig. Values are quoted for a POSIX shell, or for PowerShell on Windows. |
| `exitStatus` | The plugin's exit code | Absent when the plugin was killed or did not start. |
| `pluginMessage` | The plugin's stderr | See below. |

`pluginMessage` is the last non-empty stderr line after Rust:

1. decodes it as lossy UTF-8;
2. removes ANSI escape sequences, control characters and bidirectional formatting characters;
3. applies the shared error-detail redaction patterns, which now live in `kubecove-models` and match `src/lib/error-redaction.ts`;
4. removes every URL query and fragment, JSON Web Tokens, and `refresh_token`, `secret` and `session_token` values.

The line is not shortened. The UI wraps it.

Stdout, the plugin's other arguments and its environment never leave Rust. `Debug` output and logs exclude `pluginMessage`. MCP tools return only `message` and `kind`, as before, so agents do not receive the field.

The first plugin run happens while `kube-rs` builds the client, so the client builder attaches the data there. A token refresh during a request fails later, inside the client's service stack. A client layer wraps only errors whose source chain holds an authentication error, so `AppError::from` can describe a refresh failure too. Other errors pass through unchanged.

Both clients show the same panel when the field is present: "Could not sign in to `<context>`", the exit status, the plugin line, a Copy terminal command button and the sign-in details. Errors without the field look as before.

## Consequences

Users see the plugin's own reason for a failed sign-in and the exact kubeconfig entry to fix, without leaving KubeCove.

A plugin can still print a secret in a form the patterns do not recognize. The redaction removes the forms seen in common plugins and OAuth flows. The plugin line is shown only to the desktop user who owns the kubeconfig and could run the plugin themselves.

Errors that the clients combine across workspace contexts keep only their combined message and lose the field. The Svelte and GPUI clients behave the same way there.

Any change that forwards stdout, plugin arguments or environment, or that sends the field to server sessions, logs or MCP, requires amending this ADR.
