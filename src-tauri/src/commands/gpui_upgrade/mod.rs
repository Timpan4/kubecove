//! Local Tauri-to-GPUI installation (ADR 0036). The webview never supplies a path or URL.
mod feed;
mod install;
#[cfg(test)]
mod tests;

use crate::models::{AppError, AppErrorKind, GpuiUpgradeInfo};
use feed::{Feed, Platform, Release};
use std::{
    io::{self, Seek},
    path::{Path, PathBuf},
    process::Command,
};
use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tauri_plugin_opener::OpenerExt;
use tokio::{io::AsyncWriteExt, sync::Mutex};

#[derive(Default)]
pub struct GpuiUpgrade(Mutex<Option<Installed>>);

struct Installed {
    release: Release,
    path: PathBuf,
}

impl Installed {
    fn info(&self) -> GpuiUpgradeInfo {
        GpuiUpgradeInfo::Installed {
            path: self.path.clone(),
            version: self.release.version.clone(),
            install_kind: self.release.platform.kind(),
        }
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
    let installed = state.0.lock().await;
    if let Some(installed) = installed.as_ref() {
        return Ok(installed.info());
    }
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
    Ok(Feed::configured()?.latest(platform).await?.map_or_else(
        || GpuiUpgradeInfo::Unavailable {
            target: platform.key().into(),
        },
        |release| release.info(),
    ))
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
    let mut installed = state.0.try_lock().map_err(|_| busy())?;
    if let Some(previous) = installed.as_ref() {
        return Ok(Some(previous.info()));
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
    let result =
        install_release(release, feed.public_key, &folder, &cache, feed::client()?).await?;
    let info = result.info();
    *installed = Some(result);
    Ok(Some(info))
}

async fn install_release(
    release: Release,
    public_key: String,
    folder: &Path,
    cache: &Path,
    client: reqwest::Client,
) -> Result<Installed, AppError> {
    let platform = release.platform;
    let version = release.version.clone();
    let lock = install::lock_install(cache)?;
    let folder = folder.canonicalize().map_err(io_error)?;
    // A previous installer may still be running or waiting for the user after Tauri quits.
    let installer_dir = if platform == Platform::Windows {
        Some(
            tempfile::Builder::new()
                .prefix("KubeCove-GPUI-")
                .tempdir_in(&folder)
                .map_err(io_error)?,
        )
    } else {
        None
    };
    let folder = installer_dir
        .as_ref()
        .map_or(folder, |dir| dir.path().to_owned());
    let path = install::destination(&folder, platform, &version)?;
    // A shared application directory must never own the predictable, reclaimable download.
    let mut staged = install::stage(
        installer_dir
            .as_ref()
            .map_or(cache, tempfile::TempDir::path),
    )?;
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
    blocking(move || {
        let _lock = lock;
        feed::verify(
            staged.as_file_mut(),
            &release.artifact.signature,
            &public_key,
            &version,
            &platform.asset(&version),
        )?;
        if platform == Platform::Mac {
            staged.rewind().map_err(io_error)?;
            install::publish_bundle(staged, &folder)?;
        } else {
            install::publish_executable(staged, platform, &path)?;
        }
        if let Some(directory) = installer_dir {
            let _ = directory.keep();
        }
        Ok(Installed { release, path })
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

pub async fn reveal_gpui_upgrade<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, GpuiUpgrade>,
) -> Result<(), AppError> {
    let guard = state.0.try_lock().map_err(|_| busy())?;
    let installed = guard
        .as_ref()
        .ok_or_else(|| invalid("Install GPUI before revealing it"))?;
    app.opener()
        .reveal_item_in_dir(&installed.path)
        .map_err(|e| AppError::new(format!("Could not reveal GPUI: {e}"), AppErrorKind::Io))
}

pub async fn launch_gpui_upgrade<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, GpuiUpgrade>,
) -> Result<(), AppError> {
    let guard = state.0.try_lock().map_err(|_| busy())?;
    let installed = guard
        .as_ref()
        .ok_or_else(|| invalid("Install GPUI before launching it"))?;
    let release = &installed.release;
    let platform = release.platform;
    let path = installed.path.clone();
    let version = release.version.clone();
    let signature = release.artifact.signature.clone();
    blocking(move || {
        if platform == Platform::Windows
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
        let mut command = if platform == Platform::Mac {
            let mut cmd = Command::new(Path::new("/usr/bin/open"));
            cmd.arg("-n").arg(path);
            cmd
        } else {
            // Refuse an executable replaced between installation and the separate launch action.
            feed::verify(
                &mut std::fs::File::open(&path).map_err(io_error)?,
                &signature,
                &Feed::configured()?.public_key,
                &version,
                &platform.asset(&version),
            )?;
            Command::new(path)
        };
        command.spawn().map(drop).map_err(io_error)
    })
    .await
}
