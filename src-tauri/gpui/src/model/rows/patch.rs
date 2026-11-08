//! Reconcile watch reads while preserving keys whose inputs did not change.
use super::PreparedRows;
use kubecove_lib::models::{ResourceSummary, WatchResourceKey};
use std::collections::{HashMap, HashSet};
pub(crate) type Name = (Option<String>, String);

#[derive(Default)]
pub(crate) struct ObservedNames(Vec<Name>);
impl ObservedNames {
    pub(crate) fn insert(&mut self, name: Name) {
        self.0.push(name);
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

fn same_keys(a: &ResourceSummary, b: &ResourceSummary) -> bool {
    a.name == b.name
        && a.kind == b.kind
        && a.namespace == b.namespace
        && a.cluster == b.cluster
        && a.api_version == b.api_version
        && a.dynamic == b.dynamic
        && same_projection(a, b)
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
impl PreparedRows {
    pub(crate) fn patch(
        &mut self,
        key: &WatchResourceKey,
        mut rows: Vec<ResourceSummary>,
        names: Option<ObservedNames>,
    ) -> Vec<ResourceSummary> {
        // Watch rows carry the backend's owners, so compare them with those. If no
        // key changes, the rows keep their positions and the same inherited owners.
        self.swap_owners();
        let names =
            if let Some(kind) = self
                .kinds
                .iter()
                .position(|(kind, _)| *kind == key.resource_kind.kind)
            {
                let members = &self.by_kind[kind];
                let span = crate::frame_profile::scope("watch", "ordered_check");
                let mut keys_changed = false;
                let ordered_names = names.as_ref().filter(|names| names.0.len() == rows.len());
                let mut complete = names.is_none() || ordered_names.is_some();
                let ordered =
                    rows.len() == members.len()
                        && members.iter().zip(&rows).enumerate().all(
                            |(position, (&index, row))| {
                                let old = &self.rows[index];
                                let matches =
                                    old.cluster == row.cluster
                                        && old.kind == row.kind
                                        && old.name == row.name
                                        && old.namespace == row.namespace
                                        && old.api_version == row.api_version
                                        && old.dynamic == row.dynamic
                                        && (key.resource_kind.api_version.is_some()
                                            || row.dynamic != Some(true))
                                        && key.resource_kind.api_version.as_ref().is_none_or(
                                            |api| row.api_version.as_ref() == Some(api),
                                        )
                                        && key
                                            .namespace
                                            .as_ref()
                                            .is_none_or(|ns| row.namespace.as_ref() == Some(ns));
                                complete &= ordered_names
                                    .is_none_or(|names| names.ordered_match(position, row));
                                keys_changed |= matches && !same_projection(old, row);
                                matches
                            },
                        );
                let coverage = if ordered && complete {
                    Coverage::All
                } else if ordered {
                    Coverage::of(names, &rows)
                } else {
                    names.map_or(Coverage::All, |names| {
                        Coverage::Named(NameIndex::from(names))
                    })
                };
                drop(span);
                match coverage {
                    Coverage::All if ordered => {
                        let _span = crate::frame_profile::scope("watch", "replace_rows");
                        if members.len() == self.rows.len() {
                            std::mem::swap(&mut self.rows, &mut rows);
                        } else {
                            for (&index, row) in members.iter().zip(&mut rows) {
                                std::mem::swap(&mut self.rows[index], row);
                            }
                        }
                        if keys_changed {
                            self.rebuild();
                        } else {
                            self.swap_owners();
                        }
                        return rows;
                    }
                    Coverage::All => None,
                    Coverage::Named(names) => Some(names),
                }
            } else {
                names.map(NameIndex::from)
            };
        let _span = crate::frame_profile::scope("watch", "reconcile");
        if reconcile_indexed(&mut self.rows, key, rows, names.as_ref()) {
            self.rebuild();
        } else {
            self.swap_owners();
        }
        Vec::new()
    }
}
