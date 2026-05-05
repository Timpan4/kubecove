use crate::models::{AppError, ClusterContext, NamespaceSummary};
use chrono::{TimeZone, Utc};
use k8s_openapi::api::core::v1::Namespace;
use kube::config::{Config, KubeConfigOptions, Kubeconfig};
use kube::{Api, Client, Resource, ResourceExt};

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

pub fn namespaces_summary_from(namespaces: &[Namespace]) -> Vec<NamespaceSummary> {
    namespaces
        .iter()
        .map(|ns| NamespaceSummary {
            name: ns.name_any(),
            status: ns.status.as_ref().and_then(|s| s.phase.clone()),
            age: ns.meta().creation_timestamp.as_ref().map(|t| {
                let diff = Utc::now().signed_duration_since(Utc.timestamp_opt(t.0.as_second(), 0).unwrap());
                if diff.num_seconds() < 60 {
                    format!("{}s", diff.num_seconds())
                } else if diff.num_minutes() < 60 {
                    format!("{}m", diff.num_minutes())
                } else if diff.num_hours() < 24 {
                    format!("{}h", diff.num_hours())
                } else {
                    format!("{}d", diff.num_days())
                }
            }),
        })
        .collect()
}

#[tauri::command]
pub async fn list_namespaces(cluster_context: String) -> Result<Vec<NamespaceSummary>, AppError> {
    let config = Config::from_kubeconfig(&KubeConfigOptions {
        context: Some(cluster_context),
        ..Default::default()
    })
    .await
    .map_err(|e| AppError::KubeConfig(e.to_string()))?;

    let client = Client::try_from(config).map_err(|e| AppError::Kube(e.to_string()))?;

    let ns_api: Api<Namespace> = Api::all(client);
    let namespace_list = ns_api.list(&Default::default()).await.map_err(|e| AppError::Kube(e.to_string()))?;

    Ok(namespaces_summary_from(namespace_list.items.as_slice()))
}
