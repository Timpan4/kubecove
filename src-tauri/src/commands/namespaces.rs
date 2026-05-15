use crate::commands::helpers::{k8s_creation_timestamp_to_rfc3339, list_params};
use crate::models::{AppError, NamespaceSummary};
use chrono::{DateTime, TimeZone, Utc};
use kube::{api::Api, config::KubeConfigOptions, Client};
use std::time::Instant;

fn namespace_age(creation_timestamp: Option<DateTime<Utc>>) -> String {
    match creation_timestamp {
        Some(t) => {
            let now = Utc::now();
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

pub async fn namespaces_summary_from(
    cluster_context: String,
) -> Result<Vec<NamespaceSummary>, AppError> {
    let options = KubeConfigOptions {
        context: Some(cluster_context.clone()),
        ..Default::default()
    };

    let config = kube::Config::from_kubeconfig(&options)
        .await
        .map_err(|e| AppError::kube(e.to_string()))?;

    let client = Client::try_from(config).map_err(|e| AppError::kube(e.to_string()))?;
    let ns_api: Api<k8s_openapi::api::core::v1::Namespace> = Api::all(client);

    let namespaces = ns_api
        .list(&list_params())
        .await
        .map_err(|e| AppError::kube(e.to_string()))?;

    let summaries: Vec<NamespaceSummary> = namespaces
        .iter()
        .map(|ns| NamespaceSummary {
            name: ns.metadata.name.clone().unwrap_or_default(),
            age: namespace_age(ns.metadata.creation_timestamp.clone().map(|t| {
                Utc.timestamp_opt(t.0.as_second(), 0)
                    .single()
                    .unwrap_or_else(Utc::now)
            })),
            created_at: k8s_creation_timestamp_to_rfc3339(&ns.metadata.creation_timestamp),
        })
        .collect();

    Ok(summaries)
}

#[tauri::command]
pub async fn list_namespaces(cluster_context: String) -> Result<Vec<NamespaceSummary>, AppError> {
    let started = Instant::now();
    eprintln!(
        "[k8s-manager:backend] list_namespaces start context={}",
        cluster_context
    );
    let result = namespaces_summary_from(cluster_context.clone()).await;
    match &result {
        Ok(rows) => eprintln!(
            "[k8s-manager:backend] list_namespaces done context={} rows={} ms={}",
            cluster_context,
            rows.len(),
            started.elapsed().as_millis()
        ),
        Err(err) => eprintln!(
            "[k8s-manager:backend] list_namespaces error context={} kind={} message={} ms={}",
            cluster_context,
            err.kind,
            err.message,
            started.elapsed().as_millis()
        ),
    }
    result
}
