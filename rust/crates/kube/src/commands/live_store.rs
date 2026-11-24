use crate::commands::builtin_kinds::builtin_kind;
use crate::commands::helpers::FluxOwnershipIndex;
use crate::models::{
    AppError, DiscoveredResourceKind, NamespaceSummary, ResourceTopology, WatchResourceKind,
};
#[cfg(test)]
use std::time::Instant;
use std::{future::Future, sync::Arc, time::Duration};

const COMPLETED_GRACE: Duration = Duration::from_millis(500);
const RESOURCE_FRESHNESS: Duration = Duration::from_secs(30);
/// Flux ownership comes from cluster-wide lists with no watch feed; a short
/// freshness window bounds staleness while absorbing per-request refetches.
const FLUX_INDEX_FRESHNESS: Duration = Duration::from_secs(30);
const MAX_FRESHNESS: Duration = Duration::from_secs(30);
const _: () = {
    assert!(COMPLETED_GRACE.as_nanos() <= MAX_FRESHNESS.as_nanos());
    assert!(RESOURCE_FRESHNESS.as_nanos() <= MAX_FRESHNESS.as_nanos());
    assert!(FLUX_INDEX_FRESHNESS.as_nanos() <= MAX_FRESHNESS.as_nanos());
};
#[path = "live_store_cache.rs"]
mod cache;
use cache::{CacheEntry, CacheMode, SharedCache};
#[cfg(test)]
use cache::{MAX_CACHE_ENTRIES, ReadyValue};

#[cfg(feature = "bench-support")]
#[doc(hidden)]
pub const BENCHMARK_RESOURCE_CACHE_CAPACITY: usize = cache::MAX_CACHE_ENTRIES;

#[path = "live_store_refresh.rs"]
mod refresh;

#[path = "live_store_diagnostics.rs"]
mod cache_diagnostics;

#[path = "live_store_list.rs"]
mod list;
pub use list::{ListSnapshot, ListedResources};

#[path = "live_store_watch.rs"]
mod watch;
pub use watch::{WatchOrigin, WatchedRow};

#[derive(Clone)]
pub struct ClusterLiveStore {
    namespaces: Arc<SharedCache<Vec<NamespaceSummary>>>,
    resource_kinds: Arc<SharedCache<Vec<DiscoveredResourceKind>>>,
    present_custom_resource_kinds: Arc<SharedCache<Vec<DiscoveredResourceKind>>>,
    resources: Arc<SharedCache<ListSnapshot>>,
    topologies: Arc<SharedCache<ResourceTopology>>,
    flux_ownership_indexes: Arc<SharedCache<FluxOwnershipIndex>>,
    watched_rows: Option<WatchedRow>,
}

impl Default for ClusterLiveStore {
    fn default() -> Self {
        Self {
            namespaces: Arc::new(SharedCache::new("namespaces")),
            resource_kinds: Arc::new(SharedCache::new("resource_kinds")),
            present_custom_resource_kinds: Arc::new(SharedCache::new(
                "present_custom_resource_kinds",
            )),
            resources: Arc::new(SharedCache::new("resources")),
            topologies: Arc::new(SharedCache::new("topologies")),
            flux_ownership_indexes: Arc::new(SharedCache::new("flux_ownership_indexes")),
            watched_rows: None,
        }
    }
}

impl ClusterLiveStore {
    pub fn cancel_loading(&self) -> usize {
        self.namespaces.cancel_loading()
            + self.resource_kinds.cancel_loading()
            + self.present_custom_resource_kinds.cancel_loading()
            + self.resources.cancel_loading()
            + self.topologies.cancel_loading()
            + self.flux_ownership_indexes.cancel_loading()
    }

    /// Caches the cluster-wide Flux ownership index per kubeconfig source and
    /// context. The index comes from two cluster-wide lists, so it is reused
    /// across scope requests within `FLUX_INDEX_FRESHNESS` and re-scoped to
    /// the requested namespaces by the caller.
    pub async fn flux_ownership_index<F, Fut>(
        &self,
        source_key: String,
        cluster_context: String,
        loader: F,
    ) -> Result<Arc<FluxOwnershipIndex>, AppError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<FluxOwnershipIndex, AppError>> + Send + 'static,
    {
        self.flux_ownership_indexes
            .get_or_load(
                context_cache_key(&source_key, &cluster_context),
                CacheMode::LiveFor(FLUX_INDEX_FRESHNESS),
                loader,
            )
            .await
    }

    pub async fn namespaces<F, Fut>(
        &self,
        source_key: String,
        cluster_context: String,
        loader: F,
    ) -> Result<Arc<Vec<NamespaceSummary>>, AppError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<Vec<NamespaceSummary>, AppError>> + Send + 'static,
    {
        self.namespaces
            .get_or_load(
                context_cache_key(&source_key, &cluster_context),
                CacheMode::GraceOnly,
                loader,
            )
            .await
    }

