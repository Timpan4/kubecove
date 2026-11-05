use crate::gitops::GitOpsInventory;
pub use crate::live_sessions::LiveSessions;
use kubecove_lib::{
    commands::{
        self, ArgoConnectionStore, BackendCancellationRegistry, ClusterAccess, ClusterLiveStore,
        PodExecRegistry, PortForwardRegistry, StreamRegistry,
    },
    models::{
        AppError, AppErrorKind, ArgoApplicationInspector, ArgoApplicationRef,
        ArgoApplicationSummary, ClusterContext, ClusterOperationResult, DeploymentRevision,
        HelmReleaseDetails, HelmReleaseList, HelmReleaseReconciliation, HelmReleaseSummary,
        IncidentCockpitSummary, KubernetesYamlLintResult, NamespaceSummary,
        RbacAccessReviewRequest, RbacAccessReviewResult, RbacInspectionSummary,
        ResourceActionPreview, ResourceActionRequest, ResourceActionState,
        ResourceActionStateRequest, ResourceDetailsFull, ResourceEventSummary, ResourceSummary,
        ResourceTopology, WatchResourceKey, YamlApplyPreview, YamlApplyRequest, YamlApplyResult,
        YamlViewMode,
    },
    runtime::State,
};
use std::{future::Future, pin::Pin, sync::Arc};

pub type Read<T> = Pin<Box<dyn Future<Output = Result<T, AppError>> + Send>>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scope {
    pub context: String,
    pub namespace: Option<String>,
}

pub struct ScopeResult {
    pub namespaces: Vec<NamespaceSummary>,
    pub resources: Vec<ResourceSummary>,
    pub warnings: Vec<String>,
}

