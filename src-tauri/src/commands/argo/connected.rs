use super::scope::scoped_connection;
pub(crate) use super::transport::{api_delete, api_get, api_post, redact_secret_fields};
use super::transport::{canonical_url, http_client, response_json, safe_http_error, url};
use crate::commands::{
    gitops_crd::{client_for_context, find_api_resource, get_crd_object},
    BackendCancellationRegistry,
};
use crate::models::{
    AppError, ArgoApplicationHistory, ArgoApplicationInspector, ArgoApplicationRef,
    ArgoConnectionProfile, ArgoConnectionStatus, ArgoManagedResource, ArgoResourceComparison,
    ArgoServerCapability,
};
use k8s_openapi::api::core::v1::Service;
use kube::{
    api::{Api, ListParams},
    core::DynamicObject,
};
use reqwest::Client as HttpClient;
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Mutex};
use uuid::Uuid;

const ARGO_SERVICE_NAMES: [&str; 2] = ["argocd-server", "argo-cd-argocd-server"];

#[derive(Default)]
pub struct ArgoConnectionStore {
    pub(crate) connections: Mutex<HashMap<String, ConnectedArgo>>,
    pub(crate) sessions: super::session::SessionStore,
}

#[derive(Clone)]
pub(crate) struct ConnectedArgo {
    pub(crate) profile: ArgoConnectionProfile,
    pub(crate) token: String,
    pub(crate) username: Option<String>,
    pub(crate) client: HttpClient,
    pub(crate) generation: String,
}

