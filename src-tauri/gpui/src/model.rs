//! Presentation model shared by the overview and resource table. Each rule
//! cites the Svelte module it mirrors so the two clients stay comparable.

mod rows;
use crate::table_sort::{self, Sort};
use crate::theme::Tokens;
use gpui_kit::Hsla;
use kubecove_lib::models::{
    GitOpsOwnerSummary, HealthAssessmentSource, HealthAssessmentState, ResourceSummary,
};
pub use rows::Visible;
pub(crate) use rows::{KindFilter, ObservedNames, PreparedRows, RowsId, reconcile};
use std::collections::{HashMap, HashSet};

/// `DEFAULT_WORKSPACE_KINDS` in `src/lib/workspace-model.ts`.
pub const WORKSPACE_KINDS: &[&str] = &[
    "Pod",
    "Deployment",
    "ReplicaSet",
    "Job",
    "CronJob",
    "Service",
    "Ingress",
    "ConfigMap",
    "Secret",
];

pub const PAGE_SIZE: usize = 50;

/// `buildResourceHealthSummary` in `src/features/resources/helpers.ts`.
#[derive(Default, Clone, Copy)]
pub struct HealthSummary {
    pub total: usize,
    pub healthy: usize,
    pub attention: usize,
    pub degraded: usize,
    pub unknown: usize,
    pub not_evaluated: usize,
    pub restart_evidence: usize,
}

impl HealthSummary {
    pub fn of<'a>(rows: impl IntoIterator<Item = &'a ResourceSummary>) -> Self {
        let mut summary = Self::default();
        for row in rows {
            summary.add(row);
        }
        summary
    }

    pub fn add(&mut self, row: &ResourceSummary) {
        self.total += 1;
        match row.health_assessment.state {
            HealthAssessmentState::Healthy => self.healthy += 1,
            HealthAssessmentState::NeedsAttention => self.attention += 1,
            HealthAssessmentState::Degraded => self.degraded += 1,
            HealthAssessmentState::NotEvaluated => self.not_evaluated += 1,
            HealthAssessmentState::Unknown => self.unknown += 1,
        }
        if row.restarts.unwrap_or(0) > 0 {
            self.restart_evidence += 1;
        }
    }
}

/// `gitOpsOwnershipGroup` in `src/lib/gitops-ownership-evidence.ts`.
pub fn ownership_group(row: &ResourceSummary) -> (String, u8) {
    if let Some(owner) = owner_ref(row) {
        return (owner.label(), 0);
    }
    if let Some(release) = &row.helm_release {
        return (helm_group(release), 1);
    }
    (no_owner_group(row).into(), 2)
}

fn helm_group(release: &str) -> String {
    format!("Helm release: {release}")
}

fn no_owner_group(row: &ResourceSummary) -> &'static str {
    if row.git_ops_ownership_partial {
        "No GitOps ownership evidence (partial coverage)"
    } else {
        "No GitOps ownership evidence"
    }
}

/// A GitOps owner that a provider adapter recognizes, borrowed from its row.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct OwnerRef<'a> {
    pub provider: &'a str,
    pub kind: &'a str,
    /// Trimmed.
    pub name: &'a str,
    pub namespace: Option<&'a str>,
    pub confidence: &'a str,
    pub provenance: &'a str,
    pub partial: bool,
}

/// The row's GitOps owner, as `gitOpsOwnership` in
/// `src/lib/gitops-ownership-evidence.ts` returns it. An Argo CD name
/// annotation stands in when the backend reports no owner, and a row with no
/// recognized owner has none.
pub fn owner_ref(row: &ResourceSummary) -> Option<OwnerRef<'_>> {
    // Rust owners always state `partial`; an annotation-only Argo app is partial.
    let owner = match row.git_ops_owner.as_ref() {
        Some(owner) => OwnerRef {
            provider: &owner.provider,
            kind: &owner.kind,
            name: owner.name.trim(),
            namespace: owner.namespace.as_deref(),
            confidence: &owner.confidence,
            provenance: &owner.provenance,
            partial: owner.partial,
        },
        None => OwnerRef {
            provider: "argo",
            kind: "Application",
            name: row.argo_app.as_deref()?.trim(),
            namespace: None,
            confidence: "metadata",
            provenance: "argocd-name-annotation",
            partial: true,
        },
    };
    let recognized = matches!(
        (owner.provider, owner.kind),
        ("argo", "Application") | ("flux", "Kustomization" | "HelmRelease")
    );
    (recognized && !owner.name.is_empty()).then_some(owner)
}

