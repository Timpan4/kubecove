# ADR 0036: Install GPUI beside the Tauri desktop client

Accepted for 0.15.x; retired after `app-v0.15.1` once Tauri was removed and `0.16.0+` consolidated on `KubeCove` (`com.timpan.kubecove`, `latest-desktop.json`). Legacy `0.15.1` installs continue to read the frozen `latest.json` and `latest-gpui.json` manifests carried forward by `release.yml`.

The Tauri desktop client offers migration through its local Settings window. The server, web client and remote cluster transport have no upgrade commands. Rust reads ADR 0034's feed, accepts a same-version or newer build, and binds both the version and exact platform asset name to the verified minisign trusted comment. A translated Intel process on Apple silicon selects the native Apple silicon build.

The user picks a destination folder through a Rust-owned native dialog on Linux and macOS. The download goes to the user's application cache. Windows uses a fresh directory under the user's temporary folder for the download and installer. After verification, Rust publishes the AppImage, app bundle, or installer without replacing an existing destination. A separate action launches or reveals the recorded result. Windows asks for confirmation before starting setup. Tauri stays open so its normal close guard can protect drafts.

Development builds check the feed only when the user asks. Rust limits the manifest to the same 2 MiB budget as other remote JSON responses. Windows stages installers under a per-user temporary parent and reclaims unused installer directories on the next installation.

Rust keeps the native path and sends a display string to Settings. Launch requires a location protected against writes by other users. Rust records the executable's SHA-256 digest during installation and checks the path permissions and digest again before starting that executable. On macOS this is the main binary inside the verified bundle. Settings offers only the folder action when Rust cannot establish launch safety.

Kubeconfig sources, MCP settings and token, and stored server credentials already use shared storage. Workspaces use the existing JSON export and import. Display preferences and Argo CD profiles start fresh. Users who run MCP can reveal the installed app, quit Tauri to free the listener port, then start GPUI.

## Version 0.15.1 handoff

Both legacy update feeds freeze at 0.15.1. Settings checks the consolidated feed first and offers GitHub Releases for a compatible KubeCove 0.16.0 or later. The bridge installs a side-by-side build only from the legacy GPUI feed. See [ADR 0034](0034-gpui-release-bundles-and-updates.md) for the release asset and bundle identity checks.

On startup, when the document becomes hidden, and before opening the installer or release page, Tauri snapshots the allowed fields of `kubecove-settings` and `kubecove-workspaces`. Rust writes schema version 1 to `tauri-migration.json` in the shared `com.timpan.kubecove` config directory. Entries retain the storage key and its state envelope. The file includes the source app version and UTC export time. Equal entries keep the previous file. Replacement is atomic and private to the owner on Unix.

Allowed state includes display and behavior settings, workspace scope and shortcuts, port-forward presets, pins, recents, RBAC review notes and Argo connection metadata. Credentials, unknown fields, session storage, transient errors and update-check history are excluded. User-entered text is retained unless it contains recognizable credential syntax. An arbitrary password entered as ordinary text cannot be distinguished from a name or note. Credential stores retain sole ownership of saved authentication data.

GPUI imports the first valid snapshot once and records completion in `live-sessions.json` together with the merged data. It ignores later snapshots and never renames, moves or deletes `tauri-migration.json`.
