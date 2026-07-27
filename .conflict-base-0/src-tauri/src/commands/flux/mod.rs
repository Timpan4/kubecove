use crate::commands::gitops_crd::{
    client_for_context, discover_api_resources, get_crd_object, has_api_resource, list_crd_objects,
    resource_metadata, resource_status, resource_yaml,
};
use crate::commands::helpers::{k8s_creation_timestamp_to_rfc3339, resource_age};
use crate::models::{
    AppError, FluxDetectionSummary, FluxInventoryResource, FluxResourceDetails, FluxResourceKind,
    FluxResourceSummary, YamlEncoding, YamlViewMode,
};
use chrono::{TimeZone, Utc};
use kube::api::{ApiResource, DynamicObject};
use kube::Client;
use serde_json::Value;

pub(crate) const FLUX_KINDS: &[(&str, &str, &str, &str, bool, &str)] = &[
    (
        "source.toolkit.fluxcd.io",
        "v1",
        "GitRepository",
        "gitrepositories",
        true,
        "Sources",
    ),
    (
        "source.toolkit.fluxcd.io",
        "v1",
        "OCIRepository",
        "ocirepositories",
        true,
        "Sources",
    ),
    (
        "source.toolkit.fluxcd.io",
        "v1",
        "HelmRepository",
        "helmrepositories",
        true,
        "Sources",
    ),
    (
        "source.toolkit.fluxcd.io",
        "v1",
        "HelmChart",
        "helmcharts",
        true,
        "Sources",
    ),
    (
        "source.toolkit.fluxcd.io",
        "v1",
        "Bucket",
        "buckets",
        true,
        "Sources",
    ),
    (
        "kustomize.toolkit.fluxcd.io",
        "v1",
        "Kustomization",
        "kustomizations",
        true,
        "Kustomize",
    ),
    (
        "helm.toolkit.fluxcd.io",
        "v2",
        "HelmRelease",
        "helmreleases",
        true,
        "Helm",
    ),
    (
        "notification.toolkit.fluxcd.io",
        "v1beta3",
        "Provider",
        "providers",
        true,
        "Notifications",
    ),
    (
        "notification.toolkit.fluxcd.io",
        "v1beta3",
        "Alert",
        "alerts",
        true,
        "Notifications",
    ),
    (
        "notification.toolkit.fluxcd.io",
        "v1",
        "Receiver",
        "receivers",
        true,
        "Notifications",
    ),
    (
        "image.toolkit.fluxcd.io",
        "v1",
        "ImageRepository",
        "imagerepositories",
        true,
        "Image Automation",
    ),
    (
        "image.toolkit.fluxcd.io",
        "v1",
        "ImagePolicy",
        "imagepolicies",
        true,
        "Image Automation",
    ),
    (
        "image.toolkit.fluxcd.io",
        "v1",
        "ImageUpdateAutomation",
        "imageupdateautomations",
        true,
        "Image Automation",
    ),
];

#[tauri::command]
pub async fn detect_flux(
    cluster_context: String,
    kubeconfig_env_var: Option<String>,
) -> Result<FluxDetectionSummary, AppError> {
    let client = client_for_context(&cluster_context, kubeconfig_env_var).await?;
    let installed = installed_flux_kinds(&client).await?;
    let missing_kinds = flux_kinds()
        .into_iter()
        .filter(|kind| {
            !installed
                .iter()
                .any(|candidate| same_flux_kind(candidate, kind))
        })
        .collect::<Vec<_>>();

    Ok(FluxDetectionSummary {
        detected: !installed.is_empty(),
        kinds: installed,
        missing_kinds,
    })
}

#[tauri::command]
pub async fn list_flux_resources(
    cluster_context: String,
    resource_kind: FluxResourceKind,
    kubeconfig_env_var: Option<String>,
) -> Result<Vec<FluxResourceSummary>, AppError> {
    let client = client_for_context(&cluster_context, kubeconfig_env_var).await?;
    if !flux_kind_exists(&client, &resource_kind).await? {
        return Ok(vec![]);
    }
    let api_resource = api_resource_from_flux_kind(&resource_kind);
    let items = list_crd_objects(client, &api_resource).await?;

    Ok(items
        .iter()
        .map(|object| flux_summary(&cluster_context, &resource_kind, object))
        .collect())
}

#[tauri::command]
pub async fn get_flux_resource_details(
    cluster_context: String,
    resource_kind: FluxResourceKind,
    name: String,
    namespace: Option<String>,
    kubeconfig_env_var: Option<String>,
    yaml_view_mode: Option<YamlViewMode>,
    yaml_encoding: Option<YamlEncoding>,
) -> Result<FluxResourceDetails, AppError> {
    let client = client_for_context(&cluster_context, kubeconfig_env_var).await?;
    if !flux_kind_exists(&client, &resource_kind).await? {
        return Err(AppError::new("Flux resource kind not found", "cluster"));
    }
    let api_resource = api_resource_from_flux_kind(&resource_kind);
    let namespace = if resource_kind.namespaced {
        Some(namespace.as_deref().ok_or_else(|| {
            AppError::new(
                "Namespace required for namespaced Flux resource",
                "validation",
            )
        })?)
    } else {
        None
    };
    let object = get_crd_object(client, &api_resource, &name, namespace).await?;

    let yaml = resource_yaml(&object, yaml_view_mode, yaml_encoding)?;
    let metadata = resource_metadata(&object)?;
    let summary = flux_summary(&cluster_context, &resource_kind, &object);

    Ok(FluxResourceDetails {
        summary,
        yaml,
        metadata,
        status: resource_status(&object),
    })
}

