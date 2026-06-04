mod applications;
mod appprojects;
mod appsets;

use crate::{commands::kubeconfig::KubeconfigSource, models::AppError};
use kube::{discovery::Discovery, Client};

pub use applications::{get_argocd_application_details, list_argocd_applications};
pub use appprojects::{get_argocd_appproject_details, list_argocd_appprojects};
pub use appsets::{get_argocd_appset_details, list_argocd_appsets};

/// Detect if Argo CD CRDs are present in the cluster.
#[tauri::command]
pub async fn detect_argocd(
    cluster_context: String,
    kubeconfig_env_var: Option<String>,
) -> Result<bool, AppError> {
    let client = client_for_context(&cluster_context, kubeconfig_env_var).await?;
    // Check specifically for the Application CRD in argoproj.io group
    match find_api_resource(&client, "argoproj.io", "Application").await {
        Ok(Some(_)) => Ok(true),
        Ok(None) => Ok(false),
        Err(err) => Err(err),
    }
}

pub(crate) async fn client_for_context(
    cluster_context: &str,
    kubeconfig_env_var: Option<String>,
) -> Result<Client, AppError> {
    let source = KubeconfigSource::new(kubeconfig_env_var)?;
    source.client_for_context(cluster_context).await
}

/// Find API resource for a given kind in the cluster discovery.
pub(crate) async fn find_api_resource(
    client: &Client,
    group: &str,
    kind: &str,
) -> Result<Option<kube::discovery::ApiResource>, AppError> {
    let discovery = Discovery::new(client.clone())
        .run_aggregated()
        .await
        .map_err(|e| AppError::kube(e.to_string()))?;

    for g in discovery.groups() {
        if g.name() == group {
            for (ar, _) in g.recommended_resources() {
                if ar.kind == kind {
                    return Ok(Some(ar));
                }
            }
        }
    }
    Ok(None)
}
