//! Failures the mock acceptance scenarios cannot schedule: denied/failed opens,
//! cancellation during open, and output after its detail is gone.
use super::{Controlled, open};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext};
use kubecove_gpui::backend::{ExecStream, Read};
use kubecove_lib::models::*;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

#[gpui_kit::test]
fn header_sessions_dismiss_without_stopping_sessions(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("live-sessions", cx);
        window.render_frame(cx);
        assert!(window.try_find("live-session-popover").is_some());
        window.press("escape", cx);
        window.render_frame(cx);
        assert!(window.try_find("live-session-popover").is_none());
        window.click("live-sessions", cx);
        window.render_frame(cx);
        window.click("live-sessions", cx);
        window.render_frame(cx);
        assert!(window.try_find("live-session-popover").is_none());
        window.click("live-sessions", cx);
        window.render_frame(cx);
        window.click("collapse-sidebar", cx);
        window.render_frame(cx);
        assert!(window.try_find("live-session-popover").is_none());
    })
    .unwrap();
    assert!(backend.sessions.stopped_forwards.lock().unwrap().is_empty());
}

#[derive(Default)]
pub struct SessionControls {
    pub enabled: AtomicBool,
    pub transport: AtomicBool,
    pub pending: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
    pub forward_pending: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
    pub stopped_forwards: Arc<Mutex<Vec<String>>>,
    /// Output the next exec session prints, then keeps open as a shell does.
    pub exec_output: Mutex<Option<String>>,
}
impl SessionControls {
    pub fn forward(&self, request: PortForwardRequest) -> Read<PortForwardSessionSummary> {
        let pending = self.forward_pending.lock().unwrap().take();
        Box::pin(async move {
            let Some(pending) = pending else {
                return Err(AppError::new(
                    "pods/portforward is forbidden",
                    AppErrorKind::Forbidden,
                ));
            };
            let _ = pending.await;
            Ok(serde_json::from_value(serde_json::json!({
                "id": "delayed-forward", "clusterContext": request.cluster_context,
                "kubeconfigSourceKey": "kubeconfigSource=active", "namespace": request.namespace,
                "targetKind": request.target_kind, "targetName": request.target_name,
                "podName": "payments-api", "remotePort": request.remote_port,
                "resolvedPodName": "payments-api", "resolvedPodPort": request.remote_port,
                "localPort": request.local_port.unwrap(), "localAddress": "127.0.0.1",
                "localUrl": "http://127.0.0.1:19090", "status": "listening", "startedAt": ""
            }))
            .unwrap())
        })
    }
    pub fn stop_forward(&self, id: String) -> Read<bool> {
        self.stopped_forwards.lock().unwrap().push(id);
        Box::pin(async { Ok(true) })
    }
    pub fn exec(&self, _: PodExecSessionRequest) -> Read<ExecStream> {
        let transport = self.transport.load(Ordering::SeqCst);
        let pending = self.pending.lock().unwrap().take();
        let output = self.exec_output.lock().unwrap().take();
        Box::pin(async move {
            if let Some(pending) = pending {
                let _ = pending.await;
            }
            if let Some(output) = output {
                return Ok(scripted_exec(output));
            }
            Err(AppError::new(
                if transport {
                    "Exec transport disconnected"
                } else {
                    "pods/exec is forbidden"
                },
                if transport {
                    AppErrorKind::Cluster
                } else {
                    AppErrorKind::Forbidden
                },
            ))
        })
    }
}
/// A session that prints `output` and then stays open.
fn scripted_exec(output: String) -> ExecStream {
    let summary: PodExecSessionSummary = serde_json::from_value(serde_json::json!({
        "id": "scripted-exec", "clusterContext": "mock-dev", "namespace": "payments",
        "podName": "payments-api", "command": ["/bin/sh"], "stdin": true, "tty": true,
        "terminalCols": 80, "terminalRows": 24, "status": "running", "startedAt": ""
    }))
    .unwrap();
    let (send, receive) = tokio::sync::mpsc::unbounded_channel();
    let _ = send.send(PodExecSessionMessage::Output {
        session_id: "scripted-exec".into(),
        stream: "stdout".into(),
        data: output,
    });
    // The sender owns the stream's lifetime, so the session stays open.
    ExecStream::new(summary, receive, send)
}
pub(super) fn pod(cx: &mut TestAppContext, backend: Arc<Controlled>) -> gpui_kit::AnyWindowHandle {
    backend.sessions.enabled.store(true, Ordering::SeqCst);
    let handle = open(cx, backend);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("resource:Pod:payments:payments-api:mock-dev", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("show-exec", cx);
        window.render_frame(cx);
        window.click("exec-start", cx);
    })
    .unwrap();
    cx.run_until_parked();
    handle
}
#[gpui_kit::test]
fn refused_exec_is_visible(cx: &mut TestAppContext) {
    let handle = pod(cx, Arc::new(Controlled::default()));
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        // The typed kind classifies it, as `FriendlyError.svelte` does.
        assert_eq!(
            w.find("exec-error").label(),
            Some("KubeCove does not have permission for this")
        );
    })
    .unwrap();
}
#[gpui_kit::test]
fn exec_transport_error_is_visible(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    backend.sessions.transport.store(true, Ordering::SeqCst);
    let handle = pod(cx, backend);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(
            w.find("exec-error").label(),
            Some("KubeCove could not reach the Kubernetes API")
        );
    })
    .unwrap();
}
#[gpui_kit::test]
fn closing_a_starting_exec_cancels_it(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let (send, receive) = tokio::sync::oneshot::channel();
    *backend.sessions.pending.lock().unwrap() = Some(receive);
    let handle = pod(cx, backend);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.click("exec-close:0", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(
        send.send(()).is_err(),
        "closed view releases the pending start"
    );
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find("exec-error").is_none());
    })
    .unwrap();
}

