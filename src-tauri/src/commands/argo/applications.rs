use crate::commands::gitops_crd::{
    client_for_context, find_api_resource, get_crd_object, list_crd_objects, resource_metadata,
    resource_status, resource_yaml,
};
use crate::commands::helpers::{k8s_creation_timestamp_to_rfc3339, resource_age};
use crate::models::{
    AppError, ArgoApplicationDetails, ArgoApplicationSummary, YamlEncoding, YamlViewMode,
};
use chrono::{TimeZone, Utc};
use std::collections::BTreeSet;

/// List Argo CD Applications in the cluster.
#[tauri::command]
pub async fn list_argocd_applications(
    cluster_context: String,
    kubeconfig_env_var: Option<String>,
) -> Result<Vec<ArgoApplicationSummary>, AppError> {
    let client = client_for_context(&cluster_context, kubeconfig_env_var).await?;

    let ar = match find_api_resource(&client, "argoproj.io", "Application").await? {
        Some(ar) => ar,
        None => return Ok(vec![]),
    };

    let items = list_crd_objects(client.clone(), &ar).await?;

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
            let resource_namespaces =
                application_resource_namespaces(data, destination_namespace.as_deref());
            let tracked_resource_count = application_tracked_resource_count(data);

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
                resource_namespaces,
                tracked_resource_count,
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
    kubeconfig_env_var: Option<String>,
    yaml_view_mode: Option<YamlViewMode>,
    yaml_encoding: Option<YamlEncoding>,
) -> Result<ArgoApplicationDetails, AppError> {
    let client = client_for_context(&cluster_context, kubeconfig_env_var).await?;

    let ar = match find_api_resource(&client, "argoproj.io", "Application").await? {
        Some(ar) => ar,
        None => return Err(AppError::new("Application CRD not found", "cluster")),
    };

    let obj = get_crd_object(client.clone(), &ar, &name, namespace.as_deref()).await?;

    let yaml = resource_yaml(&obj, yaml_view_mode, yaml_encoding)?;
    let metadata = resource_metadata(&obj)?;
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
    let resource_namespaces =
        application_resource_namespaces(data, destination_namespace.as_deref());
    let tracked_resource_count = application_tracked_resource_count(data);
    let status = resource_status(&obj);

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
        resource_namespaces,
        tracked_resource_count,
    };

    Ok(ArgoApplicationDetails {
        summary,
        yaml,
        metadata,
        status,
    })
}

fn application_resource_namespaces(
    data: &serde_json::Map<String, serde_json::Value>,
    destination_namespace: Option<&str>,
) -> Vec<String> {
    let mut namespaces = BTreeSet::new();
    if let Some(resources) = data
        .get("status")
        .and_then(|status| status.get("resources"))
        .and_then(|resources| resources.as_array())
    {
        for resource in resources {
            if let Some(namespace) = resource
                .get("namespace")
                .and_then(|namespace| namespace.as_str())
                .map(str::trim)
                .filter(|namespace| !namespace.is_empty())
            {
                namespaces.insert(namespace.to_string());
            }
        }
    }
    if namespaces.is_empty() {
        if let Some(namespace) = destination_namespace
            .map(str::trim)
            .filter(|namespace| !namespace.is_empty())
        {
            namespaces.insert(namespace.to_string());
        }
    }
    namespaces.into_iter().collect()
}

fn application_tracked_resource_count(
    data: &serde_json::Map<String, serde_json::Value>,
) -> Option<usize> {
    data.get("status")
        .and_then(|status| status.get("resources"))
        .and_then(|resources| resources.as_array())
        .map(Vec::len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_unique_tracked_resource_namespaces() {
        let data = json!({
            "spec": {
                "destination": {
                    "namespace": "fallback"
                }
            },
            "status": {
                "resources": [
                    { "kind": "Deployment", "namespace": "traefik", "name": "traefik" },
                    { "kind": "Service", "namespace": "traefik", "name": "traefik" },
                    { "kind": "ConfigMap", "namespace": "monitoring", "name": "dashboard" },
                    { "kind": "ClusterRole", "name": "traefik" },
                    { "kind": "CustomResourceDefinition", "namespace": "", "name": "ignored" }
                ]
            }
        });
        let data = data.as_object().expect("application data");

        assert_eq!(
            application_resource_namespaces(data, Some("fallback")),
            vec!["monitoring".to_string(), "traefik".to_string()]
        );
        assert_eq!(application_tracked_resource_count(data), Some(5));
    }

    #[test]
    fn falls_back_to_destination_namespace_without_status_resources() {
        let data = json!({
            "spec": {
                "destination": {
                    "namespace": "traefik"
                }
            }
        });
        let data = data.as_object().expect("application data");

        assert_eq!(
            application_resource_namespaces(data, Some("traefik")),
            vec!["traefik".to_string()]
        );
        assert_eq!(application_tracked_resource_count(data), None);
    }

    #[test]
    fn returns_empty_namespaces_for_all_namespace_fallback() {
        let data = json!({
            "status": {
                "resources": [
                    { "kind": "ClusterRole", "name": "traefik" }
                ]
            }
        });
        let data = data.as_object().expect("application data");

        assert_eq!(
            application_resource_namespaces(data, None),
            Vec::<String>::new()
        );
        assert_eq!(application_tracked_resource_count(data), Some(1));
    }
}
