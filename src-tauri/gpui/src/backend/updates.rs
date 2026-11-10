//! Update check and installation for the native client (ADR 0034).
//!
//! Each release publishes one signed build per platform and `latest-gpui.json`, a manifest in
//! the Tauri updater's static format. The manifest and the download are untrusted. A build is
//! installed only when its bytes carry a signature from the key in `tauri.conf.json` and that
//! signature names the version the manifest announced.
use base64::Engine as _;
use kubecove_lib::models::{AppError, AppErrorKind};
use minisign_verify::{PublicKey, Signature, StreamVerifier};
use std::{ffi::OsString, io, path::PathBuf};
use tokio::sync::{mpsc, watch};

mod install;

pub use install::{InstallTarget, Staged, bundle_of};

#[derive(Clone, Debug, Default)]
pub enum UpdateStatus {
    #[default]
    Idle,
    UpToDate,
    Available(AvailableUpdate),
}

/// A newer release from the update manifest. `notes` is the manifest's `notes`
/// field, the same text the web's updater plugin exposes as `body`.
#[derive(Clone, Debug, Default)]
pub struct AvailableUpdate {
    pub version: String,
    pub notes: Option<String>,
    pub artifact: Artifact,
}

/// This platform's build in the manifest: where it is and the signature its bytes must carry.
#[derive(Clone, Debug, Default)]
pub struct Artifact {
    pub url: String,
    /// The `.sig` file's contents: base64 of the minisign signature text.
    pub signature: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DownloadProgress {
    pub downloaded: u64,
    /// The response's content length. Without it the download has no percentage.
    pub total: Option<u64>,
}

impl DownloadProgress {
    /// The rounded percentage, as the web store computes it from the same two numbers.
    pub fn percent(self) -> Option<u64> {
        let total = self.total.filter(|total| *total > 0)?;
        Some(
            (self
                .downloaded
                .saturating_mul(100)
                .saturating_add(total / 2)
                / total)
                .min(100),
        )
    }
}

/// The command that starts the installed version. The app quits after starting it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Relaunch {
    pub program: PathBuf,
    pub args: Vec<OsString>,
}

impl Relaunch {
    pub fn start(&self) -> Result<(), AppError> {
        std::process::Command::new(&self.program)
            .args(&self.args)
            .spawn()
            .map(drop)
            .map_err(|error| failed("Could not relaunch KubeCove", &error))
    }
}

/// The release feed and the key its builds are signed with.
pub struct Feed {
    pub endpoint: String,
    pub public_key: String,
}

const MANIFEST: &str = "latest-gpui.json";
/// Every release asset of this client starts with this, and the signer records the asset's
/// file name in the signature.
const ARTIFACT_PREFIX: &str = "kubecove-gpui_";

impl Feed {
    /// The GPUI manifest sits beside the desktop app's `latest.json` on the same release, and
    /// both clients trust the same key, so neither value is maintained twice.
    pub fn release() -> Result<Self, AppError> {
        let invalid = || AppError::new("Invalid updater configuration", AppErrorKind::Validation);
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../../../tauri.conf.json"))
                .map_err(|_| invalid())?;
        let updater = &config["plugins"]["updater"];
        let desktop = updater["endpoints"][0].as_str().ok_or_else(invalid)?;
        let directory = desktop.strip_suffix("latest.json").ok_or_else(invalid)?;
        Ok(Self {
            endpoint: format!("{directory}{MANIFEST}"),
            public_key: updater["pubkey"].as_str().ok_or_else(invalid)?.to_owned(),
        })
    }
}

/// This build's key in the manifest's `platforms`, as the Tauri updater names it.
pub fn platform() -> String {
    let os = if cfg!(target_os = "macos") {
        "darwin"
    } else {
        std::env::consts::OS
    };
    format!("{os}-{}", std::env::consts::ARCH)
}

