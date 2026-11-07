//! Typed discovery, list and watch commands for native live views.
use super::{
    INSPECTION_KINDS, KindRows, ListVersions, LogStream, NativeBackend, Read, Scope, ScopeResult,
};
use kubecove_lib::{
    commands::{self, ClusterAccess, list_projection::bounded_join_all},
    models::{
        AppErrorKind, DiscoveredResourceKind, ResourceSummary, ResourceWatchStart,
        WatchResourceKey, WatchResourceKind,
    },
    runtime::{Channel, ChannelClosed, State},
};
use std::sync::Arc;
use tokio::sync::mpsc;

/// The owner determines the lifetime of a resource or selected-object event watch.
#[derive(Clone)]
pub enum WatchTarget {
    /// `resource_version` is the version of the rows the owner shows for `key`.
    Resource {
        scope: Scope,
        key: Box<WatchResourceKey>,
        resource_version: Option<String>,
    },
    Events(Box<ResourceSummary>),
}

/// The identity of a watch key: kind, group version, plural and namespace.
/// Views key their watches and their rows' list versions by it.
pub fn watch_signature(key: &WatchResourceKey) -> String {
    format!(
        "{}:{}:{}:{}",
        key.resource_kind.kind,
        key.resource_kind.api_version.as_deref().unwrap_or(""),
        key.resource_kind.plural.as_deref().unwrap_or(""),
        key.namespace.as_deref().unwrap_or("")
    )
}

/// The watch key of a discovered kind in `namespace`, or cluster-wide when the
/// kind is cluster-scoped.
pub fn discovered_key(
    kind: &DiscoveredResourceKind,
    namespace: Option<String>,
) -> WatchResourceKey {
    WatchResourceKey {
        resource_kind: WatchResourceKind {
            kind: kind.kind.clone(),
            group: Some(kind.group.clone()),
            version: Some(kind.version.clone()),
            api_version: Some(kind.api_version.clone()),
            plural: Some(kind.plural.clone()),
            namespaced: Some(kind.namespaced),
        },
        namespace: if kind.namespaced { namespace } else { None },
    }
}

pub(super) fn resource_kinds(
    backend: &NativeBackend,
    context: String,
) -> Read<Vec<DiscoveredResourceKind>> {
    let store = backend.store.clone();
    backend.read(async move {
        commands::list_resource_kinds(context, None, State::new(&store))
            .await
            .map(Arc::unwrap_or_clone)
    })
}

pub(super) fn namespace_custom_kinds(
    backend: &NativeBackend,
    scope: Scope,
) -> Read<Vec<DiscoveredResourceKind>> {
    let store = backend.store.clone();
    backend.read(async move {
        commands::list_present_custom_resource_kinds(
            scope.context,
            scope.namespace.into_iter().collect(),
            None,
            State::new(&store),
        )
        .await
        .map(Arc::unwrap_or_clone)
    })
}

pub(super) fn kind_resources(
    backend: &NativeBackend,
    scope: Scope,
    key: WatchResourceKey,
    mut recycled: Vec<ResourceSummary>,
) -> Read<KindRows> {
    let store = backend.store.clone();
    backend.read(async move {
        let access = ClusterAccess::resolve(None)?;
        store.refresh_resources(
            access.source_key(),
            &scope.context,
            std::slice::from_ref(&key),
        );
        let kind = key.resource_kind;
        let snapshot = if let (Some(group), Some(version), Some(api_version), Some(plural)) =
            (kind.group, kind.version, kind.api_version, kind.plural)
        {
            commands::list_dynamic_resource_snapshot(
                scope.context,
                DiscoveredResourceKind {
                    group,
                    version,
                    api_version,
                    plural,
                    kind: kind.kind,
                    namespaced: kind.namespaced.unwrap_or(true),
                    short_names: Vec::new(),
                },
                key.namespace,
                None,
                State::new(&store),
            )
            .await?
        } else {
            commands::list_resource_snapshot(
                scope.context,
                kind.kind,
                key.namespace,
                None,
                State::new(&store),
            )
            .await?
        };
        let rows = match Arc::try_unwrap(snapshot.rows) {
            Ok(rows) => reuse_row_buffer(recycled, rows),
            Err(rows) => {
                recycled.clone_from(rows.as_ref());
                recycled
            }
        };
        Ok(KindRows {
            rows,
            resource_version: snapshot.resource_version,
        })
    })
}

pub(super) fn reuse_row_buffer(
    mut recycled: Vec<ResourceSummary>,
    rows: Vec<ResourceSummary>,
) -> Vec<ResourceSummary> {
    if recycled.capacity() == 0 {
        return rows;
    }
    recycled.clear();
    recycled.extend(rows);
    recycled
}

pub(super) fn watch(backend: &NativeBackend, target: WatchTarget) -> Read<LogStream> {
    let registry = backend.streams.clone();
    let store = backend.store.clone();
    let runtime = backend
        .runtime
        .as_ref()
        .expect("runtime exists until drop")
        .handle()
        .clone();
    backend.read(async move {
        let (sender, messages) = mpsc::unbounded_channel();
        let channel = Channel::new(move |message| sender.send(message).map_err(|_| ChannelClosed));
        let id = match target {
            WatchTarget::Resource {
                scope,
                key,
                resource_version,
            } => {
                commands::start_resource_watch(
                    scope.context,
                    vec![ResourceWatchStart {
                        key: *key,
                        resource_version,
                    }],
                    None,
                    channel,
                    State::new(&registry),
                    State::new(&store),
                )
                .await?
            }
            WatchTarget::Events(resource) => {
                commands::start_resource_event_watch(
                    resource.cluster,
                    resource.kind,
                    resource.name,
                    resource.namespace,
                    None,
                    channel,
                    State::new(&registry),
                )
                .await?
            }
        };
        Ok(LogStream::new(
            messages,
            super::detail_reads::StopStream {
                id,
                registry,
                runtime,
            },
        ))
    })
}

