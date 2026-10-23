use crate::commands::kubeconfig::KubeconfigSource;
use crate::models::{AppError, ClusterOperationTarget};
use kube::api::PatchParams;

/// Previews must never persist: every preview request goes to the API server
/// with `dryRun=All`. Centralized here so tests can pin the behavior.
pub(super) fn preview_patch_params() -> PatchParams {
    PatchParams::default().dry_run()
}

pub(super) async fn operation_client_for(
    target: &ClusterOperationTarget,
    kubeconfig_env_var: Option<String>,
) -> Result<kube::Client, AppError> {
    KubeconfigSource::new(kubeconfig_env_var)?
        .fail_closed()
        .operation_client_for_context(&target.cluster_context)
        .await
}

pub(super) async fn client_for(
    target: &ClusterOperationTarget,
    kubeconfig_env_var: Option<String>,
) -> Result<kube::Client, AppError> {
    KubeconfigSource::new(kubeconfig_env_var)?
        .fail_closed()
        .client_for_context(&target.cluster_context)
        .await
}
