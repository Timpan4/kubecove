use super::{dynamic_resources_summary_from, resources_summary_from};
#[cfg(any(test, feature = "bench-support"))]
use crate::commands::helpers::enrich_resource_summaries_with_flux_inventory;
use crate::commands::helpers::{fetch_flux_ownership_index, filter_flux_ownership_index};
use crate::commands::MAX_LIST_CONCURRENCY;
use crate::models::AppErrorKind;
use crate::runtime::State;
use crate::{
    commands::{
        diagnostic_field, BackendCancellationRegistry, ClusterAccess, ClusterLiveStore, FiniteRead,
    },
    models::{
        AppError, DiscoveredResourceKind, ResourceListRequest, ResourceScopeFailure,
        ResourceSummary,
    },
};
use futures_util::future::join_all;
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
    serde_json::to_string(&SharedRows::new(batches, index))
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
    let results = join_all(futures).await;
    let mut batches = Vec::new();
    let mut completed_requests = Vec::new();
    let mut failures = Vec::new();
    for (request, fetched) in results {
        match fetched {
            Ok(rows) => {
                batches.push(rows);
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
    let client = access.read_client(&cluster_context).await?;
    let index = live_store
        .flux_ownership_index(source_key, cluster_context.clone(), {
            let client = client.clone();
            move || fetch_flux_ownership_index(client)
        })
        .await?;
    let index = filter_flux_ownership_index(Arc::unwrap_or_clone(index), &requested_namespaces);
    Ok(ResourceScopeResponse {
        resources: SharedRows::new(batches, &index),
        completed_requests,
        failures,
    })
}

async fn cached_scope_resources<F, Fut>(
    live_store: &ClusterLiveStore,
    permits: Arc<Semaphore>,
    source_key: String,
    cluster_context: String,
    kind: ResourceScopeKind,
    namespace: Option<String>,
    loader: F,
) -> Result<Arc<Vec<ResourceSummary>>, AppError>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<Vec<ResourceSummary>, AppError>> + Send + 'static,
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
mod tests {
    use super::*;
    use crate::commands::{ClientIntent, FixedClusterAccess, FIXED_SOURCE_KEY};
    use http::{Request, Response};
    use kube::client::Body;

    fn unused_cluster() -> Arc<FixedClusterAccess> {
        let (service, _) = tower_test::mock::pair::<Request<Body>, Response<Body>>();
        FixedClusterAccess::new(kube::Client::new(service, "default"))
    }

    fn pod_request(namespace: Option<&str>) -> ResourceListRequest {
        ResourceListRequest {
            kind: Some("Pod".to_string()),
            resource_kind: None,
            namespace: namespace.map(ToString::to_string),
        }
    }

    // Failure mode: cancelling a scope leaves shared loaders holding its permits,
    // so a later request for the first queued namespace can never acquire one.
    #[tokio::test]
    async fn cancelled_scope_does_not_strand_a_later_cached_request() {
        use futures_util::FutureExt;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Duration;

        let dynamic = DiscoveredResourceKind {
            group: "example.com".into(),
            version: "v1".into(),
            api_version: "example.com/v1".into(),
            kind: "Widget".into(),
            plural: "widgets".into(),
            namespaced: true,
            short_names: Vec::new(),
        };
        for kind in [
            ResourceScopeKind::Typed("Pod".into()),
            ResourceScopeKind::Dynamic(dynamic),
        ] {
            let store = ClusterLiveStore::default();
            let permits = Arc::new(Semaphore::new(MAX_LIST_CONCURRENCY));
            let started = Arc::new(AtomicUsize::new(0));
            let (release, _) = tokio::sync::watch::channel(false);
            let scope = join_all((0..=MAX_LIST_CONCURRENCY).map(|index| {
                let started = Arc::clone(&started);
                let mut wait = release.subscribe();
                cached_scope_resources(
                    &store,
                    Arc::clone(&permits),
                    "scope-cancel".into(),
                    "test".into(),
                    kind.clone(),
                    Some(format!("namespace-{index}")),
                    move || async move {
                        started.fetch_add(1, Ordering::SeqCst);
                        wait.wait_for(|released| *released).await.unwrap();
                        Ok(Vec::new())
                    },
                )
            }));
            let mut scope = Box::pin(scope);
            assert!(scope.as_mut().now_or_never().is_none());
            assert_eq!(started.load(Ordering::SeqCst), MAX_LIST_CONCURRENCY);
            drop(scope);
            release.send(true).unwrap();

            // Use the existing mock-client test deadline, not a latency budget.
            let rows = tokio::time::timeout(
                Duration::from_secs(5),
                cached_scope_resources(
                    &store,
                    Arc::new(Semaphore::new(MAX_LIST_CONCURRENCY)),
                    "scope-cancel".into(),
                    "test".into(),
                    kind,
                    Some(format!("namespace-{MAX_LIST_CONCURRENCY}")),
                    || async { Ok(Vec::new()) },
                ),
            )
            .await
            .expect("cancelled scope must not block a later request")
            .unwrap();
            assert!(rows.is_empty());
        }
    }

    #[tokio::test]
    async fn fetch_error_drives_other_shared_loads_to_completion() {
        use futures_util::FutureExt;
        use tokio::sync::oneshot;

        let store = ClusterLiveStore::default();
        let source = FIXED_SOURCE_KEY;
        let cluster = unused_cluster();
        let mut releases = Vec::new();
        // Seed pending shared loads without driving them in another task.
        for namespace in ["a", "b"] {
            let (release, wait) = oneshot::channel::<()>();
            releases.push(release);
            let load = store.typed_resources(
                source.into(),
                "test".into(),
                "Pod".into(),
                Some(namespace.into()),
                move || async move {
                    wait.await.unwrap();
                    if namespace == "a" {
                        Err(AppError::new("first error", AppErrorKind::Validation))
                    } else {
                        Ok(Vec::new())
                    }
                },
            );
            assert!(load.now_or_never().is_none());
        }
        let access = cluster.access();
        let scope = resource_scope_result_from(
            &access,
            "test".into(),
            vec![pod_request(Some("a")), pod_request(Some("b"))],
            store.clone(),
        );
        futures_util::pin_mut!(scope);
        assert!(scope.as_mut().now_or_never().is_none());
        releases.remove(0).send(()).unwrap();
        assert!(scope.as_mut().now_or_never().is_none());
        releases.remove(0).send(()).unwrap();
        let error = scope.await.err().expect("scope error");
        assert_eq!(error.message, "first error");
        assert_eq!(store.cancel_loading(), 0);
        // A completed second batch must be reusable without calling its loader.
        store
            .typed_resources(
                source.into(),
                "test".into(),
                "Pod".into(),
                Some("b".into()),
                || async { panic!("completed cache load must be reused") },
            )
            .await
            .expect("completed cache load");
    }

    #[tokio::test]
    async fn empty_requests_return_without_requesting_a_client() {
        let cluster = unused_cluster();
        let result = resource_scope_from(
            &cluster.access(),
            "missing-context".to_string(),
            Vec::new(),
            ClusterLiveStore::default(),
        )
        .await;

        assert!(result.expect("empty scope").is_empty());
        assert_eq!(cluster.intents(), Vec::<ClientIntent>::new());
    }

    #[test]
    fn groups_multiple_namespaces_for_one_kind_without_promoting_to_all() {
        let groups = group_requests(vec![
            pod_request(Some("default")),
            pod_request(Some("argocd")),
        ])
        .expect("groups");
        let group = groups.get("typed:Pod").expect("pod group");

        assert!(!should_promote_to_all(group));
        assert_eq!(
            fetch_namespaces(group),
            vec![Some("argocd".to_string()), Some("default".to_string())]
        );
        assert_eq!(group.namespaces.len(), 2);
    }

    #[test]
    fn keeps_large_namespace_sets_namespace_scoped() {
        let groups = group_requests(vec![
            pod_request(Some("default")),
            pod_request(Some("argocd")),
            pod_request(Some("jobs")),
            pod_request(Some("kube-system")),
            pod_request(Some("monitoring")),
            pod_request(Some("platform")),
            pod_request(Some("staging")),
            pod_request(Some("prod")),
        ])
        .expect("groups");
        let group = groups.get("typed:Pod").expect("pod group");

        assert!(!should_promote_to_all(group));
        assert_eq!(
            fetch_namespaces(group),
            vec![
                Some("argocd".to_string()),
                Some("default".to_string()),
                Some("jobs".to_string()),
                Some("kube-system".to_string()),
                Some("monitoring".to_string()),
                Some("platform".to_string()),
                Some("prod".to_string()),
                Some("staging".to_string()),
            ]
        );
    }

    #[test]
    fn promotes_explicit_all_namespace_request_to_one_fetch() {
        let groups = group_requests(vec![pod_request(None)]).expect("groups");
        let group = groups.get("typed:Pod").expect("pod group");

        assert!(should_promote_to_all(group));
        assert_eq!(fetch_namespaces(group), vec![None]);
    }

    #[test]
    fn all_namespace_group_keeps_ownership_read_cluster_wide() {
        let groups =
            group_requests(vec![pod_request(None), pod_request(Some("default"))]).expect("groups");

        assert_eq!(ownership_namespaces(&groups).len(), 0);
    }

    #[test]
    fn rejects_requests_without_one_kind_identity() {
        let err = request_kind(&ResourceListRequest {
            kind: None,
            resource_kind: None,
            namespace: None,
        })
        .expect_err("missing kind");

        assert_eq!(err.kind, AppErrorKind::Validation);
    }
}
