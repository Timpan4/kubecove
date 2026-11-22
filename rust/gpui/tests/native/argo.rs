//! Failure modes beyond shared mocks: failed login, cancelled connection,
//! stale inspection after an Application or profile switch, and failed submit.
use super::*;
use gpui_kit::test::TestQueryExt;
#[path = "../parity/scroll.rs"]
#[allow(dead_code)] // This suite uses reveal; the parity driver also uses seek.
pub(super) mod scroll;
use kubecove_gpui::backend::{ArgoConnect, ArgoInspection};
use kubecove_lib::models::{
    ArgoConnectionStatus, ArgoOperationConfirmation, ArgoOperationPreflight, ArgoOperationRequest,
    ArgoOperationResult,
};

#[derive(Default)]
pub struct ArgoControls {
    pub document: Mutex<kubecove_gpui::preferences::Document>,
    pub saves: std::sync::atomic::AtomicUsize,
    pub fail_save: std::sync::atomic::AtomicUsize,
    pub forget_fail: AtomicBool,
    pub forgotten: Mutex<Vec<String>>,
    pub discovery_fail: AtomicBool,
    pub reviews: Mutex<Vec<ArgoOperationRequest>>,
    pub lazy: AtomicBool,
    pub comparison_gate: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
    pub comparison_fail: AtomicBool,
    pub enabled: AtomicBool,
    fail: AtomicBool,
    connect_gate: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
    inspect_gate: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
    pub submitted: std::sync::atomic::AtomicUsize,
    pub run_gate: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
    pub accept: AtomicBool,
    pub observation: std::sync::atomic::AtomicUsize,
    pub repeated_revision: AtomicBool,
    pub connections: std::sync::atomic::AtomicUsize,
    /// The token each connect request carried.
    pub tokens: Mutex<Vec<Option<String>>>,
    preview_gate: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
    preview_fail: AtomicBool,
}
impl ArgoControls {
    pub fn preflight(&self, request: ArgoOperationRequest) -> Read<ArgoOperationPreflight> {
        self.reviews.lock().unwrap().push(request.clone());
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
        let gate = self.run_gate.lock().unwrap().take();
        let accept = self.accept.load(Ordering::SeqCst);
        Box::pin(async move {
            if let Some(gate) = gate {
                let _ = gate.await;
            }
            if accept {
                return Ok(ArgoOperationResult {
                    accepted: true,
                    message: "Fixture accepted".into(),
                    ..Default::default()
                });
            }
            Err(AppError::new(
                "Argo CD execution refused",
                AppErrorKind::Forbidden,
            ))
        })
    }
    pub fn connect(&self, request: ArgoConnect) -> Read<ArgoConnectionStatus> {
        self.connections.fetch_add(1, Ordering::SeqCst);
        self.tokens.lock().unwrap().push(request.token.clone());
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
    pub fn discover(&self) -> Read<Vec<kubecove_lib::models::ArgoServerCapability>> {
        if self.discovery_fail.load(Ordering::SeqCst) {
            return Box::pin(async {
                Err(AppError::new(
                    "Fixture discovery unavailable",
                    AppErrorKind::ArgoConnection,
                ))
            });
        }
        Box::pin(async {
            Ok(vec![kubecove_lib::models::ArgoServerCapability {
                id: "fixture-service".into(),
                name: "argocd-server".into(),
                namespace: Some("argocd".into()),
                endpoint: Some(kubecove_lib::models::ArgoServerEndpoint::ServiceTunnel {
                    namespace: "argocd".into(),
                    service_name: "argocd-server".into(),
                    service_port: 443,
                    scheme: "https".into(),
                    root_path: None,
                    tls_server_name: None,
                }),
                ..Default::default()
            }])
        })
    }
    pub fn preview(&self) -> Read<kubecove_lib::models::ArgoTunnelTargetPreview> {
        let gate = self.preview_gate.lock().unwrap().take();
        let fail = self.preview_fail.load(Ordering::SeqCst);
        Box::pin(async move {
            if let Some(gate) = gate {
                let _ = gate.await;
            }
            if fail {
                return Err(AppError::new(
                    "Fixture tunnel preview refused",
                    AppErrorKind::ArgoConnection,
                ));
            }
            Ok(kubecove_lib::models::ArgoTunnelTargetPreview {
                namespace: "argocd".into(),
                service_name: "argocd-server".into(),
                pod_name: "fixture-pod".into(),
                certificate_sha256: Some("3f".repeat(32)),
            })
        })
    }
    pub fn inspect(&self, request: ArgoInspection) -> Read<ArgoApplicationInspector> {
        let lazy = self.lazy.load(Ordering::SeqCst);
        let gate = self.inspect_gate.lock().unwrap().take();
        let observation = self.observation.load(Ordering::SeqCst);
        let repeated_revision = self.repeated_revision.load(Ordering::SeqCst);
        Box::pin(async move {
            if let Some(gate) = gate {
                let _ = gate.await;
            }
            if observation == 3 {
                return Err(AppError::new(
                    "Fixture observation unavailable",
                    AppErrorKind::ArgoConnection,
                ));
            }
            let mut application = request.application;
            application.uid = Some(
                if observation == 2 {
                    "replacement"
                } else {
                    "original"
                }
                .into(),
            );
            // Argo CD wrote the Application after the tab opened.
            if observation == 6 {
                application.resource_version = Some("fresh".into());
            }
            Ok(ArgoApplicationInspector {
                application,
                resources: if lazy {
                    ["Deployment", "ConfigMap"]
                        .map(|kind| kubecove_lib::models::ArgoManagedResource {
                            group: Some(if kind == "Deployment" { "apps" } else { "" }.into()),
                            version: Some("v1".into()),
                            kind: Some(kind.into()),
                            namespace: Some("payments".into()),
                            name: Some("same-name".into()),
                            status: Some("OutOfSync".into()),
                            ..Default::default()
                        })
                        .into()
                } else {
                    vec![]
                },
                history: if repeated_revision {
                    serde_json::from_value(json!([
                        {"id":18,"revision":"same-commit","revisions":[],"deployedAt":null,"initiatedBy":null,"source":null,"sources":[]},
                        {"id":17,"revision":"same-commit","revisions":[],"deployedAt":null,"initiatedBy":null,"source":null,"sources":[]}
                    ])).unwrap()
                } else {
                    Vec::new()
                },
                operation_state: match observation {
                    1 => Some(
                        json!({"phase":"Failed","startedAt":"new-operation","message":"Fixture sync failed"}),
                    ),
                    5 => Some(json!({"phase":"Failed","message":"Fixture sync failed"})),
                    _ => None,
                },
                operation_requested: observation == 4,
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
pub(super) fn step(
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
pub(super) fn open_argo(
    cx: &mut TestAppContext,
    backend: Arc<Controlled>,
) -> gpui_kit::AnyWindowHandle {
    backend.argo.enabled.store(true, Ordering::SeqCst);
    let handle = open(cx, backend);
    step(cx, handle, |w, cx| {
        w.click("resource:Application:payments:payments-api:mock-dev", cx)
    });
    step(cx, handle, |w, cx| w.click("show-argo", cx));
    handle
}
pub(super) fn connect(cx: &mut TestAppContext, h: gpui_kit::AnyWindowHandle) {
    connect_with(cx, h, false);
}
pub(super) fn connect_remembered(cx: &mut TestAppContext, h: gpui_kit::AnyWindowHandle) {
    connect_with(cx, h, true);
}
fn connect_with(cx: &mut TestAppContext, h: gpui_kit::AnyWindowHandle, remember: bool) {
    step(cx, h, |w, cx| {
        w.click("argo-connect-open", cx);
        w.render_frame(cx);
        // The dialog selects a discovered in-cluster server first, as the web does.
        w.click("argo-external", cx);
        w.render_frame(cx);
        scroll::reveal(w, "detail-body", "argo-url", cx);
        w.click("argo-url", cx);
        w.input("https://argocd.example.test", cx);
        scroll::reveal(w, "detail-body", "argo-token", cx);
        w.click("argo-token", cx);
        w.input("native-fixture-token", cx);
        if remember {
            scroll::reveal(w, "detail-body", "argo-remember", cx);
            w.click("argo-remember", cx);
        }
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
            "resource:Application:payments:other-app:mock-dev"
        ));
        w.click("resource:Application:payments:other-app:mock-dev", cx)
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

pub(super) fn preview(
    cx: &mut TestAppContext,
    backend: Arc<Controlled>,
) -> gpui_kit::AnyWindowHandle {
    let h = open_argo(cx, backend);
    step(cx, h, |w, cx| {
        w.click("argo-connect-open", cx);
    });
    step(cx, h, |w, cx| {
        scroll::reveal(w, "detail-body", "argo-server:fixture-service", cx);
        w.click("argo-server:fixture-service", cx);
        w.render_frame(cx);
        scroll::reveal(w, "detail-body", "argo-token", cx);
        w.click("argo-token", cx);
        w.input("native-fixture-token", cx);
    });
    step(cx, h, |w, cx| {
        scroll::reveal(w, "detail-body", "argo-connect-submit", cx);
        w.click("argo-connect-submit", cx);
    });
    h
}
#[gpui_kit::test]
fn tunnel_preview_cancellation_sends_no_credential(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let (send, gate) = tokio::sync::oneshot::channel();
    *backend.argo.preview_gate.lock().unwrap() = Some(gate);
    let h = preview(cx, backend.clone());
    step(cx, h, |w, cx| {
        scroll::reveal(w, "detail-body", "argo-connect-cancel", cx);
        w.click("argo-connect-cancel", cx);
    });
    assert!(send.send(()).is_err());
    assert_eq!(backend.argo.connections.load(Ordering::SeqCst), 0);
}
#[gpui_kit::test]
fn refused_tunnel_preview_sends_no_credential(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    backend.argo.preview_fail.store(true, Ordering::SeqCst);
    let h = preview(cx, backend.clone());
    step(cx, h, |w, _| {
        assert!(
            w.find("argo-connection-error")
                .label()
                .unwrap()
                .contains("Fixture tunnel preview refused")
        );
    });
    assert_eq!(backend.argo.connections.load(Ordering::SeqCst), 0);
}