/// Both production commands and controlled UI-test responses use this seam.
/// Dropping a pending read must cancel its host-owned task.
pub trait Backend: Send + Sync + 'static {
    fn diagnostics(&self, _enabled: bool, _clear: bool) -> Read<adapters::BackendDiagnostics> {
        Box::pin(async { Ok(Default::default()) })
    }
    fn open_documentation(&self) -> Read<()> {
        settings_integrations::unavailable("Documentation opener unavailable")
    }
    fn reveal_secret(&self, _resource: ResourceSummary, _key: String) -> Read<String> {
        settings_integrations::unavailable("Secret reveal unavailable")
    }
    fn load_preferences(&self) -> Result<crate::preferences::Document, AppError> {
        Ok(Default::default())
    }
    fn save_preferences(
        &self,
        document: crate::preferences::Document,
    ) -> Result<crate::preferences::Document, AppError> {
        Ok(document)
    }
    argo::defaults!();
    mcp::defaults!();
    fn remote_status(&self) -> Read<Option<RemoteConnectionSummary>> {
        Box::pin(async { Ok(None) })
    }
    fn connect_remote(&self, _url: String, _token: String) -> Read<RemoteConnectionSummary> {
        settings_integrations::unavailable("Remote host adapter unavailable")
    }
    fn disconnect_remote(&self) -> Read<()> {
        Box::pin(async { Ok(()) })
    }
    fn pick_kubeconfig_paths(&self) -> Read<KubeconfigSourcesSummary> {
        settings_integrations::unavailable("File dialog unavailable")
    }
    fn add_kubeconfig_paths(&self, _paths: Vec<String>) -> Read<KubeconfigSourcesSummary> {
        settings_integrations::unavailable("Source writes unavailable")
    }
    fn remove_kubeconfig_path(&self, _path: String) -> Read<KubeconfigSourcesSummary> {
        settings_integrations::unavailable("Source writes unavailable")
    }
    fn reorder_kubeconfig_paths(&self, _paths: Vec<String>) -> Read<KubeconfigSourcesSummary> {
        settings_integrations::unavailable("Source writes unavailable")
    }
    fn updates_enabled(&self) -> bool {
        !cfg!(debug_assertions)
    }
    fn check_updates(&self) -> Read<UpdateStatus> {
        settings_integrations::unavailable("Update checks unavailable")
    }
    fn install_update(&self) -> Read<()> {
        settings_integrations::unavailable(INSTALL_UNAVAILABLE)
    }
    fn import_workspace(&self) -> Read<Option<String>> {
        settings_integrations::unavailable("File dialog unavailable")
    }
    fn export_workspace(&self, _name: String, _content: String) -> Read<bool> {
        settings_integrations::unavailable("File dialog unavailable")
    }
    fn resource_kinds(
        &self,
        _context: String,
    ) -> Read<Vec<kubecove_lib::models::DiscoveredResourceKind>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    fn namespace_custom_kinds(
        &self,
        scope: Scope,
    ) -> Read<Vec<kubecove_lib::models::DiscoveredResourceKind>> {
        let read = self.resource_kinds(scope.context);
        Box::pin(async move {
            Ok(read
                .await?
                .into_iter()
                .filter(|kind| kind.namespaced)
                .collect())
        })
    }
    fn kind_resources(&self, scope: Scope, key: WatchResourceKey) -> Read<Vec<ResourceSummary>> {
        let read = self.resources(scope, false);
        Box::pin(async move {
            Ok(read
                .await?
                .resources
                .into_iter()
                .filter(|row| {
                    row.kind == key.resource_kind.kind
                        && key
                            .resource_kind
                            .api_version
                            .as_ref()
                            .is_none_or(|api| row.api_version.as_ref() == Some(api))
                })
                .collect())
        })
    }
    fn watch(&self, _target: WatchTarget) -> Read<LogStream> {
        Box::pin(async {
            Err(AppError::new(
                "Live updates unavailable",
                AppErrorKind::UnsupportedOperation,
            ))
        })
    }
    fn contexts(&self) -> Read<Vec<ClusterContext>>;
    fn resources(&self, scope: Scope, refresh: bool) -> Read<ScopeResult>;
    /// Built-in kinds and namespaces, read before custom resources so the table
    /// can render first. Defaults to the combined read.
    fn scope_builtin_resources(&self, scope: Scope, refresh: bool) -> Read<ScopeResult> {
        self.resources(scope, refresh)
    }
    /// Discovered custom-resource rows, read after the built-ins. `None` when the
    /// combined read already covers them.
    fn scope_custom_resources(&self, _scope: Scope) -> Option<Read<ScopeResult>> {
        None
    }
    /// The Application resource workspace uses all supported and discovered kinds.
    fn application_resources(
        &self,
        application: ArgoApplicationSummary,
        refresh: bool,
    ) -> Read<ScopeResult> {
        self.resources(
            Scope {
                context: application.cluster,
                namespace: None,
            },
            refresh,
        )
    }
    fn details(&self, resource: ResourceSummary) -> Read<ResourceDetailsFull>;
    /// The ownership graph for the scope's namespaces (all when unscoped).
    fn topology(&self, scope: Scope) -> Read<ResourceTopology>;
    fn network_topology(&self, _scope: Scope) -> Read<ResourceTopology> {
        Box::pin(async {
            Err(AppError::new(
                "Network Flow unavailable",
                AppErrorKind::UnsupportedOperation,
            ))
        })
    }
    fn app_usage(&self) -> Read<kubecove_lib::models::AppUsageMetrics> {
        Box::pin(async {
            Err(AppError::new(
                "Usage metrics unavailable",
                AppErrorKind::UnsupportedOperation,
            ))
        })
    }
    fn metrics(&self, _scope: Scope) -> Read<kubecove_lib::models::ResourceMetricsSummary> {
        Box::pin(async {
            Err(AppError::new(
                "metrics API unavailable",
                AppErrorKind::UnsupportedOperation,
            ))
        })
    }
    /// Argo CD and Flux inventory from Kubernetes CRDs.
    fn gitops(&self, context: String) -> Read<GitOpsInventory>;
    /// The Cluster Overview's [`CLUSTER_KINDS`]; `namespaces` stays empty.
    fn cluster_resources(&self, context: String, refresh: bool) -> Read<ScopeResult>;
    /// Events for one resource, matched by UID when the details supplied one.
    fn events(
        &self,
        resource: ResourceSummary,
        uid: Option<String>,
    ) -> Read<Vec<ResourceEventSummary>>;
    /// `list_incident_cockpit` for the context's workspace kinds in every namespace.
    fn incidents(&self, context: String) -> Read<IncidentCockpitSummary>;
    /// The host's port forwards and exec sessions with the active kubeconfig source.
    fn live_sessions(&self) -> Read<LiveSessions>;
    fn start_forward(
        &self,
        request: kubecove_lib::models::PortForwardRequest,
    ) -> Read<kubecove_lib::models::PortForwardSessionSummary> {
        let _ = request;
        session_controls::unavailable()
    }
    fn stop_forward(&self, id: String) -> Read<bool> {
        let _ = id;
        session_controls::unavailable()
    }
    fn exec(&self, request: kubecove_lib::models::PodExecSessionRequest) -> Read<ExecStream> {
        let _ = request;
        session_controls::unavailable()
    }
    fn exec_input(&self, id: String, data: String) -> Read<bool> {
        let _ = (id, data);
        session_controls::unavailable()
    }
    fn exec_resize(
        &self,
        id: String,
        size: kubecove_lib::models::PodExecTerminalSize,
    ) -> Read<bool> {
        let _ = (id, size);
        session_controls::unavailable()
    }
    fn stop_exec(&self, id: String) -> Read<bool> {
        let _ = id;
        session_controls::unavailable()
    }
    fn cleanup_sessions(
        &self,
        request: kubecove_lib::models::LiveSessionCleanupRequest,
    ) -> Read<kubecove_lib::models::LiveSessionCleanupResult> {
        let _ = request;
        session_controls::unavailable()
    }
    /// Starts a log stream; dropping the returned stream stops it.
    fn log_stream(&self, target: LogTarget) -> Read<LogStream>;
    /// `list_deployment_revisions`: ReplicaSet pod templates a Deployment owns.
    fn deployment_revisions(&self, resource: ResourceSummary) -> Read<Vec<DeploymentRevision>>;
    /// `get_resource_action_state`: the action baseline and its permission checks.
    fn action_state(&self, request: ResourceActionStateRequest) -> Read<ResourceActionState>;
    /// `preview_resource_action`: an API-server dry run that changes nothing.
    fn preview_action(&self, request: ResourceActionRequest) -> Read<ResourceActionPreview>;
    /// `execute_resource_action`: a confirmed action guarded by the previewed identity.
    fn execute_action(&self, request: ResourceActionRequest) -> Read<ClusterOperationResult>;
    /// `get_resource_yaml` in a view mode; the apply draft reads `ApplyClean`.
    fn resource_yaml(&self, resource: ResourceSummary, mode: YamlViewMode) -> Read<String>;
    /// Selected read encoding, shared by the YAML pickers and draft loader.
    fn resource_yaml_encoded(
        &self,
        resource: ResourceSummary,
        mode: YamlViewMode,
        encoding: kubecove_lib::models::YamlEncoding,
    ) -> Read<String> {
        let read = self.resource_yaml(resource, mode);
        Box::pin(async move {
            let yaml = read.await?;
            if encoding == kubecove_lib::models::YamlEncoding::Yaml {
                return Ok(yaml);
            }
            let value: serde_json::Value = serde_yaml::from_str(&yaml).map_err(|e| {
                kubecove_lib::models::AppError::new(
                    e.to_string(),
                    kubecove_lib::models::AppErrorKind::Serialization,
                )
            })?;
            commands::helpers::serialize_json_value_document(&value, encoding)
        })
    }
    /// `prepare_yaml_apply`: a server-side apply dry run of a draft.
    fn prepare_yaml_apply(&self, request: YamlApplyRequest) -> Read<YamlApplyPreview>;
    /// `apply_yaml`: applies a draft whose dry run the user reviewed.
    fn apply_yaml(&self, request: YamlApplyRequest) -> Read<YamlApplyResult>;
    /// `lint_kubernetes_yaml`: parser and Kubernetes checks of an editor draft, with no cluster write.
    fn lint_yaml(&self, request: YamlApplyRequest) -> Read<KubernetesYamlLintResult>;
    /// `list_namespaces` for a context the workspace launcher scopes.
    fn namespaces(&self, context: String) -> Read<Vec<NamespaceSummary>>;
    /// `list_rbac_inspection` for the RBAC Cockpit.
    fn rbac(&self, context: String) -> Read<RbacInspectionSummary>;
    /// `review_rbac_access`: one SubjectAccessReview, run only on an explicit user action.
    fn review_rbac(&self, request: RbacAccessReviewRequest) -> Read<RbacAccessReviewResult>;
    /// `get_kubeconfig_sources`.
    fn kubeconfig_sources(&self) -> Read<KubeconfigSourcesSummary>;
    /// Why desktop-only source changes (file picks, removals, reordering and labels) are
    /// unavailable, or `None` when they are.
    fn kubeconfig_writes_unavailable(&self) -> Option<&'static str>;
    /// Why the environment variable and typed path edits would not persist, or `None` when
    /// they do. Remote mode saves both on the server.
    fn kubeconfig_source_edits_unavailable(&self) -> Option<&'static str>;
    /// `set_kubeconfig_env_var`.
    fn set_kubeconfig_env_var(&self, env_var: String) -> Read<KubeconfigSourcesSummary>;
    /// `set_show_kubeconfig_source_labels`.
    fn set_show_kubeconfig_source_labels(&self, show: bool) -> Read<KubeconfigSourcesSummary>;
    /// `list_helm_releases`: releases from Helm storage across the context's namespaces.
    fn helm_releases(&self, context: String) -> Read<HelmReleaseList>;
    /// `get_helm_release_details`: one release's storage record, values and manifest summary.
    fn helm_release(&self, release: HelmReleaseSummary) -> Read<HelmReleaseDetails>;
    /// `get_helm_release_reconciliation`: the manifest compared with live Helm-labelled resources.
    fn helm_reconciliation(&self, release: HelmReleaseSummary) -> Read<HelmReleaseReconciliation>;
    /// `get_argo_application_inspector` over the Kubernetes transport; `context` names the cluster.
    fn argo_inspector(&self, application: ArgoApplicationRef) -> Read<ArgoApplicationInspector>;
}

