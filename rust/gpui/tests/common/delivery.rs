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
use std::collections::HashMap;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitOps {
    argo: Option<Vec<ArgoApplicationSummary>>,
    flux: Option<Vec<FluxResourceSummary>>,
    app_sets: Vec<ArgoApplicationSetSummary>,
    projects: Vec<ArgoAppProjectSummary>,
    applications: Vec<Application>,
    many_argo: Vec<ArgoApplicationSummary>,
    many_applications: Vec<Application>,
    /// The "same-name" inventory: a second tenant-catalog in another namespace.
    same_name_argo: Vec<ArgoApplicationSummary>,
    same_name_applications: Vec<Application>,
}

#[derive(Deserialize)]
struct Application {
    details: ResourceDetailsFull,
    inspector: ArgoApplicationInspector,
}

#[derive(Deserialize)]
pub struct Helm {
    list: HelmReleaseList,
    /// Each context's list, so a release opened from another workspace context
    /// reads its own context.
    lists: HashMap<String, HelmReleaseList>,
    details: Vec<HelmReleaseDetails>,
    reconciliations: Vec<HelmReleaseReconciliation>,
}

fn ready<T: Send + 'static>(found: Option<T>, what: &str) -> Read<T> {
    let missing = format!("No fixture {what}");
    Box::pin(async move { found.ok_or_else(|| AppError::new(missing, AppErrorKind::NotFound)) })
}

fn same_release(a: &HelmReleaseSummary, b: &HelmReleaseSummary) -> bool {
    a.cluster == b.cluster && a.namespace == b.namespace && a.storage_name == b.storage_name
}

impl GitOps {
    pub fn inventory(&self, mode: &str) -> Read<GitOpsInventory> {
        let inventory = GitOpsInventory {
            argo: match mode {
                "many" => Some(self.many_argo.clone()),
                "same-name" => Some(self.same_name_argo.clone()),
                _ => self.argo.clone(),
            },
            flux: self.flux.clone(),
            app_sets: self.app_sets.clone(),
            projects: self.projects.clone(),
            ..Default::default()
        };
        Box::pin(async move { Ok(inventory) })
    }

    fn applications(&self, mode: &str) -> &[Application] {
        match mode {
            "many" => &self.many_applications,
            "same-name" => &self.same_name_applications,
            _ => &self.applications,
        }
    }

    /// An Application's details, which the GitOps page's Details button inspects.
    pub fn application_details(
        &self,
        resource: &ResourceSummary,
        mode: &str,
    ) -> Option<ResourceDetailsFull> {
        self.applications(mode)
            .iter()
            .map(|app| &app.details)
            .find(|details| {
                details.summary.kind == resource.kind
                    && details.summary.name == resource.name
                    && details.summary.namespace == resource.namespace
            })
            .cloned()
    }

    pub fn inspector(
        &self,
        application: ArgoApplicationRef,
        mode: &str,
    ) -> Read<ArgoApplicationInspector> {
        let found = self
            .applications(mode)
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
    /// `list_helm_releases` for `context`; a context the export did not list reads the primary list.
    pub fn releases(&self, context: &str) -> Read<HelmReleaseList> {
        let list = self.lists.get(context).unwrap_or(&self.list).clone();
        ready(Some(list), "Helm releases")
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
