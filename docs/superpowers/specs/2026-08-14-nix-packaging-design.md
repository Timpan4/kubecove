# Nix packaging design

## Goal

Package KubeCove for Linux with reproducible Nix builds and support both flake and non-flake invocation:

```sh
nix build
nix run
nix-build -A kubecove
```

The package must keep frontend dependency resolution separate from the Rust/Tauri application build.

## Entry points and dependency pins

`flake.nix` pins nixpkgs, bun2nix, and flake-compat. `flake.lock` is the only dependency pin source.

`default.nix` has two evaluation paths:

1. `flake.nix` supplies `pkgs` and `bun2nix`, and `default.nix` returns the package set directly.
2. Legacy evaluation without supplied arguments bootstraps the exact flake-compat revision recorded in `flake.lock`. Flake-compat evaluates the root flake, whose call back into `default.nix` uses the first path and therefore does not recurse.

The returned package set contains:

- `frontend`: compiled frontend assets.
- `kubecove`: the complete desktop application.
- `default`: an alias for `kubecove`.

The flake exposes the same three package names for `x86_64-linux` and `aarch64-linux`, plus an app named `kubecove` so `nix run` launches the installed binary.

## Frontend derivation

`nix/frontend.nix` uses `bun2nix.mkDerivation` and `bun2nix.fetchBunDeps`. Its dependency cache comes from a committed `bun.nix` generated from `bun.lock`.

The frontend source contains only files needed by the production Vite build: package metadata and lockfiles, TypeScript and Vite configuration, `index.html`, `public`, and `src`. Rust-only changes therefore do not rebuild the frontend.

The derivation runs the existing `bun run build` command and installs `dist` as its output. It sets `VITE_KUBECOVE_RELEASE_CHANNEL=dev`, because a Nix-managed application must not replace itself through Tauri's updater.

`bun.nix` must be regenerated with the pinned bun2nix version whenever `bun.lock` changes.

## Rust and Tauri derivation

`nix/kubecove.nix` uses `rustPlatform.buildRustPackage` with `cargo-tauri.hook`, `cargoHash`, `cargoRoot = "src-tauri"`, and `buildAndTestSubdir = "src-tauri"`.

Its source contains the Rust/Tauri tree only. A generated Tauri configuration overlay:

- Clears `build.beforeBuildCommand` so the Rust build never invokes Bun.
- Points `build.frontendDist` at the frontend derivation.
- Clears `bundle.externalBin` so the vendored architecture-specific sidecar is not bundled.
- Disables updater artifact generation.

The derivation uses the standard Linux Tauri build and runtime dependencies from nixpkgs, including WebKitGTK 4.1, GTK, GLib networking, OpenSSL, libsecret, Ayatana app-indicator, librsvg, pkg-config, and the appropriate GApps wrapper.

The installed executable is wrapped with:

```sh
KUBECOVE_KUBECONFORM=${pkgs.kubeconform}/bin/kubeconform
```

This uses nixpkgs' independently packaged kubeconform and permits both supported Linux architectures. The existing application resolver already treats this environment variable as the highest-priority sidecar location.

The package metadata uses the existing description, AGPL-3.0-or-later license, project homepage, Linux platforms, and `kubecove` as the main program.

## Runtime behavior

`nix run` launches the wrapped `kubecove` executable. Kubernetes configuration, credentials, keyring integration, and application state continue to use their existing runtime locations. No backend behavior or storage contract changes.

Automatic application update checks remain disabled in the Nix build. Updates are delivered by rebuilding or updating the Nix input/package.

## Release pipeline

The release workflow includes a required Nix verification job before creating or publishing release assets. A matrix builds `.#kubecove` on native `x86_64-linux` and `aarch64-linux` GitHub-hosted runners. The existing release gate reports the matrix result and fails unless it succeeds.

No Nix archive is attached to GitHub releases and no external binary cache is introduced. Tagged releases are consumed directly with:

```sh
nix run github:Timpan4/kubecove/app-vX.Y.Z
```

A focused release-workflow test protects the required Nix matrix and release-gate dependency from accidental removal.

## Failure handling

Builds run without network access after fixed-output dependencies have been fetched:

- Cargo dependencies are validated by `cargoHash`.
- Bun dependencies are represented by generated `bun.nix` and fetched through bun2nix.
- A stale `bun.nix` causes the frontend build to fail rather than resolving unpinned packages.
- Missing frontend assets, Tauri bundle output, or kubeconform dependencies cause the application derivation to fail.

## Verification

The implementation is complete when these checks pass on Linux:

```sh
nix build .#frontend
nix build .#kubecove
nix flake check
nix-build -A frontend
nix-build -A kubecove
nix run .
```

For `nix run`, verification consists of confirming that the desktop process starts successfully. No release artifact archive, binary cache, development shell, or upstream nixpkgs submission is included.
