use crate::commands::helpers::{
    fetch_and_serialize_cluster, k8s_creation_timestamp_to_rfc3339, resource_age,
};
use crate::models::{AppError, ResourceDetailsFull, ResourceSummary};
use chrono::{TimeZone, Utc};
use kube::Client;

pub(super) async fn node_details(
    client: Client,
    cluster_context: String,
    name: String,
    _namespace: Option<String>,
) -> Result<ResourceDetailsFull, AppError> {
    let (node, yaml) =
        fetch_and_serialize_cluster::<k8s_openapi::api::core::v1::Node>(client, &name).await?;
    let metadata = serde_json::to_value(&node.metadata)
        .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
    let status = node
        .status
        .as_ref()
        .and_then(|s| serde_json::to_value(s).ok());
    let summary = ResourceSummary {
        kind: "Node".to_string(),
        cluster: cluster_context.clone(),
        name: name.clone(),
        namespace: None,
        age: resource_age(node.metadata.creation_timestamp.clone().map(|t| {
            Utc.timestamp_opt(t.0.as_second(), 0)
                .single()
                .unwrap_or_else(Utc::now)
        })),
        api_version: None,
        group: None,
        version: None,
        plural: None,
        namespaced: None,
        dynamic: None,
        created_at: k8s_creation_timestamp_to_rfc3339(&node.metadata.creation_timestamp),
        status: node.status.as_ref().and_then(|s| {
            s.conditions
                .as_ref()?
                .iter()
                .find(|c| c.type_ == "Ready")
                .map(|c| c.status.clone())
        }),
        ready: node.status.as_ref().and_then(|s| {
            s.conditions.as_ref().map(|conds| {
                format!(
                    "{}/{}",
                    conds
                        .iter()
                        .filter(|c| c.type_ == "Ready" && c.status == "True")
                        .count(),
                    1
                )
            })
        }),
        restarts: None,
        owner_ref: None,
        argo_app: None,
        helm_release: None,
    };
    Ok(ResourceDetailsFull {
        summary,
        yaml,
        metadata,
        status,
    })
}

pub(super) async fn storageclass_details(
    client: Client,
    cluster_context: String,
    name: String,
    _namespace: Option<String>,
) -> Result<ResourceDetailsFull, AppError> {
    let (sc, yaml) =
        fetch_and_serialize_cluster::<k8s_openapi::api::storage::v1::StorageClass>(client, &name)
            .await?;
    let metadata = serde_json::to_value(&sc.metadata)
        .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
    let summary = ResourceSummary {
        kind: "StorageClass".to_string(),
        cluster: cluster_context.clone(),
        name: name.clone(),
        namespace: None,
        age: resource_age(sc.metadata.creation_timestamp.clone().map(|t| {
            Utc.timestamp_opt(t.0.as_second(), 0)
                .single()
                .unwrap_or_else(Utc::now)
        })),
        api_version: None,
        group: None,
        version: None,
        plural: None,
        namespaced: None,
        dynamic: None,
        created_at: k8s_creation_timestamp_to_rfc3339(&sc.metadata.creation_timestamp),
        status: None,
        ready: None,
        restarts: None,
        owner_ref: None,
        argo_app: None,
        helm_release: None,
    };
    Ok(ResourceDetailsFull {
        summary,
        yaml,
        metadata,
        status: None,
    })
}

pub(super) async fn pv_details(
    client: Client,
    cluster_context: String,
    name: String,
    _namespace: Option<String>,
) -> Result<ResourceDetailsFull, AppError> {
    let (pv, yaml) =
        fetch_and_serialize_cluster::<k8s_openapi::api::core::v1::PersistentVolume>(client, &name)
            .await?;
    let metadata = serde_json::to_value(&pv.metadata)
        .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
    let status = pv
        .status
        .as_ref()
        .and_then(|s| serde_json::to_value(s).ok());
    let summary = ResourceSummary {
        kind: "PersistentVolume".to_string(),
        cluster: cluster_context.clone(),
        name: name.clone(),
        namespace: None,
        age: resource_age(pv.metadata.creation_timestamp.clone().map(|t| {
            Utc.timestamp_opt(t.0.as_second(), 0)
                .single()
                .unwrap_or_else(Utc::now)
        })),
        api_version: None,
        group: None,
        version: None,
        plural: None,
        namespaced: None,
        dynamic: None,
        created_at: k8s_creation_timestamp_to_rfc3339(&pv.metadata.creation_timestamp),
        status: pv.status.as_ref().and_then(|s| s.phase.as_ref()).cloned(),
        ready: None,
        restarts: None,
        owner_ref: None,
        argo_app: None,
        helm_release: None,
    };
    Ok(ResourceDetailsFull {
        summary,
        yaml,
        metadata,
        status,
    })
}
