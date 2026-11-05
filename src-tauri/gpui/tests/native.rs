#[path = "native/argo.rs"]
mod argo;
#[path = "native/argo_gaps.rs"]
mod argo_gaps;
#[path = "native/argo_outcomes.rs"]
mod argo_outcomes;
#[path = "native/detail_tabs.rs"]
mod detail_tabs;
#[path = "native/kind_selection.rs"]
mod kind_selection;
#[path = "native/live_data.rs"]
mod live_data;
#[path = "native/page_reads.rs"]
mod page_reads;
#[path = "native/preferences.rs"]
mod preferences;
#[path = "native/preferences_migration.rs"]
mod preferences_migration;
#[path = "native/remote_connections.rs"]
mod remote_connections;
#[path = "native/session_controls.rs"]
mod session_controls;
#[path = "native/settings_integrations.rs"]
mod settings_integrations;
#[path = "native/shell.rs"]
mod shell;
#[path = "native/watch_burst.rs"]
mod watch_burst;
#[path = "native/workspaces.rs"]
mod workspaces;

use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, Bounds, TestAppContext, WindowBounds, WindowOptions, px, size};
use kubecove_gpui::{
    Inspector,
    backend::{
        Backend, KubeconfigSourcesSummary, LiveSessions, LogStream, LogTarget, Read, Scope,
        ScopeResult,
    },
    gitops::GitOpsInventory,
};
use kubecove_lib::models::{
    AppError, AppErrorKind, ArgoApplicationInspector, ArgoApplicationRef, ClusterContext,
    ClusterOperationResult, DeploymentRevision, HelmReleaseDetails, HelmReleaseList,
    HelmReleaseReconciliation, HelmReleaseSummary, IncidentCockpitSummary, NamespaceSummary,
    RbacAccessReviewRequest, RbacAccessReviewResult, RbacInspectionSummary, ResourceActionPreview,
    ResourceActionRequest, ResourceActionState, ResourceActionStateRequest, ResourceDetailsFull,
    ResourceEventSummary, ResourceSummary, ResourceTopology, YamlApplyPreview, YamlApplyRequest,
    YamlApplyResult, YamlViewMode,
};
use serde_json::json;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

#[derive(Default)]
struct Controlled {
    shell: shell::ShellControls,
    pages: page_reads::PageControls,
    argo: argo::ArgoControls,
    secret: adapters::SecretControls,
    sessions: session_controls::SessionControls,
    live: live_data::LiveControls,
    fail: AtomicBool,
    pending: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
    detail: detail_tabs::DetailControls,
    workspace: workspaces::WorkspaceControls,
}

const YAML: &str = "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: payments-api\n  namespace: payments\n";

fn resource(context: &str) -> ResourceSummary {
    serde_json::from_value(json!({"kind":"Deployment","cluster":context,"name":"payments-api","namespace":"payments","age":"1d"})).unwrap()
}

