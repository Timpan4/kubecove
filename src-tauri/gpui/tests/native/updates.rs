//! Update installation is a trust boundary (ADR 0034). Failure modes, written before the code:
//!
//! Manifest: no version or an invalid one; a version that is not newer; a newer version with
//! no build, URL or signature for this platform.
//!
//! Signature: bytes changed after signing; a signature from another key; a signature with no
//! signed version; a signature for another version than the manifest announces; a signature
//! the same key made for the Tauri desktop app's build of that version; text that is not a
//! signature or a key.
//!
//! Download: a failed request; a body that is not the signed build; a body that ends before its
//! announced length, even when the bytes that arrived are the signed build; a refused, failed or
//! cut-short download that leaves its staged file behind, for any install target; a server that
//! sends part of the build and then stalls, which must end the install instead of waiting
//! forever. That test waits the full `STALL_TIMEOUT`.
//!
//! Install: a payload of the wrong kind for the platform; a missing install path; a failure
//! that leaves the installed version changed or leaves work files behind; an archive without
//! exactly one app bundle; lost executable permissions; an installer left beside the verified
//! one under its staged name.
//!
//! The throwaway keys and signatures below come from `tauri signer generate` and `tauri signer sign`.
//! These fixtures check legacy Tauri signer compatibility alongside the standalone minisign release
//! checks. The signer records the file name, so each signature covers a copy of `PAYLOAD` named
//! like a release asset.
use crate::scratch::Scratch;
use kubecove_gpui::backend::updates::{
    self, Artifact, AvailableUpdate, DownloadProgress, Feed, InstallTarget, Relaunch, UpdateStatus,
};
use kubecove_lib::models::AppError;
use std::{
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
};