impl OwnerRef<'_> {
    /// `ownerName` in `gitOpsOwnership`: the Flux owner is qualified by its namespace.
    pub fn name(self) -> String {
        match (self.provider, self.namespace) {
            ("flux", Some(namespace)) => format!("{namespace}/{}", self.name),
            _ => self.name.to_owned(),
        }
    }

    /// The owner's group label.
    pub fn label(self) -> String {
        let name = self.name();
        let label = if self.provider == "argo" {
            format!("Owned by Argo CD: {name}")
        } else {
            format!("Owned by Flux {}: {name}", self.kind)
        };
        if self.partial {
            format!("{label} (partial evidence)")
        } else {
            label
        }
    }

    /// The owner filter's `filterKey`: `provider:kind:namespace:name`.
    pub fn filter_key(self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.provider,
            self.kind,
            self.namespace.unwrap_or_default(),
            self.name
        )
    }
}

/// `inheritGitOpsOwnership` in `src/lib/gitops-ownership-evidence.ts`.
///
/// A row without direct GitOps evidence takes the `gitOpsOwner` of its nearest
/// `ownerRef` ancestor in the same namespace that has evidence. Parents match by
/// namespace and name only, and the last row with a name wins, as in the web map.
/// The rows change in place, so the table, search and detail pane read the
/// inherited owner from the row itself without a second copy of every row.
/// Returns each changed row's index with the owner it had before.
pub fn inherit_gitops_owners(
    rows: &mut [ResourceSummary],
) -> Vec<(usize, Option<GitOpsOwnerSummary>)> {
    fn namespace_key(row: &ResourceSummary) -> &str {
        row.namespace.as_deref().unwrap_or_default()
    }
    let inherited: Vec<(usize, usize)> = {
        let rows = &*rows;
        let owner_keys: HashSet<(&str, &str)> = rows
            .iter()
            .filter_map(|row| Some((namespace_key(row), row.owner_ref.as_deref()?)))
            .collect();
        if owner_keys.is_empty() {
            return Vec::new();
        }
        let mut parents: HashMap<(&str, &str), usize> = HashMap::new();
        for (index, row) in rows.iter().enumerate() {
            let key = (namespace_key(row), row.name.as_str());
            if owner_keys.contains(&key) {
                parents.insert(key, index);
            }
        }
        if parents.is_empty() {
            return Vec::new();
        }
        rows.iter()
            .enumerate()
            .filter_map(|(index, row)| {
                if owner_ref(row).is_some() {
                    return None;
                }
                let mut seen = HashSet::from([index]);
                let mut current = row;
                while let Some(parent) = current.owner_ref.as_deref() {
                    let &owner_index = parents.get(&(namespace_key(current), parent))?;
                    if !seen.insert(owner_index) {
                        return None;
                    }
                    let owner = &rows[owner_index];
                    if owner_ref(owner).is_some() {
                        return Some((index, owner_index));
                    }
                    current = owner;
                }
                None
            })
            .collect()
    };
    // An owner row has its own label, so it is never itself in `inherited` and
    // still holds its own owner while the rows below it copy from it.
    inherited
        .into_iter()
        .map(|(index, owner_index)| {
            let owner = rows[owner_index].git_ops_owner.clone();
            (
                index,
                std::mem::replace(&mut rows[index].git_ops_owner, owner),
            )
        })
        .collect()
}

/// `formatResourceTypeGroupLabel` in `src/features/resources/helpers.ts`.
pub fn type_label(row: &ResourceSummary) -> String {
    let kind = row.kind.as_str();
    if let Some(plural) = row
        .plural
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        let shared = kind
            .chars()
            .zip(plural.chars())
            .take_while(|(a, b)| a.eq_ignore_ascii_case(b))
            .count();
        return if shared > 0 {
            format!("{}{}", &kind[..shared], &plural[shared..])
        } else {
            let mut chars = plural.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect())
                .unwrap_or_default()
        };
    }
    let lower = kind.to_ascii_lowercase();
    let consonant_y =
        lower.ends_with('y') && !lower[..lower.len() - 1].ends_with(['a', 'e', 'i', 'o', 'u']);
    if consonant_y {
        format!("{}ies", &kind[..kind.len() - 1])
    } else if ["s", "x", "z", "ch", "sh"]
        .iter()
        .any(|end| lower.ends_with(end))
    {
        format!("{kind}es")
    } else {
        format!("{kind}s")
    }
}

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
fn type_key(group: &str, type_label: &str) -> String {
    format!("{}::type:{type_label}", group_key(group))
}

