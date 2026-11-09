//! Reconcile watch reads while preserving keys whose inputs did not change.
use super::PreparedRows;
use kubecove_lib::models::{ResourceSummary, WatchResourceKey};
use std::{
    collections::{HashMap, HashSet},
    hash::{DefaultHasher, Hash, Hasher},
};
pub(crate) type Name = (Option<String>, String);

#[derive(Default)]
pub(crate) struct ObservedNames(Vec<Name>);
impl ObservedNames {
    pub(crate) fn insert(&mut self, name: Name) {
        self.0.push(name);
    }
    pub(crate) fn to_vec(&self) -> Vec<Name> {
        self.0.clone()
    }
    fn ordered_match(&self, index: usize, row: &ResourceSummary) -> bool {
        let (namespace, name) = &self.0[index];
        *namespace == row.namespace && *name == row.name
    }
}
impl FromIterator<Name> for ObservedNames {
    fn from_iter<T: IntoIterator<Item = Name>>(names: T) -> Self {
        Self(names.into_iter().collect())
    }
}

#[derive(Default)]
struct NameIndex {
    cluster: HashSet<String>,
    namespaces: HashMap<String, HashSet<String>>,
}
impl From<ObservedNames> for NameIndex {
    fn from(names: ObservedNames) -> Self {
        let mut index = Self::default();
        for (namespace, name) in names.0 {
            match namespace {
                Some(namespace) => {
                    index.namespaces.entry(namespace).or_default().insert(name);
                }
                None => {
                    index.cluster.insert(name);
                }
            }
        }
        index
    }
}
impl NameIndex {
    fn len(&self) -> usize {
        self.cluster.len() + self.namespaces.values().map(HashSet::len).sum::<usize>()
    }
    fn contains(&self, namespace: Option<&str>, name: &str) -> bool {
        match namespace {
            Some(namespace) => self
                .namespaces
                .get(namespace)
                .is_some_and(|names| names.contains(name)),
            None => self.cluster.contains(name),
        }
    }
    fn iter(&self) -> impl Iterator<Item = (Option<&str>, &str)> {
        let cluster = self.cluster.iter().map(|name| (None, name.as_str()));
        let namespaced = self.namespaces.iter().flat_map(|(namespace, names)| {
            names
                .iter()
                .map(move |name| (Some(namespace.as_str()), name.as_str()))
        });
        cluster.chain(namespaced)
    }
}

enum Coverage {
    All,
    Named(NameIndex),
}
impl Coverage {
    fn of(names: Option<ObservedNames>, rows: &[ResourceSummary]) -> Self {
        let Some(names) = names else {
            return Self::All;
        };
        if names.0.len() == rows.len()
            && rows
                .iter()
                .enumerate()
                .all(|(index, row)| names.ordered_match(index, row))
        {
            return Self::All;
        }
        let names = NameIndex::from(names);
        if names.len() == rows.len()
            && rows
                .iter()
                .all(|row| names.contains(row.namespace.as_deref(), &row.name))
        {
            Self::All
        } else {
            Self::Named(names)
        }
    }
}

fn same_projection(a: &ResourceSummary, b: &ResourceSummary) -> bool {
    let same_owner = match (&a.git_ops_owner, &b.git_ops_owner) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            a.provider == b.provider
                && a.kind == b.kind
                && a.name == b.name
                && a.namespace == b.namespace
                && a.partial == b.partial
                && a.confidence == b.confidence
                && a.provenance == b.provenance
        }
        _ => false,
    };
    a.plural == b.plural
        && a.group == b.group
        && a.owner_ref == b.owner_ref
        && a.argo_app == b.argo_app
        && a.helm_release == b.helm_release
        && a.git_ops_ownership_partial == b.git_ops_ownership_partial
        && same_owner
}

fn same_identity(a: &ResourceSummary, b: &ResourceSummary) -> bool {
    a.name == b.name
        && a.kind == b.kind
        && a.namespace == b.namespace
        && a.cluster == b.cluster
        && a.api_version == b.api_version
        && a.dynamic == b.dynamic
}

fn same_keys(a: &ResourceSummary, b: &ResourceSummary) -> bool {
    same_identity(a, b) && same_projection(a, b)
}

