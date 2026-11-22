//! Local Tauri-to-GPUI installation (ADR 0036). The webview never supplies a path or URL.
mod feed;
mod install;
mod integrity;
#[cfg(target_os = "macos")]
mod macos_acl;
#[cfg(test)]
mod tests;
#[cfg(windows)]
mod windows_acl;

use crate::models::{AppError, AppErrorKind, GpuiUpgradeInfo};
use feed::{Feed, Platform, Release};
use std::{
    io::{self, Seek},
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};
use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tauri_plugin_opener::OpenerExt;
use tokio::{
    io::AsyncWriteExt,
    sync::{Mutex, OwnedMutexGuard},
};

#[derive(Default)]
pub struct GpuiUpgrade {
    installed: Arc<Mutex<Option<Arc<Installed>>>>,
    action: Arc<Mutex<()>>,
}

impl GpuiUpgrade {
    fn begin_install(&self) -> Result<InstallAction, AppError> {
        Ok(InstallAction {
            _guard: Arc::clone(&self.action)
                .try_lock_owned()
                .map_err(|_| busy())?,
            installed: Arc::clone(&self.installed),
        })
    }

    async fn installed_info(&self) -> Result<Option<GpuiUpgradeInfo>, AppError> {
        let snapshot = self.installed.lock().await.clone();
        let Some(installed) = snapshot else {
            return Ok(None);
        };
        // Permission checks may touch a removable or network volume.
        blocking(move || Ok(Some(installed.info()))).await
    }
}

struct InstallAction {
    _guard: OwnedMutexGuard<()>,
    installed: Arc<Mutex<Option<Arc<Installed>>>>,
}

impl InstallAction {
    async fn publish(
        self,
        publish: impl FnOnce() -> Result<Installed, AppError> + Send + 'static,
    ) -> Result<GpuiUpgradeInfo, AppError> {
        // Once publication starts, the worker owns the action and records the result
        // even when the IPC caller stops waiting for it.
        blocking(move || Ok(self.commit(publish()?))).await
    }

    fn commit(self, installed: Installed) -> GpuiUpgradeInfo {
        let installed = Arc::new(installed);
        *self.installed.blocking_lock() = Some(Arc::clone(&installed));
        installed.info()
    }
}

struct Installed {
    platform: Platform,
    version: String,
    path: PathBuf,
    launch_digest: Option<ring::digest::Digest>,
    _installer_lease: Option<std::fs::File>,
}

impl Installed {
    fn info(&self) -> GpuiUpgradeInfo {
        GpuiUpgradeInfo::Installed {
            path: self.path.to_string_lossy().into_owned(),
            can_launch: self.launch_digest.is_some()
                && integrity::protected_install(self.platform, &self.path),
            version: self.version.clone(),
            install_kind: self.platform.kind(),
        }
    }

    fn verify_launch(&self) -> Result<PathBuf, AppError> {
        let binary = integrity::executable(self.platform, &self.path);
        let expected = self
            .launch_digest
            .as_ref()
            .filter(|_| integrity::protected_install(self.platform, &self.path))
            .ok_or_else(|| {
                invalid("Cannot launch GPUI from this location. Open its folder to inspect it.")
            })?;
        let actual = integrity::sha256(std::fs::File::open(&binary).map_err(io_error)?)?;
        if actual.as_ref() != expected.as_ref() {
            return Err(invalid(
                "The installed GPUI executable has changed since verification",
            ));
        }
        Ok(binary)
    }
}

fn invalid(message: &str) -> AppError {
    AppError::new(message, AppErrorKind::Validation)
}
fn network(message: &str) -> AppError {
    AppError::new(message, AppErrorKind::Network)
}
fn io_error(error: io::Error) -> AppError {
    AppError::new(format!("GPUI installation: {error}"), AppErrorKind::Io)
}
fn busy() -> AppError {
    invalid("A GPUI upgrade action is already in progress")
}

pub async fn check_gpui_upgrade(
    state: State<'_, GpuiUpgrade>,
) -> Result<GpuiUpgradeInfo, AppError> {
    let platform = match Platform::current() {
        Ok(platform) => platform,
        Err(message) => {
            let os = if cfg!(target_os = "macos") {
                "darwin"
            } else {
                std::env::consts::OS
            };
            return Ok(GpuiUpgradeInfo::Unsupported {
                target: format!("{os}-{}", std::env::consts::ARCH),
                message,
            });
        }
    };
    let feed = Feed::configured()?;
    let consolidated_error = match feed.consolidated(platform).await {
        Ok(Some(version)) => {
            return Ok(GpuiUpgradeInfo::Consolidated {
                version: version.to_string(),
            });
        }
        result => result.err(),
    };
    if let Some(info) = state.installed_info().await? {
        return Ok(info);
    }
    let latest = feed.latest(platform).await;
    if let Some(info) = state.installed_info().await? {
        return Ok(info);
    }
    match latest {
        Ok(Some(release)) => Ok(release.info()),
        Ok(None) => consolidated_error.map_or_else(
            || {
                Ok(GpuiUpgradeInfo::Unavailable {
                    target: platform.key().into(),
                })
            },
            Err,
        ),
        Err(error) => Err(consolidated_error.unwrap_or(error)),
    }
}