pub(crate) fn flux_kinds() -> Vec<FluxResourceKind> {
    FLUX_KINDS
        .iter()
        .map(
            |(group, version, kind, plural, namespaced, category)| FluxResourceKind {
                group: (*group).to_string(),
                version: (*version).to_string(),
                api_version: format!("{group}/{version}"),
                kind: (*kind).to_string(),
                plural: (*plural).to_string(),
                namespaced: *namespaced,
                category: (*category).to_string(),
            },
        )
        .collect()
}

async fn installed_flux_kinds(client: &Client) -> Result<Vec<FluxResourceKind>, AppError> {
    let resources = discover_api_resources(client).await?;
    Ok(flux_kinds()
        .into_iter()
        .filter(|kind| has_api_resource(&resources, &kind.group, &kind.kind))
        .collect())
}

async fn flux_kind_exists(client: &Client, kind: &FluxResourceKind) -> Result<bool, AppError> {
    Ok(installed_flux_kinds(client)
        .await?
        .iter()
        .any(|candidate| same_flux_kind(candidate, kind)))
}

fn same_flux_kind(left: &FluxResourceKind, right: &FluxResourceKind) -> bool {
    left.group == right.group && left.kind == right.kind
}

fn api_resource_from_flux_kind(kind: &FluxResourceKind) -> ApiResource {
    ApiResource {
        group: kind.group.clone(),
        version: kind.version.clone(),
        api_version: kind.api_version.clone(),
        kind: kind.kind.clone(),
        plural: kind.plural.clone(),
    }
}

pub(crate) fn ready_status(data: &Value) -> Option<String> {
    data.get("status")
        .and_then(|status| status.get("conditions"))
        .and_then(Value::as_array)?
        .iter()
        .find(|condition| {
            condition
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|value| value == "Ready")
        })
        .and_then(|condition| {
            condition
                .get("status")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    condition
                        .get("reason")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
        })
}

pub(crate) fn inventory(data: &Value) -> Vec<FluxInventoryResource> {
    data.get("status")
        .and_then(|status| status.get("inventory"))
        .and_then(|inventory| inventory.get("entries"))
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| {
                    let id = entry.get("id").and_then(Value::as_str)?;
                    Some(FluxInventoryResource {
                        id: id.to_string(),
                        version: entry
                            .get("v")
                            .or_else(|| entry.get("version"))
                            .and_then(Value::as_str)
                            .map(str::to_string),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn flux_summary(
    cluster_context: &str,
    resource_kind: &FluxResourceKind,
    object: &DynamicObject,
) -> FluxResourceSummary {
    FluxResourceSummary {
        cluster: cluster_context.to_string(),
        name: object.metadata.name.clone().unwrap_or_default(),
        namespace: object.metadata.namespace.clone(),
        age: resource_age(object.metadata.creation_timestamp.clone().map(|timestamp| {
            Utc.timestamp_opt(timestamp.0.as_second(), 0)
                .single()
                .unwrap_or_else(Utc::now)
        })),
        created_at: k8s_creation_timestamp_to_rfc3339(&object.metadata.creation_timestamp),
        resource_kind: resource_kind.clone(),
        ready_status: ready_status(&object.data),
        suspended: object
            .data
            .get("spec")
            .and_then(|spec| spec.get("suspend"))
            .and_then(Value::as_bool),
        source_kind: source_field(&object.data, "kind"),
        source_name: source_field(&object.data, "name"),
        source_namespace: source_field(&object.data, "namespace"),
        interval: object
            .data
            .get("spec")
            .and_then(|spec| spec.get("interval"))
            .and_then(Value::as_str)
            .map(str::to_string),
        last_applied_revision: revision(&object.data),
        message: ready_message(&object.data),
        inventory: inventory(&object.data),
    }
}

fn source_field(data: &Value, field: &str) -> Option<String> {
    data.get("spec")
        .and_then(|spec| spec.get("sourceRef"))
        .and_then(|source_ref| source_ref.get(field))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn revision(data: &Value) -> Option<String> {
    data.get("status")
        .and_then(|status| status.get("lastAppliedRevision"))
        .or_else(|| {
            data.get("status")
                .and_then(|status| status.get("artifact"))
                .and_then(|artifact| artifact.get("revision"))
        })
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn ready_message(data: &Value) -> Option<String> {
    data.get("status")
        .and_then(|status| status.get("conditions"))
        .and_then(Value::as_array)?
        .iter()
        .find(|condition| {
            condition
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|value| value == "Ready")
        })
        .and_then(|condition| condition.get("message"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn flux_catalog_contains_current_api_families() {
        let kinds = flux_kinds();

        assert!(kinds.iter().any(|kind| kind.kind == "GitRepository"));
        assert!(kinds.iter().any(|kind| kind.kind == "Kustomization"));
        assert!(kinds.iter().any(|kind| kind.kind == "HelmRelease"));
        assert!(kinds.iter().any(|kind| kind.kind == "Alert"));
        assert!(kinds
            .iter()
            .any(|kind| kind.kind == "ImageUpdateAutomation"));
    }

    #[test]
    fn extracts_ready_status_and_inventory_entries() {
        let data = json!({
            "status": {
                "conditions": [
                    { "type": "Ready", "status": "True", "message": "Applied revision main@sha1:abc" }
                ],
                "inventory": {
                    "entries": [
                        { "id": "default_api_apps_Deployment", "v": "v1" }
                    ]
                }
            }
        });

        assert_eq!(ready_status(&data), Some("True".to_string()));
        assert_eq!(inventory(&data).len(), 1);
        assert_eq!(
            ready_message(&data),
            Some("Applied revision main@sha1:abc".to_string())
        );
    }
}
