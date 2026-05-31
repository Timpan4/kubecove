use crate::models::{AppError, PortForwardRequest, PortForwardSessionSummary};
use chrono::Utc;
use k8s_openapi::api::core::v1::Pod;
use kube::{api::Api, config::KubeConfigOptions, Client};
use std::{
    collections::BTreeMap,
    net::{Ipv4Addr, SocketAddrV4},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use tauri::{async_runtime::JoinHandle, State};
use tokio::{
    io::copy_bidirectional,
    net::{TcpListener, TcpStream},
    task::JoinSet,
};

mod service;

const LOCAL_ADDRESS: &str = "127.0.0.1";
const MIN_USER_LOCAL_PORT: u16 = 1024;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum PortForwardTargetKind {
    Pod,
    Service,
}

impl PortForwardTargetKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pod => "Pod",
            Self::Service => "Service",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct ValidatedPortForwardRequest {
    cluster_context: String,
    namespace: String,
    target_kind: PortForwardTargetKind,
    target_name: String,
    remote_port: u16,
    local_port: Option<u16>,
}

#[derive(Debug, Clone)]
struct PortForwardTarget {
    cluster_context: String,
    namespace: String,
    target_kind: PortForwardTargetKind,
    target_name: String,
    pod_name: String,
    remote_port: u16,
    pod_port: u16,
}

struct PortForwardSession {
    summary: PortForwardSessionSummary,
    handle: Option<JoinHandle<()>>,
}

#[derive(Default)]
struct PortForwardState {
    sessions: BTreeMap<String, PortForwardSession>,
}

#[derive(Clone, Default)]
pub struct PortForwardRegistry {
    next_id: Arc<AtomicU64>,
    state: Arc<Mutex<PortForwardState>>,
}

impl PortForwardRegistry {
    fn session_id(&self) -> String {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        format!("port-forward-{id}")
    }

    fn has_local_port(&self, local_port: u16) -> bool {
        self.state
            .lock()
            .expect("port-forward registry lock")
            .sessions
            .values()
            .any(|session| session.summary.local_port == local_port)
    }

    fn insert(
        &self,
        summary: PortForwardSessionSummary,
        handle: JoinHandle<()>,
    ) -> Result<PortForwardSessionSummary, AppError> {
        let mut state = self.state.lock().expect("port-forward registry lock");
        if state
            .sessions
            .values()
            .any(|session| session.summary.local_port == summary.local_port)
        {
            handle.abort();
            return Err(AppError::new(
                format!("local port {} is already forwarded", summary.local_port),
                "session",
            ));
        }
        state.sessions.insert(
            summary.id.clone(),
            PortForwardSession {
                summary: summary.clone(),
                handle: Some(handle),
            },
        );
        Ok(summary)
    }

    fn list(&self) -> Vec<PortForwardSessionSummary> {
        self.state
            .lock()
            .expect("port-forward registry lock")
            .sessions
            .values()
            .map(|session| session.summary.clone())
            .collect()
    }

    fn mark_status(&self, session_id: &str, status: &str, last_error: Option<String>) {
        if let Some(session) = self
            .state
            .lock()
            .expect("port-forward registry lock")
            .sessions
            .get_mut(session_id)
        {
            session.summary.status = status.to_string();
            session.summary.last_error = last_error;
        }
    }

    fn mark_error(&self, session_id: &str, message: String) {
        self.mark_status(session_id, "error", Some(message));
    }

    fn stop(&self, session_id: &str) -> bool {
        let session = self
            .state
            .lock()
            .expect("port-forward registry lock")
            .sessions
            .remove(session_id);
        let Some(mut session) = session else {
            return false;
        };
        if let Some(handle) = session.handle.take() {
            handle.abort();
        }
        true
    }

    #[cfg(test)]
    fn insert_summary_for_test(&self, summary: PortForwardSessionSummary) {
        self.state
            .lock()
            .expect("port-forward registry lock")
            .sessions
            .insert(
                summary.id.clone(),
                PortForwardSession {
                    summary,
                    handle: None,
                },
            );
    }
}

fn validate_port(value: i64, field: &str) -> Result<u16, AppError> {
    if !(1..=u16::MAX as i64).contains(&value) {
        return Err(AppError::new(
            format!("{field} must be between 1 and 65535"),
            "validation",
        ));
    }
    Ok(value as u16)
}

fn target_text(value: Option<&String>) -> Option<String> {
    value
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn validate_target_kind(request: &PortForwardRequest) -> Result<PortForwardTargetKind, AppError> {
    let target_kind =
        target_text(request.target_kind.as_ref()).unwrap_or_else(|| "Pod".to_string());
    match target_kind.as_str() {
        "Pod" => Ok(PortForwardTargetKind::Pod),
        "Service" => Ok(PortForwardTargetKind::Service),
        _ => Err(AppError::new(
            "port-forward target kind must be Pod or Service",
            "validation",
        )),
    }
}

fn validate_request(request: &PortForwardRequest) -> Result<ValidatedPortForwardRequest, AppError> {
    if request.cluster_context.trim().is_empty() || request.namespace.trim().is_empty() {
        return Err(AppError::new(
            "port-forward target is required",
            "validation",
        ));
    }
    let target_kind = validate_target_kind(request)?;
    let target_name = target_text(request.target_name.as_ref())
        .or_else(|| target_text(request.pod_name.as_ref()))
        .ok_or_else(|| AppError::new("port-forward target is required", "validation"))?;

    let remote_port = validate_port(request.remote_port, "remote_port")?;
    let local_port = request
        .local_port
        .map(|port| validate_port(port, "local_port"))
        .transpose()?;
    if matches!(local_port, Some(port) if port < MIN_USER_LOCAL_PORT) {
        return Err(AppError::new(
            "local_port must be 1024 or higher",
            "validation",
        ));
    }

    Ok(ValidatedPortForwardRequest {
        cluster_context: request.cluster_context.trim().to_string(),
        namespace: request.namespace.trim().to_string(),
        target_kind,
        target_name,
        remote_port,
        local_port,
    })
}

async fn resolve_port_forward_target(
    request: ValidatedPortForwardRequest,
) -> Result<PortForwardTarget, AppError> {
    match request.target_kind {
        PortForwardTargetKind::Pod => Ok(PortForwardTarget {
            cluster_context: request.cluster_context,
            namespace: request.namespace,
            target_kind: PortForwardTargetKind::Pod,
            target_name: request.target_name.clone(),
            pod_name: request.target_name,
            remote_port: request.remote_port,
            pod_port: request.remote_port,
        }),
        PortForwardTargetKind::Service => service::resolve_service_target(request).await,
    }
}

pub(super) async fn client_for_context(cluster_context: &str) -> Result<Client, AppError> {
    let options = KubeConfigOptions {
        context: Some(cluster_context.to_string()),
        ..Default::default()
    };

    let config = kube::Config::from_kubeconfig(&options)
        .await
        .map_err(|e| AppError::kube(e.to_string()))?;
    Client::try_from(config).map_err(|e| AppError::kube(e.to_string()))
}

fn port_forward_error_message(err: kube::Error) -> String {
    let message = err.to_string();
    if message.to_ascii_lowercase().contains("forbidden") {
        return format!("port-forward forbidden by Kubernetes RBAC: {message}");
    }
    message
}

async fn verify_pod_port_forward(
    client: Client,
    target: &PortForwardTarget,
) -> Result<(), AppError> {
    let pods: Api<Pod> = Api::namespaced(client, &target.namespace);
    let forwarder = pods
        .portforward(&target.pod_name, &[target.pod_port])
        .await
        .map_err(|err| AppError::kube(port_forward_error_message(err)))?;
    forwarder.abort();
    Ok(())
}

fn session_summary(
    id: String,
    target: &PortForwardTarget,
    local_port: u16,
) -> PortForwardSessionSummary {
    PortForwardSessionSummary {
        id,
        cluster_context: target.cluster_context.clone(),
        namespace: target.namespace.clone(),
        target_kind: target.target_kind.as_str().to_string(),
        target_name: target.target_name.clone(),
        pod_name: target.pod_name.clone(),
        remote_port: target.remote_port,
        resolved_pod_name: target.pod_name.clone(),
        resolved_pod_port: target.pod_port,
        local_port,
        local_address: LOCAL_ADDRESS.to_string(),
        local_url: format!("http://{LOCAL_ADDRESS}:{local_port}"),
        status: "listening".to_string(),
        started_at: Utc::now().to_rfc3339(),
        last_error: None,
    }
}

async fn forward_connection(
    client: Client,
    target: PortForwardTarget,
    mut local_stream: TcpStream,
) -> Result<(), String> {
    let pods: Api<Pod> = Api::namespaced(client, &target.namespace);
    let mut forwarder = pods
        .portforward(&target.pod_name, &[target.pod_port])
        .await
        .map_err(port_forward_error_message)?;
    let mut pod_stream = forwarder
        .take_stream(target.pod_port)
        .ok_or_else(|| format!("remote port {} did not open", target.pod_port))?;
    let error_future = forwarder.take_error(target.pod_port);

    let result = if let Some(error_future) = error_future {
        tokio::pin!(error_future);
        tokio::select! {
            copy_result = copy_bidirectional(&mut local_stream, &mut pod_stream) => {
                copy_result.map(|_| ()).map_err(|err| err.to_string())
            }
            port_error = &mut error_future => {
                match port_error {
                    Some(message) if !message.trim().is_empty() => Err(message),
                    _ => Ok(()),
                }
            }
        }
    } else {
        copy_bidirectional(&mut local_stream, &mut pod_stream)
            .await
            .map(|_| ())
            .map_err(|err| err.to_string())
    };

    forwarder.abort();
    result
}

async fn run_port_forward_session(
    session_id: String,
    target: PortForwardTarget,
    listener: TcpListener,
    registry: PortForwardRegistry,
    client: Client,
) {
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            accept_result = listener.accept() => {
                let (local_stream, _) = match accept_result {
                    Ok(accepted) => accepted,
                    Err(err) => {
                        registry.mark_error(&session_id, err.to_string());
                        break;
                    }
                };
                registry.mark_status(&session_id, "connected", None);
                let connection_target = target.clone();
                let connection_client = client.clone();
                connections.spawn(async move {
                    forward_connection(connection_client, connection_target, local_stream).await
                });
            }
            join_result = connections.join_next(), if !connections.is_empty() => {
                match join_result {
                    Some(Ok(Ok(()))) => {
                        if connections.is_empty() {
                            registry.mark_status(&session_id, "listening", None);
                        }
                    }
                    Some(Ok(Err(message))) => {
                        registry.mark_error(&session_id, message);
                    }
                    Some(Err(err)) => {
                        registry.mark_error(&session_id, err.to_string());
                    }
                    None => {}
                }
            }
        }
    }
}

