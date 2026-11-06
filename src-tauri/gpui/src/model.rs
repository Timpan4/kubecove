//! Presentation model shared by the overview and resource table. Each rule
//! cites the Svelte module it mirrors so the two clients stay comparable.

mod rows;
use crate::table_sort::{self, Sort};
use crate::theme::Tokens;
use gpui_kit::Hsla;
use kubecove_lib::models::{HealthAssessmentSource, HealthAssessmentState, ResourceSummary};
pub use rows::Visible;
pub(crate) use rows::{KindFilter, ObservedNames, PreparedRows, reconcile};
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

/// The GitOps owner filter entry for a row, as `gitOpsOwnership` in
/// `src/lib/gitops-ownership-evidence.ts` computes it: the `filterKey`
/// (`provider:kind:namespace:name`) and the group label. Owners the adapters
/// do not recognize have no entry.
pub fn gitops_owner_filter(row: &ResourceSummary) -> Option<(String, String)> {
    let label = gitops_label(row)?;
    let key = match row.git_ops_owner.as_ref() {
        Some(owner) => format!(
            "{}:{}:{}:{}",
            owner.provider,
            owner.kind,
            owner.namespace.as_deref().unwrap_or_default(),
            owner.name.trim()
        ),
        None => format!("argo:Application::{}", row.argo_app.as_deref()?.trim()),
    };
    Some((key, label))
}

/// `gitOpsOwnershipGroup` in `src/lib/gitops-ownership-evidence.ts`.
pub fn ownership_group(row: &ResourceSummary) -> (String, u8) {
    if let Some(label) = gitops_label(row) {
        return (label, 0);
    }
    if let Some(release) = &row.helm_release {
        return (format!("Helm release: {release}"), 1);
    }
    let label = if row.git_ops_ownership_partial {
        "No GitOps ownership evidence (partial coverage)"
    } else {
        "No GitOps ownership evidence"
    };
    (label.into(), 2)
}

/// A GitOps owner that a provider adapter recognizes, as `gitOpsOwnership`
/// returns it in `src/lib/gitops-ownership-evidence.ts`.
pub struct GitOpsOwned {
    pub provider: String,
    pub kind: String,
    pub name: String,
    pub namespace: Option<String>,
    pub confidence: String,
    pub provenance: String,
    pub partial: bool,
}

/// The row's GitOps owner. An Argo CD name annotation stands in when the
/// backend reports no owner, and a row with no recognized owner has none.
pub fn gitops_owner(row: &ResourceSummary) -> Option<GitOpsOwned> {
    // Rust owners always state `partial`; an annotation-only Argo app is partial.
    let owner = match row.git_ops_owner.as_ref() {
        Some(owner) => GitOpsOwned {
            provider: owner.provider.clone(),
            kind: owner.kind.clone(),
            name: owner.name.trim().to_owned(),
            namespace: owner.namespace.clone(),
            confidence: owner.confidence.clone(),
            provenance: owner.provenance.clone(),
            partial: owner.partial,
        },
        None => GitOpsOwned {
            provider: "argo".into(),
            kind: "Application".into(),
            name: row.argo_app.as_deref()?.trim().to_owned(),
            namespace: None,
            confidence: "metadata".into(),
            provenance: "argocd-name-annotation".into(),
            partial: true,
        },
    };
    let recognized = matches!(
        (owner.provider.as_str(), owner.kind.as_str()),
        ("argo", "Application") | ("flux", "Kustomization" | "HelmRelease")
    );
    (recognized && !owner.name.is_empty()).then_some(owner)
}

/// `ownerName` in `gitOpsOwnership`: the Flux owner is qualified by its namespace.
pub fn gitops_owner_name(row: &ResourceSummary) -> Option<String> {
    let owner = gitops_owner(row)?;
    Some(match (owner.provider.as_str(), owner.namespace) {
        ("flux", Some(namespace)) => format!("{namespace}/{}", owner.name),
        _ => owner.name,
    })
}

fn gitops_label(row: &ResourceSummary) -> Option<String> {
    let owner = gitops_owner(row)?;
    let name = gitops_owner_name(row)?;
    let label = if owner.provider == "argo" {
        format!("Owned by Argo CD: {name}")
    } else {
        format!("Owned by Flux {}: {name}", owner.kind)
    };
    Some(if owner.partial {
        format!("{label} (partial evidence)")
    } else {
        label
    })
}

