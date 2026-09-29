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

impl<T: Clone + Send + Sync + CachePayloadWeight + 'static> SharedCache<T> {
    fn clear_diagnostic_counters(&self) {
        let _entries = self.entries.lock().expect("live store cache lock");
        for counter in [
            &self.counters.hits,
            &self.counters.misses,
            &self.counters.joins,
            &self.counters.evictions,
            &self.counters.restored_reload_failures,
        ] {
            counter.store(0, Ordering::Relaxed);
        }
    }

    pub(super) fn diagnostics(&self) -> CacheDiagnosticSnapshot {
        let entries = self.entries.lock().expect("live store cache lock");
        let mut snapshot = CacheDiagnosticSnapshot {
            label: self.label.to_string(),
            weight_kind: "shallow-payload-lower-bound".to_string(),
            hits: self.counters.hits.load(Ordering::Relaxed),
            misses: self.counters.misses.load(Ordering::Relaxed),
            joins: self.counters.joins.load(Ordering::Relaxed),
            evictions: self.counters.evictions.load(Ordering::Relaxed),
            restored_reload_failures: self
                .counters
                .restored_reload_failures
                .load(Ordering::Relaxed),
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
    pub(crate) fn clear_diagnostic_counters(&self) {
        self.namespaces.clear_diagnostic_counters();
        self.resource_kinds.clear_diagnostic_counters();
        self.present_custom_resource_kinds
            .clear_diagnostic_counters();
        self.resources.clear_diagnostic_counters();
        self.topologies.clear_diagnostic_counters();
        self.flux_ownership_indexes.clear_diagnostic_counters();
    }

    pub(crate) fn diagnostics(&self) -> Vec<CacheDiagnosticSnapshot> {
        vec![
            self.namespaces.diagnostics(),
            self.resource_kinds.diagnostics(),
            self.present_custom_resource_kinds.diagnostics(),
            self.resources.diagnostics(),
            self.topologies.diagnostics(),
            self.flux_ownership_indexes.diagnostics(),
        ]
    }
}
