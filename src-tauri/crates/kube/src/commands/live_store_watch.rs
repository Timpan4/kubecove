//! Watch events applied to cached lists. A store built with
//! [`ClusterLiveStore::applying_watch_rows`] puts each watched object's row
//! into the clean cached lists of its kind, so the next read is served from
//! the cache instead of a new list. Any list it cannot update in place is
//! marked dirty, as every list is without it, and lists again on its next read.
use super::{
    CacheEntry, ClusterLiveStore, ListSnapshot, ScopeNamespace, SharedCache,
    namespace_key_for_namespaced, resource_cache_key, typed_kind_key, watch_kind_keys,
};
use crate::models::{DiscoveredResourceKind, ResourceSummary, WatchResourceKind};
use kube::api::{DynamicObject, WatchEvent};
use std::sync::Arc;

/// The row a typed list of `kind` (first argument) in `cluster_context`
/// (second) holds for a watched object, or `None` when the kind has no typed
/// list. It must match the list's own projection.
pub type WatchedRow = fn(&str, &str, &DynamicObject) -> Option<ResourceSummary>;

impl<T: Clone> SharedCache<T> {
    /// Changes a clean cached value in place. A dirty value stays dirty, and a
    /// load in flight is marked dirty, because the change may predate its list.
    fn patch_ready(&self, key: &str, patch: impl FnOnce(&mut T)) {
        let mut entries = self.entries.lock().expect("live store cache lock");
        match entries.get_mut(key) {
            Some(CacheEntry::Ready(ready)) if !ready.dirty => {
                patch(Arc::make_mut(&mut ready.value))
            }
            Some(CacheEntry::Loading { dirty, .. }) => *dirty = true,
            Some(CacheEntry::Ready(_)) | None => {}
        }
    }
}

/// Replaces, adds or (with no `row`) removes the object named `name` in
/// `namespace`, and moves the list version to the event's.
fn apply_row(
    snapshot: &mut ListSnapshot,
    namespace: Option<&str>,
    name: &str,
    row: Option<ResourceSummary>,
    resource_version: Option<&str>,
) {
    let rows = Arc::make_mut(&mut snapshot.rows);
    let held = rows
        .iter()
        .position(|row| row.namespace.as_deref() == namespace && row.name == name);
    match (held, row) {
        (Some(index), Some(row)) => rows[index] = row,
        (None, Some(row)) => rows.push(row),
        (Some(index), None) => {
            rows.remove(index);
        }
        (None, None) => {}
    }
    if let Some(version) = resource_version {
        snapshot.resource_version = Some(version.to_owned());
    }
}

fn discovered(kind: &WatchResourceKind) -> Option<DiscoveredResourceKind> {
    Some(DiscoveredResourceKind {
        group: kind.group.clone()?,
        version: kind.version.clone()?,
        api_version: kind.api_version.clone()?,
        plural: kind.plural.clone()?,
        kind: kind.kind.clone(),
        namespaced: kind.namespaced.unwrap_or(true),
        short_names: Vec::new(),
    })
}

impl ClusterLiveStore {
    /// Applies watch events to cached lists, with `typed` projecting objects
    /// of typed lists. Without it, a watch event marks the lists dirty.
    #[must_use]
    pub fn applying_watch_rows(mut self, typed: WatchedRow) -> Self {
        self.watched_rows = Some(typed);
        self
    }

    /// Records a watch event of `resource_kind` for the cached lists it
    /// covers: the object's namespace and the all-namespace list of each kind
    /// key. Topologies and present custom kinds still go dirty.
    pub fn apply_watch_event(
        &self,
        source_key: &str,
        cluster_context: &str,
        resource_kind: &WatchResourceKind,
        event: &WatchEvent<DynamicObject>,
    ) {
        let (object, deleted) = match event {
            WatchEvent::Added(object) | WatchEvent::Modified(object) => (object, false),
            WatchEvent::Deleted(object) => (object, true),
            WatchEvent::Bookmark(_) | WatchEvent::Error(_) => return,
        };
        let namespace = object.metadata.namespace.as_deref();
        let (Some(typed), Some(name)) = (self.watched_rows, object.metadata.name.as_deref()) else {
            self.mark_watch_resource_dirty(source_key, cluster_context, resource_kind, namespace);
            return;
        };
        let namespace_key =
            namespace_key_for_namespaced(resource_kind.namespaced.unwrap_or(true), namespace);
        let version = object.metadata.resource_version.as_deref();
        for kind_key in watch_kind_keys(resource_kind) {
            let keys = [&namespace_key, &ScopeNamespace::All]
                .map(|scope| resource_cache_key(source_key, cluster_context, &kind_key, scope));
            let row = if deleted {
                Ok(None)
            } else if kind_key == typed_kind_key(&resource_kind.kind) {
                typed(&resource_kind.kind, cluster_context, object)
                    .map(Some)
                    .ok_or(())
            } else {
                discovered(resource_kind)
                    .map(|kind| {
                        crate::commands::dynamic_projection::dynamic_resource_summary(
                            cluster_context,
                            &kind,
                            object,
                        )
                    })
                    .map(Some)
                    .ok_or(())
            };
            for key in &keys {
                match &row {
                    Ok(row) => self.resources.patch_ready(key, |snapshot| {
                        apply_row(snapshot, namespace, name, row.clone(), version);
                    }),
                    Err(()) => self.resources.mark_dirty(key),
                }
            }
        }
        self.mark_derived_dirty(source_key, cluster_context);
    }
}

#[cfg(test)]
#[path = "live_store_watch_tests.rs"]
mod tests;
