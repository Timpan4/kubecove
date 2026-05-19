use crate::models::{
    AppError, DiscoveredResourceKind, NamespaceSummary, ResourceSummary, ResourceTopology,
    WatchResourceKind,
};
use futures_util::future::{BoxFuture, FutureExt, Shared};
use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const COMPLETED_GRACE: Duration = Duration::from_millis(500);

type SharedLoad<T> = Shared<BoxFuture<'static, Result<T, AppError>>>;

#[derive(Clone, Copy)]
enum CacheMode {
    GraceOnly,
    LiveUntilDirty,
}

struct ReadyValue<T> {
    value: T,
    completed_at: Instant,
    dirty: bool,
}

enum CacheEntry<T> {
    Ready(ReadyValue<T>),
    Loading {
        future: SharedLoad<T>,
        previous: Option<ReadyValue<T>>,
        dirty: bool,
    },
}

struct SharedCache<T> {
    label: &'static str,
    entries: Mutex<HashMap<String, CacheEntry<T>>>,
}

impl<T> SharedCache<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn new(label: &'static str) -> Self {
        Self {
            label,
            entries: Mutex::new(HashMap::new()),
        }
    }

    fn ready_value(value: T) -> ReadyValue<T> {
        ReadyValue {
            value,
            completed_at: Instant::now(),
            dirty: false,
        }
    }

    fn can_reuse(ready: &ReadyValue<T>, mode: CacheMode) -> bool {
        if ready.dirty {
            return false;
        }

        match mode {
            CacheMode::GraceOnly => ready.completed_at.elapsed() <= COMPLETED_GRACE,
            CacheMode::LiveUntilDirty => true,
        }
    }

    fn peek(&self, key: &str, mode: CacheMode) -> Option<T> {
        let entries = self.entries.lock().expect("live store cache lock");
        match entries.get(key) {
            Some(CacheEntry::Ready(ready)) if Self::can_reuse(ready, mode) => {
                Some(ready.value.clone())
            }
            _ => None,
        }
    }

    async fn get_or_load<F, Fut>(
        &self,
        key: String,
        mode: CacheMode,
        loader: F,
    ) -> Result<T, AppError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, AppError>> + Send + 'static,
    {
        let future = {
            let mut entries = self.entries.lock().expect("live store cache lock");
            match entries.get(&key) {
                Some(CacheEntry::Ready(ready)) if Self::can_reuse(ready, mode) => {
                    eprintln!(
                        "[kubecove:backend] live_store cache_hit area={} key={}",
                        self.label, key
                    );
                    return Ok(ready.value.clone());
                }
                Some(CacheEntry::Loading { future, .. }) => {
                    eprintln!(
                        "[kubecove:backend] live_store singleflight_join area={} key={}",
                        self.label, key
                    );
                    future.clone()
                }
                _ => {
                    eprintln!(
                        "[kubecove:backend] live_store cache_miss area={} key={}",
                        self.label, key
                    );
                    let previous = entries.remove(&key).and_then(|entry| match entry {
                        CacheEntry::Ready(ready) => Some(ready),
                        CacheEntry::Loading { previous, .. } => previous,
                    });
                    let future = async move { loader().await }.boxed().shared();
                    entries.insert(
                        key.clone(),
                        CacheEntry::Loading {
                            future: future.clone(),
                            previous,
                            dirty: false,
                        },
                    );
                    future
                }
            }
        };

        let result = future.await;
        let mut entries = self.entries.lock().expect("live store cache lock");
        let dirty_while_loading = entries.get(&key).is_some_and(|entry| {
            matches!(entry, CacheEntry::Loading { dirty: true, .. })
                || matches!(entry, CacheEntry::Ready(ready) if ready.dirty)
        });
        match &result {
            Ok(value) => {
                let mut ready = Self::ready_value(value.clone());
                ready.dirty = dirty_while_loading;
                entries.insert(key, CacheEntry::Ready(ready));
            }
            Err(_) => {
                let previous = entries.remove(&key).and_then(|entry| match entry {
                    CacheEntry::Loading {
                        previous, dirty, ..
                    } => previous.map(|mut ready| {
                        ready.dirty |= dirty;
                        ready
                    }),
                    CacheEntry::Ready(ready) => Some(ready),
                });
                if let Some(previous) = previous {
                    entries.insert(key, CacheEntry::Ready(previous));
                }
            }
        }
        result
    }

    fn mark_dirty(&self, key: &str) {
        let mut entries = self.entries.lock().expect("live store cache lock");
        if let Some(entry) = entries.get_mut(key) {
            match entry {
                CacheEntry::Ready(ready) => ready.dirty = true,
                CacheEntry::Loading { dirty, .. } => *dirty = true,
            }
        }
    }

    fn mark_dirty_where(&self, mut matches: impl FnMut(&str) -> bool) {
        let mut entries = self.entries.lock().expect("live store cache lock");
        for (key, entry) in entries.iter_mut() {
            if !matches(key) {
                continue;
            }
            match entry {
                CacheEntry::Ready(ready) => ready.dirty = true,
                CacheEntry::Loading { dirty, .. } => *dirty = true,
            }
        }
    }
}

