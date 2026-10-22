use crate::{
    commands::{
        diagnostic_field,
        kubeconfig::{cluster_contexts_from_source, KubeconfigSource},
        FiniteRead,
    },
    models::{AppError, ClusterContext},
};

pub fn get_cluster_contexts(
    kubeconfig_env_var: Option<String>,
) -> Result<Vec<ClusterContext>, AppError> {
    let source = KubeconfigSource::new(kubeconfig_env_var)?;
    cluster_contexts_from_source(&source)
}

pub fn list_kube_contexts(
    kubeconfig_env_var: Option<String>,
) -> Result<Vec<ClusterContext>, AppError> {
    FiniteRead::new("list_kube_contexts").run_sync(
        || get_cluster_contexts(kubeconfig_env_var),
        |rows| vec![diagnostic_field("rows", rows.len())],
    )
}
