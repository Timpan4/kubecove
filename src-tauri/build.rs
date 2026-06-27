#![deny(clippy::all, clippy::pedantic, clippy::style)]
#![allow(clippy::missing_errors_doc)]

fn main() {
    ensure_sidecar_permissions();
    tauri_build::build();
}

#[cfg(unix)]
fn ensure_sidecar_permissions() {
    use std::{fs, os::unix::fs::PermissionsExt, path::Path};

    for name in [
        "bin/kubeconform-x86_64-unknown-linux-gnu",
        "bin/kubeconform-x86_64-apple-darwin",
        "bin/kubeconform-aarch64-apple-darwin",
    ] {
        let path = Path::new(name);
        if !path.exists() {
            continue;
        }
        let Ok(metadata) = fs::metadata(path) else {
            continue;
        };
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o755);
        let _ = fs::set_permissions(path, permissions);
    }
}

#[cfg(not(unix))]
fn ensure_sidecar_permissions() {}
