use super::scope::scoped_connection;
use super::{
    connected::{
        api_get, inspector_from_application, kubernetes_application, managed_resource,
        redact_secret_fields, state, text,
    },
    ArgoConnectionStore,
};
use crate::models::{AppError, ArgoApplicationRef, ArgoManagedResource, ArgoResourceComparison};
use serde_json::Value;

#[tauri::command]
pub async fn get_argo_resource_comparison(
    store: tauri::State<'_, ArgoConnectionStore>,
    cluster_context: String,
    kubeconfig_env_var: Option<String>,
    connection_id: Option<String>,
    transport: String,
    application: ArgoApplicationRef,
    resource: ArgoManagedResource,
    _redact_secrets: Option<bool>,
) -> Result<ArgoResourceComparison, AppError> {
    if transport == "kubernetes" {
        let inspector = inspector_from_application(
            &kubernetes_application(
                &cluster_context,
                application.namespace.as_deref(),
                &application.name,
                kubeconfig_env_var,
            )
            .await?,
        )?;
        let resource = inspector
            .resources
            .into_iter()
            .find(|candidate| {
                candidate.group == resource.group
                    && candidate.version == resource.version
                    && candidate.kind == resource.kind
                    && candidate.namespace == resource.namespace
                    && candidate.name == resource.name
            })
            .ok_or_else(|| AppError::new("managed resource not found", "notFound"))?;
        return Ok(ArgoResourceComparison {
            resource,
            exact: Some(false),
            provenance: Some("kubernetes-status-no-diff".into()),
            ..Default::default()
        });
    }
    if transport != "connected" {
        return Err(AppError::new("invalid Argo CD transport", "argoConnection"));
    }
    let connection = scoped_connection(
        &store,
        &connection_id
            .ok_or_else(|| AppError::new("Argo CD connection required", "argoConnection"))?,
        &cluster_context,
        application.workspace_id.as_deref(),
    )?;
    let mut value = api_get(
        &connection,
        &format!(
            "/api/v1/applications/{}/managed-resources?appNamespace={}",
            application.name,
            application.namespace.clone().unwrap_or_default()
        ),
    )
    .await?;
    redact_secret_fields(&mut value);
    let item = value
        .get("items")
        .or_else(|| value.get("managedResources"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|item| {
            text(item, "group") == resource.group
                && text(item, "version") == resource.version
                && text(item, "kind") == resource.kind
                && text(item, "namespace") == resource.namespace
                && text(item, "name") == resource.name
        })
        .ok_or_else(|| AppError::new("managed resource not found", "notFound"))?;
    let available_actions = api_get(&connection, &actions_path(&application, &resource))
        .await?
        .get("actions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(ArgoResourceComparison {
        resource: managed_resource(item),
        target_state: state(item.get("targetState"), true),
        live_state: state(item.get("liveState"), true),
        normalized_live_state: state(item.get("normalizedLiveState"), true),
        predicted_live_state: state(item.get("predictedLiveState"), true),
        modified: item.get("modified").and_then(Value::as_bool),
        exact: Some(true),
        provenance: Some("argocd-managed-resource".into()),
        available_actions,
    })
}
fn actions_path(application: &ArgoApplicationRef, resource: &ArgoManagedResource) -> String {
    let mut url =
        reqwest::Url::parse("https://argo.invalid/api/v1/applications").expect("static URL");
    url.path_segments_mut()
        .expect("static URL")
        .push(&application.name);
    url.set_path(&format!("{}/resource/actions", url.path()));
    let mut query = url.query_pairs_mut();
    for (key, value) in [
        ("appNamespace", application.namespace.as_deref()),
        ("project", application.project.as_deref()),
        ("group", resource.group.as_deref()),
        ("version", resource.version.as_deref()),
        ("kind", resource.kind.as_deref()),
        ("namespace", resource.namespace.as_deref()),
        ("resourceName", resource.name.as_deref()),
    ] {
        if let Some(value) = value {
            query.append_pair(key, value);
        }
    }
    drop(query);
    format!("{}?{}", url.path(), url.query().unwrap_or_default())
}
