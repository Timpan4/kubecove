use super::{invalid, network};
use crate::models::{AppError, GpuiInstallKind, GpuiUpgradeInfo};
use base64::Engine as _;
use minisign_verify::{PublicKey, Signature};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Seek},
    time::Duration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Platform {
    Linux,
    Mac,
    Windows,
}

impl Platform {
    pub(super) fn resolve(os: &str, arch: &str, translated: bool) -> Result<Self, String> {
        const INTEL: &str = "KubeCove GPUI requires Apple silicon. Version 0.14.1 was the last universal build for Intel Macs.";
        match (os, arch) {
            ("linux", "x86_64") => Ok(Self::Linux),
            ("windows", "x86_64") => Ok(Self::Windows),
            ("macos", "aarch64") => Ok(Self::Mac),
            ("macos", "x86_64") if translated => Ok(Self::Mac),
            ("macos", "x86_64") => Err(INTEL.into()),
            _ => Err(format!("KubeCove GPUI has no release for {os}-{arch}.")),
        }
    }

    pub(super) fn current() -> Result<Self, String> {
        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        let translated = std::process::Command::new("/usr/sbin/sysctl")
            .args(["-in", "sysctl.proc_translated"])
            .output()
            .is_ok_and(|out| out.status.success() && out.stdout.trim_ascii() == b"1");
        #[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
        let translated = false;
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        Self::resolve(os, arch, translated)
    }

    pub(super) fn key(self) -> &'static str {
        match self {
            Self::Linux => "linux-x86_64",
            Self::Windows => "windows-x86_64",
            Self::Mac => "darwin-aarch64",
        }
    }

    pub(super) fn kind(self) -> GpuiInstallKind {
        match self {
            Self::Linux => GpuiInstallKind::AppImage,
            Self::Windows => GpuiInstallKind::WindowsInstaller,
            Self::Mac => GpuiInstallKind::AppBundle,
        }
    }

    pub(super) fn asset(self, version: &str) -> String {
        let suffix = match self {
            Self::Linux => "x86_64.AppImage",
            Self::Mac => "aarch64.app.tar.gz",
            Self::Windows => "x64-setup.exe",
        };
        format!("kubecove-gpui_{version}_{suffix}")
    }
}

#[derive(Deserialize)]
struct Manifest {
    version: String,
    notes: Option<String>,
    pub_date: Option<String>,
    platforms: HashMap<String, Artifact>,
}

#[derive(Deserialize)]
pub(super) struct Artifact {
    pub(super) url: String,
    pub(super) signature: String,
}

pub(super) struct Release {
    pub(super) platform: Platform,
    pub(super) version: String,
    pub(super) artifact: Artifact,
    notes: Option<String>,
    published_at: Option<String>,
}

impl Release {
    pub(super) fn info(&self) -> GpuiUpgradeInfo {
        GpuiUpgradeInfo::Available {
            target: self.platform.key().into(),
            version: self.version.clone(),
            notes: self.notes.clone(),
            published_at: self.published_at.clone(),
            asset_name: self.platform.asset(&self.version),
            install_kind: self.platform.kind(),
        }
    }
}

pub(super) fn release(
    value: serde_json::Value,
    platform: Platform,
    current: &str,
) -> Result<Option<Release>, AppError> {
    let mut manifest: Manifest =
        serde_json::from_value(value).map_err(|_| invalid("Invalid GPUI release manifest"))?;
    let parse = |s: &str| {
        semver::Version::parse(s.strip_prefix('v').unwrap_or(s))
            .map_err(|_| invalid("Invalid GPUI release version"))
    };
    let version = parse(&manifest.version)?;
    if version.cmp_precedence(&parse(current)?).is_lt() {
        return Ok(None);
    }
    let Some(artifact) = manifest.platforms.remove(platform.key()) else {
        return Ok(None);
    };
    if artifact.signature.is_empty()
        || reqwest::Url::parse(&artifact.url).map_or(true, |url| url.scheme() != "https")
    {
        return Err(invalid("Invalid GPUI release asset"));
    }
    Ok(Some(Release {
        platform,
        version: version.to_string(),
        artifact,
        notes: manifest.notes,
        published_at: manifest.pub_date,
    }))
}

