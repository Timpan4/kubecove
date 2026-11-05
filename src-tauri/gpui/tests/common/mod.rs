//! The browser-mock fixture both shared-test drivers render. `e2e/parity/visual/export-fixture.ts`
//! exports the web's `tauri-dev-mocks` responses, so the GPUI client sees the same data the
//! Svelte client does in the visual comparison and the parity scenarios.

mod argo;
mod delivery;
mod detail_tabs;
mod live_data;
mod mcp;
mod session_controls;
mod settings_integrations;

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
    NamespaceSummary, PodExecSessionSummary, PortForwardSessionSummary, RbacAccessReviewRequest,
    RbacAccessReviewResult, RbacAccessReviewTarget, RbacInspectionSummary, ResourceActionPreview,
    ResourceActionRequest, ResourceActionState, ResourceActionStateRequest, ResourceDetailsFull,
    ResourceEventSummary, ResourceSummary, ResourceTopology, YamlApplyPreview, YamlApplyRequest,
    YamlApplyResult, YamlViewMode,
};
use serde::Deserialize;
use std::{collections::HashMap, path::Path, sync::Mutex};

#[derive(Deserialize)]
pub struct Fixture {
    mcp: mcp::McpFixture,
    #[serde(rename = "argoConnected")]
    argo: argo::Argo,
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
    details: Vec<ResourceDetailsFull>,
    events: Vec<ResourceEvents>,
    topology: ResourceTopology,
    #[serde(rename = "networkTopology")]
    network_topology: ResourceTopology,
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
    mcp::fixture!();
    fn open_documentation(&self) -> Read<()> {
        self.documentation_requests
            .lock()
            .unwrap()
            .push(kubecove_gpui::backend::adapters::DOCUMENTATION_URL.into());
        Box::pin(async { Ok(()) })
    }
    fn reveal_secret(&self, resource: ResourceSummary, key: String) -> Read<String> {
        let request = serde_json::json!({"clusterContext": resource.cluster, "name": resource.name, "namespace": resource.namespace, "key": key});
        let found = self
            .secret_reveals
            .iter()
            .find(|entry| entry["request"] == request)
            .and_then(|entry| entry["encoded"].as_str())
            .map(str::to_owned);
        Box::pin(async move {
            found.ok_or_else(|| {
                AppError::new("No matching Secret reveal fixture", AppErrorKind::NotFound)
            })
        })
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
        self.argo.run(confirmation)
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
        _token: String,
    ) -> Read<kubecove_gpui::backend::RemoteConnectionSummary> {
        self.connect_remote_read(url)
    }
    fn disconnect_remote(&self) -> Read<()> {
        self.disconnect_remote_read()
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
        self.session_controls.stop(id)
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
        self.session_controls.stop_exec(id)
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
    ) -> Read<Vec<ResourceSummary>> {
        if self.live_failed() {
            return Box::pin(async {
                Err(AppError::new(
                    "Scripted reload failure",
                    AppErrorKind::Cluster,
                ))
            });
        }
        let rows = self
            .resources
            .iter()
            .filter(|row| row.kind == key.resource_kind.kind)
            .filter(|row| {
                key.resource_kind
                    .api_version
                    .as_ref()
                    .is_none_or(|api| row.api_version.as_ref() == Some(api))
            })
            .filter(|row| {
                key.namespace
                    .as_ref()
                    .is_none_or(|ns| row.namespace.as_ref() == Some(ns))
            })
            .map(|row| ResourceSummary {
                cluster: scope.context.clone(),
                ..row.clone()
            })
            .collect();
        let rows = self.live_rows(rows);
        Box::pin(async move { Ok(rows) })
    }
    fn resource_kinds(&self, _: String) -> Read<Vec<kubecove_lib::models::DiscoveredResourceKind>> {
        let kinds = self.kinds.clone();
        Box::pin(async move { Ok(kinds) })
    }
    fn watch(&self, target: kubecove_gpui::backend::WatchTarget) -> Read<LogStream> {
        self.live_watch(target)
    }
    fn contexts(&self) -> Read<Vec<ClusterContext>> {
        let contexts = self.cluster_contexts.clone();
        Box::pin(async move { Ok(contexts) })
    }
    fn resources(&self, scope: Scope, _: bool) -> Read<ScopeResult> {
        if self.live_failed() {
            return Box::pin(async {
                Err(AppError::new(
                    "Scripted reload failure",
                    AppErrorKind::Cluster,
                ))
            });
        }
        // Cluster-scoped kinds come from `cluster_resources`, as in the native backend.
        // The mocks serve the same rows for every context, labeled with it.
        let resources = self
            .resources
            .iter()
            .filter(|row| !CLUSTER_KINDS.contains(&row.kind.as_str()))
            .filter(|row| {
                scope.namespace.is_none() || row.namespace.as_deref() == scope.namespace.as_deref()
            })
            .map(|row| ResourceSummary {
                cluster: scope.context.clone(),
                ..row.clone()
            })
            .collect();
        let resources = self.live_rows(resources);
        let namespaces = self.namespaces.clone();
        Box::pin(async move {
            Ok(ScopeResult {
                namespaces,
                resources,
                warnings: vec![],
            })
        })
    }
    fn details(&self, resource: ResourceSummary) -> Read<ResourceDetailsFull> {
        let found = self
            .details
            .iter()
            .find(|detail| {
                detail.summary.kind == resource.kind
                    && detail.summary.name == resource.name
                    && detail.summary.namespace == resource.namespace
            })
            .cloned()
            .or_else(|| self.gitops.application_details(&resource));
        Box::pin(async move {
            found.ok_or_else(|| AppError::new("No fixture details", AppErrorKind::NotFound))
        })
    }
    fn application_resources(&self, _: ArgoApplicationSummary, _: bool) -> Read<ScopeResult> {
        let resources = self.resources.clone();
        let namespaces = self.namespaces.clone();
        Box::pin(async move {
            Ok(ScopeResult {
                namespaces,
                resources,
                warnings: vec![],
            })
        })
    }
    /// The all-namespaces graph, or a namespace's graph the fixture exported.
    fn topology(&self, scope: Scope) -> Read<ResourceTopology> {
        let topology = scope
            .namespace
            .and_then(|namespace| self.namespace_topologies.get(&namespace))
            .unwrap_or(&self.topology)
            .clone();
        Box::pin(async move { Ok(topology) })
    }
    fn network_topology(&self, scope: Scope) -> Read<ResourceTopology> {
        let mut graph = self.network_topology.clone();
        if let Some(namespace) = &scope.namespace {
            graph
                .nodes
                .retain(|node| node.namespace.as_ref() == Some(namespace));
            let ids: std::collections::HashSet<_> =
                graph.nodes.iter().map(|node| &node.id).collect();
            graph
                .edges
                .retain(|edge| ids.contains(&edge.source) && ids.contains(&edge.target));
        }
        Box::pin(async move { Ok(graph) })
    }
    fn app_usage(&self) -> Read<kubecove_lib::models::AppUsageMetrics> {
        let usage = self.app_usage.clone();
        Box::pin(async move { Ok(usage) })
    }
    fn metrics(&self, scope: Scope) -> Read<kubecove_lib::models::ResourceMetricsSummary> {
        let mut metrics = self.metrics.clone();
        for metric in metrics
            .pods
            .iter_mut()
            .chain(&mut metrics.nodes)
            .chain(&mut metrics.workloads)
        {
            metric.cluster = scope.context.clone();
        }
        Box::pin(async move { Ok(metrics) })
    }
    fn gitops(&self, _: String) -> Read<GitOpsInventory> {
        self.gitops.inventory()
    }
    fn cluster_resources(&self, _: String, _: bool) -> Read<ScopeResult> {
        let resources = self
            .resources
            .iter()
            .filter(|row| CLUSTER_KINDS.contains(&row.kind.as_str()))
            .cloned()
            .collect();
        Box::pin(async move {
            Ok(ScopeResult {
                namespaces: vec![],
                resources,
                warnings: vec![],
            })
        })
    }
    fn events(
        &self,
        resource: ResourceSummary,
        _: Option<String>,
    ) -> Read<Vec<ResourceEventSummary>> {
        if let Some(events) = self.scripted_events(&resource) {
            return Box::pin(async move { Ok(events) });
        }
        let found = self
            .events
            .iter()
            .find(|row| {
                row.kind == resource.kind
                    && row.name == resource.name
                    && row.namespace == resource.namespace
            })
            .map_or_else(Vec::new, |row| row.events.clone());
        Box::pin(async move { Ok(found) })
    }
    fn incidents(&self, _: String) -> Read<IncidentCockpitSummary> {
        let incidents = self.incidents.clone();
        Box::pin(async move { Ok(incidents) })
    }
    fn live_sessions(&self) -> Read<LiveSessions> {
        let sessions = &self.live_sessions;
        let sessions = LiveSessions {
            port_forwards: self.session_controls.list(&sessions.port_forwards),
            exec_sessions: self.session_controls.execs(&sessions.exec_sessions),
            source_key: sessions.source_key.clone(),
            show_source_labels: sessions.show_source_labels,
        };
        Box::pin(async move { Ok(sessions) })
    }
    fn log_stream(&self, target: LogTarget) -> Read<LogStream> {
        self.detail_tabs.log_stream(target)
    }
    fn deployment_revisions(&self, resource: ResourceSummary) -> Read<Vec<DeploymentRevision>> {
        self.detail_tabs.deployment_revisions(resource)
    }
    fn action_state(&self, request: ResourceActionStateRequest) -> Read<ResourceActionState> {
        self.detail_tabs.action_state(request)
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
        self.detail_tabs.prepare_yaml_apply(request)
    }
    fn apply_yaml(&self, request: YamlApplyRequest) -> Read<YamlApplyResult> {
        self.detail_tabs.apply_yaml(request)
    }
    fn namespaces(&self, _: String) -> Read<Vec<NamespaceSummary>> {
        let namespaces = self.namespaces.clone();
        Box::pin(async move { Ok(namespaces) })
    }
    fn rbac(&self, context: String) -> Read<RbacInspectionSummary> {
        let found = self.rbac.get(&context).cloned();
        Box::pin(async move {
            found.ok_or_else(|| AppError::new("No fixture RBAC inspection", AppErrorKind::NotFound))
        })
    }
    fn review_rbac(&self, request: RbacAccessReviewRequest) -> Read<RbacAccessReviewResult> {
        let verb = match &request.target {
            RbacAccessReviewTarget::Resource { verb, .. }
            | RbacAccessReviewTarget::NonResource { verb, .. } => verb.trim().to_lowercase(),
        };
        let found = self
            .rbac_reviews
            .iter()
            .find(|review| review.verb == verb)
            .map(|review| (review.result.clone(), review.error.clone()));
        Box::pin(async move {
            match found {
                Some((Some(result), _)) => Ok(result),
                Some((None, Some(error))) => Err(AppError::new(error, AppErrorKind::Cluster)),
                _ => Err(AppError::new(
                    format!("No fixture review for {verb}"),
                    AppErrorKind::NotFound,
                )),
            }
        })
    }
    fn kubeconfig_sources(&self) -> Read<KubeconfigSourcesSummary> {
        let sources = self.kubeconfig.lock().expect("kubeconfig lock").clone();
        Box::pin(async move { Ok(sources) })
    }
    fn kubeconfig_writes_unavailable(&self) -> Option<&'static str> {
        None
    }
    fn set_kubeconfig_env_var(&self, env_var: String) -> Read<KubeconfigSourcesSummary> {
        let mut sources = self.kubeconfig.lock().expect("kubeconfig lock");
        let trimmed = env_var.trim();
        sources.kubeconfig_env_var = if trimmed.is_empty() {
            "KUBECONFIG".to_owned()
        } else {
            trimmed.to_owned()
        };
        let sources = sources.clone();
        let saved = self.save_settings(&sources);
        Box::pin(async move {
            saved?;
            Ok(sources)
        })
    }
    fn set_show_kubeconfig_source_labels(&self, show: bool) -> Read<KubeconfigSourcesSummary> {
        let mut sources = self.kubeconfig.lock().expect("kubeconfig lock");
        sources.show_source_labels = show;
        let sources = sources.clone();
        let saved = self.save_settings(&sources);
        Box::pin(async move {
            saved?;
            Ok(sources)
        })
    }

    fn helm_releases(&self, _: String) -> Read<HelmReleaseList> {
        self.helm.releases()
    }
    fn helm_release(&self, release: HelmReleaseSummary) -> Read<HelmReleaseDetails> {
        self.helm.release(release)
    }
    fn helm_reconciliation(&self, release: HelmReleaseSummary) -> Read<HelmReleaseReconciliation> {
        self.helm.reconciliation(release)
    }
    fn argo_inspector(&self, application: ArgoApplicationRef) -> Read<ArgoApplicationInspector> {
        self.gitops.inspector(application)
    }
}
