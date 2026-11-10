//! Desktop MCP listener lifecycle (ADR 0028): its settings file, bearer token and the
//! loopback listener. MCP is off until the user enables it in Settings.

use crate::models::{AppError, AppErrorKind, McpStatus};
use crate::runtime::State;
use kubecove_kube::commands::ClusterLiveStore;
use kubecove_mcp::{
    EndpointPolicy, McpContext, McpEndpoint, TOOL_NAMES, load_or_create_token, rotate_token,
    serve_loopback,
};
use serde::{Deserialize, Serialize};
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};
use tokio::{
    sync::{Mutex, OwnedMutexGuard},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

const SETTINGS_FILE_NAME: &str = "mcp-settings.json";
/// Chosen by the product owner on 2026-11-02 (ADR 0028).
const DEFAULT_PORT: u16 = 47_615;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpSettings {
    enabled: bool,
    port: u16,
}

impl Default for McpSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: DEFAULT_PORT,
        }
    }
}

struct Listener {
    address: SocketAddr,
    shutdown: CancellationToken,
    task: JoinHandle<()>,
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.shutdown.cancel();
        self.task.abort();
    }
}

#[derive(Default)]
struct HostState {
    settings: McpSettings,
    listener: Option<Listener>,
    error: Option<String>,
    load_error: Option<String>,
}

struct Paths {
    config_dir: PathBuf,
    live_store: ClusterLiveStore,
}

/// Managed state for the desktop MCP listener.
#[derive(Default)]
pub struct McpHost {
    paths: OnceLock<Paths>,
    state: Arc<Mutex<HostState>>,
}

/// The host lock, held from [`McpHost::prepare`] until [`McpHost::start`] has started the
/// listener.
pub struct McpStartup(OwnedMutexGuard<HostState>);

impl McpHost {
    /// Loads saved settings and takes the host lock, so a command that arrives before the
    /// listener has started waits for it instead of reading defaults. Tools read through
    /// `live_store`, the same caches the app's views use. Returns `None` when already
    /// prepared.
    pub fn prepare(&self, config_dir: PathBuf, live_store: ClusterLiveStore) -> Option<McpStartup> {
        let settings = load_settings(&config_dir);
        // Nothing else holds the lock before the host is prepared.
        let mut state = Arc::clone(&self.state).try_lock_owned().ok()?;
        self.paths
            .set(Paths {
                config_dir,
                live_store,
            })
            .ok()?;
        match settings {
            Ok(settings) => state.settings = settings,
            Err(error) => state.load_error = Some(error.message),
        }
        Some(McpStartup(state))
    }

    /// Starts the listener when MCP was left enabled, then releases the host lock.
    pub async fn start(&self, mut startup: McpStartup) {
        self.restart(&mut startup.0).await;
    }

    fn paths(&self) -> Result<&Paths, AppError> {
        self.paths
            .get()
            .ok_or_else(|| AppError::new("MCP is not initialized", AppErrorKind::Internal))
    }

    async fn restart(&self, state: &mut HostState) {
        if let Some(mut listener) = state.listener.take() {
            listener.shutdown.cancel();
            // Graceful drain would wait on any client that holds a connection open, so the
            // accept loop is aborted; rebinding the port waits only for that.
            listener.task.abort();
            let _ = (&mut listener.task).await;
        }
        state.error = None;
        if state.load_error.is_some() || !state.settings.enabled {
            return;
        }
        match self.listen(state.settings.port).await {
            Ok(listener) => state.listener = Some(listener),
            Err(error) => state.error = Some(error.message),
        }
    }

    async fn listen(&self, port: u16) -> Result<Listener, AppError> {
        let paths = self.paths()?;
        let token = load_or_create_token(&paths.config_dir).map_err(token_error)?;
        let shutdown = CancellationToken::new();
        let context = Arc::new(McpContext::new(paths.live_store.clone(), None));
        let endpoint =
            McpEndpoint::new(context, token, EndpointPolicy::loopback(), shutdown.clone());
        let (address, task) = serve_loopback(port, endpoint, shutdown.clone())
            .await
            .map_err(|error| {
                AppError::new(
                    format!("Could not listen on 127.0.0.1:{port}: {error}"),
                    AppErrorKind::Io,
                )
            })?;
        Ok(Listener {
            address,
            shutdown,
            task,
        })
    }
}

