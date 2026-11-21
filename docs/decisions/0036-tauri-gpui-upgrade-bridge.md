# ADR 0036: Install GPUI beside the Tauri desktop client

Accepted.

The Tauri desktop client offers migration through its local Settings window. The server, web client and remote cluster transport have no upgrade commands. Rust reads ADR 0034's feed, accepts a same-version or newer build, and binds both the version and exact platform asset name to the verified minisign trusted comment. A translated Intel process on Apple silicon selects the native Apple silicon build.

The user picks a destination folder through a Rust-owned native dialog on Linux and macOS. The download goes to the user's application cache. Windows uses a fresh directory under the user's temporary folder for the download and installer. After verification, Rust publishes the AppImage, app bundle, or installer without replacing an existing destination. A separate action launches or reveals the recorded result. Windows asks for confirmation before starting setup. Tauri stays open so its normal close guard can protect drafts.

Development builds check the feed only when the user asks. Rust limits the manifest to the same 2 MiB budget as other remote JSON responses. Windows stages installers under a per-user temporary parent and reclaims unused installer directories on the next installation.

Rust keeps the native path and sends a display string to Settings. Launch requires a location protected against writes by other users. Rust records the executable's SHA-256 digest during installation and checks the path permissions and digest again before starting that executable. On macOS this is the main binary inside the verified bundle. Settings offers only the folder action when Rust cannot establish launch safety.

Kubeconfig sources, MCP settings and token, and stored server credentials already use shared storage. Workspaces use the existing JSON export and import. Display preferences and Argo CD profiles start fresh. Users who run MCP can reveal the installed app, quit Tauri to free the listener port, then start GPUI.
