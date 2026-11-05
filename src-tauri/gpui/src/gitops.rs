//! GitOps inventory for the workspace overview and the GitOps page, mirroring
//! the Argo CD and Flux queries in `WorkspaceOverview.svelte` and
//! `GitOpsSurface.svelte`. It reads Kubernetes CRDs only.

use kubecove_lib::models::{
    ArgoAppProjectSummary, ArgoApplicationSetSummary, ArgoApplicationSummary, FluxResourceSummary,
};

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
    pub list_error: Option<String>,
}

impl GitOpsInventory {
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
