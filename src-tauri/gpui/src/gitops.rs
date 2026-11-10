//! GitOps inventory for the workspace overview and the GitOps page, mirroring
//! the Argo CD and Flux queries in `WorkspaceOverview.svelte` and
//! `GitOpsSurface.svelte`. It reads Kubernetes CRDs only.

use crate::backend::{ListVersions, watch_signature};
use kubecove_lib::models::{
    AppError, ArgoAppProjectSummary, ArgoApplicationSetSummary, ArgoApplicationSummary,
    FluxResourceKind, FluxResourceSummary, VersionedList, WatchResourceKey, WatchResourceKind,
};

/// The Argo CD kinds the GitOps page lists and watches, with their plurals.
pub const ARGO_KINDS: [(&str, &str); 3] = [
    ("Application", "applications"),
    ("ApplicationSet", "applicationsets"),
    ("AppProject", "appprojects"),
];

/// The cluster-wide watch key of an Argo CD kind in `ARGO_KINDS`.
pub fn argo_watch_key(kind: &str, plural: &str) -> WatchResourceKey {
    WatchResourceKey {
        resource_kind: WatchResourceKind {
            kind: kind.into(),
            group: None,
            version: None,
            api_version: Some("argoproj.io/v1alpha1".into()),
            plural: Some(plural.into()),
            namespaced: Some(true),
        },
        namespace: None,
    }
}

/// The cluster-wide watch key of a listed Flux kind.
pub fn flux_watch_key(kind: &FluxResourceKind) -> WatchResourceKey {
    WatchResourceKey {
        resource_kind: WatchResourceKind {
            kind: kind.kind.clone(),
            group: Some(kind.group.clone()),
            version: Some(kind.version.clone()),
            api_version: Some(kind.api_version.clone()),
            plural: Some(kind.plural.clone()),
            namespaced: Some(kind.namespaced),
        },
        namespace: None,
    }
}

#[derive(Clone, Debug, Default)]
pub struct GitOpsInventory {
    /// Applications when the Argo CD Application CRD is installed.
    pub argo: Option<Vec<ArgoApplicationSummary>>,
    /// Resources across the installed Flux kinds when Flux is detected.
    pub flux: Option<Vec<FluxResourceSummary>>,
    /// ApplicationSets; empty unless Argo CD is detected.
    pub app_sets: Vec<ArgoApplicationSetSummary>,
    /// AppProjects; empty unless Argo CD is detected.
    pub projects: Vec<ArgoAppProjectSummary>,
    /// A failed provider read, reported while preserving the other CRD lists.
    pub list_error: Option<AppError>,
    /// The list version of each listed kind, by `watch_signature` of its watch
    /// key. The page starts that kind's watch at this version.
    pub versions: ListVersions,
}

impl GitOpsInventory {
    /// Records the version `list` came from under `key` and returns its rows.
    pub fn keep<T>(&mut self, key: &WatchResourceKey, list: VersionedList<T>) -> Vec<T> {
        if let Some(version) = list.resource_version {
            self.versions.insert(watch_signature(key), version);
        }
        list.items
    }

    /// The version of the rows listed for `key`, or `None` when its list failed
    /// or reported none, so its watch starts without one.
    pub fn version(&self, key: &WatchResourceKey) -> Option<String> {
        self.versions.get(&watch_signature(key)).cloned()
    }

    /// `gitOpsDetected`: either provider is installed.
    pub fn detected(&self) -> bool {
        self.argo.is_some() || self.flux.is_some()
    }

    /// `argoDrift`: applications with a sync status other than `Synced`.
    pub fn out_of_sync(&self) -> usize {
        self.argo
            .iter()
            .flatten()
            .filter(|app| drifted(app))
            .count()
    }
}

fn drifted(app: &ArgoApplicationSummary) -> bool {
    app.sync_status
        .as_deref()
        .is_some_and(|status| !status.is_empty() && status != "Synced")
}
