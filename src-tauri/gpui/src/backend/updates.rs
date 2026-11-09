//! Update check and installation for the native client (ADR 0034).
//!
//! Each release publishes one signed build per platform and `latest-gpui.json`, a manifest in
//! the Tauri updater's static format. The manifest and the download are untrusted. A build is
//! installed only when its bytes carry a signature from the key in `tauri.conf.json` and that
//! signature names the version the manifest announced.
use base64::Engine as _;
use kubecove_lib::models::{AppError, AppErrorKind};
use minisign_verify::{PublicKey, Signature};
use std::{
    ffi::OsString,
    fs, io,
    io::Write as _,
    path::{Path, PathBuf},
};
use tokio::sync::watch;

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
/// for `version`. The manifest is not signed, so its version alone does not prove which release
/// the URL and signature point at: without the second check a tampered manifest could pair a
/// new version number with an older signed build.
pub fn verify(
    bytes: &[u8],
    signature: &str,
    public_key: &str,
    version: &str,
) -> Result<(), AppError> {
    let invalid = || AppError::new("Invalid update signature", AppErrorKind::Validation);
    let text = |encoded: &str| {
        base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .ok()
            .and_then(|decoded| String::from_utf8(decoded).ok())
            .ok_or_else(invalid)
    };
    let public_key = PublicKey::decode(&text(public_key)?).map_err(|_| invalid())?;
    let signature = Signature::decode(&text(signature)?).map_err(|_| invalid())?;
    // The Tauri signer always prehashes, so the legacy unhashed form is refused.
    public_key
        .verify(bytes, &signature, false)
        .map_err(|_| invalid())?;
    // The trusted comment is covered by the signature, so it is read only after `verify`.
    // The signer writes it as tab-separated `key:value` fields.
    let comment = signature.trusted_comment();
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
    // The Tauri desktop app is signed with the same key and names its version too. The signed
    // file name tells the two products apart.
    if !signed("file:").is_some_and(|file| file.starts_with(ARTIFACT_PREFIX)) {
        return Err(AppError::new(
            "The update is not a KubeCove GPUI build",
            AppErrorKind::Validation,
        ));
    }
    Ok(())
}

/// What an update replaces on this machine.
#[derive(Clone, Debug)]
pub enum InstallTarget {
    /// Linux: the AppImage file this process runs from.
    AppImage(PathBuf),
    /// macOS: the `.app` bundle that holds the running executable.
    AppBundle(PathBuf),
    /// Windows: the directory the verified installer is saved to. The installer runs on relaunch.
    Installer(PathBuf),
}

/// The NSIS installer's passive mode, and its restart of the app when it finishes.
const INSTALLER_ARGS: [&str; 2] = ["/P", "/R"];

impl InstallTarget {
    /// The running app's install location. A build started outside its bundle has none.
    pub fn current() -> Result<Self, AppError> {
        #[cfg(target_os = "linux")]
        let target = std::env::var_os("APPIMAGE").map(|path| Self::AppImage(path.into()));
        #[cfg(target_os = "macos")]
        let target = std::env::current_exe()
            .ok()
            .and_then(|executable| bundle_of(&executable))
            .map(Self::AppBundle);
        #[cfg(target_os = "windows")]
        let target = Some(Self::Installer(std::env::temp_dir()));
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        let target: Option<Self> = None;
        target.ok_or_else(|| {
            AppError::new(
                "Updates install only when KubeCove runs from its release AppImage, app bundle or installer",
                AppErrorKind::UnsupportedOperation,
            )
        })
    }

    /// Puts verified `bytes` in place. A failure leaves the installed version as it was.
    pub fn install(&self, bytes: &[u8], version: &str) -> Result<Relaunch, AppError> {
        match self {
            Self::AppImage(path) => replace_appimage(path, bytes)
                .map_err(|error| failed("Could not replace the AppImage", &error))
                .map(|()| Relaunch {
                    program: path.clone(),
                    args: Vec::new(),
                }),
            Self::AppBundle(bundle) => replace_bundle(bundle, bytes)
                .map_err(|error| failed("Could not replace the app bundle", &error))
                .map(|()| Relaunch {
                    program: "/usr/bin/open".into(),
                    args: vec!["-n".into(), bundle.clone().into()],
                }),
            Self::Installer(directory) => {
                let installer = directory.join(format!("KubeCove-GPUI-{version}-setup.exe"));
                save_installer(&installer, bytes)
                    .map_err(|error| failed("Could not save the installer", &error))
                    .map(|()| Relaunch {
                        program: installer,
                        args: INSTALLER_ARGS.map(OsString::from).to_vec(),
                    })
            }
        }
    }
}

/// `<Name>.app` for an executable at `<Name>.app/Contents/MacOS/<binary>`.
pub fn bundle_of(executable: &Path) -> Option<PathBuf> {
    let macos = executable.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    (macos.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && bundle.extension()? == "app")
        .then(|| bundle.to_owned())
}

