//! GitOps inventory for the workspace overview, mirroring the Argo CD and Flux
//! queries in `src/features/workspaces/WorkspaceOverview.svelte`. It reads
//! Kubernetes CRDs only; connected Argo CD API access (ADR 0013) is out of scope.

use kubecove_lib::models::{ArgoApplicationSummary, FluxResourceSummary};

#[derive(Clone, Debug, Default)]
pub struct GitOpsInventory {
    /// Applications when the Argo CD Application CRD is installed.
    pub argo: Option<Vec<ArgoApplicationSummary>>,
    /// Resources across the installed Flux kinds when Flux is detected.
    pub flux: Option<Vec<FluxResourceSummary>>,
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
