#[path = "native/argo.rs"]
mod argo;
#[path = "native/argo_gaps.rs"]
mod argo_gaps;
#[path = "native/argo_outcomes.rs"]
mod argo_outcomes;
#[path = "native/default_read_retry.rs"]
mod default_read_retry;
#[path = "native/detail_tabs.rs"]
mod detail_tabs;
#[path = "native/exec_context_menu.rs"]
mod exec_context_menu;
#[path = "native/inspection.rs"]
mod inspection;
#[path = "native/kind_selection.rs"]
mod kind_selection;
#[path = "native/live_data.rs"]
mod live_data;
#[path = "native/map_live.rs"]
mod map_live;
#[path = "native/page_reads.rs"]
mod page_reads;
#[path = "native/preferences.rs"]
mod preferences;
#[path = "native/preferences_migration.rs"]
mod preferences_migration;
#[path = "native/presentation.rs"]
mod presentation;
#[path = "native/read_retry.rs"]
mod read_retry;
#[path = "native/remote_connections.rs"]
mod remote_connections;
#[path = "native/scope_load.rs"]
mod scope_load;
#[path = "native/session_controls.rs"]
mod session_controls;
#[path = "native/settings_integrations.rs"]
mod settings_integrations;
#[path = "native/shell.rs"]
mod shell;
#[path = "native/updates.rs"]
mod updates;
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
    HelmReleaseReconciliation, HelmReleaseSummary, IncidentCockpitSummary,
    KubernetesYamlLintResult, NamespaceSummary, RbacAccessReviewRequest, RbacAccessReviewResult,
    RbacInspectionSummary, ResourceActionPreview, ResourceActionRequest, ResourceActionState,
    ResourceActionStateRequest, ResourceDetailsFull, ResourceEventSummary, ResourceSummary,
    ResourceTopology, YamlApplyPreview, YamlApplyRequest, YamlApplyResult, YamlViewMode,
};
use serde_json::json;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

#[derive(Default)]
struct Controlled {
    shell: shell::ShellControls,
    retry: read_retry::RetryControls,
    default_retry: read_retry::RetryControls,
    pages: page_reads::PageControls,
    argo: argo::ArgoControls,
    secret: adapters::SecretControls,
    sessions: session_controls::SessionControls,
    live: live_data::LiveControls,
    map_live: map_live::MapControls,
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
    fn import_workspace(&self) -> Read<Option<String>> {
        self.workspace.import()
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
    fn watch_start_version(
        &self,
        _scope: Scope,
        _key: kubecove_lib::models::WatchResourceKey,
    ) -> Option<Read<String>> {
        self.live.start_version()
    }
    fn contexts(&self, _kubeconfig_source_key: Option<String>) -> Read<Vec<ClusterContext>> {
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
        self.live.reads.lock().unwrap().push("list");
        self.default_retry.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(kind) = self.default_retry.errors.lock().unwrap().pop_front() {
            return Box::pin(async move { Err(AppError::new("scripted resource failure", kind)) });
        }
        if let Some(read) = self.workspace.read(&scope) {
            return read;
        }
        let fail = self.fail.load(Ordering::SeqCst);
        let pending = self.pending.lock().unwrap().take();
        let pod = self.sessions.enabled.load(Ordering::SeqCst);
        let argo = self.argo.enabled.load(Ordering::SeqCst);
        let secret = self.secret.enabled.load(Ordering::SeqCst);
        let map_rows = self.map_live.rows(&scope.context);
        let versions = self.live.list_versions.clone();
        let returned = self.live.returned_lists.clone();
        Box::pin(async move {
            if let Some(pending) = pending {
                let _ = pending.await;
            }
            let versions = versions.lock().unwrap().clone();
            returned.lock().unwrap().push(versions.clone());
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
                resources: if let Some(rows) = map_rows {
                    rows
                } else if argo {
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
                versions,
                listed: vec![],
            })
        })
    }
    fn details(&self, summary: ResourceSummary) -> Read<ResourceDetailsFull> {
        let yaml = if summary.kind == "Secret" {
            adapters::SECRET_YAML
        } else {
            YAML
        };
        let gate = self.detail.details_gate.lock().unwrap().take();
        Box::pin(async move {
            if let Some(gate) = gate {
                let _ = gate.await;
            }
            Ok(ResourceDetailsFull {
                summary,
                yaml: yaml.into(),
                metadata: json!({"name":"payments-api","namespace":"payments"}),
                status: None,
            })
        })
    }
    fn topology(&self, scope: Scope) -> Read<Arc<ResourceTopology>> {
        self.retry.calls.fetch_add(1, Ordering::SeqCst);
        let error = self.retry.errors.lock().unwrap().pop_front();
        let gate = self.retry.gate.lock().unwrap().take();
        let topology = self.map_live.topology(&scope.context);
        Box::pin(async move {
            if let Some(gate) = gate {
                let _ = gate.await;
            }
            if let Some(kind) = error {
                return Err(AppError::new("scripted topology failure", kind));
            }
            if let Some(topology) = topology {
                return Ok(topology);
            }
            Ok(Arc::new(serde_json::from_value(json!({
                "nodes": [{"id": MAP_NODE, "kind": "Deployment", "name": "payments-api", "namespace": "payments", "status": "Available", "health": "healthy", "selectable": true, "summary": resource(&scope.context)}],
                "edges": [],
                "warnings": [],
            }))
            .unwrap()))
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
                versions: Default::default(),
                listed: vec![],
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
        self.detail.deployment_revisions()
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
    fn resource_yaml(&self, resource: ResourceSummary, mode: YamlViewMode) -> Read<String> {
        self.detail.resource_yaml(resource, mode)
    }
    fn prepare_yaml_apply(&self, request: YamlApplyRequest) -> Read<YamlApplyPreview> {
        self.detail.prepare_yaml_apply(request)
    }
    fn lint_yaml(&self, _: YamlApplyRequest) -> Read<KubernetesYamlLintResult> {
        // Native tests exercise editing without lint findings.
        Box::pin(async {
            Ok(KubernetesYamlLintResult {
                diagnostics: Vec::new(),
                notes: Vec::new(),
            })
        })
    }
    fn apply_yaml(&self, request: YamlApplyRequest) -> Read<YamlApplyResult> {
        self.detail.apply_yaml(request)
    }
    fn namespaces(&self, _: String, _: Option<String>) -> Read<Vec<NamespaceSummary>> {
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
        self.shell.kubeconfig_sources()
    }
    /// Like the production backend, which cannot persist kubeconfig settings.
    fn kubeconfig_writes_unavailable(&self) -> Option<&'static str> {
        Some(KUBECONFIG_WRITES)
    }
    fn kubeconfig_source_edits_unavailable(&self) -> Option<&'static str> {
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
            |window, cx| {
                cx.new(|cx| {
                    Inspector::new(backend, window, cx).with_saved_workspace(
                        "Parity mock workspace",
                        "mock-dev",
                        cx,
                    )
                })
            },
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

/// Refresh lives in the Live updates menu, as in the web top bar. The menu
/// stays open after a refresh, so this closes it again.
fn refresh(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("live-status", cx);
        window.render_frame(cx);
        window.click("refresh", cx);
        window.render_frame(cx);
        window.click("live-status", cx);
    })
    .unwrap();
}
#[path = "native/adapters.rs"]
mod adapters;
#[path = "native/mcp.rs"]
mod mcp;
