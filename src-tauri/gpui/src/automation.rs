//! The E2E flavor's agent automation endpoint (ADR 0035). Startup checks the E2E
//! environment before anything else runs; the endpoint then serves MCP on its own
//! loopback listener and hands each tool call to the UI thread.

mod tools;
pub mod ui;

use gpui_kit::{AnyWindowHandle, App, AppContext as _};
use kubecove_lib::models::{AppError, AppErrorKind};
use kubecove_mcp::{EndpointPolicy, McpEndpoint, serve_loopback, write_token_file};
use std::{
    error::Error,
    net::SocketAddr,
    path::{Path, PathBuf},
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

const PORT_ENV: &str = "KUBECOVE_AUTOMATION_PORT";
/// The bearer token file in the data directory, replaced at every launch.
pub const TOKEN_FILE_NAME: &str = "automation-token";

/// A checked E2E environment: the pinned kubeconfig loads and every path and the port
/// are valid. Nothing has been written yet.
pub struct Startup {
    data_dir: PathBuf,
    port: u16,
}

impl Startup {
    /// Requires `KUBECOVE_E2E=1`, the pinned kubeconfig and data directory, and
    /// `KUBECOVE_AUTOMATION_PORT`.
    pub fn from_env() -> Result<Self, AppError> {
        let pinned = kubecove_lib::e2e::pinned_config()?;
        let port = std::env::var(PORT_ENV)
            .ok()
            .and_then(|value| value.trim().parse::<u16>().ok())
            .ok_or_else(|| {
                AppError::new(
                    format!("{PORT_ENV} must be a port number; 0 picks a free port"),
                    AppErrorKind::Validation,
                )
            })?;
        Ok(Self {
            data_dir: pinned.data_dir,
            port,
        })
    }

    /// Settings, preferences and workspaces live here instead of the user's config.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Writes a new token and binds the listener. Fails before any window opens when the
    /// port cannot be bound.
    pub fn listen(self) -> Result<Endpoint, Box<dyn Error>> {
        let token_path = self.data_dir.join(TOKEN_FILE_NAME);
        let token = write_token_file(&token_path)?;
        let (jobs, requests) = mpsc::unbounded_channel();
        // The listener lives as long as the process; quitting the app ends it.
        let shutdown = CancellationToken::new();
        let endpoint = McpEndpoint::with_handler(
            tools::AutomationTools::new(jobs),
            token,
            EndpointPolicy::loopback(),
            shutdown.clone(),
        );
        let port = self.port;
        let (bound, address) = std::sync::mpsc::channel::<std::io::Result<SocketAddr>>();
        std::thread::Builder::new()
            .name("kubecove-automation".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = bound.send(Err(error));
                        return;
                    }
                };
                runtime.block_on(async move {
                    match serve_loopback(port, endpoint, shutdown).await {
                        Ok((address, task)) => {
                            let _ = bound.send(Ok(address));
                            let _ = task.await;
                        }
                        Err(error) => {
                            let _ = bound.send(Err(error));
                        }
                    }
                });
            })?;
        let address = address
            .recv()
            .map_err(|_| "the automation listener stopped before binding")??;
        println!(
            "[kubecove:automation] MCP endpoint http://{address}/mcp; bearer token in {}",
            token_path.display()
        );
        Ok(Endpoint { requests })
    }
}

/// Tool calls waiting for the window.
pub struct Endpoint {
    requests: mpsc::UnboundedReceiver<tools::Job>,
}

impl Endpoint {
    /// Performs each call on `window` on the UI thread, in arrival order.
    pub fn attach(mut self, window: AnyWindowHandle, cx: &mut App) {
        cx.spawn(async move |cx| {
            let mut references = ui::References::default();
            while let Some(job) = self.requests.recv().await {
                let result = cx
                    .update_window(window, |_, window, cx| {
                        ui::perform(job.operation, &mut references, window, cx)
                    })
                    .unwrap_or_else(|error| Err(format!("the KubeCove window is gone: {error}")));
                let _ = job.reply.send(result);
            }
        })
        .detach();
    }
}
