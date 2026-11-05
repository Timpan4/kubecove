//! Failure modes beyond shared mocks: failed login, cancelled connection,
//! stale inspection after an Application or profile switch, and failed submit.
use super::*;
use gpui_kit::test::TestQueryExt;
#[path = "../parity/scroll.rs"]
mod scroll;
use kubecove_gpui::backend::{ArgoConnect, ArgoInspection};
use kubecove_lib::models::{
    ArgoConnectionStatus, ArgoOperationConfirmation, ArgoOperationPreflight, ArgoOperationRequest,
    ArgoOperationResult,
};

#[derive(Default)]
pub struct ArgoControls {
    pub enabled: AtomicBool,
    fail: AtomicBool,
    connect_gate: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
    inspect_gate: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
    pub submitted: std::sync::atomic::AtomicUsize,
}
impl ArgoControls {
    pub fn preflight(&self, request: ArgoOperationRequest) -> Read<ArgoOperationPreflight> {
        Box::pin(async move {
            Ok(ArgoOperationPreflight {
                allowed: true,
                transport: request.transport.clone(),
                action: request.action.clone(),
                session_id: Some("native-review".into()),
                reviewed_request: Some(request),
                ..Default::default()
            })
        })
    }
    pub fn run(&self, confirmation: ArgoOperationConfirmation) -> Read<ArgoOperationResult> {
        assert_eq!(confirmation.session_id, "native-review");
        self.submitted.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {
            Err(AppError::new(
                "Argo CD execution refused",
                AppErrorKind::Forbidden,
            ))
        })
    }
    pub fn connect(&self, request: ArgoConnect) -> Read<ArgoConnectionStatus> {
        let fail = self.fail.load(Ordering::SeqCst);
        let gate = self.connect_gate.lock().unwrap().take();
        Box::pin(async move {
            if let Some(gate) = gate {
                let _ = gate.await;
            }
            if fail {
                return Err(AppError::new(
                    "Argo CD login refused",
                    AppErrorKind::Forbidden,
                ));
            }
            Ok(ArgoConnectionStatus {
                profile: Some(request.profile),
                connected: true,
                ..Default::default()
            })
        })
    }
    pub fn inspect(&self, request: ArgoInspection) -> Read<ArgoApplicationInspector> {
        let gate = self.inspect_gate.lock().unwrap().take();
        Box::pin(async move {
            if let Some(gate) = gate {
                let _ = gate.await;
            }
            Ok(ArgoApplicationInspector {
                application: request.application,
                transport: if request.connection_id.is_some() {
                    "connected"
                } else {
                    "kubernetes"
                }
                .into(),
                ..Default::default()
            })
        })
    }
}
pub fn resource(context: &str, name: &str) -> ResourceSummary {
    ResourceSummary {
        kind: "Application".into(),
        name: name.into(),
        api_version: Some("argoproj.io/v1alpha1".into()),
        dynamic: Some(true),
        ..super::resource(context)
    }
}
fn step(
    cx: &mut TestAppContext,
    handle: gpui_kit::AnyWindowHandle,
    act: impl FnOnce(&mut gpui_kit::Window, &mut gpui_kit::App),
) {
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        act(w, cx);
    })
    .unwrap();
    cx.run_until_parked();
}
fn open_argo(cx: &mut TestAppContext, backend: Arc<Controlled>) -> gpui_kit::AnyWindowHandle {
    backend.argo.enabled.store(true, Ordering::SeqCst);
    let handle = open(cx, backend);
    step(cx, handle, |w, cx| {
        w.click("resource:Application:payments:payments-api", cx)
    });
    step(cx, handle, |w, cx| w.click("show-argo", cx));
    handle
}
fn connect(cx: &mut TestAppContext, h: gpui_kit::AnyWindowHandle) {
    step(cx, h, |w, cx| {
        w.click("argo-connect-open", cx);
        w.render_frame(cx);
        scroll::reveal(w, "detail-body", "argo-url", cx);
        w.click("argo-url", cx);
        w.input("https://argocd.example.test", cx);
    });
    step(cx, h, |w, cx| {
        scroll::reveal(w, "detail-body", "argo-connect-submit", cx);
        w.click("argo-connect-submit", cx);
    });
}
#[gpui_kit::test]
fn connection_failure_stays_visible(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    backend.argo.fail.store(true, Ordering::SeqCst);
    let h = open_argo(cx, backend);
    connect(cx, h);
    step(cx, h, |w, _| {
        assert!(w.elements().iter().any(|e| {
            e.label()
                .is_some_and(|l| l.contains("Argo CD login refused"))
        }))
    });
}
#[gpui_kit::test]
fn cancelling_connection_drops_the_read(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let (send, gate) = tokio::sync::oneshot::channel();
    *backend.argo.connect_gate.lock().unwrap() = Some(gate);
    let h = open_argo(cx, backend);
    connect(cx, h);
    step(cx, h, |w, cx| {
        scroll::reveal(w, "detail-body", "argo-connect-cancel", cx);
        w.click("argo-connect-cancel", cx);
    });
    assert!(
        send.send(()).is_err(),
        "cancel must drop the pending connection"
    );
}
#[gpui_kit::test]
fn changing_application_drops_old_inspection(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let (send, gate) = tokio::sync::oneshot::channel();
    *backend.argo.inspect_gate.lock().unwrap() = Some(gate);
    let h = open_argo(cx, backend);
    step(cx, h, |w, cx| {
        assert!(scroll::seek(
            w,
            cx,
            "search",
            "resource:Application:payments:other-app"
        ));
        w.click("resource:Application:payments:other-app", cx)
    });
    assert!(
        send.send(()).is_err(),
        "old Application read must be cancelled"
    );
    step(cx, h, |w, _| {
        assert!(
            w.find("detail-identity")
                .label()
                .is_some_and(|s| s.contains("other-app"))
        )
    });
}
#[gpui_kit::test]
fn changing_profile_drops_old_inspection(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let (send, gate) = tokio::sync::oneshot::channel();
    *backend.argo.inspect_gate.lock().unwrap() = Some(gate);
    let h = open_argo(cx, backend);
    connect(cx, h);
    assert!(
        send.send(()).is_err(),
        "new profile must drop the Kubernetes inspection"
    );
}

#[gpui_kit::test]
fn failed_execution_consumes_the_confirmation(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let h = open_argo(cx, backend.clone());
    step(cx, h, |w, cx| {
        scroll::reveal(w, "detail-body", "argo-operation:Refresh", cx);
        w.click("argo-operation:Refresh", cx);
    });
    step(cx, h, |w, cx| {
        scroll::reveal(w, "detail-body", "argo-review-confirm", cx);
        w.click("argo-review-confirm", cx);
    });
    step(cx, h, |w, _| {
        assert!(
            w.find("argo-operation-result")
                .label()
                .unwrap()
                .contains("Argo CD execution refused")
        );
        assert!(w.try_find("argo-review-confirm").is_none());
    });
    assert_eq!(backend.argo.submitted.load(Ordering::SeqCst), 1);
}
