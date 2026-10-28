use super::scope::scoped_connection;
use super::transport::resource_actions_path;
use super::{
    connected::{
        api_get, apply_clean_state, connected_application_path, inspector_from_application,
        kubernetes_application, kubernetes_comparison, managed_resource, redact_secret_fields,
        state, text,
    },
    ArgoConnectionStore,
};
use crate::models::AppErrorKind;
use crate::{
    commands::{diagnostic_field, BackendCancellationRegistry, ClusterAccess, FiniteRead},
    models::{AppError, ArgoApplicationRef, ArgoManagedResource, ArgoResourceComparison},
};
use serde_json::Value;

async fn argo_resource_comparison(
    store: &ArgoConnectionStore,
    access: &ClusterAccess,
    cluster_context: String,
    connection_id: Option<String>,
    transport: String,
    application: ArgoApplicationRef,
    resource: ArgoManagedResource,
) -> Result<ArgoResourceComparison, AppError> {
    if transport == "kubernetes" {
        let inspector = inspector_from_application(
            &kubernetes_application(
                access,
                &cluster_context,
                application.namespace.as_deref(),
                &application.name,
            )
            .await?,
        )?;
        return kubernetes_selected_comparison(inspector, &resource);
    }
    if transport != "connected" {
        return Err(AppError::new(
            "invalid Argo CD transport",
            AppErrorKind::ArgoConnection,
        ));
    }
    let connection = scoped_connection(
        store,
        &connection_id.ok_or_else(|| {
            AppError::new("Argo CD connection required", AppErrorKind::ArgoConnection)
        })?,
        &cluster_context,
        application.workspace_id.as_deref(),
        access.source_key(),
    )?;
    let value = api_get(
        &connection,
        &connected_application_path(
            &connection.profile.url,
            &application.name,
            application.namespace.as_deref(),
            None,
            true,
        )?,
    )
    .await?;
    let mut comparison = connected_selected_comparison(value, &resource)?;
    comparison.available_actions = match actions_path(&application, &comparison.resource) {
        Some(path) => api_get(&connection, &path)
            .await?
            .get("actions")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        None => vec![],
    };
    Ok(comparison)
}

fn kubernetes_selected_comparison(
    inspector: crate::models::ArgoApplicationInspector,
    requested: &ArgoManagedResource,
) -> Result<ArgoResourceComparison, AppError> {
    let resource = inspector
        .resources
        .into_iter()
        .find(|candidate| {
            candidate.group == requested.group
                && candidate.version == requested.version
                && candidate.kind == requested.kind
                && candidate.namespace == requested.namespace
                && candidate.name == requested.name
        })
        .ok_or_else(|| AppError::new("managed resource not found", AppErrorKind::NotFound))?;
    Ok(kubernetes_comparison(resource))
}

