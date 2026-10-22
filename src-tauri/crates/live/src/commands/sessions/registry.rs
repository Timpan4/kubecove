use crate::commands::LiveSessionScope;
use crate::models::AppErrorKind;
use crate::models::{AppError, PortForwardSessionSummary};
use crate::runtime::JoinHandle;
use chrono::Utc;
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use super::target::PortForwardTarget;

pub(super) const LOCAL_ADDRESS: &str = "127.0.0.1";

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
    /// Stop all listeners when their host session expires.
    pub fn stop_all(&self) {
        self.stop_where(|_| true);
    }

    pub(super) fn session_id(&self) -> String {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        format!("port-forward-{id}")
    }

    pub(super) fn has_local_port(&self, local_port: u16) -> bool {
        self.state
            .lock()
            .expect("port-forward registry lock")
            .sessions
            .values()
            .any(|session| session.summary.local_port == local_port)
    }

    pub(super) fn insert(
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
                AppErrorKind::LiveSessionTargetUnavailable,
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

    pub(super) fn list(&self) -> Vec<PortForwardSessionSummary> {
        self.state
            .lock()
            .expect("port-forward registry lock")
            .sessions
            .values()
            .map(|session| session.summary.clone())
            .collect()
    }

    pub(super) fn mark_status(&self, session_id: &str, status: &str, last_error: Option<String>) {
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

    pub(super) fn mark_error(&self, session_id: &str, message: String) {
        self.mark_status(session_id, "error", Some(message));
    }

    pub(super) fn mark_resolved_target(&self, session_id: &str, target: &PortForwardTarget) {
        if let Some(session) = self
            .state
            .lock()
            .expect("port-forward registry lock")
            .sessions
            .get_mut(session_id)
        {
            session.summary.pod_name.clone_from(&target.pod_name);
            session
                .summary
                .resolved_pod_name
                .clone_from(&target.pod_name);
            session.summary.resolved_pod_port = target.pod_port;
        }
    }

    pub(super) fn stop(&self, session_id: &str) -> bool {
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

    pub(crate) fn stop_outside_scope(&self, scope: &LiveSessionScope) -> Vec<String> {
        self.stop_where(|summary| {
            !scope.contains(
                &summary.cluster_context,
                summary.kubeconfig_source_key.as_deref(),
            )
        })
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
                if let Some(handle) = session.handle {
                    handle.abort();
                }
                id
            })
            .collect()
    }

    #[cfg(test)]
    pub(super) fn insert_summary_for_test(&self, summary: PortForwardSessionSummary) {
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

pub(super) fn session_summary(
    id: String,
    target: &PortForwardTarget,
    local_port: u16,
) -> PortForwardSessionSummary {
    PortForwardSessionSummary {
        id,
        cluster_context: target.cluster_context.clone(),
        kubeconfig_env_var: target.kubeconfig_env_var.clone(),
        kubeconfig_source_key: target.kubeconfig_source_key.clone(),
        kubeconfig_source_label: target.kubeconfig_source_label.clone(),
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