/// Downloads the update's build, verifies it and installs it. Nothing is written before the
/// signature checks out.
pub async fn install(
    update: AvailableUpdate,
    public_key: String,
    target: InstallTarget,
    progress: watch::Sender<DownloadProgress>,
) -> Result<Relaunch, AppError> {
    let bytes = download(&update.artifact.url, &progress).await?;
    // Hashing the build and writing it block, so they run off the async workers.
    tokio::task::spawn_blocking(move || {
        verify(
            &bytes,
            &update.artifact.signature,
            &public_key,
            &update.version,
        )?;
        target.install(&bytes, &update.version)
    })
    .await
    .map_err(|_| AppError::new("Update installation stopped", AppErrorKind::Internal))?
}

async fn download(
    url: &str,
    progress: &watch::Sender<DownloadProgress>,
) -> Result<Vec<u8>, AppError> {
    let failed = || AppError::new("Could not download the update", AppErrorKind::Network);
    let mut response = reqwest::Client::new()
        .get(url)
        .header(reqwest::header::ACCEPT, "application/octet-stream")
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|_| failed())?;
    let total = response.content_length();
    let mut bytes = Vec::new();
    progress.send_replace(DownloadProgress {
        downloaded: 0,
        total,
    });
    while let Some(chunk) = response.chunk().await.map_err(|_| failed())? {
        bytes.extend_from_slice(&chunk);
        progress.send_replace(DownloadProgress {
            downloaded: bytes.len() as u64,
            total,
        });
    }
    Ok(bytes)
}

fn failed(action: &str, error: &io::Error) -> AppError {
    AppError::new(format!("{action}: {error}"), AppErrorKind::Io)
}

fn invalid_data(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// A hidden sibling of `path`, so a rename between the two stays on one file system.
fn sibling(path: &Path, purpose: &str) -> io::Result<PathBuf> {
    let name = path
        .file_name()
        .ok_or_else(|| invalid_data("The install path has no file name"))?;
    let mut hidden = OsString::from(".");
    hidden.push(name);
    hidden.push(".");
    hidden.push(purpose);
    Ok(path.with_file_name(hidden))
}

/// Writes a new file and syncs it. It refuses an existing path, so it never writes through a
/// link someone else placed there.
fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Writes the new AppImage beside the current one and renames it over it, so the path always
/// holds a complete AppImage. The running app keeps its already open file.
fn replace_appimage(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if !bytes.starts_with(b"\x7fELF") {
        return Err(invalid_data("The update is not an AppImage"));
    }
    let permissions = fs::metadata(path)?.permissions();
    let staged = sibling(path, "update")?;
    remove_stale(&staged)?;
    let replaced = write_new(&staged, bytes)
        .and_then(|()| fs::set_permissions(&staged, permissions))
        .and_then(|()| fs::rename(&staged, path));
    if replaced.is_err() {
        let _ = fs::remove_file(&staged);
    }
    replaced
}

/// Unpacks the `.app.tar.gz` beside the installed bundle, then moves the installed bundle
/// aside and the new one into its place. A failed move puts the installed bundle back. The
/// previous bundle is deleted only once the new one is in place.
fn replace_bundle(bundle: &Path, bytes: &[u8]) -> io::Result<()> {
    fs::symlink_metadata(bundle)?;
    let staging = sibling(bundle, "update")?;
    let previous = sibling(bundle, "previous")?;
    remove_stale(&staging)?;
    remove_stale(&previous)?;
    fs::create_dir(&staging)?;
    let replaced = unpack_bundle(bytes, &staging).and_then(|unpacked| {
        fs::rename(bundle, &previous)?;
        fs::rename(&unpacked, bundle).inspect_err(|_| {
            let _ = fs::rename(&previous, bundle);
        })
    });
    let _ = fs::remove_dir_all(&staging);
    if replaced.is_ok() {
        let _ = fs::remove_dir_all(&previous);
    }
    replaced
}

/// Unpacks the archive into `staging` and returns its one `.app` directory. `Archive::unpack`
/// skips entries that would land outside `staging`.
fn unpack_bundle(bytes: &[u8], staging: &Path) -> io::Result<PathBuf> {
    tar::Archive::new(flate2::read::GzDecoder::new(bytes)).unpack(staging)?;
    let mut bundles = Vec::new();
    for entry in fs::read_dir(staging)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir()
            && path.extension().is_some_and(|extension| extension == "app")
        {
            bundles.push(path);
        }
    }
    match <[PathBuf; 1]>::try_from(bundles) {
        Ok([bundle]) => Ok(bundle),
        Err(_) => Err(invalid_data("The update does not hold one app bundle")),
    }
}

fn save_installer(installer: &Path, bytes: &[u8]) -> io::Result<()> {
    if !bytes.starts_with(b"MZ") {
        return Err(invalid_data("The update is not a Windows installer"));
    }
    remove_stale(installer)?;
    write_new(installer, bytes)
}

/// Removes what an interrupted earlier install left at `path`, without following a link.
fn remove_stale(path: &Path) -> io::Result<()> {
    let removed = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(error) => Err(error),
    };
    match removed {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}
