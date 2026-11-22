//! Reconcile watch reads while preserving keys whose inputs did not change.
mod named;
use super::PreparedRows;
use kubecove_lib::models::{GitOpsOwnerSummary, ResourceSummary, WatchResourceKey};
use std::{
    collections::{HashMap, HashSet},
    hash::{DefaultHasher, Hash, Hasher},
    sync::Arc,
};
pub(crate) type Name = (Option<String>, String);

/// The names a batch of watch notices carried, in arrival order. The named read
/// shares them, and the patch takes them back once that read is done.
#[derive(Default)]
pub(crate) struct ObservedNames(Arc<Vec<Name>>);
impl ObservedNames {
    pub(crate) fn insert(&mut self, name: Name) {
        Arc::make_mut(&mut self.0).push(name);
    }
    pub(crate) fn shared(&self) -> Arc<Vec<Name>> {
        self.0.clone()
    }
    pub(crate) fn iter(&self) -> impl Iterator<Item = &Name> {
        self.0.iter()
    }
    /// The names themselves, copied only if a read still holds them.
    fn into_names(self) -> Vec<Name> {
        Arc::try_unwrap(self.0).unwrap_or_else(|shared| (*shared).clone())
    }
    fn ordered_match(&self, index: usize, row: &ResourceSummary) -> bool {
        let (namespace, name) = &self.0[index];
        *namespace == row.namespace && *name == row.name
    }
}
impl FromIterator<Name> for ObservedNames {
    fn from_iter<T: IntoIterator<Item = Name>>(names: T) -> Self {
        Self(Arc::new(names.into_iter().collect()))
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
        for (namespace, name) in names.into_names() {
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

fn same_projection(
    a: &ResourceSummary,
    b: &ResourceSummary,
    owner: &Option<GitOpsOwnerSummary>,
) -> bool {
    a.plural == b.plural
        && a.group == b.group
        && a.owner_ref == b.owner_ref
        && a.argo_app == b.argo_app
        && a.helm_release == b.helm_release
        && a.git_ops_ownership_partial == b.git_ops_ownership_partial
        && owner == &b.git_ops_owner
}

fn same_identity(a: &ResourceSummary, b: &ResourceSummary) -> bool {
    a.name == b.name
        && a.kind == b.kind
        && a.namespace == b.namespace
        && a.cluster == b.cluster
        && a.api_version == b.api_version
        && a.dynamic == b.dynamic
}

/// One context's watch of one resource key. Rows of other contexts are
/// outside it, so its reads never touch them.
#[derive(Clone, Copy)]
struct Watch<'a> {
    context: &'a str,
    key: &'a WatchResourceKey,
}

impl Watch<'_> {
    /// Whether this watch lists `row`.
    fn lists(self, row: &ResourceSummary) -> bool {
        watch_lists(self.context, self.key, row)
    }

    /// The slot hash of the row this watch lists under a name.
    fn slot(self, namespace: Option<&str>, name: &str) -> u64 {
        slot_hash(self.context, &self.key.resource_kind.kind, namespace, name)
    }
}

pub(crate) fn watch_lists(context: &str, key: &WatchResourceKey, row: &ResourceSummary) -> bool {
    row.cluster == context
        && row.kind == key.resource_kind.kind
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

fn slot_hash(cluster: &str, kind: &str, namespace: Option<&str>, name: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    (cluster, kind, namespace, name).hash(&mut hasher);
    hasher.finish()
}

/// A row that a read of noticed names added to the rows it patched. A read
/// without names reloads a kind and records none.
pub(crate) struct Added {
    pub cluster: String,
    pub kind: String,
    pub namespace: Option<String>,
    pub name: String,
}

pub(crate) enum RowsChange {
    /// Rows replaced in place without changing any prepared keys.
    Updated(Vec<usize>),
    Rebuilt,
}

/// Applies a watch read of `key` in `context` to `current`. Rows of other
/// contexts stay as they are. Returns whether a row was added, deleted or
/// changed its keys.
#[cfg(test)]
pub(crate) fn reconcile(
    current: &mut Vec<ResourceSummary>,
    context: &str,
    key: &WatchResourceKey,
    rows: Vec<ResourceSummary>,
    names: Option<ObservedNames>,
    added: &mut Vec<Added>,
) -> bool {
    let names = names.map(NameIndex::from);
    matches!(
        reconcile_indexed(current, Watch { context, key }, rows, names.as_ref(), added),
        RowsChange::Rebuilt
    )
}

fn reconcile_indexed(
    current: &mut Vec<ResourceSummary>,
    watch: Watch<'_>,
    rows: Vec<ResourceSummary>,
    names: Option<&NameIndex>,
    added: &mut Vec<Added>,
) -> RowsChange {
    // `Name` holds no context; `matches` admits only rows of the watch's
    // context, so two contexts' rows never share a name here.
    let matches = |row: &ResourceSummary| {
        watch.lists(row)
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
    let mut updated = Vec::new();
    let mut index = 0;
    current.retain_mut(|row| {
        let position = index;
        index += 1;
        if !matches(row) {
            return true;
        }
        if let Some(new) = next.remove(&(row.namespace.clone(), row.name.clone())) {
            keys_changed |=
                !same_identity(row, &new) || !same_projection(row, &new, &row.git_ops_owner);
            updated.push(position);
            *row = new;
            true
        } else {
            keys_changed = true;
            false
        }
    });
    keys_changed |= !next.is_empty();
    if names.is_some() {
        added.extend(next.keys().map(|(namespace, name)| Added {
            cluster: watch.context.to_owned(),
            kind: watch.key.resource_kind.kind.clone(),
            namespace: namespace.clone(),
            name: name.clone(),
        }));
    }
    current.extend(next.into_values());
    if keys_changed {
        RowsChange::Rebuilt
    } else {
        RowsChange::Updated(updated)
    }
}

impl PreparedRows {
    /// Applies a watch read of `key` in `context`. Rows of other contexts stay
    /// as they are. Returns the read's buffer when the rows were patched in
    /// place, for the next read to reuse off the UI thread.
    #[cfg(test)]
    pub(crate) fn patch(
        &mut self,
        context: &str,
        key: &WatchResourceKey,
        rows: Vec<ResourceSummary>,
        names: Option<ObservedNames>,
    ) -> Vec<ResourceSummary> {
        self.patch_noting(context, key, rows, names, &mut Vec::new())
            .0
    }

    /// Applies a watch read as `patch` does and records in `added` the rows a
    /// read of noticed names added.
    pub(crate) fn patch_noting(
        &mut self,
        context: &str,
        key: &WatchResourceKey,
        rows: Vec<ResourceSummary>,
        names: Option<ObservedNames>,
        added: &mut Vec<Added>,
    ) -> (Vec<ResourceSummary>, RowsChange) {
        let result = self.patch_rows(Watch { context, key }, rows, names, added);
        if matches!(&result.1, RowsChange::Updated(indices) if !indices.is_empty()) {
            self.revision += 1;
        }
        result
    }

    fn patch_rows(
        &mut self,
        watch: Watch<'_>,
        mut rows: Vec<ResourceSummary>,
        names: Option<ObservedNames>,
        added: &mut Vec<Added>,
    ) -> (Vec<ResourceSummary>, RowsChange) {
        if let Some(names) = &names
            && let Some(indices) = self.replace_named(
                watch,
                &mut rows,
                names
                    .iter()
                    .map(|(ns, name)| (ns.as_deref(), name.as_str())),
            )
        {
            return (rows, RowsChange::Updated(indices));
        }
        let kind = self
            .kinds
            .iter()
            .position(|(kind, _)| *kind == watch.key.resource_kind.kind);
        let names = match kind {
            Some(kind) => match self.patch_members(kind, watch, &mut rows, names) {
                Ok(change) => return (rows, change),
                Err(names) => names,
            },
            None => names.map(NameIndex::from),
        };
        let _span = crate::frame_profile::scope("watch", "reconcile");
        self.swap_owners();
        let change = reconcile_indexed(&mut self.rows, watch, rows, names.as_ref(), added);
        match &change {
            RowsChange::Rebuilt => self.rebuild(),
            RowsChange::Updated(_) => self.swap_owners(),
        }
        (Vec::new(), change)
    }

    /// Patches the listed rows of a kind the view holds in the watch's context
    /// in place. `Err` hands the names back, with the rows untouched, when the
    /// read adds or deletes rows of the kind in a way only a reconcile resolves.
    fn patch_members(
        &mut self,
        kind: usize,
        watch: Watch<'_>,
        rows: &mut Vec<ResourceSummary>,
        names: Option<ObservedNames>,
    ) -> Result<RowsChange, Option<NameIndex>> {
        let rank = self
            .contexts
            .iter()
            .position(|context| context == watch.context)
            .and_then(|rank| u32::try_from(rank).ok());
        let (by_kind, context) = (&self.by_kind[kind], &self.context);
        let members = move || kind_members(by_kind, context, rank);
        let span = crate::frame_profile::scope("watch", "ordered_check");
        let count = members().count();
        let ordered_names = names.as_ref().filter(|names| names.0.len() == rows.len());
        let mut complete = names.is_none() || ordered_names.is_some();
        let ordered = rows.len() == count
            && members()
                .zip(rows.iter())
                .enumerate()
                .all(|(position, (index, row))| {
                    complete &=
                        ordered_names.is_none_or(|names| names.ordered_match(position, row));
                    same_identity(&self.rows[index], row) && watch.lists(row)
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
        let positions: Vec<(usize, usize)> = match coverage {
            Coverage::All if ordered => members().zip(0..).collect(),
            Coverage::Named(names) if ordered => members()
                .zip(0..)
                .filter(|&(_, position)| {
                    let row = &rows[position];
                    names.contains(row.namespace.as_deref(), &row.name)
                })
                .collect(),
            Coverage::All => return Err(None),
            Coverage::Named(names) => {
                return Err(Some(names));
            }
        };
        let keys_changed = positions.iter().any(|&(index, position)| {
            !same_projection(
                &self.rows[index],
                &rows[position],
                self.backend_owner(index),
            )
        });
        if keys_changed {
            self.swap_owners();
        }
        if positions.len() == self.rows.len() && positions.len() == rows.len() {
            if !keys_changed {
                self.swap_owners();
            }
            std::mem::swap(&mut self.rows, rows);
            if !keys_changed {
                self.swap_owners();
            }
        } else {
            for &(index, position) in &positions {
                if keys_changed {
                    std::mem::swap(&mut self.rows[index], &mut rows[position]);
                } else {
                    self.replace_row(index, &mut rows[position]);
                }
            }
        }
        if keys_changed {
            self.rebuild();
            Ok(RowsChange::Rebuilt)
        } else {
            Ok(RowsChange::Updated(
                positions.into_iter().map(|(index, _)| index).collect(),
            ))
        }
    }
}

/// The rows in `by_kind` whose context rank is `rank`, in row order.
fn kind_members<'a>(
    by_kind: &'a [usize],
    context: &'a [u32],
    rank: Option<u32>,
) -> impl Iterator<Item = usize> + 'a {
    by_kind
        .iter()
        .copied()
        .filter(move |&index| Some(context[index]) == rank)
}