const PAYLOAD: &[u8] = b"\x7fELF KubeCove update test payload";
const VERSION: &str = "99.0.0";
const PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEEzMUMxMTIxRTFFOTY2OUMKUldTakhCRWg0ZWxtbktPMGZRNDVkanBLdkZiQm1oUWpiRU9WRlRTbmlvK0luNDBwRUl5dC92UjQK";
const OTHER_PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDQ2QkI5ODI5OUI4OUE5MDYKUldSR3U1Z3BtNG1wQm9EeEMzeTBZa1N4bk9JcG1CRG94aWUxR1dJUm5aRkV4QTdKblhndmFDQ0kK";
/// `kubecove_99.0.0_x86_64.AppImage` signed by `PUBLIC_KEY`'s key with `version:99.0.0`.
const SIGNATURE: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IGRpc3Bvc2FibGUgdGVzdCBzaWduYXR1cmUKUlVTakhCRWg0ZWxtbkhRSFBZeEIxN2lCQXJjaERaNitqeS9iSUxQY1k4NlBSUytXUS9zanY5L0puYlJHQU0zVDVUbmdtMmMvSHBvaUduVE1kUk1sZU05b3drb1Rrclk4blFZPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzk0MjYzMjgyCWZpbGU6a3ViZWNvdmVfOTkuMC4wX3g4Nl82NC5BcHBJbWFnZQl2ZXJzaW9uOjk5LjAuMAoxNkk4a2UwbytVWjNSa3AvTldCbG5EbndRa21ybWNsa2VIeFQ5UWJDUy9SMnRmWHpDSFE2STlHVlBxRHpqQ1Aycyt2VzVtRTNSeVFpRVJsaFVvUE9Edz09Cg==";
/// The same file signed by the same key without a `version:` field.
const SIGNATURE_WITHOUT_VERSION: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IGRpc3Bvc2FibGUgdGVzdCBzaWduYXR1cmUKUlVTakhCRWg0ZWxtbkhRSFBZeEIxN2lCQXJjaERaNitqeS9iSUxQY1k4NlBSUytXUS9zanY5L0puYlJHQU0zVDVUbmdtMmMvSHBvaUduVE1kUk1sZU05b3drb1Rrclk4blFZPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzk0MjYzMjgyCWZpbGU6a3ViZWNvdmVfOTkuMC4wX3g4Nl82NC5BcHBJbWFnZQpOakJHWi8yK0ZiUHVjVXF6VGpYQ3Y3UE1VSFYvQkFRRmltQ1MwMm1JTTZzNnBlUmdvMVdDemJhT2hFU1lSVmNLS2RWN2doSitlbkdndkVsYzdMY0RDdz09Cg==";
/// The same file signed by `OTHER_PUBLIC_KEY`'s key with `version:99.0.0`.
const OTHER_KEY_SIGNATURE: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IGRpc3Bvc2FibGUgdGVzdCBzaWduYXR1cmUKUlVSR3U1Z3BtNG1wQnNMOVhQTm9oQkticWE2SGJ5SlRZOG9VM1FDRGtrcExGQjJkQnloZFRiYW9FM2FnblBtMmU2NmozUmgrV2YreHQxV1ArelZkWGsvSXV2TmcrUXN5WWdvPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzk0MjYzMjgyCWZpbGU6a3ViZWNvdmVfOTkuMC4wX3g4Nl82NC5BcHBJbWFnZQl2ZXJzaW9uOjk5LjAuMApJWUVTbTI3L2pTY2Y0Q2prbGZFemZES1I3d29NczBKM0RYcFpZYlk5dC95WmJFU21xT1g3U1BSdW90WVkwTUIxaUVEMEt6SDZocFJrSGs4SmpvNEFEZz09Cg==";
/// The same bytes named `KubeCove_99.0.0_amd64.AppImage`, as the legacy Tauri app's asset was,
/// signed by `PUBLIC_KEY`'s key with `version:99.0.0`.
const TAURI_APP_SIGNATURE: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IGRpc3Bvc2FibGUgdGVzdCBzaWduYXR1cmUKUlVTakhCRWg0ZWxtbkhRSFBZeEIxN2lCQXJjaERaNitqeS9iSUxQY1k4NlBSUytXUS9zanY5L0puYlJHQU0zVDVUbmdtMmMvSHBvaUduVE1kUk1sZU05b3drb1Rrclk4blFZPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzk0MjYzMjgyCWZpbGU6S3ViZUNvdmVfOTkuMC4wX2FtZDY0LkFwcEltYWdlCXZlcnNpb246OTkuMC4wCjBhRlg5d3RzelF5dWV6ZVA1eTBSa3M5aTZiWFN5ZFdaaHp6bFlxYjJZeFd1T25UbHR0Zk9oOTZQdTExU0ljb0Ztc3VLbWNxV1E2Tk5LODBycUZqL0NBPT0K";
/// The same bytes named `kubecove-gpui_99.0.0_x86_64.AppImage`, as the legacy side-by-side GPUI
/// build was, signed by `PUBLIC_KEY`'s key with `version:99.0.0`.
const SIDE_BY_SIDE_SIGNATURE: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IGRpc3Bvc2FibGUgdGVzdCBzaWduYXR1cmUKUlVTakhCRWg0ZWxtbkhRSFBZeEIxN2lCQXJjaERaNitqeS9iSUxQY1k4NlBSUytXUS9zanY5L0puYlJHQU0zVDVUbmdtMmMvSHBvaUduVE1kUk1sZU05b3drb1Rrclk4blFZPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzk0MjYzMjgyCWZpbGU6a3ViZWNvdmUtZ3B1aV85OS4wLjBfeDg2XzY0LkFwcEltYWdlCXZlcnNpb246OTkuMC4wCmNNN0V5TEJTWU9uejVSYlY0bSs1bFF4dW5OR09mUEFVVlV4akc2cXJHbUNHSzlkUFZOMDNLSzJsbE1HSGNxNGgydnQrc29US1ZMZU4xaHl6MFJzU0FBPT0K";

/// A fresh worktree-local directory for one test, removed when the test ends.
fn directory(name: &str) -> Scratch {
    Scratch::new("update-tests", name)
}

