use crate::models::{AppError, ClusterContext};
use kube::config::Kubeconfig;

#[tauri::command]
pub async fn list_kube_contexts() -> Result<Vec<ClusterContext>, AppError> {
    let kubeconfig = Kubeconfig::read().map_err(|error| AppError::KubeConfig(error.to_string()))?;

    Ok(cluster_contexts_from_kubeconfig(&kubeconfig))
}

pub fn cluster_contexts_from_kubeconfig(kubeconfig: &Kubeconfig) -> Vec<ClusterContext> {
    kubeconfig
        .contexts
        .iter()
        .map(|named_context| ClusterContext {
            name: named_context.name.clone(),
            cluster: named_context
                .context
                .as_ref()
                .map(|context| context.cluster.clone()),
            user: named_context
                .context
                .as_ref()
                .and_then(|context| context.user.clone()),
            namespace: named_context
                .context
                .as_ref()
                .and_then(|context| context.namespace.clone()),
        })
        .collect()
}