/// `inheritGitOpsOwnership` in `src/lib/gitops-ownership-evidence.ts`.
///
/// A row without direct GitOps evidence takes the `gitOpsOwner` of its nearest
/// `ownerRef` ancestor in the same namespace that has evidence. Parents match by
/// namespace and name only, and the last row with a name wins, as in the web map.
/// Callers run this once per data change and keep the result as the source for
/// filtering and grouping, so frames never walk owner chains.
pub fn inherit_gitops_owners(rows: &[ResourceSummary]) -> Vec<ResourceSummary> {
    let namespace_key = |row: &ResourceSummary| row.namespace.clone().unwrap_or_default();
    let owner_keys: HashSet<(String, String)> = rows
        .iter()
        .filter_map(|row| Some((namespace_key(row), row.owner_ref.clone()?)))
        .collect();
    if owner_keys.is_empty() {
        return rows.to_vec();
    }
    let mut parents: HashMap<(String, String), usize> = HashMap::new();
    for (index, row) in rows.iter().enumerate() {
        let key = (namespace_key(row), row.name.clone());
        if owner_keys.contains(&key) {
            parents.insert(key, index);
        }
    }
    if parents.is_empty() {
        return rows.to_vec();
    }
    rows.iter()
        .enumerate()
        .map(|(index, row)| {
            if gitops_label(row).is_some() {
                return row.clone();
            }
            let mut seen = HashSet::from([index]);
            let mut current = row;
            while let Some(owner_ref) = current.owner_ref.as_ref() {
                let Some(&owner_index) = parents.get(&(namespace_key(current), owner_ref.clone()))
                else {
                    break;
                };
                if !seen.insert(owner_index) {
                    break;
                }
                let owner = &rows[owner_index];
                if gitops_label(owner).is_some() {
                    let mut inherited = row.clone();
                    inherited.git_ops_owner = owner.git_ops_owner.clone();
                    return inherited;
                }
                current = owner;
            }
            row.clone()
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

/// Rows sorted by the table's column, grouped by ownership, then kind lane, as
/// `buildResourceTableDisplay` and `buildResourceTableEntries` do. Only the
/// current page's rows are cloned.
pub fn table_page(
    rows: &[ResourceSummary],
    view: &TableView<'_>,
    preferred: impl Fn(&ResourceSummary) -> bool,
    lanes: &crate::preferences::KindLanes,
    usage: impl Fn(&ResourceSummary) -> table_sort::Usage,
) -> TablePage {
    let order = table_sort::order(rows, view.sort, usage);
    let mut sorted: Vec<&ResourceSummary> = order.into_iter().map(|index| &rows[index]).collect();
    let grouped_by_gitops = sorted.iter().any(|row| gitops_label(row).is_some());
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

/// Reuses the prepared display order for the default sort.
pub fn indexed_table_page(
    visible: &Visible,
    view: &TableView<'_>,
    preferred: impl Fn(&ResourceSummary) -> bool,
    lanes: &crate::preferences::KindLanes,
    usage: impl Fn(&ResourceSummary) -> table_sort::Usage,
) -> TablePage {
    if view.sort == Sort::default() {
        page_from_order(visible.ordered().collect(), visible.grouped_by_gitops, view)
    } else {
        table_page(
            &visible.iter().cloned().collect::<Vec<_>>(),
            view,
            preferred,
            lanes,
            usage,
        )
    }
}

#[cfg(test)]
fn display_order(rows: &[ResourceSummary]) -> (Vec<usize>, bool) {
    let grouped = rows.iter().any(|row| gitops_label(row).is_some());
    let collapsed = HashSet::new();
    let mut indices = Vec::new();
    for page in 0..rows.len().div_ceil(PAGE_SIZE) {
        let view = TableView {
            page,
            sort: Sort::default(),
            collapsed: &collapsed,
            selected: None,
        };
        let output = table_page(
            rows,
            &view,
            |_| false,
            &crate::preferences::KindLanes::default(),
            |_| Default::default(),
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
