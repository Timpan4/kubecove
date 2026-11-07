//! A resource list and the `metadata.resourceVersion` it was read at. Reads
//! return the version with the rows, so a view can start its watch at exactly
//! the snapshot it shows. A watch that starts at "0" instead replays every
//! existing object as an ADDED event.
use super::cache_diagnostics::CachePayloadWeight;
use crate::models::ResourceSummary;
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

/// The rows a read is served and the version of the list they came from. A
/// named namespace served from an all-namespace list carries that list's
/// version, which is the version of the filtered snapshot too.
#[derive(Clone)]
pub struct ListSnapshot {
    pub rows: Arc<Vec<ResourceSummary>>,
    pub resource_version: Option<String>,
}

impl From<ListedResources> for ListSnapshot {
    fn from(listed: ListedResources) -> Self {
        Self {
            rows: Arc::new(listed.rows),
            resource_version: listed.resource_version,
        }
    }
}

impl CachePayloadWeight for ListSnapshot {
    fn retained_items(&self) -> usize {
        self.rows.retained_items()
    }
    fn shallow_payload_bytes(&self) -> usize {
        size_of::<Self>() + self.rows.shallow_payload_bytes()
    }
}