#[derive(Clone)]
pub struct ClusterLiveStore {
    namespaces: Arc<SharedCache<Vec<NamespaceSummary>>>,
    resource_kinds: Arc<SharedCache<Vec<DiscoveredResourceKind>>>,
    resources: Arc<SharedCache<Vec<ResourceSummary>>>,
    topologies: Arc<SharedCache<ResourceTopology>>,
}

impl Default for ClusterLiveStore {
    fn default() -> Self {
        Self {
            namespaces: Arc::new(SharedCache::new("namespaces")),
            resource_kinds: Arc::new(SharedCache::new("resource_kinds")),
            resources: Arc::new(SharedCache::new("resources")),
            topologies: Arc::new(SharedCache::new("topologies")),
        }
    }
}

impl ClusterLiveStore {
    pub async fn namespaces<F, Fut>(
        &self,
        cluster_context: String,
        loader: F,
    ) -> Result<Vec<NamespaceSummary>, AppError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<Vec<NamespaceSummary>, AppError>> + Send + 'static,
    {
        self.namespaces
            .get_or_load(
                format!("context={}", cluster_context),
                CacheMode::GraceOnly,
                loader,
            )
            .await
    }

    pub async fn resource_kinds<F, Fut>(
        &self,
        cluster_context: String,
        loader: F,
    ) -> Result<Vec<DiscoveredResourceKind>, AppError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<Vec<DiscoveredResourceKind>, AppError>> + Send + 'static,
    {
        self.resource_kinds
            .get_or_load(
                format!("context={}", cluster_context),
                CacheMode::GraceOnly,
                loader,
            )
            .await
    }

    pub async fn typed_resources<F, Fut>(
        &self,
        cluster_context: String,
        kind: String,
        namespace: Option<String>,
        loader: F,
    ) -> Result<Vec<ResourceSummary>, AppError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<Vec<ResourceSummary>, AppError>> + Send + 'static,
    {
        let kind_key = typed_kind_key(&kind);
        let namespace_key = namespace_key_for_typed(&kind, namespace.as_deref());
        self.resources_for_scope(cluster_context, kind_key, namespace_key, loader)
            .await
    }

    pub async fn dynamic_resources<F, Fut>(
        &self,
        cluster_context: String,
        resource_kind: DiscoveredResourceKind,
        namespace: Option<String>,
        loader: F,
    ) -> Result<Vec<ResourceSummary>, AppError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<Vec<ResourceSummary>, AppError>> + Send + 'static,
    {
        let kind_key = dynamic_kind_key(
            &resource_kind.api_version,
            &resource_kind.plural,
            &resource_kind.kind,
        );
        let namespace_key =
            namespace_key_for_namespaced(resource_kind.namespaced, namespace.as_deref());
        self.resources_for_scope(cluster_context, kind_key, namespace_key, loader)
            .await
    }

