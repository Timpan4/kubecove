//! A feed can lag Tauri or omit our platform. The same signing key signs both clients.
//! Test the wire identity, payload type, archive boundary and no-clobber publication.
use super::{feed::*, install::*};
use base64::Engine as _;
use std::{fs, io::Write, path::Path};

const PAYLOAD: &[u8] = b"\x7fELF KubeCove GPUI update test payload";
const VERSION: &str = "99.0.0";
const PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEU4NzNCNTAxQTkzMzE2NEYKUldSUEZqT3BBYlZ6Nk42RW1rS2tEekNobVdxZUgxVUV3NklnWVF6ZTkydnhuQWtVL3IyZHBzMzkK";
const SIGNATURE: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVSUEZqT3BBYlZ6NkF0YW5vcVhuQWo4N2VDekdiMFo0OXpzZW1mcWFFTDZlM244Y0hVUDRISTdyUVpqakNuUUV1QjlGakxsbmpHZXlZSHhzd20rL2dkOGZzMzgzRjFLUkE0PQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzk0MjYzMjgyCWZpbGU6a3ViZWNvdmUtZ3B1aV85OS4wLjBfeDg2XzY0LkFwcEltYWdlCXZlcnNpb246OTkuMC4wCklXZ2RqTGwzRUs1ejY3MVVBZ0RzT1VVc21zYTJmWTNCSm5lL2dOT05rWHljd01CVkdlcWxOZUwwcHhjZ1IweWg4NlFhQjEyci80MisrTkJRSHRid0FBPT0K";
const TAURI_APP_SIGNATURE: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVSUEZqT3BBYlZ6Nk9SekhGNEFVYW9aendTQ1pLeWFuWXZrSnBkVlBsbFBlem8zWWUzNnZEL2ZHd1JUNHJ1OUVZL3VMR0d6WkFWYngydzNON2tkVFB5cnBRdWRwbkNxT3dFPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzk0MjYzMjgyCWZpbGU6S3ViZUNvdmVfOTkuMC4wX2FtZDY0LkFwcEltYWdlCXZlcnNpb246OTkuMC4wCjNtTEFqaExMQTBoaVltY2ViL1ZsR2w4TUc0cm80ZnVPdU51dlFUSUxUeGEyUHZsVnRtMFcwNjhnSlB4dThDdkM1VVdLVzBXQkc1RFBuVDBaRllSNUJnPT0K";

#[test]
fn rosetta_and_unsupported_platforms() {
    assert_eq!(
        Platform::resolve("macos", "x86_64", true).unwrap(),
        Platform::Mac
    );
    assert!(
        Platform::resolve("macos", "x86_64", false)
            .unwrap_err()
            .contains("0.14.1")
    );
    assert!(Platform::resolve("linux", "aarch64", false).is_err());
}

#[test]
fn installation_lock_protects_stale_cleanup_and_releases_after_failure() {
    let root = tempfile::tempdir().unwrap();
    let lock = lock_install(root.path()).unwrap();
    assert!(lock_install(root.path()).is_err());
    let stale = stage(root.path()).unwrap().keep().unwrap().1;
    fs::write(&stale, b"partial download").unwrap();
    let replacement = stage(root.path()).unwrap();
    assert_eq!(replacement.as_file().metadata().unwrap().len(), 0);
    drop(replacement);
    assert!(!stale.exists());
    #[cfg(unix)]
    {
        let tauri = root.path().join("Tauri.AppImage");
        fs::write(&tauri, b"Tauri").unwrap();
        std::os::unix::fs::symlink(&tauri, &stale).unwrap();
        assert!(stage(root.path()).is_err());
        assert_eq!(fs::read(tauri).unwrap(), b"Tauri");
    }
    drop(lock);
    assert!(lock_install(root.path()).is_ok());
}

#[test]
fn only_same_or_newer_releases_are_available() {
    let manifest = serde_json::json!({"version":"2.0.0","platforms":{"linux-x86_64":{"url":"https://example.org/download","signature":"sig"}}});
    for (current, available) in [("1.0.0", true), ("2.0.0", true), ("3.0.0", false)] {
        assert_eq!(
            release(manifest.clone(), Platform::Linux, current)
                .unwrap()
                .is_some(),
            available
        );
    }
}

#[test]
fn only_the_exact_signed_payload_version_and_platform_can_install() {
    let filename = Platform::Linux.asset(VERSION);
    let check = |bytes: &[u8], sig: &str, v: &str, name: &str| {
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(bytes).unwrap();
        verify(&mut file, sig, PUBLIC_KEY, v, name)
    };
    check(PAYLOAD, SIGNATURE, VERSION, &filename).unwrap();
    assert!(check(b"changed", SIGNATURE, VERSION, &filename).is_err());
    assert!(check(PAYLOAD, SIGNATURE, "100.0.0", &filename).is_err());
    assert!(check(PAYLOAD, TAURI_APP_SIGNATURE, VERSION, &filename).is_err());
    assert!(
        check(
            PAYLOAD,
            SIGNATURE,
            VERSION,
            &Platform::Windows.asset(VERSION)
        )
        .is_err()
    );
    assert!(check(PAYLOAD, "invalid", VERSION, &filename).is_err());
    // The legacy algorithm tag is "Ed" instead of "ED".
    let b64 = base64::engine::general_purpose::STANDARD;
    let decoded = String::from_utf8(b64.decode(SIGNATURE).unwrap()).unwrap();
    let mut lines = decoded.lines().map(str::to_owned).collect::<Vec<_>>();
    let mut packet = b64.decode(&lines[1]).unwrap();
    packet[..2].copy_from_slice(b"Ed");
    lines[1] = b64.encode(packet);
    assert!(check(PAYLOAD, &b64.encode(lines.join("\n")), VERSION, &filename).is_err());
}

