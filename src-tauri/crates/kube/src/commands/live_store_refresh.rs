use super::{CacheEntry, ClusterLiveStore, SharedCache, context_cache_key, watch_kind_keys};
use crate::models::WatchResourceKey;

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
