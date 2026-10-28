use crate::commands::helpers::{
    list_params, normalize_k8s_yaml_value, serialize_json_value_document,
};
use crate::commands::{
    diagnostic_field, BackendCancellationRegistry, ClusterAccess, ClusterLiveStore, FiniteRead,
};
use crate::models::AppErrorKind;
use crate::models::{
    AppError, DiscoveredResourceKind, ResourceDetailsFull, ResourceSummary, YamlEncoding,
    YamlViewMode,
};
use crate::runtime::State;
use kube::api::{Api, DynamicObject};
#[cfg(feature = "bench-support")]
pub use kubecove_kube::commands::dynamic_projection::dynamic_resource_summaries_at;
pub(crate) use kubecove_kube::commands::dynamic_projection::{
    api_resource_from_discovered, dynamic_resource_summary, dynamic_resource_summary_at,
};
use serde_json::Value;
use std::sync::Arc;

fn dynamic_status_value(data: &Value) -> Option<Value> {
    data.get("status").cloned()
}

fn is_core_v1_secret(resource_kind: &DiscoveredResourceKind) -> bool {
    resource_kind.group.is_empty()
        && resource_kind.version == "v1"
        && resource_kind.plural == "secrets"
}

fn redact_dynamic_secret_fields(value: &mut Value) {
    if let Some(annotation) =
        value.pointer_mut("/metadata/annotations/kubectl.kubernetes.io~1last-applied-configuration")
    {
        *annotation = Value::String("REDACTED".into());
    }
    let Value::Object(root) = value else {
        return;
    };

    for field in ["data", "stringData"] {
        if let Some(Value::Object(values)) = root.get_mut(field) {
            for value in values.values_mut() {
                *value = Value::String("REDACTED".to_string());
            }
        }
    }
}

pub fn serialize_dynamic_resource_document(
    resource_kind: &DiscoveredResourceKind,
    object: &DynamicObject,
    mode: YamlViewMode,
    encoding: YamlEncoding,
) -> Result<String, AppError> {
    let mut value = serde_json::to_value(object)
        .map_err(|e| AppError::new(e.to_string(), AppErrorKind::Serialization).with_source(e))?;
    normalize_k8s_yaml_value(&mut value, mode);
    if is_core_v1_secret(resource_kind) {
        redact_dynamic_secret_fields(&mut value);
    }
    serialize_json_value_document(&value, encoding)
}

pub async fn dynamic_resources_summary_from(
    access: &ClusterAccess,
    cluster_context: String,
    resource_kind: DiscoveredResourceKind,
    namespace: Option<String>,
) -> Result<Vec<ResourceSummary>, AppError> {
    crate::commands::helpers::validate_namespace(namespace.as_deref())?;
    let client = access.read_client(&cluster_context).await?;
    let api_resource = api_resource_from_discovered(&resource_kind)?;
    let api: Api<DynamicObject> = if resource_kind.namespaced {
        if let Some(namespace) = namespace.as_deref() {
            Api::namespaced_with(client, namespace, &api_resource)
        } else {
            Api::all_with(client, &api_resource)
        }
    } else {
        Api::all_with(client, &api_resource)
    };

    let objects = kubecove_kube::commands::list_projection::list::<
        _,
        kubecove_kube::commands::list_projection::Dynamic,
    >(api, &list_params())
    .await
    .map_err(AppError::from)?;
    Ok(objects
        .items
        .into_iter()
        .map(|object| dynamic_resource_summary(&cluster_context, &resource_kind, &object.into()))
        .collect())
}

pub async fn list_dynamic_resources(
    cluster_context: String,
    resource_kind: DiscoveredResourceKind,
    namespace: Option<String>,
    kubeconfig_env_var: Option<String>,
    live_store: State<'_, ClusterLiveStore>,
) -> Result<Arc<Vec<ResourceSummary>>, AppError> {
    FiniteRead::new("list_dynamic_resources")
        .label("context", &cluster_context)
        .label("kind", &resource_kind.kind)
        .label("api_version", &resource_kind.api_version)
        .label("namespace", namespace.as_deref().unwrap_or("<all>"))
        .run(
            async {
                let access = ClusterAccess::resolve(kubeconfig_env_var.as_deref())?;
                live_store
                    .dynamic_resources(
                        access.source_key().to_string(),
                        cluster_context.clone(),
                        resource_kind.clone(),
                        namespace.clone(),
                        {
                            let cluster_context = cluster_context.clone();
                            let resource_kind = resource_kind.clone();
                            let namespace = namespace.clone();
                            move || async move {
                                dynamic_resources_summary_from(
                                    &access,
                                    cluster_context,
                                    resource_kind,
                                    namespace,
                                )
                                .await
                            }
                        },
                    )
                    .await
            },
            |rows| {
                vec![
                    diagnostic_field("kind", &resource_kind.kind),
                    diagnostic_field("rows", rows.len()),
                ]
            },
        )
        .await
}

