//! Watch events applied to cached lists. A store built with
//! [`ClusterLiveStore::applying_watch_rows`] puts each watched object's row
//! into the clean cached lists of its kind, so the next read is served from
//! the cache instead of a new list. Any list it cannot update in place is
//! marked dirty, as every list is without it, and lists again on its next read.
//!
//! Several watches can cover one list (a namespace watch and an all-namespace
//! watch, or one kind under two keys), and a list can be read while a watch is
//! behind it. Resource versions order the two: Kubernetes guarantees they are
//! comparable integers (KEP-5504, stable in 1.35). A list already at or past an
//! event's version holds that change, so the event is skipped. A newer event
//! patches the list only when its watch covers the list's whole scope and has
//! delivered every change after the list's version. Otherwise the list goes
//! dirty, as does any list whose version is not an integer.
use super::list::RowSlots;
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

/// The watch an event came from.
#[derive(Clone, Copy, Debug, Default)]
pub struct WatchOrigin<'a> {
    /// The namespace the watch covers, or `None` for all namespaces.
    pub namespace: Option<&'a str>,
    /// The resource version after which the watch has delivered every change
    /// before this event. `None` while it replays objects from "0", which
    /// arrive out of version order and omit deletions.
    pub position: Option<&'a str>,
}

/// What one watch event does to one clean cached list.
enum Patch {
    /// The list already holds this change or a newer one.
    Skip,
    Apply,
    /// The list may miss changes the event does not carry.
    Dirty,
}

enum Projection {
    Deferred,
    Row(Option<ResourceSummary>),
    Failed,
}

struct WatchChange<'a> {
    namespace: Option<&'a str>,
    name: &'a str,
    version: Option<&'a str>,
    event_number: Option<u64>,
    position: Option<u64>,
}

fn version_number(version: &str) -> Option<u64> {
    version.parse().ok()
}

/// Decides how an event at `event` from a watch that `covers` the list's scope
/// and has delivered every change after `position` updates `snapshot`.
fn decide(
    snapshot: &ListSnapshot,
    event: Option<u64>,
    position: Option<u64>,
    covers: bool,
) -> Patch {
    let listed = snapshot
        .resource_version
        .as_deref()
        .and_then(version_number);
    let (Some(event), Some(listed)) = (event, listed) else {
        return Patch::Dirty;
    };
    if event <= listed {
        return Patch::Skip;
    }
    match position {
        Some(position) if covers && position <= listed => Patch::Apply,
        _ => Patch::Dirty,
    }
}

impl SharedCache<ListSnapshot> {
    /// Updates a clean cached list when the watch covers it. Dirty values stay dirty,
    /// and a load in flight goes dirty because the event may predate the list.
    /// Returns true when a clean value can accept the event and still needs its projection.
    fn patch_ready(
        &self,
        key: &str,
        covers: bool,
        change: &WatchChange<'_>,
        projection: &Projection,
    ) -> bool {
        loop {
            let (base, edit, row) = {
                let mut entries = self.entries.lock().expect("live store cache lock");
                let ready = match entries.get_mut(key) {
                    Some(CacheEntry::Ready(ready)) if !ready.dirty => ready,
                    Some(CacheEntry::Loading { dirty, .. }) => {
                        *dirty = true;
                        return false;
                    }
                    Some(CacheEntry::Ready(_)) | None => return false,
                };
                match decide(&ready.value, change.event_number, change.position, covers) {
                    Patch::Skip => return false,
                    Patch::Dirty => {
                        ready.dirty = true;
                        return false;
                    }
                    Patch::Apply => {}
                }
                let row = match projection {
                    Projection::Deferred => return true,
                    Projection::Failed => {
                        ready.dirty = true;
                        return false;
                    }
                    Projection::Row(row) => row.as_ref(),
                };
                let Some(slots) = &mut ready.slots else {
                    let base = Arc::clone(&ready.value);
                    drop(entries);
                    self.install_slots(key, &base);
                    continue;
                };
                let Some(edit) = slots.edit(
                    &ready.value.rows,
                    change.namespace,
                    change.name,
                    row.is_none(),
                ) else {
                    Arc::make_mut(&mut ready.value).resource_version =
                        change.version.map(str::to_owned);
                    return false;
                };
                if let Some(snapshot) = Arc::get_mut(&mut ready.value)
                    && let Some(rows) = Arc::get_mut(&mut snapshot.rows)
                {
                    let retired = edit.apply_rows(rows, row);
                    slots.apply(edit);
                    snapshot.resource_version = change.version.map(str::to_owned);
                    drop(entries);
                    drop(retired);
                    return false;
                }
                (Arc::clone(&ready.value), edit, row)
            };

            // Readers keep their exact snapshot. Copy the surviving rows outside
            // the mutex and publish only if this is still the same clean list.
            let rows = Arc::new(edit.copied_rows(&base.rows, row));
            let mut entries = self.entries.lock().expect("live store cache lock");
            if let Some(CacheEntry::Ready(ready)) = entries.get_mut(key)
                && !ready.dirty
                && Arc::ptr_eq(&ready.value, &base)
            {
                ready.slots.as_mut().expect("indexed snapshot").apply(edit);
                ready.value = Arc::new(ListSnapshot::new(rows, change.version.map(str::to_owned)));
                return false;
            }
        }
    }

