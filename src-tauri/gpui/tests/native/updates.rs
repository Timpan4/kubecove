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
//! Download: a failed request; a body that is not the signed build.
//!
//! Install: a payload of the wrong kind for the platform; a missing install path; a failure
//! that leaves the installed version changed or leaves work files behind; an archive without
//! exactly one app bundle; lost executable permissions.
//!
//! The keys below are throwaway test keys made with `tauri signer generate`. The signatures
//! come from `tauri signer sign`, the tool the release uses, so the encoding checked here is
//! the one a release produces. The signer records the signed file's name, so each signature
//! was made over a copy of `PAYLOAD` named as a release asset is.
use kubecove_gpui::backend::updates::{
    self, Artifact, AvailableUpdate, DownloadProgress, Feed, InstallTarget, UpdateStatus,
};
use std::{
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
};

const PAYLOAD: &[u8] = b"\x7fELF KubeCove GPUI update test payload";
const VERSION: &str = "99.0.0";
const PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEU4NzNCNTAxQTkzMzE2NEYKUldSUEZqT3BBYlZ6Nk42RW1rS2tEekNobVdxZUgxVUV3NklnWVF6ZTkydnhuQWtVL3IyZHBzMzkK";
const OTHER_PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDdEQzI0MkVDNUFEN0ZCNUIKUldSYis5ZGE3RUxDZmRkR1oxblpNcDdYdGw1blpUVVF2N1FhYmROZk5CeUc5VEs5ZUY3OGRjemUK";
/// `kubecove-gpui_99.0.0_x86_64.AppImage` signed by `PUBLIC_KEY`'s key with
/// `--app-version 99.0.0`.
const SIGNATURE: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVSUEZqT3BBYlZ6NkF0YW5vcVhuQWo4N2VDekdiMFo0OXpzZW1mcWFFTDZlM244Y0hVUDRISTdyUVpqakNuUUV1QjlGakxsbmpHZXlZSHhzd20rL2dkOGZzMzgzRjFLUkE0PQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzk0MjYzMjgyCWZpbGU6a3ViZWNvdmUtZ3B1aV85OS4wLjBfeDg2XzY0LkFwcEltYWdlCXZlcnNpb246OTkuMC4wCklXZ2RqTGwzRUs1ejY3MVVBZ0RzT1VVc21zYTJmWTNCSm5lL2dOT05rWHljd01CVkdlcWxOZUwwcHhjZ1IweWg4NlFhQjEyci80MisrTkJRSHRid0FBPT0K";
/// The same file signed by the same key without `--app-version`.
const SIGNATURE_WITHOUT_VERSION: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVSUEZqT3BBYlZ6Nk5PM3U3NGpuTi8wMDZmMWY3Z0NiNFdaZW1nNmUxaEFxZVJlMCtmWmhlTStybEtwZ1YrNnZRNWp3RUxPL1FjSWw0Z0I0bWdpQlZTTFY0YVRoNkhFVmd3PQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzk0MjYzMjgyCWZpbGU6a3ViZWNvdmUtZ3B1aV85OS4wLjBfeDg2XzY0LkFwcEltYWdlCis1S2o2QjJlR0x1K2FQWmVGbk1CbmxSc3ZnVVJQZXJlSzhaelRoOEdpenZ2OXFKczZkYllpWXVVak5DaEd3ZnJkWVdOMVZCd2lNQVREUmo3dGVGb0NnPT0K";
/// The same file signed by `OTHER_PUBLIC_KEY`'s key with `--app-version 99.0.0`.
const OTHER_KEY_SIGNATURE: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVSYis5ZGE3RUxDZmM5YjZ2cStCdFFQYURvUldoRVFSQ25nTUtFZ21xb2JPbnRISndxMEJxN0hSYWdMbzM4cFVERUdqWTZGenZYVEYzSHRGaG12SGdwdm1Wc3g4WjFSb3dJPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzk0MjYzMjgyCWZpbGU6a3ViZWNvdmUtZ3B1aV85OS4wLjBfeDg2XzY0LkFwcEltYWdlCXZlcnNpb246OTkuMC4wClZXcGpRUEZVVitJeFZTUVF0TjBWOXY4MTdSWW1CZW9GRXByNDR6UVNzRS85bWR4SHA1N09SOUdnVFdrYnl0dTVaeVBSbUtEeXhXWjMyZmlUQ08vNkF3PT0K";
/// The same bytes named `KubeCove_99.0.0_amd64.AppImage`, as the Tauri app's asset is, signed
/// by `PUBLIC_KEY`'s key with `--app-version 99.0.0`.
const TAURI_APP_SIGNATURE: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVSUEZqT3BBYlZ6Nk9SekhGNEFVYW9aendTQ1pLeWFuWXZrSnBkVlBsbFBlem8zWWUzNnZEL2ZHd1JUNHJ1OUVZL3VMR0d6WkFWYngydzNON2tkVFB5cnBRdWRwbkNxT3dFPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzk0MjYzMjgyCWZpbGU6S3ViZUNvdmVfOTkuMC4wX2FtZDY0LkFwcEltYWdlCXZlcnNpb246OTkuMC4wCjNtTEFqaExMQTBoaVltY2ViL1ZsR2w4TUc0cm80ZnVPdU51dlFUSUxUeGEyUHZsVnRtMFcwNjhnSlB4dThDdkM1VVdLVzBXQkc1RFBuVDBaRllSNUJnPT0K";

