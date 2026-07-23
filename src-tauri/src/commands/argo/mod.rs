mod applications;
mod appprojects;
mod appsets;
mod comparison;
mod connected;
mod operations;
mod scope;
mod transport;

use crate::{commands::gitops_crd, models::AppError};

pub use applications::{get_argocd_application_details, list_argocd_applications};
pub use appprojects::{get_argocd_appproject_details, list_argocd_appprojects};
pub use appsets::{get_argocd_appset_details, list_argocd_appsets};
pub use comparison::get_argo_resource_comparison;
pub use connected::{
    connect_argo_server, disconnect_argo_server, discover_argo_servers, forget_argo_credential,
    get_argo_application_inspector, get_argo_application_resources, get_argo_connection_status,
    ArgoConnectionStore,
};
pub use operations::{preflight_argo_operation, run_argo_operation};

/// Detect if Argo CD CRDs are present in the cluster.
#[tauri::command]
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
