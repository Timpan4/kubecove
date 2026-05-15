mod applications;
mod appprojects;
mod appsets;

use crate::models::AppError;
use kube::{config::KubeConfigOptions, discovery::Discovery, Client};

pub use applications::{get_argocd_application_details, list_argocd_applications};
pub use appprojects::{get_argocd_appproject_details, list_argocd_appprojects};
pub use appsets::{get_argocd_appset_details, list_argocd_appsets};

/// Detect if Argo CD CRDs are present in the cluster.
#[tauri::command]
pub async fn detect_argocd(cluster_context: String) -> Result<bool, AppError> {
    let options = KubeConfigOptions {
        context: Some(cluster_context.clone()),
        ..Default::default()
    };

    let config = kube::Config::from_kubeconfig(&options)
        .await
        .map_err(|e| AppError::kube(e.to_string()))?;

    let client = Client::try_from(config).map_err(|e| AppError::kube(e.to_string()))?;

    // Check specifically for the Application CRD in argoproj.io group
    match find_api_resource(&client, "argoproj.io", "Application").await {
        Ok(Some(_)) => Ok(true),
        Ok(None) => Ok(false),
        Err(err) => Err(err),
    }
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