pub(super) async fn check() -> Result<UpdateStatus, AppError> {
    let response = reqwest::Client::new()
        .get(Feed::release()?.endpoint)
        .send()
        .await
        .map_err(|_| AppError::new("Could not check for updates", AppErrorKind::Network))?;
    if response.status() == reqwest::StatusCode::NO_CONTENT {
        return Ok(UpdateStatus::UpToDate);
    }
    let manifest: serde_json::Value = response
        .error_for_status()
        .map_err(|_| AppError::new("Update check failed", AppErrorKind::Network))?
        .json()
        .await
        .map_err(|_| AppError::new("Invalid update manifest", AppErrorKind::Validation))?;
    release(&manifest, &platform())
}

/// Reads the manifest. A newer version is offered only with a build for `platform`.
pub fn release(manifest: &serde_json::Value, platform: &str) -> Result<UpdateStatus, AppError> {
    let invalid = |message: &str| AppError::new(message, AppErrorKind::Validation);
    let version = manifest["version"]
        .as_str()
        .ok_or_else(|| invalid("Missing update version"))?;
    let version = semver::Version::parse(version.trim_start_matches('v'))
        .map_err(|_| invalid("Invalid update version"))?;
    let current =
        semver::Version::parse(env!("CARGO_PKG_VERSION")).expect("Cargo package version is semver");
    if version <= current {
        return Ok(UpdateStatus::UpToDate);
    }
    let build = &manifest["platforms"][platform];
    let field = |name: &str| {
        build[name]
            .as_str()
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| {
                AppError::new(
                    format!("KubeCove {version} has no build for {platform}"),
                    AppErrorKind::Validation,
                )
            })
    };
    Ok(UpdateStatus::Available(AvailableUpdate {
        version: version.to_string(),
        notes: manifest["notes"]
            .as_str()
            .map(str::trim)
            .filter(|notes| !notes.is_empty())
            .map(str::to_owned),
        artifact: Artifact {
            url: field("url")?,
            signature: field("signature")?,
        },
    }))
}

/// Accepts `bytes` only when `signature` is a signature of them by `public_key` and it was made
/// for `version`. It runs the same check `install` runs over a download as it arrives.
pub fn verify(
    bytes: &[u8],
    signature: &str,
    public_key: &str,
    version: &str,
) -> Result<(), AppError> {
    let signed = Signed::decode(signature, public_key)?;
    let mut verifier = signed.stream()?;
    verifier.update(bytes);
    signed.check(verifier, version)
}

/// The release key and a build's signature, decoded.
struct Signed {
    public_key: PublicKey,
    signature: Signature,
}

fn invalid_signature() -> AppError {
    AppError::new("Invalid update signature", AppErrorKind::Validation)
}

impl Signed {
    fn decode(signature: &str, public_key: &str) -> Result<Self, AppError> {
        let text = |encoded: &str| {
            base64::engine::general_purpose::STANDARD
                .decode(encoded.trim())
                .ok()
                .and_then(|decoded| String::from_utf8(decoded).ok())
                .ok_or_else(invalid_signature)
        };
        Ok(Self {
            public_key: PublicKey::decode(&text(public_key)?).map_err(|_| invalid_signature())?,
            signature: Signature::decode(&text(signature)?).map_err(|_| invalid_signature())?,
        })
    }

    /// Hashes the build as it arrives. The stream form accepts only prehashed signatures, so
    /// the legacy unhashed form is refused; the Tauri signer always prehashes.
    fn stream(&self) -> Result<StreamVerifier<'_>, AppError> {
        self.public_key
            .verify_stream(&self.signature)
            .map_err(|_| invalid_signature())
    }

    /// Accepts the hashed build only when the signature holds and was made for `version`. The
    /// manifest is not signed, so its version alone does not prove which release the URL and
    /// signature point at: without the second check a tampered manifest could pair a new
    /// version number with an older signed build.
    fn check(&self, mut verifier: StreamVerifier<'_>, version: &str) -> Result<(), AppError> {
        verifier.finalize().map_err(|_| invalid_signature())?;
        // The trusted comment is covered by the signature, so it is read only after `finalize`.
        // The signer writes it as tab-separated `key:value` fields.
        let comment = self.signature.trusted_comment();
        let signed = |key: &str| {
            comment
                .split('\t')
                .find_map(|field| field.strip_prefix(key))
        };
        if signed("version:") != Some(version) {
            return Err(AppError::new(
                format!("The update is not signed for version {version}"),
                AppErrorKind::Validation,
            ));
        }
        // The Tauri desktop app is signed with the same key and names its version too. The
        // signed file name tells the two products apart.
        if !signed("file:").is_some_and(|file| file.starts_with(ARTIFACT_PREFIX)) {
            return Err(AppError::new(
                "The update is not a KubeCove GPUI build",
                AppErrorKind::Validation,
            ));
        }
        Ok(())
    }
}

