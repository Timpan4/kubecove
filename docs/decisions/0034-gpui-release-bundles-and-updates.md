# ADR 0034: GPUI release bundles and update installation

## Status

Accepted on 2026-11-09 for the native client. Extends [ADR 0029](0029-gpui-native-client-and-feature-parity.md). The owner chose native bundles on Linux, macOS and Windows, updated in place as the Tauri desktop app is.

## Context

The native client could check for a newer release but not install one. The release feed held only Tauri installers, so installing from it would have replaced the native client with the Tauri app. The Tauri app updates through `tauri-plugin-updater`: a JSON manifest names one artifact per platform, each artifact has a minisign signature, and the app verifies the download against a public key compiled into it.

## Decision

### What a release publishes

The release workflow (`.github/workflows/gpui-release.yml`, called from `release.yml`) builds the native client on Linux x64, macOS and Windows x64 and bundles it with cargo-packager 0.11.8, a build-time tool that the binary does not link. Each release includes these native assets:

| Platform | Asset | How the client installs it |
| --- | --- | --- |
| Linux x64 | `kubecove_<version>_x86_64.AppImage` | Replaces the file at `$APPIMAGE` |
| macOS, Apple silicon | `kubecove_<version>_aarch64.app.tar.gz` with `KubeCove.app` at the archive root | Swaps the bundle that holds the running executable |
| Windows x64 | `kubecove_<version>_x64-setup.exe` (NSIS, per-user install) | Runs the installer in passive mode on relaunch |

On Linux and Windows the updater asset is also the first-install download. On macOS the release also ships `kubecove_<version>_aarch64.dmg`, built by cargo-packager from the same bundle. It is outside the updater feed and has no `.sig`.

The Linux release ships an AppImage. Debian and RPM installers are available only from the legacy Tauri build.

The native client has no Intel macOS build; the owner dropped it after 0.14.1, which shipped a universal bundle. An Intel Mac running 0.14.1 finds no `darwin-x86_64` entry and reports that the newer release has no build for its platform.

One Linux job signs all three assets, so the updater key reaches one runner. The signing job decodes the existing base64-wrapped minisign key (`TAURI_SIGNING_PRIVATE_KEY`) and passes its optional password to standalone `minisign` on stdin. Each `.sig` contains the base64-encoded minisign signature text. Its trusted comment binds the timestamp, file name and version as tab-separated fields. `latest-desktop.json` lists the assets in the static updater format: `version`, `notes`, `pub_date`, and `platforms` keyed `linux-x86_64`, `darwin-aarch64` and `windows-x86_64`, each with `url` and `signature`.

Starting with 0.16.0, the consolidated desktop product is named `KubeCove` with the bundle identifier `com.timpan.kubecove`, matching the `com.timpan.kubecove` config directory (ADR 0030). Through 0.15.1, it shipped side by side as `KubeCove GPUI` (`com.timpan.kubecove.gpui`) on `latest-gpui.json`. The GPUI crate version follows the app version, because the client compares its own version with the manifest's.

A manual run of `release.yml` with the `gpui-dry-run` input builds, bundles, signs and verifies all three platforms and keeps the results as workflow artifacts. It creates no release, tag or draft.

### Trust

The client trusts one thing: the public key in `src-tauri/gpui/updater.json`, compiled into the binary. The manifest and the download are untrusted. Until the download has ended and passed these checks, its bytes go only to a staged file that is never executed, renamed or unpacked, and every failure removes that file:

1. The downloaded bytes must carry a valid signature from that key. Unhashed legacy signatures are refused, because the signing job uses prehashed minisign signatures.
2. The signature's trusted comment must hold `version:<version>` equal to the version the manifest announced. The trusted comment is covered by the signature. Without this check a tampered manifest could pair a new version number with an older signed build.
3. The trusted comment's `file:` field, which the signer sets to the signed file's name, must equal the exact expected consolidated asset name (`kubecove_<version>_...` for this platform). Legacy Tauri (`KubeCove_<version>_...`) and side-by-side GPUI (`kubecove-gpui_<version>_...`) artifacts were signed with the same key and are refused.

`src-tauri/gpui/updater.json` also supplies the compiled `latest-desktop.json` URL. The release verifier reads its public key from the same file.

### Install and relaunch

The client streams the artifact. Each chunk goes to the staged file and to minisign's streaming verifier, then is dropped, so memory use stays flat however large the response is. No trusted size exists to cap the download: the manifest and `Content-Length` are untrusted and the signature carries no size. File writes, hashing and the install run off the UI thread and off the async workers. A download that fails or ends before its announced length never installs.