fn connected_selected_comparison(
    mut value: Value,
    requested: &ArgoManagedResource,
) -> Result<ArgoResourceComparison, AppError> {
    redact_secret_fields(&mut value);
    let item = value
        .get("items")
        .or_else(|| value.get("managedResources"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|item| {
            text(item, "group") == requested.group
                && text(item, "version") == requested.version
                && text(item, "kind") == requested.kind
                && text(item, "namespace") == requested.namespace
                && text(item, "name") == requested.name
        })
        .ok_or_else(|| AppError::new("managed resource not found", AppErrorKind::NotFound))?;
    let resource = managed_resource(item);
    Ok(ArgoResourceComparison {
        resource,
        target_state: state(item.get("targetState"), true).map(apply_clean_state),
        live_state: state(item.get("liveState"), true),
        normalized_live_state: state(item.get("normalizedLiveState"), true).map(apply_clean_state),
        predicted_live_state: state(item.get("predictedLiveState"), true),
        modified: item.get("modified").and_then(Value::as_bool),
        exact: Some(true),
        provenance: Some("argocd-managed-resource".into()),
        available_actions: Vec::new(),
    })
}

#[cfg(feature = "bench-support")]
pub fn benchmark_kubernetes_selected_comparison(
    object: &kube::core::DynamicObject,
    requested: &ArgoManagedResource,
) -> Result<ArgoResourceComparison, AppError> {
    let inspector = inspector_from_application(object)?;
    kubernetes_selected_comparison(inspector, requested)
}

#[cfg(feature = "bench-support")]
pub fn benchmark_connected_selected_comparison(
    value: Value,
    requested: &ArgoManagedResource,
) -> Result<ArgoResourceComparison, AppError> {
    connected_selected_comparison(value, requested)
}

pub async fn get_argo_resource_comparison(
    cluster_context: String,
    kubeconfig_env_var: Option<String>,
    connection_id: Option<String>,
    transport: String,
    application: ArgoApplicationRef,
    resource: ArgoManagedResource,
    request_id: Option<String>,
    cancel_scope: Option<String>,
    store: crate::runtime::State<'_, ArgoConnectionStore>,
    cancellations: crate::runtime::State<'_, BackendCancellationRegistry>,
) -> Result<ArgoResourceComparison, AppError> {
    FiniteRead::new("get_argo_resource_comparison")
        .label("context", &cluster_context)
        .label("transport", &transport)
        .label("application", &application.name)
        .cancellable(&cancellations, cancel_scope, request_id)
        .run(
            Box::pin(async {
                let access = ClusterAccess::resolve(kubeconfig_env_var.as_deref())?;
                argo_resource_comparison(
                    &store,
                    &access,
                    cluster_context,
                    connection_id,
                    transport,
                    application,
                    resource,
                )
                .await
            }),
            |comparison| {
                vec![diagnostic_field(
                    "modified",
                    comparison
                        .modified
                        .map_or_else(|| "unknown".to_string(), |modified| modified.to_string()),
                )]
            },
        )
        .await
}

fn required_identity(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.trim().is_empty())
}

fn actions_path(
    application: &ArgoApplicationRef,
    resource: &ArgoManagedResource,
) -> Option<String> {
    required_identity(resource.version.as_deref())?;
    required_identity(resource.kind.as_deref())?;
    required_identity(resource.name.as_deref())?;
    Some(resource_actions_path(application, resource))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_path_encodes_application_and_resource_identity() {
        let path = actions_path(
            &ArgoApplicationRef {
                name: "app/a?#".into(),
                namespace: Some("app ns/&?#".into()),
                project: Some("project/&?#".into()),
                ..Default::default()
            },
            &ArgoManagedResource {
                group: Some("apps/&?#".into()),
                version: Some("v1/?.".into()),
                kind: Some("Deployment/#".into()),
                namespace: Some("ns/&?#".into()),
                name: Some("resource/&?#".into()),
                ..Default::default()
            },
        );
        assert_eq!(
            path.as_deref(),
            Some("/api/v1/applications/app%2Fa%3F%23/resource/actions?appNamespace=app+ns%2F%26%3F%23&project=project%2F%26%3F%23&group=apps%2F%26%3F%23&version=v1%2F%3F.&kind=Deployment%2F%23&namespace=ns%2F%26%3F%23&resourceName=resource%2F%26%3F%23")
        );
    }

    #[test]
    fn actions_path_requires_server_action_identity() {
        let application = ArgoApplicationRef {
            name: "app".into(),
            ..Default::default()
        };
        let complete = ArgoManagedResource {
            version: Some("v1".into()),
            kind: Some("ConfigMap".into()),
            name: Some("settings".into()),
            ..Default::default()
        };

        assert!(actions_path(&application, &complete).is_some());
        for incomplete in [
            ArgoManagedResource {
                version: None,
                ..complete.clone()
            },
            ArgoManagedResource {
                kind: Some(" ".into()),
                ..complete.clone()
            },
            ArgoManagedResource {
                name: Some(String::new()),
                ..complete.clone()
            },
        ] {
            assert!(actions_path(&application, &incomplete).is_none());
        }
    }
}
