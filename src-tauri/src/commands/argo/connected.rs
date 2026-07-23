use super::scope::scoped_connection;
use crate::commands::gitops_crd::{client_for_context, find_api_resource, get_crd_object};
use crate::models::{
    AppError, ArgoApplicationHistory, ArgoApplicationInspector, ArgoApplicationRef,
    ArgoConnectionProfile, ArgoConnectionStatus, ArgoManagedResource, ArgoServerCapability,
};
use futures_util::StreamExt;
use k8s_openapi::api::core::v1::Service;
use kube::{api::Api, core::DynamicObject};
use reqwest::{Certificate, Client as HttpClient};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};

const ARGO_SERVICE_NAMES: [&str; 2] = ["argocd-server", "argo-cd-argocd-server"];

#[derive(Default)]
pub struct ArgoConnectionStore {
    pub(crate) connections: Mutex<HashMap<String, ConnectedArgo>>,
    preflights: Mutex<HashMap<String, (String, Instant)>>,
}
static PREFLIGHT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub(crate) struct ConnectedArgo {
    pub(crate) profile: ArgoConnectionProfile,
    pub(crate) token: String,
    pub(crate) username: Option<String>,
    pub(crate) client: HttpClient,
}

pub(crate) fn issue_preflight(
    store: &ArgoConnectionStore,
    request: &crate::models::ArgoOperationRequest,
) -> Result<String, AppError> {
    let token = format!(
        "{}-{}",
        Instant::now().elapsed().as_nanos(),
        PREFLIGHT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let request = serde_json::to_string(request)
        .map_err(|_| AppError::new("invalid operation request", "argoOperationUnavailable"))?;
    store
        .preflights
        .lock()
        .map_err(|_| {
            AppError::new(
                "Argo CD operation state unavailable",
                "argoOperationUnavailable",
            )
        })?
        .insert(
            token.clone(),
            (request, Instant::now() + Duration::from_secs(60)),
        );
    Ok(token)
}
pub(crate) fn consume_preflight(
    store: &ArgoConnectionStore,
    request: &crate::models::ArgoOperationRequest,
) -> Result<(), AppError> {
    let token = request
        .preflight_token
        .as_deref()
        .ok_or_else(|| AppError::new("operation preflight required", "argoOperationUnavailable"))?;
    let (expected, expires) = store
        .preflights
        .lock()
        .map_err(|_| {
            AppError::new(
                "Argo CD operation state unavailable",
                "argoOperationUnavailable",
            )
        })?
        .remove(token)
        .ok_or_else(|| {
            AppError::new(
                "operation preflight expired or already used",
                "argoOperationUnavailable",
            )
        })?;
    let mut actual = request.clone();
    actual.preflight_token = None;
    let actual = serde_json::to_string(&actual)
        .map_err(|_| AppError::new("invalid operation request", "argoOperationUnavailable"))?;
    if expires < Instant::now() || expected != actual {
        return Err(AppError::new(
            "operation changed since preflight",
            "argoOperationUnavailable",
        ));
    }
    Ok(())
}
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;

pub(crate) async fn api_get(connection: &ConnectedArgo, path: &str) -> Result<Value, AppError> {
    let response = connection
        .client
        .get(url(&connection.profile.url, path)?)
        .bearer_auth(&connection.token)
        .send()
        .await
        .map_err(safe_http_error)?;
    if !response.status().is_success() {
        return Err(api_status_error(response).await);
    }
    response_json(response).await
}
pub(crate) async fn api_post(
    connection: &ConnectedArgo,
    path: &str,
    payload: Value,
) -> Result<Value, AppError> {
    let response = connection
        .client
        .post(url(&connection.profile.url, path)?)
        .bearer_auth(&connection.token)
        .json(&payload)
        .send()
        .await
        .map_err(safe_http_error)?;
    if !response.status().is_success() {
        return Err(api_status_error(response).await);
    }
    response_json(response).await
}
pub(crate) async fn api_delete(connection: &ConnectedArgo, path: &str) -> Result<Value, AppError> {
    let response = connection
        .client
        .delete(url(&connection.profile.url, path)?)
        .bearer_auth(&connection.token)
        .send()
        .await
        .map_err(safe_http_error)?;
    if !response.status().is_success() {
        return Err(api_status_error(response).await);
    }
    response_json(response).await
}
async fn response_json(response: reqwest::Response) -> Result<Value, AppError> {
    let bytes = capped_response_bytes(response).await?;
    serde_json::from_slice(&bytes).map_err(|_| AppError::new("invalid Argo CD response", "argoApi"))
}
async fn api_status_error(response: reqwest::Response) -> AppError {
    let status = response.status();
    let body = capped_response_bytes(response).await.unwrap_or_default();
    let body = &body[..body.len().min(4096)];
    let detail = serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("message")
                .or_else(|| value.get("error"))
                .and_then(Value::as_str)
                .map(safe_message)
        });
    AppError::new(
        detail.map_or_else(
            || format!("Argo CD API request failed ({status})"),
            |message| format!("Argo CD API request failed ({status}): {message}"),
        ),
        "argoApi",
    )
}
async fn capped_response_bytes(response: reqwest::Response) -> Result<Vec<u8>, AppError> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(AppError::new(
            "Argo CD response exceeded size limit",
            "argoApi",
        ));
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        push_capped_chunk(&mut bytes, &chunk.map_err(safe_http_error)?)?;
    }
    Ok(bytes)
}
fn push_capped_chunk(bytes: &mut Vec<u8>, chunk: &[u8]) -> Result<(), AppError> {
    if chunk.len() > MAX_RESPONSE_BYTES.saturating_sub(bytes.len()) {
        return Err(AppError::new(
            "Argo CD response exceeded size limit",
            "argoApi",
        ));
    }
    bytes.extend_from_slice(chunk);
    Ok(())
}
fn safe_message(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(240)
        .collect()
}
pub(crate) fn redact_secret_fields(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let is_secret = map.get("kind").and_then(Value::as_str) == Some("Secret");
            if is_secret {
                for key in ["data", "stringData"] {
                    if let Some(Value::Object(data)) = map.get_mut(key) {
                        for item in data.values_mut() {
                            *item = Value::String("[REDACTED]".into());
                        }
                    }
                }
            }
            for child in map.values_mut() {
                redact_secret_fields(child);
            }
        }
        Value::Array(values) => {
            for child in values {
                redact_secret_fields(child);
            }
        }
        _ => {}
    }
}

