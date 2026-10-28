mod credentials;
mod discovery;
mod inspection;
mod inspector_read;
mod store;

pub(crate) use super::transport::{api_delete, api_get, api_post, redact_secret_fields};
#[cfg(feature = "bench-support")]
pub use inspection::{benchmark_connected_inspector, benchmark_kubernetes_inspector};
pub(crate) use inspection::{
    inspector_from_application, kubernetes_comparison, managed_resource, state, text,
};
pub(crate) use inspector_read::{kubernetes_application, managed_resource_path};
pub use store::ArgoConnectionStore;
pub(crate) use store::ConnectedArgo;

use self::{
    credentials::{
        connection_generation, delete_credential, load_credential, save_credential,
        ActiveCredentialStore, StoredCredential,
    },
    inspector_read::application_inspector,
};
use super::certificate_pin::{parse_certificate_sha256, pinned_tls_config, PresentedCertificate};
use super::transport::{
    argo_url, http_client, normalize_endpoint, response_json, safe_http_error, url, ServerTrust,
};
use super::tunnel::{verify_confirmed_target, ArgoServiceTunnel};
use crate::commands::{diagnostic_field, BackendCancellationRegistry, ClusterAccess, FiniteRead};
use crate::models::AppErrorKind;
use crate::models::{
    AppError, ArgoApplicationInspector, ArgoApplicationRef, ArgoConfirmedTarget,
    ArgoConnectionProfile, ArgoConnectionStatus, ArgoServerCapability, ArgoServerEndpoint,
    ArgoTunnelTargetPreview,
};
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
                discovery::discover(&client).await
            }),
            |capabilities| vec![diagnostic_field("servers", capabilities.len())],
        )
        .await
}

/// Resolves the Pod a private tunnel reaches now and, for HTTPS, the certificate it presents,
/// so the user can confirm both before any credential is sent.
pub async fn preview_argo_tunnel_target(
    cluster_context: String,
    kubeconfig_env_var: Option<String>,
    endpoint: ArgoServerEndpoint,
) -> Result<ArgoTunnelTargetPreview, AppError> {
    let (endpoint, server_url) = normalize_endpoint(endpoint)?;
    let ArgoServerEndpoint::ServiceTunnel {
        namespace,
        service_name,
        service_port,
        scheme,
        ..
    } = endpoint
    else {
        return Err(AppError::new(
            "only Argo CD Service tunnels have a target to preview",
            AppErrorKind::ArgoConnection,
        ));
    };
    let access = ClusterAccess::resolve(kubeconfig_env_var.as_deref())?;
    let tunnel = ArgoServiceTunnel::start(
        &access,
        &cluster_context,
        namespace.clone(),
        service_name.clone(),
        service_port,
    )
    .await?;
    let certificate_sha256 = if scheme == "https" {
        Some(probe_certificate(&server_url, &tunnel).await?)
    } else {
        None
    };
    Ok(ArgoTunnelTargetPreview {
        namespace,
        service_name,
        pod_name: tunnel.pod_name().to_string(),
        certificate_sha256,
    })
}

/// A verifier without a pin rejects every certificate, so the probe ends at the TLS
/// handshake and never sends an HTTP request.
async fn probe_certificate(
    server_url: &str,
    tunnel: &ArgoServiceTunnel,
) -> Result<String, AppError> {
    let (config, presented) = pinned_tls_config(None)?;
    let (host, address) = tunnel_route(server_url, tunnel)?;
    let client = http_client(ServerTrust::Pinned(config), Some((&host, address)))?;
    let _ = client.get(url(server_url, "/")?).send().await;
    presented.get().ok_or_else(|| {
        AppError::new(
            "Argo CD did not present an HTTPS certificate on this Service port; if the server runs with server.insecure, use HTTP",
            AppErrorKind::ArgoConnection,
        )
    })
}

/// The URL keeps the Service host for the Host header and SNI; the socket is the tunnel.
fn tunnel_route(
    server_url: &str,
    tunnel: &ArgoServiceTunnel,
) -> Result<(String, SocketAddr), AppError> {
    let host = argo_url(server_url)?
        .host_str()
        .expect("normalized service endpoint has a host")
        .to_string();
    let address = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, tunnel.local_port()));
    Ok((host, address))
}

