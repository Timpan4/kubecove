use super::find_api_resource;
use crate::commands::helpers::{k8s_creation_timestamp_to_rfc3339, list_params, resource_age};
use crate::models::{AppError, ArgoApplicationDetails, ArgoApplicationSummary};
use chrono::{TimeZone, Utc};
use kube::{
    api::{Api, DynamicObject},
    config::KubeConfigOptions,
    Client,
};

/// List Argo CD Applications in the cluster.
#[tauri::command]
pub async fn list_argocd_applications(
    cluster_context: String,
) -> Result<Vec<ArgoApplicationSummary>, AppError> {
    let options = KubeConfigOptions {
        context: Some(cluster_context.clone()),
        ..Default::default()
    };

    let config = kube::Config::from_kubeconfig(&options)
        .await
        .map_err(|e| AppError::kube(e.to_string()))?;

    let client = Client::try_from(config).map_err(|e| AppError::kube(e.to_string()))?;

    let ar = match find_api_resource(&client, "argoproj.io", "Application").await? {
        Some(ar) => ar,
        None => return Ok(vec![]),
    };

    let api: Api<DynamicObject> = Api::all_with(client.clone(), &ar);
    let items = api
        .list(&list_params())
        .await
        .map_err(|e| AppError::kube(e.to_string()))?;

    let summaries: Vec<ArgoApplicationSummary> = items
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
            let destination_namespace = data
                .get("spec")
                .and_then(|s| s.get("destination"))
                .and_then(|d| d.get("namespace"))
                .and_then(|n| n.as_str())
                .map(String::from);
            let destination_server = data
                .get("spec")
                .and_then(|s| s.get("destination"))
                .and_then(|d| d.get("server"))
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

            Some(ArgoApplicationSummary {
                cluster: cluster_context.clone(),
                name,
                age,
                created_at: k8s_creation_timestamp_to_rfc3339(&obj.metadata.creation_timestamp),
                namespace,
                project,
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

/// Get detailed Argo CD Application information including YAML, metadata, and status.
#[tauri::command]
pub async fn get_argocd_application_details(
    cluster_context: String,
    name: String,
    namespace: Option<String>,
) -> Result<ArgoApplicationDetails, AppError> {
    let options = KubeConfigOptions {
        context: Some(cluster_context.clone()),
        ..Default::default()
    };

    let config = kube::Config::from_kubeconfig(&options)
        .await
        .map_err(|e| AppError::kube(e.to_string()))?;

    let client = Client::try_from(config).map_err(|e| AppError::kube(e.to_string()))?;

    let ar = match find_api_resource(&client, "argoproj.io", "Application").await? {
        Some(ar) => ar,
        None => return Err(AppError::new("Application CRD not found", "cluster")),
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

    let yaml =
        serde_yaml::to_string(&obj).map_err(|e| AppError::new(e.to_string(), "serialization"))?;
    let metadata = serde_json::to_value(&obj.metadata)
        .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
    let data = obj
        .data
        .as_object()
        .ok_or_else(|| AppError::new("invalid application data", "cluster"))?;

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
        .and_then(|s| s.get("destination"))
        .and_then(|d| d.get("namespace"))
        .and_then(|n| n.as_str())
        .map(String::from);
    let destination_server = data
        .get("spec")
        .and_then(|s| s.get("destination"))
        .and_then(|d| d.get("server"))
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
    let status = data.get("status").cloned();

    let summary = ArgoApplicationSummary {
        cluster: cluster_context.clone(),
        name,
        age,
        created_at: k8s_creation_timestamp_to_rfc3339(&obj.metadata.creation_timestamp),
        namespace,
        project,
        sync_status,
        health_status,
        destination_namespace,
        destination_server,
        source_repo,
        source_revision,
    };

    Ok(ArgoApplicationDetails {
        summary,
        yaml,
        metadata,
        status,
    })
}