pub async fn dynamic_resource_details_from(
    access: &ClusterAccess,
    cluster_context: String,
    resource_kind: DiscoveredResourceKind,
    name: String,
    namespace: Option<String>,
    yaml_view_mode: Option<YamlViewMode>,
    yaml_encoding: Option<YamlEncoding>,
) -> Result<ResourceDetailsFull, AppError> {
    crate::commands::helpers::validate_path_segment(&name, "resource name")?;
    crate::commands::helpers::validate_namespace(namespace.as_deref())?;
    if resource_kind.namespaced && namespace.is_none() {
        return Err(AppError::new(
            format!("namespace required for {}", resource_kind.kind),
            AppErrorKind::Cluster,
        ));
    }

    let client = access.read_client(&cluster_context).await?;
    let api_resource = api_resource_from_discovered(&resource_kind)?;
    let api: Api<DynamicObject> = if resource_kind.namespaced {
        Api::namespaced_with(
            client,
            namespace
                .as_deref()
                .expect("namespaced resource has namespace"),
            &api_resource,
        )
    } else {
        Api::all_with(client, &api_resource)
    };

    let object = api.get(&name).await.map_err(AppError::from)?;
    dynamic_resource_details_from_object(
        cluster_context,
        resource_kind,
        object,
        yaml_view_mode.unwrap_or_default(),
        yaml_encoding.unwrap_or_default(),
    )
}

pub fn dynamic_resource_details_from_object(
    cluster_context: String,
    resource_kind: DiscoveredResourceKind,
    mut object: DynamicObject,
    yaml_view_mode: YamlViewMode,
    yaml_encoding: YamlEncoding,
) -> Result<ResourceDetailsFull, AppError> {
    if is_core_v1_secret(&resource_kind) {
        crate::commands::helpers::redact_secret_metadata(&mut object.metadata);
    }
    let yaml = serialize_dynamic_resource_document(
        &resource_kind,
        &object,
        yaml_view_mode,
        yaml_encoding,
    )?;
    let metadata = serde_json::to_value(&object.metadata)
        .map_err(|e| AppError::new(e.to_string(), AppErrorKind::Serialization).with_source(e))?;
    let status = dynamic_status_value(&object.data);
    let summary = dynamic_resource_summary(&cluster_context, &resource_kind, &object);

    Ok(ResourceDetailsFull {
        summary,
        yaml,
        metadata,
        status,
    })
}

