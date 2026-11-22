//! Update check and installation for the native client (ADR 0034).
//!
//! Each release publishes one signed build per platform and `latest-gpui.json`, a manifest in
//! the Tauri updater's static format. The manifest and the download are untrusted. A build is
//! installed only when its bytes carry a signature from the key in `gpui/updater.json` and that
//! signature names the version the manifest announced.
use base64::Engine as _;
use kubecove_lib::models::{AppError, AppErrorKind};
use minisign_verify::{PublicKey, Signature, StreamVerifier};
use std::{ffi::OsString, io, path::PathBuf, time::Duration};
use tokio::sync::{mpsc, watch};

#[cfg(test)]
mod feed_tests;
mod install;

pub use install::{InstallTarget, Staged, bundle_of};

#[derive(Clone, Debug, Default)]
pub enum UpdateStatus {
    #[default]
    Idle,
    UpToDate,
    Unpublished,
    Available(AvailableUpdate),
    Consolidated(String),
}

impl UpdateStatus {
    pub(crate) fn release_page_message(&self) -> Option<String> {
        match self {
            Self::Consolidated(version) => Some(format!(
                "KubeCove {version} requires a separate installation."
            )),
            Self::Unpublished => Some("No desktop update has been published yet.".into()),
            _ => None,
        }
    }
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
#[derive(serde::Deserialize)]
pub struct Feed {
    pub endpoint: String,
    pub public_key: String,
}

/// Every release asset of this client starts with this, and the signer records the asset's
/// file name in the signature.
const ARTIFACT_PREFIX: &str = "kubecove-gpui";

impl Feed {
    pub fn release() -> Result<Self, AppError> {
        serde_json::from_str(include_str!("../../updater.json")).map_err(|error| {
            AppError::new("Invalid gpui/updater.json", AppErrorKind::Validation).with_source(error)
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

/// How long the update check and download wait for a connection or for more bytes. Without it a
/// server that accepts and then stalls leaves Checking or Installing pending until restart. It
/// limits silence, not the whole transfer, so a slow download that keeps arriving still
/// completes. The owner chose 30 seconds (#662); nothing in the project or the Tauri updater
/// sets one.
pub const STALL_TIMEOUT: Duration = Duration::from_secs(30);

fn client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(STALL_TIMEOUT)
        .read_timeout(STALL_TIMEOUT)
        .build()
}

pub(super) async fn check() -> Result<UpdateStatus, AppError> {
    let failed = || AppError::new("Could not check for updates", AppErrorKind::Network);
    let endpoint = Feed::release()?.endpoint;
    let client = client().map_err(|_| failed())?;
    let current = semver::Version::parse(env!("CARGO_PKG_VERSION")).expect("valid package version");
    check_feeds(&client, &endpoint, &platform(), &current).await
}

async fn check_feeds(
    client: &reqwest::Client,
    endpoint: &str,
    target: &str,
    current: &semver::Version,
) -> Result<UpdateStatus, AppError> {
    let consolidated_error =
        match kubecove_lib::desktop_release::consolidated(client, endpoint, target).await {
            Ok(Some(version)) => {
                return Ok(if version.cmp_precedence(current).is_gt() {
                    UpdateStatus::Consolidated(version.to_string())
                } else {
                    UpdateStatus::UpToDate
                });
            }
            result => result.err(),
        };
    match (
        check_legacy(client, endpoint, target, current).await,
        consolidated_error,
    ) {
        (Ok(status @ UpdateStatus::Available(_)), _) => Ok(status),
        (_, Some(error)) => Err(error),
        (result, None) => result,
    }
}

async fn check_legacy(
    client: &reqwest::Client,
    endpoint: &str,
    target: &str,
    current: &semver::Version,
) -> Result<UpdateStatus, AppError> {
    let response = client
        .get(endpoint)
        .send()
        .await
        .map_err(|_| AppError::new("Could not check for updates", AppErrorKind::Network))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(UpdateStatus::Unpublished);
    }
    if response.status() == reqwest::StatusCode::NO_CONTENT {
        return Ok(UpdateStatus::UpToDate);
    }
    let manifest = response
        .error_for_status()
        .map_err(|_| AppError::new("Update check failed", AppErrorKind::Network))?;
    let manifest = kubecove_lib::desktop_release::read_manifest(manifest).await?;
    release_for_version(&manifest, target, current)
}

/// Reads the manifest. A newer version is offered only with a build for `platform`.
pub fn release(manifest: &serde_json::Value, platform: &str) -> Result<UpdateStatus, AppError> {
    let current =
        semver::Version::parse(env!("CARGO_PKG_VERSION")).expect("Cargo package version is semver");
    release_for_version(manifest, platform, &current)
}

pub fn release_for_version(
    manifest: &serde_json::Value,
    platform: &str,
    current: &semver::Version,
) -> Result<UpdateStatus, AppError> {
    let invalid = |message: &str| AppError::new(message, AppErrorKind::Validation);
    let version = kubecove_lib::desktop_release::version(manifest)?;
    if !version.cmp_precedence(current).is_gt() {
        return Ok(UpdateStatus::UpToDate);
    }
    legacy_version(&version)?;
    kubecove_lib::desktop_release::validate_product(
        manifest,
        platform,
        "KubeCove GPUI",
        "com.timpan.kubecove.gpui",
    )?;
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
    let url = field("url")?;
    let expected = kubecove_lib::desktop_release::asset_name(ARTIFACT_PREFIX, &version, platform);
    if !expected.is_some_and(|name| kubecove_lib::desktop_release::matches_asset(&url, &name)) {
        return Err(invalid("Unexpected side-by-side update artifact"));
    }
    Ok(UpdateStatus::Available(AvailableUpdate {
        version: version.to_string(),
        notes: manifest["notes"]
            .as_str()
            .map(str::trim)
            .filter(|notes| !notes.is_empty())
            .map(str::to_owned),
        artifact: Artifact {
            url,
            signature: field("signature")?,
        },
    }))
}

fn legacy_version(version: &semver::Version) -> Result<(), AppError> {
    if version
        .cmp_precedence(&kubecove_lib::desktop_release::LEGACY_MAX)
        .is_gt()
    {
        return Err(AppError::new(
            "This release requires a separate KubeCove installer",
            AppErrorKind::Validation,
        ));
    }
    Ok(())
}

/// Accepts `bytes` only when `signature` is a signature of them by `public_key` and it was made
/// for `version`. It runs the same check `install` runs over a download as it arrives.
pub fn verify(
    bytes: &[u8],
    signature: &str,
    public_key: &str,
    version: &str,
    platform: &str,
) -> Result<(), AppError> {
    let signed = Signed::decode(signature, public_key)?;
    let mut verifier = signed.stream()?;
    verifier.update(bytes);
    signed.check(verifier, version, platform)
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
    /// the legacy unhashed form is refused; release bundles use prehashed signatures.
    fn stream(&self) -> Result<StreamVerifier<'_>, AppError> {
        self.public_key
            .verify_stream(&self.signature)
            .map_err(|_| invalid_signature())
    }

    /// Accepts the hashed build only when the signature holds and was made for `version`. The
    /// manifest is not signed, so its version alone does not prove which release the URL and
    /// signature point at: without the second check a tampered manifest could pair a new
    /// version number with an older signed build.
    fn check(
        &self,
        mut verifier: StreamVerifier<'_>,
        version: &str,
        platform: &str,
    ) -> Result<(), AppError> {
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
        let parsed = semver::Version::parse(version).map_err(|_| invalid_signature())?;
        let name = kubecove_lib::desktop_release::asset_name(ARTIFACT_PREFIX, &parsed, platform)
            .ok_or_else(invalid_signature)?;
        if signed("file:") != Some(name.as_str()) {
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
    let version = semver::Version::parse(&update.version).map_err(|_| invalid_signature())?;
    legacy_version(&version)?;
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
    let platform = match target {
        InstallTarget::AppImage(_) => "linux-x86_64",
        InstallTarget::AppBundle(_) => "darwin-aarch64",
        InstallTarget::Installer(_) => "windows-x86_64",
    };
    signed.check(verifier, version, platform)?;
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
    let mut response = client()
        .map_err(|_| download_failed())?
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
