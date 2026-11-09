# ADR 0034: GPUI release bundles and update installation

## Status

Accepted on 2026-11-09 for the native client. Extends [ADR 0029](0029-gpui-native-client-and-feature-parity.md). The owner chose native bundles on Linux, macOS and Windows, updated in place as the Tauri desktop app is.

## Context

The native client could check for a newer release but not install one. The release feed held only Tauri installers, so installing from it would have replaced the native client with the Tauri app. The Tauri app updates through `tauri-plugin-updater`: a JSON manifest names one artifact per platform, each artifact has a minisign signature, and the app verifies the download against a public key compiled into it.

## Decision

### What a release publishes

The release workflow (`.github/workflows/gpui-release.yml`, called from `release.yml`) builds the native client on Linux x64, macOS and Windows x64 and bundles it with cargo-packager 0.11.8, a build-time tool that the binary does not link. Each release gets these assets beside the Tauri ones:

| Platform | Asset | How the client installs it |
| --- | --- | --- |
| Linux x64 | `kubecove-gpui_<version>_x86_64.AppImage` | Replaces the file at `$APPIMAGE` |
| macOS, universal | `kubecove-gpui_<version>_universal.app.tar.gz` with `KubeCove GPUI.app` at the archive root | Swaps the bundle that holds the running executable |
| Windows x64 | `kubecove-gpui_<version>_x64-setup.exe` (NSIS, per-user install) | Runs the installer in passive mode on relaunch |

One Linux job signs all three assets, so the updater key reaches one runner. Each asset gets a `.sig` file made with `tauri signer sign --app-version <version>` and the existing updater key (`TAURI_SIGNING_PRIVATE_KEY`). `latest-gpui.json` lists the assets in the Tauri updater's static format: `version`, `notes`, `pub_date`, and `platforms` keyed `linux-x86_64`, `darwin-x86_64`, `darwin-aarch64` and `windows-x86_64`, each with `url` and `signature`. Both `darwin` keys name the universal archive. The Tauri feed `latest.json` is unchanged, so neither client can be offered the other's artifact by its own feed.

The product is named KubeCove GPUI with the bundle identifier `com.timpan.kubecove.gpui`, so it installs beside the Tauri app. It keeps reading settings from the `com.timpan.kubecove` config directory (ADR 0030). The GPUI crate version follows the app version, because the client compares its own version with the manifest's.

A manual run of `release.yml` with the `gpui-dry-run` input builds, bundles, signs and verifies all three platforms and keeps the results as workflow artifacts. It creates no release, tag or draft.

### Trust

The client trusts one thing: the public key in `src-tauri/tauri.conf.json`, compiled into the binary. The manifest and the download are untrusted. Before anything is written:

1. The downloaded bytes must carry a valid signature from that key. Unhashed legacy signatures are refused, because the Tauri signer always prehashes.
2. The signature's trusted comment must hold `version:<version>` equal to the version the manifest announced. The trusted comment is covered by the signature. Without this check a tampered manifest could pair a new version number with an older signed build. The Tauri plugin calls the same rule `requireSignedVersion`; the native client always applies it, since it has no older unsigned-version releases to stay compatible with.
3. The trusted comment's `file:` field, which the signer sets to the signed file's name, must start with `kubecove-gpui_`. Both clients are signed with one key and both sign their version, so without this check a tampered manifest could point the native client at the Tauri artifact of the same version.

The manifest URL is derived from the Tauri endpoint in the same config, so the two clients share one release location and one key, and neither value is kept twice.

### Install and relaunch

The client downloads the artifact into memory, as the Tauri plugin does, then verifies and installs off the UI thread.

- **Linux.** The payload must be an ELF file. The client writes it to a new hidden file beside the AppImage, copies the AppImage's permissions, syncs it and renames it over the AppImage. The path always holds a complete AppImage. Relaunch starts the AppImage.
- **macOS.** The client unpacks the archive into a hidden directory beside the bundle, requires exactly one `.app` directory in it, moves the installed bundle aside, moves the new one into its place, and deletes the old one. If the second move fails it moves the installed bundle back. Relaunch runs `open -n` on the bundle.
- **Windows.** The payload must be a PE file. The client saves it to the user's temporary directory. Relaunch runs it with `/P` (passive) and `/R` (restart the app), and the app quits so the installer can replace its files.

Work files are created with `create_new`, so the client never writes through a link placed at a predictable path. A stale work file from an interrupted install is removed first, without following links.

Relaunch quits the app, so it obeys the close guard: it stays open while a YAML draft has unsaved changes.

The update states match the web: available, downloading with progress, installed and waiting for relaunch, and failed with the reason. Settings and the header popover show the same state.

### Differences from the Tauri updater

- On Windows the Tauri plugin starts the installer and exits the app as soon as the download finishes. The native client waits for Relaunch, so the user can finish work first.
- On macOS the Tauri plugin swaps bundles atomically where the file system allows and asks for administrator rights when the install location is not writable. The native client uses two renames and reports an error when the location is not writable. A crash between the two renames leaves the old bundle under a hidden name beside the install path.
- On Linux the client updates only an AppImage. A binary started outside an AppImage, such as a Nix or development build, reports that it cannot install updates.
- There are no proxy, header, timeout or downgrade settings.

## Risks accepted

- The manifest is not signed. Whoever can replace release assets can withhold updates or announce a version with no valid build, which fails verification. They cannot make the client install a build the key did not sign as a native client build of that version.
- The signed file name does not name the platform's install kind beyond its extension, and the client does not compare it with the platform. A tampered manifest could point one platform at another platform's native build of the same version. The payload check (ELF, PE or an archive with one app bundle) then refuses it.
- The bundles carry no Apple or Windows code signature, as the Tauri artifacts do not. Operating system warnings on first launch are the same as for the Tauri app.
- A signed installer saved in the temporary directory can be replaced by another process of the same user before Relaunch runs it. That process already runs with the user's rights.

## Verification

- Native tests (`src-tauri/gpui/tests/native/updates.rs`) list the failure modes first and cover manifest reading, the signature, signed-version and signed-file-name checks with signatures made by the Tauri signer, a download from a local server through to the replaced AppImage, and the bundle and installer paths. The macOS and Windows file operations run on Linux in those tests; only the platform's own launch command is not exercised.
- Shared scenarios `settings-updates.*` cover the install, progress, relaunch and failure states in both clients with scripted results.
- The release dry run proves that each platform builds, bundles and signs, and that each signature verifies against the public key and names the version.
- Not covered by any automated check: installing a real update on macOS and Windows, and the relaunch on all three platforms.