fn names(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

/// Answers one request on a local port with `status` and `body`.
fn serve(status: &'static str, body: &'static [u8]) -> String {
    serve_announcing(status, body, body.len())
}

/// Answers one request with `status`, a `Content-Length` of `length` and `body`, then closes.
fn serve_announcing(status: &'static str, body: &'static [u8], length: usize) -> String {
    respond(status, body, length, false)
}

/// Answers one request with `body` and a `Content-Length` of `length`, then keeps the
/// connection open and sends nothing more until the client closes it.
fn serve_then_stall(body: &'static [u8], length: usize) -> String {
    respond("200 OK", body, length, true)
}

fn respond(status: &'static str, body: &'static [u8], length: usize, stall: bool) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/build", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut chunk = [0u8; 1024];
        while !request.ends_with(b"\r\n\r\n") {
            let read = stream.read(&mut chunk).unwrap();
            if read == 0 {
                return;
            }
            request.extend_from_slice(&chunk[..read]);
        }
        write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        stream.write_all(body).unwrap();
        // The client sends nothing more, so a read returns only when it closes the connection.
        while stall && stream.read(&mut chunk).is_ok_and(|read| read > 0) {}
    });
    url
}

/// Runs the whole install against `url`, with the test key, into `target`.
fn install_from(
    url: String,
    target: InstallTarget,
) -> (Result<Relaunch, AppError>, DownloadProgress) {
    // reqwest is built without a TLS provider; `main` installs this one in the app.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let (sender, receiver) = tokio::sync::watch::channel(DownloadProgress::default());
    let update = AvailableUpdate {
        version: VERSION.into(),
        notes: None,
        artifact: Artifact {
            url,
            signature: SIGNATURE.into(),
        },
    };
    // An outer deadline, so a lost stall timeout fails the stall test instead of hanging it.
    // The owner chose twice the stall timeout (#718).
    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async {
            tokio::time::timeout(
                2 * updates::STALL_TIMEOUT,
                updates::install(update, PUBLIC_KEY.into(), target, sender),
            )
            .await
        })
        .expect("the install outlived twice the stall timeout");
    let progress = *receiver.borrow();
    (result, progress)
}

/// Stages `payload` for `target` and installs it. The update flow verifies the signature
/// between the two steps; the tests that call this cover the file handling alone.
fn install_unsigned(target: &InstallTarget, payload: &[u8]) -> Result<Relaunch, AppError> {
    let mut staged = target.stage()?;
    staged.write(payload)?;
    staged.install(VERSION)
}

fn installed_appimage(directory: &Path) -> PathBuf {
    let appimage = directory.join("KubeCove.AppImage");
    std::fs::write(&appimage, b"\x7fELF installed version").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&appimage, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    appimage
}

fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    for (path, contents) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o755);
        builder.append_data(&mut header, path, *contents).unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap()
}

