//! Host composition and the stable command API used by desktop and server.

#[cfg(feature = "bench-support")]
pub mod bench_support;

pub use kubecove_gitops::commands::*;
#[cfg(feature = "desktop")]
pub use kubecove_host::remote::{
    remote_connect, remote_connection_status, remote_disconnect, remote_invoke,
    remote_subscribe_channel, remote_unsubscribe_channel, RemoteConnectionStore,
};
#[cfg(feature = "bench-support")]
pub(crate) use kubecove_host::usage;
pub use kubecove_host::usage::{get_app_usage_metrics, AppUsageMonitor};
#[cfg(feature = "desktop")]
pub use kubecove_host::workspace_files::{pick_workspace_import_json, save_workspace_export_json};
#[cfg(feature = "desktop")]
pub use kubecove_host::{remote, workspace_files};
pub use kubecove_inspection::commands::*;
pub use kubecove_kube::commands::*;
pub use kubecove_live::commands::*;

use crate::{
    models::{AppError, CancelWorkspaceRequestsResult},
    runtime::State,
};

// Workspace changes coordinate infrastructure and GitOps without a back-edge
// from client cancellation into Argo connection management.
#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn cancel_workspace_requests(
    registry: State<'_, BackendCancellationRegistry>,
    live_store: State<'_, ClusterLiveStore>,
    argo_connections: State<'_, ArgoConnectionStore>,
) -> Result<CancelWorkspaceRequestsResult, AppError> {
    let cancelled_requests = registry.cancel_all();
    let client_generation = helpers::client_cache::rotate_client_generation();
    let cancelled_loads = live_store.cancel_loading();
    argo_connections.close_all().await?;
    Ok(CancelWorkspaceRequestsResult {
        cancelled_requests,
        cancelled_loads,
        client_generation,
    })
}
