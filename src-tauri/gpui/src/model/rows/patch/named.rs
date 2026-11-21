//! Name-based replacement of existing resources without rebuilding their keys.

use super::{PreparedRows, ResourceSummary, Watch, same_identity, same_projection, slot_hash};

impl PreparedRows {
    /// Replaces each named row with its row in `rows` when every name is one
    /// row the view holds and the read still lists, with the same keys.
    /// Returns `None`, with nothing replaced, when a name was added, deleted,
    /// held twice or changed keys.
    pub(super) fn replace_named<'a>(
        &mut self,
        watch: Watch<'_>,
        rows: &mut [ResourceSummary],
        names: impl Iterator<Item = (Option<&'a str>, &'a str)>,
    ) -> Option<Vec<usize>> {
        // The read's named rows, sorted by slot hash like the view's slots.
        let read = slot_table(rows.iter().enumerate().filter(|(_, row)| watch.lists(row)));
        let current = &self.rows;
        let slots = self
            .slots
            .get_or_insert_with(|| slot_table(current.iter().enumerate()));
        let mut pairs = Vec::with_capacity(read.len());
        for (namespace, name) in names {
            let hash = watch.slot(namespace, name);
            let (Ok(held), Ok(listed)) = (
                find_slot(slots, current, hash, watch, namespace, name),
                find_slot(&read, rows, hash, watch, namespace, name),
            ) else {
                return None;
            };
            match (held, listed) {
                (Some(index), Some(position)) => pairs.push((index, position)),
                (None, None) => {}
                _ => return None,
            }
        }
        pairs.sort_unstable();
        pairs.dedup();
        if pairs.iter().any(|&(index, position)| {
            let old = &self.rows[index];
            let new = &rows[position];
            !same_identity(old, new) || !same_projection(old, new, self.backend_owner(index))
        }) {
            return None;
        }
        for &(index, position) in &pairs {
            self.replace_row(index, &mut rows[position]);
        }
        Some(pairs.into_iter().map(|(index, _)| index).collect())
    }

    pub(super) fn replace_row(&mut self, index: usize, incoming: &mut ResourceSummary) {
        let current = &mut self.rows[index];
        let inherited = self.inherited.binary_search_by_key(&index, |(row, _)| *row);
        if let Ok(position) = inherited {
            let raw = &mut self.inherited[position].1;
            std::mem::swap(&mut current.git_ops_owner, raw);
            std::mem::swap(current, incoming);
            std::mem::swap(&mut current.git_ops_owner, raw);
        } else {
            std::mem::swap(current, incoming);
        }
    }
}

/// Row indices sorted by the hash of their cluster, kind, namespace and name.
fn slot_table<'a>(rows: impl Iterator<Item = (usize, &'a ResourceSummary)>) -> Vec<(u64, u32)> {
    let mut slots: Vec<_> = rows
        .map(|(index, row)| {
            (
                slot_hash(&row.cluster, &row.kind, row.namespace.as_deref(), &row.name),
                u32::try_from(index).expect("row index fits u32"),
            )
        })
        .collect();
    slots.sort_unstable();
    slots
}

/// The one row in `slots` that `watch` lists under this name.
/// `Ok(None)` when there is none; `Err` when two rows share the name.
fn find_slot(
    slots: &[(u64, u32)],
    rows: &[ResourceSummary],
    hash: u64,
    watch: Watch<'_>,
    namespace: Option<&str>,
    name: &str,
) -> Result<Option<usize>, ()> {
    let start = slots.partition_point(|&(slot, _)| slot < hash);
    let mut found = None;
    for &(_, index) in slots[start..].iter().take_while(|(slot, _)| *slot == hash) {
        let index = index as usize;
        let row = &rows[index];
        if row.namespace.as_deref() == namespace
            && row.name == name
            && watch.lists(row)
            && found.replace(index).is_some()
        {
            return Err(());
        }
    }
    Ok(found)
}
