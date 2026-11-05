//! The GitOps page model: `src/features/gitops/surfaceModel.ts`,
//! `surfaceSummary.ts` and the sidebar kinds of `gitops-nav.ts`. Card helpers
//! live in [`selection`].

mod selection;

pub use selection::{Selection, SourceMode, status_label};

use crate::gitops::GitOpsInventory;
use kubecove_lib::models::{HealthAssessment, HealthAssessmentState};

/// `ARGO_NAV_KINDS`: the sidebar label of each Argo CD kind.
pub const ARGO_KINDS: [(ArgoKind, &str); 3] = [
    (ArgoKind::Applications, "Argo CD Applications"),
    (ArgoKind::ApplicationSets, "Argo CD ApplicationSets"),
    (ArgoKind::AppProjects, "Argo CD AppProjects"),
];

/// `FLUX_FAMILIES`: each family's label and its kinds as `(kind, sidebar label)`.
pub const FLUX_FAMILIES: [(&str, &[(&str, &str)]); 4] = [
    (
        "Sources",
        &[
            ("GitRepository", "Flux Git Repositories"),
            ("OCIRepository", "Flux OCI Repositories"),
            ("HelmRepository", "Flux Helm Repositories"),
            ("HelmChart", "Flux Helm Charts"),
            ("Bucket", "Flux Buckets"),
        ],
    ),
    (
        "Workloads",
        &[
            ("Kustomization", "Flux Kustomizations"),
            ("HelmRelease", "Flux Helm Releases"),
        ],
    ),
    (
        "Notifications",
        &[
            ("Provider", "Flux Providers"),
            ("Alert", "Flux Alerts"),
            ("Receiver", "Flux Receivers"),
        ],
    ),
    (
        "Image Automation",
        &[
            ("ImageRepository", "Flux Image Repositories"),
            ("ImagePolicy", "Flux Image Policies"),
            ("ImageUpdateAutomation", "Flux Image Update Automations"),
        ],
    ),
];

/// `ArgoKindKey`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgoKind {
    Applications,
    ApplicationSets,
    AppProjects,
}

/// The sidebar node selected inside GitOps (`selectedNode` in the web).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Focus {
    /// The GitOps section row itself.
    #[default]
    Section,
    /// The Argo CD provider group, or one of its kinds.
    Argo(Option<ArgoKind>),
    /// The Flux provider group (no family or kind), a family by its label, or
    /// one Flux kind such as `HelmRelease`. A family selects every Flux
    /// resource, as in the web.
    Flux {
        family: Option<&'static str>,
        kind: Option<&'static str>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    Argo,
    Flux,
}

impl Provider {
    pub fn label(self) -> &'static str {
        match self {
            Provider::Argo => "Argo CD",
            Provider::Flux => "Flux",
        }
    }
}

/// `GitOpsRailItem`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RailItem {
    pub key: String,
    pub provider: Provider,
    pub label: String,
    pub count: usize,
    pub disabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Healthy,
    Degraded,
}

/// `GitOpsSummaryFact`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fact {
    pub label: &'static str,
    pub value: usize,
    pub tone: Option<Tone>,
}

/// `GitOpsSummary`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    pub active_provider: Provider,
    pub detected_providers: Vec<Provider>,
    pub total_objects: usize,
    pub facts: Vec<Fact>,
}

/// `GitOpsTable` without its cells; the page renders [`Row`]s.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    pub title: String,
    pub empty: &'static str,
}

/// `GitOpsUnavailableProvider`.
pub struct Unavailable {
    pub title: &'static str,
    pub description: &'static str,
}

/// One `GitOpsSelection` as an index into the inventory, so the page keeps
/// its row list without borrowing the inventory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    App(usize),
    AppSet(usize),
    Project(usize),
    Flux(usize),
}

impl Row {
    pub fn resolve(self, data: &GitOpsInventory) -> Option<Selection<'_>> {
        Some(match self {
            Row::App(index) => Selection::App(data.argo.as_ref()?.get(index)?),
            Row::AppSet(index) => Selection::AppSet(data.app_sets.get(index)?),
            Row::Project(index) => Selection::Project(data.projects.get(index)?),
            Row::Flux(index) => Selection::Flux(data.flux.as_ref()?.get(index)?),
        })
    }
}

