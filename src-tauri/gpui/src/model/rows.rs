//! Immutable resource rows and prepared search/group keys. Filter changes only
//! select and order indices; resource strings stay with their source snapshot.

use super::{HealthSummary, OwnerRef};
mod free;
mod order;
mod patch;
mod search;
mod sort;
use crate::collate::Key;
use kubecove_lib::models::{GitOpsOwnerSummary, ResourceSummary};
pub(crate) use patch::{ObservedNames, reconcile};
pub(crate) use search::SearchMatches;
use std::{
    collections::HashMap,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(test)]
mod patch_tests;
#[cfg(test)]
mod tests;

pub(crate) struct KindFilter<'a, F> {
    pub kinds: &'a [String],
    pub selected: F,
}

pub(crate) struct RowKey {
    /// The row's GitOps owner in `PreparedRows::owners`.
    owner: Option<u32>,
    group: usize,
    lane: usize,
    kind: usize,
    pub(crate) variant: usize,
}

/// A distinct GitOps owner of the prepared rows.
struct Owner {
    /// The owner filter's `filterKey`.
    key: String,
    label: String,
    group: usize,
}

/// Which prepared rows a derived view was built from. Each preparation takes a
/// new one; patching rows in place keeps it.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct RowsId(u64);

static ROWS_ID: AtomicU64 = AtomicU64::new(1);

impl Default for RowsId {
    fn default() -> Self {
        Self(ROWS_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Default)]
pub(crate) struct PreparedRows {
    rows: Vec<ResourceSummary>,
    /// The contexts in rank order: the caller's list, which keeps a context's
    /// rank while it has no rows, then any other row cluster in first-seen order.
    contexts: Vec<String>,
    /// Each row's index in `contexts`, which is its context rank.
    context: Vec<u32>,
    identity: RowsId,
    keys: Vec<RowKey>,
    search: search::SearchText,
    groups: Vec<u8>,
    kinds: Vec<(String, usize)>,
    by_kind: Vec<Vec<usize>>,
    variants: Vec<usize>,
    lanes: Vec<Lane>,
    by_name: Vec<usize>,
    sort: sort::SortColumns,
    owners: Vec<Owner>,
    /// Set by `with_inherited_owners`: each row whose GitOps owner it inherited,
    /// with the owner the backend sent. `swap_owners` exchanges the two, so a
    /// watch patch compares and replaces backend rows.
    inherited: Vec<(usize, Option<GitOpsOwnerSummary>)>,
    /// Row indices sorted by a hash of kind, namespace and name, built by the
    /// first named watch patch. Patches in place keep every row's index.
    slots: Option<Vec<(u64, u32)>>,
}

struct Lane {
    group: usize,
    kind: usize,
}

impl std::ops::Deref for PreparedRows {
    type Target = [ResourceSummary];
    fn deref(&self) -> &Self::Target {
        &self.rows
    }
}

impl PreparedRows {
    pub(crate) fn fill_kind_mask(
        &self,
        kinds: &KindFilter<'_, impl Fn(&str, Option<&str>) -> bool>,
        allowed: &mut Vec<bool>,
    ) {
        allowed.clear();
        allowed.extend(self.variants.iter().map(|&index| {
            let row = &self.rows[index];
            kinds.kinds.contains(&row.kind)
                && (kinds.selected)(&row.kind, row.api_version.as_deref())
        }));
    }

    pub(crate) fn keyed_rows(&self) -> impl Iterator<Item = (&ResourceSummary, &RowKey)> {
        self.rows.iter().zip(&self.keys)
    }

    /// The owner filter's `filterKey` and label of a row's GitOps owner.
    pub(crate) fn owner(&self, key: &RowKey) -> Option<(&str, &str)> {
        let owner = &self.owners[key.owner? as usize];
        Some((&owner.key, &owner.label))
    }

    pub(crate) fn identity(&self) -> RowsId {
        self.identity
    }

    pub(crate) fn contexts(&self) -> &[String] {
        &self.contexts
    }

    /// Whether the view lists several contexts, which adds the context to
    /// owner and Helm group labels, owner filter keys and search.
    pub(crate) fn multi_context(&self) -> bool {
        self.contexts.len() > 1
    }