fn credential_key(profile: &ArgoConnectionProfile) -> String {
    format!(
        "{}:{}:{}:{}",
        profile.id,
        profile.url,
        profile.cluster_context.clone().unwrap_or_default(),
        profile.workspace_id.clone().unwrap_or_default()
    )
}
fn load_credential(profile: &ArgoConnectionProfile) -> Result<Option<String>, AppError> {
    let entry =
        keyring::Entry::new("KubeCove Argo CD", &credential_key(profile)).map_err(|_| {
            AppError::new(
                "native credential storage unavailable",
                "credentialUnavailable",
            )
        })?;
    match entry.get_password() {
        Ok(token) => Ok((!token.is_empty()).then_some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(AppError::new(
            "native credential storage unavailable",
            "credentialUnavailable",
        )),
    }
}
fn save_credential(profile: &ArgoConnectionProfile, token: &str) -> Result<(), AppError> {
    let entry =
        keyring::Entry::new("KubeCove Argo CD", &credential_key(profile)).map_err(|_| {
            AppError::new(
                "native credential storage unavailable",
                "credentialUnavailable",
            )
        })?;
    entry.set_password(token).map_err(|_| {
        AppError::new(
            "native credential storage unavailable",
            "credentialUnavailable",
        )
    })
}
fn delete_credential(profile: &ArgoConnectionProfile) -> Result<(), AppError> {
    let entry =
        keyring::Entry::new("KubeCove Argo CD", &credential_key(profile)).map_err(|_| {
            AppError::new(
                "native credential storage unavailable",
                "credentialUnavailable",
            )
        })?;
    credential_deleted(entry.delete_credential())
}
fn credential_deleted(result: Result<(), keyring::Error>) -> Result<(), AppError> {
    match result {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(AppError::new(
            "native credential storage unavailable",
            "credentialUnavailable",
        )),
    }
}

#[tauri::command]
pub async fn discover_argo_servers(
    cluster_context: String,
    kubeconfig_env_var: Option<String>,
) -> Result<Vec<ArgoServerCapability>, AppError> {
    let client = client_for_context(&cluster_context, kubeconfig_env_var).await?;
    let services: Api<Service> = Api::all(client);
    let list = services
        .list(&ListParams::default())
        .await
        .map_err(AppError::from)?;
    Ok(list
        .items
        .into_iter()
        .filter_map(|service| {
            let name = service.metadata.name?;
            let likely =
                ARGO_SERVICE_NAMES.contains(&name.as_str()) || name.contains("argocd-server");
            likely.then(|| ArgoServerCapability {
                id: format!(
                    "service:{}:{}",
                    service.metadata.namespace.clone().unwrap_or_default(),
                    name
                ),
                name,
                namespace: service.metadata.namespace,
                url: None,
                transport: "serviceTunnel".into(),
                unavailable_reason: Some(
                    "service tunnel is not available in this build; use manual URL".into(),
                ),
            })
        })
        .collect())
}

#[tauri::command]
pub async fn connect_argo_server(
    store: tauri::State<'_, ArgoConnectionStore>,
    id: String,
    server_url: String,
    token: Option<String>,
    username: Option<String>,
    password: Option<String>,
    insecure_tls: bool,
    custom_ca_pem: Option<Vec<u8>>,
    remember_credential: bool,
    cluster_context: Option<String>,
    workspace_id: Option<String>,
) -> Result<ArgoConnectionStatus, AppError> {
    let profile = ArgoConnectionProfile {
        id,
        url: canonical_url(&server_url)?
            .to_string()
            .trim_end_matches('/')
            .to_owned(),
        cluster_context,
        workspace_id,
        transport: "connected".into(),
        remember_credential,
    };
    let client = http_client(insecure_tls, custom_ca_pem)?;
    let submitted_token = token.filter(|value| !value.is_empty());
    let stored_token = if submitted_token.is_none() && remember_credential {
        load_credential(&profile)?
    } else {
        None
    };
    let token = if let Some(token) = submitted_token.or(stored_token) {
        token
    } else {
        let user = username.ok_or_else(|| {
            AppError::new(
                "token or local login credentials required",
                "argoConnection",
            )
        })?;
        let password = password.ok_or_else(|| {
            AppError::new(
                "token or local login credentials required",
                "argoConnection",
            )
        })?;
        let response = client
            .post(url(&profile.url, "/api/v1/session")?)
            .json(&json!({"username": user, "password": password}))
            .send()
            .await
            .map_err(safe_http_error)?;
        if !response.status().is_success() {
            return Err(AppError::new(
                format!("Argo CD login failed ({})", response.status()),
                "argoConnection",
            ));
        }
        response_json(response)
            .await?
            .get("token")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| AppError::new("Argo CD login returned no token", "argoConnection"))?
    };
    let response = client
        .get(url(&profile.url, "/api/v1/session/userinfo")?)
        .bearer_auth(&token)
        .send()
        .await
        .map_err(safe_http_error)?;
    if !response.status().is_success() {
        return Err(AppError::new(
            format!("Argo CD session validation failed ({})", response.status()),
            "argoConnection",
        ));
    }
    let userinfo = response_json(response).await?;
    let user = userinfo
        .get("username")
        .or_else(|| userinfo.get("sub"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    if remember_credential {
        save_credential(&profile, &token)?;
    } else {
        // A stale remembered credential must not revive on a later reconnect.
        delete_credential(&profile)?;
    }
    store
        .connections
        .lock()
        .map_err(|_| AppError::new("Argo CD connection state unavailable", "argoConnection"))?
        .insert(
            profile.id.clone(),
            ConnectedArgo {
                profile: profile.clone(),
                token,
                username: user.clone(),
                client,
                generation: Uuid::new_v4().to_string(),
            },
        );
    Ok(ArgoConnectionStatus {
        profile: Some(profile),
        connected: true,
        username: user,
        unavailable_reason: None,
    })
}

#[tauri::command]
pub fn get_argo_connection_status(
    store: tauri::State<'_, ArgoConnectionStore>,
    id: String,
) -> Result<ArgoConnectionStatus, AppError> {
    let connection = store
        .connections
        .lock()
        .map_err(|_| AppError::new("Argo CD connection state unavailable", "argoConnection"))?
        .get(&id)
        .cloned();
    Ok(match connection {
        Some(connection) => ArgoConnectionStatus {
            profile: Some(connection.profile),
            connected: true,
            username: connection.username,
            unavailable_reason: None,
        },
        None => ArgoConnectionStatus {
            profile: None,
            connected: false,
            username: None,
            unavailable_reason: None,
        },
    })
}

#[tauri::command]
pub fn disconnect_argo_server(
    store: tauri::State<'_, ArgoConnectionStore>,
    id: String,
) -> Result<(), AppError> {
    store
        .connections
        .lock()
        .map_err(|_| AppError::new("Argo CD connection state unavailable", "argoConnection"))?
        .remove(&id);
    Ok(())
}

#[tauri::command]
pub fn forget_argo_credential(profile: ArgoConnectionProfile) -> Result<(), AppError> {
    delete_credential(&profile)
}

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
            Value::String(text) => serde_json::from_str(text)
                .ok()
                .or_else(|| Some(Value::String(text.clone()))),
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
        .ok_or_else(|| AppError::new("invalid Application data", "serialization"))?;
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

fn kubernetes_comparison(resource: ArgoManagedResource) -> ArgoResourceComparison {
    ArgoResourceComparison {
        resource,
        exact: Some(false),
        provenance: Some("kubernetes-status-no-diff".into()),
        ..Default::default()
    }
}

fn connected_comparison(value: &Value) -> ArgoResourceComparison {
    ArgoResourceComparison {
        resource: managed_resource(value),
        target_state: state(value.get("targetState"), true),
        live_state: state(value.get("liveState"), true),
        normalized_live_state: state(value.get("normalizedLiveState"), true),
        predicted_live_state: state(value.get("predictedLiveState"), true),
        modified: value.get("modified").and_then(Value::as_bool),
        exact: Some(true),
        provenance: Some("argocd-managed-resource".into()),
        ..Default::default()
    }
}

fn connected_inspector(
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

pub(crate) async fn kubernetes_application(
    cluster_context: &str,
    namespace: Option<&str>,
    name: &str,
    kubeconfig_env_var: Option<String>,
) -> Result<DynamicObject, AppError> {
    let client = client_for_context(cluster_context, kubeconfig_env_var).await?;
    let ar = find_api_resource(&client, "argoproj.io", "Application")
        .await?
        .ok_or_else(|| AppError::new("Application CRD not found", "cluster"))?;
    get_crd_object(client, &ar, name, namespace).await
}

async fn kubernetes_inspector(
    cluster_context: &str,
    application: &ArgoApplicationRef,
    kubeconfig_env_var: Option<String>,
) -> Result<ArgoApplicationInspector, AppError> {
    let workspace_id = application.workspace_id.clone();
    let mut object = kubernetes_application(
        cluster_context,
        application.namespace.as_deref(),
        &application.name,
        kubeconfig_env_var,
    )
    .await?;
    redact_secret_fields(&mut object.data);
    let mut inspector = inspector_from_application(&object)?;
    inspector.application.context = Some(cluster_context.to_string());
    inspector.application.workspace_id = workspace_id;
    Ok(inspector)
}

async fn connected_inspector_read(
    store: &ArgoConnectionStore,
    cluster_context: &str,
    connection_id: Option<&str>,
    application: ArgoApplicationRef,
) -> Result<ArgoApplicationInspector, AppError> {
    let connection = scoped_connection(
        store,
        connection_id
            .ok_or_else(|| AppError::new("Argo CD connection required", "argoConnection"))?,
        cluster_context,
        application.workspace_id.as_deref(),
    )?;
    let namespace = application.namespace.clone().unwrap_or_default();
    let response = api_get(
        &connection,
        &format!(
            "/api/v1/applications/{}?appNamespace={}&project={}",
            application.name,
            namespace,
            application.project.clone().unwrap_or_default()
        ),
    )
    .await?;
    let managed = api_get(
        &connection,
        &format!(
            "/api/v1/applications/{}/managed-resources?appNamespace={}",
            application.name, namespace
        ),
    )
    .await?;
    Ok(connected_inspector(application, response, managed))
}

fn inspection_failure(error: &AppError) -> crate::models::ArgoInspectionFailure {
    let kind = match error.kind.as_str() {
        "argoApi"
        | "argoConnection"
        | "cancelled"
        | "cluster"
        | "forbidden"
        | "kubeconfig"
        | "network"
        | "notFound"
        | "providerDiscoveryUnavailable"
        | "serialization" => error.kind.clone(),
        _ => "inspection".into(),
    };
    let message = match kind.as_str() {
        "forbidden" => "access denied",
        "notFound" => "Application not found",
        "cancelled" => "request cancelled",
        "kubeconfig" => "cluster configuration unavailable",
        "network" => "network unavailable",
        "argoConnection" => "connection unavailable",
        "argoApi" => "Argo CD API request failed",
        "providerDiscoveryUnavailable" => "Application API unavailable",
        "serialization" => "invalid response",
        _ => "read failed",
    };
    crate::models::ArgoInspectionFailure {
        kind,
        message: message.into(),
    }
}

fn with_connected_fallback(
    mut inspector: ArgoApplicationInspector,
    error: &AppError,
) -> ArgoApplicationInspector {
    inspector.connected_fallback = Some(crate::models::ArgoConnectedFallback {
        transport: "connected".into(),
        failure: inspection_failure(error),
    });
    inspector
}

fn inspection_error(connected: &AppError, kubernetes: &AppError) -> AppError {
    let connected = inspection_failure(connected);
    let kubernetes = inspection_failure(kubernetes);
    AppError::new(
        format!(
            "Connected inspection failed ({}) {}; Kubernetes inspection failed ({}) {}",
            connected.kind, connected.message, kubernetes.kind, kubernetes.message
        ),
        "argoInspection",
    )
}

async fn application_inspector(
    store: &ArgoConnectionStore,
    cluster_context: String,
    kubeconfig_env_var: Option<String>,
    connection_id: Option<String>,
    transport: String,
    application: ArgoApplicationRef,
) -> Result<ArgoApplicationInspector, AppError> {
    if transport == "kubernetes" {
        return kubernetes_inspector(&cluster_context, &application, kubeconfig_env_var).await;
    }
    if transport != "connected" {
        return Err(AppError::new("invalid Argo CD transport", "argoConnection"));
    }
    match connected_inspector_read(
        store,
        &cluster_context,
        connection_id.as_deref(),
        application.clone(),
    )
    .await
    {
        Ok(inspector) => Ok(inspector),
        Err(connected_error) => {
            match kubernetes_inspector(&cluster_context, &application, kubeconfig_env_var).await {
                Ok(inspector) => Ok(with_connected_fallback(inspector, &connected_error)),
                Err(kubernetes_error) => Err(inspection_error(&connected_error, &kubernetes_error)),
            }
        }
    }
}

#[tauri::command]
pub async fn get_argo_application_inspector(
    store: tauri::State<'_, ArgoConnectionStore>,
    cancellations: tauri::State<'_, BackendCancellationRegistry>,
    cluster_context: String,
    kubeconfig_env_var: Option<String>,
    connection_id: Option<String>,
    transport: String,
    application: ArgoApplicationRef,
    _redact_secrets: Option<bool>,
    request_id: Option<String>,
    cancel_scope: Option<String>,
) -> Result<ArgoApplicationInspector, AppError> {
    cancellations
        .execute(
            cancel_scope,
            request_id,
            application_inspector(
                &store,
                cluster_context,
                kubeconfig_env_var,
                connection_id,
                transport,
                application,
            ),
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_preserves_configured_base_path() {
        assert_eq!(
            url("https://argo.example/argo-cd", "/api/v1/applications").unwrap(),
            "https://argo.example/argo-cd/api/v1/applications"
        );
    }

    #[test]
    fn missing_credential_is_already_deleted() {
        assert!(credential_deleted(Err(keyring::Error::NoEntry)).is_ok());
    }

    #[test]
    fn connected_inspection_eagerly_compares_and_redacts_nested_secrets() {
        let inspector = connected_inspector(
            ArgoApplicationRef {
                name: "demo".into(),
                namespace: Some("argocd".into()),
                ..Default::default()
            },
            serde_json::json!({
                "status": {
                    "conditions": [{"nested": {"kind": "Secret", "data": {"key": "value"}}}]
                }
            }),
            serde_json::json!({
                "items": [{
                    "group": "", "version": "v1", "kind": "Secret", "namespace": "default", "name": "db",
                    "targetState": "{\"kind\":\"Secret\",\"data\":{\"password\":\"plaintext\"}}",
                    "liveState": {"kind": "Secret", "stringData": {"password": "plaintext"}},
                    "modified": true
                }]
            }),
        );
        let value = serde_json::to_value(&inspector).unwrap();

        assert_eq!(value["transport"], "connected");
        assert_eq!(value["provenance"], "argocd-api");
        assert_eq!(value["comparisons"][0]["exact"], true);
        assert_eq!(
            value["comparisons"][0]["availableActions"],
            serde_json::json!([])
        );
        assert_eq!(
            value["comparisons"][0]["targetState"]["data"]["password"],
            "[REDACTED]"
        );
        assert_eq!(
            value["conditions"][0]["nested"]["data"]["key"],
            "[REDACTED]"
        );
    }

    #[test]
    fn kubernetes_comparisons_do_not_imply_desired_state() {
        let comparison = kubernetes_comparison(ArgoManagedResource {
            kind: Some("Deployment".into()),
            name: Some("api".into()),
            ..Default::default()
        });

        assert_eq!(comparison.exact, Some(false));
        assert_eq!(
            comparison.provenance.as_deref(),
            Some("kubernetes-status-no-diff")
        );
        assert!(comparison.target_state.is_none());
        assert!(comparison.live_state.is_none());
        assert!(comparison.available_actions.is_empty());
    }

    #[test]
    fn fallback_keeps_complete_kubernetes_provenance_visible() {
        let inspector = ArgoApplicationInspector {
            transport: "kubernetes".into(),
            provenance: "kubernetes-status-no-diff".into(),
            resources: vec![ArgoManagedResource {
                kind: Some("Deployment".into()),
                name: Some("api".into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        let fallback = with_connected_fallback(
            inspector,
            &AppError::new("token=plaintext", "argoConnection"),
        );

        assert_eq!(fallback.transport, "kubernetes");
        assert_eq!(fallback.provenance, "kubernetes-status-no-diff");
        assert_eq!(fallback.resources.len(), 1);
        let connected = fallback.connected_fallback.expect("fallback metadata");
        assert_eq!(connected.transport, "connected");
        assert_eq!(connected.failure.kind, "argoConnection");
        assert_eq!(connected.failure.message, "connection unavailable");
    }

    #[test]
    fn fallback_and_combined_errors_are_sanitized() {
        let connected = AppError::new(
            "token=plaintext https://user:password@argo.example/api kubeconfig /tmp/config",
            "argoConnection",
        );
        let kubernetes = AppError::new("Secret data: plaintext", "cluster");
        let fallback = inspection_failure(&connected);
        let error = inspection_error(&connected, &kubernetes);

        assert_eq!(fallback.kind, "argoConnection");
        assert_eq!(fallback.message, "connection unavailable");
        assert_eq!(error.kind, "argoInspection");
        assert!(error.message.contains("argoConnection"));
        assert!(error.message.contains("cluster"));
        assert!(!error.message.contains("plaintext"));
        assert!(!error.message.contains("https://"));
    }
}
