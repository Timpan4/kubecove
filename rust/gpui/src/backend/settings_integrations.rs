//! Desktop host capabilities. Dialogs remain behind Backend so offscreen tests
//! never open a picker. Only the production entry point initializes user storage.
use super::{NativeBackend, Read};
use kubecove_lib::{
    commands,
    models::{AppError, AppErrorKind},
};
use std::path::PathBuf;

pub use kubecove_host::remote::RemoteConnectionSummary;

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

pub fn unavailable<T: Send + 'static>(reason: &'static str) -> Read<T> {
    Box::pin(async move { Err(AppError::new(reason, AppErrorKind::UnsupportedOperation)) })
}
