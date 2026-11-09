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

/// A whole-kind read narrowed to the rows a watch notice named.
fn named_rows(read: Read<KindRows>, names: Vec<(Option<String>, String)>) -> Read<KindRows> {
    Box::pin(async move {
        let mut read = read.await?;
        let names = live_data::NameSet::new(names);
        read.rows.retain(|row| names.contains(row));
        Ok(read)
    })
}

/// `list_namespaces` for a context with the selected kubeconfig source, as the web passes it.
/// A source that cannot be read falls back to the default source, as the contexts read does.
pub fn namespaces_for_context(
    backend: Arc<dyn Backend>,
    context: String,
) -> Read<Vec<NamespaceSummary>> {
    Box::pin(async move {
        let source = backend
            .kubeconfig_sources()
            .await
            .ok()
            .map(|sources| sources.source_key);
        backend.namespaces(context, source).await
    })
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scope {
    pub context: String,
    pub namespace: Option<String>,
}

#[derive(Clone, Default)]
pub struct ScopeResult {
    pub namespaces: Vec<NamespaceSummary>,
    pub resources: Vec<ResourceSummary>,
    pub warnings: Vec<String>,
    /// The list version of each kind's rows, by `live_data::watch_signature` of its
    /// watch key. A view starts that watch at this version.
    pub versions: ListVersions,
    /// The lists this read completed. A row missing from a completed list is
    /// gone; one missing from a failed or unread list may still exist.
    pub listed: Vec<Listed>,
}

/// One completed list, as the web's `ResourceEntryPointCoverage` request: a
/// built-in key has no API version and covers every version of its kind.
#[derive(Clone)]
pub struct Listed {
    pub context: String,
    pub key: WatchResourceKey,
}

pub type ListVersions = std::collections::HashMap<String, String>;

/// One kind's rows and the version of the list they came from.
#[derive(Default)]
pub struct KindRows {
    pub rows: Vec<ResourceSummary>,
    pub resource_version: Option<String>,
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
    /// Signs in to the selected OIDC server through the system browser (ADR 0031).
    fn remote_sign_in(&self) -> Read<Option<RemoteConnectionSummary>> {
        settings_integrations::unavailable("Remote host adapter unavailable")
    }
    fn remote_cancel_sign_in(&self) -> Read<()> {
        Box::pin(async { Ok(()) })
    }
    fn remote_sign_in_link(&self) -> Read<Option<String>> {
        Box::pin(async { Ok(None) })
    }
    fn remote_start_new_session(&self) -> Read<RemoteConnectionSummary> {
        settings_integrations::unavailable("Remote host adapter unavailable")
    }
    fn remote_sign_out(&self) -> Read<RemoteConnectionSummary> {
        settings_integrations::unavailable("Remote host adapter unavailable")
    }
    /// Changes whenever a remote call fails, so the client reads the session
    /// state again and shows its notice.
    fn remote_failures(&self) -> u64 {
        0
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
    fn kind_resources(
        &self,
        scope: Scope,
        key: WatchResourceKey,
        recycled: Vec<ResourceSummary>,
    ) -> Read<KindRows> {
        let read = self.resources(scope, false);
        Box::pin(async move {
            let mut result = read.await?;
            let resource_version = result.versions.remove(&watch_signature(&key));
            let rows = result
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
                .collect();
            Ok(KindRows {
                rows: live_data::reuse_row_buffer(recycled, rows),
                resource_version,
            })
        })
    }
    /// The rows of `key` that a watch notice named, at a version that shows
    /// the noticed changes. A named row the read leaves out was deleted.
    fn named_kind_resources(
        &self,
        scope: Scope,
        key: WatchResourceKey,
        names: Vec<(Option<String>, String)>,
        recycled: Vec<ResourceSummary>,
    ) -> Read<KindRows> {
        named_rows(self.kind_resources(scope, key, recycled), names)
    }
    fn watch(&self, _target: WatchTarget) -> Read<LogStream> {
        Box::pin(async {
            Err(AppError::new(
                "Live updates unavailable",
                AppErrorKind::UnsupportedOperation,
            ))
        })
    }
    /// `list_kube_contexts` for the kubeconfig source key the web passes, or the default source.
    fn contexts(&self, kubeconfig_source_key: Option<String>) -> Read<Vec<ClusterContext>>;
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
    /// The ownership graph for the scope's namespaces (all when unscoped). It is
    /// shared with the backend's topology cache rather than copied.
    fn topology(&self, scope: Scope) -> Read<Arc<ResourceTopology>>;
    fn network_topology(&self, _scope: Scope) -> Read<Arc<ResourceTopology>> {
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
    /// `list_namespaces` for a context, with the kubeconfig source the web passes to it.
    fn namespaces(
        &self,
        context: String,
        kubeconfig_source_key: Option<String>,
    ) -> Read<Vec<NamespaceSummary>>;
    /// `list_rbac_inspection` for the RBAC Cockpit.
    fn rbac(&self, context: String) -> Read<RbacInspectionSummary>;
    /// `review_rbac_access`: one SubjectAccessReview, run only on an explicit user action.
    fn review_rbac(&self, request: RbacAccessReviewRequest) -> Read<RbacAccessReviewResult>;
    /// `get_kubeconfig_sources`.
    fn kubeconfig_sources(&self) -> Read<KubeconfigSourcesSummary>;
    /// Why file picks are unavailable, or `None` when they are. Picks read the desktop's
    /// filesystem, so remote mode offers typed server paths instead.
    fn kubeconfig_writes_unavailable(&self) -> Option<&'static str>;
    /// Why the environment variable, typed paths, removals, reordering and labels would not
    /// persist, or `None` when they do. Remote mode saves all of them on the server.
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
pub use live_data::{WatchTarget, discovered_key, watch_signature};
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
mod native_reads;
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
            // Watch events update the cached lists, so a notice reads its rows
            // from the cache instead of listing the kind again.
            store: ClusterLiveStore::default()
                .applying_watch_rows(commands::watched_resource_summary),
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
