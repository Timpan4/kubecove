//! The browser-mock fixture both shared-test drivers render. `e2e/parity/visual/export-fixture.ts`
//! exports the web's `tauri-dev-mocks` responses, so the GPUI client sees the same data the
//! Svelte client does in the visual comparison and the parity scenarios.

mod argo;
mod context_topology;
mod coverage;
mod delivery;
mod detail_tabs;
mod launcher_sources;
mod live_data;
mod mcp;
mod namespace_rbac;
mod page_reads;
mod resource_fixture;
mod session_controls;
mod settings_integrations;
mod settings_remote;
mod timing;
mod unexpected;

use detail_tabs::DetailTabs;
use kubecove_gpui::{
    backend::{
        Backend, CLUSTER_KINDS, KubeconfigSourcesSummary, LiveSessions, LogStream, LogTarget, Read,
        Scope, ScopeResult,
    },
    gitops::GitOpsInventory,
};
use kubecove_lib::models::{
    AppError, AppErrorKind, ArgoApplicationInspector, ArgoApplicationRef, ArgoApplicationSummary,
    ClusterContext, ClusterOperationResult, DeploymentRevision, HelmReleaseDetails,
    HelmReleaseList, HelmReleaseReconciliation, HelmReleaseSummary, IncidentCockpitSummary,
    KubernetesYamlLintResult, NamespaceSummary, PodExecSessionSummary, PortForwardSessionSummary,
    RbacAccessReviewRequest, RbacAccessReviewResult, RbacInspectionSummary, ResourceActionPreview,
    ResourceActionRequest, ResourceActionState, ResourceActionStateRequest, ResourceDetailsFull,
    ResourceEventSummary, ResourceSummary, ResourceTopology, YamlApplyPreview, YamlApplyRequest,
    YamlApplyResult, YamlViewMode,
};
use serde::Deserialize;
use std::{collections::HashMap, path::Path, sync::Mutex};

#[derive(Deserialize)]
pub struct Fixture {
    #[serde(skip)]
    pub shell_read_phase: Mutex<String>,
    #[serde(skip)]
    timing: timing::Timing,
    #[serde(default)]
    coverage: HashMap<String, HashMap<String, serde_json::Value>>,
    #[serde(skip)]
    coverage_state: coverage::CoverageState,
    #[serde(rename = "overflowEvents", default)]
    overflow_events: Vec<ResourceEventSummary>,
    #[serde(rename = "pageReads", default)]
    page_reads: HashMap<String, serde_json::Value>,
    #[serde(skip)]
    page_read_phase: Mutex<String>,
    mcp: mcp::McpFixture,
    #[serde(rename = "argoConnected")]
    pub argo: argo::Argo,
    #[serde(rename = "secretReveals")]
    secret_reveals: Vec<serde_json::Value>,
    #[serde(skip)]
    documentation_requests: Mutex<Vec<String>>,
    #[serde(skip)]
    preferences: kubecove_gpui::preferences::Store,
    #[serde(skip)]
    settings_file: std::path::PathBuf,
    #[serde(skip)]
    settings: settings_integrations::SettingsIntegrations,
    #[serde(rename = "sessionControls", default)]
    session_controls: session_controls::SessionControls,
    kinds: Vec<kubecove_lib::models::DiscoveredResourceKind>,
    #[serde(skip)]
    live: live_data::LiveData,
    #[serde(rename = "liveEvents")]
    live_events: Vec<ResourceEventSummary>,
    #[serde(rename = "contexts")]
    cluster_contexts: Vec<ClusterContext>,
    namespaces: Vec<NamespaceSummary>,
    resources: Vec<ResourceSummary>,
    #[serde(rename = "contextResources")]
    context_resources: Vec<ResourceSummary>,
    #[serde(skip)]
    pub failed_workspace_context: Mutex<Option<String>>,
    /// Set by the frames harness: scope reads build their rows on the background
    /// executor, as `NativeBackend` builds them on its runtime, and the dispatcher
    /// drain still waits for them. UI tests leave it unset and build them inline.
    #[serde(skip)]
    pub off_thread_reads:
        std::sync::OnceLock<(std::sync::Weak<Fixture>, gpui_kit::BackgroundExecutor)>,
    details: Vec<ResourceDetailsFull>,
    events: Vec<ResourceEvents>,
    topology: ResourceTopology,
    #[serde(rename = "networkTopology")]
    network_topology: ResourceTopology,
    #[serde(rename = "contextTopologies", default)]
    context_topologies: HashMap<String, context_topology::ContextTopology>,
    metrics: kubecove_lib::models::ResourceMetricsSummary,
    #[serde(rename = "appUsage")]
    app_usage: kubecove_lib::models::AppUsageMetrics,
    gitops: delivery::GitOps,
    helm: delivery::Helm,
    /// Per-namespace graphs, which the incident guide reads for Pods.
    #[serde(rename = "namespaceTopologies")]
    namespace_topologies: HashMap<String, ResourceTopology>,
    incidents: IncidentCockpitSummary,
    #[serde(rename = "liveSessions")]
    live_sessions: FixtureSessions,
    #[serde(rename = "detailTabs")]
    detail_tabs: DetailTabs,
    rbac: HashMap<String, RbacInspectionSummary>,
    #[serde(rename = "rbacReviews")]
    rbac_reviews: Vec<RbacReview>,
    /// The mocks keep kubeconfig settings in page memory, so writes persist
    /// until the next scenario builds a fresh fixture.
    kubeconfig: Mutex<KubeconfigSourcesSummary>,
}