    /// The resource view's rows, with GitOps owners inherited in place as the
    /// filter, table and detail pane show them.
    pub(crate) fn with_inherited_owners(
        mut rows: Vec<ResourceSummary>,
        contexts: &[String],
    ) -> Self {
        let inherited = {
            let _profile = crate::frame_profile::scope("rows", "inherit_owners");
            super::inherit_gitops_owners(&mut rows)
        };
        let mut prepared = Self::new(rows, contexts);
        prepared.inherited = inherited;
        prepared
    }

    /// The rows with the GitOps owners the backend sent.
    pub(crate) fn backend_rows(&self) -> Vec<ResourceSummary> {
        let mut rows = self.rows.clone();
        for (index, owner) in &self.inherited {
            rows[*index].git_ops_owner = owner.clone();
        }
        rows
    }

    fn swap_owners(&mut self) {
        for (index, owner) in &mut self.inherited {
            std::mem::swap(&mut self.rows[*index].git_ops_owner, owner);
        }
    }

    /// Prepares patched rows again. Only the resource view's rows are patched.
    fn rebuild(&mut self) {
        let contexts = std::mem::take(&mut self.contexts);
        *self = Self::with_inherited_owners(std::mem::take(&mut self.rows), &contexts);
    }

    /// Prepares the rows of a view whose contexts, in rank order, are `contexts`.
    pub(crate) fn new(rows: Vec<ResourceSummary>, contexts: &[String]) -> Self {
        let _span = crate::frame_profile::scope("rows", "prepare");
        let (contexts, context) = context_ranks(&rows, contexts);
        let multi_context = contexts.len() > 1;
        let names: Vec<Key<'_>> = rows.iter().map(|row| Key::new(&row.name, true)).collect();
        let mut sorted: Vec<_> = (0..rows.len()).collect();
        sorted.sort_by(|&a, &b| {
            names[a]
                .cmp(&names[b])
                .then_with(|| context[a].cmp(&context[b]))
        });
        let mut groups = Vec::new();
        // Group labels, owner strings and owner groups are built once per
        // distinct owner or Helm release, not once per row.
        let mut group_ids: HashMap<String, usize> = HashMap::new();
        let mut group = |label: String, rank: u8| {
            let next = groups.len();
            *group_ids.entry(label).or_insert_with(|| {
                groups.push(rank);
                next
            })
        };
        // Owners and Helm releases are distinct per context.
        let mut owner_ids: HashMap<(u32, OwnerRef<'_>), u32> = HashMap::new();
        let mut owners: Vec<Owner> = Vec::new();
        let mut helm_groups: HashMap<(u32, &str), usize> = HashMap::new();
        let mut kinds: Vec<(String, usize)> = Vec::new();
        let mut by_kind: Vec<Vec<usize>> = Vec::new();
        let mut lane_ids = HashMap::new();
        let mut lanes = Vec::new();
        let mut variants = Vec::new();
        let mut variant_ids = HashMap::new();
        let mut search = search::SearchText::with_capacity(rows.len());
        let keys = rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let row_context = context[index];
                let owner_ref = super::owner_ref(row);
                let owner = owner_ref.map(|owner_ref| {
                    *owner_ids
                        .entry((row_context, owner_ref))
                        .or_insert_with(|| {
                            let label =
                                super::in_context(owner_ref.label(), &row.cluster, multi_context);
                            owners.push(Owner {
                                key: owner_ref.filter_key(&row.cluster, multi_context),
                                group: group(label.clone(), 0),
                                label,
                            });
                            u32::try_from(owners.len() - 1).expect("owner count fits u32")
                        })
                });
                let group = match (owner, row.helm_release.as_deref()) {
                    (Some(owner), _) => owners[owner as usize].group,
                    (None, Some(release)) => *helm_groups
                        .entry((row_context, release))
                        .or_insert_with(|| {
                            let label = super::helm_group(release);
                            group(super::in_context(label, &row.cluster, multi_context), 1)
                        }),
                    (None, None) => group(super::no_owner_group(row).to_owned(), 2),
                };
                let kind = kinds
                    .iter()
                    .position(|(kind, _)| *kind == row.kind)
                    .unwrap_or_else(|| {
                        by_kind.push(Vec::new());
                        kinds.push((
                            row.kind.clone(),
                            crate::preferences::KindLanes::default().order(&row.kind),
                        ));
                        kinds.len() - 1
                    });
                by_kind[kind].push(index);
                let variant = *variant_ids
                    .entry((row.kind.as_str(), row.api_version.as_deref()))
                    .or_insert_with(|| {
                        let id = variants.len();
                        variants.push(index);
                        id
                    });
                let next_lane = lanes.len();
                let lane = *lane_ids.entry((group, kind)).or_insert_with(|| {
                    lanes.push(Lane { group, kind });
                    next_lane
                });
                search.push(
                    row,
                    owner_ref,
                    multi_context.then_some(row.cluster.as_str()),
                );
                RowKey {
                    owner,
                    group,
                    lane,
                    kind,
                    variant,
                }
            })
            .collect();
        drop(variant_ids);
        drop(owner_ids);
        let sort = sort::SortColumns::new(&rows, &names, &sorted, &kinds, &context, &contexts);
        drop(names);
        Self {
            rows,
            contexts,
            context,
            identity: RowsId::default(),
            keys,
            search,
            groups,
            kinds,
            by_kind,
            variants,
            lanes,
            by_name: sorted,
            sort,
            owners,
            inherited: Vec::new(),
            slots: None,
        }
    }
}