/// A fresh worktree-local directory for one test.
fn directory(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../.t/update-tests")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
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
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .unwrap();
        stream.write_all(body).unwrap();
    });
    url
}

/// Runs the whole install against `url`, with the test key and an AppImage at `appimage`.
fn install_from(
    url: String,
    appimage: &Path,
) -> (
    Result<updates::Relaunch, kubecove_lib::models::AppError>,
    DownloadProgress,
) {
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
    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(updates::install(
            update,
            PUBLIC_KEY.into(),
            InstallTarget::AppImage(appimage.to_owned()),
            sender,
        ));
    let progress = *receiver.borrow();
    (result, progress)
}

fn installed_appimage(directory: &Path) -> PathBuf {
    let appimage = directory.join("KubeCove GPUI.AppImage");
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
    let build = serde_json::json!({"url": "https://example.test/build", "signature": "c2ln"});
    for version in [env!("CARGO_PKG_VERSION"), "0.0.1"] {
        let manifest =
            serde_json::json!({"version": version, "platforms": {"linux-x86_64": build}});
        assert!(matches!(
            updates::release(&manifest, "linux-x86_64").unwrap(),
            UpdateStatus::UpToDate
        ));
    }
    for manifest in [
        serde_json::json!({}),
        serde_json::json!({"version": "invalid"}),
        serde_json::json!({"version": "99.0.0"}),
        serde_json::json!({"version": "99.0.0", "platforms": {"darwin-aarch64": build}}),
        serde_json::json!({"version": "99.0.0", "platforms": {"linux-x86_64": {"url": "https://example.test/build"}}}),
        serde_json::json!({"version": "99.0.0", "platforms": {"linux-x86_64": {"url": "", "signature": "c2ln"}}}),
    ] {
        assert!(
            updates::release(&manifest, "linux-x86_64").is_err(),
            "offered {manifest}"
        );
    }
    let manifest = serde_json::json!({"version": "v99.0.0", "notes": " Fixes. ", "platforms": {"linux-x86_64": build}});
    let UpdateStatus::Available(update) = updates::release(&manifest, "linux-x86_64").unwrap()
    else {
        panic!("a newer release with a build was not offered");
    };
    assert_eq!(update.version, "99.0.0");
    assert_eq!(update.notes.as_deref(), Some("Fixes."));
    assert_eq!(update.artifact.url, "https://example.test/build");
    assert_eq!(update.artifact.signature, "c2ln");
}

#[test]
fn the_feed_is_the_gpui_manifest_beside_the_desktop_feed() {
    let feed = Feed::release().unwrap();
    assert!(feed.endpoint.starts_with("https://"));
    assert!(feed.endpoint.ends_with("/latest-gpui.json"));
    assert!(!feed.public_key.is_empty());
}

