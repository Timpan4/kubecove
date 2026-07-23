use super::connected::{api_delete, api_get, api_post, consume_preflight, issue_preflight};
use super::scope::scoped_connection;
use crate::commands::gitops_crd::{client_for_context, find_api_resource};
use crate::models::{
    AppError, ArgoApplicationRef, ArgoOperationPreflight, ArgoOperationRequest, ArgoOperationResult,
};
use k8s_openapi::api::authorization::v1::{
    ResourceAttributes, SelfSubjectAccessReview, SelfSubjectAccessReviewSpec,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::{
    api::{Api, Patch, PatchParams, PostParams},
    core::DynamicObject,
};
use serde_json::{json, Value};

fn valid(request: &ArgoOperationRequest) -> Result<(), AppError> {
    if !matches!(request.transport.as_str(), "connected" | "kubernetes") {
        return Err(AppError::new(
            "invalid Argo CD transport",
            "argoOperationUnavailable",
        ));
    }
    if !matches!(
        request.action.as_str(),
        "refresh" | "hardRefresh" | "sync" | "retry" | "rollback" | "terminate" | "resourceAction"
    ) {
        return Err(AppError::new(
            "operation is not allowlisted",
            "argoOperationUnavailable",
        ));
    }
    if request.application.name.trim().is_empty() {
        return Err(AppError::new(
            "application name required",
            "argoOperationUnavailable",
        ));
    }
    if request.action == "rollback" && request.history_id.is_none() {
        return Err(AppError::new(
            "history ID required for rollback",
            "argoOperationUnavailable",
        ));
    }
    if request.action == "resourceAction"
        && (request.resource_action.as_deref().is_none_or(str::is_empty)
            || request.resources.len() != 1)
    {
        return Err(AppError::new(
            "one server-reported resource action and resource required",
            "argoOperationUnavailable",
        ));
    }
    Ok(())
}

#[tauri::command]
pub async fn preflight_argo_operation(
    store: tauri::State<'_, super::ArgoConnectionStore>,
    request: ArgoOperationRequest,
) -> Result<ArgoOperationPreflight, AppError> {
    valid(&request)?;
    if request.transport == "connected" {
        let id = request
            .connection_id
            .as_deref()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| AppError::new("Argo CD connection required", "argoConnection"))?;
        let _ = scoped_connection(
            &store,
            id,
            request.cluster_context.as_deref().ok_or_else(|| {
                AppError::new("clusterContext required", "argoOperationUnavailable")
            })?,
            request.application.workspace_id.as_deref(),
        )?;
    } else {
        if !matches!(
            request.action.as_str(),
            "refresh" | "hardRefresh" | "sync" | "retry"
        ) {
            return Err(AppError::new(
                "operation unavailable in Kubernetes fallback",
                "argoOperationUnavailable",
            ));
        }
        fallback_allowed(&request).await?;
    }
    let resolved_request = resolve_request(&store, &request).await?;
    let token = issue_preflight(&store, &resolved_request)?;
    Ok(ArgoOperationPreflight {
        allowed: true,
        transport: request.transport.clone(),
        action: request.action.clone(),
        reason: None,
        preflight_token: Some(token),
        resolved_request: Some(resolved_request),
    })
}

async fn fallback_allowed(request: &ArgoOperationRequest) -> Result<(), AppError> {
    let context = request
        .cluster_context
        .as_deref()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            AppError::new(
                "clusterContext required for Kubernetes fallback",
                "argoOperationUnavailable",
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
                "argoOperationUnavailable",
            )
        })?;
    if request.resource_version.as_deref().is_none_or(str::is_empty) {
        return Err(AppError::new(
            "resourceVersion required for Kubernetes fallback",
            "argoOperationUnavailable",
        ));
    }
    let client = client_for_context(context, request.kubeconfig_env_var.clone()).await?;
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
            "argoOperationUnavailable",
        ));
    }
    Ok(())
}

