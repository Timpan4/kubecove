use crate::commands::gitops_crd::{
    client_for_context, find_api_resource, get_crd_object, list_crd_objects, resource_metadata,
    resource_yaml,
};
use crate::commands::helpers::{k8s_creation_timestamp_to_rfc3339, resource_age};
use crate::models::{
    AppError, ArgoApplicationSetDetails, ArgoApplicationSetSummary, YamlEncoding, YamlViewMode,
};
use chrono::{TimeZone, Utc};

/// List Argo CD `ApplicationSets` in the cluster.
#[tauri::command]
pub async fn list_argocd_appsets(
    cluster_context: String,
    kubeconfig_env_var: Option<String>,
) -> Result<Vec<ArgoApplicationSetSummary>, AppError> {
    let client = client_for_context(&cluster_context, kubeconfig_env_var).await?;

    let ar = match find_api_resource(&client, "argoproj.io", "ApplicationSet").await? {
        Some(ar) => ar,
        None => return Ok(vec![]),
    };

    let items = list_crd_objects(client.clone(), &ar).await?;

    let summaries: Vec<ArgoApplicationSetSummary> = items
        .iter()
        .filter_map(|obj| {
            let name = obj.metadata.name.clone().unwrap_or_default();
            let namespace = obj.metadata.namespace.clone();
            let age = resource_age(obj.metadata.creation_timestamp.clone().map(|t| {
                Utc.timestamp_opt(t.0.as_second(), 0)
                    .single()
                    .unwrap_or_else(Utc::now)
            }));
            let data = obj.data.as_object()?;

            let project = data
                .get("spec")
                .and_then(|s| s.get("project"))
                .and_then(|p| p.as_str())
                .map(String::from);
            let status = data
                .get("status")
                .and_then(|s| s.get("conditions"))
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("type"))
                .and_then(|t| t.as_str())
                .map(String::from);
            let sync_status = data
                .get("status")
                .and_then(|s| s.get("sync"))
                .and_then(|s| s.get("status"))
                .and_then(|st| st.as_str())
                .map(String::from);
            let health_status = data
                .get("status")
                .and_then(|s| s.get("health"))
                .and_then(|h| h.get("status"))
                .and_then(|st| st.as_str())
                .map(String::from);
            let destination_namespace = data
                .get("spec")
                .and_then(|s| s.get("generatorParams"))
                .and_then(|arr| arr.as_array())
                .and_then(|a| a.first())
                .and_then(|p| p.get("dest-namespace"))
                .and_then(|n| n.as_str())
                .map(String::from);
            let destination_server = data
                .get("spec")
                .and_then(|s| s.get("generatorParams"))
                .and_then(|arr| arr.as_array())
                .and_then(|a| a.first())
                .and_then(|p| p.get("dest-server"))
                .and_then(|s| s.as_str())
                .map(String::from);
            let source_repo = data
                .get("spec")
                .and_then(|s| s.get("source"))
                .and_then(|s| s.get("repoURL"))
                .and_then(|r| r.as_str())
                .map(String::from);
            let source_revision = data
                .get("spec")
                .and_then(|s| s.get("source"))
                .and_then(|s| s.get("targetRevision"))
                .and_then(|r| r.as_str())
                .map(String::from);

            Some(ArgoApplicationSetSummary {
                cluster: cluster_context.clone(),
                name,
                age,
                created_at: k8s_creation_timestamp_to_rfc3339(&obj.metadata.creation_timestamp),
                namespace,
                project,
                status,
                sync_status,
                health_status,
                destination_namespace,
                destination_server,
                source_repo,
                source_revision,
            })
        })
        .collect();

    Ok(summaries)
}

/// Get detailed Argo CD `ApplicationSet` information including YAML and metadata.
#[tauri::command]
pub async fn get_argocd_appset_details(
    cluster_context: String,
    name: String,
    namespace: Option<String>,
    kubeconfig_env_var: Option<String>,
    yaml_view_mode: Option<YamlViewMode>,
    yaml_encoding: Option<YamlEncoding>,
) -> Result<ArgoApplicationSetDetails, AppError> {
    let client = client_for_context(&cluster_context, kubeconfig_env_var).await?;

    let ar = match find_api_resource(&client, "argoproj.io", "ApplicationSet").await? {
        Some(ar) => ar,
        None => return Err(AppError::new("ApplicationSet CRD not found", "cluster")),
    };

    let obj = get_crd_object(client.clone(), &ar, &name, namespace.as_deref()).await?;
    let yaml = resource_yaml(&obj, yaml_view_mode, yaml_encoding)?;
    let metadata = resource_metadata(&obj)?;
    let data = obj
        .data
        .as_object()
        .ok_or_else(|| AppError::new("invalid ApplicationSet data", "cluster"))?;

    let name = obj.metadata.name.clone().unwrap_or_default();
    let namespace = obj.metadata.namespace.clone();
    let age = resource_age(obj.metadata.creation_timestamp.clone().map(|t| {
        Utc.timestamp_opt(t.0.as_second(), 0)
            .single()
            .unwrap_or_else(Utc::now)
    }));
    let project = data
        .get("spec")
        .and_then(|s| s.get("project"))
        .and_then(|p| p.as_str())
        .map(String::from);
    let destination_namespace = data
        .get("spec")
        .and_then(|s| s.get("generatorParams"))
        .and_then(|arr| arr.as_array())
        .and_then(|a| a.first())
        .and_then(|p| p.get("dest-namespace"))
        .and_then(|n| n.as_str())
        .map(String::from);
    let destination_server = data
        .get("spec")
        .and_then(|s| s.get("generatorParams"))
        .and_then(|arr| arr.as_array())
        .and_then(|a| a.first())
        .and_then(|p| p.get("dest-server"))
        .and_then(|s| s.as_str())
        .map(String::from);
    let source_repo = data
        .get("spec")
        .and_then(|s| s.get("source"))
        .and_then(|s| s.get("repoURL"))
        .and_then(|r| r.as_str())
        .map(String::from);
    let source_revision = data
        .get("spec")
        .and_then(|s| s.get("source"))
        .and_then(|s| s.get("targetRevision"))
        .and_then(|r| r.as_str())
        .map(String::from);
    let status = data
        .get("status")
        .and_then(|s| s.get("conditions"))
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|c| c.get("type"))
        .and_then(|t| t.as_str())
        .map(String::from);
    let sync_status = data
        .get("status")
        .and_then(|s| s.get("sync"))
        .and_then(|s| s.get("status"))
        .and_then(|st| st.as_str())
        .map(String::from);
    let health_status = data
        .get("status")
        .and_then(|s| s.get("health"))
        .and_then(|h| h.get("status"))
        .and_then(|st| st.as_str())
        .map(String::from);
    let summary = ArgoApplicationSetSummary {
        cluster: cluster_context.clone(),
        name: name.clone(),
        age,
        created_at: k8s_creation_timestamp_to_rfc3339(&obj.metadata.creation_timestamp),
        namespace: namespace.clone(),
        project,
        status,
        sync_status,
        health_status,
        destination_namespace,
        destination_server,
        source_repo,
        source_revision,
    };
    Ok(ArgoApplicationSetDetails {
        summary,
        yaml,
        metadata,
    })
}
