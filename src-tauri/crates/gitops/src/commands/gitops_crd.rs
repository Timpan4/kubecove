use crate::commands::helpers::{
    k8s_timestamp_to_datetime, list_params, resource_age, serialize_resource_document,
};
use crate::models::AppError;
use crate::models::AppErrorKind;
use chrono::Utc;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::{
    api::{Api, ApiResource, DynamicObject},
    discovery::Discovery,
    Client,
};
use serde_json::Value;

use crate::models::{YamlEncoding, YamlViewMode};

/// Age of an Argo CD or Flux object; an unrepresentable creation timestamp reads as just created.
pub(crate) fn object_age(metadata: &ObjectMeta) -> String {
    resource_age(
        metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| k8s_timestamp_to_datetime(&timestamp.0).unwrap_or_else(Utc::now)),
    )
}

pub(crate) async fn discover_api_resources(client: &Client) -> Result<Vec<ApiResource>, AppError> {
    let discovery = Discovery::new(client.clone())
        .run_aggregated()
        .await
        .map_err(AppError::from)?;
    let mut resources = Vec::new();
    for group in discovery.groups() {
        for (api_resource, _) in group.recommended_resources() {
            resources.push(api_resource.clone());
        }
    }
    Ok(resources)
}

pub(crate) fn find_discovered_api_resource(
    resources: &[ApiResource],
    group: &str,
    kind: &str,
) -> Option<ApiResource> {
    resources
        .iter()
        .find(|resource| resource.group == group && resource.kind == kind)
        .cloned()
}

pub(crate) async fn find_api_resource(
    client: &Client,
    group: &str,
    kind: &str,
) -> Result<Option<ApiResource>, AppError> {
    let resources = discover_api_resources(client).await?;
    Ok(find_discovered_api_resource(&resources, group, kind))
}

pub(crate) async fn list_crd_objects(
    client: Client,
    api_resource: &ApiResource,
) -> Result<Vec<DynamicObject>, AppError> {
    crate::commands::helpers::validate_api_resource(api_resource)?;
    Api::<DynamicObject>::all_with(client, api_resource)
        .list(&list_params())
        .await
        .map(|list| list.items)
        .map_err(AppError::from)
}

pub(crate) async fn get_crd_object(
    client: Client,
    api_resource: &ApiResource,
    name: &str,
    namespace: Option<&str>,
) -> Result<DynamicObject, AppError> {
    crate::commands::helpers::validate_namespace(namespace)?;
    crate::commands::helpers::validate_path_segment(name, "resource name")?;
    crate::commands::helpers::validate_api_resource(api_resource)?;
    dynamic_api(client, api_resource, namespace)
        .get(name)
        .await
        .map_err(AppError::from)
}

fn dynamic_api(
    client: Client,
    api_resource: &ApiResource,
    namespace: Option<&str>,
) -> Api<DynamicObject> {
    match namespace {
        Some(namespace) => Api::namespaced_with(client, namespace, api_resource),
        None => Api::all_with(client, api_resource),
    }
}

pub(crate) fn resource_yaml(
    object: &DynamicObject,
    yaml_view_mode: Option<YamlViewMode>,
    yaml_encoding: Option<YamlEncoding>,
) -> Result<String, AppError> {
    serialize_resource_document(
        object,
        yaml_view_mode.unwrap_or_default(),
        yaml_encoding.unwrap_or_default(),
    )
}

pub(crate) fn resource_metadata(object: &DynamicObject) -> Result<Value, AppError> {
    serde_json::to_value(&object.metadata)
        .map_err(|err| AppError::new(err.to_string(), AppErrorKind::Serialization).with_source(err))
}

pub(crate) fn resource_status(object: &DynamicObject) -> Option<Value> {
    object.data.get("status").cloned()
}