#[tauri::command]
pub async fn run_argo_operation(
    store: tauri::State<'_, super::ArgoConnectionStore>,
    request: ArgoOperationRequest,
) -> Result<ArgoOperationResult, AppError> {
    valid(&request)?;
    consume_preflight(&store, &request)?;
    if request.transport == "kubernetes" {
        return kubernetes_operation(request).await;
    }
    let connection = scoped_connection(
        &store,
        request
            .connection_id
            .as_deref()
            .ok_or_else(|| AppError::new("Argo CD connection required", "argoConnection"))?,
        request
            .cluster_context
            .as_deref()
            .ok_or_else(|| AppError::new("clusterContext required", "argoOperationUnavailable"))?,
        request.application.workspace_id.as_deref(),
    )?;
    if request.action == "resourceAction" {
        validate_resource_action(&connection, &request).await?;
    }
    let path = match request.action.as_str() {
        "refresh" => application_path(&request.application, "", Some("normal")),
        "hardRefresh" => application_path(&request.application, "", Some("hard")),
        "sync" | "retry" => application_path(&request.application, "/sync", None),
        "rollback" => application_path(&request.application, "/rollback", None),
        "terminate" => application_path(&request.application, "/operation", None),
        "resourceAction" => application_path(&request.application, "/resource/actions/v2", None),
        _ => unreachable!(),
    };
    let value = if request.action == "refresh" || request.action == "hardRefresh" {
        api_get(&connection, &path).await?
    } else if request.action == "terminate" {
        api_delete(&connection, &path).await?
    } else {
        api_post(&connection, &path, connected_payload(&request)).await?
    };
    accepted("connected", value)
}

fn accepted(transport: &str, operation: Value) -> Result<ArgoOperationResult, AppError> {
    Ok(ArgoOperationResult {
        accepted: true,
        transport: transport.into(),
        message: "Argo CD accepted operation".into(),
        operation: Some(operation),
    })
}
fn application_path(app: &ArgoApplicationRef, suffix: &str, refresh: Option<&str>) -> String {
    let mut url =
        reqwest::Url::parse("https://argo.invalid/api/v1/applications").expect("static URL");
    url.path_segments_mut().expect("static URL").push(&app.name);
    url.set_path(&format!("{}{}", url.path(), suffix));
    let mut query = url.query_pairs_mut();
    if let Some(v) = app.namespace.as_deref() {
        query.append_pair("appNamespace", v);
    }
    if let Some(v) = app.project.as_deref() {
        query.append_pair("project", v);
    }
    if let Some(v) = refresh {
        query.append_pair("refresh", v);
    }
    drop(query);
    format!(
        "{}{}",
        url.path(),
        url.query().map(|q| format!("?{q}")).unwrap_or_default()
    )
}
fn identity(resource: &crate::models::ArgoManagedResource) -> Value {
    json!({"group": resource.group, "kind": resource.kind, "namespace": resource.namespace, "name": resource.name})
}
fn connected_payload(request: &ArgoOperationRequest) -> Value {
    if matches!(request.action.as_str(), "sync" | "retry") {
        if let Some(payload) = &request.sync_payload {
            return payload.clone();
        }
    }
    match request.action.as_str() {
        "sync" | "retry" => {
            json!({"revision":request.revision,"resources":request.resources.iter().map(identity).collect::<Vec<_>>(),"prune":request.prune,"dryRun":request.dry_run,"syncOptions":{"items":if request.force == Some(true) { vec!["Force=true", "Replace=true"] } else { vec![] }}})
        }
        "rollback" => {
            json!({"name":request.application.name,"appNamespace":request.application.namespace,"project":request.application.project,"id":request.history_id,"prune":request.prune,"dryRun":request.dry_run})
        }
        "resourceAction" => {
            let parameters = request
                .resource_action_parameters
                .as_ref()
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
                .filter_map(|(name, value)| {
                    value
                        .as_str()
                        .map(|value| json!({"name":name,"value":value}))
                })
                .collect::<Vec<_>>();
            json!({"appNamespace":request.application.namespace,"project":request.application.project,"group":request.resources[0].group,"version":request.resources[0].version,"kind":request.resources[0].kind,"namespace":request.resources[0].namespace,"resourceName":request.resources[0].name,"action":request.resource_action,"resourceActionParameters":parameters})
        }
        _ => json!({}),
    }
}
fn kubernetes_sync_payload(request: &ArgoOperationRequest) -> Value {
    let http = request
        .sync_payload
        .clone()
        .unwrap_or_else(|| sync_payload(request));
    let mut value = http.as_object().cloned().unwrap_or_default();
    let options = value
        .remove("syncOptions")
        .and_then(|value| value.get("items").and_then(Value::as_array).cloned())
        .unwrap_or_default();
    if let Some(strategy) = value.remove("strategy") {
        value.insert("syncStrategy".into(), strategy);
    }
    value.insert("syncOptions".into(), Value::Array(options));
    Value::Object(value)
}
fn resource_action_path(request: &ArgoOperationRequest) -> String {
    let resource = &request.resources[0];
    let mut url = reqwest::Url::parse(&format!(
        "https://argo.invalid{}",
        application_path(&request.application, "/resource/actions", None)
    ))
    .expect("static URL");
    let mut q = url.query_pairs_mut();
    for (key, value) in [
        ("group", resource.group.as_deref()),
        ("version", resource.version.as_deref()),
        ("kind", resource.kind.as_deref()),
        ("namespace", resource.namespace.as_deref()),
        ("resourceName", resource.name.as_deref()),
    ] {
        if let Some(value) = value {
            q.append_pair(key, value);
        }
    }
    drop(q);
    format!("{}?{}", url.path(), url.query().unwrap_or_default())
}
async fn validate_resource_action(
    connection: &super::connected::ConnectedArgo,
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
            "argoOperationUnavailable",
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
            "argoOperationUnavailable",
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
            "argoOperationUnavailable",
        ));
    }
    Ok(())
}
async fn kubernetes_operation(
    request: ArgoOperationRequest,
) -> Result<ArgoOperationResult, AppError> {
    fallback_allowed(&request).await?;
    let context = request.cluster_context.as_deref().expect("validated");
    let client = client_for_context(context, request.kubeconfig_env_var.clone()).await?;
    let ar = find_api_resource(&client, "argoproj.io", "Application")
        .await?
        .ok_or_else(|| AppError::new("Application CRD not found", "cluster"))?;
    let api = Api::<DynamicObject>::namespaced_with(
        client,
        request.application.namespace.as_deref().expect("validated"),
        &ar,
    );
    let version = request.resource_version.clone().expect("validated");
    let patch = match request.action.as_str() {
        "refresh" | "hardRefresh" => {
            json!({"metadata":{"resourceVersion":version,"annotations":{"argocd.argoproj.io/refresh":if request.action == "hardRefresh" { "hard" } else { "normal" }}}})
        }
        "sync" | "retry" => {
            json!({"metadata":{"resourceVersion":version},"operation":{"sync":kubernetes_sync_payload(&request)}})
        }
        _ => unreachable!(),
    };
    api.patch(
        &request.application.name,
        &PatchParams::default(),
        &Patch::Merge(&patch),
    )
    .await?;
    Ok(ArgoOperationResult {
        accepted: true,
        transport: "kubernetes".into(),
        message: "Kubernetes-authorized fallback accepted; Argo RBAC not evaluated".into(),
        operation: None,
    })
}

