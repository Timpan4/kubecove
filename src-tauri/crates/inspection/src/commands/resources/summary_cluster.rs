use crate::commands::helpers::{base_resource_summary, list_params, update_resource_health};
use crate::models::{AppError, ResourceSummary};
use chrono::{DateTime, Utc};
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::{api::Api, Client};

pub(crate) fn node_resource_summary(
    cluster_context: &str,
    node: &k8s_openapi::api::core::v1::Node,
) -> ResourceSummary {
    node_resource_summary_at(cluster_context, node, Utc::now())
}

pub fn node_resource_summary_at(
    cluster_context: &str,
    node: &k8s_openapi::api::core::v1::Node,
    now: DateTime<Utc>,
) -> ResourceSummary {
    let mut summary = base_resource_summary(
        "Node",
        cluster_context,
        &node.metadata,
        super::summary_core::age_from_metadata_at(&node.metadata, now),
    );
    if let Some(ref status) = node.status {
        if let Some(ref conditions) = status.conditions {
            let ready_count = conditions
                .iter()
                .filter(|c| c.type_ == "Ready" && c.status == "True")
                .count();
            summary.ready = Some(format!("{ready_count}/{}", 1));
            summary.status = conditions
                .iter()
                .find(|c| c.type_ == "Ready")
                .map(|c| c.status.clone());
        }
    }
    update_resource_health(&mut summary);
    summary
}

pub(crate) fn storage_class_resource_summary(
    cluster_context: &str,
    storage_class: &k8s_openapi::api::storage::v1::StorageClass,
) -> ResourceSummary {
    storage_class_resource_summary_at(cluster_context, storage_class, Utc::now())
}

pub fn storage_class_resource_summary_at(
    cluster_context: &str,
    storage_class: &k8s_openapi::api::storage::v1::StorageClass,
    now: DateTime<Utc>,
) -> ResourceSummary {
    base_resource_summary(
        "StorageClass",
        cluster_context,
        &storage_class.metadata,
        super::summary_core::age_from_metadata_at(&storage_class.metadata, now),
    )
}

pub(crate) fn persistent_volume_resource_summary(
    cluster_context: &str,
    volume: &k8s_openapi::api::core::v1::PersistentVolume,
) -> ResourceSummary {
    persistent_volume_resource_summary_at(cluster_context, volume, Utc::now())
}

pub fn persistent_volume_resource_summary_at(
    cluster_context: &str,
    volume: &k8s_openapi::api::core::v1::PersistentVolume,
    now: DateTime<Utc>,
) -> ResourceSummary {
    let mut summary = base_resource_summary(
        "PersistentVolume",
        cluster_context,
        &volume.metadata,
        super::summary_core::age_from_metadata_at(&volume.metadata, now),
    );
    summary.status = volume
        .status
        .as_ref()
        .and_then(|status| status.phase.clone());
    update_resource_health(&mut summary);
    summary
}

pub(crate) fn custom_resource_definition_resource_summary(
    cluster_context: &str,
    crd: &CustomResourceDefinition,
) -> ResourceSummary {
    custom_resource_definition_resource_summary_at(cluster_context, crd, Utc::now())
}

pub fn custom_resource_definition_resource_summary_at(
    cluster_context: &str,
    crd: &CustomResourceDefinition,
    now: DateTime<Utc>,
) -> ResourceSummary {
    let mut summary = base_resource_summary(
        "CustomResourceDefinition",
        cluster_context,
        &crd.metadata,
        super::summary_core::age_from_metadata_at(&crd.metadata, now),
    );
    summary.api_version = Some("apiextensions.k8s.io/v1".to_string());
    summary.group = Some("apiextensions.k8s.io".to_string());
    summary.version = Some("v1".to_string());
    summary.plural = Some("customresourcedefinitions".to_string());
    summary.namespaced = Some(false);
    summary.status = crd.status.as_ref().and_then(|status| {
        status
            .conditions
            .as_ref()?
            .iter()
            .find(|condition| condition.type_ == "Established")
            .or_else(|| status.conditions.as_ref()?.first())
            .map(|condition| format!("{}: {}", condition.type_, condition.status))
    });
    update_resource_health(&mut summary);
    summary
}

pub(super) async fn cluster_resource_summaries(
    client: Client,
    cluster_context: &str,
    kind: &str,
) -> Result<Option<Vec<ResourceSummary>>, AppError> {
    let summaries = match kind {
        "Node" => node_summaries(client, cluster_context).await?,
        "StorageClass" => storageclass_summaries(client, cluster_context).await?,
        "PersistentVolume" => persistentvolume_summaries(client, cluster_context).await?,
        "CustomResourceDefinition" => crd_summaries(client, cluster_context).await?,
        _ => return Ok(None),
    };

    Ok(Some(summaries))
}

async fn node_summaries(
    client: Client,
    cluster_context: &str,
) -> Result<Vec<ResourceSummary>, AppError> {
    let api: Api<k8s_openapi::api::core::v1::Node> = Api::all(client);
    Ok(api
        .list(&list_params())
        .await
        .map_err(AppError::from)?
        .iter()
        .map(|node| node_resource_summary(cluster_context, node))
        .collect())
}

async fn storageclass_summaries(
    client: Client,
    cluster_context: &str,
) -> Result<Vec<ResourceSummary>, AppError> {
    let api: Api<k8s_openapi::api::storage::v1::StorageClass> = Api::all(client);
    Ok(api
        .list(&list_params())
        .await
        .map_err(AppError::from)?
        .iter()
        .map(|sc| storage_class_resource_summary(cluster_context, sc))
        .collect())
}

async fn persistentvolume_summaries(
    client: Client,
    cluster_context: &str,
) -> Result<Vec<ResourceSummary>, AppError> {
    let api: Api<k8s_openapi::api::core::v1::PersistentVolume> = Api::all(client);
    Ok(api
        .list(&list_params())
        .await
        .map_err(AppError::from)?
        .iter()
        .map(|pv| persistent_volume_resource_summary(cluster_context, pv))
        .collect())
}

async fn crd_summaries(
    client: Client,
    cluster_context: &str,
) -> Result<Vec<ResourceSummary>, AppError> {
    let api: Api<CustomResourceDefinition> = Api::all(client);
    Ok(api
        .list(&list_params())
        .await
        .map_err(AppError::from)?
        .iter()
        .map(|crd| custom_resource_definition_resource_summary(cluster_context, crd))
        .collect())
}