pub async fn get_dynamic_resource_details(
    cluster_context: String,
    resource_kind: DiscoveredResourceKind,
    name: String,
    namespace: Option<String>,
    kubeconfig_env_var: Option<String>,
    yaml_view_mode: Option<YamlViewMode>,
    yaml_encoding: Option<YamlEncoding>,
    request_id: Option<String>,
    cancel_scope: Option<String>,
    cancellations: State<'_, BackendCancellationRegistry>,
) -> Result<ResourceDetailsFull, AppError> {
    FiniteRead::new("get_dynamic_resource_details")
        .label("context", &cluster_context)
        .label("kind", &resource_kind.kind)
        .label("api_version", &resource_kind.api_version)
        .label("namespace", namespace.as_deref().unwrap_or("<cluster>"))
        .label("name", &name)
        .cancellable(&cancellations, cancel_scope, request_id)
        .run(
            {
                let resource_kind = resource_kind.clone();
                async move {
                    let access = ClusterAccess::resolve(kubeconfig_env_var.as_deref())?;
                    dynamic_resource_details_from(
                        &access,
                        cluster_context,
                        resource_kind,
                        name,
                        namespace,
                        yaml_view_mode,
                        yaml_encoding,
                    )
                    .await
                }
            },
            |details| {
                vec![
                    diagnostic_field("kind", &resource_kind.kind),
                    diagnostic_field("yamlBytes", details.yaml.len()),
                    diagnostic_field("hasStatus", details.status.is_some()),
                ]
            },
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{HealthAssessmentState, ResourceHealth};
    use kube::api::ApiResource;
    use kubecove_kube::commands::dynamic_projection::dynamic_status_from_data;
    use serde_json::json;

    #[test]
    fn discovered_kind_cannot_redirect_the_resource_path() {
        for plural in ["secrets/token?ignored=", "../secrets", "%2e%2e", "pods/log"] {
            let mut kind = widget_kind();
            kind.plural = plural.into();
            assert!(api_resource_from_discovered(&kind).is_err());
        }
    }

    fn widget_kind() -> DiscoveredResourceKind {
        DiscoveredResourceKind {
            group: "example.com".to_string(),
            version: "v1".to_string(),
            api_version: "example.com/v1".to_string(),
            kind: "Widget".to_string(),
            plural: "widgets".to_string(),
            short_names: Vec::new(),
            namespaced: true,
        }
    }

    fn secret_kind() -> DiscoveredResourceKind {
        DiscoveredResourceKind {
            group: String::new(),
            version: "v1".to_string(),
            api_version: "v1".to_string(),
            kind: "Secret".to_string(),
            plural: "secrets".to_string(),
            short_names: Vec::new(),
            namespaced: true,
        }
    }

    #[test]
    fn rejects_invalid_discovered_kind() {
        let mut kind = widget_kind();
        kind.plural = String::new();

        let err = api_resource_from_discovered(&kind).unwrap_err();
        assert_eq!(err.kind, AppErrorKind::Cluster);
        assert_eq!(err.message, "invalid discovered resource kind");
    }

    #[test]
    fn extracts_dynamic_status_from_phase_or_conditions() {
        assert_eq!(
            dynamic_status_from_data(&json!({ "status": { "phase": "Running" } })),
            Some("Running".to_string())
        );
        assert_eq!(
            dynamic_status_from_data(&json!({
                "status": {
                    "conditions": [
                        { "type": "Available", "status": "False" },
                        { "type": "Ready", "status": "True" }
                    ]
                }
            })),
            Some("Ready: True".to_string())
        );
    }

    #[test]
    fn assesses_argocd_application_with_same_argo_contract() {
        let mut resource_kind = widget_kind();
        resource_kind.group = "argoproj.io".to_string();
        resource_kind.api_version = "argoproj.io/v1".to_string();
        resource_kind.plural = "applications".to_string();
        resource_kind.kind = "Application".to_string();
        let api_resource = api_resource_from_discovered(&resource_kind).unwrap();
        let object = DynamicObject::new("payments", &api_resource).data(json!({
            "status": { "health": { "status": "Missing" }, "sync": { "status": "OutOfSync" } }
        }));

        let summary = dynamic_resource_summary("kind-kind", &resource_kind, &object);

        assert_eq!(
            summary.health_assessment.state,
            HealthAssessmentState::Degraded
        );
        assert_eq!(summary.health, ResourceHealth::Degraded);
    }

    #[test]
    fn ignores_unrecognized_dynamic_conditions() {
        let resource_kind = widget_kind();
        let api_resource = api_resource_from_discovered(&resource_kind).unwrap();
        let object = DynamicObject::new("sample-widget", &api_resource).data(json!({
            "status": { "conditions": [{ "type": "ObservedGeneration", "status": "False" }] }
        }));

        assert_eq!(
            dynamic_resource_summary("kind-kind", &resource_kind, &object)
                .health_assessment
                .state,
            HealthAssessmentState::NotEvaluated
        );
    }

    #[test]
    fn redacts_core_v1_secret_dynamic_yaml() {
        let resource_kind = secret_kind();
        let api_resource = api_resource_from_discovered(&resource_kind).unwrap();
        let object = DynamicObject::new("api-token", &api_resource)
            .within("default")
            .data(json!({
                "data": {
                    "password": "c3VwZXItc2VjcmV0LXBhc3N3b3Jk"
                },
                "stringData": {
                    "token": "plain-secret"
                },
                "type": "Opaque"
            }));

        let yaml = serialize_dynamic_resource_document(
            &resource_kind,
            &object,
            YamlViewMode::Kubectl,
            YamlEncoding::Yaml,
        )
        .unwrap();

        assert!(yaml.contains("REDACTED"));
        assert!(!yaml.contains("c3VwZXItc2VjcmV0LXBhc3N3b3Jk"));
        assert!(!yaml.contains("plain-secret"));
    }

    #[test]
    fn audit_secret_endpoint_is_redacted_despite_forged_kind() {
        let mut resource_kind = secret_kind();
        resource_kind.kind = "Widget".into();
        resource_kind.api_version = "example.com/v1".into();
        let object = DynamicObject::new(
            "token",
            &ApiResource::erase::<k8s_openapi::api::core::v1::Secret>(&()),
        )
        .data(json!({"data":{"password":"sensitive"}}));
        let yaml = serialize_dynamic_resource_document(
            &resource_kind,
            &object,
            YamlViewMode::Kubectl,
            YamlEncoding::Yaml,
        )
        .unwrap();
        assert!(!yaml.contains("sensitive"));
    }
}