#[test]
fn a_release_is_offered_only_when_newer_and_built_for_this_platform() {
    let url = "https://example.test/kubecove_0.16.0_x86_64.AppImage";
    let build = serde_json::json!({"url": url, "signature": "c2ln"});
    for version in [env!("CARGO_PKG_VERSION"), "0.0.1"] {
        let manifest =
            serde_json::json!({"version": version, "platforms": {"linux-x86_64": build}});
        assert!(matches!(
            updates::release(&manifest, "linux-x86_64").unwrap(),
            UpdateStatus::UpToDate
        ));
    }
    let check = |manifest: &serde_json::Value| {
        updates::release_for_version(manifest, "linux-x86_64", &semver::Version::new(0, 15, 1))
    };
    for manifest in [
        serde_json::json!({}),
        serde_json::json!({"version": "invalid"}),
        serde_json::json!({"version": "0.16.0"}),
        serde_json::json!({"version": "0.16.0", "platforms": {"darwin-aarch64": build}}),
        serde_json::json!({"version": "0.16.0", "platforms": {"linux-x86_64": {"url": url}}}),
        serde_json::json!({"version": "0.16.0", "platforms": {"linux-x86_64": {"url": "", "signature": "c2ln"}}}),
        serde_json::json!({"version": "0.16.0", "bundleId":"com.timpan.kubecove.gpui", "platforms":{"linux-x86_64":build}}),
        serde_json::json!({"version": "0.16.0", "platforms":{"linux-x86_64":{"url":"https://example.test/kubecove-gpui_0.16.0_x86_64.AppImage","signature":"c2ln"}}}),
    ] {
        assert!(check(&manifest).is_err(), "offered {manifest}");
    }
    let legacy = serde_json::json!({
        "version": "0.15.1",
        "platforms": {"linux-x86_64": {"url": "https://example.test/kubecove_0.15.1_x86_64.AppImage", "signature": "c2ln"}}
    });
    assert!(
        updates::release_for_version(&legacy, "linux-x86_64", &semver::Version::new(0, 15, 0))
            .is_err()
    );
    let manifest = serde_json::json!({"version": "v0.16.0", "notes": " Fixes. ", "platforms": {"linux-x86_64": build}});
    let UpdateStatus::Available(update) = check(&manifest).unwrap() else {
        panic!("a newer release with a build was not offered");
    };
    assert_eq!(update.version, "0.16.0");
    assert_eq!(update.notes.as_deref(), Some("Fixes."));
    assert_eq!(update.artifact.url, url);
    assert_eq!(update.artifact.signature, "c2ln");
}

#[test]
fn the_release_feed_points_to_the_published_desktop_manifest() {
    let feed = Feed::release().unwrap();
    assert_eq!(
        feed.endpoint,
        "https://github.com/Timpan4/kubecove/releases/latest/download/latest-desktop.json"
    );
    assert!(!feed.public_key.is_empty());
}

#[test]
fn only_the_release_key_and_the_announced_version_verify() {
    updates::verify(PAYLOAD, SIGNATURE, PUBLIC_KEY, VERSION, "linux-x86_64").unwrap();
    assert!(updates::verify(PAYLOAD, SIGNATURE, PUBLIC_KEY, VERSION, "darwin-aarch64").is_err());
    let mut changed = PAYLOAD.to_vec();
    changed[10] ^= 1;
    for (case, bytes, signature, key, version) in [
        (
            "changed bytes",
            changed.as_slice(),
            SIGNATURE,
            PUBLIC_KEY,
            VERSION,
        ),
        ("another key", PAYLOAD, SIGNATURE, OTHER_PUBLIC_KEY, VERSION),
        (
            "another key's signature",
            PAYLOAD,
            OTHER_KEY_SIGNATURE,
            PUBLIC_KEY,
            VERSION,
        ),
        (
            "no signed version",
            PAYLOAD,
            SIGNATURE_WITHOUT_VERSION,
            PUBLIC_KEY,
            VERSION,
        ),
        ("another version", PAYLOAD, SIGNATURE, PUBLIC_KEY, "99.0.1"),
        (
            "the legacy Tauri app's build",
            PAYLOAD,
            TAURI_APP_SIGNATURE,
            PUBLIC_KEY,
            VERSION,
        ),
        (
            "the legacy side-by-side GPUI build",
            PAYLOAD,
            SIDE_BY_SIDE_SIGNATURE,
            PUBLIC_KEY,
            VERSION,
        ),
        ("an empty signature", PAYLOAD, "", PUBLIC_KEY, VERSION),
        (
            "text that is not base64",
            PAYLOAD,
            "not a signature",
            PUBLIC_KEY,
            VERSION,
        ),
        (
            "base64 that is not a signature",
            PAYLOAD,
            "c2lnbmF0dXJl",
            PUBLIC_KEY,
            VERSION,
        ),
        (
            "a key that is not a key",
            PAYLOAD,
            SIGNATURE,
            "c2lnbmF0dXJl",
            VERSION,
        ),
    ] {
        assert!(
            updates::verify(bytes, signature, key, version, "linux-x86_64").is_err(),
            "accepted {case}"
        );
    }
}