pub mod adapters;
mod argo;
mod live_data;
pub use argo::{ArgoConnect, ArgoInspection, ArgoTarget};
pub use live_data::WatchTarget;
mod delivery;
mod delivery_workspace;
pub use delivery_workspace::APPLICATION_KINDS;
mod detail_reads;
mod mcp;
mod remote;
mod security;
pub mod settings_integrations;
mod workspace_files;
pub use detail_reads::{LogStream, LogTarget};
pub use kubecove_lib::commands::kubeconfig::KubeconfigSourcesSummary;
pub use remote::DesktopBackend;
pub use settings_integrations::{
    AvailableUpdate, INSTALL_UNAVAILABLE, RemoteConnectionSummary, UpdateStatus,
};
mod inventory;
mod session_controls;
mod topology_metrics;
pub use inventory::CLUSTER_KINDS;
pub use session_controls::ExecStream;

/// Default built-in workspace kinds. Discovery adds custom kinds.
pub use crate::model::WORKSPACE_KINDS as INSPECTION_KINDS;

pub struct NativeBackend {
    mcp: Arc<kubecove_lib::mcp::McpHost>,
    settings_initialized: bool,
    preferences: crate::preferences::Store,
    runtime: Option<tokio::runtime::Runtime>,
    usage: commands::AppUsageMonitor,
    store: ClusterLiveStore,
    cancellations: BackendCancellationRegistry,
    port_forwards: PortForwardRegistry,
    exec_sessions: PodExecRegistry,
    streams: Arc<StreamRegistry>,
    argo_connections: Arc<ArgoConnectionStore>,
}

