use crate::models::{AppError, ClusterContext};
use kube::config::Kubeconfig;

pub fn get_cluster_contexts() -> Result<Vec<ClusterContext>, AppError> {
    let kubeconfig = Kubeconfig::read().map_err(|e: kube::config::KubeconfigError| AppError::kube(e.to_string()))?;

    let cluster_contexts: Vec<ClusterContext> = kubeconfig
        .contexts
        .iter()
        .map(|ctx| ClusterContext {
            name: ctx.name.clone(),
        })
        .collect();

    Ok(cluster_contexts)
}

#[tauri::command]
pub fn list_kube_contexts() -> Result<Vec<ClusterContext>, AppError> {
    get_cluster_contexts()
}