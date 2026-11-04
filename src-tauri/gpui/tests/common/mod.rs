//! The browser-mock fixture both shared-test drivers render. `e2e/parity/visual/export-fixture.ts`
//! exports the web's `tauri-dev-mocks` responses, so the GPUI client sees the same data the
//! Svelte client does in the visual comparison and the parity scenarios.

mod detail_tabs;

use detail_tabs::DetailTabs;
use kubecove_gpui::{
    backend::{
        Backend, CLUSTER_KINDS, KubeconfigSourcesSummary, LiveSessions, LogStream, LogTarget, Read,
        Scope, ScopeResult,
    },
    gitops::GitOpsInventory,
};
use kubecove_lib::models::{
    AppError, AppErrorKind, ArgoApplicationSummary, ClusterContext, ClusterOperationResult,
    DeploymentRevision, FluxResourceSummary, IncidentCockpitSummary, NamespaceSummary,
    PodExecSessionSummary, PortForwardSessionSummary, RbacAccessReviewRequest,
    RbacAccessReviewResult, RbacAccessReviewTarget, RbacInspectionSummary, ResourceActionPreview,
    ResourceActionRequest, ResourceActionState, ResourceActionStateRequest, ResourceDetailsFull,
    ResourceEventSummary, ResourceSummary, ResourceTopology, YamlApplyPreview, YamlApplyRequest,
    YamlApplyResult, YamlViewMode,
};
use serde::Deserialize;
use std::{collections::HashMap, path::Path, sync::Mutex};

#[derive(Deserialize)]
pub struct Fixture {
    #[serde(rename = "contexts")]
    cluster_contexts: Vec<ClusterContext>,
    namespaces: Vec<NamespaceSummary>,
    resources: Vec<ResourceSummary>,
    details: Vec<ResourceDetailsFull>,
    events: Vec<ResourceEvents>,
    topology: ResourceTopology,
    gitops: GitOps,
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
struct GitOps {
    argo: Option<Vec<ArgoApplicationSummary>>,
    flux: Option<Vec<FluxResourceSummary>>,
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
    pub fn load(path: &Path) -> Self {
        let bytes = std::fs::read(path)
            .unwrap_or_else(|error| panic!("fixture {} unreadable: {error}", path.display()));
        serde_json::from_slice(&bytes).expect("fixture must match the Rust models")
    }
}

impl Backend for Fixture {
    fn contexts(&self) -> Read<Vec<ClusterContext>> {
        let contexts = self.cluster_contexts.clone();
        Box::pin(async move { Ok(contexts) })
    }
    fn resources(&self, scope: Scope, _: bool) -> Read<ScopeResult> {
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
            .cloned();
        Box::pin(async move {
            found.ok_or_else(|| AppError::new("No fixture details", AppErrorKind::NotFound))
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
    fn gitops(&self, _: String) -> Read<GitOpsInventory> {
        let inventory = GitOpsInventory {
            argo: self.gitops.argo.clone(),
            flux: self.gitops.flux.clone(),
        };
        Box::pin(async move { Ok(inventory) })
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
            port_forwards: sessions.port_forwards.clone(),
            exec_sessions: sessions.exec_sessions.clone(),
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
    /// The browser mocks serve one YAML shape for every view mode, as `details` holds it.
    fn resource_yaml(&self, resource: ResourceSummary, _: YamlViewMode) -> Read<String> {
        let details = self.details(resource);
        Box::pin(async move {
            let details = details.await?;
            if details.summary.dynamic == Some(true) {
                return Err(AppError::new(
                    format!("unsupported resource kind: {}", details.summary.kind),
                    AppErrorKind::UnsupportedOperation,
                ));
            }
            Ok(details.yaml)
        })
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
        Box::pin(async move { Ok(sources) })
    }
    fn set_show_kubeconfig_source_labels(&self, show: bool) -> Read<KubeconfigSourcesSummary> {
        let mut sources = self.kubeconfig.lock().expect("kubeconfig lock");
        sources.show_source_labels = show;
        let sources = sources.clone();
        Box::pin(async move { Ok(sources) })
    }
}
