//! ADR 0003 and 0005 command adapter. Registries belong to this host, never another window.
use super::{NativeBackend, Read};
use kubecove_lib::{
    commands,
    models::*,
    runtime::{Channel, ChannelClosed, State},
};
use tokio::sync::mpsc;

pub(super) fn unavailable<T: Send + 'static>() -> Read<T> {
    Box::pin(async {
        Err(AppError::new(
            "Live session controls unavailable",
            AppErrorKind::UnsupportedOperation,
        ))
    })
}

/// Owns the output channel and stops the process when its view or a stale start is dropped.
pub struct ExecStream {
    pub summary: PodExecSessionSummary,
    pub messages: mpsc::UnboundedReceiver<PodExecSessionMessage>,
    _owner: Box<dyn Send>,
}

impl ExecStream {
    pub fn new(
        summary: PodExecSessionSummary,
        messages: mpsc::UnboundedReceiver<PodExecSessionMessage>,
        owner: impl Send + 'static,
    ) -> Self {
        Self {
            summary,
            messages,
            _owner: Box::new(owner),
        }
    }
}

struct StopExec {
    id: String,
    registry: commands::PodExecRegistry,
    runtime: tokio::runtime::Handle,
}
impl Drop for StopExec {
    fn drop(&mut self) {
        let id = self.id.clone();
        let registry = self.registry.clone();
        self.runtime.spawn(async move {
            let _ = commands::stop_pod_exec_session(id, State::new(&registry)).await;
        });
    }
}

pub(super) fn start_forward(
    backend: &NativeBackend,
    request: PortForwardRequest,
) -> Read<PortForwardSessionSummary> {
    let registry = backend.port_forwards.clone();
    backend
        .read(async move { commands::start_pod_port_forward(request, State::new(&registry)).await })
}
pub(super) fn stop_forward(backend: &NativeBackend, id: String) -> Read<bool> {
    let registry = backend.port_forwards.clone();
    backend.read(async move { commands::stop_port_forward(id, State::new(&registry)).await })
}
pub(super) fn exec(backend: &NativeBackend, request: PodExecSessionRequest) -> Read<ExecStream> {
    let registry = backend.exec_sessions.clone();
    let runtime = backend
        .runtime
        .as_ref()
        .expect("runtime exists until drop")
        .handle()
        .clone();
    backend.read(async move {
        let (sender, messages) = mpsc::unbounded_channel();
        let channel = Channel::new(move |message| sender.send(message).map_err(|_| ChannelClosed));
        let summary =
            commands::start_pod_exec_session(request, channel, State::new(&registry)).await?;
        let owner = StopExec {
            id: summary.id.clone(),
            registry,
            runtime,
        };
        Ok(ExecStream::new(summary, messages, owner))
    })
}
pub(super) fn input(backend: &NativeBackend, id: String, data: String) -> Read<bool> {
    let registry = backend.exec_sessions.clone();
    backend
        .read(async move { commands::write_pod_exec_stdin(id, data, State::new(&registry)).await })
}
pub(super) fn resize(backend: &NativeBackend, id: String, size: PodExecTerminalSize) -> Read<bool> {
    let registry = backend.exec_sessions.clone();
    backend.read(async move {
        commands::resize_pod_exec_terminal(id, size, State::new(&registry)).await
    })
}
pub(super) fn stop_exec(backend: &NativeBackend, id: String) -> Read<bool> {
    let registry = backend.exec_sessions.clone();
    backend.read(async move { commands::stop_pod_exec_session(id, State::new(&registry)).await })
}
pub(super) fn cleanup(
    backend: &NativeBackend,
    request: LiveSessionCleanupRequest,
) -> Read<LiveSessionCleanupResult> {
    let forwards = backend.port_forwards.clone();
    let execs = backend.exec_sessions.clone();
    backend.read(async move {
        commands::stop_live_sessions_outside_scope(
            request,
            State::new(&forwards),
            State::new(&execs),
        )
        .await
    })
}