#[test]
fn a_verified_download_replaces_the_appimage_and_keeps_it_executable() {
    let directory = directory("install");
    let appimage = installed_appimage(&directory);
    let (result, progress) = install_from(
        serve("200 OK", PAYLOAD),
        InstallTarget::AppImage(appimage.clone()),
    );
    let relaunch = result.unwrap();
    assert_eq!(std::fs::read(&appimage).unwrap(), PAYLOAD);
    assert_eq!(relaunch.program, appimage);
    assert!(relaunch.args.is_empty());
    let size = PAYLOAD.len() as u64;
    assert_eq!(
        progress,
        DownloadProgress {
            downloaded: size,
            total: Some(size)
        }
    );
    assert_eq!(names(&directory), ["KubeCove.AppImage"]);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&appimage).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o755);
    }
}

#[test]
fn a_tampered_failed_or_cut_short_download_changes_nothing_and_leaves_no_staged_file() {
    let directory = directory("refuse");
    let appimage = installed_appimage(&directory);
    let bundle = directory.join("KubeCove.app");
    let binary = bundle.join("Contents/MacOS/kubecove-gpui");
    std::fs::create_dir_all(binary.parent().unwrap()).unwrap();
    std::fs::write(&binary, b"installed version").unwrap();
    let installers = directory.join("installers");
    std::fs::create_dir(&installers).unwrap();
    for target in [
        InstallTarget::AppImage(appimage.clone()),
        InstallTarget::AppBundle(bundle),
        InstallTarget::Installer(installers.clone()),
    ] {
        for (case, url) in [
            (
                "a tampered build",
                serve("200 OK", b"\x7fELF another build"),
            ),
            ("a failed request", serve("404 Not Found", b"")),
            // Every byte of the signed build arrives, then the body ends one byte short of its
            // announced length. Only a download that ends as announced may install.
            (
                "a body cut short",
                serve_announcing("200 OK", PAYLOAD, PAYLOAD.len() + 1),
            ),
        ] {
            let (result, _) = install_from(url, target.clone());
            let error = result.expect_err(&format!("installed {case} into {target:?}"));
            if case == "a body cut short" {
                assert_eq!(error.message, "Could not download the update");
            }
            assert_eq!(
                std::fs::read(&appimage).unwrap(),
                b"\x7fELF installed version"
            );
            assert_eq!(std::fs::read(&binary).unwrap(), b"installed version");
            assert_eq!(
                names(&directory),
                ["KubeCove.AppImage", "KubeCove.app", "installers"],
                "{case} into {target:?} left a staged file"
            );
            assert!(
                names(&installers).is_empty(),
                "{case} left a staged installer"
            );
        }
    }
}

#[test]
fn a_download_that_stalls_fails_after_the_stall_timeout_and_changes_nothing() {
    let directory = directory("stall");
    let appimage = installed_appimage(&directory);
    let sent = PAYLOAD.len() / 2;
    let started = std::time::Instant::now();
    let (result, progress) = install_from(
        serve_then_stall(&PAYLOAD[..sent], PAYLOAD.len()),
        InstallTarget::AppImage(appimage.clone()),
    );
    assert!(result.is_err(), "a stalled download installed");
    assert!(started.elapsed() >= updates::STALL_TIMEOUT);
    // The first bytes arrived, so the request itself did not fail.
    assert_eq!(progress.downloaded, sent as u64);
    assert_eq!(
        std::fs::read(&appimage).unwrap(),
        b"\x7fELF installed version"
    );
    assert_eq!(names(&directory), ["KubeCove.AppImage"]);
}

#[test]
fn an_appimage_install_refuses_other_payloads_and_a_missing_path() {
    let directory = directory("appimage");
    let appimage = installed_appimage(&directory);
    let target = InstallTarget::AppImage(appimage.clone());
    assert!(install_unsigned(&target, b"MZ a Windows installer").is_err());
    assert_eq!(
        std::fs::read(&appimage).unwrap(),
        b"\x7fELF installed version"
    );
    let missing = InstallTarget::AppImage(directory.join("missing.AppImage"));
    assert!(install_unsigned(&missing, PAYLOAD).is_err());
    assert_eq!(names(&directory), ["KubeCove.AppImage"]);
}