fn same_resource(left: &ResourceSummary, right: &ResourceSummary) -> bool {
    left.kind == right.kind && left.namespace == right.namespace && left.name == right.name
}

pub struct TablePage {
    pub entries: Vec<Entry>,
    pub rows: Vec<ResourceSummary>,
    pub total: usize,
    pub page_count: usize,
}

/// Rows already in the table's sort order, grouped by ownership, then kind
/// lane, as `buildResourceTableDisplay` and `buildResourceTableEntries` do.
/// The tests compare the prepared order with this direct form of the web's.
#[cfg(test)]
fn grouped_page(
    mut sorted: Vec<&ResourceSummary>,
    view: &TableView<'_>,
    preferred: impl Fn(&ResourceSummary) -> bool,
    lanes: &crate::preferences::KindLanes,
) -> TablePage {
    let grouped_by_gitops = sorted.iter().any(|row| owner_ref(row).is_some());
    if grouped_by_gitops {
        sorted.sort_by_key(|row| !preferred(row));
    }
    let mut groups: Vec<(String, u8, Vec<&ResourceSummary>)> = Vec::new();
    for row in sorted {
        let (label, rank) = if grouped_by_gitops {
            ownership_group(row)
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
    page_from_order(display, grouped_by_gitops, view)
}

fn page_from_order(
    display: Vec<&ResourceSummary>,
    grouped_by_gitops: bool,
    view: &TableView<'_>,
) -> TablePage {
    let total = display.len();
    let page_count = total.div_ceil(PAGE_SIZE).max(1);
    let page = view.page.min(page_count - 1);
    let page_rows: Vec<ResourceSummary> = display
        .iter()
        .skip(page * PAGE_SIZE)
        .take(PAGE_SIZE)
        .map(|row| (*row).clone())
        .collect();
    // The selected resource's groups stay open even when collapsed, as on the web.
    let selected = view
        .selected
        .and_then(|selected| display.iter().find(|row| same_resource(row, selected)))
        .map(|row| {
            let group = ownership_group(row).0;
            let type_label = type_label(row);
            (group_key(&group), type_key(&group, &type_label))
        });
    let mut entries = Vec::new();
    let mut previous: Option<(String, String)> = None;
    for (index, row) in page_rows.iter().enumerate() {
        let group = ownership_group(row).0;
        let kind = type_label(row);
        let group_changed = previous.as_ref().is_none_or(|(g, _)| *g != group);
        let type_changed = previous
            .as_ref()
            .is_none_or(|(g, t)| (grouped_by_gitops && *g != group) || *t != kind);
        let (row_group_key, row_type_key) = (group_key(&group), type_key(&group, &kind));
        let group_collapsed = grouped_by_gitops
            && selected
                .as_ref()
                .is_none_or(|(key, _)| *key != row_group_key)
            && view.collapsed.contains(&row_group_key);
        let type_collapsed = selected
            .as_ref()
            .is_none_or(|(_, key)| *key != row_type_key)
            && view.collapsed.contains(&row_type_key);
        if grouped_by_gitops && group_changed {
            let count = page_rows
                .iter()
                .filter(|other| ownership_group(other).0 == group)
                .count();
            entries.push(Entry::Group {
                key: row_group_key.clone(),
                label: group.clone(),
                count,
                collapsed: group_collapsed,
            });
        }
        if type_changed && !group_collapsed {
            let count = page_rows
                .iter()
                .filter(|other| {
                    (!grouped_by_gitops || ownership_group(other).0 == group)
                        && type_label(other) == kind
                })
                .count();
            entries.push(Entry::Type {
                key: row_type_key.clone(),
                label: kind.clone(),
                kind: row.kind.clone(),
                count,
                collapsed: type_collapsed,
            });
        }
        if !group_collapsed && !type_collapsed {
            entries.push(Entry::Row(index));
        }
        previous = Some((group, kind));
    }
    TablePage {
        entries,
        rows: page_rows,
        total,
        page_count,
    }
}

/// The table page in display order: the prepared order for the default sort,
/// and a sort of the visible rows' packed keys for any other sort.
pub fn indexed_table_page(
    visible: &Visible,
    view: &TableView<'_>,
    usage: impl Fn(&ResourceSummary) -> table_sort::Usage,
) -> TablePage {
    let _profile = crate::frame_profile::scope("table", "project");
    if view.sort == Sort::default() {
        return page_from_order(visible.ordered().collect(), visible.grouped_by_gitops, view);
    }
    let rows = visible.all_rows();
    let order = visible.sorted_order(view.sort, usage);
    page_from_order(
        order.into_iter().map(|index| &rows[index]).collect(),
        visible.grouped_by_gitops,
        view,
    )
}

#[cfg(test)]
fn display_order(rows: &[ResourceSummary]) -> (Vec<usize>, bool) {
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
        let order = table_sort::reference::order(&refs, view.sort, |_| Default::default());
        let output = grouped_page(
            order.into_iter().map(|index| refs[index]).collect(),
            &view,
            |_| false,
            &crate::preferences::KindLanes::default(),
        );
        indices.extend(output.rows.iter().map(|row| {
            rows.iter()
                .position(|candidate| same_resource(candidate, row))
                .unwrap()
        }));
    }
    (indices, grouped)
}

/// Icon and tone from `src/app/svelte/resourceVisuals.ts`.
pub fn kind_visual(kind: &str) -> (&'static str, Hsla) {
    match kind {
        "Pod" => ("box", Tokens::pod()),
        "Deployment" => ("rocket", Tokens::workload()),
        "ReplicaSet" => ("box", Tokens::replicaset()),
        "StatefulSet" => ("database", Tokens::storage()),
        "DaemonSet" => ("server", Tokens::workload()),
        "Job" => ("briefcase-business", Tokens::cluster()),
        "CronJob" => ("clock-3", Tokens::cluster()),
        "Service" => ("network", Tokens::network()),
        "Ingress" => ("earth", Tokens::network()),
        "ConfigMap" => ("file-cog", Tokens::config()),
        "Secret" => ("key-round", Tokens::secret()),
        "PersistentVolumeClaim" => ("hard-drive", Tokens::storage()),
        "PersistentVolume" => ("database", Tokens::storage()),
        "StorageClass" => ("layers", Tokens::storage()),
        "Node" => ("server", Tokens::cluster()),
        "Application" => ("git-branch", Tokens::primary()),
        "ApplicationSet" => ("workflow", Tokens::primary()),
        "AppProject" => ("shield", Tokens::primary()),
        _ => ("package", Tokens::muted_foreground()),
    }
}

pub fn group_visual(label: &str) -> (&'static str, Hsla) {
    if label.starts_with("Owned by Argo CD") || label.starts_with("Tracked by Argo CD") {
        ("git-branch", Tokens::primary())
    } else {
        ("package", Tokens::muted_foreground())
    }
}

/// `healthSourceSummary` in `src/lib/resource-health.ts`.
pub fn health_source(sources: &[HealthAssessmentSource]) -> String {
    if sources.is_empty() {
        return "Health contract".into();
    }
    sources
        .iter()
        .map(|source| match source {
            HealthAssessmentSource::Kubernetes => "Kubernetes status/conditions",
            HealthAssessmentSource::ArgoHealth => "Argo CD health",
            HealthAssessmentSource::ArgoSync => "Argo CD sync",
            HealthAssessmentSource::WarningEvent => "Kubernetes Warning Event",
            HealthAssessmentSource::ContainerRestart => "Container restart",
            HealthAssessmentSource::Provider => "Provider availability",
            HealthAssessmentSource::HealthContract => "Health contract",
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

/// `formatExactTimestamp(value, "local", "millisecond")` in
/// `src/components/timestamp-format.ts`.
pub fn exact_timestamp(
    value: &str,
    timezone: crate::preferences::TimestampTimezone,
) -> Option<String> {
    let parsed = chrono::DateTime::parse_from_rfc3339(value).ok()?;
    Some(match timezone {
        crate::preferences::TimestampTimezone::Local => parsed
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M:%S%.3f")
            .to_string(),
        crate::preferences::TimestampTimezone::Utc => parsed
            .with_timezone(&chrono::Utc)
            .format("%Y-%m-%d %H:%M:%S%.3f UTC")
            .to_string(),
    })
}

/// Kinds label in the resource scope trigger: first three, then `+N`.
pub fn kinds_label(kinds: &[String]) -> String {
    let shown: Vec<&str> = kinds.iter().take(3).map(String::as_str).collect();
    match kinds.len().saturating_sub(3) {
        0 => shown.join(", "),
        more => format!("{} +{more}", shown.join(", ")),
    }
}
