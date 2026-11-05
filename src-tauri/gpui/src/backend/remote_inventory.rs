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
            .map(|kind| WatchResourceKey {
                resource_kind: WatchResourceKind {
                    kind: (*kind).into(),
                    group: None,
                    version: None,
                    api_version: None,
                    plural: None,
                    namespaced: Some(!CLUSTER_KINDS.contains(kind)),
                },
                namespace: if CLUSTER_KINDS.contains(kind) {
                    None
                } else {
                    scope.namespace.clone()
                },
            })
            .collect::<Vec<_>>();
        keys.extend(discovered.iter().map(|kind| WatchResourceKey {
            resource_kind: WatchResourceKind {
                kind: kind.kind.clone(),
                group: Some(kind.group.clone()),
                version: Some(kind.version.clone()),
                api_version: Some(kind.api_version.clone()),
                plural: Some(kind.plural.clone()),
                namespaced: Some(kind.namespaced),
            },
            namespace: if kind.namespaced {
                scope.namespace.clone()
            } else {
                None
            },
        }));
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
    };
    for kind in kinds {
        let namespace = if CLUSTER_KINDS.contains(kind) {
            None
        } else {
            scope.namespace.clone()
        };
        match invoke::<Vec<ResourceSummary>>(
            remote.clone(),
            "list_resources",
            json!({"clusterContext":scope.context,"kind":kind,"namespace":namespace}),
        )
        .await
        {
            Ok(rows) => result.resources.extend(rows),
            Err(error) if error.kind == AppErrorKind::Forbidden => {
                result.warnings.push(format!("{kind}: {}", error.message))
            }
            Err(error) => return Err(error),
        }
    }
    for kind in discovered {
        let namespace = if kind.namespaced {
            scope.namespace.clone()
        } else {
            None
        };
        match invoke::<Vec<ResourceSummary>>(
            remote.clone(),
            "list_dynamic_resources",
            json!({"clusterContext":scope.context,"resourceKind":kind,"namespace":namespace}),
        )
        .await
        {
            Ok(rows) => result.resources.extend(rows),
            Err(error) => result
                .warnings
                .push(format!("{}: {}", kind.kind, error.message)),
        }
    }
    Ok(result)
}
