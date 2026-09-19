use super::*;
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
        before - entries.len()
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
                .evict_where(|key| key == context_key);
        for key in keys {
            for kind in watch_kind_keys(&key.resource_kind) {
                let kind_prefix = format!("{prefix}kind={kind}|namespace=");
                cleared += self.resources.evict_where(|cached| {
                    let Some(namespace) = cached.strip_prefix(&kind_prefix) else {
                        return false;
                    };
                    key.namespace
                        .as_deref()
                        .is_none_or(|requested| requested.is_empty() || namespace == requested)
                        || namespace == "<all>"
                        || namespace == "<cluster>"
                });
            }
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
                group: Some("".into()),
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
            cache
                .get_or_load("key".into(), CacheMode::GraceOnly, || async { Ok(2) })
                .await
                .unwrap(),
            2
        );
    }
}
