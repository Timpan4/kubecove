use super::{kind_api_resource, owned_revision_template};
use crate::commands::builtin_kinds::builtin_kind;
use crate::models::{
    AppError, AppErrorKind, ClusterOperationTarget, ResourceActionContainer, ResourceActionEnv,
    ResourceActionKind, ResourceActionPermission, ResourceActionRequest, ResourceActionRevision,
    ResourceActionState,
};
use k8s_openapi::api::authorization::v1::{
    ResourceAttributes, SelfSubjectAccessReview, SelfSubjectAccessReviewSpec,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::api::{Api, DynamicObject, ListParams, PostParams};
use serde_json::Value;

pub(super) fn permission_from_review(
    action: ResourceActionKind,
    verb: &str,
    resource: &str,
    allowed: Option<bool>,
    error: Option<&str>,
) -> ResourceActionPermission {
    ResourceActionPermission {
        action,
        allowed: allowed == Some(true),
        reason: match (allowed, error) {
            (Some(true), _) => "Allowed by Kubernetes authorization review".into(),
            (Some(false), _) => "Kubernetes denied this permission".into(),
            (_, Some(error)) => format!("Permission check failed: {error}"),
            _ => "Permission check returned no decision".into(),
        },
        required_permission: format!("{verb} {resource}"),
    }
}

pub(super) async fn access_review(
    client: kube::Client,
    target: &ClusterOperationTarget,
    verb: &str,
    resource: &str,
) -> Result<bool, AppError> {
    let group = builtin_kind(&target.kind)
        .map(|kind| kind.group.to_string())
        .unwrap_or_default();
    let api = Api::<SelfSubjectAccessReview>::all(client);
    let review = SelfSubjectAccessReview {
        metadata: ObjectMeta::default(),
        spec: SelfSubjectAccessReviewSpec {
            resource_attributes: Some(ResourceAttributes {
                group: Some(group),
                namespace: target.namespace.clone(),
                name: (verb != "list").then(|| target.name.clone()),
                resource: Some(resource.to_string()),
                subresource: None,
                verb: Some(verb.to_string()),
                version: None,
                field_selector: None,
                label_selector: None,
            }),
            non_resource_attributes: None,
        },
        status: None,
    };
    let result = api.create(&PostParams::default(), &review).await?;
    let status = result.status.ok_or_else(|| {
        AppError::new(
            "permission review returned no status",
            AppErrorKind::Forbidden,
        )
    })?;
    if status
        .evaluation_error
        .as_deref()
        .is_some_and(|error| !error.is_empty())
    {
        return Err(AppError::new(
            status.evaluation_error.unwrap_or_default(),
            AppErrorKind::Forbidden,
        ));
    }
    Ok(status.allowed && status.denied != Some(true))
}

pub(super) fn permissions_for_kind(
    kind: &str,
) -> Vec<(ResourceActionKind, &'static str, &'static str)> {
    let mut permissions = Vec::new();
    let resource = match kind {
        "Deployment" => "deployments",
        "StatefulSet" => "statefulsets",
        "DaemonSet" => "daemonsets",
        "Pod" => "pods",
        "ConfigMap" => "configmaps",
        _ => return permissions,
    };
    permissions.push((ResourceActionKind::Edit, "patch", resource));
    match kind {
        "Deployment" => {
            for action in [
                ResourceActionKind::Restart,
                ResourceActionKind::Pause,
                ResourceActionKind::Resume,
                ResourceActionKind::Rollback,
            ] {
                permissions.push((action, "patch", resource));
            }
        }
        "StatefulSet" | "DaemonSet" => {
            permissions.push((ResourceActionKind::Restart, "patch", resource));
        }
        _ => {}
    }
    permissions.push((ResourceActionKind::Delete, "delete", resource));
    permissions
}

pub(super) fn set_rollback_unavailable(permissions: &mut [ResourceActionPermission], reason: &str) {
    if let Some(permission) = permissions
        .iter_mut()
        .find(|permission| permission.action == ResourceActionKind::Rollback)
    {
        permission.allowed = false;
        permission.reason = reason.to_string();
        permission.required_permission = "list replicasets".into();
    }
}

pub(super) fn set_rollback_permission_text(
    permissions: &mut [ResourceActionPermission],
    required: &str,
) {
    if let Some(permission) = permissions
        .iter_mut()
        .find(|permission| permission.action == ResourceActionKind::Rollback)
    {
        permission.required_permission = required.into();
    }
}

pub(super) async fn owned_revisions(
    client: kube::Client,
    target: &ClusterOperationTarget,
    uid: &str,
) -> Result<Vec<ResourceActionRevision>, AppError> {
    let api = Api::<DynamicObject>::namespaced_with(
        client,
        target.namespace.as_deref().unwrap_or_default(),
        &kind_api_resource("ReplicaSet")?,
    );
    let list = api.list(&ListParams::default()).await?;
    let mut revisions = Vec::new();
    for object in list.items {
        let value = action_value(&object)?;
        if let Ok(template) = owned_revision_template(&value, &target.name, uid) {
            if let Some(revision) = value
                .pointer("/metadata/annotations/deployment.kubernetes.io~1revision")
                .and_then(Value::as_str)
                .and_then(|v| v.parse().ok())
            {
                revisions.push(ResourceActionRevision {
                    revision,
                    images: template
                        .pointer("/spec/containers")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(|container| {
                            container
                                .get("image")
                                .and_then(Value::as_str)
                                .map(str::to_string)
                        })
                        .collect(),
                });
            }
        }
    }
    revisions.sort_by_key(|revision| std::cmp::Reverse(revision.revision));
    Ok(revisions)
}

pub(super) async fn owned_revision_by_number(
    client: kube::Client,
    target: &ClusterOperationTarget,
    uid: &str,
    revision: i64,
) -> Result<Value, AppError> {
    let api = Api::<DynamicObject>::namespaced_with(
        client,
        target.namespace.as_deref().unwrap_or_default(),
        &kind_api_resource("ReplicaSet")?,
    );
    let list = api.list(&ListParams::default()).await?;
    for object in list.items {
        let value = action_value(&object)?;
        let found = value
            .pointer("/metadata/annotations/deployment.kubernetes.io~1revision")
            .and_then(Value::as_str)
            .and_then(|v| v.parse::<i64>().ok())
            == Some(revision);
        if found && owned_revision_template(&value, &target.name, uid).is_ok() {
            return Ok(value);
        }
    }
    Err(AppError::new(
        "revision is not owned by the selected Deployment",
        AppErrorKind::Validation,
    ))
}

pub(super) fn revision_source_identity(revision: &Value) -> Result<(String, String), AppError> {
    let uid = revision
        .pointer("/metadata/uid")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::new("revision UID is missing", AppErrorKind::InvalidResource))?;
    let resource_version = revision
        .pointer("/metadata/resourceVersion")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::new(
                "revision resourceVersion is missing",
                AppErrorKind::InvalidResource,
            )
        })?;
    Ok((uid.to_string(), resource_version.to_string()))
}

