use crate::{
    commands::kubeconfig::{cluster_contexts_from_source, KubeconfigSource},
    models::{AppError, ClusterContext},
};

pub fn get_cluster_contexts(
    kubeconfig_env_var: Option<String>,
) -> Result<Vec<ClusterContext>, AppError> {
    let source = KubeconfigSource::new(kubeconfig_env_var)?;
    cluster_contexts_from_source(&source)
}

#[tauri::command]
pub fn list_kube_contexts(
    kubeconfig_env_var: Option<String>,
) -> Result<Vec<ClusterContext>, AppError> {
    get_cluster_contexts(kubeconfig_env_var)
}