/// `review_rbac_access` answers by verb in the browser mocks.
#[derive(Deserialize)]
struct RbacReview {
    verb: String,
    result: Option<RbacAccessReviewResult>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct ResourceEvents {
    kind: String,
    name: String,
    namespace: Option<String>,
    events: Vec<ResourceEventSummary>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixtureSessions {
    port_forwards: Vec<PortForwardSessionSummary>,
    exec_sessions: Vec<PodExecSessionSummary>,
    source_key: Option<String>,
    show_source_labels: bool,
}

impl Fixture {
    #[allow(dead_code)]
    pub fn terminal_output(&self, container: &str, data: &str) {
        self.session_controls.terminal_output(container, data);
    }
    #[allow(dead_code)]
    pub fn documentation_requests(&self) -> Vec<String> {
        self.documentation_requests.lock().unwrap().clone()
    }
    // Shared with native and visual tests, which do not advance fixture operations.
    #[allow(dead_code)]
    pub fn complete_argo_operation(&self) {
        self.argo
            .complete
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
    // The visual harness shares the fixture but does not replay acceptance scripts.
    #[allow(dead_code)]
    pub fn replay_sessions(&self, script: &str) {
        self.session_controls.replay(script);
    }
    pub fn load(path: &Path) -> Self {
        let bytes = std::fs::read(path)
            .unwrap_or_else(|error| panic!("fixture {} unreadable: {error}", path.display()));
        let mut fixture: Self =
            serde_json::from_slice(&bytes).expect("fixture must match the Rust models");
        fixture.initialize_settings_storage();
        fixture
    }
}

impl Backend for Fixture {
    fn diagnostics(
        &self,
        enabled: bool,
        clear: bool,
    ) -> Read<kubecove_gpui::backend::adapters::BackendDiagnostics> {
        let read = Box::pin(async { Ok(Default::default()) });
        if enabled && !clear {
            self.timing.wrap("get_backend_diagnostics", "", read)
        } else {
            read
        }
    }
    mcp::fixture!();
    fn open_documentation(&self) -> Read<()> {
        self.documentation_requests
            .lock()
            .unwrap()
            .push(kubecove_gpui::backend::adapters::DOCUMENTATION_URL.into());
        Box::pin(async { Ok(()) })
    }
    fn reveal_secret(&self, resource: ResourceSummary, key: String) -> Read<String> {
        self.reveal_secret_fixture(resource, key)
    }
    fn discover_argo(
        &self,
        request: kubecove_gpui::backend::ArgoTarget,
    ) -> Read<Vec<kubecove_lib::models::ArgoServerCapability>> {
        self.argo.discover(request)
    }
    fn preview_argo(
        &self,
        request: kubecove_gpui::backend::ArgoTarget,
    ) -> Read<kubecove_lib::models::ArgoTunnelTargetPreview> {
        self.argo.preview(request)
    }
    fn load_preferences(&self) -> Result<kubecove_gpui::preferences::Document, AppError> {
        self.preferences.load()
    }
    fn save_preferences(
        &self,
        document: kubecove_gpui::preferences::Document,
    ) -> Result<kubecove_gpui::preferences::Document, AppError> {
        self.preferences.save(document)
    }
    fn connect_argo(
        &self,
        request: kubecove_gpui::backend::ArgoConnect,
    ) -> Read<kubecove_lib::models::ArgoConnectionStatus> {
        self.argo.connect(request)
    }
    fn disconnect_argo(&self, id: String) -> Read<()> {
        self.argo.disconnect(id)
    }
    fn forget_argo(&self, profile: kubecove_lib::models::ArgoConnectionProfile) -> Read<()> {
        self.argo.forget(profile)
    }
    fn compare_argo(
        &self,
        request: kubecove_gpui::backend::ArgoInspection,
        resource: kubecove_lib::models::ArgoManagedResource,
    ) -> Read<kubecove_lib::models::ArgoResourceComparison> {
        self.argo.compare(request, resource)
    }
    fn inspect_argo(
        &self,
        request: kubecove_gpui::backend::ArgoInspection,
    ) -> Read<ArgoApplicationInspector> {
        self.argo.inspect(request)
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
        // The web mock routes this command through mockTiming with an empty context.
        self.timing
            .wrap("run_argo_operation", "", self.argo.run(confirmation))
    }
    fn reorder_kubeconfig_paths(&self, paths: Vec<String>) -> Read<KubeconfigSourcesSummary> {
        self.reorder_kubeconfig_read(paths)
    }
    fn remove_kubeconfig_path(&self, path: String) -> Read<KubeconfigSourcesSummary> {
        let mut sources = self.kubeconfig.lock().unwrap();
        sources.paths.retain(|entry| entry.path != path);
        let sources = sources.clone();
        let saved = self.save_settings(&sources);
        Box::pin(async move {
            saved?;
            Ok(sources)
        })
    }
    fn import_workspace(&self) -> Read<Option<String>> {
        self.import_workspace_read()
    }
    fn export_workspace(&self, _name: String, content: String) -> Read<bool> {
        self.export_workspace_read(content)
    }
    fn remote_status(&self) -> Read<Option<kubecove_gpui::backend::RemoteConnectionSummary>> {
        self.remote_status_read()
    }
    fn connect_remote(
        &self,
        url: String,
        token: String,
    ) -> Read<kubecove_gpui::backend::RemoteConnectionSummary> {
        self.connect_remote_read(url, token)
    }
    fn disconnect_remote(&self) -> Read<()> {
        self.disconnect_remote_read()
    }
    fn remote_sign_in(&self) -> Read<Option<kubecove_gpui::backend::RemoteConnectionSummary>> {
        self.remote_sign_in_read()
    }
    fn remote_cancel_sign_in(&self) -> Read<()> {
        self.remote_cancel_sign_in_read()
    }
    fn remote_sign_in_link(&self) -> Read<Option<String>> {
        self.remote_sign_in_link_read()
    }
    fn remote_start_new_session(&self) -> Read<kubecove_gpui::backend::RemoteConnectionSummary> {
        self.remote_start_new_session_read()
    }
    fn remote_sign_out(&self) -> Read<kubecove_gpui::backend::RemoteConnectionSummary> {
        self.remote_sign_out_read()
    }
    fn remote_session_changes(&self) -> tokio::sync::watch::Receiver<u64> {
        self.settings.session_changes.subscribe()
    }
    fn pick_kubeconfig_paths(&self) -> Read<KubeconfigSourcesSummary> {
        self.pick_kubeconfig_read()
    }
    fn updates_enabled(&self) -> bool {
        self.updates_enabled_read()
    }
    fn check_updates(&self) -> Read<kubecove_gpui::backend::UpdateStatus> {
        self.check_updates_read()
    }
    fn cleanup_sessions(
        &self,
        request: kubecove_lib::models::LiveSessionCleanupRequest,
    ) -> Read<kubecove_lib::models::LiveSessionCleanupResult> {
        self.session_controls.cleanup(
            &self.live_sessions.port_forwards,
            &self.live_sessions.exec_sessions,
            request,
        )
    }
    fn start_forward(
        &self,
        request: kubecove_lib::models::PortForwardRequest,
    ) -> Read<PortForwardSessionSummary> {
        self.session_controls.start(request)
    }
    fn stop_forward(&self, id: String) -> Read<bool> {
        self.coverage_stop_forward(id)
    }
    fn exec(
        &self,
        request: kubecove_lib::models::PodExecSessionRequest,
    ) -> Read<kubecove_gpui::backend::ExecStream> {
        self.session_controls.exec(request)
    }
    fn exec_input(&self, id: String, data: String) -> Read<bool> {
        self.session_controls.input(id, data)
    }
    fn stop_exec(&self, id: String) -> Read<bool> {
        self.coverage_stop_exec(self.session_controls.stop_exec(id))
    }
    fn exec_resize(
        &self,
        id: String,
        size: kubecove_lib::models::PodExecTerminalSize,
    ) -> Read<bool> {
        self.session_controls.resize(id, size)
    }
    fn kind_resources(
        &self,
        scope: Scope,
        key: kubecove_lib::models::WatchResourceKey,
        recycled: Vec<ResourceSummary>,
    ) -> Read<kubecove_gpui::backend::KindRows> {
        self.kind_read(scope, key, recycled)
    }
    fn named_kind_resources(
        &self,
        scope: Scope,
        key: kubecove_lib::models::WatchResourceKey,
        names: kubecove_gpui::backend::WatchNames,
        recycled: Vec<ResourceSummary>,
    ) -> Read<kubecove_gpui::backend::KindRows> {
        self.named_kind_read(scope, key, names, recycled)
    }
    fn namespace_custom_kinds(
        &self,
        scope: Scope,
    ) -> Read<Vec<kubecove_lib::models::DiscoveredResourceKind>> {
        self.namespace_custom_kinds_fixture(scope)
    }
    fn resource_kinds(
        &self,
        context: String,
    ) -> Read<Vec<kubecove_lib::models::DiscoveredResourceKind>> {
        let missing = *self.shell_read_phase.lock().unwrap() == "kind-missing";
        let kinds = if missing {
            Vec::new()
        } else {
            self.kinds.clone()
        };
        self.timing.read("list_resource_kinds", &context, "", kinds)
    }
    fn watch(&self, target: kubecove_gpui::backend::WatchTarget) -> Read<LogStream> {
        self.live_watch(target)
    }
    fn contexts(&self, _kubeconfig_source_key: Option<String>) -> Read<Vec<ClusterContext>> {
        self.launcher_contexts()
    }
    fn resources(&self, scope: Scope, _: bool) -> Read<ScopeResult> {
        self.resource_fixture(scope)
    }
    fn details(&self, resource: ResourceSummary) -> Read<ResourceDetailsFull> {
        self.details_fixture(resource)
    }
    fn application_resources(&self, app: ArgoApplicationSummary, _: bool) -> Read<ScopeResult> {
        self.application_resources_fixture(app)
    }
    fn topology(&self, scope: Scope) -> Read<std::sync::Arc<ResourceTopology>> {
        self.topology_fixture(scope)
    }
    fn network_topology(&self, scope: Scope) -> Read<std::sync::Arc<ResourceTopology>> {
        self.network_topology_fixture(scope)
    }
    fn app_usage(&self, _: bool) -> Read<kubecove_lib::models::AppUsageMetrics> {
        let usage = self.app_usage.clone();
        self.timing.read("get_app_usage_metrics", "", "", usage)
    }
    fn metrics(&self, scope: Scope) -> Read<kubecove_lib::models::ResourceMetricsSummary> {
        self.metrics_fixture(scope)
    }
    fn gitops(&self, context: String) -> Read<GitOpsInventory> {
        self.gitops_fixture(context)
    }
    fn cluster_resources(&self, _: String, _: bool) -> Read<ScopeResult> {
        self.cluster_resources_fixture()
    }
    fn events(
        &self,
        resource: ResourceSummary,
        _: Option<String>,
    ) -> Read<Vec<ResourceEventSummary>> {
        self.events_fixture(resource)
    }
    fn incidents(&self, context: String) -> Read<IncidentCockpitSummary> {
        let incidents = self.incidents.clone();
        self.coverage_read(
            "list_incident_cockpit",
            self.page_read(
                "list_incident_cockpit",
                self.timing
                    .read("list_incident_cockpit", &context, "", incidents),
            ),
        )
    }
    fn live_sessions(&self) -> Read<LiveSessions> {
        let sessions = &self.live_sessions;
        let sessions = LiveSessions {
            port_forwards: self.session_controls.list(&sessions.port_forwards),
            exec_sessions: self.session_controls.execs(&sessions.exec_sessions),
            source_key: sessions.source_key.clone(),
            show_source_labels: sessions.show_source_labels,
        };
        let forwards = self.coverage_read(
            "list_port_forwards",
            Box::pin(async move { Ok(sessions.port_forwards) }),
        );
        let exec = self.coverage_read(
            "list_pod_exec_sessions",
            Box::pin(async move { Ok(sessions.exec_sessions) }),
        );
        self.timing.wrap(
            "list_port_forwards",
            "",
            Box::pin(async move {
                Ok(LiveSessions {
                    port_forwards: forwards.await?,
                    exec_sessions: exec.await?,
                    source_key: sessions.source_key,
                    show_source_labels: sessions.show_source_labels,
                })
            }),
        )
    }
    fn log_stream(&self, target: LogTarget) -> Read<LogStream> {
        self.detail_tabs.log_stream(target, &self.coverage_script())
    }
    fn deployment_revisions(&self, resource: ResourceSummary) -> Read<Vec<DeploymentRevision>> {
        self.detail_tabs.deployment_revisions(resource)
    }
    fn action_state(&self, request: ResourceActionStateRequest) -> Read<ResourceActionState> {
        let context = request.target.cluster_context.clone();
        self.timing.wrap(
            "get_resource_action_state",
            &context,
            self.detail_tabs.action_state(request),
        )
    }
    fn preview_action(&self, request: ResourceActionRequest) -> Read<ResourceActionPreview> {
        self.detail_tabs.preview_action(request)
    }
    fn execute_action(&self, request: ResourceActionRequest) -> Read<ClusterOperationResult> {
        self.detail_tabs.execute_action(request)
    }
    fn resource_yaml_encoded(
        &self,
        resource: ResourceSummary,
        mode: YamlViewMode,
        encoding: kubecove_lib::models::YamlEncoding,
    ) -> Read<String> {
        self.detail_tabs.resource_yaml(resource, mode, encoding)
    }

    fn resource_yaml(&self, resource: ResourceSummary, mode: YamlViewMode) -> Read<String> {
        self.detail_tabs
            .resource_yaml(resource, mode, kubecove_lib::models::YamlEncoding::Yaml)
    }

    fn prepare_yaml_apply(&self, request: YamlApplyRequest) -> Read<YamlApplyPreview> {
        if self.coverage_script() == "conflict" && !request.force_conflicts {
            return Box::pin(async {
                Err(AppError::new(
                    "Controlled field ownership conflict",
                    AppErrorKind::FieldManagerConflict,
                ))
            });
        }
        // The dry run is a timed mock read, as lint_yaml is, so a scripted failure applies.
        let context = request.cluster_context.clone();
        let preview = self.detail_tabs.prepare_yaml_apply(request);
        let timing = self.timing.clone();
        Box::pin(async move {
            timing
                .read("prepare_yaml_apply", &context, "", preview.await?)
                .await
        })
    }
    fn apply_yaml(&self, request: YamlApplyRequest) -> Read<YamlApplyResult> {
        self.detail_tabs.apply_yaml(request)
    }
    fn lint_yaml(&self, request: YamlApplyRequest) -> Read<KubernetesYamlLintResult> {
        // The browser mock's lint is a command like any other, so the timing gate applies.
        let context = request.cluster_context.clone();
        let lint = self.detail_tabs.lint_yaml(&request);
        let timing = self.timing.clone();
        Box::pin(async move {
            let lint = lint?;
            timing
                .read("lint_kubernetes_yaml", &context, "", lint)
                .await
        })
    }
    fn namespaces(
        &self,
        context: String,
        _kubeconfig_source_key: Option<String>,
    ) -> Read<Vec<NamespaceSummary>> {
        self.namespaces_read(context)
    }
    fn rbac(&self, context: String) -> Read<RbacInspectionSummary> {
        self.rbac_read(context)
    }
    fn review_rbac(&self, request: RbacAccessReviewRequest) -> Read<RbacAccessReviewResult> {
        self.review_rbac_read(request)
    }
    fn kubeconfig_sources(&self) -> Read<KubeconfigSourcesSummary> {
        self.launcher_kubeconfig_sources()
    }
    fn kubeconfig_writes_unavailable(&self) -> Option<&'static str> {
        None
    }
    fn kubeconfig_source_edits_unavailable(&self) -> Option<&'static str> {
        None
    }
    fn set_kubeconfig_env_var(&self, env_var: String) -> Read<KubeconfigSourcesSummary> {
        self.set_kubeconfig_env_var_read(env_var)
    }
    fn add_kubeconfig_paths(&self, paths: Vec<String>) -> Read<KubeconfigSourcesSummary> {
        self.add_kubeconfig_paths_read(paths)
    }
    fn set_show_kubeconfig_source_labels(&self, show: bool) -> Read<KubeconfigSourcesSummary> {
        self.set_source_labels_read(show)
    }

    fn helm_releases(&self, context: String) -> Read<HelmReleaseList> {
        self.coverage_read(
            "list_helm_releases",
            self.page_read(
                "list_helm_releases",
                self.timing
                    .wrap("list_helm_releases", &context, self.helm.releases()),
            ),
        )
    }
    fn helm_release(&self, release: HelmReleaseSummary) -> Read<HelmReleaseDetails> {
        self.coverage_read("get_helm_release_details", self.helm.release(release))
    }
    fn helm_reconciliation(&self, release: HelmReleaseSummary) -> Read<HelmReleaseReconciliation> {
        self.helm.reconciliation(release)
    }
    fn argo_inspector(&self, application: ArgoApplicationRef) -> Read<ArgoApplicationInspector> {
        self.gitops.inspector(application)
    }
}