    fn install_slots(&self, key: &str, base: &Arc<ListSnapshot>) {
        let slots = Box::new(RowSlots::new(&base.rows));
        let mut entries = self.entries.lock().expect("live store cache lock");
        if let Some(CacheEntry::Ready(ready)) = entries.get_mut(key)
            && !ready.dirty
            && ready.slots.is_none()
            && Arc::ptr_eq(&ready.value, base)
        {
            ready.slots = Some(slots);
        }
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

    /// Records a watch event of `resource_kind` from `origin` for the cached
    /// lists it covers: the object's namespace and the all-namespace list of
    /// each kind key. Topologies and present custom kinds still go dirty.
    pub fn apply_watch_event(
        &self,
        source_key: &str,
        cluster_context: &str,
        resource_kind: &WatchResourceKind,
        origin: WatchOrigin<'_>,
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
        let namespaced = resource_kind.namespaced.unwrap_or(true);
        let namespace_key = namespace_key_for_namespaced(namespaced, namespace);
        let watch_scope = namespace_key_for_namespaced(namespaced, origin.namespace);
        let version = object.metadata.resource_version.as_deref();
        let change = WatchChange {
            namespace,
            name,
            version,
            event_number: version.and_then(version_number),
            position: origin.position.and_then(version_number),
        };
        let typed_key = typed_kind_key(&resource_kind.kind);
        for kind_key in watch_kind_keys(resource_kind) {
            let mut projection = if deleted {
                Projection::Row(None)
            } else {
                Projection::Deferred
            };
            for scope in [&namespace_key, &ScopeNamespace::All] {
                // A namespace watch says nothing about other namespaces, so it
                // cannot vouch for an all-namespace list.
                let covers = match &watch_scope {
                    ScopeNamespace::Named(_) => scope == &watch_scope,
                    ScopeNamespace::All | ScopeNamespace::Cluster => true,
                };
                let key = resource_cache_key(source_key, cluster_context, &kind_key, scope);
                if self
                    .resources
                    .patch_ready(&key, covers, &change, &projection)
                {
                    let projected = if kind_key == typed_key {
                        typed(&resource_kind.kind, cluster_context, object)
                    } else {
                        discovered(resource_kind).map(|kind| {
                            crate::commands::dynamic_projection::dynamic_resource_summary(
                                cluster_context,
                                &kind,
                                object,
                            )
                        })
                    };
                    projection =
                        projected.map_or(Projection::Failed, |row| Projection::Row(Some(row)));
                    // The cache may have changed while projection ran outside its lock.
                    self.resources
                        .patch_ready(&key, covers, &change, &projection);
                }
            }
        }
        self.mark_derived_dirty(source_key, cluster_context);
    }
}

#[cfg(test)]
#[path = "live_store_watch_tests.rs"]
mod tests;
