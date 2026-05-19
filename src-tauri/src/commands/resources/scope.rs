use super::{dynamic_resources_summary_from, resources_summary_from};
use crate::{
    commands::ClusterLiveStore,
    models::{AppError, DiscoveredResourceKind, ResourceListRequest, ResourceSummary},
};
use futures_util::future::join_all;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;
use tauri::State;

#[derive(Debug, Clone)]
enum ResourceScopeKind {
    Typed(String),
    Dynamic(DiscoveredResourceKind),
}

#[derive(Debug)]
struct ResourceScopeGroup {
    kind: ResourceScopeKind,
    namespaces: BTreeSet<String>,
    all_namespaces: bool,
}

impl ResourceScopeGroup {
    fn new(kind: ResourceScopeKind) -> Self {
        Self {
            kind,
            namespaces: BTreeSet::new(),
            all_namespaces: false,
        }
    }

    fn add_namespace(&mut self, namespace: Option<String>) {
        match namespace {
            Some(namespace) if !namespace.trim().is_empty() => {
                self.namespaces.insert(namespace);
            }
            _ => self.all_namespaces = true,
        }
    }
}

fn request_kind(request: &ResourceListRequest) -> Result<ResourceScopeKind, AppError> {
    match (&request.kind, &request.resource_kind) {
        (Some(kind), None) if !kind.trim().is_empty() => Ok(ResourceScopeKind::Typed(kind.clone())),
        (None, Some(resource_kind)) => Ok(ResourceScopeKind::Dynamic(resource_kind.clone())),
        _ => Err(AppError::new(
            "resource scope request must include exactly one kind",
            "validation",
        )),
    }
}

fn kind_key(kind: &ResourceScopeKind) -> String {
    match kind {
        ResourceScopeKind::Typed(kind) => format!("typed:{}", kind),
        ResourceScopeKind::Dynamic(kind) => {
            format!("dynamic:{}:{}:{}", kind.api_version, kind.plural, kind.kind)
        }
    }
}

fn group_requests(
    requests: Vec<ResourceListRequest>,
) -> Result<BTreeMap<String, ResourceScopeGroup>, AppError> {
    let mut groups = BTreeMap::new();
    for request in requests {
        let kind = request_kind(&request)?;
        let key = kind_key(&kind);
        groups
            .entry(key)
            .or_insert_with(|| ResourceScopeGroup::new(kind))
            .add_namespace(request.namespace);
    }
    Ok(groups)
}

fn should_promote_to_all(group: &ResourceScopeGroup) -> bool {
    group.all_namespaces || group.namespaces.len() > 1
}

fn filter_promoted_rows(
    rows: Vec<ResourceSummary>,
    group: &ResourceScopeGroup,
) -> Vec<ResourceSummary> {
    if group.all_namespaces || group.namespaces.is_empty() {
        return rows;
    }
    rows.into_iter()
        .filter(|row| {
            row.namespace
                .as_deref()
                .is_some_and(|namespace| group.namespaces.contains(namespace))
        })
        .collect()
}

fn dedupe_rows(rows: Vec<ResourceSummary>) -> Vec<ResourceSummary> {
    let mut by_key = BTreeMap::new();
    for row in rows {
        by_key.insert(
            format!(
                "{}:{}:{}:{}:{}:{}",
                row.cluster,
                row.kind,
                row.api_version.as_deref().unwrap_or(""),
                row.plural.as_deref().unwrap_or(""),
                row.namespace.as_deref().unwrap_or(""),
                row.name
            ),
            row,
        );
    }
    by_key.into_values().collect()
}

pub async fn resource_scope_from(
    cluster_context: String,
    requests: Vec<ResourceListRequest>,
    live_store: ClusterLiveStore,
) -> Result<Vec<ResourceSummary>, AppError> {
    let groups = group_requests(requests)?;
    let futures = groups.into_values().map(|group| {
        let live_store = live_store.clone();
        let cluster_context = cluster_context.clone();
        async move {
            let promoted = should_promote_to_all(&group);
            let namespace = if promoted {
                None
            } else {
                group.namespaces.iter().next().cloned()
            };
            let rows = match group.kind.clone() {
                ResourceScopeKind::Typed(kind) => {
                    live_store
                        .typed_resources(
                            cluster_context.clone(),
                            kind.clone(),
                            namespace.clone(),
                            {
                                let cluster_context = cluster_context.clone();
                                move || resources_summary_from(cluster_context, kind, namespace)
                            },
                        )
                        .await?
                }
                ResourceScopeKind::Dynamic(resource_kind) => {
                    live_store
                        .dynamic_resources(
                            cluster_context.clone(),
                            resource_kind.clone(),
                            namespace.clone(),
                            {
                                let cluster_context = cluster_context.clone();
                                move || {
                                    dynamic_resources_summary_from(
                                        cluster_context,
                                        resource_kind,
                                        namespace,
                                    )
                                }
                            },
                        )
                        .await?
                }
            };
            let rows = if promoted {
                filter_promoted_rows(rows, &group)
            } else {
                rows
            };
            Ok::<_, AppError>(rows)
        }
    });

    let results = join_all(futures).await;
    let mut rows = Vec::new();
    for result in results {
        rows.extend(result?);
    }
    Ok(dedupe_rows(rows))
}

#[tauri::command]
pub async fn list_resource_scope(
    cluster_context: String,
    requests: Vec<ResourceListRequest>,
    live_store: State<'_, ClusterLiveStore>,
) -> Result<Vec<ResourceSummary>, AppError> {
    let started = Instant::now();
    eprintln!(
        "[kubecove:backend] list_resource_scope start context={} requests={}",
        cluster_context,
        requests.len()
    );
    let result = resource_scope_from(
        cluster_context.clone(),
        requests,
        live_store.inner().clone(),
    )
    .await;
    match &result {
        Ok(rows) => eprintln!(
            "[kubecove:backend] list_resource_scope done context={} rows={} ms={}",
            cluster_context,
            rows.len(),
            started.elapsed().as_millis()
        ),
        Err(err) => eprintln!(
            "[kubecove:backend] list_resource_scope error context={} error_kind={} message={} ms={}",
            cluster_context,
            err.kind,
            err.message,
            started.elapsed().as_millis()
        ),
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pod_request(namespace: Option<&str>) -> ResourceListRequest {
        ResourceListRequest {
            kind: Some("Pod".to_string()),
            resource_kind: None,
            namespace: namespace.map(ToString::to_string),
        }
    }

    #[test]
    fn groups_multiple_namespaces_for_one_kind() {
        let groups = group_requests(vec![
            pod_request(Some("default")),
            pod_request(Some("argocd")),
        ])
        .expect("groups");
        let group = groups.get("typed:Pod").expect("pod group");

        assert!(should_promote_to_all(group));
        assert_eq!(group.namespaces.len(), 2);
    }

    #[test]
    fn rejects_requests_without_one_kind_identity() {
        let err = request_kind(&ResourceListRequest {
            kind: None,
            resource_kind: None,
            namespace: None,
        })
        .expect_err("missing kind");

        assert_eq!(err.kind, "validation");
    }
}