/// HTTPS through a tunnel accepts one certificate: the one confirmed with the Pod, or for a
/// Service-level reconnect, the one saved with the remembered credential.
fn tunnel_certificate_pin(
    confirmed: &ArgoConfirmedTarget,
    stored: Option<&StoredCredential>,
) -> Result<String, AppError> {
    if confirmed.pod_name.is_none() {
        return stored
            .and_then(|record| record.certificate_sha256.clone())
            .ok_or_else(|| {
                AppError::new(
                    "this saved Argo CD profile has no confirmed certificate; connect and confirm the target again",
                    AppErrorKind::ArgoConnection,
                )
            });
    }
    let fingerprint = confirmed.certificate_sha256.as_deref().ok_or_else(|| {
        AppError::new(
            "confirm the Argo CD server certificate before sending credentials",
            AppErrorKind::ArgoConnection,
        )
    })?;
    parse_certificate_sha256(fingerprint)
}

/// A request rejected by the certificate pin reports the rotation instead of a TLS error.
fn request_error(error: reqwest::Error, pin: Option<&(String, PresentedCertificate)>) -> AppError {
    let rotated = pin.is_some_and(|(pinned, presented)| {
        presented
            .get()
            .is_some_and(|fingerprint| &fingerprint != pinned)
    });
    if rotated {
        return AppError::new(
            "the Argo CD server certificate changed since it was confirmed; connect and confirm the target again",
            AppErrorKind::ArgoConnection,
        );
    }
    safe_http_error(error)
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
    let credential_store = ActiveCredentialStore;
    let submitted_token = token.filter(|value| !value.is_empty());
    let username = username.filter(|value| !value.is_empty());
    let password = password.filter(|value| !value.is_empty());
    let submitted_credentials =
        submitted_token.is_some() || username.is_some() || password.is_some();
    let stored = if remember_credential {
        load_credential(&credential_store, &profile)?
    } else {
        None
    };
    let mut tunnel = None;
    let mut pin = None;
    let client = match &profile.endpoint {
        ArgoServerEndpoint::ExternalHttps { .. } => http_client(
            ServerTrust::Default {
                insecure_tls,
                custom_ca_pem: custom_ca_pem.clone(),
            },
            None,
        )?,
        ArgoServerEndpoint::ServiceTunnel {
            namespace,
            service_name,
            service_port,
            scheme,
            ..
        } => {
            if insecure_tls || custom_ca_pem.is_some() {
                return Err(AppError::new(
                    "Argo CD Service tunnels trust only the confirmed certificate; insecure TLS and custom CAs do not apply",
                    AppErrorKind::ArgoConnection,
                ));
            }
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
            // A remembered credential was saved after a full confirmation of this Service.
            let service_level_allowed = !submitted_credentials && stored.is_some();
            let confirmed = verify_confirmed_target(
                confirmed_target.as_ref(),
                namespace,
                service_name,
                started.pod_name(),
                service_level_allowed,
            )?;
            let (host, address) = tunnel_route(&profile.url, &started)?;
            let trust = if scheme == "https" {
                let fingerprint = tunnel_certificate_pin(confirmed, stored.as_ref())?;
                let (config, presented) = pinned_tls_config(Some(fingerprint.clone()))?;
                pin = Some((fingerprint, presented));
                ServerTrust::Pinned(config)
            } else {
                ServerTrust::Default {
                    insecure_tls: false,
                    custom_ca_pem: None,
                }
            };
            tunnel = Some(started);
            http_client(trust, Some((&host, address)))?
        }
    };
    let stored_token = stored.as_ref().map(|record| record.token.clone());
    // Credentials typed now win over a remembered token, which may have expired.
    let token = if let Some(token) = submitted_token.clone().or(if submitted_credentials {
        None
    } else {
        stored_token
    }) {
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
            .map_err(|error| request_error(error, pin.as_ref()))?;
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
        .map_err(|error| request_error(error, pin.as_ref()))?;
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
    let certificate_sha256 = pin.map(|(fingerprint, _)| fingerprint);
    let pin_changed = stored
        .as_ref()
        .is_some_and(|record| record.certificate_sha256 != certificate_sha256);
    let generation = connection_generation(
        stored.as_ref(),
        submitted_credentials || pin_changed,
        remember_credential,
        insecure_tls,
        custom_ca_pem.as_deref(),
    );
    if remember_credential && (submitted_credentials || stored.is_none() || pin_changed) {
        save_credential(
            &credential_store,
            &profile,
            &token,
            generation.clone(),
            certificate_sha256,
        )?;
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
    let connection = store.lock_connections()?.get(&id).cloned();
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
