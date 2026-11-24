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

impl ListSnapshot {
    pub fn new(rows: Arc<Vec<ResourceSummary>>, resource_version: Option<String>) -> Self {
        Self {
            rows,
            resource_version,
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
        size_of::<Self>() + self.rows.shallow_payload_bytes()
    }
}

/// Row positions sorted by a hash of each row's namespace and name, so a watch
/// event finds its row without scanning the list.
pub(super) struct RowSlots(Vec<(u64, u32)>);

#[derive(Clone, Copy)]
pub(super) enum RowEdit {
    Replace(usize),
    Insert {
        slot: usize,
        hash: u64,
        index: u32,
    },
    Delete {
        slot: usize,
        index: u32,
        shift: bool,
    },
}

impl RowEdit {
    pub(super) fn copied_rows(
        self,
        rows: &[ResourceSummary],
        row: Option<&ResourceSummary>,
    ) -> Vec<ResourceSummary> {
        let removed = match self {
            Self::Replace(index) => Some(index),
            Self::Delete { index, .. } => Some(index as usize),
            Self::Insert { .. } => None,
        };
        let (before, after) = removed.map_or((rows, &[][..]), |index| {
            (&rows[..index], &rows[index + 1..])
        });
        let mut copied =
            Vec::with_capacity(before.len() + after.len() + usize::from(row.is_some()));
        copied.extend_from_slice(before);
        copied.extend(row.cloned());
        copied.extend_from_slice(after);
        copied
    }

    pub(super) fn apply_rows(
        self,
        rows: &mut Vec<ResourceSummary>,
        row: Option<&ResourceSummary>,
    ) -> Option<ResourceSummary> {
        match (self, row) {
            (Self::Replace(index), Some(row)) => {
                Some(std::mem::replace(&mut rows[index], row.clone()))
            }
            (Self::Insert { .. }, Some(row)) => {
                rows.push(row.clone());
                None
            }
            (Self::Delete { index, .. }, None) => Some(rows.remove(index as usize)),
            _ => unreachable!("row edit and watch projection agree"),
        }
    }
}

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
    pub(super) fn new(rows: &[ResourceSummary]) -> Self {
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

    pub(super) fn shallow_payload_bytes(&self) -> usize {
        size_of::<Self>() + self.0.capacity() * size_of::<(u64, u32)>()
    }

    pub(super) fn edit(
        &self,
        rows: &[ResourceSummary],
        namespace: Option<&str>,
        name: &str,
        deleting: bool,
    ) -> Option<RowEdit> {
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
        match (held, deleting) {
            (Some(slot), false) => Some(RowEdit::Replace(self.0[slot].1 as usize)),
            (None, false) => Some(RowEdit::Insert {
                slot: self.0.partition_point(|&(slot, _)| slot <= hash),
                hash,
                index: slot_index(rows.len()),
            }),
            (Some(slot), true) => {
                let index = self.0[slot].1;
                Some(RowEdit::Delete {
                    slot,
                    index,
                    shift: (index as usize) + 1 < rows.len(),
                })
            }
            (None, true) => None,
        }
    }

    pub(super) fn apply(&mut self, edit: RowEdit) {
        match edit {
            RowEdit::Replace(_) => {}
            RowEdit::Insert { slot, hash, index } => self.0.insert(slot, (hash, index)),
            RowEdit::Delete { slot, index, shift } => {
                self.0.remove(slot);
                if shift {
                    for (_, later) in &mut self.0 {
                        if *later > index {
                            *later -= 1;
                        }
                    }
                }
            }
        }
    }
}
