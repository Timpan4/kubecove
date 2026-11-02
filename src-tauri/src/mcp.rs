//! Desktop MCP listener lifecycle (ADR 0028): its settings file, bearer token and the
//! loopback listener. MCP is off until the user enables it in Settings.

use crate::models::{AppError, AppErrorKind, McpStatus};
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
use tauri::State;
use tokio::{sync::Mutex, task::JoinHandle};
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

#[derive(Default)]
struct HostState {
    settings: McpSettings,
    listener: Option<Listener>,
    error: Option<String>,
}

struct Paths {
    config_dir: PathBuf,
    live_store: ClusterLiveStore,
}

/// Managed state for the desktop MCP listener.
#[derive(Default)]
pub struct McpHost {
    paths: OnceLock<Paths>,
    state: Mutex<HostState>,
}

impl McpHost {
    /// Loads saved settings and starts the listener when MCP was left enabled. Tools read
    /// through `live_store`, the same caches the app's views use.
    pub async fn start(&self, config_dir: PathBuf, live_store: ClusterLiveStore) {
        let settings = load_settings(&config_dir);
        if self
            .paths
            .set(Paths {
                config_dir,
                live_store,
            })
            .is_err()
        {
            return;
        }
        let mut state = self.state.lock().await;
        state.settings = settings;
        self.restart(&mut state).await;
    }

    fn paths(&self) -> Result<&Paths, AppError> {
        self.paths
            .get()
            .ok_or_else(|| AppError::new("MCP is not initialized", AppErrorKind::Internal))
    }

    async fn restart(&self, state: &mut HostState) {
        if let Some(listener) = state.listener.take() {
            listener.shutdown.cancel();
            // Rebinding the same port must wait until the old listener is closed.
            let _ = listener.task.await;
        }
        state.error = None;
        if !state.settings.enabled {
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

fn load_settings(config_dir: &Path) -> McpSettings {
    std::fs::read_to_string(config_dir.join(SETTINGS_FILE_NAME))
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

fn save_settings(config_dir: &Path, settings: McpSettings) -> Result<(), AppError> {
    let content = serde_json::to_string_pretty(&settings).map_err(|error| {
        AppError::new(error.to_string(), AppErrorKind::Serialization).with_source(error)
    })?;
    std::fs::create_dir_all(config_dir)
        .and_then(|()| std::fs::write(config_dir.join(SETTINGS_FILE_NAME), content))
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
    Ok(status(&*host.state.lock().await))
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
    save_settings(&host.paths()?.config_dir, settings)?;
    state.settings = settings;
    host.restart(&mut state).await;
    Ok(status(&state))
}

/// Returns the bearer token so the user can copy it into an agent's configuration.
/// The frontend calls this only on an explicit reveal (ADR 0028). Token file access holds
/// the host lock so it never overlaps a rotation.
pub async fn reveal_mcp_token(host: State<'_, McpHost>) -> Result<String, AppError> {
    let _state = host.state.lock().await;
    load_or_create_token(&host.paths()?.config_dir).map_err(token_error)
}

/// Replaces the token. A running listener restarts so the old token stops working.
pub async fn rotate_mcp_token(host: State<'_, McpHost>) -> Result<McpStatus, AppError> {
    let mut state = host.state.lock().await;
    rotate_token(&host.paths()?.config_dir).map_err(token_error)?;
    host.restart(&mut state).await;
    Ok(status(&state))
}
