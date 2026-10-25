use crate::commands::{
    diagnostic_field,
    helpers::{
        k8s_creation_timestamp_to_rfc3339, k8s_timestamp_to_datetime, list_params, resource_age_at,
    },
    ClusterAccess, ClusterLiveStore, FiniteRead,
};
use crate::models::{AppError, NamespaceSummary};
use crate::runtime::State;
use chrono::{DateTime, Utc};
use kube::api::Api;
use std::sync::Arc;

pub fn namespace_summary_from_at(
    namespace: &k8s_openapi::api::core::v1::Namespace,
    now: DateTime<Utc>,
) -> crate::models::NamespaceSummary {
    crate::models::NamespaceSummary {
        name: namespace.metadata.name.clone().unwrap_or_default(),
        age: namespace
            .metadata
            .creation_timestamp
            .as_ref()
            .and_then(|timestamp| k8s_timestamp_to_datetime(&timestamp.0))
            .map_or_else(|| "unknown".to_string(), |time| resource_age_at(time, now)),
        created_at: k8s_creation_timestamp_to_rfc3339(&namespace.metadata.creation_timestamp),
    }
}

pub async fn namespaces_summary_from(
    access: &ClusterAccess,
    cluster_context: &str,
) -> Result<Vec<NamespaceSummary>, AppError> {
    let client = access.read_client(cluster_context).await?;
    let ns_api: Api<k8s_openapi::api::core::v1::Namespace> = Api::all(client);

    let namespaces = ns_api.list(&list_params()).await.map_err(AppError::from)?;

    let summaries: Vec<NamespaceSummary> = namespaces
        .iter()
        .map(|ns| namespace_summary_from_at(ns, Utc::now()))
        .collect();

    Ok(summaries)
}

pub async fn list_namespaces(
    cluster_context: String,
    kubeconfig_env_var: Option<String>,
    live_store: State<'_, ClusterLiveStore>,
) -> Result<Arc<Vec<NamespaceSummary>>, AppError> {
    FiniteRead::new("list_namespaces")
        .label("context", &cluster_context)
        .run(
            async move {
                let access = ClusterAccess::resolve(kubeconfig_env_var.as_deref())?;
                live_store
                    .namespaces(
                        access.source_key().to_string(),
                        cluster_context.clone(),
                        move || async move { namespaces_summary_from(&access, &cluster_context).await },
                    )
                    .await
            },
            |rows| vec![diagnostic_field("rows", rows.len())],
        )
        .await
}
