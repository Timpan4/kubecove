//! The scope in `workspaceShellModel.ts::GITOPS_RESOURCE_KINDS`, read through
//! the existing typed commands. Partial permission failures preserve usable rows.

use super::{ListVersions, ScopeResult};
use kubecove_lib::{
    commands::{self, ClusterAccess, ClusterLiveStore},
    models::{AppError, AppErrorKind, ArgoApplicationSummary, WatchResourceKey, WatchResourceKind},
    runtime::State,
};
use std::sync::Arc;

pub const APPLICATION_KINDS: &[&str] = &[
    "Pod",
    "Deployment",
    "ReplicaSet",
    "StatefulSet",
    "DaemonSet",
    "Service",
    "Ingress",
    "ConfigMap",
    "Secret",
    "PersistentVolumeClaim",
    "Job",
    "CronJob",
    "Node",
    "StorageClass",
    "PersistentVolume",
    "CustomResourceDefinition",
];

pub async fn resources(
    app: ArgoApplicationSummary,
    refresh: bool,
    store: ClusterLiveStore,
) -> Result<ScopeResult, AppError> {
    let mut warnings = Vec::new();
    let namespaces =
        match commands::list_namespaces(app.cluster.clone(), None, State::new(&store)).await {
            Ok(rows) => Arc::unwrap_or_clone(rows),
            Err(error) if error.kind == AppErrorKind::Forbidden => {
                warnings.push(format!("Namespaces: {}", error.message));
                Vec::new()
            }
            Err(error) => return Err(error),
        };
    let mut targets = app.resource_namespaces;
    if targets.is_empty()
        && let Some(namespace) = app.destination_namespace.filter(|ns| !ns.is_empty())
    {
        targets.push(namespace);
    }
    targets.sort();
    targets.dedup();
    let namespace_scopes: Vec<_> = if targets.is_empty() {
        vec![None]
    } else {
        targets.into_iter().map(Some).collect()
    };
    let discovered =
        match commands::list_resource_kinds(app.cluster.clone(), None, State::new(&store)).await {
            Ok(kinds) => Arc::unwrap_or_clone(kinds),
            Err(error) => {
                warnings.push(format!("Custom Resources: {}", error.message));
                Vec::new()
            }
        };
    let mut requests = Vec::new();
    for kind in APPLICATION_KINDS {
        let cluster_scoped = super::CLUSTER_KINDS.contains(kind);
        let scopes = if cluster_scoped {
            &[None][..]
        } else {
            &namespace_scopes
        };
        for namespace in scopes {
            requests.push((
                WatchResourceKind {
                    kind: (*kind).into(),
                    namespaced: Some(!cluster_scoped),
                    group: None,
                    version: None,
                    api_version: None,
                    plural: None,
                },
                namespace.clone(),
                None,
            ));
        }
    }
    for kind in discovered {
        let scopes = if kind.namespaced {
            &namespace_scopes
        } else {
            &[None][..]
        };
        for namespace in scopes {
            requests.push((
                WatchResourceKind {
                    kind: kind.kind.clone(),
                    group: Some(kind.group.clone()),
                    version: Some(kind.version.clone()),
                    api_version: Some(kind.api_version.clone()),
                    plural: Some(kind.plural.clone()),
                    namespaced: Some(kind.namespaced),
                },
                namespace.clone(),
                Some(kind.clone()),
            ));
        }
    }
    if refresh {
        let access = ClusterAccess::resolve(None)?;
        let keys = requests
            .iter()
            .map(|(resource_kind, namespace, _)| WatchResourceKey {
                resource_kind: resource_kind.clone(),
                namespace: namespace.clone(),
            })
            .collect::<Vec<_>>();
        let targets = namespace_scopes
            .iter()
            .flatten()
            .cloned()
            .collect::<Vec<_>>();
        store.refresh_view(access.source_key(), &app.cluster, &keys, &targets);
    }
    let mut resources = Vec::new();
    let mut versions = ListVersions::new();
    for (kind, namespace, dynamic) in requests {
        // Each request lists one watch key, so its version backs exactly the rows
        // a watch on that key shows. A view that watches every namespace of
        // several listed ones finds no version under its key and starts without one.
        let key = WatchResourceKey {
            resource_kind: kind.clone(),
            namespace: namespace.clone(),
        };
        // As in `live_data.rs`: a built-in kind fails the read unless access is
        // denied, so `read_retry` retries it; a custom kind only warns.
        let custom = dynamic.is_some();
        let result = if let Some(kind) = dynamic {
            commands::list_dynamic_resource_snapshot(
                app.cluster.clone(),
                kind,
                namespace,
                None,
                State::new(&store),
            )
            .await
        } else {
            commands::list_resource_snapshot(
                app.cluster.clone(),
                kind.kind.clone(),
                namespace,
                None,
                State::new(&store),
            )
            .await
        };
        match result {
            Ok(snapshot) => {
                resources.extend(Arc::unwrap_or_clone(snapshot.rows));
                if let Some(version) = snapshot.resource_version {
                    versions.insert(super::watch_signature(&key), version);
                }
            }
            Err(error) if custom || error.kind == AppErrorKind::Forbidden => {
                warnings.push(format!("{}: {}", kind.kind, error.message));
            }
            Err(error) => return Err(error),
        }
    }
    Ok(ScopeResult {
        namespaces,
        resources,
        warnings,
        versions,
        listed: Vec::new(),
    })
}