async fn start_pod_port_forward_in_registry(
    request: PortForwardRequest,
    registry: &PortForwardRegistry,
) -> Result<PortForwardSessionSummary, AppError> {
    let request = validate_request(&request)?;
    if let Some(local_port) = request.local_port {
        if registry.has_local_port(local_port) {
            return Err(AppError::new(
                format!("local port {local_port} is already forwarded"),
                "session",
            ));
        }
    }

    let bind_port = request.local_port.unwrap_or(0);
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, bind_port))
        .await
        .map_err(|err| AppError::new(format!("local port unavailable: {err}"), "session"))?;
    let local_port = listener
        .local_addr()
        .map_err(|err| AppError::new(format!("local port unavailable: {err}"), "session"))?
        .port();
    if registry.has_local_port(local_port) {
        return Err(AppError::new(
            format!("local port {local_port} is already forwarded"),
            "session",
        ));
    }

    let target = resolve_port_forward_target(request).await?;
    let client = client_for_context(&target.cluster_context).await?;
    verify_pod_port_forward(client.clone(), &target).await?;

    let session_id = registry.session_id();
    let summary = session_summary(session_id.clone(), &target, local_port);
    let handle_registry = registry.clone();
    let handle_target = target.clone();
    let handle_session_id = session_id.clone();
    let handle = tauri::async_runtime::spawn(async move {
        run_port_forward_session(
            handle_session_id,
            handle_target,
            listener,
            handle_registry,
            client,
        )
        .await;
    });
    registry.insert(summary, handle)
}

#[tauri::command]
pub async fn start_pod_port_forward(
    request: PortForwardRequest,
    registry: State<'_, PortForwardRegistry>,
) -> Result<PortForwardSessionSummary, AppError> {
    start_pod_port_forward_in_registry(request, registry.inner()).await
}

#[tauri::command]
pub async fn stop_port_forward(
    session_id: String,
    registry: State<'_, PortForwardRegistry>,
) -> Result<bool, AppError> {
    if session_id.trim().is_empty() {
        return Err(AppError::new(
            "port-forward session id is required",
            "validation",
        ));
    }
    Ok(registry.stop(session_id.trim()))
}

#[tauri::command]
pub async fn list_port_forwards(
    registry: State<'_, PortForwardRegistry>,
) -> Result<Vec<PortForwardSessionSummary>, AppError> {
    Ok(registry.list())
}

#[cfg(test)]
mod tests;
