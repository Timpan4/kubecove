//! Native reads for the GitOps inventory and the Cluster Overview's
//! cluster-scoped kinds. Both use the Kubernetes API through the shared commands.

use super::ScopeResult;
use crate::gitops::GitOpsInventory;
use kubecove_lib::{
    commands::{self, ClusterAccess, ClusterLiveStore},
    models::{AppError, AppErrorKind, WatchResourceKey, WatchResourceKind},
    runtime::State,
};
use std::sync::Arc;

/// `CLUSTER_SCOPED_KINDS` in `src/lib/types.ts`, the kinds `resolveTreeScope`
/// gives the Cluster Overview section.
pub const CLUSTER_KINDS: &[&str] = &[
    "Node",
    "StorageClass",
    "PersistentVolume",
    "CustomResourceDefinition",
];

/// The `detectArgoCD`, `listArgoApplications`, `listArgoApplicationSets`,
/// `listArgoAppProjects`, `detectFlux` and `listFluxResources` queries, read from CRDs.
pub(super) async fn gitops(context: String) -> Result<GitOpsInventory, AppError> {
    let mut inventory = GitOpsInventory::default();
    let mut errors = Vec::new();
    match commands::detect_argocd(context.clone(), None).await {
        Ok(true) => {
            inventory.argo = Some(
                match commands::list_argocd_applications(context.clone(), None).await {
                    Ok(rows) => rows,
                    Err(error) => {
                        errors.push(error);
                        Vec::new()
                    }
                },
            );
            match commands::list_argocd_appsets(context.clone(), None).await {
                Ok(rows) => inventory.app_sets = rows,
                Err(error) => errors.push(error),
            }
            match commands::list_argocd_appprojects(context.clone(), None).await {
                Ok(rows) => inventory.projects = rows,
                Err(error) => errors.push(error),
            }
        }
        Ok(false) => {}
        Err(error) => errors.push(error),
    }
    match commands::detect_flux(context.clone(), None).await {
        Ok(detection) if detection.detected => {
            let mut rows = Vec::new();
            for kind in detection.kinds {
                match commands::list_flux_resources(context.clone(), kind, None).await {
                    Ok(resources) => rows.extend(resources),
                    Err(error) => errors.push(error),
                }
            }
            inventory.flux = Some(rows);
        }
        Ok(_) => {}
        Err(error) => errors.push(error),
    }
    if !inventory.detected() {
        if let Some(error) = errors.into_iter().next() {
            return Err(error);
        }
    } else if let Some(kind) = errors.first().map(|error| error.kind) {
        // The first failing read sets the kind, as the web's first failing query does.
        let message = errors
            .iter()
            .map(|error| error.message.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        inventory.list_error = Some(AppError::new(message, kind));
    }
    Ok(inventory)
}

/// Lists the cluster-scoped kinds. A forbidden kind becomes a warning, like
/// the workspace scope read.
pub(super) async fn cluster_resources(
    context: String,
    refresh: bool,
    store: ClusterLiveStore,
) -> Result<ScopeResult, AppError> {
    if refresh {
        let access = ClusterAccess::resolve(None)?;
        let keys: Vec<_> = CLUSTER_KINDS
            .iter()
            .map(|kind| WatchResourceKey {
                resource_kind: WatchResourceKind {
                    kind: (*kind).to_owned(),
                    group: None,
                    version: None,
                    api_version: None,
                    plural: None,
                    namespaced: Some(false),
                },
                namespace: None,
            })
            .collect();
        store.refresh_view(access.source_key(), &context, &keys, &[]);
    }
    let mut resources = Vec::new();
    let mut warnings = Vec::new();
    for kind in CLUSTER_KINDS {
        match commands::list_resources(
            context.clone(),
            (*kind).to_owned(),
            None,
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
        namespaces: Vec::new(),
        resources,
        warnings,
    })
}