fn choose_folder<R: Runtime>(
    app: &AppHandle<R>,
    platform: Platform,
) -> Result<Option<PathBuf>, AppError> {
    if platform == Platform::Windows {
        return Ok(Some(std::env::temp_dir()));
    }
    let start = if platform == Platform::Mac {
        Some(PathBuf::from("/Applications"))
    } else {
        app.path().download_dir().ok()
    };
    let mut dialog = app
        .dialog()
        .file()
        .set_title("Choose where to install KubeCove GPUI");
    if let Some(start) = start {
        dialog = dialog.set_directory(start);
    }
    dialog
        .blocking_pick_folder()
        .map(|p| {
            p.into_path()
                .map_err(|_| invalid("Choose a local folder for KubeCove GPUI"))
        })
        .transpose()
}

pub async fn install_gpui_upgrade<R: Runtime>(
    version: String,
    app: AppHandle<R>,
    state: State<'_, GpuiUpgrade>,
) -> Result<Option<GpuiUpgradeInfo>, AppError> {
    let action = state.begin_install()?;
    if let Some(info) = state.installed_info().await? {
        return Ok(Some(info));
    }
    let platform =
        Platform::current().map_err(|_| invalid("This platform cannot install KubeCove GPUI"))?;
    let feed = Feed::configured()?;
    let release = feed
        .latest(platform)
        .await?
        .filter(|release| release.version == version)
        .ok_or_else(|| invalid("The release has changed. Check again before installing."))?;
    let cache = app
        .path()
        .app_cache_dir()
        .map_err(|_| invalid("Cannot locate the application cache"))?;
    let Some(folder) = blocking(move || choose_folder(&app, platform)).await? else {
        return Ok(None);
    };
    install_release(
        release,
        feed.public_key,
        &folder,
        &cache,
        feed::client()?,
        action,
    )
    .await
    .map(Some)
}

async fn install_release(
    release: Release,
    public_key: String,
    folder: &Path,
    cache: &Path,
    client: reqwest::Client,
    action: InstallAction,
) -> Result<GpuiUpgradeInfo, AppError> {
    let platform = release.platform;
    let version = release.version.clone();
    let lock = install::lock_install(cache)?;
    let folder = folder.canonicalize().map_err(io_error)?;
    let staging_root = if platform == Platform::Windows {
        &folder
    } else {
        cache
    };
    let (staging, lease) = install::stage_installer(staging_root)?;
    let destination = if platform == Platform::Windows {
        staging.path()
    } else {
        &folder
    };
    let path = install::destination(destination, platform, &version)?;
    let mut staged = install::stage(staging.path())?;
    let mut output = tokio::fs::File::from_std(staged.as_file().try_clone().map_err(io_error)?);
    let mut response = client
        .get(&release.artifact.url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|_| network("Could not download KubeCove GPUI"))?;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| network("The GPUI download was interrupted"))?
    {
        output.write_all(&chunk).await.map_err(io_error)?;
    }
    output.flush().await.map_err(io_error)?;
    drop(output);
    action
        .publish(move || {
            let _lock = lock;
            let artifact_digest = feed::verify(
                staged.as_file_mut(),
                &release.artifact.signature,
                &public_key,
                &version,
                &platform.asset(&version),
            )?;
            let digest = if platform == Platform::Mac {
                staged.rewind().map_err(io_error)?;
                install::publish_bundle(staged, &folder)?
            } else {
                install::publish_executable(staged, platform, &path)?;
                Some(artifact_digest)
            };
            let launch_digest = digest.filter(|_| integrity::protected_install(platform, &path));
            // Settings and Windows setup still need the installer after publication.
            let installer_lease = (platform == Platform::Windows).then(|| {
                let _ = staging.keep();
                lease
            });
            Ok(Installed {
                platform,
                version,
                path,
                launch_digest,
                _installer_lease: installer_lease,
            })
        })
        .await
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|_| AppError::new("GPUI installation task stopped", AppErrorKind::Internal))?
}

pub async fn open_desktop_releases_page<R: Runtime>(app: AppHandle<R>) -> Result<(), AppError> {
    app.opener()
        .open_url(crate::desktop_release::RELEASES_URL, None::<&str>)
        .map_err(|e| {
            AppError::new(
                format!("Could not open GitHub Releases: {e}"),
                AppErrorKind::Io,
            )
        })
}

pub async fn reveal_gpui_upgrade<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, GpuiUpgrade>,
) -> Result<(), AppError> {
    let installed = state
        .installed
        .lock()
        .await
        .clone()
        .ok_or_else(|| invalid("Install GPUI before revealing it"))?;
    app.opener()
        .reveal_item_in_dir(&installed.path)
        .map_err(|e| AppError::new(format!("Could not reveal GPUI: {e}"), AppErrorKind::Io))
}

pub async fn launch_gpui_upgrade<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, GpuiUpgrade>,
) -> Result<(), AppError> {
    let _action = state.action.try_lock().map_err(|_| busy())?;
    let installed = state
        .installed
        .lock()
        .await
        .clone()
        .ok_or_else(|| invalid("Install GPUI before launching it"))?;
    blocking(move || {
        if installed.launch_digest.is_none() {
            return Err(invalid(
                "Cannot launch GPUI from this location. Open its folder to inspect it.",
            ));
        }
        let version = &installed.version;
        if installed.platform == Platform::Windows
            && !app
                .dialog()
                .message(format!(
                    "Run the verified KubeCove GPUI {version} installer for this user?"
                ))
                .title("Install KubeCove GPUI")
                .buttons(MessageDialogButtons::OkCancel)
                .blocking_show()
        {
            return Ok(());
        }
        // Start the exact executable just hashed, including inside the macOS bundle.
        Command::new(installed.verify_launch()?)
            .spawn()
            .map(drop)
            .map_err(io_error)
    })
    .await
}