fn flux(data: &GitOpsInventory) -> &[kubecove_lib::models::FluxResourceSummary] {
    data.flux.as_deref().unwrap_or_default()
}

/// `shouldShowFluxByDefault`: neither an Argo CD node nor a detected Argo CD.
fn flux_by_default(data: &GitOpsInventory, focus: Focus) -> bool {
    !matches!(focus, Focus::Argo(_)) && data.argo.is_none() && !flux(data).is_empty()
}

/// `buildGitOpsTable`.
pub fn table(data: &GitOpsInventory, focus: Focus) -> Table {
    let (title, empty) = match focus {
        Focus::Argo(Some(ArgoKind::ApplicationSets)) => (
            "Argo CD ApplicationSets".to_owned(),
            "No Argo CD ApplicationSets found",
        ),
        Focus::Argo(Some(ArgoKind::AppProjects)) => (
            "Argo CD AppProjects".to_owned(),
            "No Argo CD AppProjects found",
        ),
        Focus::Flux {
            kind: Some(kind), ..
        } => (format!("Flux {kind}"), "No Flux resources found"),
        Focus::Flux { kind: None, .. } => ("Flux Resources".to_owned(), "No Flux resources found"),
        _ if flux_by_default(data, focus) => {
            ("Flux Resources".to_owned(), "No Flux resources found")
        }
        _ => (
            "Argo CD Applications".to_owned(),
            "No Argo CD Applications found",
        ),
    };
    Table { title, empty }
}

/// `buildGitOpsSelections`, as inventory indices.
pub fn rows(data: &GitOpsInventory, focus: Focus) -> Vec<Row> {
    let flux_rows = |kind: Option<&str>| -> Vec<Row> {
        flux(data)
            .iter()
            .enumerate()
            .filter(|(_, item)| kind.is_none_or(|kind| item.resource_kind.kind == kind))
            .map(|(index, _)| Row::Flux(index))
            .collect()
    };
    match focus {
        Focus::Argo(Some(ArgoKind::ApplicationSets)) => {
            (0..data.app_sets.len()).map(Row::AppSet).collect()
        }
        Focus::Argo(Some(ArgoKind::AppProjects)) => {
            (0..data.projects.len()).map(Row::Project).collect()
        }
        Focus::Flux { kind, .. } => flux_rows(kind),
        _ if flux_by_default(data, focus) => flux_rows(None),
        _ => (0..data.argo.as_ref().map_or(0, Vec::len))
            .map(Row::App)
            .collect(),
    }
}

/// `buildGitOpsRailItems`.
pub fn rail_items(data: &GitOpsInventory) -> Vec<RailItem> {
    let argo_disabled = data.argo.is_none();
    let argo = [
        (
            "argo:applications",
            "Applications",
            data.argo.as_ref().map_or(0, Vec::len),
        ),
        (
            "argo:applicationSets",
            "ApplicationSets",
            data.app_sets.len(),
        ),
        ("argo:appProjects", "AppProjects", data.projects.len()),
    ];
    let mut items: Vec<RailItem> = argo
        .into_iter()
        .map(|(key, label, count)| RailItem {
            key: key.to_owned(),
            provider: Provider::Argo,
            label: label.to_owned(),
            count,
            disabled: argo_disabled,
        })
        .collect();
    let rows = flux(data);
    let flux_disabled = data.flux.is_none();
    if data.flux.is_some() {
        if rows.is_empty() {
            items.push(RailItem {
                key: "flux:resources".to_owned(),
                provider: Provider::Flux,
                label: "Resources".to_owned(),
                count: 0,
                disabled: flux_disabled,
            });
        }
        let mut counts: Vec<(&str, usize)> = Vec::new();
        for row in rows {
            let kind = row.resource_kind.kind.as_str();
            match counts.iter_mut().find(|(seen, _)| *seen == kind) {
                Some((_, count)) => *count += 1,
                None => counts.push((kind, 1)),
            }
        }
        counts.sort_by(|left, right| crate::collate::collate(left.0, right.0, false));
        items.extend(counts.into_iter().map(|(kind, count)| RailItem {
            key: format!("flux:{kind}"),
            provider: Provider::Flux,
            label: kind.to_owned(),
            count,
            disabled: flux_disabled,
        }));
    }
    items
}

