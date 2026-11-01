use super::{
    AtomicU64, CacheEntry, ClusterLiveStore, FluxOwnershipIndex, Ordering, ResourceTopology,
    SharedCache,
};
pub(super) use crate::models::BackendCacheDiagnosticSnapshot as CacheDiagnosticSnapshot;
use std::mem::size_of;

#[derive(Default)]
pub(super) struct CacheCounters {
    pub hits: AtomicU64,
    pub misses: AtomicU64,
    pub joins: AtomicU64,
    pub evictions: AtomicU64,
    pub restored_reload_failures: AtomicU64,
}

impl CacheCounters {
    fn pairs<'a>(&'a self, other: &'a Self) -> [(&'a AtomicU64, &'a AtomicU64); 5] {
        [
            (&self.hits, &other.hits),
            (&self.misses, &other.misses),
            (&self.joins, &other.joins),
            (&self.evictions, &other.evictions),
            (
                &self.restored_reload_failures,
                &other.restored_reload_failures,
            ),
        ]
    }
}

pub(super) trait CachePayloadWeight {
    fn retained_items(&self) -> usize;
    fn shallow_payload_bytes(&self) -> usize;
}

impl<T> CachePayloadWeight for Vec<T> {
    fn retained_items(&self) -> usize {
        self.len()
    }
    fn shallow_payload_bytes(&self) -> usize {
        size_of::<Self>() + self.len() * size_of::<T>()
    }
}

impl CachePayloadWeight for ResourceTopology {
    fn retained_items(&self) -> usize {
        self.nodes.len() + self.edges.len() + self.warnings.len()
    }
    fn shallow_payload_bytes(&self) -> usize {
        size_of::<Self>()
            + self.nodes.len() * size_of::<crate::models::TopologyNode>()
            + self.edges.len() * size_of::<crate::models::TopologyEdge>()
            + self.warnings.len() * size_of::<String>()
    }
}

impl CachePayloadWeight for FluxOwnershipIndex {
    fn retained_items(&self) -> usize {
        self.retained_item_count()
    }
    fn shallow_payload_bytes(&self) -> usize {
        self.shallow_payload_bytes()
    }
}

impl<T: Send + Sync + CachePayloadWeight + 'static> SharedCache<T> {
    fn clear_diagnostic_counters(&self) {
        let _entries = self.entries.lock().expect("live store cache lock");
        for (counter, cleared) in self.counters.pairs(&self.cleared) {
            cleared.store(counter.load(Ordering::Relaxed), Ordering::Relaxed);
        }
    }

    /// Counters since the last clear, as the diagnostics table shows them.
    #[cfg(test)]
    pub(super) fn diagnostics(&self) -> CacheDiagnosticSnapshot {
        self.snapshot(true)
    }

    fn snapshot(&self, since_clear: bool) -> CacheDiagnosticSnapshot {
        let entries = self.entries.lock().expect("live store cache lock");
        let count = |counter: &AtomicU64, cleared: &AtomicU64| {
            let total = counter.load(Ordering::Relaxed);
            if since_clear {
                total.saturating_sub(cleared.load(Ordering::Relaxed))
            } else {
                total
            }
        };
        let (counters, cleared) = (&self.counters, &self.cleared);
        let mut snapshot = CacheDiagnosticSnapshot {
            label: self.label.to_string(),
            weight_kind: "shallow-payload-lower-bound".to_string(),
            hits: count(&counters.hits, &cleared.hits),
            misses: count(&counters.misses, &cleared.misses),
            joins: count(&counters.joins, &cleared.joins),
            evictions: count(&counters.evictions, &cleared.evictions),
            restored_reload_failures: count(
                &counters.restored_reload_failures,
                &cleared.restored_reload_failures,
            ),
            ..Default::default()
        };
        for entry in entries.values() {
            let retained = match entry {
                CacheEntry::Ready(ready) => {
                    snapshot.ready += 1;
                    if ready.dirty {
                        snapshot.dirty += 1;
                    }
                    Some(ready)
                }
                CacheEntry::Loading {
                    previous, dirty, ..
                } => {
                    snapshot.loading += 1;
                    if *dirty || previous.as_ref().is_some_and(|ready| ready.dirty) {
                        snapshot.dirty += 1;
                    }
                    previous.as_ref()
                }
            };
            if let Some(ready) = retained {
                snapshot.retained_items += ready.value.retained_items();
                snapshot.shallow_payload_bytes += ready.value.shallow_payload_bytes();
            }
        }
        snapshot
    }
}

impl ClusterLiveStore {
    pub fn clear_diagnostic_counters(&self) {
        self.namespaces.clear_diagnostic_counters();
        self.resource_kinds.clear_diagnostic_counters();
        self.present_custom_resource_kinds
            .clear_diagnostic_counters();
        self.resources.clear_diagnostic_counters();
        self.topologies.clear_diagnostic_counters();
        self.flux_ownership_indexes.clear_diagnostic_counters();
    }

    pub fn diagnostics(&self) -> Vec<CacheDiagnosticSnapshot> {
        self.snapshots(true)
    }

    /// Like [`Self::diagnostics`], but counters cover the store's whole life
    /// and ignore diagnostics clears. Server metrics export these.
    pub fn lifetime_diagnostics(&self) -> Vec<CacheDiagnosticSnapshot> {
        self.snapshots(false)
    }

    fn snapshots(&self, since_clear: bool) -> Vec<CacheDiagnosticSnapshot> {
        vec![
            self.namespaces.snapshot(since_clear),
            self.resource_kinds.snapshot(since_clear),
            self.present_custom_resource_kinds.snapshot(since_clear),
            self.resources.snapshot(since_clear),
            self.topologies.snapshot(since_clear),
            self.flux_ownership_indexes.snapshot(since_clear),
        ]
    }
}
