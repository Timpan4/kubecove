use crate::commands::{PodExecRegistry, PortForwardRegistry};
use crate::models::AppErrorKind;
use crate::models::{AppError, LiveSessionCleanupRequest, LiveSessionCleanupResult};
use crate::runtime::State;
use std::collections::HashSet;

/// The workspace scope that port-forwards and Pod exec sessions stop outside
/// of when live sessions are not kept across workspace switches (ADR 0003).
pub struct LiveSessionScope {
    allowed_cluster_contexts: HashSet<String>,
    kubeconfig_source_key: String,
}

impl LiveSessionScope {
    pub fn new(request: LiveSessionCleanupRequest) -> Result<Self, AppError> {
        let kubeconfig_source_key = request.kubeconfig_source_key.trim();
        if kubeconfig_source_key.is_empty() {
            return Err(AppError::new(
                "kubeconfig source key is required",
                AppErrorKind::Validation,
            ));
        }
        Ok(Self {
            kubeconfig_source_key: kubeconfig_source_key.to_string(),
            allowed_cluster_contexts: request
                .allowed_cluster_contexts
                .into_iter()
                .map(|context| context.trim().to_string())
                .filter(|context| !context.is_empty())
                .collect(),
        })
    }

    /// A session is inside only when both its context and its kubeconfig
    /// source match. A session without a recorded source is outside every scope.
    pub fn contains(&self, cluster_context: &str, kubeconfig_source_key: Option<&str>) -> bool {
        self.allowed_cluster_contexts.contains(cluster_context)
            && kubeconfig_source_key == Some(self.kubeconfig_source_key.as_str())
    }
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn stop_live_sessions_outside_scope(
    request: LiveSessionCleanupRequest,
    port_forwards: State<'_, PortForwardRegistry>,
    pod_execs: State<'_, PodExecRegistry>,
) -> Result<LiveSessionCleanupResult, AppError> {
    let scope = LiveSessionScope::new(request)?;
    let stopped_port_forward_ids = port_forwards.stop_outside_scope(&scope);
    let stopped_pod_exec_ids = pod_execs.stop_outside_scope(&scope);

    Ok(LiveSessionCleanupResult {
        stopped_port_forwards: stopped_port_forward_ids.len(),
        stopped_pod_exec_sessions: stopped_pod_exec_ids.len(),
        stopped_port_forward_ids,
        stopped_pod_exec_ids,
    })
}