async fn resolve_request(
    store: &super::ArgoConnectionStore,
    request: &ArgoOperationRequest,
) -> Result<ArgoOperationRequest, AppError> {
    if request.action != "retry" {
        let mut resolved = request.clone();
        if request.action == "sync" {
            resolved.sync_payload = Some(sync_payload(request));
        }
        return Ok(resolved);
    }
    let sync =
        if request.transport == "connected" {
            let connection = scoped_connection(
                store,
                request.connection_id.as_deref().ok_or_else(|| {
                    AppError::new("Argo CD connection required", "argoConnection")
                })?,
                request.cluster_context.as_deref().ok_or_else(|| {
                    AppError::new("clusterContext required", "argoOperationUnavailable")
                })?,
                request.application.workspace_id.as_deref(),
            )?;
            api_get(
                &connection,
                &application_path(&request.application, "", None),
            )
            .await?
            .pointer("/status/operationState/operation/sync")
            .cloned()
        } else {
            let context = request.cluster_context.as_deref().ok_or_else(|| {
                AppError::new("clusterContext required", "argoOperationUnavailable")
            })?;
            let client = client_for_context(context, request.kubeconfig_env_var.clone()).await?;
            let ar = find_api_resource(&client, "argoproj.io", "Application")
                .await?
                .ok_or_else(|| AppError::new("Application CRD not found", "cluster"))?;
            Api::<DynamicObject>::namespaced_with(
                client,
                request.application.namespace.as_deref().ok_or_else(|| {
                    AppError::new("application namespace required", "argoOperationUnavailable")
                })?,
                &ar,
            )
            .get(&request.application.name)
            .await?
            .data
            .pointer("/status/operationState/operation/sync")
            .cloned()
        }
        .ok_or_else(|| {
            AppError::new(
                "no recorded sync operation to retry",
                "argoOperationUnavailable",
            )
        })?;
    recorded_retry(request, sync)
}

