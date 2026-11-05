//! Host composition and the stable command API used by desktop and server.

#[cfg(feature = "bench-support")]
pub mod bench_support;

#[cfg(feature = "desktop")]
pub use crate::mcp::{
    McpHost, get_mcp_status, reveal_mcp_token, rotate_mcp_token, set_mcp_settings,
};
pub use kubecove_gitops::commands::*;
#[cfg(feature = "desktop")]
pub use kubecove_host::remote::{
    RemoteConnectionStore, remote_connect, remote_connection_status, remote_disconnect,
    remote_invoke, remote_subscribe_channel, remote_unsubscribe_channel,
};
#[cfg(feature = "bench-support")]
pub(crate) use kubecove_host::usage;
pub use kubecove_host::usage::{AppUsageMonitor, get_app_usage_metrics};
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
pub async fn cancel_workspace_requests(
    registry: State<'_, BackendCancellationRegistry>,
    live_store: State<'_, ClusterLiveStore>,
    argo_connections: State<'_, ArgoConnectionStore>,
) -> Result<CancelWorkspaceRequestsResult, AppError> {
    let cancelled_requests = registry.cancel_all();
    let client_generation = helpers::client_cache::rotate_client_generation();
    let cancelled_loads = live_store.cancel_loading();
    argo_connections.close_all().await?;
    // After a 10k-Pod workspace closed, the desktop host's allocator held 478 MiB
    // of freed response buffers against 73 MiB in use.
    let _ = tokio::task::spawn_blocking(release_freed_heap).await;
    Ok(CancelWorkspaceRequestsResult {
        cancelled_requests,
        cancelled_loads,
        client_generation,
    })
}

/// glibc keeps freed pages in its per-thread arenas for the life of the process,
/// so without this the host stays at its peak after large responses are freed.
pub fn release_freed_heap() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    // SAFETY: malloc_trim takes no pointers and only releases glibc's own free pages.
    unsafe {
        libc::malloc_trim(0);
    }
}