fn status(state: &HostState) -> McpStatus {
    McpStatus {
        enabled: state.settings.enabled,
        port: state.settings.port,
        running: state.listener.is_some(),
        url: state
            .listener
            .as_ref()
            .map(|listener| format!("http://{}/mcp", listener.address)),
        error: state.error.clone(),
        tools: TOOL_NAMES.iter().map(|name| (*name).to_owned()).collect(),
    }
}

fn load_settings(config_dir: &Path) -> Result<McpSettings, AppError> {
    let content = match std::fs::read_to_string(config_dir.join(SETTINGS_FILE_NAME)) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(McpSettings::default());
        }
        Err(error) => {
            return Err(AppError::new(
                format!("failed to read MCP settings: {error}"),
                AppErrorKind::Io,
            ));
        }
    };
    let settings: McpSettings = serde_json::from_str(&content)
        .map_err(|_| AppError::new("Invalid MCP settings file", AppErrorKind::Serialization))?;
    if settings.port == 0 {
        return Err(AppError::new(
            "Invalid MCP port in settings file",
            AppErrorKind::Validation,
        ));
    }
    Ok(settings)
}

fn ensure_loaded(state: &HostState) -> Result<(), AppError> {
    match &state.load_error {
        Some(error) => Err(AppError::new(error.clone(), AppErrorKind::Io)),
        None => Ok(()),
    }
}

fn save_settings(config_dir: &Path, settings: McpSettings) -> Result<(), AppError> {
    let content = serde_json::to_string_pretty(&settings).map_err(|error| {
        AppError::new(error.to_string(), AppErrorKind::Serialization).with_source(error)
    })?;
    // Replace by rename so a failed or interrupted write keeps the previous settings.
    // Callers hold the host lock, so one staging name is enough.
    let path = config_dir.join(SETTINGS_FILE_NAME);
    let staged = config_dir.join(format!("{SETTINGS_FILE_NAME}.tmp"));
    std::fs::create_dir_all(config_dir)
        .and_then(|()| std::fs::write(&staged, content))
        .and_then(|()| std::fs::rename(&staged, &path))
        .map_err(|error| {
            AppError::new(
                format!("failed to save MCP settings: {error}"),
                AppErrorKind::Io,
            )
        })
}

// io::Error messages name the file path, never its content.
#[allow(clippy::needless_pass_by_value)]
fn token_error(error: std::io::Error) -> AppError {
    AppError::new(
        format!("failed to read or create the MCP token: {error}"),
        AppErrorKind::Io,
    )
}

pub async fn get_mcp_status(host: State<'_, McpHost>) -> Result<McpStatus, AppError> {
    let state = host.state.lock().await;
    ensure_loaded(&state)?;
    Ok(status(&state))
}

pub async fn set_mcp_settings(
    enabled: bool,
    port: u16,
    host: State<'_, McpHost>,
) -> Result<McpStatus, AppError> {
    // Port 0 would bind a random port that agent configuration cannot name.
    if port == 0 {
        return Err(AppError::new(
            "port must be between 1 and 65535",
            AppErrorKind::Validation,
        ));
    }
    let settings = McpSettings { enabled, port };
    let mut state = host.state.lock().await;
    ensure_loaded(&state)?;
    save_settings(&host.paths()?.config_dir, settings)?;
    state.settings = settings;
    host.restart(&mut state).await;
    Ok(status(&state))
}

/// Returns the bearer token so the user can copy it into an agent's configuration.
/// The frontend calls this only on an explicit reveal (ADR 0028). Token file access holds
/// the host lock so it never overlaps a rotation.
pub async fn reveal_mcp_token(host: State<'_, McpHost>) -> Result<String, AppError> {
    let state = host.state.lock().await;
    ensure_loaded(&state)?;
    load_or_create_token(&host.paths()?.config_dir).map_err(token_error)
}

/// Replaces the token. A running listener restarts so the old token stops working.
pub async fn rotate_mcp_token(host: State<'_, McpHost>) -> Result<McpStatus, AppError> {
    let mut state = host.state.lock().await;
    ensure_loaded(&state)?;
    rotate_token(&host.paths()?.config_dir).map_err(token_error)?;
    host.restart(&mut state).await;
    Ok(status(&state))
}
