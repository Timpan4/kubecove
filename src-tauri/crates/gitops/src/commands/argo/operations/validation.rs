use super::super::connected::api_get;
use super::payload::resource_action_path;
use crate::commands::ClusterAccess;
use crate::models::AppErrorKind;
use crate::models::{AppError, ArgoApplicationRef, ArgoOperationRequest};
use k8s_openapi::api::authorization::v1::{
    ResourceAttributes, SelfSubjectAccessReview, SelfSubjectAccessReviewSpec,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::{
    api::{Api, PostParams},
    core::DynamicObject,
};
use serde_json::Value;

/// Kubernetes-transport fallback only supports a subset of the connected
/// actions; enforced at preflight and again at execution so a stale or
/// forged session can never reach the patch builder.
pub(super) const KUBERNETES_TRANSPORT_ACTIONS: [&str; 4] =
    ["refresh", "hardRefresh", "sync", "retry"];

pub(super) fn ensure_kubernetes_transport_action(action: &str) -> Result<(), AppError> {
    if KUBERNETES_TRANSPORT_ACTIONS.contains(&action) {
        Ok(())
    } else {
        Err(AppError::new(
            "operation unavailable in Kubernetes fallback",
            AppErrorKind::ArgoOperationUnavailable,
        ))
    }
}

pub(super) fn valid(request: &ArgoOperationRequest) -> Result<(), AppError> {
    if request.transport == "kubernetes" {
        crate::commands::helpers::validate_namespace(request.application.namespace.as_deref())?;
        crate::commands::helpers::validate_path_segment(
            &request.application.name,
            "Application name",
        )?;
    }
    if !matches!(request.transport.as_str(), "connected" | "kubernetes") {
        return Err(AppError::new(
            "invalid Argo CD transport",
            AppErrorKind::ArgoOperationUnavailable,
        ));
    }
    if !matches!(
        request.action.as_str(),
        "refresh" | "hardRefresh" | "sync" | "retry" | "rollback" | "terminate" | "resourceAction"
    ) {
        return Err(AppError::new(
            "operation is not allowlisted",
            AppErrorKind::ArgoOperationUnavailable,
        ));
    }
    if request.application.name.trim().is_empty() {
        return Err(AppError::new(
            "application name required",
            AppErrorKind::ArgoOperationUnavailable,
        ));
    }
    if request.action == "rollback" && request.history_id.is_none() {
        return Err(AppError::new(
            "history ID required for rollback",
            AppErrorKind::ArgoOperationUnavailable,
        ));
    }
    if request.action == "resourceAction" {
        let resource = request.resources.first();
        if request.resource_action.as_deref().is_none_or(str::is_empty)
            || request.resources.len() != 1
            || resource.is_none_or(|resource| {
                [
                    resource.version.as_deref(),
                    resource.kind.as_deref(),
                    resource.name.as_deref(),
                ]
                .into_iter()
                .any(|value| value.is_none_or(|value| value.trim().is_empty()))
            })
        {
            return Err(AppError::new(
                "one server-reported resource action and complete resource identity required",
                AppErrorKind::ArgoOperationUnavailable,
            ));
        }
    }
    Ok(())
}

pub(super) async fn fallback_allowed(
    access: &ClusterAccess,
    request: &ArgoOperationRequest,
) -> Result<(), AppError> {
    let context = request
        .cluster_context
        .as_deref()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            AppError::new(
                "clusterContext required for Kubernetes fallback",
                AppErrorKind::ArgoOperationUnavailable,
            )
        })?;
    let namespace = request
        .application
        .namespace
        .as_deref()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            AppError::new(
                "application namespace required for Kubernetes fallback",
                AppErrorKind::ArgoOperationUnavailable,
            )
        })?;
    if request
        .resource_version
        .as_deref()
        .is_none_or(str::is_empty)
    {
        return Err(AppError::new(
            "resourceVersion required for Kubernetes fallback",
            AppErrorKind::ArgoOperationUnavailable,
        ));
    }
    let client = access.preview_client(context).await?;
    let review = SelfSubjectAccessReview {
        metadata: ObjectMeta::default(),
        spec: SelfSubjectAccessReviewSpec {
            resource_attributes: Some(ResourceAttributes {
                group: Some("argoproj.io".into()),
                resource: Some("applications".into()),
                verb: Some("patch".into()),
                namespace: Some(namespace.into()),
                name: Some(request.application.name.clone()),
                version: None,
                subresource: None,
                field_selector: None,
                label_selector: None,
            }),
            non_resource_attributes: None,
        },
        status: None,
    };
    let status = Api::<SelfSubjectAccessReview>::all(client)
        .create(&PostParams::default(), &review)
        .await?
        .status;
    if !status.is_some_and(|value| value.allowed && !value.denied.unwrap_or(false)) {
        return Err(AppError::new(
            "operation unavailable or not authorized",
            AppErrorKind::ArgoOperationUnavailable,
        ));
    }
    Ok(())
}

