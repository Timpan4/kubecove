mod credentials;
mod discovery;
mod inspection;
mod inspector_read;
mod store;

pub(crate) use super::transport::{api_delete, api_get, api_post, redact_secret_fields};
#[cfg(feature = "bench-support")]
pub use inspection::{benchmark_connected_inspector, benchmark_kubernetes_inspector};
pub(crate) use inspection::{inspector_from_application, managed_resource, state, text};
pub(crate) use inspector_read::{connected_application_path, kubernetes_application};
pub use store::ArgoConnectionStore;
pub(crate) use store::ConnectedArgo;

use self::{
    credentials::{
        connection_generation, delete_credential, load_credential, save_credential,
        ActiveCredentialStore,
    },
    discovery::{is_argo_labeled, servicetunnel_capabilities, tunnel_target_unavailable},
    inspector_read::application_inspector,
};
use super::transport::{
    argo_url, http_client, normalize_endpoint, response_json, safe_http_error, url,
};
use super::tunnel::{verify_confirmed_target, ArgoServiceTunnel};
use crate::commands::{diagnostic_field, BackendCancellationRegistry, ClusterAccess, FiniteRead};
use crate::models::AppErrorKind;
use crate::models::{
    AppError, ArgoApplicationInspector, ArgoApplicationRef, ArgoConfirmedTarget,
    ArgoConnectionProfile, ArgoConnectionStatus, ArgoServerCapability, ArgoServerEndpoint,
};
use k8s_openapi::api::core::v1::Service;
use kube::api::{Api, ListParams};
use serde_json::{json, Value};
use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    sync::Arc,
};
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

pub async fn discover_argo_servers(
    cluster_context: String,
    kubeconfig_env_var: Option<String>,
    request_id: Option<String>,
    cancel_scope: Option<String>,
    cancellations: crate::runtime::State<'_, BackendCancellationRegistry>,
) -> Result<Vec<ArgoServerCapability>, AppError> {
    FiniteRead::new("discover_argo_servers")
        .label("context", &cluster_context)
        .cancellable(&cancellations, cancel_scope, request_id)
        .run(
            Box::pin(async move {
                let client = ClusterAccess::resolve(kubeconfig_env_var.as_deref())?
                    .read_client(&cluster_context)
                    .await?;
                let services: Api<Service> = Api::all(client.clone());
                let list = services
                    .list(&ListParams::default())
                    .await
                    .map_err(AppError::from)?;
                let mut capabilities = Vec::new();
                for service in list.items {
                    for mut capability in servicetunnel_capabilities(&service) {
                        capability.argo_labeled = is_argo_labeled(&service);
                        if let Some(ArgoServerEndpoint::ServiceTunnel {
                            namespace,
                            service_name,
                            service_port,
                            ..
                        }) = capability.endpoint.as_ref()
                        {
                            match crate::commands::sessions::service::resolve_service_target(
                                client.clone(),
                                namespace,
                                service_name,
                                *service_port,
                            )
                            .await
                            {
                                Ok(target) => capability.target_pod = Some(target.pod_name),
                                Err(error) => {
                                    capability.unavailable =
                                        Some(tunnel_target_unavailable(&error));
                                    capability.unavailable_reason = Some(error.message);
                                }
                            }
                        }
                        capabilities.push(capability);
                    }
                }
                Ok(capabilities)
            }),
            |capabilities| vec![diagnostic_field("servers", capabilities.len())],
        )
        .await
}