/// `gitOpsActiveRailKey`.
pub fn active_rail_key(data: &GitOpsInventory, focus: Focus) -> String {
    match focus {
        Focus::Argo(Some(ArgoKind::ApplicationSets)) => "argo:applicationSets".to_owned(),
        Focus::Argo(Some(ArgoKind::AppProjects)) => "argo:appProjects".to_owned(),
        Focus::Flux {
            kind: Some(kind), ..
        } => format!("flux:{kind}"),
        Focus::Flux { kind: None, .. } => "flux:resources".to_owned(),
        _ if flux_by_default(data, focus) => flux(data).first().map_or_else(
            || "flux:resources".to_owned(),
            |row| format!("flux:{}", row.resource_kind.kind),
        ),
        _ => "argo:applications".to_owned(),
    }
}

/// `gitOpsUnavailableProvider`: only the provider group rows report it.
pub fn unavailable(data: &GitOpsInventory, focus: Focus) -> Option<Unavailable> {
    match focus {
        Focus::Argo(None) if data.argo.is_none() => Some(Unavailable {
            title: "Argo CD not detected",
            description: "Argo CD CRDs were not detected in this cluster.",
        }),
        Focus::Flux {
            family: None,
            kind: None,
        } if data.flux.is_none() => Some(Unavailable {
            title: "Flux not detected",
            description: "Flux CRDs were not detected in this cluster.",
        }),
        _ => None,
    }
}

/// The health facts both providers share, after their object counts.
fn health_facts<'a>(health: impl Iterator<Item = &'a HealthAssessment>) -> [Fact; 5] {
    use HealthAssessmentState as State;
    let mut counts = [0usize; 5];
    for item in health {
        counts[match item.state {
            State::Healthy => 0,
            State::NeedsAttention => 1,
            State::Degraded => 2,
            State::Unknown => 3,
            State::NotEvaluated => 4,
        }] += 1;
    }
    let fact = |label, index: usize, tone| Fact {
        label,
        value: counts[index],
        tone,
    };
    [
        fact("Healthy", 0, Some(Tone::Healthy)),
        fact("Needs attention", 1, None),
        fact("Degraded", 2, Some(Tone::Degraded)),
        fact("Unknown", 3, None),
        fact("Not evaluated", 4, None),
    ]
}

/// `buildGitOpsSummary`.
pub fn summary(data: &GitOpsInventory, active_rail_key: &str) -> Summary {
    let apps = data.argo.as_deref().unwrap_or_default();
    let flux = flux(data);
    let mut detected_providers = Vec::new();
    if data.argo.is_some() {
        detected_providers.push(Provider::Argo);
    }
    if data.flux.is_some() {
        detected_providers.push(Provider::Flux);
    }
    let total_objects = apps.len() + data.app_sets.len() + data.projects.len() + flux.len();
    let count = |label, value| Fact {
        label,
        value,
        tone: None,
    };
    if active_rail_key.starts_with("flux:") {
        let mut kinds: Vec<&str> = flux
            .iter()
            .map(|item| item.resource_kind.kind.as_str())
            .collect();
        kinds.sort_unstable();
        kinds.dedup();
        let mut facts = vec![count("Resources", flux.len()), count("Kinds", kinds.len())];
        facts.extend(health_facts(
            flux.iter().map(|item| &item.health_assessment),
        ));
        return Summary {
            active_provider: Provider::Flux,
            detected_providers,
            total_objects,
            facts,
        };
    }
    let mut facts = vec![
        count("Applications", apps.len()),
        count("ApplicationSets", data.app_sets.len()),
        count("AppProjects", data.projects.len()),
    ];
    facts.extend(health_facts(
        apps.iter()
            .map(|item| &item.health_assessment)
            .chain(data.app_sets.iter().map(|item| &item.health_assessment))
            .chain(data.projects.iter().map(|item| &item.health_assessment)),
    ));
    Summary {
        active_provider: Provider::Argo,
        detected_providers,
        total_objects,
        facts,
    }
}
