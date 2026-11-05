//! Desktop host capabilities. Dialogs remain behind Backend so offscreen tests
//! never open a picker. Only the production entry point initializes user storage.
use super::{NativeBackend, Read};
use kubecove_lib::{
    commands,
    models::{AppError, AppErrorKind},
};
use std::path::PathBuf;

pub use kubecove_host::remote::RemoteConnectionSummary;

#[derive(Clone, Debug, Default)]
pub enum UpdateStatus {
    #[default]
    Idle,
    UpToDate,
    Available(String),
}

pub const INSTALL_UNAVAILABLE: &str = "Native installation is disabled. The release feed contains Tauri installers; a signed GPUI release artifact and installer are required.";

pub fn config_directory() -> Result<PathBuf, AppError> {
    dirs::config_dir()
        .map(|path| path.join("com.timpan.kubecove"))
        .ok_or_else(|| AppError::new("Platform config directory is unavailable", AppErrorKind::Io))
}

impl NativeBackend {
    pub fn with_legacy_session_file(mut self, path: PathBuf) -> Self {
        self.preferences.set_legacy_file(path);
        self
    }

    pub fn with_settings_directory(directory: PathBuf) -> std::io::Result<Self> {
        commands::init_kubeconfig_settings_path(directory.clone());
        let mut backend = Self::new()?;
        backend.settings_initialized = true;
        backend = backend.with_mcp_directory(directory.clone());
        backend.preferences = crate::preferences::Store::file(directory);
        Ok(backend)
    }

    pub(super) fn pick_kubeconfig(&self) -> Read<super::KubeconfigSourcesSummary> {
        self.read(async {
            let files = rfd::AsyncFileDialog::new()
                .set_title("Add kubeconfig files")
                .add_filter("Kubeconfig", &["yaml", "yml", "conf", "config"])
                .pick_files()
                .await;
            match files {
                None => commands::get_kubeconfig_sources(),
                Some(files) => {
                    let paths = files
                        .into_iter()
                        .map(|file| {
                            file.path().to_str().map(str::to_owned).ok_or_else(|| {
                                AppError::new(
                                    "Kubeconfig path is not UTF-8",
                                    AppErrorKind::Validation,
                                )
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    commands::add_kubeconfig_paths(paths)
                }
            }
        })
    }
}

pub(super) async fn check_updates() -> Result<UpdateStatus, AppError> {
    // Reuse the desktop's configured endpoint, never an independently maintained feed.
    let config: serde_json::Value = serde_json::from_str(include_str!("../../../tauri.conf.json"))
        .map_err(|_| AppError::new("Invalid updater configuration", AppErrorKind::Validation))?;
    let endpoint = config["plugins"]["updater"]["endpoints"][0]
        .as_str()
        .ok_or_else(|| AppError::new("Missing update endpoint", AppErrorKind::Validation))?;
    let response = reqwest::Client::new()
        .get(endpoint)
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
    update_status(&manifest)
}

pub fn update_status(manifest: &serde_json::Value) -> Result<UpdateStatus, AppError> {
    let version = manifest["version"]
        .as_str()
        .ok_or_else(|| AppError::new("Missing update version", AppErrorKind::Validation))?;
    let version = semver::Version::parse(version.trim_start_matches('v'))
        .map_err(|_| AppError::new("Invalid update version", AppErrorKind::Validation))?;
    let current =
        semver::Version::parse(env!("CARGO_PKG_VERSION")).expect("Cargo package version is semver");
    Ok(if version > current {
        UpdateStatus::Available(version.to_string())
    } else {
        UpdateStatus::UpToDate
    })
}

pub fn unavailable<T: Send + 'static>(reason: &'static str) -> Read<T> {
    Box::pin(async move { Err(AppError::new(reason, AppErrorKind::UnsupportedOperation)) })
}