/// The contexts in rank order and each row's rank. `listed` comes first, so
/// a listed context keeps its rank while it has no rows. A row of a context
/// outside `listed` appends that context, in first-seen order, so every row
/// has a rank.
fn context_ranks(rows: &[ResourceSummary], listed: &[String]) -> (Vec<String>, Vec<u32>) {
    let rank = |count: usize| u32::try_from(count).expect("context count fits u32");
    let mut contexts: Vec<String> = Vec::with_capacity(listed.len());
    let mut ids: HashMap<&str, u32> = HashMap::with_capacity(listed.len());
    for context in listed {
        if let std::collections::hash_map::Entry::Vacant(entry) = ids.entry(context.as_str()) {
            entry.insert(rank(contexts.len()));
            contexts.push(context.clone());
        }
    }
    let ranks = rows
        .iter()
        .map(|row| {
            let next = rank(contexts.len());
            *ids.entry(row.cluster.as_str()).or_insert_with(|| {
                contexts.push(row.cluster.clone());
                next
            })
        })
        .collect();
    (contexts, ranks)
}

#[derive(Default)]
pub struct Visible {
    // The key identity survives releasing the row owner for watch reconciliation.
    identity: RowsId,
    source: Rc<PreparedRows>,
    indices: Vec<usize>,
    previous: Vec<usize>,
    /// Display order under the default sort.
    order: Vec<usize>,
    /// Visible rows in name order, the default sort's sequence.
    by_name: Vec<usize>,
    grouping: order::Grouping,
    preferred: Vec<bool>,
    lane_order: Vec<usize>,
    allowed: Vec<bool>,
    selected: Vec<bool>,
    matches: SearchMatches,
    pub(super) grouped_by_gitops: bool,
    generation: u64,
    health: HealthSummary,
}

static GENERATION: AtomicU64 = AtomicU64::new(1);