#[test]
fn a_bundle_install_swaps_the_app_and_refuses_an_archive_without_one() {
    let directory = directory("bundle");
    let bundle = directory.join("KubeCove.app");
    let binary = bundle.join("Contents/MacOS/kubecove-gpui");
    std::fs::create_dir_all(binary.parent().unwrap()).unwrap();
    std::fs::write(&binary, b"installed version").unwrap();
    let info = include_bytes!("../../../test-fixtures/consolidated-info.plist");
    std::fs::write(bundle.join("Contents/Info.plist"), info).unwrap();
    std::fs::write(bundle.join("Contents/removed-in-update"), b"").unwrap();
    assert_eq!(updates::bundle_of(&binary), Some(bundle.clone()));
    assert_eq!(
        updates::bundle_of(Path::new("/usr/bin/kubecove-gpui")),
        None
    );

    let target = InstallTarget::AppBundle(bundle.clone());
    for (case, payload) in [
        ("no bundle", archive(&[("README", b"no app here")])),
        (
            "side-by-side bundle",
            archive(&[
                (
                    "KubeCove GPUI.app/Contents/MacOS/kubecove-gpui",
                    b"another product",
                ),
                ("KubeCove GPUI.app/Contents/Info.plist", info),
            ]),
        ),
        (
            "two bundles",
            archive(&[
                ("One.app/Contents/MacOS/one", b"one"),
                ("Two.app/Contents/MacOS/two", b"two"),
            ]),
        ),
        (
            "wrong identifier",
            archive(&[(
                "KubeCove.app/Contents/Info.plist",
                include_str!("../../../test-fixtures/consolidated-info.plist")
                    .replace("com.timpan.kubecove", "com.timpan.kubecove.gpui")
                    .as_bytes(),
            )]),
        ),
        ("not an archive", b"not gzip".to_vec()),
    ] {
        assert!(
            install_unsigned(&target, &payload).is_err(),
            "installed {case}"
        );
        assert_eq!(std::fs::read(&binary).unwrap(), b"installed version");
        assert_eq!(names(&directory), ["KubeCove.app"]);
    }

    let update = archive(&[
        ("KubeCove.app/Contents/MacOS/kubecove-gpui", b"new version"),
        ("KubeCove.app/Contents/Info.plist", info),
    ]);
    let relaunch = install_unsigned(&target, &update).unwrap();
    assert_eq!(std::fs::read(&binary).unwrap(), b"new version");
    assert!(!bundle.join("Contents/removed-in-update").exists());
    assert_eq!(names(&directory), ["KubeCove.app"]);
    assert_eq!(relaunch.program, Path::new("/usr/bin/open"));
    assert_eq!(relaunch.args, ["-n".into(), bundle.into_os_string()]);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&binary).unwrap().permissions().mode();
        assert_eq!(
            mode & 0o111,
            0o111,
            "the app binary lost its executable bits"
        );
    }
}

#[test]
fn a_windows_install_saves_the_installer_and_runs_it_passively_on_relaunch() {
    let directory = directory("installer");
    let target = InstallTarget::Installer(directory.to_path_buf());
    assert!(install_unsigned(&target, PAYLOAD).is_err());
    assert!(names(&directory).is_empty());

    let installer = directory.join("KubeCove-99.0.0-setup.exe");
    // An interrupted earlier install left a file at the same path.
    std::fs::write(&installer, b"stale").unwrap();
    let relaunch = install_unsigned(&target, b"MZ new installer").unwrap();
    assert_eq!(std::fs::read(&installer).unwrap(), b"MZ new installer");
    assert_eq!(names(&directory), ["KubeCove-99.0.0-setup.exe"]);
    assert_eq!(relaunch.program, installer);
    assert_eq!(relaunch.args, ["/P", "/R"]);
}