pub(super) struct Feed {
    endpoint: String,
    pub(super) public_key: String,
}

impl Feed {
    pub(super) fn configured() -> Result<Self, AppError> {
        let bad = || invalid("Invalid compiled updater configuration");
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../../../tauri.conf.json")).map_err(|_| bad())?;
        let updater = &config["plugins"]["updater"];
        let base = updater["endpoints"][0]
            .as_str()
            .and_then(|s| s.strip_suffix("latest.json"))
            .ok_or_else(bad)?;
        Ok(Self {
            endpoint: format!("{base}latest-gpui.json"),
            public_key: updater["pubkey"].as_str().ok_or_else(bad)?.into(),
        })
    }

    pub(super) async fn latest(&self, platform: Platform) -> Result<Option<Release>, AppError> {
        #[cfg(feature = "e2e")]
        if std::env::var("KUBECOVE_E2E_SMOKE").as_deref() == Ok("1") {
            // The isolated desktop smoke exercises real IPC and manifest selection offline.
            let manifest =
                serde_json::json!({"version": env!("CARGO_PKG_VERSION"), "platforms": {}});
            return release(manifest, platform, env!("CARGO_PKG_VERSION"));
        }
        let response = client()?
            .get(&self.endpoint)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|_| network("Could not check for a GPUI release"))?;
        if response.status() == reqwest::StatusCode::NO_CONTENT {
            return Ok(None);
        }
        let value = read_manifest(response).await?;
        release(value, platform, env!("CARGO_PKG_VERSION"))
    }
}

// Match the existing 2 MiB remote JSON budget in gitops/commands/argo/transport.rs.
pub(super) const MANIFEST_BYTES: usize = 2 * 1024 * 1024;

pub(super) async fn read_manifest(
    mut response: reqwest::Response,
) -> Result<serde_json::Value, AppError> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| network("Could not read the GPUI release manifest"))?
    {
        if chunk.len() > MANIFEST_BYTES - bytes.len() {
            return Err(invalid("The GPUI release manifest is too large"));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| invalid("Invalid GPUI release manifest"))
}

pub(super) fn client() -> Result<reqwest::Client, AppError> {
    // ADR 0034 limits a stall, not the duration of a progressing transfer.
    let stall = Duration::from_secs(30);
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::builder()
        .https_only(true)
        .connect_timeout(stall)
        .read_timeout(stall)
        .build()
        .map_err(|_| network("Could not initialize the release client"))
}

pub(super) fn verify(
    file: &mut File,
    sig: &str,
    key: &str,
    version: &str,
    name: &str,
) -> Result<ring::digest::Digest, AppError> {
    let bad = || invalid("Invalid GPUI update signature");
    let decode = |s: &str| {
        base64::engine::general_purpose::STANDARD
            .decode(s.trim())
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .ok_or_else(bad)
    };
    let key = PublicKey::decode(&decode(key)?).map_err(|_| bad())?;
    let sig = Signature::decode(&decode(sig)?).map_err(|_| bad())?;
    // The streaming verifier requires a prehashed signature.
    let mut verifier = key.verify_stream(&sig).map_err(|_| bad())?;
    let mut digest = ring::digest::Context::new(&ring::digest::SHA256);
    file.rewind().map_err(super::io_error)?;
    let mut reader = BufReader::new(file);
    loop {
        let bytes = reader.fill_buf().map_err(super::io_error)?;
        if bytes.is_empty() {
            break;
        }
        verifier.update(bytes);
        digest.update(bytes);
        let count = bytes.len();
        reader.consume(count);
    }
    verifier.finalize().map_err(|_| bad())?;
    for (prefix, expected) in [("version:", version), ("file:", name)] {
        if !sig
            .trusted_comment()
            .split('\t')
            .filter_map(|s| s.strip_prefix(prefix))
            .eq([expected])
        {
            return Err(invalid(
                "Signature does not match the requested GPUI version and asset",
            ));
        }
    }
    Ok(digest.finish())
}
