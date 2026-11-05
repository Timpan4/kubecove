//! Typed discovery, list and watch commands for native live views.
use super::{INSPECTION_KINDS, LogStream, NativeBackend, Read, Scope, ScopeResult};
use kubecove_lib::{
    commands::{self, ClusterAccess},
    models::{
        AppErrorKind, DiscoveredResourceKind, ResourceSummary, WatchResourceKey, WatchResourceKind,
    },
    runtime::{Channel, ChannelClosed, State},
};
use std::sync::Arc;
use tokio::sync::mpsc;

/// The owner determines the lifetime of a resource or selected-object event watch.
#[derive(Clone)]
pub enum WatchTarget {
    Resource { scope: Scope, key: WatchResourceKey },
    Events(Box<ResourceSummary>),
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
) -> Read<Vec<ResourceSummary>> {
    let store = backend.store.clone();
    backend.read(async move {
        let access = ClusterAccess::resolve(None)?;
        store.refresh_view(
            access.source_key(),
            &scope.context,
            std::slice::from_ref(&key),
            &[],
        );
        let kind = key.resource_kind;
        let rows = if let (Some(group), Some(version), Some(api_version), Some(plural)) =
            (kind.group, kind.version, kind.api_version, kind.plural)
        {
            commands::list_dynamic_resources(
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
            commands::list_resources(
                scope.context,
                kind.kind,
                key.namespace,
                None,
                State::new(&store),
            )
            .await?
        };
        Ok(Arc::unwrap_or_clone(rows))
    })
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
            WatchTarget::Resource { scope, key } => {
                commands::start_resource_watch(
                    scope.context,
                    vec![key],
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
        if refresh {
            let access = ClusterAccess::resolve(None)?;
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
            let namespaces: Vec<_> = scope.namespace.iter().cloned().collect();
            store.refresh_view(access.source_key(), &scope.context, &keys, &namespaces);
        }
        let mut warnings = Vec::new();
        let namespaces = match commands::list_namespaces(
            scope.context.clone(),
            None,
            State::new(&store),
        )
        .await
        {
            Ok(rows) => Arc::unwrap_or_clone(rows),
            Err(error) if error.kind == AppErrorKind::Forbidden => {
                warnings.push(format!("Namespaces: {}", error.message));
                Vec::new()
            }
            Err(error) => return Err(error),
        };
        let mut resources = Vec::new();
        for kind in kinds {
            match commands::list_resources(
                scope.context.clone(),
                kind.to_owned(),
                scope.namespace.clone(),
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
            namespaces,
            resources,
            warnings,
        })
    })
}

/// Discovered custom kinds and their rows, read after the built-in resources.
pub(super) fn custom_resources(backend: &NativeBackend, scope: Scope) -> Read<ScopeResult> {
    let store = backend.store.clone();
    backend.read(async move {
        let mut warnings = Vec::new();
        let mut resources = Vec::new();
        let discovered =
            crate::read_retry::cancelled_read(
                || {
                    Box::pin({
                        let store = store.clone();
                        let context = scope.context.clone();
                        async move {
                            commands::list_resource_kinds(context, None, State::new(&store)).await
                        }
                    })
                },
                tokio::time::sleep,
            )
            .await;
        match discovered {
            Ok(discovered) => {
                for kind in discovered.iter() {
                    let namespace = if kind.namespaced {
                        scope.namespace.clone()
                    } else {
                        None
                    };
                    match commands::list_dynamic_resources(
                        scope.context.clone(),
                        kind.clone(),
                        namespace,
                        None,
                        State::new(&store),
                    )
                    .await
                    {
                        Ok(rows) => resources.extend(Arc::unwrap_or_clone(rows)),
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
        })
    })
}
