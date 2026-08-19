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

The installers include the application and runtime only. Provide your own kubeconfig and Kubernetes tooling where needed; installers do not include cluster credentials, `kubectl`, Helm, or GitOps CLIs.

### Windows installer migration

Windows releases use the NSIS setup executable so manual installation and in-app updates use the same installer path. If an older MSI installation is present, Windows can prompt once to remove it. Uninstall that MSI, install the NSIS setup executable, and later updates do not need the migration.

## First launch

The workspace launcher discovers contexts from `$KUBECONFIG`; if it is unset, Kubernetes default discovery uses `~/.kube/config` (or `%USERPROFILE%\.kube\config` on Windows). Select a readable context, optionally narrow namespaces, and create a workspace. A missing or unreadable kubeconfig is reported instead of exposing its contents.

For a different source, open Settings and choose a valid environment-variable name or add kubeconfig paths. The interface receives source summaries and context names, not raw kubeconfig data, tokens, or certificates.

Continue with [workspaces and scope](https://github.com/Timpan4/kubecove/wiki/Workspaces-and-Scope).

## Update

Release-channel builds check the published update feed at launch and from the update control in the app header or Settings. Development and ad hoc builds use the development channel and do not check for updates.

When an update is available, review the release notes, select **Install**, wait for the download and installer step to finish, then select **Relaunch**. Relaunching is required to complete the update. If the check or install fails, use the reported error and retry after resolving the network or platform issue.

For a clean reinstall or a platform-specific artifact, use the [latest release installers](https://github.com/Timpan4/kubecove/releases/latest).