impl NativeBackend {
    pub fn new() -> std::io::Result<Self> {
        Ok(Self {
            mcp: Arc::default(),
            settings_initialized: false,
            preferences: Default::default(),
            runtime: Some(
                tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()?,
            ),
            usage: Default::default(),
            store: ClusterLiveStore::default(),
            cancellations: BackendCancellationRegistry::default(),
            port_forwards: PortForwardRegistry::default(),
            exec_sessions: PodExecRegistry::default(),
            streams: Arc::default(),
            argo_connections: Arc::default(),
        })
    }

    fn read<T: Send + 'static>(
        &self,
        future: impl Future<Output = Result<T, AppError>> + Send + 'static,
    ) -> Read<T> {
        let task = self
            .runtime
            .as_ref()
            .expect("runtime exists until drop")
            .spawn(future);
        let guard = AbortOnDrop(task.abort_handle());
        Box::pin(async move {
            let _guard = guard;
            task.await.map_err(|error| {
                AppError::new(
                    format!("Backend task failed: {error}"),
                    AppErrorKind::Cluster,
                )
            })?
        })
    }
}

struct AbortOnDrop(tokio::task::AbortHandle);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl Drop for NativeBackend {
    fn drop(&mut self) {
        self.cancellations.cancel_all();
        self.store.cancel_loading();
        self.streams.stop_all();
        self.port_forwards.stop_all();
        self.exec_sessions.stop_all();
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

fn supported(kind: &str) -> Result<(), AppError> {
    if APPLICATION_KINDS.contains(&kind) {
        Ok(())
    } else {
        Err(AppError::new(
            format!("Unsupported inspection kind: {kind}"),
            AppErrorKind::Validation,
        ))
    }
}

impl Backend for NativeBackend {
    fn diagnostics(&self, enabled: bool, clear: bool) -> Read<adapters::BackendDiagnostics> {
        adapters::diagnostics(self, enabled, clear)
    }
    fn open_documentation(&self) -> Read<()> {
        adapters::open_documentation(self)
    }
    fn reveal_secret(&self, resource: ResourceSummary, key: String) -> Read<String> {
        adapters::reveal_secret(self, resource, key)
    }
    fn load_preferences(&self) -> Result<crate::preferences::Document, AppError> {
        self.preferences.load()
    }
    fn save_preferences(
        &self,
        document: crate::preferences::Document,
    ) -> Result<crate::preferences::Document, AppError> {
        self.preferences.save(document)
    }
    argo::native!();
    mcp::native!();
    fn add_kubeconfig_paths(&self, paths: Vec<String>) -> Read<KubeconfigSourcesSummary> {
        if !self.settings_initialized {
            return self.read(security::unsaved());
        }
        self.read(async move { commands::add_kubeconfig_paths(paths) })
    }
    fn start_forward(
        &self,
        request: kubecove_lib::models::PortForwardRequest,
    ) -> Read<kubecove_lib::models::PortForwardSessionSummary> {
        session_controls::start_forward(self, request)
    }
    fn stop_forward(&self, id: String) -> Read<bool> {
        session_controls::stop_forward(self, id)
    }
    fn exec(&self, request: kubecove_lib::models::PodExecSessionRequest) -> Read<ExecStream> {
        session_controls::exec(self, request)
    }
    fn exec_input(&self, id: String, data: String) -> Read<bool> {
        session_controls::input(self, id, data)
    }
    fn exec_resize(
        &self,
        id: String,
        size: kubecove_lib::models::PodExecTerminalSize,
    ) -> Read<bool> {
        session_controls::resize(self, id, size)
    }
    fn stop_exec(&self, id: String) -> Read<bool> {
        session_controls::stop_exec(self, id)
    }
    fn cleanup_sessions(
        &self,
        request: kubecove_lib::models::LiveSessionCleanupRequest,
    ) -> Read<kubecove_lib::models::LiveSessionCleanupResult> {
        session_controls::cleanup(self, request)
    }
    fn resource_kinds(
        &self,
        context: String,
    ) -> Read<Vec<kubecove_lib::models::DiscoveredResourceKind>> {
        live_data::resource_kinds(self, context)
    }
    fn namespace_custom_kinds(
        &self,
        scope: Scope,
    ) -> Read<Vec<kubecove_lib::models::DiscoveredResourceKind>> {
        live_data::namespace_custom_kinds(self, scope)
    }
    fn kind_resources(&self, scope: Scope, key: WatchResourceKey) -> Read<Vec<ResourceSummary>> {
        live_data::kind_resources(self, scope, key)
    }
    fn watch(&self, target: WatchTarget) -> Read<LogStream> {
        live_data::watch(self, target)
    }

    fn contexts(&self) -> Read<Vec<ClusterContext>> {
        self.read(async { commands::list_kube_contexts(None) })
    }

    fn resources(&self, scope: Scope, refresh: bool) -> Read<ScopeResult> {
        live_data::resources(self, scope, refresh)
    }

    fn scope_builtin_resources(&self, scope: Scope, refresh: bool) -> Read<ScopeResult> {
        live_data::builtin_resources(self, scope, refresh)
    }

    fn scope_custom_resources(&self, scope: Scope) -> Option<Read<ScopeResult>> {
        Some(live_data::custom_resources(self, scope))
    }

    fn details(&self, resource: ResourceSummary) -> Read<ResourceDetailsFull> {
        let cancellations = self.cancellations.clone();
        if let Some(kind) =
            delivery::dynamic_kind(&resource).filter(|_| resource.dynamic == Some(true))
        {
            return self.read(delivery::dynamic_details(resource, kind, cancellations));
        }
        self.read(async move {
            supported(&resource.kind)?;
            commands::get_resource_details(
                resource.cluster,
                resource.kind,
                resource.name,
                resource.namespace,
                None,
                None,
                None,
                State::new(&cancellations),
            )
            .await
        })
    }

    fn application_resources(
        &self,
        application: ArgoApplicationSummary,
        refresh: bool,
    ) -> Read<ScopeResult> {
        self.read(delivery_workspace::resources(
            application,
            refresh,
            self.store.clone(),
        ))
    }

    fn topology(&self, scope: Scope) -> Read<ResourceTopology> {
        topology_metrics::topology(self, scope, "ownership")
    }

    fn network_topology(&self, scope: Scope) -> Read<ResourceTopology> {
        topology_metrics::topology(self, scope, "networkFlow")
    }

    fn app_usage(&self) -> Read<kubecove_lib::models::AppUsageMetrics> {
        topology_metrics::app_usage(self)
    }

    fn metrics(&self, scope: Scope) -> Read<kubecove_lib::models::ResourceMetricsSummary> {
        topology_metrics::metrics(self, scope)
    }

    fn gitops(&self, context: String) -> Read<GitOpsInventory> {
        self.read(inventory::gitops(context))
    }

    fn cluster_resources(&self, context: String, refresh: bool) -> Read<ScopeResult> {
        self.read(inventory::cluster_resources(
            context,
            refresh,
            self.store.clone(),
        ))
    }

    fn events(
        &self,
        resource: ResourceSummary,
        uid: Option<String>,
    ) -> Read<Vec<ResourceEventSummary>> {
        let cancellations = self.cancellations.clone();
        self.read(async move {
            commands::list_resource_events(
                resource.cluster,
                resource.kind,
                resource.name,
                resource.namespace,
                uid,
                None,
                None,
                None,
                State::new(&cancellations),
            )
            .await
        })
    }

    fn incidents(&self, context: String) -> Read<IncidentCockpitSummary> {
        let requests = crate::incidents::workspace_requests();
        let store = self.store.clone();
        self.read(async move {
            commands::list_incident_cockpit(context, requests, None, State::new(&store)).await
        })
    }

    fn live_sessions(&self) -> Read<LiveSessions> {
        let (port_forwards, exec_sessions) =
            (self.port_forwards.clone(), self.exec_sessions.clone());
        self.read(async move {
            let access = ClusterAccess::resolve(None)?;
            Ok(LiveSessions {
                port_forwards: commands::list_port_forwards(State::new(&port_forwards)).await?,
                exec_sessions: commands::list_pod_exec_sessions(State::new(&exec_sessions)).await?,
                source_key: Some(access.source_key().to_owned()),
                show_source_labels: access.source_label().is_some(),
            })
        })
    }

    fn log_stream(&self, target: LogTarget) -> Read<LogStream> {
        detail_reads::log_stream(self, target)
    }

    fn deployment_revisions(&self, resource: ResourceSummary) -> Read<Vec<DeploymentRevision>> {
        self.read(async move {
            let namespace = resource.namespace.unwrap_or_default();
            commands::list_deployment_revisions(resource.cluster, resource.name, namespace, None)
                .await
        })
    }

    fn action_state(&self, request: ResourceActionStateRequest) -> Read<ResourceActionState> {
        self.read(commands::get_resource_action_state(request))
    }

    fn preview_action(&self, request: ResourceActionRequest) -> Read<ResourceActionPreview> {
        self.read(commands::preview_resource_action(request))
    }

    fn execute_action(&self, request: ResourceActionRequest) -> Read<ClusterOperationResult> {
        self.read(commands::execute_resource_action(request))
    }

    fn resource_yaml(&self, resource: ResourceSummary, mode: YamlViewMode) -> Read<String> {
        self.resource_yaml_encoded(resource, mode, kubecove_lib::models::YamlEncoding::Yaml)
    }

    fn resource_yaml_encoded(
        &self,
        resource: ResourceSummary,
        mode: YamlViewMode,
        encoding: kubecove_lib::models::YamlEncoding,
    ) -> Read<String> {
        detail_reads::resource_yaml(self, resource, mode, encoding)
    }

    fn prepare_yaml_apply(&self, request: YamlApplyRequest) -> Read<YamlApplyPreview> {
        self.read(commands::prepare_yaml_apply(request))
    }

    fn apply_yaml(&self, request: YamlApplyRequest) -> Read<YamlApplyResult> {
        self.read(commands::apply_yaml(request))
    }

    fn lint_yaml(&self, request: YamlApplyRequest) -> Read<KubernetesYamlLintResult> {
        self.read(commands::lint_kubernetes_yaml(request))
    }

    fn namespaces(&self, context: String) -> Read<Vec<NamespaceSummary>> {
        let store = self.store.clone();
        self.read(async move {
            commands::list_namespaces(context, None, State::new(&store))
                .await
                .map(Arc::unwrap_or_clone)
        })
    }

    fn rbac(&self, context: String) -> Read<RbacInspectionSummary> {
        self.read(security::rbac(context, self.cancellations.clone()))
    }

    fn review_rbac(&self, request: RbacAccessReviewRequest) -> Read<RbacAccessReviewResult> {
        self.read(security::review(request, self.cancellations.clone()))
    }

    fn kubeconfig_sources(&self) -> Read<KubeconfigSourcesSummary> {
        self.read(security::kubeconfig_sources())
    }

    fn kubeconfig_writes_unavailable(&self) -> Option<&'static str> {
        (!self.settings_initialized).then_some(security::KUBECONFIG_WRITES)
    }

    fn kubeconfig_source_edits_unavailable(&self) -> Option<&'static str> {
        (!self.settings_initialized).then_some(security::KUBECONFIG_WRITES)
    }