/// Downloads the update's build, verifies it and installs it. The download goes chunk by chunk
/// to a staged file and the signature check, so memory use does not grow with its size. The
/// staged file is put in place only after the signature checks out, and removed otherwise.
pub async fn install(
    update: AvailableUpdate,
    public_key: String,
    target: InstallTarget,
    progress: watch::Sender<DownloadProgress>,
) -> Result<Relaunch, AppError> {
    let signed = Signed::decode(&update.artifact.signature, &public_key)?;
    // 1 is the least capacity tokio allows, so at most one chunk waits between the two sides.
    let (sender, chunks) = mpsc::channel(1);
    let version = update.version;
    // Writing and hashing the build block, so they run off the async workers.
    let writer = tokio::task::spawn_blocking(move || receive(&target, &signed, &version, chunks));
    let downloaded = download(&update.artifact.url, &progress, &sender).await;
    // A download that stopped before `End` leaves the writer waiting until the sender is gone.
    drop(sender);
    let installed = writer
        .await
        .map_err(|_| AppError::new("Update installation stopped", AppErrorKind::Internal))?;
    downloaded?;
    installed
}

/// What the download sends the writer. Only `End` completes a download: a sender dropped before
/// it means the download failed or was abandoned.
enum Piece {
    Chunk(Vec<u8>),
    End,
}

/// Writes each chunk to the staged file and the signature check, and installs the file once the
/// download has ended and the signature holds. Every early return drops the staged file, which
/// removes it.
fn receive(
    target: &InstallTarget,
    signed: &Signed,
    version: &str,
    mut chunks: mpsc::Receiver<Piece>,
) -> Result<Relaunch, AppError> {
    let mut verifier = signed.stream()?;
    let mut staged = target.stage()?;
    loop {
        match chunks.blocking_recv() {
            Some(Piece::Chunk(chunk)) => {
                verifier.update(&chunk);
                staged.write(&chunk)?;
            }
            Some(Piece::End) => break,
            None => return Err(download_failed()),
        }
    }
    signed.check(verifier, version)?;
    staged.install(version)
}

fn download_failed() -> AppError {
    AppError::new("Could not download the update", AppErrorKind::Network)
}

/// Sends the build to `writer` as it arrives, then `End`. It stops early without an error when
/// the writer has stopped, because the writer's own result says why.
async fn download(
    url: &str,
    progress: &watch::Sender<DownloadProgress>,
    writer: &mpsc::Sender<Piece>,
) -> Result<(), AppError> {
    let mut response = reqwest::Client::new()
        .get(url)
        .header(reqwest::header::ACCEPT, "application/octet-stream")
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|_| download_failed())?;
    let total = response.content_length();
    let mut downloaded = 0u64;
    progress.send_replace(DownloadProgress { downloaded, total });
    while let Some(chunk) = response.chunk().await.map_err(|_| download_failed())? {
        downloaded = downloaded.saturating_add(chunk.len() as u64);
        if writer.send(Piece::Chunk(chunk.into())).await.is_err() {
            return Ok(());
        }
        progress.send_replace(DownloadProgress { downloaded, total });
    }
    let _ = writer.send(Piece::End).await;
    Ok(())
}

fn failed(action: &str, error: &io::Error) -> AppError {
    AppError::new(format!("{action}: {error}"), AppErrorKind::Io)
}