fn bundle(entries: &[(&str, tar::EntryType)]) -> Vec<u8> {
    let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    for &(name, kind) in entries {
        let mut header = tar::Header::new_gnu();
        // Raw header names allow traversal inputs that the archive writer itself refuses.
        header.as_mut_bytes()[..name.len()].copy_from_slice(name.as_bytes());
        header.set_entry_type(kind);
        if kind.is_symlink() {
            header.set_link_name("../../outside").unwrap();
        }
        header.set_mode(0o755);
        header.set_size(0);
        header.set_cksum();
        archive.append(&header, &[][..]).unwrap();
    }
    archive.into_inner().unwrap().finish().unwrap()
}

#[test]
fn publishes_one_gpui_bundle_and_refuses_unsafe_archive_entries() {
    use tar::EntryType::{Regular, Symlink};
    let binary = "KubeCove GPUI.app/Contents/MacOS/kubecove-gpui";
    let root = tempfile::tempdir().unwrap();
    let tauri = root.path().join("KubeCove.app");
    fs::create_dir(&tauri).unwrap();
    for (bad, kind) in [
        ("Other.app/file", Regular),
        ("KubeCove GPUI.app/../outside", Regular),
        ("/outside", Regular),
        ("KubeCove GPUI.app/link", Symlink),
    ] {
        assert!(
            publish_bundle(&bundle(&[(binary, Regular), (bad, kind)])[..], root.path()).is_err()
        );
        assert!(!root.path().join(BUNDLE).exists());
    }
    let data = bundle(&[(binary, Regular)]);
    publish_bundle(&data[..], root.path()).unwrap();
    assert!(root.path().join(binary).is_file());
    assert!(publish_bundle(&data[..], root.path()).is_err());
    assert!(tauri.is_dir());
}

#[test]
fn executable_publication_requires_correct_magic_and_a_new_destination() {
    let root = tempfile::tempdir().unwrap();
    let existing = root.path().join("KubeCove.AppImage");
    fs::write(&existing, b"Tauri").unwrap();
    let install = |platform, payload: &[u8], target: &Path| {
        let mut staged = tempfile::NamedTempFile::new_in(root.path()).unwrap();
        staged.write_all(payload).unwrap();
        publish_executable(staged, platform, target)
    };
    for (platform, good, bad) in [
        (Platform::Linux, &b"\x7fELF"[..], &b"MZ"[..]),
        (Platform::Windows, &b"MZ"[..], &b"\x7fELF"[..]),
    ] {
        let path = root.path().join(platform.asset(VERSION));
        assert!(install(platform, bad, &path).is_err());
        assert!(!path.exists());
        assert!(install(platform, good, &existing).is_err());
        install(platform, good, &path).unwrap();
        assert_eq!(fs::read(path).unwrap(), good);
    }
    assert_eq!(fs::read(existing).unwrap(), b"Tauri");
}

#[tokio::test]
async fn download_publishes_only_a_complete_verified_appimage_beside_tauri() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let root = tempfile::tempdir().unwrap();
    let cache = root.path().join("cache");
    let apps = root.path().join("apps");
    fs::create_dir(&apps).unwrap();
    let tauri = apps.join("KubeCove.AppImage");
    fs::write(&tauri, b"Tauri").unwrap();
    let manifest = serde_json::json!({"version":VERSION, "platforms":{"linux-x86_64":{
        "signature":SIGNATURE, "url":"https://example.org/asset"
    }}});
    let _ = rustls::crypto::ring::default_provider().install_default();
    for length in [PAYLOAD.len() + 1, PAYLOAD.len()] {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let mut build = release(manifest.clone(), Platform::Linux, VERSION)
            .unwrap()
            .unwrap();
        build.artifact.url = format!("http://{}/asset", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 1024];
            assert!(stream.read(&mut request).await.unwrap() > 0);
            let headers =
                format!("HTTP/1.1 200 OK\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n");
            stream.write_all(headers.as_bytes()).await.unwrap();
            stream.write_all(PAYLOAD).await.unwrap();
            stream.shutdown().await.unwrap();
        });
        let result = super::install_release(
            build,
            PUBLIC_KEY.to_owned(),
            &apps,
            &cache,
            reqwest::Client::new(),
        )
        .await;
        server.await.unwrap();
        let appimage = apps.join(Platform::Linux.asset(VERSION));
        if length == PAYLOAD.len() {
            assert_eq!(result.unwrap().path, appimage.canonicalize().unwrap());
            assert_eq!(fs::read(&appimage).unwrap(), PAYLOAD);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_ne!(
                    fs::metadata(appimage).unwrap().permissions().mode() & 0o100,
                    0
                );
            }
        } else {
            assert!(result.is_err());
            assert!(!appimage.exists());
        }
        assert_eq!(fs::read(&tauri).unwrap(), b"Tauri");
        assert!(!cache.join(".kubecove-gpui-bridge.download").exists());
    }
}