/// Whether a watch of `key` lists `row`.
fn in_key(key: &WatchResourceKey, row: &ResourceSummary) -> bool {
    row.kind == key.resource_kind.kind
        && (key.resource_kind.api_version.is_some() || row.dynamic != Some(true))
        && key
            .resource_kind
            .api_version
            .as_ref()
            .is_none_or(|api| row.api_version.as_ref() == Some(api))
        && key
            .namespace
            .as_ref()
            .is_none_or(|ns| row.namespace.as_ref() == Some(ns))
}

fn slot_hash(kind: &str, namespace: Option<&str>, name: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    (kind, namespace, name).hash(&mut hasher);
    hasher.finish()
}

pub(crate) fn reconcile(
    current: &mut Vec<ResourceSummary>,
    key: &WatchResourceKey,
    rows: Vec<ResourceSummary>,
    names: Option<ObservedNames>,
) -> bool {
    let names = names.map(NameIndex::from);
    reconcile_indexed(current, key, rows, names.as_ref())
}

fn reconcile_indexed(
    current: &mut Vec<ResourceSummary>,
    key: &WatchResourceKey,
    rows: Vec<ResourceSummary>,
    names: Option<&NameIndex>,
) -> bool {
    let matches = |row: &ResourceSummary| {
        in_key(key, row)
            && names
                .as_ref()
                .is_none_or(|names| names.contains(row.namespace.as_deref(), &row.name))
    };
    let mut next: HashMap<_, _> = rows
        .into_iter()
        .filter(&matches)
        .map(|row| ((row.namespace.clone(), row.name.clone()), row))
        .collect();
    let mut keys_changed = false;
    current.retain_mut(|row| {
        if !matches(row) {
            return true;
        }
        if let Some(new) = next.remove(&(row.namespace.clone(), row.name.clone())) {
            keys_changed |= !same_keys(row, &new);
            *row = new;
            true
        } else {
            keys_changed = true;
            false
        }
    });
    keys_changed |= !next.is_empty();
    current.extend(next.into_values());
    keys_changed
}

/// What a watch patch did to the prepared rows.
pub(crate) struct Patched {
    /// The read's buffer, for the next read to reuse off the UI thread.
    pub recycled: Vec<ResourceSummary>,
    /// Indices of the rows replaced in place, or `None` when any row may have
    /// changed or moved.
    pub changed: Option<Vec<usize>>,
}

impl PreparedRows {
    pub(crate) fn patch(
        &mut self,
        key: &WatchResourceKey,
        mut rows: Vec<ResourceSummary>,
        names: Option<ObservedNames>,
    ) -> Patched {
        // Watch rows carry the backend's owners, so compare them with those. If no
        // key changes, the rows keep their positions and the same inherited owners.
        self.swap_owners();
        let kind = self
            .kinds
            .iter()
            .position(|(kind, _)| *kind == key.resource_kind.kind);
        let names = match kind {
            Some(kind) => match self.patch_members(kind, key, &mut rows, names) {
                Ok(changed) => {
                    return Patched {
                        recycled: rows,
                        changed,
                    };
                }
                Err(names) => names,
            },
            None => names.map(NameIndex::from),
        };
        let _span = crate::frame_profile::scope("watch", "reconcile");
        if reconcile_indexed(&mut self.rows, key, rows, names.as_ref()) {
            self.rebuild();
        } else {
            self.swap_owners();
        }
        Patched {
            recycled: Vec::new(),
            changed: None,
        }
    }

