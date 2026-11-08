//! Immutable resource rows and prepared search/group keys. Filter changes only
//! select and order indices; resource strings stay with their source snapshot.

use super::{HealthSummary, gitops_label, ownership_group};
mod patch;
use crate::collate::Key;
use kubecove_lib::models::{GitOpsOwnerSummary, ResourceSummary};
pub(crate) use patch::{ObservedNames, reconcile};
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
    pub(crate) search: String,
    pub(crate) owner: Option<(String, String)>,
    group: usize,
    lane: usize,
    kind: usize,
    pub(crate) variant: usize,
    name_rank: usize,
    gitops: bool,
}

#[derive(Default)]
pub(crate) struct PreparedRows {
    rows: Vec<ResourceSummary>,
    identity: Rc<()>,
    keys: Vec<RowKey>,
    groups: Vec<u8>,
    kinds: Vec<(String, usize)>,
    by_kind: Vec<Vec<usize>>,
    variants: Vec<usize>,
    lanes: Vec<Lane>,
    by_name: Vec<usize>,
    /// Set by `with_inherited_owners`: each row whose GitOps owner it inherited,
    /// with the owner the backend sent. `swap_owners` exchanges the two, so a
    /// watch patch compares and replaces backend rows.
    inherited: Vec<(usize, Option<GitOpsOwnerSummary>)>,
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

    pub(crate) fn identity(&self) -> &Rc<()> {
        &self.identity
    }

    /// The resource view's rows, with GitOps owners inherited in place as the
    /// filter, table and detail pane show them.
    pub(crate) fn with_inherited_owners(mut rows: Vec<ResourceSummary>) -> Self {
        let inherited = {
            let _profile = crate::frame_profile::scope("rows", "inherit_owners");
            super::inherit_gitops_owners(&mut rows)
        };
        Self {
            inherited,
            ..Self::new(rows)
        }
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
        *self = Self::with_inherited_owners(std::mem::take(&mut self.rows));
    }