/// Built-in resources and namespaces. The web stages these before custom kinds.
pub(super) fn resources(backend: &NativeBackend, scope: Scope, refresh: bool) -> Read<ScopeResult> {
    let builtin = builtin_resources(backend, scope.clone(), refresh);
    let custom = custom_resources(backend, scope);
    backend.read(async move {
        let mut result = builtin.await?;
        let custom = custom.await?;
        result.resources.extend(custom.resources);
        result.warnings.extend(custom.warnings);
        result.versions.extend(custom.versions);
        Ok(result)
    })
}

pub(super) fn builtin_resources(
    backend: &NativeBackend,
    scope: Scope,
    refresh: bool,
) -> Read<ScopeResult> {
    let store = backend.store.clone();
    backend.read(async move {
        let kinds = INSPECTION_KINDS.to_vec();
        let keys: Vec<_> = kinds
            .iter()
            .map(|kind| WatchResourceKey {
                resource_kind: WatchResourceKind {
                    kind: (*kind).to_owned(),
                    group: None,
                    version: None,
                    api_version: None,
                    plural: None,
                    namespaced: Some(true),
                },
                namespace: scope.namespace.clone(),
            })
            .collect();
        if refresh {
            let access = ClusterAccess::resolve(None)?;
            let namespaces: Vec<_> = scope.namespace.iter().cloned().collect();
            store.refresh_view(access.source_key(), &scope.context, &keys, &namespaces);
        }
        let mut warnings = Vec::new();
        // The web reads a scope in one `list_resource_scope` call, which lists
        // up to MAX_LIST_CONCURRENCY kinds at once. The namespace list does not
        // feed those reads, so it runs beside them.
        let reads: Vec<_> = kinds
            .iter()
            .map(|kind| {
                commands::list_resource_snapshot(
                    scope.context.clone(),
                    (*kind).to_owned(),
                    scope.namespace.clone(),
                    None,
                    State::new(&store),
                )
            })
            .collect();
        let (namespaces, results) = futures_util::join!(
            commands::list_namespaces(scope.context.clone(), None, State::new(&store)),
            bounded_join_all(reads, commands::MAX_LIST_CONCURRENCY),
        );
        let namespaces = match namespaces {
            Ok(rows) => Arc::unwrap_or_clone(rows),
            Err(error) if error.kind == AppErrorKind::Forbidden => {
                warnings.push(format!("Namespaces: {}", error.message));
                Vec::new()
            }
            Err(error) => return Err(error),
        };
        let mut resources = Vec::new();
        let mut versions = ListVersions::new();
        for ((kind, key), result) in kinds.into_iter().zip(&keys).zip(results) {
            match result {
                Ok(snapshot) => {
                    resources.extend(Arc::unwrap_or_clone(snapshot.rows));
                    if let Some(version) = snapshot.resource_version {
                        versions.insert(watch_signature(key), version);
                    }
                }
                Err(error) if error.kind == AppErrorKind::Forbidden => {
                    warnings.push(format!("{kind}: {}", error.message))
                }
                Err(error) => return Err(error),
            }
        }
        Ok(ScopeResult {
            namespaces,
            resources,
            warnings,
            versions,
        })
    })
}

/// Custom kinds present in the scope and their rows, read after the built-in
/// resources. The web lists only present kinds (`presentCustomResourceKinds`
/// in WorkspaceShell.svelte), so empty kinds cost one shared presence probe
/// instead of a list each.
pub(super) fn custom_resources(backend: &NativeBackend, scope: Scope) -> Read<ScopeResult> {
    let store = backend.store.clone();
    backend.read(async move {
        let mut warnings = Vec::new();
        let mut resources = Vec::new();
        let mut versions = ListVersions::new();
        let discovered = crate::read_retry::cancelled_read(
            || {
                Box::pin({
                    let store = store.clone();
                    let context = scope.context.clone();
                    let namespaces = scope.namespace.iter().cloned().collect();
                    async move {
                        commands::list_present_custom_resource_kinds(
                            context,
                            namespaces,
                            None,
                            State::new(&store),
                        )
                        .await
                    }
                })
            },
            tokio::time::sleep,
        )
        .await;
        match discovered {
            Ok(discovered) => {
                let reads: Vec<_> = discovered
                    .iter()
                    .map(|kind| {
                        commands::list_dynamic_resource_snapshot(
                            scope.context.clone(),
                            kind.clone(),
                            if kind.namespaced {
                                scope.namespace.clone()
                            } else {
                                None
                            },
                            None,
                            State::new(&store),
                        )
                    })
                    .collect();
                let results = bounded_join_all(reads, commands::MAX_LIST_CONCURRENCY).await;
                for (kind, result) in discovered.iter().zip(results) {
                    match result {
                        Ok(snapshot) => {
                            resources.extend(Arc::unwrap_or_clone(snapshot.rows));
                            if let Some(version) = snapshot.resource_version {
                                versions.insert(
                                    watch_signature(&discovered_key(kind, scope.namespace.clone())),
                                    version,
                                );
                            }
                        }
                        Err(error) => warnings.push(format!("{}: {}", kind.kind, error.message)),
                    }
                }
            }
            Err(error) => warnings.push(format!("Custom Resources: {}", error.message)),
        }
        Ok(ScopeResult {
            namespaces: Vec::new(),
            resources,
            warnings,
            versions,
        })
    })
}
