use super::{
    summary_cluster::cluster_resource_summaries, summary_core::core_resource_summaries,
    summary_workloads::workload_resource_summaries,
};
use crate::commands::{diagnostic_field, ClusterAccess, ClusterLiveStore, FiniteRead};
use crate::models::AppErrorKind;
use crate::models::{AppError, ResourceSummary};
use crate::runtime::State;
use std::sync::Arc;

pub async fn resources_summary_from(
    access: &ClusterAccess,
    cluster_context: String,
    kind: String,
    namespace: Option<String>,
) -> Result<Vec<ResourceSummary>, AppError> {
    crate::commands::helpers::validate_namespace(namespace.as_deref())?;
    let client = access.read_client(&cluster_context).await?;
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
        format!("unsupported resource kind: {kind}"),
        AppErrorKind::Cluster,
    ))
}

pub async fn list_resources(
    cluster_context: String,
    kind: String,
    namespace: Option<String>,
    kubeconfig_env_var: Option<String>,
    live_store: State<'_, ClusterLiveStore>,
) -> Result<Arc<Vec<ResourceSummary>>, AppError> {
    FiniteRead::new("list_resources")
        .label("context", &cluster_context)
        .label("kind", &kind)
        .label("namespace", namespace.as_deref().unwrap_or("<all>"))
        .run(
            async {
                let access = ClusterAccess::resolve(kubeconfig_env_var.as_deref())?;
                live_store
                    .typed_resources(
                        access.source_key().to_string(),
                        cluster_context.clone(),
                        kind.clone(),
                        namespace.clone(),
                        {
                            let kind = kind.clone();
                            move || async move {
                                resources_summary_from(&access, cluster_context, kind, namespace)
                                    .await
                            }
                        },
                    )
                    .await
            },
            |rows| {
                vec![
                    diagnostic_field("kind", &kind),
                    diagnostic_field("rows", rows.len()),
                ]
            },
        )
        .await
}
