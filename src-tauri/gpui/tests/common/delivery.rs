//! The GitOps and Helm sections of the browser-mock fixture: Argo CD and Flux
//! inventory, each Application's details and Kubernetes-transport inspector, and
//! Helm releases with their details and reconciliation.

use kubecove_gpui::{backend::Read, gitops::GitOpsInventory};
use kubecove_lib::models::{
    AppError, AppErrorKind, ArgoAppProjectSummary, ArgoApplicationInspector, ArgoApplicationRef,
    ArgoApplicationSetSummary, ArgoApplicationSummary, FluxResourceSummary, HelmReleaseDetails,
    HelmReleaseList, HelmReleaseReconciliation, HelmReleaseSummary, ResourceDetailsFull,
    ResourceSummary,
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitOps {
    argo: Option<Vec<ArgoApplicationSummary>>,
    flux: Option<Vec<FluxResourceSummary>>,
    app_sets: Vec<ArgoApplicationSetSummary>,
    projects: Vec<ArgoAppProjectSummary>,
    applications: Vec<Application>,
}

#[derive(Deserialize)]
struct Application {
    details: ResourceDetailsFull,
    inspector: ArgoApplicationInspector,
}

#[derive(Deserialize)]
pub struct Helm {
    list: HelmReleaseList,
    details: Vec<HelmReleaseDetails>,
    reconciliations: Vec<HelmReleaseReconciliation>,
}

fn ready<T: Send + 'static>(found: Option<T>, what: &str) -> Read<T> {
    let missing = format!("No fixture {what}");
    Box::pin(async move { found.ok_or_else(|| AppError::new(missing, AppErrorKind::NotFound)) })
}

fn same_release(a: &HelmReleaseSummary, b: &HelmReleaseSummary) -> bool {
    a.namespace == b.namespace && a.storage_name == b.storage_name
}

impl GitOps {
    pub fn inventory(&self) -> Read<GitOpsInventory> {
        let inventory = GitOpsInventory {
            argo: self.argo.clone(),
            flux: self.flux.clone(),
            app_sets: self.app_sets.clone(),
            projects: self.projects.clone(),
            list_error: None,
        };
        Box::pin(async move { Ok(inventory) })
    }

    /// An Application's details, which the GitOps page's Details button inspects.
    pub fn application_details(&self, resource: &ResourceSummary) -> Option<ResourceDetailsFull> {
        self.applications
            .iter()
            .map(|app| &app.details)
            .find(|details| {
                details.summary.kind == resource.kind
                    && details.summary.name == resource.name
                    && details.summary.namespace == resource.namespace
            })
            .cloned()
    }

    pub fn inspector(&self, application: ArgoApplicationRef) -> Read<ArgoApplicationInspector> {
        let found = self
            .applications
            .iter()
            .find(|app| {
                app.details.summary.name == application.name
                    && app.details.summary.namespace == application.namespace
            })
            .map(|app| app.inspector.clone());
        ready(found, "Argo CD inspector")
    }
}

impl Helm {
    pub fn releases(&self) -> Read<HelmReleaseList> {
        ready(Some(self.list.clone()), "Helm releases")
    }

    pub fn release(&self, release: HelmReleaseSummary) -> Read<HelmReleaseDetails> {
        let found = self
            .details
            .iter()
            .find(|details| same_release(&details.summary, &release))
            .cloned();
        ready(found, "Helm release")
    }

    pub fn reconciliation(&self, release: HelmReleaseSummary) -> Read<HelmReleaseReconciliation> {
        let found = self
            .reconciliations
            .iter()
            .find(|row| same_release(&row.summary, &release))
            .cloned();
        ready(found, "Helm reconciliation")
    }
}
