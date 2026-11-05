//! Presentation model shared by the overview and resource table. Each rule
//! cites the Svelte module it mirrors so the two clients stay comparable.

use crate::collate::collate;
use crate::theme::Tokens;
use gpui_kit::Hsla;
use kubecove_lib::models::{HealthAssessmentSource, HealthAssessmentState, ResourceSummary};

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
            summary.total += 1;
            match row.health_assessment.state {
                HealthAssessmentState::Healthy => summary.healthy += 1,
                HealthAssessmentState::NeedsAttention => summary.attention += 1,
                HealthAssessmentState::Degraded => summary.degraded += 1,
                HealthAssessmentState::NotEvaluated => summary.not_evaluated += 1,
                HealthAssessmentState::Unknown => summary.unknown += 1,
            }
            if row.restarts.unwrap_or(0) > 0 {
                summary.restart_evidence += 1;
            }
        }
        summary
    }
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

fn gitops_label(row: &ResourceSummary) -> Option<String> {
    // Rust owners always state `partial`; an annotation-only Argo app is partial.
    let (provider, kind, name, namespace, partial) = match row.git_ops_owner.as_ref() {
        Some(owner) => (
            owner.provider.as_str(),
            owner.kind.as_str(),
            owner.name.trim().to_owned(),
            owner.namespace.clone(),
            owner.partial,
        ),
        None => {
            let app = row.argo_app.as_deref()?.trim();
            ("argo", "Application", app.to_owned(), None, true)
        }
    };
    if name.is_empty() {
        return None;
    }
    let label = match (provider, kind) {
        ("argo", "Application") => format!("Owned by Argo CD: {name}"),
        ("flux", "Kustomization" | "HelmRelease") => {
            let owner = namespace.map_or(name.clone(), |ns| format!("{ns}/{name}"));
            format!("Owned by Flux {kind}: {owner}")
        }
        _ => return None,
    };
    Some(if partial {
        format!("{label} (partial evidence)")
    } else {
        label
    })
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
        label: String,
        count: usize,
    },
    Type {
        label: String,
        kind: String,
        count: usize,
    },
    Row(usize),
}

pub struct TablePage {
    pub entries: Vec<Entry>,
    pub rows: Vec<ResourceSummary>,
    pub total: usize,
    pub page_count: usize,
}

/// Name-sorted rows grouped by ownership, then kind lane, as
/// `buildResourceTableDisplay` and `buildResourceTableEntries` do.
pub fn table_page(
    rows: &[ResourceSummary],
    page: usize,
    preferred: impl Fn(&ResourceSummary) -> bool,
    lanes: &crate::preferences::KindLanes,
) -> TablePage {
    let mut sorted: Vec<&ResourceSummary> = rows.iter().collect();
    // `Intl.Collator(undefined, { numeric: true })`.
    sorted.sort_by(|a, b| collate(&a.name, &b.name, true));
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
    let total = display.len();
    let page_count = total.div_ceil(PAGE_SIZE).max(1);
    let page = page.min(page_count - 1);
    let page_rows: Vec<ResourceSummary> = display
        .into_iter()
        .skip(page * PAGE_SIZE)
        .take(PAGE_SIZE)
        .cloned()
        .collect();
    let mut entries = Vec::new();
    let mut previous: Option<(String, String)> = None;
    for (index, row) in page_rows.iter().enumerate() {
        let group = if grouped_by_gitops {
            ownership_group(row).0
        } else {
            String::new()
        };
        let kind = type_label(row);
        let same_group = previous.as_ref().is_some_and(|(g, _)| *g == group);
        let same_type = same_group && previous.as_ref().is_some_and(|(_, t)| *t == kind);
        if grouped_by_gitops && !same_group {
            let count = page_rows
                .iter()
                .filter(|other| ownership_group(other).0 == group)
                .count();
            entries.push(Entry::Group {
                label: group.clone(),
                count,
            });
        }
        if !same_type {
            let count = page_rows
                .iter()
                .filter(|other| {
                    (!grouped_by_gitops || ownership_group(other).0 == group)
                        && type_label(other) == kind
                })
                .count();
            entries.push(Entry::Type {
                label: kind.clone(),
                kind: row.kind.clone(),
                count,
            });
        }
        entries.push(Entry::Row(index));
        previous = Some((group, kind));
    }
    TablePage {
        entries,
        rows: page_rows,
        total,
        page_count,
    }
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