impl Backend for Controlled {
    fn load_preferences(&self) -> Result<kubecove_gpui::preferences::Document, AppError> {
        Ok(self.argo.document.lock().unwrap().clone())
    }
    fn save_preferences(
        &self,
        document: kubecove_gpui::preferences::Document,
    ) -> Result<kubecove_gpui::preferences::Document, AppError> {
        let count = self.argo.saves.fetch_add(1, Ordering::SeqCst) + 1;
        if self.argo.fail_save.load(Ordering::SeqCst) == count {
            return Err(AppError::new(
                "Fixture preferences write failed",
                AppErrorKind::ArgoConnection,
            ));
        }
        let document = self.shell.save(document)?;
        *self.argo.document.lock().unwrap() = document.clone();
        Ok(document)
    }
    fn disconnect_argo(&self, _: String) -> Read<()> {
        Box::pin(async { Ok(()) })
    }
    fn compare_argo(
        &self,
        request: kubecove_gpui::backend::ArgoInspection,
        resource: kubecove_lib::models::ArgoManagedResource,
    ) -> Read<kubecove_lib::models::ArgoResourceComparison> {
        assert_eq!(request.application.context.as_deref(), Some("mock-dev"));
        assert_eq!(request.application.uid.as_deref(), Some("original"));
        assert!(request.connection_id.is_some());
        assert_eq!(resource.namespace.as_deref(), Some("payments"));
        let gate = self.argo.comparison_gate.lock().unwrap().take();
        let fail = self.argo.comparison_fail.load(Ordering::SeqCst);
        Box::pin(async move {
            if let Some(gate) = gate {
                let _ = gate.await;
            }
            if fail {
                return Err(AppError::new(
                    "Fixture comparison refused",
                    AppErrorKind::Forbidden,
                ));
            }
            Ok(kubecove_lib::models::ArgoResourceComparison {
                target_state: Some(
                    json!({"kind": resource.kind, "metadata": {"name":"same-name"}}),
                ),
                resource,
                exact: Some(true),
                ..Default::default()
            })
        })
    }
    fn forget_argo(&self, profile: kubecove_lib::models::ArgoConnectionProfile) -> Read<()> {
        if self.argo.forget_fail.load(Ordering::SeqCst) {
            return Box::pin(async {
                Err(AppError::new(
                    "Fixture credential deletion failed",
                    AppErrorKind::ArgoConnection,
                ))
            });
        }
        self.argo.forgotten.lock().unwrap().push(profile.id);
        Box::pin(async { Ok(()) })
    }
    fn discover_argo(
        &self,
        _: kubecove_gpui::backend::ArgoTarget,
    ) -> Read<Vec<kubecove_lib::models::ArgoServerCapability>> {
        self.argo.discover()
    }
    fn preview_argo(
        &self,
        _: kubecove_gpui::backend::ArgoTarget,
    ) -> Read<kubecove_lib::models::ArgoTunnelTargetPreview> {
        self.argo.preview()
    }
    fn preflight_argo(
        &self,
        request: kubecove_lib::models::ArgoOperationRequest,
    ) -> Read<kubecove_lib::models::ArgoOperationPreflight> {
        self.argo.preflight(request)
    }
    fn run_argo(
        &self,
        confirmation: kubecove_lib::models::ArgoOperationConfirmation,
    ) -> Read<kubecove_lib::models::ArgoOperationResult> {
        self.argo.run(confirmation)
    }
    fn connect_argo(
        &self,
        request: kubecove_gpui::backend::ArgoConnect,
    ) -> Read<kubecove_lib::models::ArgoConnectionStatus> {
        self.argo.connect(request)
    }
    fn inspect_argo(
        &self,
        request: kubecove_gpui::backend::ArgoInspection,
    ) -> Read<ArgoApplicationInspector> {
        self.argo.inspect(request)
    }
    fn reveal_secret(&self, _: ResourceSummary, _: String) -> Read<String> {
        self.secret.reveal()
    }
    fn start_forward(
        &self,
        request: kubecove_lib::models::PortForwardRequest,
    ) -> Read<kubecove_lib::models::PortForwardSessionSummary> {
        self.sessions.forward(request)
    }
    fn stop_forward(&self, id: String) -> Read<bool> {
        self.sessions.stop_forward(id)
    }
    fn exec(
        &self,
        request: kubecove_lib::models::PodExecSessionRequest,
    ) -> Read<kubecove_gpui::backend::ExecStream> {
        self.sessions.exec(request)
    }
    fn watch(&self, target: kubecove_gpui::backend::WatchTarget) -> Read<LogStream> {
        self.live.open(target)
    }
    fn contexts(&self) -> Read<Vec<ClusterContext>> {
        Box::pin(async {
            Ok(vec![
                ClusterContext {
                    name: "mock-dev".into(),
                    is_current: true,
                },
                ClusterContext {
                    name: "mock-other".into(),
                    is_current: false,
                },
            ])
        })
    }
    fn resources(&self, scope: Scope, _: bool) -> Read<ScopeResult> {
        if let Some(read) = self.workspace.read(&scope) {
            return read;
        }
        let fail = self.fail.load(Ordering::SeqCst);
        let pending = self.pending.lock().unwrap().take();
        let pod = self.sessions.enabled.load(Ordering::SeqCst);
        let argo = self.argo.enabled.load(Ordering::SeqCst);
        let secret = self.secret.enabled.load(Ordering::SeqCst);
        Box::pin(async move {
            if let Some(pending) = pending {
                let _ = pending.await;
            }
            if fail {
                return Err(AppError::new(
                    "Resource read denied",
                    AppErrorKind::Forbidden,
                ));
            }
            Ok(ScopeResult {
                namespaces: vec![NamespaceSummary {
                    name: "payments".into(),
                    age: "1d".into(),
                    created_at: None,
                }],
                resources: if argo {
                    vec![
                        argo::resource(&scope.context, "payments-api"),
                        argo::resource(&scope.context, "other-app"),
                    ]
                } else {
                    vec![if secret {
                        ResourceSummary {
                            kind: "Secret".into(),
                            ..resource(&scope.context)
                        }
                    } else if pod {
                        ResourceSummary {
                            kind: "Pod".into(),
                            ..resource(&scope.context)
                        }
                    } else {
                        resource(&scope.context)
                    }]
                },
                warnings: vec![],
            })
        })
    }
    fn details(&self, summary: ResourceSummary) -> Read<ResourceDetailsFull> {
        let yaml = if summary.kind == "Secret" {
            adapters::SECRET_YAML
        } else {
            YAML
        };
        Box::pin(async move {
            Ok(ResourceDetailsFull {
                summary,
                yaml: yaml.into(),
                metadata: json!({"name":"payments-api","namespace":"payments"}),
                status: None,
            })
        })
    }
    fn topology(&self, scope: Scope) -> Read<ResourceTopology> {
        Box::pin(async move {
            Ok(serde_json::from_value(json!({
                "nodes": [{"id": MAP_NODE, "kind": "Deployment", "name": "payments-api", "namespace": "payments", "status": "Available", "health": "healthy", "selectable": true, "summary": resource(&scope.context)}],
                "edges": [],
                "warnings": [],
            }))
            .unwrap())
        })
    }
    fn gitops(&self, _: String) -> Read<GitOpsInventory> {
        let inventory = self
            .pages
            .inventory
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_default();
        self.pages
            .read("GitOps", Box::pin(async move { Ok(inventory) }))
    }
    fn cluster_resources(&self, context: String, _: bool) -> Read<ScopeResult> {
        Box::pin(async move {
            Ok(ScopeResult {
                namespaces: vec![],
                resources: vec![serde_json::from_value(json!({"kind":"Node","cluster":context,"name":"dev-control-plane","namespace":null,"age":"9d"})).unwrap()],
                warnings: vec![],
            })
        })
    }
    fn events(&self, _: ResourceSummary, _: Option<String>) -> Read<Vec<ResourceEventSummary>> {
        self.live.events()
    }
    fn incidents(&self, cluster: String) -> Read<IncidentCockpitSummary> {
        let item = |kind: &str, name: &str, severity: &str| {
            let status = if kind == "Pod" {
                "CrashLoopBackOff"
            } else {
                "Progressing"
            };
            json!({
                "resource": {"kind": kind, "cluster": cluster, "name": name, "namespace": "payments", "age": "1d", "status": status},
                "severity": severity, "state": "active", "warningEventCount": 0,
                "signals": [{"kind": kind, "label": format!("{name} signal"), "message": "needs attention", "source": "test", "state": "active"}],
            })
        };
        let summary = json!({
            "cluster": cluster, "generatedAt": "", "requestedScope": [], "warnings": [],
            "items": [item("Deployment", "payments-api", "attention"), item("Pod", "payments-api-0", "degraded")],
        });
        self.pages.read(
            "Incidents",
            Box::pin(async move { Ok(serde_json::from_value(summary).unwrap()) }),
        )
    }
    fn live_sessions(&self) -> Read<LiveSessions> {
        let session = |id: &str, context: &str, source: &str| {
            json!({
                "id": id, "clusterContext": context, "kubeconfigSourceKey": source,
                "namespace": "payments", "targetKind": "Pod", "targetName": "payments-api-0",
                "podName": "payments-api-0", "remotePort": 8080, "resolvedPodName": "payments-api-0",
                "resolvedPodPort": 8080, "localPort": 18080, "localAddress": "127.0.0.1",
                "localUrl": "http://127.0.0.1:18080", "status": "listening", "startedAt": "2026-06-29T10:00:00Z",
            })
        };
        let forwards = [
            session("visible", "mock-dev", "kubeconfigSource=active"),
            session("other-source", "mock-dev", "kubeconfigSource=other"),
            session("other-context", "mock-other", "kubeconfigSource=active"),
        ];
        Box::pin(async move {
            Ok(LiveSessions {
                port_forwards: forwards
                    .into_iter()
                    .map(|forward| serde_json::from_value(forward).unwrap())
                    .collect(),
                exec_sessions: vec![],
                source_key: Some("kubeconfigSource=active".into()),
                show_source_labels: false,
            })
        })
    }
    fn log_stream(&self, target: LogTarget) -> Read<LogStream> {
        self.detail.log_stream(target)
    }
    fn deployment_revisions(&self, _: ResourceSummary) -> Read<Vec<DeploymentRevision>> {
        Box::pin(async { Ok(vec![]) })
    }
    fn action_state(&self, request: ResourceActionStateRequest) -> Read<ResourceActionState> {
        self.detail.action_state(request)
    }
    fn preview_action(&self, request: ResourceActionRequest) -> Read<ResourceActionPreview> {
        self.detail.preview_action(request)
    }
    fn execute_action(&self, request: ResourceActionRequest) -> Read<ClusterOperationResult> {
        self.detail.execute_action(request)
    }
    fn resource_yaml(&self, resource: ResourceSummary, _: YamlViewMode) -> Read<String> {
        Box::pin(async move {
            Ok(if resource.kind == "Secret" {
                adapters::SECRET_YAML
            } else {
                YAML
            }
            .into())
        })
    }
    fn prepare_yaml_apply(&self, request: YamlApplyRequest) -> Read<YamlApplyPreview> {
        self.detail.prepare_yaml_apply(request)
    }
    fn apply_yaml(&self, request: YamlApplyRequest) -> Read<YamlApplyResult> {
        self.detail.apply_yaml(request)
    }
    fn namespaces(&self, _: String) -> Read<Vec<NamespaceSummary>> {
        self.shell.namespaces()
    }
    fn rbac(&self, _: String) -> Read<RbacInspectionSummary> {
        self.pages.read(
            "RBAC",
            Box::pin(async { Err(AppError::new("RBAC read denied", AppErrorKind::Forbidden)) }),
        )
    }
    fn review_rbac(&self, _: RbacAccessReviewRequest) -> Read<RbacAccessReviewResult> {
        Box::pin(async { Err(AppError::new("Review denied", AppErrorKind::Forbidden)) })
    }
    fn kubeconfig_sources(&self) -> Read<KubeconfigSourcesSummary> {
        Box::pin(async {
            Ok(serde_json::from_value(json!({"kubeconfigEnvVar":"KUBECONFIG","paths":[],"sourceKey":"default","sourceLabel":"KUBECONFIG","showSourceLabels":true,"warnings":[]})).unwrap())
        })
    }
    /// Like the production backend, which cannot persist kubeconfig settings.
    fn kubeconfig_writes_unavailable(&self) -> Option<&'static str> {
        Some(KUBECONFIG_WRITES)
    }
    fn set_kubeconfig_env_var(&self, _: String) -> Read<KubeconfigSourcesSummary> {
        unreachable!("the settings page must not write when writes are unavailable")
    }
    fn set_show_kubeconfig_source_labels(&self, _: bool) -> Read<KubeconfigSourcesSummary> {
        unreachable!("the settings page must not write when writes are unavailable")
    }

    fn helm_releases(&self, _: String) -> Read<HelmReleaseList> {
        self.pages.read(
            "Helm",
            Box::pin(async {
                Ok(HelmReleaseList {
                    releases: vec![],
                    warnings: vec![],
                })
            }),
        )
    }
    fn helm_release(&self, _: HelmReleaseSummary) -> Read<HelmReleaseDetails> {
        Box::pin(async { Err(AppError::new("No Helm release", AppErrorKind::NotFound)) })
    }
    fn helm_reconciliation(&self, _: HelmReleaseSummary) -> Read<HelmReleaseReconciliation> {
        Box::pin(async { Err(AppError::new("No Helm release", AppErrorKind::NotFound)) })
    }
    fn argo_inspector(&self, _: ArgoApplicationRef) -> Read<ArgoApplicationInspector> {
        Box::pin(async { Err(AppError::new("No Application", AppErrorKind::NotFound)) })
    }
}