    /// Patches the listed rows of a kind the view holds in place. `Err` hands
    /// the names back, with the rows untouched, when the read adds or deletes
    /// rows of the kind in a way only a reconcile resolves.
    fn patch_members(
        &mut self,
        kind: usize,
        key: &WatchResourceKey,
        rows: &mut Vec<ResourceSummary>,
        names: Option<ObservedNames>,
    ) -> Result<Option<Vec<usize>>, Option<NameIndex>> {
        let members = &self.by_kind[kind];
        let span = crate::frame_profile::scope("watch", "ordered_check");
        let ordered_names = names.as_ref().filter(|names| names.0.len() == rows.len());
        let mut complete = names.is_none() || ordered_names.is_some();
        let ordered = rows.len() == members.len()
            && members
                .iter()
                .zip(rows.iter())
                .enumerate()
                .all(|(position, (&index, row))| {
                    complete &=
                        ordered_names.is_none_or(|names| names.ordered_match(position, row));
                    same_identity(&self.rows[index], row) && in_key(key, row)
                });
        let coverage = if ordered && complete {
            Coverage::All
        } else if ordered {
            Coverage::of(names, rows)
        } else {
            names.map_or(Coverage::All, |names| {
                Coverage::Named(NameIndex::from(names))
            })
        };
        drop(span);
        let _span = crate::frame_profile::scope("watch", "replace_rows");
        match coverage {
            Coverage::All if ordered => {
                let keys_changed = members
                    .iter()
                    .zip(rows.iter())
                    .any(|(&index, row)| !same_projection(&self.rows[index], row));
                if members.len() == self.rows.len() {
                    std::mem::swap(&mut self.rows, rows);
                } else {
                    for (&index, row) in members.iter().zip(rows.iter_mut()) {
                        std::mem::swap(&mut self.rows[index], row);
                    }
                }
                let changed = (!keys_changed).then(|| members.clone());
                Ok(self.settle(changed))
            }
            Coverage::Named(names) if ordered => {
                let mut changed = Some(Vec::new());
                for (&index, row) in members.iter().zip(rows.iter_mut()) {
                    if names.contains(row.namespace.as_deref(), &row.name) {
                        if !same_projection(&self.rows[index], row) {
                            changed = None;
                        }
                        std::mem::swap(&mut self.rows[index], row);
                        if let Some(changed) = &mut changed {
                            changed.push(index);
                        }
                    }
                }
                Ok(self.settle(changed))
            }
            Coverage::All => Err(None),
            Coverage::Named(names) => match self.replace_named(key, rows, &names) {
                Some(changed) => Ok(self.settle(Some(changed))),
                None => Err(Some(names)),
            },
        }
    }

    /// Restores inherited owners after rows were replaced in place, or prepares
    /// the rows again when a replaced row changed its keys (`changed` is `None`).
    fn settle(&mut self, changed: Option<Vec<usize>>) -> Option<Vec<usize>> {
        if changed.is_some() {
            self.swap_owners();
        } else {
            self.rebuild();
        }
        changed
    }

    /// Replaces each named row with its row in `rows` when every name is one
    /// row the view holds and the read still lists, with the same keys.
    /// Returns `None`, with nothing replaced, when a name was added, deleted,
    /// held twice or changed keys.
    fn replace_named(
        &mut self,
        key: &WatchResourceKey,
        rows: &mut [ResourceSummary],
        names: &NameIndex,
    ) -> Option<Vec<usize>> {
        // The read's named rows, sorted by slot hash like the view's slots.
        let read = slot_table(rows.iter().enumerate().filter(|(_, row)| {
            in_key(key, row) && names.contains(row.namespace.as_deref(), &row.name)
        }));
        let current = &self.rows;
        let slots = self
            .slots
            .get_or_insert_with(|| slot_table(current.iter().enumerate()));
        let mut pairs = Vec::with_capacity(read.len());
        for (namespace, name) in names.iter() {
            let hash = slot_hash(&key.resource_kind.kind, namespace, name);
            let held = find_slot(slots, current, hash, key, namespace, name).ok()?;
            let listed = find_slot(&read, rows, hash, key, namespace, name).ok()?;
            match (held, listed) {
                (Some(index), Some(position)) if same_keys(&current[index], &rows[position]) => {
                    pairs.push((index, position));
                }
                (None, None) => {}
                _ => return None,
            }
        }
        for &(index, position) in &pairs {
            std::mem::swap(&mut self.rows[index], &mut rows[position]);
        }
        Some(pairs.into_iter().map(|(index, _)| index).collect())
    }
}

/// Row indices sorted by the hash of their kind, namespace and name.
fn slot_table<'a>(rows: impl Iterator<Item = (usize, &'a ResourceSummary)>) -> Vec<(u64, u32)> {
    let mut slots: Vec<_> = rows
        .map(|(index, row)| {
            (
                slot_hash(&row.kind, row.namespace.as_deref(), &row.name),
                u32::try_from(index).expect("row index fits u32"),
            )
        })
        .collect();
    slots.sort_unstable();
    slots
}

/// The one row in `slots` that a watch of `key` lists under this name.
/// `Ok(None)` when there is none; `Err` when two rows share the name.
fn find_slot(
    slots: &[(u64, u32)],
    rows: &[ResourceSummary],
    hash: u64,
    key: &WatchResourceKey,
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
            && in_key(key, row)
            && found.replace(index).is_some()
        {
            return Err(());
        }
    }
    Ok(found)
}