    async fn resources_for_scope<F, Fut>(
        &self,
        cluster_context: String,
        kind_key: String,
        namespace_key: ScopeNamespace,
        loader: F,
    ) -> Result<Vec<ResourceSummary>, AppError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<Vec<ResourceSummary>, AppError>> + Send + 'static,
    {
        if let ScopeNamespace::Named(namespace) = &namespace_key {
            let all_key = resource_cache_key(&cluster_context, &kind_key, &ScopeNamespace::All);
            if let Some(rows) = self.resources.peek(&all_key, CacheMode::LiveUntilDirty) {
                eprintln!(
                    "[kubecove:backend] live_store cache_hit area=resources key={} covered_by={}",
                    resource_cache_key(&cluster_context, &kind_key, &namespace_key),
                    all_key
                );
                return Ok(rows
                    .into_iter()
                    .filter(|row| row.namespace.as_deref() == Some(namespace.as_str()))
                    .collect());
            }
        }

        self.resources
            .get_or_load(
                resource_cache_key(&cluster_context, &kind_key, &namespace_key),
                CacheMode::LiveUntilDirty,
                loader,
            )
            .await
    }

    pub async fn topology<F, Fut>(
        &self,
        cluster_context: String,
        namespaces: Vec<String>,
        loader: F,
    ) -> Result<ResourceTopology, AppError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<ResourceTopology, AppError>> + Send + 'static,
    {
        let key = topology_cache_key(&cluster_context, &namespaces);
        self.topologies
            .get_or_load(key, CacheMode::LiveUntilDirty, loader)
            .await
    }

    pub fn mark_watch_resource_dirty(
        &self,
        cluster_context: &str,
        resource_kind: &WatchResourceKind,
        namespace: Option<&str>,
    ) {
        let namespace_key =
            namespace_key_for_namespaced(resource_kind.namespaced.unwrap_or(true), namespace);
        for kind_key in watch_kind_keys(resource_kind) {
            let exact_key = resource_cache_key(cluster_context, &kind_key, &namespace_key);
            let all_key = resource_cache_key(cluster_context, &kind_key, &ScopeNamespace::All);
            self.resources.mark_dirty(&exact_key);
            self.resources.mark_dirty(&all_key);
        }
        self.topologies
            .mark_dirty_where(|key| key.starts_with(&format!("context={}|", cluster_context)));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ScopeNamespace {
    All,
    Cluster,
    Named(String),
}

fn typed_kind_key(kind: &str) -> String {
    format!("typed:{}", kind)
}

fn dynamic_kind_key(api_version: &str, plural: &str, kind: &str) -> String {
    format!("dynamic:{}:{}:{}", api_version, plural, kind)
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

fn resource_cache_key(context: &str, kind_key: &str, namespace: &ScopeNamespace) -> String {
    let namespace = match namespace {
        ScopeNamespace::All => "<all>",
        ScopeNamespace::Cluster => "<cluster>",
        ScopeNamespace::Named(namespace) => namespace,
    };
    format!(
        "context={}|kind={}|namespace={}",
        context, kind_key, namespace
    )
}

fn topology_cache_key(context: &str, namespaces: &[String]) -> String {
    let mut namespaces = namespaces.to_vec();
    namespaces.sort();
    namespaces.dedup();
    format!("context={}|namespaces={}", context, namespaces.join(","))
}

fn is_cluster_scoped_kind(kind: &str) -> bool {
    matches!(kind, "Node" | "StorageClass" | "PersistentVolume")
}

fn is_known_typed_kind(kind: &str) -> bool {
    matches!(
        kind,
        "Pod"
            | "Deployment"
            | "ReplicaSet"
            | "StatefulSet"
            | "DaemonSet"
            | "Service"
            | "Ingress"
            | "ConfigMap"
            | "Secret"
            | "PersistentVolumeClaim"
            | "Job"
            | "CronJob"
            | "Node"
            | "StorageClass"
            | "PersistentVolume"
    )
}

#[cfg(test)]
#[path = "live_store_tests.rs"]
mod live_store_tests;
