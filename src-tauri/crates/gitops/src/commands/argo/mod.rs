mod applications;
mod appprojects;
mod appsets;
mod comparison;
mod connected;
mod credential_store;
mod operations;
mod scope;
mod session;
mod transport;
mod tunnel;

#[cfg(feature = "bench-support")]
pub use applications::benchmark_application_summary_from_object;
#[cfg(feature = "bench-support")]
pub use appprojects::benchmark_app_project_summary_from_object;
#[cfg(feature = "bench-support")]
pub use appsets::benchmark_application_set_summary_from_object;
#[cfg(feature = "bench-support")]
pub use comparison::{
    benchmark_connected_selected_comparison, benchmark_kubernetes_selected_comparison,
};
#[cfg(feature = "bench-support")]
pub use connected::{benchmark_connected_inspector, benchmark_kubernetes_inspector};
#[cfg(feature = "bench-support")]
pub use operations::benchmark_verify_kubernetes_application_identity;

use crate::{commands::gitops_crd, models::AppError};

pub use applications::{get_argocd_application_details, list_argocd_applications};
pub use appprojects::{get_argocd_appproject_details, list_argocd_appprojects};
pub use appsets::{get_argocd_appset_details, list_argocd_appsets};
pub use comparison::get_argo_resource_comparison;
pub use connected::{
    connect_argo_server, disconnect_argo_server, discover_argo_servers, forget_argo_credential,
    get_argo_application_inspector, get_argo_connection_status, ArgoConnectionStore,
};
#[cfg(not(feature = "desktop"))]
pub use credential_store::{
    active_credential_store_description, initialize_server_credential_store,
    CredentialStoreDescription,
};
pub use operations::{preflight_argo_operation, run_argo_operation};

/// Detect if Argo CD CRDs are present in the cluster.
#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn detect_argocd(
    cluster_context: String,
    kubeconfig_env_var: Option<String>,
) -> Result<bool, AppError> {
    let client = gitops_crd::client_for_context(&cluster_context, kubeconfig_env_var).await?;
    // Check specifically for the Application CRD in argoproj.io group
    match gitops_crd::find_api_resource(&client, "argoproj.io", "Application").await {
        Ok(Some(_)) => Ok(true),
        Ok(None) => Ok(false),
        Err(err) => Err(err),
    }
}
