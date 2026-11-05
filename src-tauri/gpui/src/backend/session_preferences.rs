//! Native storage for user-authored presets. No credentials or terminal output.
use super::{NativeBackend, Read};
use kubecove_lib::models::{AppError, AppErrorKind, PortForwardRequest};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf, sync::Mutex};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SessionPreferences {
    pub presets: HashMap<String, Vec<SavedForward>>,
    pub auto_start: bool,
    pub keep: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct SavedForward {
    pub request: PortForwardRequest,
    pub label: String,
}

// Serialize atomic replacements, including multiple windows of the same host.
static STORAGE: Mutex<()> = Mutex::new(());
fn path() -> Result<PathBuf, AppError> {
    dirs::config_dir()
        .map(|root| root.join("kubecove-gpui").join("live-sessions.json"))
        .ok_or_else(|| {
            AppError::new(
                "User configuration directory unavailable",
                AppErrorKind::Validation,
            )
        })
}
fn error(error: impl std::fmt::Display) -> AppError {
    AppError::new(
        format!("Could not store live-session preferences: {error}"),
        AppErrorKind::Validation,
    )
}
pub(super) fn load(backend: &NativeBackend) -> Read<SessionPreferences> {
    backend.read(async {
        let _guard = STORAGE.lock().map_err(error)?;
        match std::fs::read(path()?) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(error),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(SessionPreferences::default()),
            Err(e) => Err(error(e)),
        }
    })
}
pub(super) fn save(backend: &NativeBackend, preferences: SessionPreferences) -> Read<()> {
    backend.read(async move {
        let _guard = STORAGE.lock().map_err(error)?;
        let path = path()?;
        let parent = path.parent().expect("configuration file has a parent");
        std::fs::create_dir_all(parent).map_err(error)?;
        let staging = path.with_extension(format!("{}.tmp", std::process::id()));
        let bytes = serde_json::to_vec_pretty(&preferences).map_err(error)?;
        std::fs::write(&staging, bytes).map_err(error)?;
        std::fs::rename(staging, path).map_err(error)
    })
}
