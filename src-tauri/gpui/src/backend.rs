use crate::gitops::GitOpsInventory;
pub use crate::live_sessions::LiveSessions;
use kubecove_lib::{
    commands::{
        self, BackendCancellationRegistry, ClusterAccess, ClusterLiveStore, PodExecRegistry,
        PortForwardRegistry, StreamRegistry,
    },
    models::{
        AppError, AppErrorKind, ClusterContext, ClusterOperationResult, DeploymentRevision,
        IncidentCockpitSummary, NamespaceSummary, RbacAccessReviewRequest, RbacAccessReviewResult,
        RbacInspectionSummary, ResourceActionPreview, ResourceActionRequest, ResourceActionState,
        ResourceActionStateRequest, ResourceDetailsFull, ResourceEventSummary, ResourceSummary,
        ResourceTopology, WatchResourceKey, WatchResourceKind, YamlApplyPreview, YamlApplyRequest,
        YamlApplyResult, YamlViewMode,
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
    fn contexts(&self) -> Read<Vec<ClusterContext>>;
    fn resources(&self, scope: Scope, refresh: bool) -> Read<ScopeResult>;
    fn details(&self, resource: ResourceSummary) -> Read<ResourceDetailsFull>;
    /// The ownership graph for the scope's namespaces (all when unscoped).
    fn topology(&self, scope: Scope) -> Read<ResourceTopology>;
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
    /// `prepare_yaml_apply`: a server-side apply dry run of a draft.
    fn prepare_yaml_apply(&self, request: YamlApplyRequest) -> Read<YamlApplyPreview>;
    /// `apply_yaml`: applies a draft whose dry run the user reviewed.
    fn apply_yaml(&self, request: YamlApplyRequest) -> Read<YamlApplyResult>;
    /// `list_namespaces` for a context the workspace launcher scopes.
    fn namespaces(&self, context: String) -> Read<Vec<NamespaceSummary>>;
    /// `list_rbac_inspection` for the RBAC Cockpit.
    fn rbac(&self, context: String) -> Read<RbacInspectionSummary>;
    /// `review_rbac_access`: one SubjectAccessReview, run only on an explicit user action.
    fn review_rbac(&self, request: RbacAccessReviewRequest) -> Read<RbacAccessReviewResult>;
    /// `get_kubeconfig_sources`.
    fn kubeconfig_sources(&self) -> Read<KubeconfigSourcesSummary>;
    /// Why kubeconfig source writes would not persist, or `None` when they do.
    fn kubeconfig_writes_unavailable(&self) -> Option<&'static str>;
    /// `set_kubeconfig_env_var`.
    fn set_kubeconfig_env_var(&self, env_var: String) -> Read<KubeconfigSourcesSummary>;
    /// `set_show_kubeconfig_source_labels`.
    fn set_show_kubeconfig_source_labels(&self, show: bool) -> Read<KubeconfigSourcesSummary>;
}

mod detail_reads;
mod security;
pub use detail_reads::{LogStream, LogTarget};
pub use kubecove_lib::commands::kubeconfig::KubeconfigSourcesSummary;
mod inventory;
pub use inventory::CLUSTER_KINDS;

/// The native client reads the default workspace kinds; custom resource
/// discovery is not implemented yet.
pub use crate::model::WORKSPACE_KINDS as INSPECTION_KINDS;

pub struct NativeBackend {
    runtime: Option<tokio::runtime::Runtime>,
    store: ClusterLiveStore,
    cancellations: BackendCancellationRegistry,
    // The native client starts no live sessions yet (ADR 0003), so these stay empty.
    port_forwards: PortForwardRegistry,
    exec_sessions: PodExecRegistry,
    streams: Arc<StreamRegistry>,
}

impl NativeBackend {
    pub fn new() -> std::io::Result<Self> {
        Ok(Self {
            runtime: Some(
                tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()?,
            ),
            store: ClusterLiveStore::default(),
            cancellations: BackendCancellationRegistry::default(),
            port_forwards: PortForwardRegistry::default(),
            exec_sessions: PodExecRegistry::default(),
            streams: Arc::default(),
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
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

fn supported(kind: &str) -> Result<(), AppError> {
    if INSPECTION_KINDS.contains(&kind) || CLUSTER_KINDS.contains(&kind) {
        Ok(())
    } else {
        Err(AppError::new(
            format!("Unsupported inspection kind: {kind}"),
            AppErrorKind::Validation,
        ))
    }
}

impl Backend for NativeBackend {
    fn contexts(&self) -> Read<Vec<ClusterContext>> {
        self.read(async { commands::list_kube_contexts(None) })
    }

    fn resources(&self, scope: Scope, refresh: bool) -> Read<ScopeResult> {
        let store = self.store.clone();
        self.read(async move {
            let kinds = INSPECTION_KINDS.to_vec();
            if refresh {
                let access = ClusterAccess::resolve(None)?;
                let keys: Vec<_> = kinds
                    .iter()
                    .map(|kind| WatchResourceKey {
                        resource_kind: WatchResourceKind {
                            kind: (*kind).to_owned(),
                            group: None,
                            version: None,
                            api_version: None,
                            plural: None,
                            namespaced: Some(true),
                        },
                        namespace: scope.namespace.clone(),
                    })
                    .collect();
                let namespaces: Vec<_> = scope.namespace.iter().cloned().collect();
                store.refresh_view(access.source_key(), &scope.context, &keys, &namespaces);
            }
            let mut warnings = Vec::new();
            let namespaces =
                match commands::list_namespaces(scope.context.clone(), None, State::new(&store))
                    .await
                {
                    Ok(rows) => Arc::unwrap_or_clone(rows),
                    Err(error) if error.kind == AppErrorKind::Forbidden => {
                        warnings.push(format!("Namespaces: {}", error.message));
                        Vec::new()
                    }
                    Err(error) => return Err(error),
                };
            let mut resources = Vec::new();
            for kind in kinds {
                match commands::list_resources(
                    scope.context.clone(),
                    kind.to_owned(),
                    scope.namespace.clone(),
                    None,
                    State::new(&store),
                )
                .await
                {
                    Ok(rows) => resources.extend(Arc::unwrap_or_clone(rows)),
                    Err(error) if error.kind == AppErrorKind::Forbidden => {
                        warnings.push(format!("{kind}: {}", error.message))
                    }
                    Err(error) => return Err(error),
                }
            }
            Ok(ScopeResult {
                namespaces,
                resources,
                warnings,
            })
        })
    }

    fn details(&self, resource: ResourceSummary) -> Read<ResourceDetailsFull> {
        let cancellations = self.cancellations.clone();
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

    fn topology(&self, scope: Scope) -> Read<ResourceTopology> {
        let store = self.store.clone();
        let cancellations = self.cancellations.clone();
        self.read(async move {
            commands::list_resource_topology(
                scope.context,
                scope.namespace.into_iter().collect(),
                Some("ownership".into()),
                None,
                None,
                None,
                State::new(&store),
                State::new(&cancellations),
            )
            .await
            .map(Arc::unwrap_or_clone)
        })
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
            supported(&resource.kind)?;
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
        let cancellations = self.cancellations.clone();
        self.read(async move {
            supported(&resource.kind)?;
            commands::get_resource_yaml(
                resource.cluster,
                resource.kind,
                resource.name,
                resource.namespace,
                None,
                Some(mode),
                None,
                None,
                None,
                State::new(&cancellations),
            )
            .await
        })
    }

    fn prepare_yaml_apply(&self, request: YamlApplyRequest) -> Read<YamlApplyPreview> {
        self.read(commands::prepare_yaml_apply(request))
    }

    fn apply_yaml(&self, request: YamlApplyRequest) -> Read<YamlApplyResult> {
        self.read(commands::apply_yaml(request))
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
        Some(security::KUBECONFIG_WRITES)
    }

    fn set_kubeconfig_env_var(&self, _: String) -> Read<KubeconfigSourcesSummary> {
        self.read(security::unsaved())
    }

    fn set_show_kubeconfig_source_labels(&self, _: bool) -> Read<KubeconfigSourcesSummary> {
        self.read(security::unsaved())
    }
}
