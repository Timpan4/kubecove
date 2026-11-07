use super::*;

pub(super) async fn inventory(
    remote: Arc<RemoteConnectionStore>,
    scope: Scope,
    kinds: &[&str],
    refresh: bool,
) -> Result<ScopeResult, AppError> {
    let mut warnings = Vec::new();
    let discovered = if kinds == CLUSTER_KINDS {
        Vec::new()
    } else {
        match invoke::<Vec<DiscoveredResourceKind>>(
            remote.clone(),
            "list_resource_kinds",
            json!({"clusterContext":scope.context}),
        )
        .await
        {
            Ok(kinds) => kinds,
            Err(error) => {
                warnings.push(format!("Custom Resources: {}", error.message));
                Vec::new()
            }
        }
    };
    if refresh {
        let mut keys = kinds
            .iter()
            .map(|kind| typed_watch_key(kind, typed_namespace(kind, &scope)))
            .collect::<Vec<_>>();
        keys.extend(
            discovered
                .iter()
                .map(|kind| discovered_watch_key(kind, discovered_namespace(kind, &scope))),
        );
        let _: Value = invoke(
            remote.clone(),
            "refresh_resource_cache",
            json!({"clusterContext":scope.context,"keys":keys,"namespaces":scope.namespace.iter().collect::<Vec<_>>()}),
        )
        .await?;
    }
    let namespaces = match invoke(
        remote.clone(),
        "list_namespaces",
        json!({"clusterContext":scope.context}),
    )
    .await
    {
        Ok(rows) => rows,
        Err(error) if error.kind == AppErrorKind::Forbidden => {
            warnings.push(format!("Namespaces: {}", error.message));
            Vec::new()
        }
        Err(error) => return Err(error),
    };
    let mut result = ScopeResult {
        namespaces,
        resources: vec![],
        warnings,
        versions: Default::default(),
    };
    for kind in kinds {
        let namespace = typed_namespace(kind, &scope);
        match list_request(
            remote.clone(),
            &scope.context,
            typed_request(kind, namespace.clone()),
        )
        .await
        {
            Ok(list) => {
                result.resources.extend(list.items);
                if let Some(version) = list.resource_version {
                    let key = typed_watch_key(kind, namespace);
                    result
                        .versions
                        .insert(crate::backend::live_data::watch_signature(&key), version);
                }
            }
            Err(error) if error.kind == AppErrorKind::Forbidden => {
                result.warnings.push(format!("{kind}: {}", error.message))
            }
            Err(error) => return Err(error),
        }
    }
    for kind in discovered {
        let namespace = discovered_namespace(&kind, &scope);
        match list_request(
            remote.clone(),
            &scope.context,
            discovered_request(kind.clone(), namespace.clone()),
        )
        .await
        {
            Ok(list) => {
                result.resources.extend(list.items);
                if let Some(version) = list.resource_version {
                    let key = discovered_watch_key(&kind, namespace);
                    result
                        .versions
                        .insert(crate::backend::live_data::watch_signature(&key), version);
                }
            }
            Err(error) => result
                .warnings
                .push(format!("{}: {}", kind.kind, error.message)),
        }
    }
    Ok(result)
}

/// Lists one request with `list_resource_scope`, the command whose result carries the
/// list version. A forbidden request comes back as an error, as `list_resources` reported it.
pub(super) async fn list_request(
    remote: Arc<RemoteConnectionStore>,
    context: &str,
    request: ResourceListRequest,
) -> Result<VersionedList<ResourceSummary>, AppError> {
    let scope: ResourceScopeResult = invoke(
        remote,
        "list_resource_scope",
        json!({"clusterContext":context,"requests":[request]}),
    )
    .await?;
    if let Some(failure) = scope.failures.into_iter().next() {
        return Err(failure.error);
    }
    Ok(VersionedList {
        items: scope.resources,
        resource_version: scope
            .resource_versions
            .into_iter()
            .next()
            .map(|version| version.resource_version),
    })
}

/// The request a view's watch key lists. A key with a complete discovered kind lists
/// that custom resource kind, as the local backend does.
pub(super) fn key_request(key: &WatchResourceKey) -> ResourceListRequest {
    let kind = &key.resource_kind;
    match (&kind.group, &kind.version, &kind.api_version, &kind.plural) {
        (Some(group), Some(version), Some(api_version), Some(plural)) => discovered_request(
            DiscoveredResourceKind {
                group: group.clone(),
                version: version.clone(),
                api_version: api_version.clone(),
                plural: plural.clone(),
                kind: kind.kind.clone(),
                namespaced: kind.namespaced.unwrap_or(true),
                short_names: Vec::new(),
            },
            key.namespace.clone(),
        ),
        _ => typed_request(&kind.kind, key.namespace.clone()),
    }
}

fn typed_request(kind: &str, namespace: Option<String>) -> ResourceListRequest {
    ResourceListRequest {
        kind: Some(kind.into()),
        resource_kind: None,
        namespace,
    }
}

fn discovered_request(
    kind: DiscoveredResourceKind,
    namespace: Option<String>,
) -> ResourceListRequest {
    ResourceListRequest {
        kind: None,
        resource_kind: Some(kind),
        namespace,
    }
}

/// The namespace a typed kind lists in. Cluster-scoped kinds list with none.
fn typed_namespace(kind: &str, scope: &Scope) -> Option<String> {
    if CLUSTER_KINDS.contains(&kind) {
        None
    } else {
        scope.namespace.clone()
    }
}

fn discovered_namespace(kind: &DiscoveredResourceKind, scope: &Scope) -> Option<String> {
    if kind.namespaced {
        scope.namespace.clone()
    } else {
        None
    }
}

/// The watch key a view uses for a typed kind. Its list version is stored under it.
fn typed_watch_key(kind: &str, namespace: Option<String>) -> WatchResourceKey {
    WatchResourceKey {
        resource_kind: WatchResourceKind {
            kind: kind.into(),
            group: None,
            version: None,
            api_version: None,
            plural: None,
            namespaced: Some(!CLUSTER_KINDS.contains(&kind)),
        },
        namespace,
    }
}

/// The watch key a view uses for a discovered custom resource kind.
fn discovered_watch_key(
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
        namespace,
    }
}
