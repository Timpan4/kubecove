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
    /// Built by the first watch event applied to the cached list.
    pub(crate) slots: Option<RowSlots>,
}

impl ListSnapshot {
    pub fn new(rows: Arc<Vec<ResourceSummary>>, resource_version: Option<String>) -> Self {
        Self {
            rows,
            resource_version,
            slots: None,
        }
    }
}

impl From<ListedResources> for ListSnapshot {
    fn from(listed: ListedResources) -> Self {
        Self::new(Arc::new(listed.rows), listed.resource_version)
    }
}

impl CachePayloadWeight for ListSnapshot {
    fn retained_items(&self) -> usize {
        self.rows.retained_items()
    }
    fn shallow_payload_bytes(&self) -> usize {
        let slots = self
            .slots
            .as_ref()
            .map_or(0, |slots| slots.0.capacity() * size_of::<(u64, u32)>());
        size_of::<Self>() + self.rows.shallow_payload_bytes() + slots
    }
}

/// Row positions sorted by a hash of each row's namespace and name, so a watch
/// event finds its row without scanning the list.
#[derive(Clone)]
pub(crate) struct RowSlots(Vec<(u64, u32)>);

fn slot_hash(namespace: Option<&str>, name: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::hash::DefaultHasher::new();
    (namespace, name).hash(&mut hasher);
    hasher.finish()
}

fn slot_index(index: usize) -> u32 {
    u32::try_from(index).expect("a list holds fewer than 2^32 rows")
}

impl RowSlots {
    pub(crate) fn new(rows: &[ResourceSummary]) -> Self {
        let mut slots: Vec<_> = rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                (
                    slot_hash(row.namespace.as_deref(), &row.name),
                    slot_index(index),
                )
            })
            .collect();
        slots.sort_unstable();
        Self(slots)
    }

    /// Replaces, adds or (with no `row`) removes the row named `name` in
    /// `namespace`, keeping the slots in step with `rows`.
    pub(crate) fn apply(
        &mut self,
        rows: &mut Vec<ResourceSummary>,
        namespace: Option<&str>,
        name: &str,
        row: Option<ResourceSummary>,
    ) {
        let hash = slot_hash(namespace, name);
        let start = self.0.partition_point(|&(slot, _)| slot < hash);
        let held = self.0[start..]
            .iter()
            .take_while(|&&(slot, _)| slot == hash)
            .position(|&(_, index)| {
                let held = &rows[index as usize];
                held.namespace.as_deref() == namespace && held.name == name
            })
            .map(|offset| start + offset);
        match (held, row) {
            (Some(slot), Some(row)) => rows[self.0[slot].1 as usize] = row,
            (None, Some(row)) => {
                let at = self.0.partition_point(|&(slot, _)| slot <= hash);
                self.0.insert(at, (hash, slot_index(rows.len())));
                rows.push(row);
            }
            (Some(slot), None) => {
                let (_, index) = self.0.remove(slot);
                rows.remove(index as usize);
                for (_, later) in &mut self.0 {
                    if *later > index {
                        *later -= 1;
                    }
                }
            }
            (None, None) => {}
        }
    }
}
