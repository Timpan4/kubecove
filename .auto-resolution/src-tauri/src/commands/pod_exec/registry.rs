use crate::models::{AppError, PodExecSessionSummary, PodExecTerminalSize};
use chrono::Utc;
use std::{
    collections::{BTreeMap, HashSet},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use tauri::async_runtime::JoinHandle;
use tokio::sync::mpsc;

use super::validation::ValidatedPodExecRequest;

pub(super) enum ExecCommand {
    Stdin(Vec<u8>),
    Resize(PodExecTerminalSize),
}

struct PodExecSession {
    summary: PodExecSessionSummary,
    commands: mpsc::UnboundedSender<ExecCommand>,
    handle: Option<JoinHandle<()>>,
}

#[derive(Default)]
struct PodExecState {
    sessions: BTreeMap<String, PodExecSession>,
}

#[derive(Clone, Default)]
pub struct PodExecRegistry {
    next_id: Arc<AtomicU64>,
    state: Arc<Mutex<PodExecState>>,
}

impl PodExecRegistry {
    pub(super) fn session_id(&self) -> String {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        format!("pod-exec-{id}")
    }

    pub(super) fn insert(
        &self,
        summary: PodExecSessionSummary,
        commands: mpsc::UnboundedSender<ExecCommand>,
    ) -> PodExecSessionSummary {
        self.state
            .lock()
            .expect("pod exec registry lock")
            .sessions
            .insert(
                summary.id.clone(),
                PodExecSession {
                    summary: summary.clone(),
                    commands,
                    handle: None,
                },
            );
        summary
    }

    pub(super) fn set_handle(&self, session_id: &str, handle: JoinHandle<()>) {
        let mut handle = Some(handle);
        if let Some(session) = self
            .state
            .lock()
            .expect("pod exec registry lock")
            .sessions
            .get_mut(session_id)
        {
            session.handle = handle.take();
        }
        if let Some(handle) = handle {
            handle.abort();
        }
    }

    pub(super) fn list(&self) -> Vec<PodExecSessionSummary> {
        self.state
            .lock()
            .expect("pod exec registry lock")
            .sessions
            .values()
            .map(|session| session.summary.clone())
            .collect()
    }

    pub(super) fn mark_status(&self, session_id: &str, status: &str, last_error: Option<String>) {
        if let Some(session) = self
            .state
            .lock()
            .expect("pod exec registry lock")
            .sessions
            .get_mut(session_id)
        {
            session.summary.status = status.to_string();
            session.summary.last_error = last_error;
        }
    }

    pub(super) fn mark_running(&self, session_id: &str) {
        self.mark_status(session_id, "running", None);
    }

    pub(super) fn mark_error(&self, session_id: &str, message: String) {
        self.mark_status(session_id, "error", Some(message));
    }

    pub(super) fn mark_terminal_size(&self, session_id: &str, size: PodExecTerminalSize) {
        if let Some(session) = self
            .state
            .lock()
            .expect("pod exec registry lock")
            .sessions
            .get_mut(session_id)
        {
            session.summary.terminal_cols = size.cols;
            session.summary.terminal_rows = size.rows;
        }
    }

    pub(super) fn mark_exited(&self, session_id: &str, exit_code: Option<i32>) {
        if let Some(session) = self
            .state
            .lock()
            .expect("pod exec registry lock")
            .sessions
            .get_mut(session_id)
        {
            session.summary.status = "exited".to_string();
            session.summary.finished_at = Some(Utc::now().to_rfc3339());
            session.summary.exit_code = exit_code;
        }
    }

    pub(super) fn send_command(
        &self,
        session_id: &str,
        command: ExecCommand,
    ) -> Result<(), AppError> {
        let commands = self
            .state
            .lock()
            .expect("pod exec registry lock")
            .sessions
            .get(session_id)
            .map(|session| session.commands.clone())
            .ok_or_else(|| AppError::new("exec session was not found", "session"))?;
        commands
            .send(command)
            .map_err(|_| AppError::new("exec session is no longer accepting input", "session"))
    }

    pub(super) fn stop(&self, session_id: &str) -> bool {
        let session = self
            .state
            .lock()
            .expect("pod exec registry lock")
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

    pub(crate) fn stop_outside_scope(
        &self,
        allowed_cluster_contexts: &HashSet<String>,
        kubeconfig_source_key: &str,
    ) -> Vec<String> {
        let mut stopped = Vec::new();
        let mut handles = Vec::new();
        {
            let mut state = self.state.lock().expect("pod exec registry lock");
            let session_ids = state
                .sessions
                .iter()
                .filter_map(|(id, session)| {
                    let in_context =
                        allowed_cluster_contexts.contains(&session.summary.cluster_context);
                    let in_source = session.summary.kubeconfig_source_key.as_deref()
                        == Some(kubeconfig_source_key);
                    (!in_context || !in_source).then(|| id.clone())
                })
                .collect::<Vec<_>>();
            for session_id in session_ids {
                if let Some(mut session) = state.sessions.remove(&session_id) {
                    if let Some(handle) = session.handle.take() {
                        handles.push(handle);
                    }
                    stopped.push(session_id);
                }
            }
        }
        for handle in handles {
            handle.abort();
        }
        stopped
    }

    #[cfg(test)]
    pub(super) fn insert_summary_for_test(&self, summary: PodExecSessionSummary) {
        let (commands, _rx) = mpsc::unbounded_channel();
        self.state
            .lock()
            .expect("pod exec registry lock")
            .sessions
            .insert(
                summary.id.clone(),
                PodExecSession {
                    summary,
                    commands,
                    handle: None,
                },
            );
    }
}

pub(super) fn session_summary(
    id: String,
    request: &ValidatedPodExecRequest,
) -> PodExecSessionSummary {
    PodExecSessionSummary {
        id,
        cluster_context: request.cluster_context.clone(),
        kubeconfig_env_var: request.kubeconfig_env_var.clone(),
        kubeconfig_source_key: request.kubeconfig_source_key.clone(),
        kubeconfig_source_label: request.kubeconfig_source_label.clone(),
        namespace: request.namespace.clone(),
        pod_name: request.pod_name.clone(),
        container: request.container.clone(),
        command: request.command.clone(),
        stdin: request.stdin,
        tty: request.tty,
        terminal_cols: request.terminal_size.cols,
        terminal_rows: request.terminal_size.rows,
        status: "starting".to_string(),
        started_at: Utc::now().to_rfc3339(),
        finished_at: None,
        exit_code: None,
        last_error: None,
    }
}