fn recorded_retry(
    request: &ArgoOperationRequest,
    sync: Value,
) -> Result<ArgoOperationRequest, AppError> {
    let mut resolved = request.clone();
    resolved.revision = sync
        .get("revision")
        .and_then(Value::as_str)
        .map(str::to_owned);
    resolved.prune = sync.get("prune").and_then(Value::as_bool);
    resolved.dry_run = sync.get("dryRun").and_then(Value::as_bool);
    resolved.force = Some(
        sync.get("syncOptions")
            .and_then(|value| {
                value
                    .get("items")
                    .and_then(Value::as_array)
                    .or_else(|| value.as_array())
            })
            .is_some_and(|items| items.iter().any(|item| item.as_str() == Some("Force=true"))),
    );
    resolved.resources = sync
        .get("resources")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|item| crate::models::ArgoManagedResource {
            group: item.get("group").and_then(Value::as_str).map(str::to_owned),
            kind: item.get("kind").and_then(Value::as_str).map(str::to_owned),
            namespace: item
                .get("namespace")
                .and_then(Value::as_str)
                .map(str::to_owned),
            name: item.get("name").and_then(Value::as_str).map(str::to_owned),
            ..Default::default()
        })
        .collect();
    resolved.sync_payload = Some(recorded_sync_payload(sync)?);
    Ok(resolved)
}

fn sync_payload(request: &ArgoOperationRequest) -> Value {
    json!({"revision":request.revision,"resources":request.resources.iter().map(identity).collect::<Vec<_>>(),"prune":request.prune,"dryRun":request.dry_run,"syncOptions":{"items":if request.force == Some(true) { vec!["Force=true", "Replace=true"] } else { vec![] }}})
}