    fn set_kubeconfig_env_var(&self, value: String) -> Read<KubeconfigSourcesSummary> {
        if !self.settings_initialized {
            return self.read(security::unsaved());
        }
        self.read(async move { commands::set_kubeconfig_env_var(value) })
    }

    fn set_show_kubeconfig_source_labels(&self, show: bool) -> Read<KubeconfigSourcesSummary> {
        if !self.settings_initialized {
            return self.read(security::unsaved());
        }
        self.read(async move { commands::set_show_kubeconfig_source_labels(show) })
    }

    fn pick_kubeconfig_paths(&self) -> Read<KubeconfigSourcesSummary> {
        if !self.settings_initialized {
            return self.read(security::unsaved());
        }
        self.pick_kubeconfig()
    }
    fn remove_kubeconfig_path(&self, path: String) -> Read<KubeconfigSourcesSummary> {
        if !self.settings_initialized {
            return self.read(security::unsaved());
        }
        self.read(async move { commands::remove_kubeconfig_path(path) })
    }
    fn reorder_kubeconfig_paths(&self, paths: Vec<String>) -> Read<KubeconfigSourcesSummary> {
        if !self.settings_initialized {
            return self.read(security::unsaved());
        }
        self.read(async move { commands::reorder_kubeconfig_paths(paths) })
    }
    fn check_updates(&self) -> Read<UpdateStatus> {
        self.read(settings_integrations::check_updates())
    }
    fn import_workspace(&self) -> Read<Option<String>> {
        self.read(workspace_files::import())
    }
    fn export_workspace(&self, name: String, content: String) -> Read<bool> {
        self.read(workspace_files::export(name, content))
    }

    fn helm_releases(&self, context: String) -> Read<HelmReleaseList> {
        self.read(commands::list_helm_releases(context, None))
    }

    fn helm_release(&self, release: HelmReleaseSummary) -> Read<HelmReleaseDetails> {
        self.read(delivery::helm_release(release))
    }

    fn helm_reconciliation(&self, release: HelmReleaseSummary) -> Read<HelmReleaseReconciliation> {
        self.read(delivery::helm_reconciliation(release))
    }

    fn argo_inspector(&self, application: ArgoApplicationRef) -> Read<ArgoApplicationInspector> {
        let (store, cancellations) = (self.argo_connections.clone(), self.cancellations.clone());
        self.read(delivery::argo_inspector(application, store, cancellations))
    }
}
