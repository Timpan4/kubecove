use crate::models::AppErrorKind;
use crate::models::{AppError, PortForwardRequest, PortForwardSessionSummary};
use k8s_openapi::api::core::v1::Pod;
use kube::{api::Api, Client};
use tokio::{
    io::{copy_bidirectional, AsyncRead, AsyncWrite},
    net::TcpStream,
};

use super::lifecycle::PortForwardRegistry;
use super::target::{
    resolve_port_forward_target, validate_request, PortForwardTarget, ValidatedPortForwardRequest,
};
use crate::commands::ClusterAccess;

fn port_forward_error(err: kube::Error) -> AppError {
    let mut error = AppError::from(err);
    if error.kind == AppErrorKind::Forbidden {
        error.message = format!(
            "port-forward forbidden by Kubernetes RBAC: {}",
            error.message
        );
    }
    error
}

// kube's Portforwarder does not abort its task on drop.
struct OwnedPortforwarder(kube::api::Portforwarder);

impl Drop for OwnedPortforwarder {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn verify_pod_port_forward(
    client: Client,
    target: &PortForwardTarget,
) -> Result<(), AppError> {
    let pods: Api<Pod> = Api::namespaced(client, &target.namespace);
    let forwarder = pods
        .portforward(&target.pod_name, &[target.pod_port])
        .await
        .map_err(|err| {
            let mut error = port_forward_error(err);
            error.kind = AppErrorKind::LiveSessionTargetUnavailable;
            error
        })?;
    forwarder.abort();
    Ok(())
}

pub async fn forward_pod_connection<S>(
    client: Client,
    namespace: String,
    pod_name: String,
    pod_port: u16,
    mut local_stream: S,
) -> Result<(), AppError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let pods: Api<Pod> = Api::namespaced(client, &namespace);
    let mut forwarder = OwnedPortforwarder(
        pods.portforward(&pod_name, &[pod_port])
            .await
            .map_err(port_forward_error)?,
    );
    let mut pod_stream = forwarder.0.take_stream(pod_port).ok_or_else(|| {
        AppError::new(
            format!("remote port {pod_port} did not open"),
            AppErrorKind::Session,
        )
    })?;
    let error_future = forwarder.0.take_error(pod_port);
    let copy = copy_bidirectional(&mut local_stream, &mut pod_stream);
    tokio::pin!(copy);
    let result = if let Some(error_future) = error_future {
        tokio::pin!(error_future);
        tokio::select! {
            copy_result = &mut copy => copy_result,
            port_error = &mut error_future => {
                match port_error {
                    Some(message) if !message.trim().is_empty() => return Err(AppError::new(message, AppErrorKind::Session)),
                    // Closing the diagnostic channel does not complete data transfer.
                    _ => copy.await,
                }
            }
        }
    } else {
        copy.await
    };
    result.map(|_| ()).map_err(|error| {
        AppError::new("Port-forward data transfer failed", AppErrorKind::Io).with_source(error)
    })
}

async fn forward_connection(
    client: Client,
    target: PortForwardTarget,
    local_stream: TcpStream,
) -> Result<(), AppError> {
    forward_pod_connection(
        client,
        target.namespace,
        target.pod_name,
        target.pod_port,
        local_stream,
    )
    .await
}

async fn resolve_and_forward_connection(
    access: ClusterAccess,
    client: Client,
    request: ValidatedPortForwardRequest,
    local_stream: TcpStream,
    session_id: String,
    registry: PortForwardRegistry,
) -> Result<(), AppError> {
    registry.mark_status(&session_id, "reconnecting", None);
    let target = resolve_port_forward_target(&access, request).await?;
    registry.mark_resolved_target(&session_id, &target);
    registry.mark_status(&session_id, "connected", None);
    forward_connection(client, target, local_stream).await
}

pub(super) async fn start_pod_port_forward_in_registry(
    request: PortForwardRequest,
    registry: &PortForwardRegistry,
) -> Result<PortForwardSessionSummary, AppError> {
    let (request, access) = validate_request(&request)?;
    let listener = registry.bind(request.local_port).await?;
    let target = resolve_port_forward_target(&access, request).await?;
    let client = access.live_session_client(&target.cluster_context).await?;
    verify_pod_port_forward(client.clone(), &target).await?;

    let session_id = registry.next_id();
    let handle_request = ValidatedPortForwardRequest {
        cluster_context: target.cluster_context.clone(),
        kubeconfig_env_var: target.kubeconfig_env_var.clone(),
        kubeconfig_source_key: target.kubeconfig_source_key.clone(),
        kubeconfig_source_label: target.kubeconfig_source_label.clone(),
        namespace: target.namespace.clone(),
        target_kind: target.target_kind,
        target_name: target.target_name.clone(),
        remote_port: target.remote_port,
        local_port: Some(listener.local_port()),
    };
    let connection_session_id = session_id.clone();
    let connection_registry = registry.clone();
    let relay_connection = move |local_stream: TcpStream| {
        resolve_and_forward_connection(
            access.clone(),
            client.clone(),
            handle_request.clone(),
            local_stream,
            connection_session_id.clone(),
            connection_registry.clone(),
        )
    };
    registry.start(session_id, &target, listener, relay_connection)
}
