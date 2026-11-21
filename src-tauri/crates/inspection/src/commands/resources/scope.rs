use super::{dynamic_resources_summary_from, resources_summary_from};
use crate::commands::MAX_LIST_CONCURRENCY;
#[cfg(any(test, feature = "bench-support"))]
use crate::commands::helpers::enrich_resource_summaries_with_flux_inventory;
use crate::commands::helpers::fetch_flux_ownership_index;
#[cfg(any(test, feature = "bench-support"))]
use crate::commands::helpers::filter_flux_ownership_index;
use crate::models::AppErrorKind;
use crate::runtime::State;
use crate::{
    commands::{
        BackendCancellationRegistry, ClusterAccess, ClusterLiveStore, FiniteRead, diagnostic_field,
    },
    models::{
        AppError, DiscoveredResourceKind, ResourceListRequest, ResourceListVersion,
        ResourceScopeFailure, ResourceSummary,
    },
};
use futures_util::future::join_all;
use kubecove_kube::commands::live_store::{ListSnapshot, ListedResources};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use tokio::sync::Semaphore;

#[path = "scope_rows.rs"]
mod rows;
use rows::SharedRows;

#[derive(Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceScopeResponse {
    resources: SharedRows,
    completed_requests: Vec<ResourceListRequest>,
    resource_versions: Vec<ResourceListVersion>,
    failures: Vec<ResourceScopeFailure>,
}

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
            AppErrorKind::Validation,
        )),
    }
}

