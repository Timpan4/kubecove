//! The list version a new resource watch starts from. A watch that starts at
//! "0" first replays every existing object as an ADDED event, and views re-list
//! the kind for each replay. A watch that starts at the version of the list a
//! view was served delivers only the changes after that list.
use super::cache_diagnostics::CachePayloadWeight;
use super::{
    CacheEntry, CacheMode, ClusterLiveStore, RESOURCE_FRESHNESS, ScopeNamespace, SharedCache,
    SharedLoad, namespace_key_for_namespaced, resource_cache_key, watch_kind_keys,
};
use crate::models::{ResourceSummary, WatchResourceKind};
use kube::api::ObjectList;
use std::sync::Arc;

/// Rows from one Kubernetes list and that list's `metadata.resourceVersion`.
pub struct ListedResources {
    pub rows: Vec<ResourceSummary>,
    pub resource_version: Option<String>,
}

impl ListedResources {
    pub fn from_list<P: Clone>(list: ObjectList<P>, row: impl FnMut(P) -> ResourceSummary) -> Self {
        Self {
            resource_version: list
                .metadata
                .resource_version
                .filter(|version| !version.is_empty()),
            rows: list.items.into_iter().map(row).collect(),
        }
    }
}

/// Rows that did not come from one list, such as test fixtures, have no
/// version, so a watch of them starts at "0".
impl From<Vec<ResourceSummary>> for ListedResources {
    fn from(rows: Vec<ResourceSummary>) -> Self {
        Self {
            rows,
            resource_version: None,
        }
    }
}

/// A cached list: the rows that reads are served and the version of that list.
pub(super) struct CachedList {
    pub(super) rows: Arc<Vec<ResourceSummary>>,
    resource_version: Option<String>,
}

impl From<ListedResources> for CachedList {
    fn from(listed: ListedResources) -> Self {
        Self {
            rows: Arc::new(listed.rows),
            resource_version: listed.resource_version,
        }
    }
}

impl CachePayloadWeight for CachedList {
    fn retained_items(&self) -> usize {
        self.rows.retained_items()
    }
    fn shallow_payload_bytes(&self) -> usize {
        size_of::<Self>() + self.rows.shallow_payload_bytes()
    }
}

enum Served {
    Ready(Option<String>),
    Loading {
        load_id: u64,
        future: SharedLoad<CachedList>,
    },
    Nothing,
}

impl SharedCache<CachedList> {
    /// What a read of `key` is served now: a clean value inside the read's
    /// freshness window, or, with `join_loading`, the clean load in flight.
    fn served(&self, key: &str, join_loading: bool) -> Served {
        let entries = self.entries.lock().expect("live store cache lock");
        match entries.get(key) {
            Some(CacheEntry::Ready(ready))
                if Self::can_reuse(ready, CacheMode::LiveFor(RESOURCE_FRESHNESS)) =>
            {
                Served::Ready(ready.value.resource_version.clone())
            }
            Some(CacheEntry::Loading {
                load_id,
                future,
                dirty: false,
                ..
            }) if join_loading => Served::Loading {
                load_id: *load_id,
                future: future.clone(),
            },
            _ => Served::Nothing,
        }
    }

    /// The version of the list a read of `key` is served, waiting for a load in
    /// flight so the watch starts at the snapshot that read returns. The outer
    /// `None` means no read would be served from this key; the inner `None`
    /// means the served rows have no usable version.
    async fn served_version(&self, key: &str, join_loading: bool) -> Option<Option<String>> {
        let (load_id, future) = match self.served(key, join_loading) {
            Served::Ready(version) => return Some(version),
            Served::Nothing => return None,
            Served::Loading { load_id, future } => (load_id, future),
        };
        // Joining does not count as a waiter, so the view's reads still own
        // the load: when they are all dropped, the load ends as cancelled.
        let Ok(list) = future.await else {
            return Some(None);
        };
        let entries = self.entries.lock().expect("live store cache lock");
        let clean = match entries.get(key) {
            Some(CacheEntry::Ready(ready)) => Arc::ptr_eq(&ready.value, &list) && !ready.dirty,
            Some(CacheEntry::Loading {
                load_id: active,
                dirty,
                ..
            }) => *active == load_id && !*dirty,
            None => false,
        };
        // A change seen while the list loaded may predate its snapshot.
        Some(list.resource_version.clone().filter(|_| clean))
    }
}

impl ClusterLiveStore {
    /// The version a new watch of `resource_kind` in `namespace` starts from:
    /// the version of the cached list a read of the same scope is served. A
    /// named namespace is served from a fresh all-namespace list first, as
    /// reads are. `None` when no clean list exists, so the watch starts at "0".
    pub async fn watch_start_version(
        &self,
        source_key: &str,
        cluster_context: &str,
        resource_kind: &WatchResourceKind,
        namespace: Option<&str>,
    ) -> Option<String> {
        let namespace =
            namespace_key_for_namespaced(resource_kind.namespaced.unwrap_or(true), namespace);
        for kind_key in watch_kind_keys(resource_kind) {
            if matches!(namespace, ScopeNamespace::Named(_)) {
                let all = resource_cache_key(
                    source_key,
                    cluster_context,
                    &kind_key,
                    &ScopeNamespace::All,
                );
                if let Some(version) = self.resources.served_version(&all, false).await {
                    return version;
                }
            }
            let exact = resource_cache_key(source_key, cluster_context, &kind_key, &namespace);
            if let Some(version) = self.resources.served_version(&exact, true).await {
                return version;
            }
        }
        None
    }
}
