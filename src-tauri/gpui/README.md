# GPUI client

This opt-in native client runs separately from the Tauri and Svelte client.
Kubernetes access goes through `Backend`, which calls the shared Rust commands.
See [ADR 0029](../../docs/decisions/0029-gpui-native-client-and-feature-parity.md)
and the [shared parity guide](../../e2e/parity/README.md) for the delivery boundary.

From the repository root:

```sh
cargo run --locked --manifest-path src-tauri/gpui/Cargo.toml
bun run e2e:parity
bun scripts/gpui-visual.ts rbac,workspaces,settings,palette
```

The parity and visual drivers render offscreen with controlled backend data.
They do not read the user's kubeconfig or change a cluster. Reports and screenshots
are retained under `e2e/artifacts/`. A passing scenario proves the UI interaction
through the command boundary, not a packaged app or a live Kubernetes request.

RBAC inspection, exact access verification, workspace scope selection, settings
search and command-palette navigation have shared scenarios. Saved workspaces and
RBAC review notes last for the session only. The repository has no Rust storage
commands for these records. Kubeconfig writes remain disabled in the production
native backend until its host initializes the existing settings storage path.
Other unsupported settings keep their controls disabled with a reason.

The workspace pins GPUI Kit 0.9.0 with components, assets and the YAML grammar.
Test support is enabled only for development builds. The backend dependency
uses the existing GUI-free configuration.