impl Visible {
    pub(crate) fn release_source(&mut self) {
        self.source = Rc::default();
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn len(&self) -> usize {
        self.indices.len()
    }
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }
    pub fn iter(&self) -> Rows<'_> {
        Rows {
            rows: &self.source.rows,
            indices: self.indices.iter(),
        }
    }
    pub fn health(&self) -> HealthSummary {
        self.health
    }
    pub(crate) fn source(&self) -> &Rc<PreparedRows> {
        &self.source
    }
    pub(crate) fn all_rows(&self) -> &[ResourceSummary] {
        &self.source.rows
    }
    pub(crate) fn contains_index(&self, index: usize) -> bool {
        self.selected[index]
    }
    /// The source rows the last refilter's query matched, before the other filters.
    pub(crate) fn search_matches(&self) -> &SearchMatches {
        &self.matches
    }
    pub(super) fn ordered(&self) -> impl Iterator<Item = &ResourceSummary> {
        self.order.iter().map(|&index| &self.source.rows[index])
    }

    #[cfg(test)]
    pub(crate) fn refilter(&mut self, source: Rc<PreparedRows>, kinds: &[String], query: &str) {
        self.refilter_with(
            source,
            KindFilter {
                kinds,
                selected: |_: &str, _: Option<&str>| true,
            },
            query,
            |_, _| true,
            |_| false,
            &crate::preferences::KindLanes::default(),
        );
    }

    pub(crate) fn refilter_with(
        &mut self,
        source: Rc<PreparedRows>,
        kinds: KindFilter<'_, impl Fn(&str, Option<&str>) -> bool>,
        query: &str,
        included: impl Fn(&ResourceSummary, Option<&str>) -> bool,
        preferred: impl Fn(&ResourceSummary) -> bool,
        lanes: &crate::preferences::KindLanes,
    ) {
        let _span = crate::frame_profile::scope("rows", "refilter");
        source.fill_kind_mask(&kinds, &mut self.allowed);
        source.search.matches(query, &mut self.matches);
        std::mem::swap(&mut self.previous, &mut self.indices);
        self.indices.clear();
        self.selected.resize(source.rows.len(), false);
        self.selected.fill(false);
        for (index, key) in source.keys.iter().enumerate() {
            if self.allowed[key.variant]
                && included(&source.rows[index], source.owner(key).map(|(key, _)| key))
                && self.matches.contains(index)
            {
                self.indices.push(index);
                self.selected[index] = true;
            }
        }
        let changed = self.identity != source.identity() || self.indices != self.previous;
        self.identity = source.identity();
        self.source = source;
        if changed {
            self.generation = GENERATION.fetch_add(1, Ordering::Relaxed);
        }
        let order_span = crate::frame_profile::scope("rows", "order");
        let mut same_order = !changed && self.lane_order.len() == self.source.kinds.len();
        self.lane_order.resize(self.source.kinds.len(), 0);
        for (index, (kind, _)) in self.source.kinds.iter().enumerate() {
            let order = lanes.order(kind);
            same_order &= self.lane_order[index] == order;
            self.lane_order[index] = order;
        }
        self.preferred.resize(self.source.rows.len(), false);
        for &index in &self.indices {
            let value = preferred(&self.source.rows[index]);
            same_order &= self.preferred[index] == value;
            self.preferred[index] = value;
        }
        if same_order {
            self.health = HealthSummary::of(self.iter());
            return;
        }
        self.grouped_by_gitops = self
            .indices
            .iter()
            .any(|&index| self.source.keys[index].owner.is_some());
        // Name order is immutable; only the preferred partition and the order
        // of lanes can change.
        self.by_name.clear();
        let selected = &self.selected;
        self.by_name.extend(
            self.source
                .by_name
                .iter()
                .copied()
                .filter(|&index| selected[index]),
        );
        self.grouping.order(
            &order::Lanes {
                source: &self.source,
                grouped: self.grouped_by_gitops,
                preferred: &self.preferred,
                lane_order: &self.lane_order,
            },
            &self.by_name,
            &mut self.order,
        );
        drop(order_span);
        self.health = HealthSummary::of(self.iter());
    }
}

impl FromIterator<ResourceSummary> for Visible {
    fn from_iter<I: IntoIterator<Item = ResourceSummary>>(rows: I) -> Self {
        let source = Rc::new(PreparedRows::new(rows.into_iter().collect(), &[]));
        let kinds: Vec<_> = source.kinds.iter().map(|(kind, _)| kind.clone()).collect();
        let mut visible = Self::default();
        visible.refilter_with(
            source,
            KindFilter {
                kinds: &kinds,
                selected: |_: &str, _: Option<&str>| true,
            },
            "",
            |_, _| true,
            |_| false,
            &crate::preferences::KindLanes::default(),
        );
        visible
    }
}

pub struct Rows<'a> {
    rows: &'a [ResourceSummary],
    indices: std::slice::Iter<'a, usize>,
}

impl<'a> Iterator for Rows<'a> {
    type Item = &'a ResourceSummary;
    fn next(&mut self) -> Option<Self::Item> {
        self.indices.next().map(|&index| &self.rows[index])
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.indices.size_hint()
    }
}

impl ExactSizeIterator for Rows<'_> {}

impl<'a> IntoIterator for &'a Visible {
    type Item = &'a ResourceSummary;
    type IntoIter = Rows<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
