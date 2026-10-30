# Install and update

## Install

1. Open the [latest release installers](https://github.com/Timpan4/kubecove/releases/latest).
2. Download the artifact for your platform:
   - macOS: `.dmg`
   - Windows: `*-setup.exe` (NSIS)
   - Linux: `.AppImage`, `.deb`, or `.rpm` when published
3. Install and launch the app.

Beta installers are not OS-signed. macOS may require **Open** from the contextual menu or approval in System Settings. Windows SmartScreen may require **More info** then **Run anyway**. An AppImage may need:

```sh
chmod +x KubeCove_*.AppImage
```

To run KubeCove on a remote host instead, see [Install the server](https://github.com/Timpan4/kubecove/wiki/Server-Installation).

The installers include the application and runtime only. Provide your own kubeconfig and Kubernetes tooling where needed; installers do not include cluster credentials, `kubectl`, Helm, or GitOps CLIs.

### Nix

On Linux with Nix, run a checkout from its repository root:

```sh
nix run
```

Run a tagged release by replacing `X.Y.Z` with its version:

```sh
nix run github:Timpan4/kubecove/app-vX.Y.Z
```

Nix builds do not use the in-app updater. Select a newer release tag to update.

### Windows installer migration

Windows releases use the NSIS setup executable so manual installation and in-app updates use the same installer path. If an older MSI installation is present, Windows can prompt once to remove it. Uninstall that MSI, install the NSIS setup executable, and later updates do not need the migration.

## First launch

The workspace launcher discovers contexts from `$KUBECONFIG`; if it is unset, Kubernetes default discovery uses `~/.kube/config` (or `%USERPROFILE%\.kube\config` on Windows). Select a readable context, optionally narrow namespaces, and create a workspace. A missing or unreadable kubeconfig is reported instead of exposing its contents.

For a different source, open Settings and choose a valid environment-variable name or add kubeconfig paths. The interface receives source summaries and context names, not raw kubeconfig data, tokens, or certificates.

Continue with [workspaces and scope](https://github.com/Timpan4/kubecove/wiki/Workspaces-and-Scope).

## Exec credential plugins

KubeCove uses the exec credential plugin configured in your kubeconfig, including
`kubectl oidc-login get-token` with Krew-installed oidc-login. Install and configure
the plugin yourself. It owns provider login, browser interaction, and token caching.
KubeCove does not bundle or manage plugins.

Commands use KubeCove's process environment and the kubeconfig's `exec.env` entries.
A desktop launcher can have a different `PATH` from your terminal. For Krew,
ensure both `kubectl` and the Krew bin directory are available to that environment.
Alternatively, set `exec.command` to the absolute path of the installed
`kubectl-oidc_login` executable and pass `get-token` followed by your existing
provider arguments. An explicit `PATH` in `exec.env` must include all directories
the plugin needs; shell variable expansion does not apply there.

If authentication fails, check the same context in a terminal, resolve the plugin's
error, then retry in KubeCove. App errors omit plugin arguments, environment values,
and captured output. A plugin configured to inherit stderr can still write to the
launching terminal. Workspace switching stops waiting on finite requests, but does
not terminate an already running plugin or its browser window.

For remote connections, operator-configured plugins execute on the server host
with that host's environment. KubeCove does not forward their browser login to your
computer. Server OIDC sign-in and forwarded-user credentials are separate features.

## Update

Release-channel builds check the published update feed at launch and from the update control in the app header or Settings. Development and ad hoc builds use the development channel and do not check for updates.

When an update is available, review the release notes, select **Install**, wait for the download and installer step to finish, then select **Relaunch**. Relaunching is required to complete the update. If the check or install fails, use the reported error and retry after resolving the network or platform issue.

For a clean reinstall or a platform-specific artifact, use the [latest release installers](https://github.com/Timpan4/kubecove/releases/latest).
