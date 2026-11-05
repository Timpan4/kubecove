//! Recorded command responses exported from the browser mocks. Never use a cluster.
use kubecove_gpui::backend::{ExecStream, Read};
use kubecove_lib::models::*;
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

#[derive(Deserialize, Default)]
pub struct SessionControls {
    forwards: Vec<Forward>,
    execs: Vec<Exec>,
    #[serde(default)]
    replay: std::collections::HashMap<String, ForwardReplay>,
    #[serde(skip)]
    state: Arc<Mutex<Running>>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForwardReplay {
    status: PortForwardSessionStatus,
    last_error: Option<String>,
}
#[derive(Deserialize)]
struct Forward {
    request: PortForwardRequest,
    summary: PortForwardSessionSummary,
}
#[derive(Deserialize)]
struct Exec {
    request: PodExecSessionRequest,
    summary: PodExecSessionSummary,
    messages: Vec<PodExecSessionMessage>,
}
#[derive(Default)]
struct Running {
    serial: usize,
    forwards: Vec<PortForwardSessionSummary>,
    stopped: Vec<String>,
    execs: Vec<(
        PodExecSessionSummary,
        mpsc::UnboundedSender<PodExecSessionMessage>,
    )>,
}
fn refused<T: Send + 'static>() -> Read<T> {
    Box::pin(async {
        Err(AppError::new(
            "Request differs from controlled session fixture",
            AppErrorKind::Validation,
        ))
    })
}
struct Owner {
    id: String,
    state: Arc<Mutex<Running>>,
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.state
            .lock()
            .unwrap()
            .execs
            .retain(|(s, _)| s.id != self.id);
    }
}
impl SessionControls {
    pub fn terminal_output(&self, container: &str, data: &str) {
        let state = self.state.lock().unwrap();
        for (summary, sender) in &state.execs {
            if summary.container.as_deref() == Some(container) {
                sender
                    .send(PodExecSessionMessage::Output {
                        session_id: summary.id.clone(),
                        stream: "stdout".into(),
                        data: data.into(),
                    })
                    .unwrap();
            }
        }
    }
    pub fn cleanup(
        &self,
        forwards: &[PortForwardSessionSummary],
        execs: &[PodExecSessionSummary],
        request: LiveSessionCleanupRequest,
    ) -> Read<LiveSessionCleanupResult> {
        let stopped_port_forward_ids: Vec<_> = self
            .list(forwards)
            .into_iter()
            .filter(|s| {
                !request
                    .allowed_cluster_contexts
                    .contains(&s.cluster_context)
                    || s.kubeconfig_source_key.as_ref() != Some(&request.kubeconfig_source_key)
            })
            .map(|s| s.id)
            .collect();
        let stopped_pod_exec_ids: Vec<_> = self
            .execs(execs)
            .into_iter()
            .filter(|s| {
                !request
                    .allowed_cluster_contexts
                    .contains(&s.cluster_context)
                    || s.kubeconfig_source_key.as_ref() != Some(&request.kubeconfig_source_key)
            })
            .map(|s| s.id)
            .collect();
        let mut state = self.state.lock().unwrap();
        state
            .forwards
            .retain(|s| !stopped_port_forward_ids.contains(&s.id));
        for (summary, sender) in &state.execs {
            if stopped_pod_exec_ids.contains(&summary.id) {
                let _ = sender.send(PodExecSessionMessage::Stopped {
                    session_id: summary.id.clone(),
                });
            }
        }
        state
            .execs
            .retain(|(s, _)| !stopped_pod_exec_ids.contains(&s.id));
        state.stopped.extend(
            stopped_port_forward_ids
                .iter()
                .chain(&stopped_pod_exec_ids)
                .cloned(),
        );
        let result = LiveSessionCleanupResult {
            stopped_port_forwards: stopped_port_forward_ids.len(),
            stopped_pod_exec_sessions: stopped_pod_exec_ids.len(),
            stopped_port_forward_ids,
            stopped_pod_exec_ids,
        };
        Box::pin(async move { Ok(result) })
    }
    pub fn replay(&self, script: &str) {
        let mut state = self.state.lock().unwrap();
        if script == "foreign-output" {
            for (_, sender) in &state.execs {
                let _ = sender.send(PodExecSessionMessage::Output {
                    session_id: "foreign-session".into(),
                    stream: "stdout".into(),
                    data: "FOREIGN SESSION OUTPUT".into(),
                });
            }
        } else {
            let update = self
                .replay
                .get(script)
                .expect("exported forward replay state");
            for session in &mut state.forwards {
                session.status = update.status;
                if update.last_error.is_some() {
                    session.last_error = update.last_error.clone();
                }
            }
        }
    }
    pub fn start(&self, request: PortForwardRequest) -> Read<PortForwardSessionSummary> {
        let Some(record) = self.forwards.iter().find(|r| {
            serde_json::to_value(&r.request).unwrap() == serde_json::to_value(&request).unwrap()
        }) else {
            return refused();
        };
        let mut summary = record.summary.clone();
        let mut state = self.state.lock().unwrap();
        state.serial += 1;
        summary.id = format!("native-forward-{}", state.serial);
        state.forwards.push(summary.clone());
        Box::pin(async move { Ok(summary) })
    }
    pub fn stop(&self, id: String) -> Read<bool> {
        let mut state = self.state.lock().unwrap();
        state.stopped.push(id.clone());
        state.forwards.retain(|s| s.id != id);
        Box::pin(async { Ok(true) })
    }
    pub fn list(&self, base: &[PortForwardSessionSummary]) -> Vec<PortForwardSessionSummary> {
        let state = self.state.lock().unwrap();
        base.iter()
            .filter(|s| !state.stopped.contains(&s.id))
            .chain(&state.forwards)
            .cloned()
            .collect()
    }
    pub fn execs(&self, base: &[PodExecSessionSummary]) -> Vec<PodExecSessionSummary> {
        let state = self.state.lock().unwrap();
        base.iter()
            .filter(|s| !state.stopped.contains(&s.id))
            .cloned()
            .chain(state.execs.iter().map(|(s, _)| s.clone()))
            .collect()
    }
    pub fn exec(&self, request: PodExecSessionRequest) -> Read<ExecStream> {
        let Some(record) = self.execs.iter().find(|r| {
            let mut expected = r.request.clone();
            expected.terminal_size = request.terminal_size.clone();
            (1..=500).contains(&request.terminal_size.cols)
                && (1..=500).contains(&request.terminal_size.rows)
                && serde_json::to_value(expected).unwrap()
                    == serde_json::to_value(&request).unwrap()
        }) else {
            return refused();
        };
        let mut summary = record.summary.clone();
        let mut state = self.state.lock().unwrap();
        state.serial += 1;
        summary.id = format!("native-exec-{}", state.serial);
        let (sender, messages) = mpsc::unbounded_channel();
        for message in &record.messages {
            let mut message = message.clone();
            match &mut message {
                PodExecSessionMessage::Started {
                    session_id,
                    summary: started,
                } => {
                    *session_id = summary.id.clone();
                    started.id = summary.id.clone();
                }
                PodExecSessionMessage::Status { session_id, .. }
                | PodExecSessionMessage::Output { session_id, .. }
                | PodExecSessionMessage::Error { session_id, .. }
                | PodExecSessionMessage::Exited { session_id, .. }
                | PodExecSessionMessage::Stopped { session_id } => *session_id = summary.id.clone(),
            }
            sender.send(message).unwrap();
        }
        state.execs.push((summary.clone(), sender));
        let owner = Owner {
            id: summary.id.clone(),
            state: self.state.clone(),
        };
        Box::pin(async move { Ok(ExecStream::new(summary, messages, owner)) })
    }
    pub fn input(&self, id: String, data: String) -> Read<bool> {
        let state = self.state.lock().unwrap();
        let Some((_, sender)) = state.execs.iter().find(|(s, _)| s.id == id) else {
            return refused();
        };
        sender
            .send(PodExecSessionMessage::Output {
                session_id: id,
                stream: "stdout".into(),
                data,
            })
            .unwrap();
        Box::pin(async { Ok(true) })
    }
    pub fn stop_exec(&self, id: String) -> Read<bool> {
        let mut state = self.state.lock().unwrap();
        state.stopped.push(id.clone());
        state.execs.retain(|(s, _)| s.id != id);
        Box::pin(async { Ok(true) })
    }
    pub fn resize(&self, id: String, size: PodExecTerminalSize) -> Read<bool> {
        // Same terminal-size contract as the browser's podExecHelpers.ts.
        if !(1..=500).contains(&size.cols) || !(1..=500).contains(&size.rows) {
            return refused();
        }
        let mut state = self.state.lock().unwrap();
        let Some((session, _)) = state.execs.iter_mut().find(|(s, _)| s.id == id) else {
            return Box::pin(async { Ok(false) });
        };
        session.terminal_cols = size.cols;
        session.terminal_rows = size.rows;
        Box::pin(async { Ok(true) })
    }
}
