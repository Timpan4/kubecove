use super::{
    summary_cluster::cluster_resource_summaries, summary_core::core_resource_summaries,
    summary_workloads::workload_resource_summaries,
};
use crate::models::{AppError, ResourceSummary};
use kube::{config::KubeConfigOptions, Client};
use std::time::Instant;

pub async fn resources_summary_from(
    cluster_context: String,
    kind: String,
    namespace: Option<String>,
) -> Result<Vec<ResourceSummary>, AppError> {
    let options = KubeConfigOptions {
        context: Some(cluster_context.clone()),
        ..Default::default()
    };

    let config = kube::Config::from_kubeconfig(&options)
        .await
        .map_err(|e| AppError::kube(e.to_string()))?;
    let client = Client::try_from(config).map_err(|e| AppError::kube(e.to_string()))?;

    if let Some(rows) = core_resource_summaries(
        client.clone(),
        &cluster_context,
        &kind,
        namespace.as_deref(),
    )
    .await?
    {
        return Ok(rows);
    }
    if let Some(rows) = workload_resource_summaries(
        client.clone(),
        &cluster_context,
        &kind,
        namespace.as_deref(),
    )
    .await?
    {
        return Ok(rows);
    }
    if let Some(rows) = cluster_resource_summaries(client, &cluster_context, &kind).await? {
        return Ok(rows);
    }

    Err(AppError::new(
        format!("unsupported resource kind: {}", kind),
        "cluster",
    ))
}

#[tauri::command]
pub async fn list_resources(
    cluster_context: String,
    kind: String,
    namespace: Option<String>,
) -> Result<Vec<ResourceSummary>, AppError> {
    let started = Instant::now();
    let namespace_label = namespace.as_deref().unwrap_or("<all>");
    eprintln!(
        "[kubecove:backend] list_resources start context={} kind={} namespace={}",
        cluster_context, kind, namespace_label
    );
    let result =
        resources_summary_from(cluster_context.clone(), kind.clone(), namespace.clone()).await;
    match &result {
        Ok(rows) => eprintln!("[kubecove:backend] list_resources done context={} kind={} namespace={} rows={} ms={}", cluster_context, kind, namespace_label, rows.len(), started.elapsed().as_millis()),
        Err(err) => eprintln!("[kubecove:backend] list_resources error context={} kind={} namespace={} error_kind={} message={} ms={}", cluster_context, kind, namespace_label, err.kind, err.message, started.elapsed().as_millis()),
    }
    result
}
