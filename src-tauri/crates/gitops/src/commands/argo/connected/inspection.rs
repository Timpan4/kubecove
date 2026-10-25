use super::super::transport::redact_secret_fields;
use crate::models::AppErrorKind;
use crate::models::{
    AppError, ArgoApplicationHistory, ArgoApplicationInspector, ArgoApplicationRef,
    ArgoManagedResource, ArgoResourceComparison,
};
use kube::core::DynamicObject;
use serde_json::Value;

pub(crate) fn text(object: &Value, field: &str) -> Option<String> {
    object.get(field).and_then(Value::as_str).map(str::to_owned)
}
pub(crate) fn managed_resource(value: &Value) -> ArgoManagedResource {
    ArgoManagedResource {
        group: text(value, "group"),
        version: text(value, "version"),
        kind: text(value, "kind"),
        namespace: text(value, "namespace"),
        name: text(value, "name"),
        status: text(value, "status"),
        health: value.get("health").and_then(|v| text(v, "status")),
        hook: value.get("hook").and_then(Value::as_bool),
        requires_pruning: value.get("requiresPruning").and_then(Value::as_bool),
        target_state: None,
        live_state: None,
    }
}
pub(crate) fn state(value: Option<&Value>, redact: bool) -> Option<Value> {
    value
        .and_then(|state| match state {
            Value::String(text) => serde_json::from_str::<Value>(text)
                .ok()
                // A JSON string literal is still unstructured text, so it takes the text path below.
                .filter(|parsed| !parsed.is_string())
                .or_else(|| {
                    serde_yaml::from_str::<Value>(text)
                        .ok()
                        .filter(|parsed| parsed.is_object() || parsed.is_array())
                })
                .or_else(|| {
                    // Unparseable text cannot be redacted structurally; withhold it if it may be a Secret.
                    Some(if redact && text.contains("Secret") {
                        Value::String("[REDACTED]".into())
                    } else {
                        Value::String(text.clone())
                    })
                }),
            _ => Some(state.clone()),
        })
        .map(|mut state| {
            if redact {
                redact_secret_fields(&mut state);
            }
            state
        })
}
fn history(value: &Value) -> ArgoApplicationHistory {
    ArgoApplicationHistory {
        id: value.get("id").and_then(Value::as_i64),
        revision: text(value, "revision"),
        revisions: value
            .get("revisions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        deployed_at: text(value, "deployedAt"),
        initiated_by: value.get("initiatedBy").and_then(|v| text(v, "username")),
        source: value.get("source").cloned(),
        sources: value
            .get("sources")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
    }
}

pub(crate) fn inspector_from_application(
    application: &DynamicObject,
) -> Result<ArgoApplicationInspector, AppError> {
    let data = application
        .data
        .as_object()
        .ok_or_else(|| AppError::new("invalid Application data", AppErrorKind::Serialization))?;
    let status = data.get("status").cloned();
    let source = status.as_ref();
    let resources: Vec<_> = source
        .and_then(|value| value.get("resources"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(managed_resource)
        .collect();
    Ok(ArgoApplicationInspector {
        refresh_requested: application
            .metadata
            .annotations
            .as_ref()
            .and_then(|annotations| annotations.get("argocd.argoproj.io/refresh"))
            .cloned(),
        operation_requested: data.get("operation").is_some_and(|value| !value.is_null()),
        application: ArgoApplicationRef {
            name: application.metadata.name.clone().unwrap_or_default(),
            namespace: application.metadata.namespace.clone(),
            project: data.get("spec").and_then(|value| text(value, "project")),
            resource_version: application.metadata.resource_version.clone(),
            uid: application.metadata.uid.clone(),
            api_version: application
                .types
                .as_ref()
                .map(|types| types.api_version.clone()),
            context: None,
            workspace_id: None,
        },
        history: source
            .and_then(|value| value.get("history"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(history)
            .collect(),
        comparisons: resources
            .iter()
            .cloned()
            .map(kubernetes_comparison)
            .collect(),
        resources,
        conditions: source
            .and_then(|value| value.get("conditions"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        operation_state: source
            .and_then(|value| value.get("operationState"))
            .cloned(),
        status,
        connected: false,
        transport: "kubernetes".into(),
        provenance: "kubernetes-status-no-diff".into(),
        connected_fallback: None,
    })
}

pub(crate) fn kubernetes_comparison(resource: ArgoManagedResource) -> ArgoResourceComparison {
    ArgoResourceComparison {
        resource,
        exact: Some(false),
        provenance: Some("kubernetes-status-no-diff".into()),
        ..Default::default()
    }
}

fn connected_comparison(value: &Value) -> ArgoResourceComparison {
    let is_secret = value.get("kind").and_then(Value::as_str) == Some("Secret");
    // Unstructured Secret state cannot be redacted field by field, so it is withheld whole.
    let redacted_state = |key: &str| match state(value.get(key), true) {
        Some(Value::String(_)) if is_secret => Some(Value::String("[REDACTED]".into())),
        other => other,
    };
    ArgoResourceComparison {
        resource: managed_resource(value),
        target_state: redacted_state("targetState"),
        live_state: redacted_state("liveState"),
        normalized_live_state: redacted_state("normalizedLiveState"),
        predicted_live_state: redacted_state("predictedLiveState"),
        modified: value.get("modified").and_then(Value::as_bool),
        exact: Some(true),
        provenance: Some("argocd-managed-resource".into()),
        ..Default::default()
    }
}

pub(super) fn connected_inspector(
    application: ArgoApplicationRef,
    mut response: Value,
    mut managed: Value,
) -> ArgoApplicationInspector {
    redact_secret_fields(&mut response);
    redact_secret_fields(&mut managed);
    let status = response.get("status").cloned();
    let comparisons: Vec<_> = managed
        .get("items")
        .or_else(|| managed.get("managedResources"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(connected_comparison)
        .collect();
    let resources = comparisons
        .iter()
        .map(|comparison| comparison.resource.clone())
        .collect();
    ArgoApplicationInspector {
        refresh_requested: response
            .pointer("/metadata/annotations/argocd.argoproj.io~1refresh")
            .and_then(Value::as_str)
            .map(str::to_owned),
        operation_requested: response
            .get("operation")
            .is_some_and(|value| !value.is_null()),
        application: ArgoApplicationRef {
            resource_version: response
                .pointer("/metadata/resourceVersion")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .or(application.resource_version),
            uid: response
                .pointer("/metadata/uid")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .or(application.uid),
            api_version: response
                .get("apiVersion")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .or(application.api_version),
            context: application.context,
            workspace_id: application.workspace_id,
            ..application
        },
        history: status
            .as_ref()
            .and_then(|value| value.get("history"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(history)
            .collect(),
        resources,
        comparisons,
        conditions: status
            .as_ref()
            .and_then(|value| value.get("conditions"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        operation_state: status
            .as_ref()
            .and_then(|value| value.get("operationState"))
            .cloned(),
        status,
        connected: true,
        transport: "connected".into(),
        provenance: "argocd-api".into(),
        connected_fallback: None,
    }
}

#[cfg(feature = "bench-support")]
pub fn benchmark_connected_inspector(
    application: ArgoApplicationRef,
    response: Value,
    managed: Value,
) -> ArgoApplicationInspector {
    connected_inspector(application, response, managed)
}

#[cfg(feature = "bench-support")]
pub fn benchmark_kubernetes_inspector(
    object: DynamicObject,
    cluster_context: &str,
    application: &ArgoApplicationRef,
) -> Result<ArgoApplicationInspector, AppError> {
    kubernetes_inspector_from_object(object, cluster_context, application.workspace_id.clone())
}

/// Builds the Kubernetes-only inspector after removing Secret fields from the Application.
pub(crate) fn kubernetes_inspector_from_object(
    mut object: DynamicObject,
    cluster_context: &str,
    workspace_id: Option<String>,
) -> Result<ArgoApplicationInspector, AppError> {
    redact_secret_fields(&mut object.data);
    let mut inspector = inspector_from_application(&object)?;
    inspector.application.context = Some(cluster_context.to_string());
    inspector.application.workspace_id = workspace_id;
    Ok(inspector)
}

#[cfg(test)]
mod tests;