fn start_forward(cx: &mut TestAppContext, backend: Arc<Controlled>) -> gpui_kit::AnyWindowHandle {
    backend.sessions.enabled.store(true, Ordering::SeqCst);
    let handle = open(cx, backend);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.click("resource:Pod:payments:payments-api:mock-dev", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.click("show-forward", cx);
        w.render_frame(cx);
        w.click("forward-remote", cx);
        w.input("9090", cx);
        w.click("forward-local", cx);
        w.input("19090", cx);
        w.click("forward-start", cx);
    })
    .unwrap();
    cx.run_until_parked();
    handle
}
#[gpui_kit::test]
fn a_forward_that_finishes_after_context_change_is_stopped(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let (send, receive) = tokio::sync::oneshot::channel();
    *backend.sessions.forward_pending.lock().unwrap() = Some(receive);
    let handle = start_forward(cx, backend.clone());
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.within("context").click("input", cx);
        w.press("down", cx);
        w.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    send.send(()).unwrap();
    cx.run_until_parked();
    assert_eq!(
        *backend.sessions.stopped_forwards.lock().unwrap(),
        ["delayed-forward"]
    );
}
/// Failure mode: Start with an invalid port while a start runs replaces the
/// running state, so the next Start overlaps the first.
#[gpui_kit::test]
fn an_invalid_start_keeps_the_running_start_guarded(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let (send, receive) = tokio::sync::oneshot::channel();
    *backend.sessions.forward_pending.lock().unwrap() = Some(receive);
    let handle = start_forward(cx, backend);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        // "190901" is past the TCP port range.
        w.click("forward-local", cx);
        w.input("1", cx);
        w.click("forward-start", cx);
        w.render_frame(cx);
        assert!(
            w.try_find("forward-error").is_none(),
            "an invalid Start replaced the running start"
        );
    })
    .unwrap();
    let _ = send.send(());
    cx.run_until_parked();
}
#[gpui_kit::test]
fn a_refused_forward_is_visible(cx: &mut TestAppContext) {
    let handle = start_forward(cx, Arc::new(Controlled::default()));
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(
            w.find("forward-error").label(),
            Some("pods/portforward is forbidden")
        );
    })
    .unwrap();
}
#[gpui_kit::test]
fn confirming_context_change_cancels_a_pending_exec(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let (send, receive) = tokio::sync::oneshot::channel();
    *backend.sessions.pending.lock().unwrap() = Some(receive);
    let handle = pod(cx, backend);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.within("context").click("input", cx);
        w.press("down", cx);
        w.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.click("exec-leave-confirm", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(send.send(()).is_err());
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find("exec-terminal").is_none());
        assert!(w.try_find("exec-error").is_none());
    })
    .unwrap();
}
/// Before the fix the form kept the deleted preset's index, so Save changes
/// panicked on the removed last entry or overwrote the one that moved into it.
#[gpui_kit::test]
fn deleting_the_edited_preset_closes_its_form(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.click("nav:Port Forwards", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.click("save-forward", cx);
        w.render_frame(cx);
        for (id, text) in [
            ("preset-namespace", "payments"),
            ("preset-service", "payments-api"),
            ("preset-remote", "80"),
        ] {
            w.click(id, cx);
            w.input(text, cx);
        }
        w.click("preset-save", cx);
        w.render_frame(cx);
        w.click("edit-preset:0", cx);
        w.render_frame(cx);
        assert!(w.try_find("preset-save").is_some());
        w.click("remove-preset:0", cx);
        w.render_frame(cx);
        assert!(w.try_find("preset:0").is_none());
        assert!(w.try_find("preset-save").is_none());
    })
    .unwrap();
}
/// Opening the launcher closes the workspace but keeps `scope.context`. Before
/// the fix a forward that finished after that exit cleanup kept running.
#[gpui_kit::test]
fn a_forward_that_finishes_after_the_workspace_closes_is_stopped(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let (send, receive) = tokio::sync::oneshot::channel();
    *backend.sessions.forward_pending.lock().unwrap() = Some(receive);
    let handle = start_forward(cx, backend.clone());
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.click("header-workspaces", cx);
    })
    .unwrap();
    cx.run_until_parked();
    send.send(()).unwrap();
    cx.run_until_parked();
    assert_eq!(
        *backend.sessions.stopped_forwards.lock().unwrap(),
        ["delayed-forward"]
    );
}
