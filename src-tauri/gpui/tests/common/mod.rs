//! The browser-mock fixture both shared-test drivers render. `e2e/parity/visual/export-fixture.ts`
//! exports the web's `tauri-dev-mocks` responses, so the GPUI client sees the same data the
//! Svelte client does in the visual comparison and the parity scenarios.

use kubecove_gpui::{
    backend::{Backend, CLUSTER_KINDS, LiveSessions, Read, Scope, ScopeResult},
    gitops::GitOpsInventory,
};
use kubecove_lib::models::{
    AppError, AppErrorKind, ArgoApplicationSummary, ClusterContext, FluxResourceSummary,
    IncidentCockpitSummary, NamespaceSummary, PodExecSessionSummary, PortForwardSessionSummary,
    ResourceDetailsFull, ResourceEventSummary, ResourceSummary, ResourceTopology,
};
use serde::Deserialize;
use std::{collections::HashMap, path::Path};

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
        let resources = self
            .resources
            .iter()
            .filter(|row| !CLUSTER_KINDS.contains(&row.kind.as_str()))
            .filter(|row| {
                scope.namespace.is_none() || row.namespace.as_deref() == scope.namespace.as_deref()
            })
            .cloned()
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
}
