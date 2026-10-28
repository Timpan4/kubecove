use crate::models::AppErrorKind;
use crate::{
    commands::{
        sessions::{
            relay::{bind_loopback, spawn_relay, RelayEvent},
            runner::forward_pod_connection,
            service::resolve_service_target,
        },
        ClusterAccess,
    },
    models::{AppError, ArgoConfirmedTarget},
    runtime::JoinHandle,
};
use k8s_openapi::api::core::v1::Pod;
use kube::{api::Api, Client};
use tokio::net::TcpStream;

pub(crate) struct ArgoServiceTunnel {
    local_port: u16,
    pod_name: String,
    relay: JoinHandle<()>,
}

impl ArgoServiceTunnel {
    pub(crate) async fn start(
        access: &ClusterAccess,
        cluster_context: &str,
        namespace: String,
        service_name: String,
        service_port: u16,
    ) -> Result<Self, AppError> {
        let client = access.live_session_client(cluster_context).await?;
        let target =
            resolve_service_target(client.clone(), &namespace, &service_name, service_port).await?;
        let pod_name = target.pod_name.clone();
        verify_port_forward(
            client.clone(),
            &target.namespace,
            &target.pod_name,
            target.pod_port,
        )
        .await?;
        let (listener, local_port) = bind_loopback(None).await.map_err(|error| {
            AppError::new(
                format!("local tunnel unavailable: {error}"),
                AppErrorKind::ArgoTunnel,
            )
            .with_source(error)
        })?;
        let route = ServiceRoute {
            namespace,
            service_name,
            service_port,
        };
        let relay = spawn_relay(
            listener,
            move |stream| relay_service_connection(client.clone(), route.clone(), stream),
            log_relay_event,
        );
        Ok(Self {
            local_port,
            pod_name,
            relay,
        })
    }

    pub(crate) fn local_port(&self) -> u16 {
        self.local_port
    }

    pub(crate) fn pod_name(&self) -> &str {
        &self.pod_name
    }

    pub(crate) fn close(&self) {
        self.relay.abort();
    }

    /// A tunnel whose relay holds `held` until the tunnel closes, so its
    /// receiver resolves when the relay stops.
    #[cfg(test)]
    pub(crate) fn test_tunnel(held: tokio::sync::oneshot::Sender<()>) -> Self {
        Self {
            local_port: 0,
            pod_name: String::new(),
            relay: crate::runtime::spawn(async move {
                let _held = held;
                std::future::pending::<()>().await;
            }),
        }
    }
}

impl Drop for ArgoServiceTunnel {
    fn drop(&mut self) {
        self.close();
    }
}

/// Credentials may only cross a tunnel whose resolved target is what the user confirmed.
///
/// A confirmation without a Pod is accepted only when `service_level_allowed`, which the
/// caller grants for a remembered credential saved after a full confirmation (ADR 0017).
pub(crate) fn verify_confirmed_target<'a>(
    confirmed: Option<&'a ArgoConfirmedTarget>,
    namespace: &str,
    service_name: &str,
    pod_name: &str,
    service_level_allowed: bool,
) -> Result<&'a ArgoConfirmedTarget, AppError> {
    let unconfirmed = || {
        AppError::new(
            "confirm the Argo CD Service tunnel target before sending credentials",
            AppErrorKind::ArgoConnection,
        )
    };
    let confirmed = confirmed.ok_or_else(unconfirmed)?;
    let pod_matches = match confirmed.pod_name.as_deref() {
        Some(confirmed_pod) => confirmed_pod == pod_name,
        None if service_level_allowed => true,
        None => return Err(unconfirmed()),
    };
    if confirmed.namespace == namespace && confirmed.service_name == service_name && pod_matches {
        return Ok(confirmed);
    }
    Err(AppError::new(
        "Argo CD Service tunnel target changed since it was confirmed; refresh discovery and confirm again",
        AppErrorKind::ArgoConnection,
    ))
}

#[derive(Clone)]
struct ServiceRoute {
    namespace: String,
    service_name: String,
    service_port: u16,
}

async fn verify_port_forward(
    client: Client,
    namespace: &str,
    pod_name: &str,
    pod_port: u16,
) -> Result<(), AppError> {
    let pods: Api<Pod> = Api::namespaced(client, namespace);
    let forwarder = pods
        .portforward(pod_name, &[pod_port])
        .await
        .map_err(port_forward_error)?;
    forwarder.abort();
    Ok(())
}

