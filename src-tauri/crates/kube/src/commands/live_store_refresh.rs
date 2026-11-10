use super::{
    CacheEntry, CacheMode, ClusterLiveStore, SharedCache, context_cache_key, watch_kind_keys,
};
use crate::models::{AppError, WatchResourceKey};
use std::{sync::Arc, sync::atomic::Ordering, time::Duration};

impl<T> SharedCache<T>
where
    T: Send + Sync + 'static,
{
    /// Serves a clean cached value at once, even past `freshness`, and reloads
    /// an expired one in the background. `TanStack Query` does the same for the
    /// web's kind catalog, so a namespace switch never waits on its refetch.
    /// A dirty, evicted or never-loaded value still loads in the foreground.
    pub(super) async fn get_or_revalidate<F, Fut>(
        self: &Arc<Self>,
        key: String,
        freshness: Duration,
        loader: F,
    ) -> Result<Arc<T>, AppError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, AppError>> + Send + 'static,
    {
        let cached = {
            let entries = self.entries.lock().expect("live store cache lock");
            match entries.get(&key) {
                Some(CacheEntry::Ready(ready)) if !ready.dirty => Some((
                    Arc::clone(&ready.value),
                    ready.completed_at.elapsed() > freshness,
                )),
                Some(CacheEntry::Loading {
                    previous: Some(previous),
                    dirty: false,
                    ..
                }) if !previous.dirty => Some((Arc::clone(&previous.value), false)),
                _ => None,
            }
        };
        let Some((value, expired)) = cached else {
            return self
                .get_or_load(key, CacheMode::LiveFor(freshness), loader)
                .await;
        };
        self.counters.hits.fetch_add(1, Ordering::Relaxed);
        if expired {
            let cache = Arc::clone(self);
            // A failed reload restores the previous value, so its error has no
            // reader here; the next read starts another reload.
            tokio::spawn(async move {
                let _ = cache
                    .get_or_load(key, CacheMode::LiveFor(freshness), loader)
                    .await;
            });
        }
        Ok(value)
    }
}

impl<T> SharedCache<T> {
    fn evict_where(&self, mut matches: impl FnMut(&str) -> bool) -> usize {
        let mut entries = self.entries.lock().expect("live store cache lock");
        let before = entries.len();
        entries.retain(|key, entry| {
            if !matches(key) {
                return true;
            }
            if let CacheEntry::Loading { cancellation, .. } = entry {
                cancellation.cancel();
            }
            false
        });
        let removed = before - entries.len();
        self.counters.evictions.fetch_add(
            u64::try_from(removed).expect("cache entry count fits u64"),
            std::sync::atomic::Ordering::Relaxed,
        );
        removed
    }
}

impl ClusterLiveStore {
    pub fn refresh_view(
        &self,
        source: &str,
        context: &str,
        keys: &[WatchResourceKey],
        namespaces: &[String],
    ) -> usize {
        let context_key = context_cache_key(source, context);
        let prefix = format!("{context_key}|");
        let mut cleared = self.namespaces.evict_where(|key| key == context_key)
            + self.resource_kinds.evict_where(|key| key == context_key)
            + self
                .flux_ownership_indexes
                .evict_where(|key| key == context_key)
            + self.resources.evict_where(covered_rows(&prefix, keys));
        let overlaps = |cached: &str| {
            cached.is_empty()
                || namespaces.is_empty()
                || cached
                    .split(',')
                    .any(|namespace| namespaces.iter().any(|selected| selected == namespace))
        };
        cleared += self.topologies.evict_where(|key| {
            key.starts_with(&prefix)
                && key
                    .rsplit_once("|namespaces=")
                    .is_some_and(|(_, scope)| overlaps(scope))
        });
        cleared += self.present_custom_resource_kinds.evict_where(|key| {
            key.strip_prefix(&format!("{prefix}present_custom_resource_kinds="))
                .is_some_and(overlaps)
        });
        cleared
    }

    /// Marks only the cached rows of `keys` stale. A watch reload re-lists one
    /// kind, so its next read must not reuse or join an older snapshot. Loads
    /// already in flight keep running for their waiters, and namespaces,
    /// kinds and topologies stay cached.
    pub fn refresh_resources(&self, source: &str, context: &str, keys: &[WatchResourceKey]) {
        let prefix = format!("{}|", context_cache_key(source, context));
        self.resources.mark_dirty_where(covered_rows(&prefix, keys));
    }
}

