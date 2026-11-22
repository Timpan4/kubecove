//! Project prepared resource indices onto the visible page's group headers and rows.

#[cfg(test)]
use super::owner_ref;
use super::{PAGE_SIZE, Visible, identity_hash, ownership_group, type_label};
use crate::table_sort::{self, Sort};
use kubecove_lib::models::ResourceSummary;
use std::collections::{HashMap, HashSet};

pub enum Entry {
    Group {
        key: String,
        label: String,
        count: usize,
        collapsed: bool,
    },
    Type {
        key: String,
        label: String,
        kind: String,
        count: usize,
        collapsed: bool,
    },
    Row(usize),
}

/// What the table shows besides its rows: the sort, the collapsed group keys
/// and the open detail's resource, whose groups stay expanded.
pub struct TableView<'a> {
    pub page: usize,
    pub sort: Sort,
    pub collapsed: &'a HashSet<String>,
    pub selected: Option<&'a ResourceSummary>,
}

/// The collapse key of an ownership group, as `resourceGroupCollapseKey` in `src/features/resources/helpers.ts`.
fn group_key(group: &str) -> String {
    format!("group:{group}")
}

/// The collapse key of a kind group, as `resourceTypeGroupCollapseKey` in `helpers.ts`.
/// Without GitOps grouping one kind group row spans every Helm release and
/// ownership group, so its key uses one empty group for all rows.
fn type_key(group: &str, type_label: &str, grouped_by_gitops: bool) -> String {
    let group = if grouped_by_gitops { group } else { "" };
    format!("group:{group}::type:{type_label}")
}

fn same_resource(left: &ResourceSummary, right: &ResourceSummary) -> bool {
    left.kind == right.kind
        && left.namespace == right.namespace
        && left.name == right.name
        && left.cluster == right.cluster
}

pub struct TablePage<Row = ResourceSummary> {
    pub entries: Vec<Entry>,
    pub rows: Vec<Row>,
    pub total: usize,
    pub page_count: usize,
}

/// Rows already in the table's sort order, grouped by ownership, then kind
/// lane, as `buildResourceTableDisplay` and `buildResourceTableEntries` do.
/// The tests compare the prepared order with this direct form of the web's.
#[cfg(test)]
pub(super) fn grouped_page(
    mut sorted: Vec<&ResourceSummary>,
    view: &TableView<'_>,
    preferred: impl Fn(&ResourceSummary) -> bool,
    lanes: &crate::preferences::KindLanes,
    multi_context: bool,
) -> TablePage {
    let grouped_by_gitops = sorted.iter().any(|row| owner_ref(row).is_some());
    if grouped_by_gitops {
        sorted.sort_by_key(|row| !preferred(row));
    }
    let mut groups: Vec<(String, u8, Vec<&ResourceSummary>)> = Vec::new();
    for row in sorted {
        let (label, rank) = if grouped_by_gitops {
            ownership_group(row, multi_context)
        } else {
            (String::new(), 0)
        };
        match groups.iter_mut().find(|group| group.0 == label) {
            Some(group) => group.2.push(row),
            None => groups.push((label, if preferred(row) { 0 } else { rank + 1 }, vec![row])),
        }
    }
    groups.sort_by_key(|group| group.1);
    let mut display = Vec::new();
    for (_, _, members) in groups {
        let mut kinds: Vec<(&str, Vec<&ResourceSummary>)> = Vec::new();
        for row in members {
            match kinds.iter_mut().find(|kind| kind.0 == row.kind) {
                Some(kind) => kind.1.push(row),
                None => kinds.push((&row.kind, vec![row])),
            }
        }
        kinds.sort_by_key(|kind| {
            if grouped_by_gitops && preferred(kind.1[0]) {
                0
            } else {
                lanes.order(kind.0) + 1
            }
        });
        display.extend(kinds.into_iter().flat_map(|kind| kind.1));
    }
    page_from_order(display, grouped_by_gitops, multi_context, view)
}

#[cfg(test)]
fn page_from_order(
    display: Vec<&ResourceSummary>,
    grouped_by_gitops: bool,
    multi_context: bool,
    view: &TableView<'_>,
) -> TablePage {
    let total = display.len();
    let page_count = total.div_ceil(PAGE_SIZE).max(1);
    let page = view.page.min(page_count - 1);
    let start = page * PAGE_SIZE;
    let page_rows = &display[start..(start + PAGE_SIZE).min(total)];
    let selected = view.selected.and_then(|selected| {
        display
            .iter()
            .copied()
            .find(|row| same_resource(row, selected))
    });
    TablePage {
        entries: page_entries(
            page_rows,
            grouped_by_gitops,
            multi_context,
            selected,
            view.collapsed,
        ),
        rows: page_rows.iter().map(|row| (*row).clone()).collect(),
        total,
        page_count,
    }
}