pub(super) fn validate_revision_review_guard(
    request: &ResourceActionRequest,
    uid: &str,
    resource_version: &str,
) -> Result<(), AppError> {
    if request.expected_revision_uid.as_deref() != Some(uid)
        || request.expected_revision_resource_version.as_deref() != Some(resource_version)
    {
        return Err(AppError::new(
            "rollback revision changed since the action was reviewed",
            AppErrorKind::Validation,
        ));
    }
    Ok(())
}

pub(super) fn state_from_value(
    target: ClusterOperationTarget,
    uid: String,
    resource_version: String,
    value: Value,
    permissions: Vec<ResourceActionPermission>,
    revisions: Vec<ResourceActionRevision>,
) -> ResourceActionState {
    let containers_path = if target.kind == "Pod" {
        "/spec/containers"
    } else {
        "/spec/template/spec/containers"
    };
    let containers = value
        .pointer(containers_path)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|container| {
            let env = container
                .get("env")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|entry| {
                    let name = entry.get("name")?.as_str()?.to_string();
                    let value = entry
                        .get("value")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    let referenced = entry.get("valueFrom").is_some();
                    Some(ResourceActionEnv {
                        name,
                        value,
                        referenced,
                    })
                })
                .collect();
            let resources = container.get("resources");
            let strings = |field: &str| {
                resources
                    .and_then(|value| value.get(field))
                    .and_then(Value::as_object)
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(|(name, value)| {
                                value.as_str().map(|v| (name.clone(), v.to_string()))
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            };
            ResourceActionContainer {
                name: container
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .into(),
                image: container
                    .get("image")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .into(),
                env,
                requests: strings("requests"),
                limits: strings("limits"),
            }
        })
        .collect();
    let string_map = |key: &str| {
        value
            .pointer(&format!("/metadata/{key}"))
            .and_then(Value::as_object)
            .map(|map| {
                map.iter()
                    .filter_map(|(key, value)| {
                        value.as_str().map(|value| (key.clone(), value.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    ResourceActionState {
        target,
        uid,
        resource_version,
        replicas: value
            .pointer("/spec/replicas")
            .and_then(Value::as_i64)
            .and_then(|v| i32::try_from(v).ok()),
        paused: value.pointer("/spec/paused").and_then(Value::as_bool),
        containers,
        labels: string_map("labels"),
        annotations: string_map("annotations"),
        permissions,
        revisions,
    }
}

pub(super) fn action_value(object: &DynamicObject) -> Result<Value, AppError> {
    let value = serde_json::to_value(object)
        .map_err(|error| AppError::new(error.to_string(), AppErrorKind::Serialization))?;
    let mut value = value;
    clean_action_display_metadata(&mut value);
    Ok(value)
}

pub(super) fn action_identity(object: &DynamicObject) -> Result<(String, String), AppError> {
    let uid = object
        .metadata
        .uid
        .clone()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::new("resource UID is missing", AppErrorKind::InvalidResource))?;
    let resource_version = object
        .metadata
        .resource_version
        .clone()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::new("resourceVersion is missing", AppErrorKind::InvalidResource)
        })?;
    Ok((uid, resource_version))
}

pub(super) fn action_yaml(value: &Value) -> Result<String, AppError> {
    let mut value = value.clone();
    crate::commands::helpers::normalize_k8s_yaml_value(
        &mut value,
        crate::models::YamlViewMode::ApplyClean,
    );
    crate::commands::helpers::serialize_json_value_document(
        &value,
        crate::models::YamlEncoding::Yaml,
    )
}

pub(super) fn clean_action_display_metadata(value: &mut Value) {
    crate::commands::helpers::normalize_k8s_yaml_value(value, crate::models::YamlViewMode::Kubectl);
    if value.get("kind").and_then(Value::as_str) == Some("Secret") {
        if let Some(map) = value.as_object_mut() {
            map.remove("data");
            map.remove("stringData");
        }
    }
}

pub(super) fn ensure_action_resource_is_not_secret(value: &Value) -> Result<(), AppError> {
    if value.get("kind").and_then(Value::as_str) == Some("Secret") {
        Err(AppError::new(
            "Secret resources are not supported by resource actions",
            AppErrorKind::UnsupportedOperation,
        ))
    } else {
        Ok(())
    }
}