fn kind_key(kind: &ResourceScopeKind) -> String {
    match kind {
        ResourceScopeKind::Typed(kind) => format!("typed:{kind}"),
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
    group.all_namespaces || group.namespaces.is_empty()
}

fn fetch_namespaces(group: &ResourceScopeGroup) -> Vec<Option<String>> {
    if should_promote_to_all(group) {
        return vec![None];
    }
    group.namespaces.iter().cloned().map(Some).collect()
}

#[cfg(feature = "bench-support")]
pub fn resource_scope_fetch_plan(
    requests: Vec<ResourceListRequest>,
) -> Result<Vec<(String, Option<String>)>, AppError> {
    let groups = group_requests(requests)?;
    Ok(groups
        .into_iter()
        .flat_map(|(kind, group)| {
            fetch_namespaces(&group)
                .into_iter()
                .map(move |namespace| (kind.clone(), namespace))
        })
        .collect())
}

fn ownership_namespaces(groups: &BTreeMap<String, ResourceScopeGroup>) -> Vec<String> {
    if groups.values().any(should_promote_to_all) {
        Vec::new()
    } else {
        groups
            .values()
            .flat_map(|group| group.namespaces.iter().cloned())
            .collect()
    }
}

fn row_key(row: &ResourceSummary) -> String {
    format!(
        "{}:{}:{}:{}:{}:{}",
        row.cluster,
        row.kind,
        row.api_version.as_deref().unwrap_or(""),
        row.plural.as_deref().unwrap_or(""),
        row.namespace.as_deref().unwrap_or(""),
        row.name
    )
}

#[cfg(any(test, feature = "bench-support"))]
fn dedupe_rows(rows: Vec<ResourceSummary>) -> Vec<ResourceSummary> {
    let mut by_key = BTreeMap::new();
    for row in rows {
        by_key.insert(row_key(&row), row);
    }
    by_key.into_values().collect()
}

#[cfg(any(test, feature = "bench-support"))]
fn assemble_post_fetch_scope(
    mut rows: Vec<ResourceSummary>,
    index: crate::commands::helpers::FluxOwnershipIndex,
    requested_namespaces: &[String],
) -> Vec<ResourceSummary> {
    let index = filter_flux_ownership_index(index, requested_namespaces);
    enrich_resource_summaries_with_flux_inventory(&mut rows, &index);
    dedupe_rows(rows)
}

#[cfg(feature = "bench-support")]
pub fn resource_scope_post_fetch(
    requests: Vec<ResourceListRequest>,
    batches: Vec<Vec<ResourceSummary>>,
    index: crate::commands::helpers::FluxOwnershipIndex,
) -> Result<Vec<ResourceSummary>, AppError> {
    let groups = group_requests(requests)?;
    let requested_namespaces = ownership_namespaces(&groups);
    let rows = batches.into_iter().flatten().collect();
    Ok(assemble_post_fetch_scope(
        rows,
        index,
        &requested_namespaces,
    ))
}

#[cfg(feature = "bench-support")]
pub fn encode_shared_scope_rows(
    batches: Vec<Arc<Vec<ResourceSummary>>>,
    index: &crate::commands::helpers::FluxOwnershipIndex,
) -> Result<String, serde_json::Error> {
    serde_json::to_string(&SharedRows::new(batches, index, &[]))
}

pub async fn resource_scope_from(
    access: &ClusterAccess,
    cluster_context: String,
    requests: Vec<ResourceListRequest>,
    live_store: ClusterLiveStore,
) -> Result<Vec<ResourceSummary>, AppError> {
    let result = resource_scope_result_from(access, cluster_context, requests, live_store).await?;
    if let Some(failure) = result.failures.into_iter().next() {
        return Err(failure.error);
    }
    Ok(result.resources.into_resources())
}

async fn resource_scope_result_from(
    access: &ClusterAccess,
    cluster_context: String,
    requests: Vec<ResourceListRequest>,
    live_store: ClusterLiveStore,
) -> Result<ResourceScopeResponse, AppError> {
    if requests.is_empty() {
        return Ok(ResourceScopeResponse::default());
    }
    let groups = group_requests(requests)?;
    let requested_namespaces = ownership_namespaces(&groups);
    let source_key = access.source_key().to_string();
    let fetches = groups.into_values().flat_map(|group| {
        fetch_namespaces(&group)
            .into_iter()
            .map(move |namespace| (group.kind.clone(), namespace))
    });
    // Keep permits outside shared cache loaders so cancellation releases them.
    let permits = Arc::new(Semaphore::new(MAX_LIST_CONCURRENCY));
    let futures = fetches.map(|(kind, namespace)| {
        let permits = Arc::clone(&permits);
        let live_store = live_store.clone();
        let cluster_context = cluster_context.clone();
        let source_key = source_key.clone();
        let access = access.clone();
        async move {
            let request = ResourceListRequest {
                kind: match &kind {
                    ResourceScopeKind::Typed(kind) => Some(kind.clone()),
                    ResourceScopeKind::Dynamic(_) => None,
                },
                resource_kind: match &kind {
                    ResourceScopeKind::Dynamic(kind) => Some(kind.clone()),
                    ResourceScopeKind::Typed(_) => None,
                },
                namespace: namespace.clone(),
            };
            let result = cached_scope_resources(
                &live_store,
                permits,
                source_key,
                cluster_context.clone(),
                kind.clone(),
                namespace.clone(),
                move || async move {
                    match kind {
                        ResourceScopeKind::Typed(kind) => {
                            resources_summary_from(&access, cluster_context, kind, namespace).await
                        }
                        ResourceScopeKind::Dynamic(kind) => {
                            dynamic_resources_summary_from(
                                &access,
                                cluster_context,
                                kind,
                                namespace,
                            )
                            .await
                        }
                    }
                },
            )
            .await;
            (request, result)
        }
    });
    // The Flux owner index does not depend on the lists, so it loads beside
    // them. A failed list still wins over an index error.
    let index = async {
        let client = access.read_client(&cluster_context).await?;
        live_store
            .flux_ownership_index(source_key.clone(), cluster_context.clone(), move || {
                fetch_flux_ownership_index(client)
            })
            .await
    };
    let (results, index) = futures_util::join!(join_all(futures), index);
    let mut batches = Vec::new();
    let mut completed_requests = Vec::new();
    let mut resource_versions = Vec::new();
    let mut failures = Vec::new();
    for (request, fetched) in results {
        match fetched {
            Ok(ListSnapshot {
                rows,
                resource_version,
                ..
            }) => {
                batches.push(rows);
                if let Some(resource_version) = resource_version {
                    resource_versions.push(ResourceListVersion {
                        request: request.clone(),
                        resource_version,
                    });
                }
                completed_requests.push(request);
            }
            Err(error) if error.kind == AppErrorKind::Forbidden => {
                failures.push(ResourceScopeFailure {
                    cluster_context: cluster_context.clone(),
                    request,
                    error,
                });
            }
            Err(error) => return Err(error),
        }
    }
    let index = index?;
    Ok(ResourceScopeResponse {
        resources: SharedRows::new(batches, &index, &requested_namespaces),
        completed_requests,
        resource_versions,
        failures,
    })
}

async fn cached_scope_resources<F, Fut, Listed>(
    live_store: &ClusterLiveStore,
    permits: Arc<Semaphore>,
    source_key: String,
    cluster_context: String,
    kind: ResourceScopeKind,
    namespace: Option<String>,
    loader: F,
) -> Result<ListSnapshot, AppError>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<Listed, AppError>> + Send + 'static,
    Listed: Into<ListedResources>,
{
    let _permit = permits.acquire().await.expect("scope semaphore stays open");
    match kind {
        ResourceScopeKind::Typed(kind) => {
            live_store
                .typed_resources(source_key, cluster_context, kind, namespace, loader)
                .await
        }
        ResourceScopeKind::Dynamic(kind) => {
            live_store
                .dynamic_resources(source_key, cluster_context, kind, namespace, loader)
                .await
        }
    }
}

pub async fn list_resource_scope(
    cluster_context: String,
    requests: Vec<ResourceListRequest>,
    kubeconfig_env_var: Option<String>,
    request_id: Option<String>,
    cancel_scope: Option<String>,
    live_store: State<'_, ClusterLiveStore>,
    cancellations: State<'_, BackendCancellationRegistry>,
) -> Result<ResourceScopeResponse, AppError> {
    let request_count = requests.len();
    let namespace_count = requests
        .iter()
        .filter_map(|request| request.namespace.as_deref())
        .filter(|namespace| !namespace.trim().is_empty())
        .collect::<BTreeSet<_>>()
        .len();
    FiniteRead::new("list_resource_scope")
        .label("context", &cluster_context)
        .label("requests", request_count)
        .cancellable(&cancellations, cancel_scope, request_id)
        .run(
            async move {
                let access = ClusterAccess::resolve(kubeconfig_env_var.as_deref())?;
                resource_scope_result_from(
                    &access,
                    cluster_context,
                    requests,
                    live_store.inner().clone(),
                )
                .await
            },
            |rows| {
                vec![
                    diagnostic_field("requests", request_count),
                    diagnostic_field("namespaces", namespace_count),
                    diagnostic_field("rows", rows.resources.len()),
                    diagnostic_field("deniedRequests", rows.failures.len()),
                ]
            },
        )
        .await
}

#[cfg(test)]
#[path = "scope_tests.rs"]
mod tests;