    pub async fn resource_kinds<F, Fut>(
        &self,
        source_key: String,
        cluster_context: String,
        loader: F,
    ) -> Result<Arc<Vec<DiscoveredResourceKind>>, AppError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<Vec<DiscoveredResourceKind>, AppError>> + Send + 'static,
    {
        self.resource_kinds
            .get_or_revalidate(
                context_cache_key(&source_key, &cluster_context),
                RESOURCE_FRESHNESS,
                loader,
            )
            .await
    }

    pub async fn present_custom_resource_kinds<F, Fut>(
        &self,
        source_key: String,
        cluster_context: String,
        namespaces: Vec<String>,
        loader: F,
    ) -> Result<Arc<Vec<DiscoveredResourceKind>>, AppError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<Vec<DiscoveredResourceKind>, AppError>> + Send + 'static,
    {
        let key =
            present_custom_resource_kinds_cache_key(&source_key, &cluster_context, &namespaces);
        self.present_custom_resource_kinds
            .get_or_load(key, CacheMode::LiveFor(RESOURCE_FRESHNESS), loader)
            .await
    }

    pub fn cached_present_custom_resource_kinds(
        &self,
        source_key: &str,
        cluster_context: &str,
        namespaces: &[String],
    ) -> Option<Arc<Vec<DiscoveredResourceKind>>> {
        self.present_custom_resource_kinds.peek(
            &present_custom_resource_kinds_cache_key(source_key, cluster_context, namespaces),
            CacheMode::LiveFor(RESOURCE_FRESHNESS),
        )
    }

    pub async fn typed_resources<F, Fut, Listed>(
        &self,
        source_key: String,
        cluster_context: String,
        kind: String,
        namespace: Option<String>,
        loader: F,
    ) -> Result<ListSnapshot, AppError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<Listed, AppError>> + Send + 'static,
        Listed: Into<ListedResources>,
    {
        let kind_key = typed_kind_key(&kind);
        let namespace_key = namespace_key_for_typed(&kind, namespace.as_deref());
        self.resources_for_scope(source_key, cluster_context, kind_key, namespace_key, loader)
            .await
    }

    pub async fn dynamic_resources<F, Fut, Listed>(
        &self,
        source_key: String,
        cluster_context: String,
        resource_kind: DiscoveredResourceKind,
        namespace: Option<String>,
        loader: F,
    ) -> Result<ListSnapshot, AppError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<Listed, AppError>> + Send + 'static,
        Listed: Into<ListedResources>,
    {
        let kind_key = dynamic_kind_key(
            &resource_kind.api_version,
            &resource_kind.plural,
            &resource_kind.kind,
        );
        let namespace_key =
            namespace_key_for_namespaced(resource_kind.namespaced, namespace.as_deref());
        self.resources_for_scope(source_key, cluster_context, kind_key, namespace_key, loader)
            .await
    }

    async fn resources_for_scope<F, Fut, Listed>(
        &self,
        source_key: String,
        cluster_context: String,
        kind_key: String,
        namespace_key: ScopeNamespace,
        loader: F,
    ) -> Result<ListSnapshot, AppError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<Listed, AppError>> + Send + 'static,
        Listed: Into<ListedResources>,
    {
        if let ScopeNamespace::Named(namespace) = &namespace_key {
            let all_key = resource_cache_key(
                &source_key,
                &cluster_context,
                &kind_key,
                &ScopeNamespace::All,
            );
            if let Some(list) = self
                .resources
                .peek(&all_key, CacheMode::LiveFor(RESOURCE_FRESHNESS))
            {
                return Ok(ListSnapshot::new(
                    Arc::new(
                        list.rows
                            .iter()
                            .filter(|row| row.namespace.as_deref() == Some(namespace.as_str()))
                            .cloned()
                            .collect(),
                    ),
                    list.resource_version.clone(),
                ));
            }
        }

        let list = self
            .resources
            .get_or_load(
                resource_cache_key(&source_key, &cluster_context, &kind_key, &namespace_key),
                CacheMode::LiveFor(RESOURCE_FRESHNESS),
                move || async move { Ok(ListSnapshot::from(loader().await?.into())) },
            )
            .await?;
        Ok(ListSnapshot::clone(&list))
    }

    pub async fn topology<F, Fut>(
        &self,
        source_key: String,
        cluster_context: String,
        namespaces: Vec<String>,
        mode: String,
        loader: F,
    ) -> Result<Arc<ResourceTopology>, AppError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<ResourceTopology, AppError>> + Send + 'static,
    {
        let key = topology_cache_key(&source_key, &cluster_context, &namespaces, &mode);
        self.topologies
            .get_or_load(key, CacheMode::LiveFor(RESOURCE_FRESHNESS), loader)
            .await
    }