    pub(crate) fn new(rows: Vec<ResourceSummary>) -> Self {
        let _span = crate::frame_profile::scope("rows", "prepare");
        let mut sorted: Vec<_> = (0..rows.len()).collect();
        sorted.sort_by_cached_key(|&row| Key::new(&rows[row].name, true));
        let mut name_rank = vec![0; rows.len()];
        for (rank, &row) in sorted.iter().enumerate() {
            name_rank[row] = rank;
        }
        let mut groups = Vec::new();
        let mut group_ids = HashMap::new();
        let mut kinds: Vec<(String, usize)> = Vec::new();
        let mut by_kind: Vec<Vec<usize>> = Vec::new();
        let mut lane_ids = HashMap::new();
        let mut lanes = Vec::new();
        let mut variants = Vec::new();
        let mut variant_ids = HashMap::new();
        let keys = rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let (label, rank) = ownership_group(row);
                let next_group = groups.len();
                let group = *group_ids.entry(label.clone()).or_insert_with(|| {
                    groups.push(rank);
                    next_group
                });
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
                RowKey {
                    search: crate::resource_search::haystack(row),
                    owner: super::gitops_owner_filter(row),
                    group,
                    lane,
                    kind,
                    variant,
                    name_rank: name_rank[index],
                    gitops: gitops_label(row).is_some(),
                }
            })
            .collect();
        drop(variant_ids);
        Self {
            rows,
            identity: Rc::default(),
            keys,
            groups,
            kinds,
            by_kind,
            variants,
            lanes,
            by_name: sorted,
            inherited: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
struct OrderKey {
    group_rank: u8,
    group_first: usize,
    lane_rank: usize,
    lane_first: usize,
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
struct First {
    ordinary: bool,
    name_rank: usize,
}

impl First {
    const UNSEEN: Self = Self {
        ordinary: true,
        name_rank: usize::MAX,
    };
}

#[derive(Default)]
pub struct Visible {
    // The key identity survives releasing the row owner for watch reconciliation.
    identity: Rc<()>,
    source: Rc<PreparedRows>,
    indices: Vec<usize>,
    previous: Vec<usize>,
    order: Vec<usize>,
    ordered_lanes: Vec<(OrderKey, usize)>,
    lane_rows: Vec<Vec<usize>>,
    preferred: Vec<bool>,
    lane_order: Vec<usize>,
    allowed: Vec<bool>,
    selected: Vec<bool>,
    group_first: Vec<First>,
    lane_first: Vec<First>,
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
        std::mem::swap(&mut self.previous, &mut self.indices);
        self.indices.clear();
        self.selected.resize(source.rows.len(), false);
        self.selected.fill(false);
        for (index, key) in source.keys.iter().enumerate() {
            if self.allowed[key.variant]
                && included(
                    &source.rows[index],
                    key.owner.as_ref().map(|(key, _)| key.as_str()),
                )
                && (query.is_empty() || key.search.contains(query))
            {
                self.indices.push(index);
                self.selected[index] = true;
            }
        }
        let changed =
            !Rc::ptr_eq(&self.identity, source.identity()) || self.indices != self.previous;
        self.identity = source.identity().clone();
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
            .any(|&index| self.source.keys[index].gitops);
        self.group_first
            .resize(self.source.groups.len(), First::UNSEEN);
        self.group_first.fill(First::UNSEEN);
        self.lane_first.resize(
            self.source.lanes.len().max(self.source.kinds.len()),
            First::UNSEEN,
        );
        self.lane_first.fill(First::UNSEEN);
        for &index in &self.indices {
            let key = &self.source.keys[index];
            let (group, lane) = if self.grouped_by_gitops {
                (key.group, key.lane)
            } else {
                (0, key.kind)
            };
            let first = First {
                ordinary: !self.grouped_by_gitops || !self.preferred[index],
                name_rank: key.name_rank,
            };
            self.group_first[group] = self.group_first[group].min(first);
            self.lane_first[lane] = self.lane_first[lane].min(first);
        }
        self.ordered_lanes.clear();
        let lane_count = if self.grouped_by_gitops {
            self.source.lanes.len()
        } else {
            self.source.kinds.len()
        };
        for lane in 0..lane_count {
            if self.lane_first[lane] == First::UNSEEN {
                continue;
            }
            let (group, kind) = if self.grouped_by_gitops {
                let lane = &self.source.lanes[lane];
                (lane.group, lane.kind)
            } else {
                (0, lane)
            };
            self.ordered_lanes.push((
                OrderKey {
                    group_rank: if !self.grouped_by_gitops || !self.group_first[group].ordinary {
                        0
                    } else {
                        self.source.groups[group] + 1
                    },
                    group_first: self.group_first[group].name_rank,
                    lane_rank: if self.grouped_by_gitops && !self.lane_first[lane].ordinary {
                        0
                    } else {
                        self.lane_order[kind] + 1
                    },
                    lane_first: self.lane_first[lane].name_rank,
                },
                lane,
            ));
        }
        self.ordered_lanes.sort_unstable_by_key(|&(key, _)| key);
        self.lane_rows.resize_with(lane_count, Vec::new);
        for rows in &mut self.lane_rows {
            rows.clear();
        }
        // Rows within a lane share its sort prefix. Name order is immutable;
        // only the preferred partition and the order of lanes can change.
        for &index in &self.source.by_name {
            if self.selected[index] {
                let key = &self.source.keys[index];
                let lane = if self.grouped_by_gitops {
                    key.lane
                } else {
                    key.kind
                };
                self.lane_rows[lane].push(index);
            }
        }
        self.order.clear();
        let preferred = &self.preferred;
        for &(_, lane) in &self.ordered_lanes {
            let rows = &self.lane_rows[lane];
            if self.grouped_by_gitops {
                self.order
                    .extend(rows.iter().copied().filter(|&index| preferred[index]));
                self.order
                    .extend(rows.iter().copied().filter(|&index| !preferred[index]));
            } else {
                self.order.extend_from_slice(rows);
            }
        }
        drop(order_span);
        self.health = HealthSummary::of(self.iter());
    }
}

impl FromIterator<ResourceSummary> for Visible {
    fn from_iter<I: IntoIterator<Item = ResourceSummary>>(rows: I) -> Self {
        let source = Rc::new(PreparedRows::new(rows.into_iter().collect()));
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
