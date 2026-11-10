//! Native reads for the GitOps inventory and the Cluster Overview's
//! cluster-scoped kinds. Both use the Kubernetes API through the shared commands.

use super::ScopeResult;
use crate::gitops::{ARGO_KINDS, GitOpsInventory, argo_watch_key, flux_watch_key};
use kubecove_lib::{
    commands::{self, ClusterAccess, ClusterLiveStore, list_projection::bounded_join_all},
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
    let [applications, app_sets, projects] =
        ARGO_KINDS.map(|(kind, plural)| argo_watch_key(kind, plural));
    // Both providers, and each provider's lists, are read side by side. The
    // results are applied in the old order (Argo CD lists, then Flux kinds), so
    // errors keep their order.
    let argo = async {
        match commands::detect_argocd(context.clone(), None).await {
            Ok(true) => Ok(Some(futures_util::join!(
                commands::list_argocd_applications(context.clone(), None),
                commands::list_argocd_appsets(context.clone(), None),
                commands::list_argocd_appprojects(context.clone(), None),
            ))),
            Ok(false) => Ok(None),
            Err(error) => Err(error),
        }
    };
    let flux = async {
        match commands::detect_flux(context.clone(), None).await {
            Ok(detection) if detection.detected => {
                let reads: Vec<_> = detection
                    .kinds
                    .into_iter()
                    .map(|kind| {
                        let key = flux_watch_key(&kind);
                        let read = commands::list_flux_resources(context.clone(), kind, None);
                        async move { (key, read.await) }
                    })
                    .collect();
                Ok(Some(
                    bounded_join_all(reads, commands::MAX_LIST_CONCURRENCY).await,
                ))
            }
            Ok(_) => Ok(None),
            Err(error) => Err(error),
        }
    };
    let (argo, flux) = futures_util::join!(argo, flux);
    match argo {
        Ok(Some((application_list, app_set_list, project_list))) => {
            inventory.argo = Some(match application_list {
                Ok(list) => inventory.keep(&applications, list),
                Err(error) => {
                    errors.push(error);
                    Vec::new()
                }
            });
            match app_set_list {
                Ok(list) => inventory.app_sets = inventory.keep(&app_sets, list),
                Err(error) => errors.push(error),
            }
            match project_list {
                Ok(list) => inventory.projects = inventory.keep(&projects, list),
                Err(error) => errors.push(error),
            }
        }
        Ok(None) => {}
        Err(error) => errors.push(error),
    }
    match flux {
        Ok(Some(lists)) => {
            let mut rows = Vec::new();
            for (key, list) in lists {
                match list {
                    Ok(list) => rows.extend(inventory.keep(&key, list)),
                    Err(error) => errors.push(error),
                }
            }
            inventory.flux = Some(rows);
        }
        Ok(None) => {}
        Err(error) => errors.push(error),
    }
    with_list_errors(inventory, errors)
}

/// Reports the failed GitOps reads in `errors`. With no provider detected the
/// first failure is the result; otherwise the lists that succeeded stay and
/// `list_error` carries the failures. The remote backend shares this.
pub(super) fn with_list_errors(
    mut inventory: GitOpsInventory,
    errors: Vec<AppError>,
) -> Result<GitOpsInventory, AppError> {
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
    if refresh {
        let access = ClusterAccess::resolve(None)?;
        store.refresh_view(access.source_key(), &context, &keys, &[]);
    }
    let mut resources = Vec::new();
    let mut warnings = Vec::new();
    let mut versions = super::ListVersions::new();
    // The kinds are listed together; results are read in kind order, so the
    // first failing kind still decides the error.
    let reads: Vec<_> = CLUSTER_KINDS
        .iter()
        .map(|kind| {
            commands::list_resource_snapshot(
                context.clone(),
                (*kind).to_owned(),
                None,
                None,
                State::new(&store),
            )
        })
        .collect();
    let results = bounded_join_all(reads, commands::MAX_LIST_CONCURRENCY).await;
    for ((kind, key), result) in CLUSTER_KINDS.iter().zip(&keys).zip(results) {
        match result {
            Ok(snapshot) => {
                resources.extend(Arc::unwrap_or_clone(snapshot.rows));
                if let Some(version) = snapshot.resource_version {
                    versions.insert(super::watch_signature(key), version);
                }
            }
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
        versions,
        listed: Vec::new(),
    })
}