fn page_entries(
    rows: &[&ResourceSummary],
    grouped_by_gitops: bool,
    multi_context: bool,
    selected: Option<&ResourceSummary>,
    collapsed: &HashSet<String>,
) -> Vec<Entry> {
    // The selected resource's groups stay open even when collapsed, as on the web.
    let selected = selected.map(|row| {
        let group = ownership_group(row, multi_context).0;
        let type_label = type_label(row);
        (
            group_key(&group),
            type_key(&group, &type_label, grouped_by_gitops),
        )
    });
    let labels: Vec<_> = rows
        .iter()
        .map(|row| (ownership_group(row, multi_context).0, type_label(row)))
        .collect();
    let mut group_counts = HashMap::new();
    let mut type_counts = HashMap::new();
    for (group, kind) in &labels {
        *group_counts.entry(group.as_str()).or_insert(0) += 1;
        *type_counts
            .entry((grouped_by_gitops.then_some(group.as_str()), kind.as_str()))
            .or_insert(0) += 1;
    }
    let mut entries = Vec::new();
    let mut previous: Option<(&str, &str)> = None;
    for (index, (row, (group, kind))) in rows.iter().zip(&labels).enumerate() {
        let group_changed = previous.is_none_or(|(g, _)| g != group);
        let type_changed =
            previous.is_none_or(|(g, t)| (grouped_by_gitops && g != group) || t != kind);
        let (row_group_key, row_type_key) =
            (group_key(group), type_key(group, kind, grouped_by_gitops));
        let group_collapsed = grouped_by_gitops
            && selected
                .as_ref()
                .is_none_or(|(key, _)| *key != row_group_key)
            && collapsed.contains(&row_group_key);
        let type_collapsed = selected
            .as_ref()
            .is_none_or(|(_, key)| *key != row_type_key)
            && collapsed.contains(&row_type_key);
        if grouped_by_gitops && group_changed {
            entries.push(Entry::Group {
                key: row_group_key,
                label: group.clone(),
                count: group_counts[group.as_str()],
                collapsed: group_collapsed,
            });
        }
        if type_changed && !group_collapsed {
            entries.push(Entry::Type {
                key: row_type_key,
                label: kind.clone(),
                kind: row.kind.clone(),
                count: type_counts[&(grouped_by_gitops.then_some(group.as_str()), kind.as_str())],
                collapsed: type_collapsed,
            });
        }
        if !group_collapsed && !type_collapsed {
            entries.push(Entry::Row(index));
        }
        previous = Some((group, kind));
    }
    entries
}

/// The table page in display order: the prepared order for the default sort,
/// and a sort of the visible rows' packed keys for any other sort.
pub fn indexed_table_page(
    visible: &Visible,
    view: &TableView<'_>,
    usage: impl Fn(&ResourceSummary) -> table_sort::Usage,
) -> TablePage {
    let alternate;
    let order = if view.sort == Sort::default() {
        visible.order()
    } else {
        alternate = visible.sorted_order(view.sort, usage);
        &alternate
    };
    let page = project_table_page(visible, view, order);
    TablePage {
        entries: page.entries,
        rows: page
            .rows
            .into_iter()
            .map(|index| visible.all_rows()[index].clone())
            .collect(),
        total: page.total,
        page_count: page.page_count,
    }
}

pub(crate) fn project_table_page(
    visible: &Visible,
    view: &TableView<'_>,
    order: &[usize],
) -> TablePage<usize> {
    let _profile = crate::frame_profile::scope("table", "project");
    let total = order.len();
    let page_count = total.div_ceil(PAGE_SIZE).max(1);
    let start = view.page.min(page_count - 1) * PAGE_SIZE;
    let indices = &order[start..(start + PAGE_SIZE).min(total)];
    let rows = visible.all_rows();
    let projected: Vec<_> = indices.iter().map(|&index| &rows[index]).collect();
    let selected = view
        .selected
        .filter(|_| !view.collapsed.is_empty())
        .and_then(|selected| {
            let hash = identity_hash(selected);
            let identities = visible.source().row_identities();
            order.iter().find_map(|&index| {
                (identities[index] == hash && same_resource(&rows[index], selected))
                    .then_some(&rows[index])
            })
        });
    TablePage {
        entries: page_entries(
            &projected,
            visible.grouped_by_gitops,
            visible.source().multi_context(),
            selected,
            view.collapsed,
        ),
        rows: indices.to_vec(),
        total,
        page_count,
    }
}

/// The reference display order of `rows` from `contexts`, the context list
/// their prepared rows were built with.
#[cfg(test)]
pub(super) fn display_order(rows: &[ResourceSummary], contexts: &[String]) -> (Vec<usize>, bool) {
    let grouped = rows.iter().any(|row| owner_ref(row).is_some());
    let collapsed = HashSet::new();
    let mut indices = Vec::new();
    for page in 0..rows.len().div_ceil(PAGE_SIZE) {
        let view = TableView {
            page,
            sort: Sort::default(),
            collapsed: &collapsed,
            selected: None,
        };
        let refs: Vec<&ResourceSummary> = rows.iter().collect();
        let order =
            table_sort::reference::order(&refs, contexts, view.sort, |_| Default::default());
        let output = grouped_page(
            order.into_iter().map(|index| refs[index]).collect(),
            &view,
            |_| false,
            &crate::preferences::KindLanes::default(),
            contexts.len() > 1,
        );
        indices.extend(output.rows.iter().map(|row| {
            rows.iter()
                .position(|candidate| same_resource(candidate, row))
                .unwrap()
        }));
    }
    (indices, grouped)
}