fn port_forward_error(error: kube::Error) -> AppError {
    let message = error.to_string();
    if crate::models::kube_error_kind(&error) == AppErrorKind::Forbidden {
        return AppError::new(
            "Kubernetes RBAC denies pods/portforward for this Argo CD Service",
            AppErrorKind::ArgoTunnelForbidden,
        )
        .with_source(error);
    }
    AppError::new(
        format!("Argo CD Service tunnel could not open: {message}"),
        AppErrorKind::ArgoTunnel,
    )
    .with_source(error)
}

/// Relays one connection to the Pod the confirmed Service resolves to now, so the tunnel
/// follows rollouts. HTTPS stays bound to the pinned certificate across Pods.
async fn relay_service_connection(
    client: Client,
    route: ServiceRoute,
    stream: TcpStream,
) -> Result<(), AppError> {
    let target = resolve_service_target(
        client.clone(),
        &route.namespace,
        &route.service_name,
        route.service_port,
    )
    .await?;
    forward_pod_connection(
        client,
        target.namespace,
        target.pod_name,
        target.pod_port,
        stream,
    )
    .await
}

fn log_relay_event(event: RelayEvent) {
    match event {
        RelayEvent::AcceptRetry { attempt, error } => {
            eprintln!("[kubecove:backend] Argo Service tunnel accept retry {attempt}: {error}");
        }
        RelayEvent::AcceptFailed(error) => {
            eprintln!("[kubecove:backend] Argo Service tunnel accept failed: {error}");
        }
        RelayEvent::ConnectionFailed(error) => {
            eprintln!("[kubecove:backend] Argo Service tunnel forward failed: {error}");
        }
        RelayEvent::ConnectionPanicked(error) => {
            eprintln!("[kubecove:backend] Argo Service tunnel task failed: {error}");
        }
        RelayEvent::ConnectionClosed { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_require_the_exact_confirmed_target() {
        let confirmed = |namespace: &str, service: &str, pod: Option<&str>| ArgoConfirmedTarget {
            namespace: namespace.into(),
            service_name: service.into(),
            pod_name: pod.map(Into::into),
            certificate_sha256: None,
        };
        let check = |target: Option<ArgoConfirmedTarget>| {
            verify_confirmed_target(target.as_ref(), "argocd", "argocd-server", "pod-a", false)
                .is_ok()
        };
        assert!(check(Some(confirmed(
            "argocd",
            "argocd-server",
            Some("pod-a")
        ))));
        assert!(!check(None));
        assert!(!check(Some(confirmed(
            "evil",
            "argocd-server",
            Some("pod-a")
        ))));
        assert!(!check(Some(confirmed(
            "argocd",
            "argo-cd-argocd-server",
            Some("pod-a")
        ))));
        assert!(!check(Some(confirmed(
            "argocd",
            "argocd-server",
            Some("pod-b")
        ))));
        assert!(!check(Some(confirmed("argocd", "argocd-server", None))));
    }

    #[test]
    fn remembered_reconnects_confirm_only_the_service() {
        let service_level = |namespace: &str, service: &str| ArgoConfirmedTarget {
            namespace: namespace.into(),
            service_name: service.into(),
            pod_name: None,
            certificate_sha256: None,
        };
        let check = |target: ArgoConfirmedTarget| {
            verify_confirmed_target(Some(&target), "argocd", "argocd-server", "pod-b", true).is_ok()
        };
        assert!(check(service_level("argocd", "argocd-server")));
        assert!(!check(service_level("evil", "argocd-server")));
        assert!(!check(service_level("argocd", "argo-cd-argocd-server")));
        assert!(!check(ArgoConfirmedTarget {
            pod_name: Some("pod-a".into()),
            ..service_level("argocd", "argocd-server")
        }));
    }

    #[test]
    fn forbidden_port_forward_is_actionable_and_redacted() {
        let error = port_forward_error(kube::Error::Api(Box::new(kube::core::Status {
            code: 403,
            reason: "Forbidden".into(),
            message: "pods/portforward is forbidden".into(),
            ..Default::default()
        })));
        assert_eq!(error.kind, AppErrorKind::ArgoTunnelForbidden);
        assert!(!error.message.contains("token"));
    }
}
