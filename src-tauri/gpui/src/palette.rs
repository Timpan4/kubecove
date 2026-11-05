//! Command palette entries and filtering, ported from
//! `src/features/command-palette/entries.ts` and `CommandPalette.svelte`.

use kubecove_lib::models::ResourceSummary;

/// `RESOURCE_RESULT_CAP` and `NAMESPACE_RESULT_CAP` in `CommandPalette.svelte`.
pub const RESOURCE_CAP: usize = 50;
pub const NAMESPACE_CAP: usize = 8;

/// Where a navigation entry leads in the native client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destination {
    WorkspaceOverview,
    ClusterOverview,
    Resources(Option<&'static str>),
    Incidents,
    PortForwards,
    GitOps(crate::gitops_surface::Focus),
    Helm,
    Rbac(Option<crate::rbac::RbacView>),
    Settings,
    Launcher,
}

pub struct NavEntry {
    pub id: String,
    pub label: String,
    search: String,
    pub destination: Destination,
}

fn entry(id: String, label: String, destination: Destination) -> NavEntry {
    NavEntry {
        search: label.to_lowercase(),
        id,
        label,
        destination,
    }
}

/// `STATIC_SECTION_NAMES` with each section's label and curated children.
const SECTIONS: &[(&str, &str, &[&str])] = &[
    ("workspaceOverview", "Workspace Overview", &[]),
    (
        "clusterOverview",
        "Cluster Overview",
        &["Node", "StorageClass", "PersistentVolume"],
    ),
    (
        "workloads",
        "Workloads",
        &[
            "Pod",
            "Deployment",
            "ReplicaSet",
            "StatefulSet",
            "DaemonSet",
            "Job",
            "CronJob",
        ],
    ),
    ("network", "Network", &["Service", "Ingress"]),
    ("config", "Config", &["ConfigMap", "Secret"]),
    (
        "storage",
        "Storage",
        &["PersistentVolumeClaim", "PersistentVolume", "StorageClass"],
    ),
    ("discovered", "Custom Resources", &[]),
    (
        "argo",
        "GitOps",
        &[
            "Argo CD Applications",
            "Argo CD ApplicationSets",
            "Argo CD AppProjects",
            "Flux Git Repositories",
            "Flux OCI Repositories",
            "Flux Helm Repositories",
            "Flux Helm Charts",
            "Flux Buckets",
            "Flux Kustomizations",
            "Flux Helm Releases",
            "Flux Providers",
            "Flux Alerts",
            "Flux Receivers",
            "Flux Image Repositories",
            "Flux Image Policies",
            "Flux Image Update Automations",
        ],
    ),
    ("helm", "Helm", &["Releases"]),
    ("incidents", "Incidents", &[]),
    ("portForwards", "Port Forwards", &[]),
    (
        "rbac",
        "RBAC",
        &[
            "Namespace Access",
            "Roles",
            "Cluster Roles",
            "Bindings",
            "Service Accounts",
        ],
    ),
];

fn destination(section: &str, child: Option<&'static str>) -> Destination {
    match section {
        "workspaceOverview" => Destination::WorkspaceOverview,
        "clusterOverview" => child.map_or(Destination::ClusterOverview, |kind| {
            Destination::Resources(Some(kind))
        }),
        "workloads" | "network" | "config" | "storage" | "discovered" => {
            Destination::Resources(child)
        }
        "incidents" => Destination::Incidents,
        "portForwards" => Destination::PortForwards,
        "argo" => {
            use crate::gitops_surface::{ARGO_KINDS, FLUX_FAMILIES, Focus};
            let focus = child
                .and_then(|child| {
                    ARGO_KINDS
                        .iter()
                        .find(|(_, label)| *label == child)
                        .map(|(kind, _)| Focus::Argo(Some(*kind)))
                        .or_else(|| {
                            FLUX_FAMILIES
                                .iter()
                                .flat_map(|(_, kinds)| kinds.iter())
                                .find(|(_, label)| *label == child)
                                .map(|(kind, _)| Focus::Flux {
                                    family: None,
                                    kind: Some(*kind),
                                })
                        })
                })
                .unwrap_or(Focus::Section);
            Destination::GitOps(focus)
        }
        "helm" => Destination::Helm,
        "rbac" => Destination::Rbac(child.and_then(|child| {
            crate::rbac::RbacView::ALL
                .into_iter()
                .find(|view| view.label() == child)
        })),
        _ => unreachable!("navigation section missing a destination: {section}"),
    }
}

/// `buildNavigationEntries`: every section and its kind children, then the
/// two header destinations.
pub fn navigation(gitops_visible: bool) -> Vec<NavEntry> {
    let mut entries = Vec::new();
    for (section, label, children) in SECTIONS {
        if *section == "argo" && !gitops_visible {
            continue;
        }
        entries.push(entry(
            format!("section:{section}"),
            (*label).to_owned(),
            destination(section, None),
        ));
        for child in *children {
            entries.push(entry(
                format!("kind:{section}:{child}"),
                format!("{label} › {child}"),
                destination(section, Some(child)),
            ));
        }
    }
    entries.push(NavEntry {
        id: "action:settings".into(),
        label: "Settings".into(),
        search: "settings preferences".into(),
        destination: Destination::Settings,
    });
    entries.push(NavEntry {
        id: "action:launcher".into(),
        label: "Open Workspaces".into(),
        search: "open workspaces launcher switch workspace".into(),
        destination: Destination::Launcher,
    });
    entries
}

/// `filterNavigationEntries`, as indices.
pub fn filter_navigation(entries: &[NavEntry], query: &str) -> Vec<usize> {
    let term = query.trim().to_lowercase();
    entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| term.is_empty() || entry.search.contains(&term))
        .map(|(index, _)| index)
        .collect()
}

/// `filterNamespaces` capped at [`NAMESPACE_CAP`]; empty without a query.
pub fn filter_namespaces<'a>(namespaces: &'a [String], query: &str) -> Vec<&'a str> {
    let term = query.trim().to_lowercase();
    if term.is_empty() {
        return vec![];
    }
    namespaces
        .iter()
        .filter(|namespace| namespace.to_lowercase().contains(&term))
        .take(NAMESPACE_CAP)
        .map(String::as_str)
        .collect()
}

/// `buildResourceSearchIndex`'s search text, built once per row.
pub fn search_text(resource: &ResourceSummary) -> String {
    let owner = resource.git_ops_owner.as_ref();
    [
        Some(resource.name.as_str()),
        resource.namespace.as_deref(),
        Some(resource.kind.as_str()),
        resource.api_version.as_deref(),
        resource.group.as_deref(),
        resource.plural.as_deref(),
        resource.owner_ref.as_deref(),
        resource.helm_release.as_deref(),
        owner.map(|owner| owner.name.as_str()),
    ]
    .into_iter()
    .flatten()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join("\n")
    .to_lowercase()
}

/// `filterResourceSearchIndex` capped at [`RESOURCE_CAP`]; empty without a query.
pub fn filter_resources(index: &[String], query: &str) -> Vec<usize> {
    let term = query.trim().to_lowercase();
    if term.is_empty() {
        return vec![];
    }
    index
        .iter()
        .enumerate()
        .filter(|(_, text)| text.contains(&term))
        .map(|(position, _)| position)
        .take(RESOURCE_CAP)
        .collect()
}
