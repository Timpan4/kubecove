use super::scope::scoped_connection;
use super::transport::resource_actions_path;
use super::{
    ArgoConnectionStore,
    connected::{
        api_get, connected_comparison, kubernetes_application, kubernetes_selected_comparison,
        managed_resource_path, redact_secret_fields,
    },
};
use crate::models::AppErrorKind;
use crate::{
    commands::{BackendCancellationRegistry, ClusterAccess, FiniteRead, diagnostic_field},
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
        let object = kubernetes_application(
            access,
            &cluster_context,
            application.namespace.as_deref(),
            &application.name,
        )
        .await?;
        return kubernetes_selected_comparison(&object, &resource);
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
        &managed_resource_path(&connection.profile.url, &application, &resource)?,
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

/// Argo CD's managed-resource items carry no API version, and cluster-scoped items an
/// empty namespace, so they match on the identity the frontend keys by: group, kind,
/// namespace and name, with empty meaning absent.
fn same_identity(item: &Value, requested: &ArgoManagedResource) -> bool {
    let matches = |key, requested: Option<&str>| {
        item.get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            == requested.filter(|value| !value.is_empty())
    };
    matches("group", requested.group.as_deref())
        && matches("kind", requested.kind.as_deref())
        && matches("namespace", requested.namespace.as_deref())
        && matches("name", requested.name.as_deref())
}

fn connected_selected_comparison(
    mut value: Value,
    requested: &ArgoManagedResource,
) -> Result<ArgoResourceComparison, AppError> {
    let field = if value.get("items").is_some() {
        "items"
    } else {
        "managedResources"
    };
    let item = value
        .get_mut(field)
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
        .find(|item| same_identity(item, requested))
        .ok_or_else(|| AppError::new("managed resource not found", AppErrorKind::NotFound))?;
    redact_secret_fields(item);
    let mut comparison = connected_comparison(item);
    // Server actions need the version the item lacks.
    comparison.resource.version = comparison
        .resource
        .version
        .or_else(|| requested.version.clone());
    Ok(comparison)
}

#[cfg(feature = "bench-support")]
pub fn benchmark_kubernetes_selected_comparison(
    object: &kube::core::DynamicObject,
    requested: &ArgoManagedResource,
) -> Result<ArgoResourceComparison, AppError> {
    kubernetes_selected_comparison(object, requested)
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
    fn connected_comparison_matches_versionless_items_by_frontend_identity() {
        let managed = serde_json::json!({"items": [
            {"group": "apps", "kind": "Deployment", "namespace": "argocd", "name": "argocd-repo-server",
             "liveState": "{\"kind\":\"Deployment\"}"},
            {"group": "apps", "kind": "Deployment", "namespace": "argocd", "name": "argocd-server",
             "liveState": "{\"kind\":\"Deployment\"}", "modified": false},
            {"group": "apiextensions.k8s.io", "kind": "CustomResourceDefinition", "namespace": "",
             "name": "applications.argoproj.io", "liveState": "{\"kind\":\"CustomResourceDefinition\"}"}
        ]});
        let deployment = ArgoManagedResource {
            group: Some("apps".into()),
            version: Some("v1".into()),
            kind: Some("Deployment".into()),
            namespace: Some("argocd".into()),
            name: Some("argocd-server".into()),
            ..Default::default()
        };
        let comparison = connected_selected_comparison(managed.clone(), &deployment).unwrap();
        assert_eq!(comparison.resource.name.as_deref(), Some("argocd-server"));
        assert_eq!(comparison.resource.version.as_deref(), Some("v1"));
        assert_eq!(comparison.modified, Some(false));

        let crd = ArgoManagedResource {
            group: Some("apiextensions.k8s.io".into()),
            version: Some("v1".into()),
            kind: Some("CustomResourceDefinition".into()),
            name: Some("applications.argoproj.io".into()),
            ..Default::default()
        };
        assert!(connected_selected_comparison(managed.clone(), &crd).is_ok());

        let other_kind = ArgoManagedResource {
            kind: Some("StatefulSet".into()),
            ..deployment
        };
        let missing = connected_selected_comparison(managed, &other_kind).unwrap_err();
        assert_eq!(missing.kind, AppErrorKind::NotFound);
    }

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
            Some(
                "/api/v1/applications/app%2Fa%3F%23/resource/actions?appNamespace=app+ns%2F%26%3F%23&project=project%2F%26%3F%23&group=apps%2F%26%3F%23&version=v1%2F%3F.&kind=Deployment%2F%23&namespace=ns%2F%26%3F%23&resourceName=resource%2F%26%3F%23"
            )
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