fn safe_http_error(error: reqwest::Error) -> AppError {
    AppError::new(
        format!("Argo CD request failed: {}", error.without_url()),
        "argoConnection",
    )
}

fn http_client(insecure_tls: bool, custom_ca_pem: Option<Vec<u8>>) -> Result<HttpClient, AppError> {
    let mut builder = HttpClient::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .connect_timeout(Duration::from_secs(8));
    if insecure_tls {
        builder = builder.danger_accept_invalid_certs(true);
    }
    if let Some(pem) = custom_ca_pem {
        let certificate = Certificate::from_pem(&pem)
            .map_err(|_| AppError::new("invalid custom CA", "argoConnection"))?;
        builder = builder.add_root_certificate(certificate);
    }
    builder.build().map_err(safe_http_error)
}

pub(crate) fn url(base: &str, path: &str) -> Result<String, AppError> {
    let mut base = canonical_url(base)?;
    if !base.path().ends_with('/') {
        base.set_path(&format!("{}/", base.path()));
    }
    base.join(path.trim_start_matches('/'))
        .map(|value| value.to_string())
        .map_err(|_| AppError::new("invalid Argo CD API path", "argoConnection"))
}
fn canonical_url(value: &str) -> Result<reqwest::Url, AppError> {
    let mut base = reqwest::Url::parse(value.trim())
        .map_err(|_| AppError::new("invalid Argo CD URL", "argoConnection"))?;
    if base.scheme() != "https" {
        return Err(AppError::new(
            "Argo CD URL must use HTTPS",
            "argoConnection",
        ));
    }
    if !base.username().is_empty() || base.password().is_some() {
        return Err(AppError::new(
            "Argo CD URL must not contain credentials",
            "argoConnection",
        ));
    }
    base.set_query(None);
    base.set_fragment(None);
    Ok(base)
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
        .list(&Default::default())
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
    let token = match submitted_token.or(stored_token) {
        Some(token) => token,
        None => {
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
        }
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
    Ok(ArgoApplicationInspector {
        application: ArgoApplicationRef {
            name: application.metadata.name.clone().unwrap_or_default(),
            namespace: application.metadata.namespace.clone(),
            project: data.get("spec").and_then(|v| text(v, "project")),
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
            .and_then(|v| v.get("history"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(history)
            .collect(),
        resources: source
            .and_then(|v| v.get("resources"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(managed_resource)
            .collect(),
        conditions: source
            .and_then(|v| v.get("conditions"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        operation_state: source.and_then(|v| v.get("operationState")).cloned(),
        status,
        connected: false,
    })
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

#[tauri::command]
pub async fn get_argo_application_inspector(
    store: tauri::State<'_, ArgoConnectionStore>,
    cluster_context: String,
    kubeconfig_env_var: Option<String>,
    connection_id: Option<String>,
    transport: String,
    application: ArgoApplicationRef,
    _redact_secrets: Option<bool>,
) -> Result<ArgoApplicationInspector, AppError> {
    if transport == "kubernetes" {
        return inspector_from_application(
            &kubernetes_application(
                &cluster_context,
                application.namespace.as_deref(),
                &application.name,
                kubeconfig_env_var,
            )
            .await?,
        );
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
    let namespace = application.namespace.clone().unwrap_or_default();
    let mut response = api_get(
        &connection,
        &format!(
            "/api/v1/applications/{}?appNamespace={}&project={}",
            application.name,
            namespace,
            application.project.clone().unwrap_or_default()
        ),
    )
    .await?;
    redact_secret_fields(&mut response);
    let status = response.get("status").cloned();
    Ok(ArgoApplicationInspector {
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
            .and_then(|v| v.get("history"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(history)
            .collect(),
        resources: status
            .as_ref()
            .and_then(|v| v.get("resources"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(managed_resource)
            .collect(),
        conditions: status
            .as_ref()
            .and_then(|v| v.get("conditions"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        operation_state: status
            .as_ref()
            .and_then(|v| v.get("operationState"))
            .cloned(),
        status,
        connected: true,
    })
}

#[tauri::command]
pub async fn get_argo_application_resources(
    store: tauri::State<'_, ArgoConnectionStore>,
    cluster_context: String,
    kubeconfig_env_var: Option<String>,
    connection_id: Option<String>,
    transport: String,
    application: ArgoApplicationRef,
    _redact_secrets: Option<bool>,
) -> Result<Vec<ArgoManagedResource>, AppError> {
    if transport == "connected" {
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
        return Ok(value
            .get("items")
            .or_else(|| value.get("managedResources"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(managed_resource)
            .collect());
    }
    if transport != "kubernetes" {
        return Err(AppError::new("invalid Argo CD transport", "argoConnection"));
    }
    Ok(inspector_from_application(
        &kubernetes_application(
            &cluster_context,
            application.namespace.as_deref(),
            &application.name,
            kubeconfig_env_var,
        )
        .await?,
    )?
    .resources)
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
}
