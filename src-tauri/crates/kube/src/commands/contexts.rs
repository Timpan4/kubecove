use crate::{
    commands::{diagnostic_field, ClusterAccess, FiniteRead},
    models::{AppError, ClusterContext},
};

pub fn list_kube_contexts(
    kubeconfig_env_var: Option<String>,
) -> Result<Vec<ClusterContext>, AppError> {
    FiniteRead::new("list_kube_contexts").run_sync(
        || ClusterAccess::resolve(kubeconfig_env_var.as_deref())?.contexts(),
        |rows| vec![diagnostic_field("rows", rows.len())],
    )
}