fn recorded_sync_payload(sync: Value) -> Result<Value, AppError> {
    let mut payload = sync.as_object().cloned().ok_or_else(|| {
        AppError::new(
            "recorded sync operation is invalid",
            "argoOperationUnavailable",
        )
    })?;
    if payload.keys().any(|key| {
        !matches!(
            key.as_str(),
            "revision"
                | "prune"
                | "dryRun"
                | "syncOptions"
                | "resources"
                | "syncStrategy"
                | "strategy"
        )
    }) {
        return Err(AppError::new(
            "recorded sync operation has unsupported fields",
            "argoOperationUnavailable",
        ));
    }
    let resources = match payload.remove("resources") {
        None => Vec::new(),
        Some(Value::Array(resources)) => resources,
        Some(_) => {
            return Err(AppError::new(
                "recorded sync resources are invalid",
                "argoOperationUnavailable",
            ))
        }
    }
    .into_iter()
    .map(|resource| {
        let resource = resource.as_object().ok_or_else(|| {
            AppError::new(
                "recorded sync resource is invalid",
                "argoOperationUnavailable",
            )
        })?;
        if resource.keys().any(|key| {
            !matches!(
                key.as_str(),
                "group" | "version" | "kind" | "namespace" | "name"
            )
        }) {
            return Err(AppError::new(
                "recorded sync resource has unsupported fields",
                "argoOperationUnavailable",
            ));
        }
        if resource.values().any(|value| !value.is_string()) {
            return Err(AppError::new(
                "recorded sync resource fields are invalid",
                "argoOperationUnavailable",
            ));
        }
        let mut selector = serde_json::Map::new();
        for key in ["group", "kind", "namespace", "name"] {
            if let Some(value) = resource.get(key) {
                selector.insert(key.into(), value.clone());
            }
        }
        Ok(Value::Object(selector))
    })
    .collect::<Result<Vec<_>, AppError>>()?;
    let options = match payload.remove("syncOptions") {
        None => Vec::new(),
        Some(Value::Array(options)) => options,
        Some(Value::Object(mut options)) if options.len() == 1 && options.contains_key("items") => {
            options
                .remove("items")
                .and_then(|items| items.as_array().cloned())
                .ok_or_else(|| {
                    AppError::new(
                        "recorded sync options are invalid",
                        "argoOperationUnavailable",
                    )
                })?
        }
        Some(_) => {
            return Err(AppError::new(
                "recorded sync options are invalid",
                "argoOperationUnavailable",
            ))
        }
    };
    if options.iter().any(|option| !option.is_string()) {
        return Err(AppError::new(
            "recorded sync option is invalid",
            "argoOperationUnavailable",
        ));
    }
    let strategy = payload
        .remove("syncStrategy")
        .or_else(|| payload.remove("strategy"));
    payload.insert("resources".into(), Value::Array(resources));
    payload.insert("syncOptions".into(), json!({"items": options}));
    if let Some(strategy) = strategy {
        payload.insert("strategy".into(), strategy);
    }
    Ok(Value::Object(payload))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_payload_matches_argocd_request_contract() {
        let request = ArgoOperationRequest {
            action: "sync".into(),
            revision: Some("main".into()),
            force: Some(true),
            resources: vec![crate::models::ArgoManagedResource {
                group: Some("apps".into()),
                version: Some("v1".into()),
                kind: Some("Deployment".into()),
                namespace: Some("default".into()),
                name: Some("web".into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(
            connected_payload(&request),
            json!({"revision":"main","resources":[{"group":"apps","kind":"Deployment","namespace":"default","name":"web"}],"prune":null,"dryRun":null,"syncOptions":{"items":["Force=true","Replace=true"]}})
        );
    }

    #[test]
    fn recorded_retry_is_replayed_as_sync_request() {
        let retry = recorded_retry(&ArgoOperationRequest { action: "retry".into(), ..Default::default() }, json!({"revision":"v2","prune":true,"syncOptions":{"items":["Force=true"]},"resources":[{"group":"apps","version":"v1","kind":"Deployment","namespace":"default","name":"web"}]})).expect("recorded sync");
        assert_eq!(
            connected_payload(&retry),
            json!({"revision":"v2","resources":[{"group":"apps","kind":"Deployment","namespace":"default","name":"web"}],"prune":true,"syncOptions":{"items":["Force=true"]}})
        );
    }

    #[test]
    fn resource_action_contract_is_flat_and_server_scoped() {
        let request = ArgoOperationRequest {
            action: "resourceAction".into(),
            application: ArgoApplicationRef {
                namespace: Some("argocd".into()),
                project: Some("default".into()),
                ..Default::default()
            },
            resource_action: Some("restart".into()),
            resources: vec![crate::models::ArgoManagedResource {
                group: Some("apps".into()),
                version: Some("v1".into()),
                kind: Some("Deployment".into()),
                namespace: Some("default".into()),
                name: Some("web".into()),
                ..Default::default()
            }],
            resource_action_parameters: Some(json!({"replicas":"3","mode":"safe"})),
            ..Default::default()
        };
        assert_eq!(
            connected_payload(&request),
            json!({"appNamespace":"argocd","project":"default","group":"apps","version":"v1","kind":"Deployment","namespace":"default","resourceName":"web","action":"restart","resourceActionParameters":[{"name":"mode","value":"safe"},{"name":"replicas","value":"3"}]})
        );
    }

    #[test]
    fn fallback_sync_uses_crd_option_array() {
        let request = ArgoOperationRequest {
            action: "sync".into(),
            force: Some(true),
            ..Default::default()
        };
        assert_eq!(
            kubernetes_sync_payload(&request)["syncOptions"],
            json!(["Force=true", "Replace=true"])
        );
        assert_eq!(
            connected_payload(&request)["syncOptions"],
            json!({"items":["Force=true", "Replace=true"]})
        );
    }

    #[test]
    fn recorded_sync_allowlists_and_maps_strategy() {
        assert_eq!(
            recorded_sync_payload(json!({"revision":"v1","syncStrategy":{"apply":{"force":true}}}))
                .unwrap(),
            json!({"revision":"v1","resources":[],"syncOptions":{"items":[]},"strategy":{"apply":{"force":true}}})
        );
        assert!(recorded_sync_payload(json!({"source":{"repoURL":"secret"}})).is_err());
        assert!(recorded_sync_payload(json!({"autoHealAttemptsCount":1})).is_err());
    }

    #[test]
    fn fallback_retry_keeps_recorded_crd_sync_strategy() {
        let retry = recorded_retry(
            &ArgoOperationRequest {
                action: "retry".into(),
                transport: "kubernetes".into(),
                ..Default::default()
            },
            json!({"syncStrategy":{"apply":{"force":true}}}),
        )
        .unwrap();
        assert_eq!(
            kubernetes_sync_payload(&retry)["syncStrategy"],
            json!({"apply":{"force":true}})
        );
    }

    #[test]
    fn recorded_sync_rejects_malformed_resources_and_options() {
        assert!(recorded_sync_payload(json!({"resources":{}})).is_err());
        assert!(recorded_sync_payload(json!({"resources":["Deployment"]})).is_err());
        assert!(recorded_sync_payload(json!({"resources":[{"name":4}]})).is_err());
        assert!(recorded_sync_payload(json!({"syncOptions":{}})).is_err());
        assert!(recorded_sync_payload(json!({"syncOptions":{"items":"Force=true"}})).is_err());
        assert!(recorded_sync_payload(json!({"syncOptions":[4]})).is_err());
    }

    #[test]
    fn rollback_payload_is_bound_to_application_and_flags() {
        let request = ArgoOperationRequest {
            action: "rollback".into(),
            history_id: Some(4),
            prune: Some(true),
            dry_run: Some(false),
            application: ArgoApplicationRef {
                name: "app".into(),
                namespace: Some("argo".into()),
                project: Some("default".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(
            connected_payload(&request),
            json!({"name":"app","appNamespace":"argo","project":"default","id":4,"prune":true,"dryRun":false})
        );
    }
}