    pub fn mark_watch_resource_dirty(
        &self,
        source_key: &str,
        cluster_context: &str,
        resource_kind: &WatchResourceKind,
        namespace: Option<&str>,
    ) {
        let namespace_key =
            namespace_key_for_namespaced(resource_kind.namespaced.unwrap_or(true), namespace);
        for kind_key in watch_kind_keys(resource_kind) {
            let exact_key =
                resource_cache_key(source_key, cluster_context, &kind_key, &namespace_key);
            let all_key =
                resource_cache_key(source_key, cluster_context, &kind_key, &ScopeNamespace::All);
            self.resources.mark_dirty(&exact_key);
            self.resources.mark_dirty(&all_key);
        }
        self.mark_derived_dirty(source_key, cluster_context);
    }

    /// Marks the context's topologies and present custom kinds dirty.
    fn mark_derived_dirty(&self, source_key: &str, cluster_context: &str) {
        let prefix = format!("{source_key}|context={cluster_context}|");
        self.topologies
            .mark_dirty_where(|key| key.starts_with(&prefix));
        self.present_custom_resource_kinds
            .mark_dirty_where(|key| key.starts_with(&prefix));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScopeNamespace {
    All,
    Cluster,
    Named(String),
}

fn typed_kind_key(kind: &str) -> String {
    format!("typed:{kind}")
}

fn dynamic_kind_key(api_version: &str, plural: &str, kind: &str) -> String {
    format!("dynamic:{api_version}:{plural}:{kind}")
}

fn watch_kind_key(resource_kind: &WatchResourceKind) -> String {
    match (
        resource_kind.api_version.as_deref(),
        resource_kind.plural.as_deref(),
    ) {
        (Some(api_version), Some(plural)) => {
            dynamic_kind_key(api_version, plural, &resource_kind.kind)
        }
        _ => typed_kind_key(&resource_kind.kind),
    }
}

fn watch_kind_keys(resource_kind: &WatchResourceKind) -> Vec<String> {
    let mut keys = vec![watch_kind_key(resource_kind)];
    if is_known_typed_kind(&resource_kind.kind) {
        let typed_key = typed_kind_key(&resource_kind.kind);
        if !keys.contains(&typed_key) {
            keys.push(typed_key);
        }
    }
    keys
}

fn namespace_key_for_typed(kind: &str, namespace: Option<&str>) -> ScopeNamespace {
    if is_cluster_scoped_kind(kind) {
        ScopeNamespace::Cluster
    } else {
        namespace_key_for_namespaced(true, namespace)
    }
}

fn namespace_key_for_namespaced(namespaced: bool, namespace: Option<&str>) -> ScopeNamespace {
    if !namespaced {
        return ScopeNamespace::Cluster;
    }
    match namespace {
        Some(namespace) if !namespace.trim().is_empty() => {
            ScopeNamespace::Named(namespace.to_string())
        }
        _ => ScopeNamespace::All,
    }
}

pub(crate) fn context_cache_key(source_key: &str, context: &str) -> String {
    format!("{source_key}|context={context}")
}

pub(crate) fn resource_cache_key(
    source_key: &str,
    context: &str,
    kind_key: &str,
    namespace: &ScopeNamespace,
) -> String {
    let namespace = match namespace {
        ScopeNamespace::All => "<all>",
        ScopeNamespace::Cluster => "<cluster>",
        ScopeNamespace::Named(namespace) => namespace,
    };
    format!("{source_key}|context={context}|kind={kind_key}|namespace={namespace}")
}

fn topology_cache_key(
    source_key: &str,
    context: &str,
    namespaces: &[String],
    mode: &str,
) -> String {
    let namespaces = normalized_namespaces(namespaces);
    format!("{source_key}|context={context}|topology={mode}|namespaces={namespaces}")
}

fn present_custom_resource_kinds_cache_key(
    source_key: &str,
    context: &str,
    namespaces: &[String],
) -> String {
    let namespaces = normalized_namespaces(namespaces);
    format!("{source_key}|context={context}|present_custom_resource_kinds={namespaces}")
}

fn normalized_namespaces(namespaces: &[String]) -> String {
    let mut namespaces: Vec<&str> = namespaces.iter().map(String::as_str).collect();
    namespaces.sort_unstable();
    namespaces.dedup();
    namespaces.join(",")
}

fn is_cluster_scoped_kind(kind: &str) -> bool {
    builtin_kind(kind).is_some_and(|entry| !entry.namespaced)
}

fn is_known_typed_kind(kind: &str) -> bool {
    builtin_kind(kind).is_some_and(|entry| entry.table)
}

#[cfg(test)]
#[path = "live_store_tests.rs"]
mod live_store_tests;

#[cfg(test)]
#[path = "live_store_race_tests.rs"]
mod race_tests;

#[cfg(test)]
#[path = "live_store_diagnostics_tests.rs"]
mod live_store_diagnostics_tests;
