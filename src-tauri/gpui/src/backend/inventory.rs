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

/// The overview's `detectArgoCD`/`listArgoApplications` and
/// `detectFlux`/`listFluxResources` queries, read from CRDs.
pub(super) async fn gitops(context: String) -> Result<GitOpsInventory, AppError> {
    let argo = if commands::detect_argocd(context.clone(), None).await? {
        Some(commands::list_argocd_applications(context.clone(), None).await?)
    } else {
        None
    };
    let detection = commands::detect_flux(context.clone(), None).await?;
    let flux = if detection.detected {
        let mut rows = Vec::new();
        for kind in detection.kinds {
            rows.extend(commands::list_flux_resources(context.clone(), kind, None).await?);
        }
        Some(rows)
    } else {
        None
    };
    Ok(GitOpsInventory { argo, flux })
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