pub(super) fn verify_kubernetes_application_identity(
    current: &DynamicObject,
    expected: &ArgoApplicationRef,
) -> Result<(), AppError> {
    let value = serde_json::to_value(current).map_err(|error| {
        AppError::new(
            "could not serialize Application identity",
            AppErrorKind::Serialization,
        )
        .with_source(error)
    })?;
    verify_application_identity(&value, expected)
}

#[cfg(feature = "bench-support")]
pub fn benchmark_verify_kubernetes_application_identity(
    current: &DynamicObject,
    expected: &ArgoApplicationRef,
) -> Result<(), AppError> {
    verify_kubernetes_application_identity(current, expected)
}

pub(super) fn verify_application_identity(
    value: &Value,
    expected: &ArgoApplicationRef,
) -> Result<(), AppError> {
    let metadata = value.get("metadata").unwrap_or(value);
    let actual = [
        (
            "name",
            metadata.get("name").and_then(Value::as_str),
            Some(expected.name.as_str()),
        ),
        (
            "namespace",
            metadata.get("namespace").and_then(Value::as_str),
            expected.namespace.as_deref(),
        ),
        (
            "project",
            value.pointer("/spec/project").and_then(Value::as_str),
            expected.project.as_deref(),
        ),
        (
            "uid",
            metadata.get("uid").and_then(Value::as_str),
            expected.uid.as_deref(),
        ),
        (
            "resourceVersion",
            metadata.get("resourceVersion").and_then(Value::as_str),
            expected.resource_version.as_deref(),
        ),
    ];
    if actual
        .into_iter()
        .any(|(_, current, expected)| expected.is_some_and(|expected| current != Some(expected)))
    {
        return Err(AppError::new(
            "Application changed since operation review",
            AppErrorKind::ArgoOperationUnavailable,
        ));
    }
    Ok(())
}

pub(super) async fn validate_resource_action(
    connection: &super::super::connected::ConnectedArgo,
    request: &ArgoOperationRequest,
) -> Result<(), AppError> {
    let actions = api_get(connection, &resource_action_path(request)).await?;
    let action = request.resource_action.as_deref();
    let action_definition = actions
        .get("actions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|item| item.get("name").and_then(Value::as_str) == action || item.as_str() == action);
    if action_definition.is_none()
        || action_definition
            .and_then(|item| item.get("disabled"))
            .and_then(Value::as_bool)
            == Some(true)
    {
        return Err(AppError::new(
            "resource action is not reported by Argo CD",
            AppErrorKind::ArgoOperationUnavailable,
        ));
    }
    let parameters = request
        .resource_action_parameters
        .as_ref()
        .and_then(Value::as_object);
    if request.resource_action_parameters.is_some()
        && parameters.is_none_or(|values| values.values().any(|value| !value.is_string()))
    {
        return Err(AppError::new(
            "resource action parameters must be string values",
            AppErrorKind::ArgoOperationUnavailable,
        ));
    }
    if action_definition
        .unwrap()
        .get("params")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|parameter| {
            parameter.get("required").and_then(Value::as_bool) == Some(true)
                && parameter
                    .get("name")
                    .and_then(Value::as_str)
                    .is_some_and(|name| {
                        parameters
                            .and_then(|values| values.get(name))
                            .and_then(Value::as_str)
                            .is_none_or(str::is_empty)
                    })
        })
    {
        return Err(AppError::new(
            "required resource action parameters missing",
            AppErrorKind::ArgoOperationUnavailable,
        ));
    }
    Ok(())
}
