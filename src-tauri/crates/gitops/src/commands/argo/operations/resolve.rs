use super::super::applications::application_api_resource;
use super::super::connected::{api_get, ConnectedArgo};
use super::super::scope::scoped_connection;
use super::super::ArgoConnectionStore;
use super::payload::{application_path, sync_payload};
use crate::commands::ClusterAccess;
use crate::models::AppErrorKind;
use crate::models::{AppError, ArgoOperationRequest};
use kube::{api::Api, core::DynamicObject};
use serde_json::{json, Value};
use std::sync::Arc;

pub(super) fn required_cluster_context(request: &ArgoOperationRequest) -> Result<&str, AppError> {
    request.cluster_context.as_deref().ok_or_else(|| {
        AppError::new(
            "clusterContext required",
            AppErrorKind::ArgoOperationUnavailable,
        )
    })
}

/// The live connection an operation names, checked against the request's cluster and the
/// given workspace.
pub(super) fn request_connection(
    store: &ArgoConnectionStore,
    access: &ClusterAccess,
    request: &ArgoOperationRequest,
    workspace_id: Option<&str>,
) -> Result<Arc<ConnectedArgo>, AppError> {
    scoped_connection(
        store,
        request.connection_id.as_deref().ok_or_else(|| {
            AppError::new("Argo CD connection required", AppErrorKind::ArgoConnection)
        })?,
        required_cluster_context(request)?,
        workspace_id,
        access.source_key(),
    )
}

pub(super) async fn resolve_request(
    store: &ArgoConnectionStore,
    access: &ClusterAccess,
    request: &ArgoOperationRequest,
) -> Result<(ArgoOperationRequest, Option<String>, Option<String>), AppError> {
    let (mut resolved, generation, instance_id) = if request.transport == "connected" {
        let connection = request_connection(
            store,
            access,
            request,
            request.application.workspace_id.as_deref(),
        )?;
        let value = api_get(
            &connection,
            &application_path(&request.application, &[], None),
        )
        .await?;
        let metadata = value.get("metadata").unwrap_or(&value);
        let mut application = request.application.clone();
        if let Some(name) = metadata.get("name").and_then(Value::as_str) {
            name.clone_into(&mut application.name);
        }
        application.namespace = metadata
            .get("namespace")
            .and_then(Value::as_str)
            .map(str::to_owned);
        application.project = value
            .pointer("/spec/project")
            .and_then(Value::as_str)
            .map(str::to_owned);
        application.uid = metadata
            .get("uid")
            .and_then(Value::as_str)
            .map(str::to_owned);
        application.resource_version = metadata
            .get("resourceVersion")
            .and_then(Value::as_str)
            .map(str::to_owned);
        application
            .workspace_id
            .clone_from(&connection.profile.workspace_id);
        let mut resolved = request.clone();
        resolved.application = application;
        resolved
            .cluster_context
            .clone_from(&connection.profile.cluster_context);
        (
            resolved,
            Some(connection.generation.clone()),
            Some(connection.instance_id.clone()),
        )
    } else {
        (request.clone(), None, None)
    };
    if resolved.action != "retry" {
        if resolved.action == "sync" {
            resolved.sync_payload = Some(sync_payload(&resolved));
        }
        return Ok((resolved, generation, instance_id));
    }
    let sync = if resolved.transport == "connected" {
        let connection = request_connection(
            store,
            access,
            &resolved,
            resolved.application.workspace_id.as_deref(),
        )?;
        api_get(
            &connection,
            &application_path(&resolved.application, &[], None),
        )
        .await?
        .pointer("/status/operationState/operation/sync")
        .cloned()
    } else {
        let context = required_cluster_context(request)?;
        let client = access.preview_client(context).await?;
        let ar = application_api_resource(&client).await?;
        Api::<DynamicObject>::namespaced_with(
            client,
            request.application.namespace.as_deref().ok_or_else(|| {
                AppError::new(
                    "application namespace required",
                    AppErrorKind::ArgoOperationUnavailable,
                )
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
            AppErrorKind::ArgoOperationUnavailable,
        )
    })?;
    Ok((recorded_retry(&resolved, sync)?, generation, instance_id))
}

pub(super) fn recorded_retry(
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

pub(super) fn recorded_sync_payload(sync: Value) -> Result<Value, AppError> {
    let mut payload = sync.as_object().cloned().ok_or_else(|| {
        AppError::new(
            "recorded sync operation is invalid",
            AppErrorKind::ArgoOperationUnavailable,
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
            AppErrorKind::ArgoOperationUnavailable,
        ));
    }
    let resources = match payload.remove("resources") {
        None => Vec::new(),
        Some(Value::Array(resources)) => resources,
        Some(_) => {
            return Err(AppError::new(
                "recorded sync resources are invalid",
                AppErrorKind::ArgoOperationUnavailable,
            ))
        }
    }
    .into_iter()
    .map(|resource| {
        let resource = resource.as_object().ok_or_else(|| {
            AppError::new(
                "recorded sync resource is invalid",
                AppErrorKind::ArgoOperationUnavailable,
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
                AppErrorKind::ArgoOperationUnavailable,
            ));
        }
        if resource.values().any(|value| !value.is_string()) {
            return Err(AppError::new(
                "recorded sync resource fields are invalid",
                AppErrorKind::ArgoOperationUnavailable,
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
                        AppErrorKind::ArgoOperationUnavailable,
                    )
                })?
        }
        Some(_) => {
            return Err(AppError::new(
                "recorded sync options are invalid",
                AppErrorKind::ArgoOperationUnavailable,
            ))
        }
    };
    if options.iter().any(|option| !option.is_string()) {
        return Err(AppError::new(
            "recorded sync option is invalid",
            AppErrorKind::ArgoOperationUnavailable,
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