const KUBECONFIG_WRITES: &str = "Kubeconfig settings cannot be saved here.";

const MAP_NODE: &str = "mock-dev:apps/v1:Deployment:payments:payments-api";

fn open(cx: &mut TestAppContext, backend: Arc<Controlled>) -> gpui_kit::AnyWindowHandle {
    cx.update(gpui_kit::init);
    let (window, _) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Default::default(),
                    // The web driver's viewport.
                    size: size(px(1440.), px(900.)),
                })),
                focus: false,
                show: false,
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Inspector::new(backend, window, cx)),
        )
        .unwrap()
    });
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("open-resources", cx);
    })
    .unwrap();
    cx.run_until_parked();
    window
}

/// Refresh lives in the Live updates menu, as in the web top bar.
fn refresh(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("live-status", cx);
        window.render_frame(cx);
        window.click("refresh", cx);
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_read_failure_is_visible_and_can_be_retried(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    backend.fail.store(true, Ordering::SeqCst);
    let handle = open(cx, backend.clone());
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("load-error").label(),
            Some("Resource read denied")
        );
        assert!(window.try_find("empty").is_none());
    })
    .unwrap();
    backend.fail.store(false, Ordering::SeqCst);
    refresh(cx, handle);
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("load-error").is_none());
        assert!(
            window
                .find("resource:Deployment:payments:payments-api")
                .visible()
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn kubeconfig_settings_the_backend_cannot_save_stay_disabled(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("open-settings", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("settings-section:Kubeconfig", cx);
        window.render_frame(cx);
        let toggle = "setting:Show kubeconfig source labels";
        assert_eq!(window.find(toggle).description(), Some(KUBECONFIG_WRITES));
        window.click(toggle, cx);
        window.click("kubeconfig-env-save", cx);
        window.click("kubeconfig-env-reset", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let toggle = window.find("setting:Show kubeconfig source labels");
        assert_eq!(toggle.checked(), Some(true));
    })
    .unwrap();
}

#[gpui_kit::test]
fn the_ownership_map_opens_resources(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    cx.update_window(handle, |_, window, cx| {
        // The first frame measures the graph viewport; the second fits it.
        window.render_frame(cx);
        window.render_frame(cx);
        assert_eq!(window.find("map-counts").label(), Some("1 nodes / 0 edges"));
        // An ownerless resource starts inside its collapsed kind group.
        assert!(window.try_find(format!("map-node:{MAP_NODE}")).is_none());
        window.click("map-group:Deployment", cx);
        window.render_frame(cx);
        window.click(format!("map-node:{MAP_NODE}"), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window
                .find("detail-context")
                .label()
                .unwrap()
                .contains("mock-dev")
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn switching_context_cancels_the_old_read(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    let (send, receive) = tokio::sync::oneshot::channel();
    *backend.pending.lock().unwrap() = Some(receive);
    refresh(cx, handle);
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.within("context").click("input", cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(
        send.send(()).is_err(),
        "old read must release its receiver on scope change"
    );
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window
                .find("scope-context")
                .label()
                .unwrap()
                .contains("mock-other")
        );
        window.click("resource:Deployment:payments:payments-api", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window
                .find("detail-context")
                .label()
                .unwrap()
                .contains("mock-other")
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_dirty_draft_requires_explicit_discard(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    cx.update_window(handle, |_, window, cx| {
        window.click("resource:Deployment:payments:payments-api", cx)
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("show-yaml", cx);
        window.render_frame(cx);
        window.click("edit-draft", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        detail_tabs::edit_draft(window, cx, "kind: Deployment\nmetadata:\n  name: changed\n");
        window.click("close-detail", cx);
        window.render_frame(cx);
        assert!(
            window.find("close-detail").visible(),
            "a dirty draft blocks closing"
        );
        window.click("discard-draft", cx);
        window.render_frame(cx);
        window.click("close-detail", cx);
        assert!(window.try_find("detail-identity").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn warning_events_feed_the_incident_summary_and_timeline(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("resource:Deployment:payments:payments-api", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("incident-signal:Warning events").label(),
            Some("1 warning reason · 4 repeats")
        );
        // Only warnings reach the timeline; BackOff gets its presentation title.
        assert!(window.find("incident-timeline").visible());
    })
    .unwrap();
}

/// Failure mode: sessions from another context or kubeconfig source leak into
/// the page. The browser mocks hold sessions from one source only.
#[gpui_kit::test]
fn port_forwards_stay_in_the_workspace_scope(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        // Both the header and page exclude sessions from another source or context.
        assert_eq!(
            window.find("live-sessions").label(),
            Some("Open live sessions (1 active)")
        );
        window.click("live-sessions", cx)
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("forward:visible").label(),
            Some("Pod/payments-api-0:8080")
        );
        assert!(window.try_find("forward:other-source").is_none());
        assert!(window.try_find("forward:other-context").is_none());
    })
    .unwrap();
}
#[path = "native/adapters.rs"]
mod adapters;
#[path = "native/mcp.rs"]
mod mcp;
