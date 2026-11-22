//! The Helm page model: `src/features/helm/helpers.ts`, `surfaceState.ts`
//! and the counts and source labels in `HelmView.svelte` and `HelmSurface.svelte`.
//! Releases are addressed by their index in the release list.

use crate::collate::collate;
use kubecove_lib::models::{
    HelmReconciliationResource, HelmReconciliationStatus, HelmReleaseSummary,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Success,
    Warning,
    Error,
    Neutral,
}

/// `helmStatusTone`.
pub fn status_tone(status: Option<&str>) -> Tone {
    match status {
        Some("deployed" | "superseded") => Tone::Success,
        Some("failed" | "uninstalled") => Tone::Error,
        Some(status) if status.starts_with("pending-") => Tone::Warning,
        _ => Tone::Neutral,
    }
}

/// One namespace in the rail: `groupHelmReleasesByNamespace` with counts.
pub struct Group<'a> {
    pub namespace: &'a str,
    pub count: usize,
}

/// `groupHelmReleasesByNamespace`: namespaces in collation order.
pub fn groups(releases: &[HelmReleaseSummary]) -> Vec<Group<'_>> {
    let mut groups: Vec<Group<'_>> = Vec::new();
    for release in releases {
        match groups
            .iter_mut()
            .find(|group| group.namespace == release.namespace)
        {
            Some(group) => group.count += 1,
            None => groups.push(Group {
                namespace: &release.namespace,
                count: 1,
            }),
        }
    }
    groups.sort_by(|left, right| collate(left.namespace, right.namespace, false));
    groups
}

/// `buildHelmReleaseState(...).filtered`: a case-insensitive search over the
/// joined release fields, then the namespace, in list order.
pub fn filter(
    releases: &[HelmReleaseSummary],
    search: &str,
    namespace: Option<&str>,
) -> Vec<usize> {
    let term = search.trim().to_lowercase();
    releases
        .iter()
        .enumerate()
        .filter(|(_, release)| term.is_empty() || matches_search(release, &term))
        .filter(|(_, release)| namespace.is_none_or(|namespace| release.namespace == namespace))
        .map(|(index, _)| index)
        .collect()
}

fn matches_search(release: &HelmReleaseSummary, term: &str) -> bool {
    [
        release.name.as_str(),
        release.namespace.as_str(),
        release.chart.as_deref().unwrap_or_default(),
        release.app_version.as_deref().unwrap_or_default(),
        release.status.as_deref().unwrap_or_default(),
        release.storage_kind.as_str(),
        release.storage_name.as_str(),
    ]
    .join(" ")
    .to_lowercase()
    .contains(term)
}

/// `resolveHelmNamespace`: a namespace filter survives only while it has releases.
pub fn resolve_namespace(
    releases: &[HelmReleaseSummary],
    namespace: Option<String>,
) -> Option<String> {
    namespace.filter(|namespace| {
        releases
            .iter()
            .any(|release| &release.namespace == namespace)
    })
}

/// `helmReleaseKey`'s identity without the cluster, which one list shares.
pub fn same_release(left: &HelmReleaseSummary, right: &HelmReleaseSummary) -> bool {
    left.namespace == right.namespace
        && left.name == right.name
        && left.storage_kind == right.storage_kind
        && left.storage_name == right.storage_name
}

/// Deployed, pending and failed counts from `HelmView.svelte`.
#[derive(Default)]
pub struct Counts {
    pub deployed: usize,
    pub pending: usize,
    pub failed: usize,
}

pub fn counts(releases: &[HelmReleaseSummary]) -> Counts {
    let mut counts = Counts::default();
    for status in releases
        .iter()
        .filter_map(|release| release.status.as_deref())
    {
        if status == "deployed" {
            counts.deployed += 1;
        } else if status == "failed" {
            counts.failed += 1;
        } else if status.starts_with("pending-") {
            counts.pending += 1;
        }
    }
    counts
}

/// `helmReconciliationStatusTone`.
pub fn reconciliation_tone(status: HelmReconciliationStatus) -> Tone {
    match status {
        HelmReconciliationStatus::Tracked => Tone::Success,
        HelmReconciliationStatus::UnlabeledLive | HelmReconciliationStatus::LabelOnly => {
            Tone::Warning
        }
        HelmReconciliationStatus::Missing => Tone::Error,
        HelmReconciliationStatus::Unavailable => Tone::Neutral,
    }
}

/// `helmReconciliationStatusLabel`.
pub fn reconciliation_label(status: HelmReconciliationStatus) -> &'static str {
    match status {
        HelmReconciliationStatus::Tracked => "Tracked",
        HelmReconciliationStatus::UnlabeledLive => "Unlabeled live",
        HelmReconciliationStatus::Missing => "Missing",
        HelmReconciliationStatus::LabelOnly => "Label-only",
        HelmReconciliationStatus::Unavailable => "Unavailable",
    }
}

/// `helmReconciliationResourceLabel`.
pub fn resource_label(resource: &HelmReconciliationResource) -> String {
    format!(
        "{}/{}",
        resource.kind.as_deref().unwrap_or("Unknown"),
        resource.name.as_deref().unwrap_or("<unnamed>")
    )
}

/// `helmReconciliationSource` in `HelmSurface.svelte`.
pub fn reconciliation_source(resource: &HelmReconciliationResource) -> &'static str {
    match (resource.in_manifest, resource.live_resource.is_some()) {
        (true, true) => "Manifest + live",
        (true, false) => "Manifest",
        (false, true) => "Live label",
        (false, false) => "-",
    }
}

/// `sortHelmReconciliationResources`: by namespace, kind, name and API version.
pub fn sort_reconciliation(resources: &mut [HelmReconciliationResource]) {
    let key = |resource: &HelmReconciliationResource| {
        [
            resource.namespace.as_deref().unwrap_or_default(),
            resource.kind.as_deref().unwrap_or_default(),
            resource.name.as_deref().unwrap_or_default(),
            resource.api_version.as_deref().unwrap_or_default(),
        ]
        .join(":")
    };
    resources.sort_by(|left, right| collate(&key(left), &key(right), false));
}
