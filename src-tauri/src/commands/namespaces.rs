use crate::commands::{
    diagnostic_field,
    helpers::{k8s_creation_timestamp_to_rfc3339, list_params},
    kubeconfig::{kubeconfig_source_key, KubeconfigSource},
    record_backend_error, record_backend_success, ClusterLiveStore,
};
use crate::models::{AppError, NamespaceSummary};
use crate::runtime::State;
use chrono::{DateTime, Utc};
use kube::api::Api;
use std::time::Instant;

fn namespace_age_at(creation_timestamp: Option<DateTime<Utc>>, now: DateTime<Utc>) -> String {
    match creation_timestamp {
        Some(t) => {
            let duration = now.signed_duration_since(t);
            if duration.num_days() > 0 {
                format!("{}d", duration.num_days())
            } else if duration.num_hours() > 0 {
                format!("{}h", duration.num_hours())
            } else if duration.num_minutes() > 0 {
                format!("{}m", duration.num_minutes())
            } else {
                "<1m".to_string()
            }
        }
        None => "unknown".to_string(),
    }
}

pub(crate) fn namespace_summary_from_at(
    namespace: &k8s_openapi::api::core::v1::Namespace,
    now: DateTime<Utc>,
) -> crate::models::NamespaceSummary {
    crate::models::NamespaceSummary {
        name: namespace.metadata.name.clone().unwrap_or_default(),
        age: namespace_age_at(
            namespace
                .metadata
                .creation_timestamp
                .as_ref()
                .and_then(|timestamp| k8s_openapi_time_to_datetime(&timestamp.0)),
            now,
        ),
        created_at: k8s_creation_timestamp_to_rfc3339(&namespace.metadata.creation_timestamp),
    }
}

pub async fn namespaces_summary_from(
    cluster_context: String,
    kubeconfig_env_var: Option<String>,
) -> Result<Vec<NamespaceSummary>, AppError> {
    let source = KubeconfigSource::new(kubeconfig_env_var)?;
    let client = source.client_for_context(&cluster_context).await?;
    let ns_api: Api<k8s_openapi::api::core::v1::Namespace> = Api::all(client);

    let namespaces = ns_api.list(&list_params()).await.map_err(AppError::from)?;

    let summaries: Vec<NamespaceSummary> = namespaces
        .iter()
        .map(|ns| namespace_summary_from_at(ns, Utc::now()))
        .collect();

    Ok(summaries)
}

fn k8s_openapi_time_to_datetime(timestamp: &k8s_openapi::jiff::Timestamp) -> Option<DateTime<Utc>> {
    DateTime::from_timestamp(timestamp.as_second(), 0)
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn list_namespaces(
    cluster_context: String,
    kubeconfig_env_var: Option<String>,
    live_store: State<'_, ClusterLiveStore>,
) -> Result<Vec<NamespaceSummary>, AppError> {
    let started = Instant::now();
    eprintln!("[kubecove:backend] list_namespaces start context={cluster_context}");
    let source_key = kubeconfig_source_key(kubeconfig_env_var.as_deref())?;
    let result = live_store
        .namespaces(source_key, cluster_context.clone(), {
            let cluster_context = cluster_context.clone();
            let kubeconfig_env_var = kubeconfig_env_var.clone();
            move || namespaces_summary_from(cluster_context, kubeconfig_env_var)
        })
        .await;
    match &result {
        Ok(rows) => {
            eprintln!(
                "[kubecove:backend] list_namespaces done context={} rows={} ms={}",
                cluster_context,
                rows.len(),
                started.elapsed().as_millis()
            );
            record_backend_success(
                "list_namespaces",
                started,
                vec![diagnostic_field("rows", rows.len())],
            );
        }
        Err(err) => {
            eprintln!(
                "[kubecove:backend] list_namespaces error context={} kind={} message={} ms={}",
                cluster_context,
                err.kind,
                err.message,
                started.elapsed().as_millis()
            );
            record_backend_error("list_namespaces", started, err.kind.as_str());
        }
    }
    result
}
