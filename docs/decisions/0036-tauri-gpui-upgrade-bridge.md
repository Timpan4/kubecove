# ADR 0036: Install GPUI beside the Tauri desktop client

Accepted for 0.15.x; retired after the `app-v0.15.2` pitstop once Tauri was removed and `0.16.0+` consolidated on `KubeCove` (`com.timpan.kubecove`, `latest-desktop.json`). Legacy installs read the `latest.json` and `latest-gpui.json` manifests of 0.15.2, carried forward by `release.yml`.

The Tauri desktop client offers migration through its local Settings window. The server, web client and remote cluster transport have no upgrade commands. Rust reads ADR 0034's feed, accepts a same-version or newer build, and binds both the version and exact platform asset name to the verified minisign trusted comment. A translated Intel process on Apple silicon selects the native Apple silicon build.

The user picks a destination folder through a Rust-owned native dialog on Linux and macOS. The download goes to the user's application cache. Windows uses a fresh directory under the user's temporary folder for the download and installer. After verification, Rust publishes the AppImage, app bundle, or installer without replacing an existing destination. A separate action launches or reveals the recorded result. Windows asks for confirmation before starting setup. Tauri stays open so its normal close guard can protect drafts.

Development builds check the feed only when the user asks. Rust limits the manifest to the same 2 MiB budget as other remote JSON responses. Windows stages installers under a per-user temporary parent and reclaims unused installer directories on the next installation.

Rust keeps the native path and sends a display string to Settings. Launch requires a location protected against writes by other users. Rust records the executable's SHA-256 digest during installation and checks the path permissions and digest again before starting that executable. On macOS this is the main binary inside the verified bundle. Settings offers only the folder action when Rust cannot establish launch safety.

Kubeconfig sources, MCP settings and token, and stored server credentials already use shared storage. Workspaces use the existing JSON export and import. Display preferences and Argo CD profiles start fresh. Users who run MCP can reveal the installed app, quit Tauri to free the listener port, then start GPUI.

## Version 0.15.2 pitstop

Both legacy update feeds end at 0.15.2. Tauri 0.15.0 and earlier update to Tauri 0.15.2 through `latest.json`, and side-by-side GPUI updates to GPUI 0.15.2 through `latest-gpui.json`. Each 0.15.2 client then updates in place from `latest-desktop.json` to the consolidated KubeCove 0.16.0 or later, like any other update. Tauri 0.15.2 has no side-by-side bridge, and it writes the snapshot below once more before the download starts. Release 0.15.1 was recalled. Its clients compiled in a 0.15.1 update limit and showed only a GitHub Releases link, so their installs need one manual install. See [ADR 0034](0034-gpui-release-bundles-and-updates.md) for the release asset and bundle identity checks.

On startup, when the document becomes hidden, and before an update starts, Tauri snapshots the allowed fields of `kubecove-settings` and `kubecove-workspaces`. Rust writes schema version 1 to `tauri-migration.json` in the shared `com.timpan.kubecove` config directory. Entries retain the storage key and its state envelope. The file includes the source app version and UTC export time. Equal entries keep the previous file. Replacement is atomic and private to the owner on Unix.

Allowed state includes display and behavior settings, workspace scope and shortcuts, port-forward presets, pins, recents, RBAC review notes and Argo connection metadata. Credentials, unknown fields, session storage, transient errors and update-check history are excluded. User-entered text is retained unless it contains recognizable credential syntax. An arbitrary password entered as ordinary text cannot be distinguished from a name or note. Credential stores retain sole ownership of saved authentication data.

GPUI imports the first valid snapshot once and records completion in `live-sessions.json` together with the merged data. It ignores later snapshots and never renames, moves or deletes `tauri-migration.json`. Port-forward presets are skipped when no kubeconfig source resolves at import time. GPUI stores a report of skipped entries with the import and shows it in Settings > General until the user dismisses it.
