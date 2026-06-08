use super::{client_for_context, find_api_resource};
use crate::commands::helpers::{
    k8s_creation_timestamp_to_rfc3339, list_params, resource_age, serialize_resource_document,
};
use crate::models::{
    AppError, ArgoAppProjectDetails, ArgoAppProjectSummary, YamlEncoding, YamlViewMode,
};
use chrono::{TimeZone, Utc};
use kube::api::{Api, DynamicObject};

/// Get detailed Argo CD AppProject information including YAML and metadata.
#[tauri::command]
pub async fn get_argocd_appproject_details(
    cluster_context: String,
    name: String,
    namespace: Option<String>,
    kubeconfig_env_var: Option<String>,
    yaml_view_mode: Option<YamlViewMode>,
    yaml_encoding: Option<YamlEncoding>,
) -> Result<ArgoAppProjectDetails, AppError> {
    let client = client_for_context(&cluster_context, kubeconfig_env_var).await?;

    let ar = match find_api_resource(&client, "argoproj.io", "AppProject").await? {
        Some(ar) => ar,
        None => return Err(AppError::new("AppProject CRD not found", "cluster")),
    };

    let api: Api<DynamicObject> = if let Some(ns) = &namespace {
        Api::namespaced_with(client.clone(), ns.as_str(), &ar)
    } else {
        Api::all_with(client.clone(), &ar)
    };

    let obj = api
        .get(&name)
        .await
        .map_err(|e| AppError::kube(e.to_string()))?;
    let yaml = serialize_resource_document(
        &obj,
        yaml_view_mode.unwrap_or_default(),
        yaml_encoding.unwrap_or_default(),
    )?;
    let metadata = serde_json::to_value(&obj.metadata)
        .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
    let data = obj
        .data
        .as_object()
        .ok_or_else(|| AppError::new("invalid AppProject data", "cluster"))?;

    let name = obj.metadata.name.clone().unwrap_or_default();
    let namespace = obj.metadata.namespace.clone();
    let age = resource_age(obj.metadata.creation_timestamp.clone().map(|t| {
        Utc.timestamp_opt(t.0.as_second(), 0)
            .single()
            .unwrap_or_else(Utc::now)
    }));
    let description = data
        .get("spec")
        .and_then(|s| s.get("description"))
        .and_then(|d| d.as_str())
        .map(String::from);
    let status = data
        .get("status")
        .and_then(|s| s.get("conditions"))
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|c| c.get("type"))
        .and_then(|t| t.as_str())
        .map(String::from);
    let summary = ArgoAppProjectSummary {
        cluster: cluster_context.clone(),
        name,
        age,
        created_at: k8s_creation_timestamp_to_rfc3339(&obj.metadata.creation_timestamp),
        namespace,
        description,
        status,
    };
    Ok(ArgoAppProjectDetails {
        summary,
        yaml,
        metadata,
    })
}

/// List Argo CD AppProjects in the cluster.
#[tauri::command]
pub async fn list_argocd_appprojects(
    cluster_context: String,
    kubeconfig_env_var: Option<String>,
) -> Result<Vec<ArgoAppProjectSummary>, AppError> {
    let client = client_for_context(&cluster_context, kubeconfig_env_var).await?;

    let ar = match find_api_resource(&client, "argoproj.io", "AppProject").await? {
        Some(ar) => ar,
        None => return Ok(vec![]),
    };

    let api: Api<DynamicObject> = Api::all_with(client.clone(), &ar);
    let items = api
        .list(&list_params())
        .await
        .map_err(|e| AppError::kube(e.to_string()))?;

    let summaries: Vec<ArgoAppProjectSummary> = items
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

            let description = data
                .get("spec")
                .and_then(|s| s.get("description"))
                .and_then(|d| d.as_str())
                .map(String::from);
            let status = data
                .get("status")
                .and_then(|s| s.get("conditions"))
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("type"))
                .and_then(|t| t.as_str())
                .map(String::from);

            Some(ArgoAppProjectSummary {
                cluster: cluster_context.clone(),
                name,
                age,
                created_at: k8s_creation_timestamp_to_rfc3339(&obj.metadata.creation_timestamp),
                namespace,
                description,
                status,
            })
        })
        .collect();

    Ok(summaries)
}
