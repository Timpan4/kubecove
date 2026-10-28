//! The Port Forward Lifecycle. It binds each forward's loopback listener,
//! relays the connections it accepts, lists the forward, and stops it on
//! request, outside the workspace scope, or when its host session ends.
//! Kubernetes forwards and desktop tunnels supply only how one accepted
//! connection is relayed.

use crate::commands::LiveSessionScope;
use crate::models::AppErrorKind;
use crate::models::{AppError, PortForwardSessionSummary};
use crate::runtime::JoinHandle;
use chrono::Utc;
use std::{
    collections::BTreeMap,
    future::Future,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use tokio::net::{TcpListener, TcpStream};

use super::{
    relay::{bind_loopback, spawn_relay, RelayEvent},
    target::PortForwardTarget,
};

pub(super) const LOCAL_ADDRESS: &str = "127.0.0.1";

/// A forward's bound loopback listener, before it relays connections.
pub struct ForwardListener {
    listener: TcpListener,
    local_port: u16,
}

impl ForwardListener {
    pub fn local_port(&self) -> u16 {
        self.local_port
    }
}

struct PortForwardSession {
    summary: PortForwardSessionSummary,
    handle: JoinHandle<()>,
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
    pub fn next_id(&self) -> String {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        format!("port-forward-{id}")
    }

    /// Binds a new forward's listener on `127.0.0.1`, on the requested port or
    /// on a free one.
    pub async fn bind(&self, local_port: Option<u16>) -> Result<ForwardListener, AppError> {
        if let Some(local_port) = local_port {
            self.ensure_unforwarded(local_port)?;
        }
        let (listener, local_port) = bind_loopback(local_port).await.map_err(|err| {
            AppError::new(
                format!("local port unavailable: {err}"),
                AppErrorKind::LiveSessionTargetUnavailable,
            )
            .with_source(err)
        })?;
        self.ensure_unforwarded(local_port)?;
        Ok(ForwardListener {
            listener,
            local_port,
        })
    }

    /// Relays the connections `listener` accepts through `relay_connection`
    /// and lists the forward under `id` until it stops.
    pub fn start<C, F>(
        &self,
        id: String,
        target: &PortForwardTarget,
        listener: ForwardListener,
        relay_connection: C,
    ) -> Result<PortForwardSessionSummary, AppError>
    where
        C: Fn(TcpStream) -> F + Send + 'static,
        F: Future<Output = Result<(), AppError>> + Send + 'static,
    {
        let summary = session_summary(id.clone(), target, listener.local_port);
        let mut state = self.state.lock().expect("port-forward registry lock");
        let conflict = if state.sessions.contains_key(&summary.id) {
            Some(format!("port-forward {} is already running", summary.id))
        } else if is_forwarded(&state, summary.local_port) {
            Some(format!(
                "local port {} is already forwarded",
                summary.local_port
            ))
        } else {
            None
        };
        if let Some(message) = conflict {
            return Err(AppError::new(
                message,
                AppErrorKind::LiveSessionTargetUnavailable,
            ));
        }
        // Admit before spawning so a rejected session never accepts a connection.
        let handle = spawn_relay(listener.listener, relay_connection, self.relay_events(id));
        state.sessions.insert(
            summary.id.clone(),
            PortForwardSession {
                summary: summary.clone(),
                handle,
            },
        );
        Ok(summary)
    }

    pub fn list(&self) -> Vec<PortForwardSessionSummary> {
        self.state
            .lock()
            .expect("port-forward registry lock")
            .sessions
            .values()
            .map(|session| session.summary.clone())
            .collect()
    }

    pub fn stop(&self, session_id: &str) -> bool {
        let session = self
            .state
            .lock()
            .expect("port-forward registry lock")
            .sessions
            .remove(session_id);
        let Some(session) = session else {
            return false;
        };
        session.handle.abort();
        true
    }

    pub fn stop_outside_scope(&self, scope: &LiveSessionScope) -> Vec<String> {
        self.stop_where(|summary| {
            !scope.contains(
                &summary.cluster_context,
                summary.kubeconfig_source_key.as_deref(),
            )
        })
    }

    /// Stop all listeners when their host session expires.
    pub fn stop_all(&self) -> Vec<String> {
        self.stop_where(|_| true)
    }

    /// Updates a forward's listed summary; a forward that already stopped is left alone.
    fn update_summary(
        &self,
        session_id: &str,
        update: impl FnOnce(&mut PortForwardSessionSummary),
    ) {
        if let Some(session) = self
            .state
            .lock()
            .expect("port-forward registry lock")
            .sessions
            .get_mut(session_id)
        {
            update(&mut session.summary);
        }
    }

    pub(super) fn mark_status(&self, session_id: &str, status: &str, last_error: Option<String>) {
        self.update_summary(session_id, |summary| {
            summary.status = status.to_string();
            summary.last_error = last_error;
        });
    }

    pub(super) fn mark_resolved_target(&self, session_id: &str, target: &PortForwardTarget) {
        self.update_summary(session_id, |summary| {
            summary.pod_name.clone_from(&target.pod_name);
            summary.resolved_pod_name.clone_from(&target.pod_name);
            summary.resolved_pod_port = target.pod_port;
        });
    }

    fn mark_error(&self, session_id: &str, message: String) {
        self.mark_status(session_id, "error", Some(message));
    }

    /// Reports a forward's relay events in its listed status.
    fn relay_events(&self, session_id: String) -> impl FnMut(RelayEvent) + Send + 'static {
        let registry = self.clone();
        move |event| match event {
            RelayEvent::AcceptRetry { attempt, error } => registry.mark_status(
                &session_id,
                "listening",
                Some(format!("accept retry {attempt}: {error}")),
            ),
            RelayEvent::AcceptFailed(error) => registry.mark_error(&session_id, error.to_string()),
            RelayEvent::ConnectionClosed { idle: true } => {
                registry.mark_status(&session_id, "listening", None);
            }
            RelayEvent::ConnectionClosed { idle: false } => {}
            RelayEvent::ConnectionFailed(error) => registry.mark_error(&session_id, error.message),
            RelayEvent::ConnectionPanicked(error) => {
                registry.mark_error(&session_id, error.to_string());
            }
        }
    }

    fn ensure_unforwarded(&self, local_port: u16) -> Result<(), AppError> {
        if is_forwarded(
            &self.state.lock().expect("port-forward registry lock"),
            local_port,
        ) {
            return Err(AppError::new(
                format!("local port {local_port} is already forwarded"),
                AppErrorKind::LiveSessionTargetUnavailable,
            ));
        }
        Ok(())
    }

    fn stop_where(&self, mut stop: impl FnMut(&PortForwardSessionSummary) -> bool) -> Vec<String> {
        let stopped = self
            .state
            .lock()
            .expect("port-forward registry lock")
            .sessions
            .extract_if(.., |_, session| stop(&session.summary))
            .collect::<Vec<_>>();
        stopped
            .into_iter()
            .map(|(id, session)| {
                session.handle.abort();
                id
            })
            .collect()
    }
}

fn is_forwarded(state: &PortForwardState, local_port: u16) -> bool {
    state
        .sessions
        .values()
        .any(|session| session.summary.local_port == local_port)
}

pub(super) fn session_summary(
    id: String,
    target: &PortForwardTarget,
    local_port: u16,
) -> PortForwardSessionSummary {
    let request = &target.request;
    PortForwardSessionSummary {
        id,
        cluster_context: request.cluster_context.clone(),
        kubeconfig_env_var: request.kubeconfig_env_var.clone(),
        kubeconfig_source_key: request.kubeconfig_source_key.clone(),
        kubeconfig_source_label: request.kubeconfig_source_label.clone(),
        namespace: request.namespace.clone(),
        target_kind: request.target_kind.as_str().to_string(),
        target_name: request.target_name.clone(),
        pod_name: target.pod_name.clone(),
        remote_port: request.remote_port,
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