#[test]
fn only_the_release_key_and_the_announced_version_verify() {
    updates::verify(PAYLOAD, SIGNATURE, PUBLIC_KEY, VERSION).unwrap();
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
            "the Tauri app's build",
            PAYLOAD,
            TAURI_APP_SIGNATURE,
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
            updates::verify(bytes, signature, key, version).is_err(),
            "accepted {case}"
        );
    }
}

#[test]
fn a_verified_download_replaces_the_appimage_and_keeps_it_executable() {
    let directory = directory("install");
    let appimage = installed_appimage(&directory);
    let (result, progress) = install_from(serve("200 OK", PAYLOAD), &appimage);
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
    assert_eq!(names(&directory), ["KubeCove GPUI.AppImage"]);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&appimage).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o755);
    }
}

#[test]
fn a_tampered_or_failed_download_leaves_the_appimage_as_it_was() {
    let directory = directory("refuse");
    let appimage = installed_appimage(&directory);
    for (status, body) in [
        ("200 OK", &b"\x7fELF another build"[..]),
        ("404 Not Found", &b""[..]),
    ] {
        let (result, _) = install_from(serve(status, body), &appimage);
        assert!(result.is_err(), "installed after {status}");
        assert_eq!(
            std::fs::read(&appimage).unwrap(),
            b"\x7fELF installed version"
        );
        assert_eq!(names(&directory), ["KubeCove GPUI.AppImage"]);
    }
}

#[test]
fn an_appimage_install_refuses_other_payloads_and_a_missing_path() {
    let directory = directory("appimage");
    let appimage = installed_appimage(&directory);
    let target = InstallTarget::AppImage(appimage.clone());
    assert!(target.install(b"MZ a Windows installer", VERSION).is_err());
    assert_eq!(
        std::fs::read(&appimage).unwrap(),
        b"\x7fELF installed version"
    );
    let missing = InstallTarget::AppImage(directory.join("missing.AppImage"));
    assert!(missing.install(PAYLOAD, VERSION).is_err());
    assert_eq!(names(&directory), ["KubeCove GPUI.AppImage"]);
}

#[test]
fn a_bundle_install_swaps_the_app_and_refuses_an_archive_without_one() {
    let directory = directory("bundle");
    let bundle = directory.join("KubeCove GPUI.app");
    let binary = bundle.join("Contents/MacOS/kubecove-gpui");
    std::fs::create_dir_all(binary.parent().unwrap()).unwrap();
    std::fs::write(&binary, b"installed version").unwrap();
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
            "two bundles",
            archive(&[
                ("One.app/Contents/MacOS/one", b"one"),
                ("Two.app/Contents/MacOS/two", b"two"),
            ]),
        ),
        ("not an archive", b"not gzip".to_vec()),
    ] {
        assert!(
            target.install(&payload, VERSION).is_err(),
            "installed {case}"
        );
        assert_eq!(std::fs::read(&binary).unwrap(), b"installed version");
        assert_eq!(names(&directory), ["KubeCove GPUI.app"]);
    }

    let update = archive(&[(
        "KubeCove GPUI.app/Contents/MacOS/kubecove-gpui",
        b"new version",
    )]);
    let relaunch = target.install(&update, VERSION).unwrap();
    assert_eq!(std::fs::read(&binary).unwrap(), b"new version");
    assert!(!bundle.join("Contents/removed-in-update").exists());
    assert_eq!(names(&directory), ["KubeCove GPUI.app"]);
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
    let target = InstallTarget::Installer(directory.clone());
    assert!(target.install(PAYLOAD, VERSION).is_err());
    assert!(names(&directory).is_empty());

    let installer = directory.join("KubeCove-GPUI-99.0.0-setup.exe");
    // An interrupted earlier install left a file at the same path.
    std::fs::write(&installer, b"stale").unwrap();
    let relaunch = target.install(b"MZ new installer", VERSION).unwrap();
    assert_eq!(std::fs::read(&installer).unwrap(), b"MZ new installer");
    assert_eq!(relaunch.program, installer);
    assert_eq!(relaunch.args, ["/P", "/R"]);
}
