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

impl<T: Clone> SharedCache<T> {
    /// Changes a clean cached value in place when `decide` allows it. A dirty
    /// value stays dirty, and a load in flight is marked dirty, because the
    /// change may predate its list.
    fn patch_ready(&self, key: &str, decide: impl FnOnce(&T) -> Patch, patch: impl FnOnce(&mut T)) {
        let mut entries = self.entries.lock().expect("live store cache lock");
        match entries.get_mut(key) {
            Some(CacheEntry::Ready(ready)) if !ready.dirty => match decide(&ready.value) {
                Patch::Skip => {}
                Patch::Apply => patch(Arc::make_mut(&mut ready.value)),
                Patch::Dirty => ready.dirty = true,
            },
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
    snapshot
        .slots
        .get_or_insert_with(|| RowSlots::new(rows))
        .apply(rows, namespace, name, row);
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
        let event_number = version.and_then(version_number);
        let position = origin.position.and_then(version_number);
        for kind_key in watch_kind_keys(resource_kind) {
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
            for scope in [&namespace_key, &ScopeNamespace::All] {
                // A namespace watch says nothing about other namespaces, so it
                // cannot vouch for an all-namespace list.
                let covers = match &watch_scope {
                    ScopeNamespace::Named(_) => scope == &watch_scope,
                    ScopeNamespace::All | ScopeNamespace::Cluster => true,
                };
                let key = resource_cache_key(source_key, cluster_context, &kind_key, scope);
                self.resources.patch_ready(
                    &key,
                    |snapshot| match (decide(snapshot, event_number, position, covers), &row) {
                        (Patch::Apply, Err(())) => Patch::Dirty,
                        (patch, _) => patch,
                    },
                    |snapshot| {
                        if let Ok(row) = &row {
                            apply_row(snapshot, namespace, name, row.clone(), version);
                        }
                    },
                );
            }
        }
        self.mark_derived_dirty(source_key, cluster_context);
    }
}

#[cfg(test)]
#[path = "live_store_watch_tests.rs"]
mod tests;
