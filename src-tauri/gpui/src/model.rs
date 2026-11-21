//! Presentation model shared by the overview and resource table. Each rule
//! cites the Svelte module it mirrors so the two clients stay comparable.

mod rows;
mod table;
use crate::theme::Tokens;
use gpui_kit::Hsla;
use kubecove_lib::models::{
    GitOpsOwnerSummary, HealthAssessmentSource, HealthAssessmentState, ResourceSummary,
};
pub use rows::Visible;
#[cfg(test)]
pub(crate) use rows::reconcile;
pub(crate) use rows::{
    Added, KindFilter, ObservedNames, PreparedRows, RowsChange, RowsId, identity_hash,
    identity_hash_of, watch_lists,
};
use std::collections::{HashMap, HashSet};
pub(crate) use table::project_table_page;
pub use table::{Entry, TablePage, TableView, indexed_table_page};
#[cfg(test)]
use table::{display_order, grouped_page};

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

/// `gitOpsOwnershipGroup` in `src/lib/gitops-ownership-evidence.ts`. In a view
/// of several contexts, owner and Helm groups belong to the row's context.
pub fn ownership_group(row: &ResourceSummary, multi_context: bool) -> (String, u8) {
    let (label, rank) = match (owner_ref(row), &row.helm_release) {
        (Some(owner), _) => (owner.label(), 0),
        (None, Some(release)) => (helm_group(release), 1),
        (None, None) => return (no_owner_group(row).into(), 2),
    };
    (in_context(label, &row.cluster, multi_context), rank)
}

fn helm_group(release: &str) -> String {
    format!("Helm release: {release}")
}

/// An owner or Helm group label. With several contexts the label names the
/// context, so the same owner name in two clusters makes two groups.
fn in_context(label: String, context: &str, multi_context: bool) -> String {
    if multi_context {
        format!("{label} · {context}")
    } else {
        label
    }
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
    /// The owner as a row's `git_ops_owner`, as `effectiveGitOpsOwner` in
    /// `src/lib/gitops-ownership-evidence.ts` builds it.
    fn to_summary(self) -> GitOpsOwnerSummary {
        GitOpsOwnerSummary {
            provider: self.provider.to_owned(),
            kind: self.kind.to_owned(),
            name: self.name.to_owned(),
            namespace: self.namespace.map(str::to_owned),
            confidence: self.confidence.to_owned(),
            provenance: self.provenance.to_owned(),
            partial: self.partial,
        }
    }

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

    /// The owner filter's `filterKey`: `provider:kind:namespace:name`, after
    /// `cluster:` in a view of several contexts.
    pub fn filter_key(self, context: &str, multi_context: bool) -> String {
        let key = format!(
            "{}:{}:{}:{}",
            self.provider,
            self.kind,
            self.namespace.unwrap_or_default(),
            self.name
        );
        if multi_context {
            format!("{context}:{key}")
        } else {
            key
        }
    }
}

/// `inheritGitOpsOwnership` in `src/lib/gitops-ownership-evidence.ts`.
///
/// A row without direct GitOps evidence takes the `gitOpsOwner` of its nearest
/// `ownerRef` ancestor in the same cluster and namespace that has evidence.
/// Parents match by cluster, namespace and name only, and the last row with a
/// name wins, as in the web map. The cluster keeps a Pod in one context from
/// inheriting the owner of a same-named ReplicaSet in another.
/// The rows change in place, so the table, search and detail pane read the
/// inherited owner from the row itself without a second copy of every row.
/// Returns each changed row's index with the owner it had before.
pub fn inherit_gitops_owners(
    rows: &mut [ResourceSummary],
) -> Vec<(usize, Option<GitOpsOwnerSummary>)> {
    /// The row's cluster and namespace with a name in them.
    fn named<'a>(row: &'a ResourceSummary, name: &'a str) -> (&'a str, &'a str, &'a str) {
        (
            &row.cluster,
            row.namespace.as_deref().unwrap_or_default(),
            name,
        )
    }
    let inherited: Vec<(usize, usize)> = {
        let rows = &*rows;
        let owner_keys: HashSet<(&str, &str, &str)> = rows
            .iter()
            .filter_map(|row| Some(named(row, row.owner_ref.as_deref()?)))
            .collect();
        if owner_keys.is_empty() {
            return Vec::new();
        }
        let mut parents: HashMap<(&str, &str, &str), usize> = HashMap::new();
        for (index, row) in rows.iter().enumerate() {
            let key = named(row, &row.name);
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
                    let &owner_index = parents.get(&named(current, parent))?;
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
    // still holds its own owner while the rows below it copy from it. An owner
    // known only by its Argo CD name annotation has no `git_ops_owner`, so the
    // rows below it take the owner `owner_ref` builds from the annotation.
    inherited
        .into_iter()
        .map(|(index, owner_index)| {
            let owner_row = &rows[owner_index];
            let owner = owner_row
                .git_ops_owner
                .clone()
                .or_else(|| owner_ref(owner_row).map(OwnerRef::to_summary));
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
                .map_or_default(|first| first.to_uppercase().chain(chars).collect())
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
    if kinds.is_empty() {
        return "None".to_owned();
    }
    let shown: Vec<&str> = kinds.iter().take(3).map(String::as_str).collect();
    match kinds.len().saturating_sub(3) {
        0 => shown.join(", "),
        more => format!("{} +{more}", shown.join(", ")),
    }
}