- **Linux.** The staged file is a new hidden file beside the AppImage. After verification the payload must be an ELF file. The client copies the AppImage's permissions to the staged file, syncs it and renames it over the AppImage. The path always holds a complete AppImage. Relaunch starts the AppImage.
- **macOS.** The staged file is a hidden file beside the bundle that holds the downloaded `.app.tar.gz`, and it is removed once the install ends. After verification the client unpacks it into a hidden directory beside the bundle, requires a single `KubeCove.app` bundle declaring `CFBundleIdentifier` as `com.timpan.kubecove` (matching the installed bundle), moves the installed bundle aside, moves the new one into its place, and deletes the old one. If the second move fails it moves the installed bundle back. If that move fails too, the installed bundle stays under its hidden name beside the install path; it is deleted only after the new bundle is in place. Relaunch runs `open -n` on the bundle.
- **Windows.** The staged file is a hidden file in the user's temporary directory whose name does not end in `.exe`. After verification the payload must be a PE file, and the client renames it to `KubeCove-<version>-setup.exe`, so an unverified file never sits at an installer's name. Relaunch runs it with `/P` (passive) and `/R` (restart the app), and the app quits so the installer can replace its files.

Work files are created with `create_new`, so the client never writes through a link placed at a predictable path. A stale work file from an interrupted install is removed first, without following links.

Relaunch quits the app, so it obeys the close guard: it stays open while a YAML draft has unsaved changes.

The update states match the web: available, downloading with progress, installed and waiting for relaunch, and failed with the reason. Settings and the header popover show the same state. A relaunch that fails keeps the installed state and shows the reason beside Relaunch, so the user can try again without a new check and download. On Windows the saved installer stays in use.

The update check and the download fail when the server sends nothing for 30 seconds, while connecting or while reading. The limit covers silence, not the whole transfer, so a slow download that keeps arriving still completes. The owner chose the value on 2026-11-16 (#662).

### Differences from the Tauri updater

- On Windows the Tauri plugin starts the installer and exits the app as soon as the download finishes. The native client waits for Relaunch, so the user can finish work first.
- On macOS the Tauri plugin swaps bundles atomically where the file system allows and asks for administrator rights when the install location is not writable. The native client uses two renames and reports an error when the location is not writable. A crash between the two renames leaves the old bundle under a hidden name beside the install path.
- On Linux the client updates only an AppImage. A binary started outside an AppImage, such as a Nix or development build, reports that it cannot install updates.
- There are no proxy, header, timeout or downgrade settings. The Tauri updater sets no timeout by default; the native client always applies the 30-second stall limit.

## Transition after 0.15.1

Version 0.15.1 is the last release of Tauri KubeCove and side-by-side KubeCove GPUI. Both clients check `latest-desktop.json` for the consolidated KubeCove 0.16.0 or later. A compatible release offers a manual download from the project's GitHub Releases page. HTTP 404 or 204 falls back to the side-by-side feed. Its updater accepts only versions through 0.15.1 and requires the signed side-by-side artifact identity. Before a macOS swap, both the installed and downloaded bundles must be named `KubeCove GPUI.app` and declare `CFBundleIdentifier` as `com.timpan.kubecove.gpui`.

Legacy clients already request assets under `releases/latest/download/`. Every subsequent latest release must carry forward the original `latest.json` and `latest-gpui.json` from tag `app-v0.15.1`, including their version-specific download URLs and signatures. Only `latest-desktop.json` advances. Its installers use `kubecove_<version>_...` and its macOS archive holds `KubeCove.app`.

## Risks accepted

- The manifest is not signed. Whoever can replace release assets can withhold updates or announce a version with no valid build, which fails verification. They cannot make the client install a build the key did not sign as a native client build of that version.
- Before 0.15.1, signed file names identified the GPUI product but did not bind the platform. The transition client checks the complete signed platform file name before installation.
- The bundles carry no Apple or Windows code signature, as the Tauri artifacts do not. Operating system warnings on first launch are the same as for the Tauri app.
- A signed installer saved in the temporary directory can be replaced by another process of the same user before Relaunch runs it. That process already runs with the user's rights.

## Verification

- Native tests (`src-tauri/gpui/tests/native/updates.rs`) list the failure modes first and cover manifest reading, the signature, signed-version and signed-file-name checks with signatures made by the Tauri signer, a download from a local server through to the replaced AppImage, a download that stalls, and the bundle and installer paths. Library tests in `src-tauri/gpui/src/backend/updates/install.rs` inject a failure at the second bundle move and at the move back. The macOS and Windows file operations run on Linux in those tests; only the platform's own launch command is not exercised.
- Shared scenarios `settings-updates.*` cover the install, progress, relaunch and failure states in both clients with scripted results.
- The signing job generates a disposable key and verifies newly signed artifacts before using the release key. The release dry run proves that each platform builds, bundles and signs, and that each signature verifies against the public key and names the version.
- Not covered by any automated check: installing a real update on macOS and Windows, and the relaunch on all three platforms.