pub async fn connect_argo_server(
    id: String,
    server_url: String,
    endpoint: Option<ArgoServerEndpoint>,
    token: Option<String>,
    username: Option<String>,
    password: Option<String>,
    insecure_tls: bool,
    custom_ca_pem: Option<Vec<u8>>,
    remember_credential: bool,
    cluster_context: Option<String>,
    kubeconfig_env_var: Option<String>,
    workspace_id: Option<String>,
    confirmed_target: Option<ArgoConfirmedTarget>,
    store: crate::runtime::State<'_, ArgoConnectionStore>,
) -> Result<ArgoConnectionStatus, AppError> {
    let connection_epoch = store.connection_epoch();
    let access = ClusterAccess::resolve(kubeconfig_env_var.as_deref())?;
    let (endpoint, normalized_url) = normalize_endpoint(
        endpoint.unwrap_or(ArgoServerEndpoint::ExternalHttps { url: server_url }),
    )?;
    let profile = ArgoConnectionProfile {
        id,
        url: normalized_url,
        endpoint,
        cluster_context,
        workspace_id,
        kubeconfig_source_key: Some(access.source_key().to_string()),
        transport: "connected".into(),
        remember_credential,
    };
    let mut tunnel = None;
    let resolved_host = if let ArgoServerEndpoint::ServiceTunnel {
        namespace,
        service_name,
        service_port,
        ..
    } = &profile.endpoint
    {
        let cluster_context = profile.cluster_context.as_deref().ok_or_else(|| {
            AppError::new(
                "clusterContext required for an Argo CD Service tunnel",
                AppErrorKind::ArgoConnection,
            )
        })?;
        let started = ArgoServiceTunnel::start(
            &access,
            cluster_context,
            namespace.clone(),
            service_name.clone(),
            *service_port,
        )
        .await?;
        verify_confirmed_target(
            confirmed_target.as_ref(),
            namespace,
            service_name,
            started.pod_name(),
        )?;
        let host = argo_url(&profile.url)?
            .host_str()
            .expect("normalized service endpoint has a host")
            .to_string();
        let address = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, started.local_port()));
        tunnel = Some(started);
        Some((host, address))
    } else {
        None
    };
    let client = http_client(
        insecure_tls,
        custom_ca_pem.clone(),
        resolved_host
            .as_ref()
            .map(|(host, address)| (host.as_str(), *address)),
    )?;
    let credential_store = ActiveCredentialStore;
    let submitted_token = token.filter(|value| !value.is_empty());
    let stored = if remember_credential {
        load_credential(&credential_store, &profile)?
    } else {
        None
    };
    let stored_token = stored.as_ref().map(|record| record.token.clone());
    let token = if let Some(token) = submitted_token.clone().or(stored_token) {
        token
    } else {
        let user = username.ok_or_else(|| {
            AppError::new(
                "token or local login credentials required",
                AppErrorKind::ArgoConnection,
            )
        })?;
        let password = password.ok_or_else(|| {
            AppError::new(
                "token or local login credentials required",
                AppErrorKind::ArgoConnection,
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
                AppErrorKind::ArgoConnection,
            ));
        }
        response_json(response)
            .await?
            .get("token")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| {
                AppError::new(
                    "Argo CD login returned no token",
                    AppErrorKind::ArgoConnection,
                )
            })?
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
            AppErrorKind::ArgoConnection,
        ));
    }
    let userinfo = response_json(response).await?;
    let user = userinfo
        .get("username")
        .or_else(|| userinfo.get("sub"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let generation = connection_generation(
        stored.as_ref(),
        submitted_token.is_some(),
        remember_credential,
        insecure_tls,
        custom_ca_pem.as_deref(),
    );
    if remember_credential && (submitted_token.is_some() || stored.is_none()) {
        save_credential(&credential_store, &profile, &token, generation.clone())?;
    } else if !remember_credential {
        // A stale remembered credential must not revive on a later reconnect.
        delete_credential(&credential_store, &profile)?;
    }
    store
        .replace_connection_at(
            profile.id.clone(),
            Arc::new(ConnectedArgo {
                profile: profile.clone(),
                token,
                username: user.clone(),
                client,
                tunnel,
                generation,
                instance_id: Uuid::new_v4().to_string(),
                gate: Arc::new(AsyncMutex::new(())),
            }),
            connection_epoch,
        )
        .await?;
    Ok(ArgoConnectionStatus {
        profile: Some(profile),
        connected: true,
        username: user,
        unavailable_reason: None,
    })
}

pub fn get_argo_connection_status(
    id: String,
    store: crate::runtime::State<'_, ArgoConnectionStore>,
) -> Result<ArgoConnectionStatus, AppError> {
    let connection = store
        .connections
        .lock()
        .map_err(|_| {
            AppError::new(
                "Argo CD connection state unavailable",
                AppErrorKind::ArgoConnection,
            )
        })?
        .get(&id)
        .cloned();
    Ok(match connection {
        Some(connection) => ArgoConnectionStatus {
            profile: Some(connection.profile.clone()),
            connected: true,
            username: connection.username.clone(),
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

pub async fn disconnect_argo_server(
    id: String,
    store: crate::runtime::State<'_, ArgoConnectionStore>,
) -> Result<(), AppError> {
    store.remove_connection(&id).await
}

pub fn forget_argo_credential(profile: ArgoConnectionProfile) -> Result<(), AppError> {
    delete_credential(&ActiveCredentialStore, &profile)
}

pub async fn get_argo_application_inspector(
    cluster_context: String,
    kubeconfig_env_var: Option<String>,
    connection_id: Option<String>,
    transport: String,
    application: ArgoApplicationRef,
    request_id: Option<String>,
    cancel_scope: Option<String>,
    store: crate::runtime::State<'_, ArgoConnectionStore>,
    cancellations: crate::runtime::State<'_, BackendCancellationRegistry>,
) -> Result<ArgoApplicationInspector, AppError> {
    FiniteRead::new("get_argo_application_inspector")
        .label("context", &cluster_context)
        .label("transport", &transport)
        .label("application", &application.name)
        .cancellable(&cancellations, cancel_scope, request_id)
        .run(
            async {
                let access = ClusterAccess::resolve(kubeconfig_env_var.as_deref())?;
                application_inspector(
                    &store,
                    &access,
                    cluster_context,
                    connection_id,
                    transport,
                    application,
                )
                .await
            },
            |inspector| {
                vec![
                    diagnostic_field("transport", &inspector.transport),
                    diagnostic_field("resources", inspector.resources.len()),
                    diagnostic_field("comparisons", inspector.comparisons.len()),
                ]
            },
        )
        .await
}