/// Matches cached row keys that a refresh of `keys` covers: the requested
/// namespace plus the all-namespace and cluster-scoped reads of each kind.
fn covered_rows<'a>(prefix: &str, keys: &'a [WatchResourceKey]) -> impl Fn(&str) -> bool + 'a {
    let kinds: Vec<(String, Option<&'a str>)> = keys
        .iter()
        .flat_map(|key| {
            watch_kind_keys(&key.resource_kind).into_iter().map(|kind| {
                (
                    format!("{prefix}kind={kind}|namespace="),
                    key.namespace
                        .as_deref()
                        .filter(|namespace| !namespace.is_empty()),
                )
            })
        })
        .collect();
    move |cached| {
        kinds.iter().any(|(kind_prefix, requested)| {
            cached.strip_prefix(kind_prefix).is_some_and(|namespace| {
                requested.is_none_or(|requested| namespace == requested)
                    || namespace == "<all>"
                    || namespace == "<cluster>"
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::live_store::{CacheMode, ScopeNamespace, resource_cache_key};
    use crate::models::{AppError, WatchResourceKind};
    use std::sync::Arc;

    #[tokio::test]
    async fn refresh_evicts_covered_scope_without_clearing_other_namespaces_or_contexts() {
        let store = ClusterLiveStore::default();
        for (context, namespace) in [
            ("dev", Some("apps")),
            ("dev", Some("other")),
            ("dev", None),
            ("prod", Some("apps")),
        ] {
            store
                .typed_resources(
                    "source".into(),
                    context.into(),
                    "Pod".into(),
                    namespace.map(str::to_owned),
                    || async { Ok(vec![]) },
                )
                .await
                .unwrap();
        }
        let key = WatchResourceKey {
            resource_kind: WatchResourceKind {
                kind: "Pod".into(),
                group: Some(String::new()),
                version: Some("v1".into()),
                api_version: Some("v1".into()),
                plural: Some("pods".into()),
                namespaced: Some(true),
            },
            namespace: Some("apps".into()),
        };
        store.refresh_view("source", "dev", &[key], &["apps".into()]);
        let all = resource_cache_key("source", "dev", "typed:Pod", &ScopeNamespace::All);
        let prod = resource_cache_key(
            "source",
            "prod",
            "typed:Pod",
            &ScopeNamespace::Named("apps".into()),
        );
        assert!(!store.resources.has_key(&all));
        assert!(store.resources.has_key(&prod));
        let apps = resource_cache_key(
            "source",
            "dev",
            "typed:Pod",
            &ScopeNamespace::Named("apps".into()),
        );
        let other = resource_cache_key(
            "source",
            "dev",
            "typed:Pod",
            &ScopeNamespace::Named("other".into()),
        );
        assert!(!store.resources.has_key(&apps));
        assert!(store.resources.has_key(&other));
    }

    // Failure mode: a watch reload of one kind cancelled the same kind's read
    // in flight, so the scope read waiting on it retried after 1 s.
    #[tokio::test]
    async fn kind_refresh_keeps_the_same_kind_read_in_flight() {
        let store = Arc::new(ClusterLiveStore::default());
        let (started, wait_started) = tokio::sync::oneshot::channel();
        let (finish, wait_finish) = tokio::sync::oneshot::channel::<()>();
        let loading = store.clone();
        let scope_read = tokio::spawn(async move {
            loading
                .typed_resources(
                    "source".into(),
                    "dev".into(),
                    "Pod".into(),
                    None,
                    || async move {
                        started.send(()).unwrap();
                        wait_finish.await.unwrap();
                        Ok(vec![])
                    },
                )
                .await
        });
        wait_started.await.unwrap();
        let key = WatchResourceKey {
            resource_kind: WatchResourceKind {
                kind: "Pod".into(),
                group: Some(String::new()),
                version: Some("v1".into()),
                api_version: Some("v1".into()),
                plural: Some("pods".into()),
                namespaced: Some(true),
            },
            namespace: None,
        };
        store.refresh_resources("source", "dev", &[key]);
        let reload = store
            .typed_resources(
                "source".into(),
                "dev".into(),
                "Pod".into(),
                None,
                || async { Ok(vec![]) },
            )
            .await;
        assert!(reload.is_ok());
        finish.send(()).unwrap();
        assert!(scope_read.await.unwrap().is_ok());
    }

    // Failure mode: a watch reload of one kind evicted every topology in the
    // context, so the map read in flight was cancelled and retried after 1 s.
    #[tokio::test]
    async fn kind_refresh_keeps_a_topology_load_in_flight() {
        let store = Arc::new(ClusterLiveStore::default());
        let (started, wait_started) = tokio::sync::oneshot::channel();
        let (finish, wait_finish) = tokio::sync::oneshot::channel::<()>();
        let loading = store.clone();
        let topology = tokio::spawn(async move {
            loading
                .topology(
                    "source".into(),
                    "dev".into(),
                    vec!["apps".into()],
                    "ownership".into(),
                    || async move {
                        started.send(()).unwrap();
                        wait_finish.await.unwrap();
                        Ok(crate::models::ResourceTopology {
                            nodes: Vec::new(),
                            edges: Vec::new(),
                            warnings: Vec::new(),
                        })
                    },
                )
                .await
        });
        wait_started.await.unwrap();
        let key = WatchResourceKey {
            resource_kind: WatchResourceKind {
                kind: "Pod".into(),
                group: Some(String::new()),
                version: Some("v1".into()),
                api_version: Some("v1".into()),
                plural: Some("pods".into()),
                namespaced: Some(true),
            },
            namespace: Some("apps".into()),
        };
        store.refresh_resources("source", "dev", &[key]);
        finish.send(()).unwrap();
        assert!(topology.await.unwrap().is_ok());
    }

    // Failure modes: a namespace switch waited about 360 ms on an expired kind
    // catalog; a background reload that never lands keeps serving old kinds;
    // a dirty catalog is served instead of reloaded.
    #[tokio::test]
    async fn expired_kind_catalog_is_served_while_it_reloads() {
        let cache = Arc::new(SharedCache::new("test"));
        let expired = std::time::Instant::now()
            .checked_sub(Duration::from_secs(31))
            .expect("cache age fixture");
        cache.entries.lock().unwrap().insert(
            "kinds".into(),
            CacheEntry::Ready(super::super::ReadyValue {
                value: Arc::new(1),
                completed_at: expired,
                dirty: false,
            }),
        );
        let (finish, wait_finish) = tokio::sync::oneshot::channel::<()>();
        let served = cache
            .get_or_revalidate("kinds".into(), Duration::from_secs(30), || async move {
                wait_finish.await.unwrap();
                Ok(2)
            })
            .await
            .unwrap();
        assert_eq!(*served, 1);
        finish.send(()).unwrap();
        let reloaded = loop {
            let value = cache
                .get_or_revalidate("kinds".into(), Duration::from_secs(30), || async { Ok(3) })
                .await
                .unwrap();
            if *value != 1 {
                break value;
            }
            tokio::task::yield_now().await;
        };
        assert_eq!(*reloaded, 2);

        cache.mark_dirty("kinds");
        let dirty_read = cache
            .get_or_revalidate("kinds".into(), Duration::from_secs(30), || async { Ok(4) })
            .await
            .unwrap();
        assert_eq!(*dirty_read, 4);
    }

    #[tokio::test]
    async fn eviction_cancels_old_load_and_cannot_overwrite_fresh_value() {
        let cache = Arc::new(SharedCache::new("test"));
        let (started, wait_started) = tokio::sync::oneshot::channel();
        let old_cache = cache.clone();
        let old = tokio::spawn(async move {
            old_cache
                .get_or_load("key".into(), CacheMode::GraceOnly, || async {
                    started.send(()).unwrap();
                    std::future::pending::<Result<u32, AppError>>().await
                })
                .await
        });
        wait_started.await.unwrap();
        assert_eq!(cache.evict_where(|key| key == "key"), 1);
        assert!(old.await.unwrap().is_err());
        assert_eq!(
            *cache
                .get_or_load("key".into(), CacheMode::GraceOnly, || async { Ok(2) })
                .await
                .unwrap(),
            2
        );
    }
}
