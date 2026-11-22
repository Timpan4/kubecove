//! `GitOpsSelection` and its card helpers: `surfaceSelection.ts`,
//! `lib/gitops-resource.ts`, the card helpers of `surfaceTooltips.ts` and the
//! card functions in `GitOpsView.svelte`.

use kubecove_lib::models::{
    ArgoAppProjectSummary, ArgoApplicationSetSummary, ArgoApplicationSourceSummary,
    ArgoApplicationSummary, FluxResourceSummary, GitOpsOwnerSummary, HealthAssessment,
    ResourceHealth, ResourceSummary,
};

/// `GitOpsSelection`: one card or list row.
#[derive(Clone, Copy, Debug)]
pub enum Selection<'a> {
    App(&'a ArgoApplicationSummary),
    AppSet(&'a ArgoApplicationSetSummary),
    Project(&'a ArgoAppProjectSummary),
    Flux(&'a FluxResourceSummary),
}

/// `GitOpsSourceMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceMode {
    Git,
    Helm,
    Multi,
    Plugin,
    Unknown,
}

/// `formatStatusLabel`: dashes become spaces and the first letter is upper case.
pub fn status_label(value: &str) -> String {
    let label = value.replace('-', " ");
    let mut chars = label.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

fn present(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.is_empty())
}

impl<'a> Selection<'a> {
    pub fn name(self) -> &'a str {
        match self {
            Selection::App(item) => &item.name,
            Selection::AppSet(item) => &item.name,
            Selection::Project(item) => &item.name,
            Selection::Flux(item) => &item.name,
        }
    }

    pub fn namespace(self) -> Option<&'a str> {
        match self {
            Selection::App(item) => item.namespace.as_deref(),
            Selection::AppSet(item) => item.namespace.as_deref(),
            Selection::Project(item) => item.namespace.as_deref(),
            Selection::Flux(item) => item.namespace.as_deref(),
        }
    }

    pub fn age(self) -> &'a str {
        match self {
            Selection::App(item) => &item.age,
            Selection::AppSet(item) => &item.age,
            Selection::Project(item) => &item.age,
            Selection::Flux(item) => &item.age,
        }
    }

    fn cluster(self) -> &'a str {
        match self {
            Selection::App(item) => &item.cluster,
            Selection::AppSet(item) => &item.cluster,
            Selection::Project(item) => &item.cluster,
            Selection::Flux(item) => &item.cluster,
        }
    }

    fn created_at(self) -> Option<&'a String> {
        match self {
            Selection::App(item) => item.created_at.as_ref(),
            Selection::AppSet(item) => item.created_at.as_ref(),
            Selection::Project(item) => item.created_at.as_ref(),
            Selection::Flux(item) => item.created_at.as_ref(),
        }
    }

    pub fn health(self) -> &'a HealthAssessment {
        match self {
            Selection::App(item) => &item.health_assessment,
            Selection::AppSet(item) => &item.health_assessment,
            Selection::Project(item) => &item.health_assessment,
            Selection::Flux(item) => &item.health_assessment,
        }
    }

    /// `gitOpsSelectionKey`.
    pub fn key(self) -> String {
        let ns = self.namespace().unwrap_or_default();
        match self {
            Selection::Flux(item) => format!(
                "flux:{}:{}:{}:{ns}:{}",
                item.cluster, item.resource_kind.api_version, item.resource_kind.plural, item.name
            ),
            Selection::App(item) => format!("argoApp:{}:{ns}:{}", item.cluster, item.name),
            Selection::AppSet(item) => format!("argoAppSet:{}:{ns}:{}", item.cluster, item.name),
            Selection::Project(item) => {
                format!("argoProject:{}:{ns}:{}", item.cluster, item.name)
            }
        }
    }

    /// `gitOpsSelectionPrimaryAction(selection) === "openResources"`.
    pub fn opens_resources(self) -> bool {
        matches!(self, Selection::App(_))
    }

    /// `gitOpsCardSubtitle`.
    pub fn subtitle(self) -> &'a str {
        match self {
            Selection::Flux(item) => &item.resource_kind.kind,
            Selection::Project(item) => item.description.as_deref().unwrap_or("AppProject"),
            Selection::App(item) => item.project.as_deref().unwrap_or("default"),
            Selection::AppSet(item) => item.project.as_deref().unwrap_or("default"),
        }
    }

    /// `gitOpsSelectionSourceMode`; `None` for AppProjects.
    pub fn source_mode(self) -> Option<SourceMode> {
        Some(match self {
            Selection::Project(_) => return None,
            Selection::App(item) => match item.source_mode.as_deref() {
                Some("git") => SourceMode::Git,
                Some("helm") => SourceMode::Helm,
                Some("multi") => SourceMode::Multi,
                Some("plugin") => SourceMode::Plugin,
                Some(_) => SourceMode::Unknown,
                None if present(item.source_repo.as_deref()).is_some() => SourceMode::Git,
                None => SourceMode::Unknown,
            },
            Selection::AppSet(item) if present(item.source_repo.as_deref()).is_some() => {
                SourceMode::Git
            }
            Selection::AppSet(_) => SourceMode::Unknown,
            Selection::Flux(item) => {
                let kind = item
                    .source_kind
                    .as_deref()
                    .unwrap_or(&item.resource_kind.kind);
                if kind.contains("Helm") {
                    SourceMode::Helm
                } else if kind.contains("Git") || item.resource_kind.kind == "Kustomization" {
                    SourceMode::Git
                } else {
                    SourceMode::Unknown
                }
            }
        })
    }

    /// `gitOpsSelectionSourceLine`.
    pub fn source_line(self) -> Option<String> {
        match self {
            Selection::App(item) => app_source_line(item),
            Selection::AppSet(item) => item.source_repo.clone(),
            Selection::Project(item) => item.description.clone(),
            Selection::Flux(item) => match (
                present(item.source_kind.as_deref()),
                present(item.source_name.as_deref()),
            ) {
                (Some(kind), Some(name)) => Some(format!("{kind}/{name}")),
                _ => item.message.clone(),
            },
        }
        .filter(|line| !line.is_empty())
    }

    /// `gitOpsSelectionRevisionLabel`.
    pub fn revision_label(self) -> Option<String> {
        match self {
            Selection::App(item) => {
                let rows = app_revisions(item);
                match rows.as_slice() {
                    [] => item.source_revision.clone(),
                    [(target, resolved)] => target
                        .clone()
                        .or_else(|| resolved.as_deref().map(|sha| short_sha(sha).to_owned())),
                    rows => Some(format!("{} revisions", rows.len())),
                }
            }
            Selection::AppSet(item) => item.source_revision.clone(),
            Selection::Flux(item) => item.last_applied_revision.clone(),
            Selection::Project(_) => None,
        }
    }

    /// `gitOpsCardBadges`: raw provider statuses, as `(label, value)`. The web's
    /// "Last request" badge tracks connected operations, which this client does not run.
    pub fn badges(self) -> Vec<(&'static str, &'a str)> {
        let pairs: [(&'static str, Option<&'a str>); 3] = match self {
            Selection::Flux(item) => [
                ("Ready", item.ready_status.as_deref()),
                ("", None),
                ("", None),
            ],
            Selection::Project(item) => {
                [("Status", item.status.as_deref()), ("", None), ("", None)]
            }
            Selection::App(item) => [
                (
                    "Operation",
                    if present(item.refresh_requested.as_deref()).is_some() {
                        Some("Refreshing")
                    } else {
                        item.operation_phase.as_deref()
                    },
                ),
                ("Sync", item.sync_status.as_deref()),
                ("Health", item.health_status.as_deref()),
            ],
            Selection::AppSet(item) => [
                ("Sync", item.sync_status.as_deref()),
                ("Health", item.health_status.as_deref()),
                ("", None),
            ],
        };
        pairs
            .into_iter()
            .filter_map(|(label, value)| present(value).map(|value| (label, value)))
            .collect()
    }

    /// `gitOpsSelectionDestination`.
    pub fn destination(self) -> &'a str {
        let value = match self {
            Selection::Flux(_) | Selection::Project(_) => self.namespace(),
            Selection::App(item) => item
                .destination_namespace
                .as_deref()
                .or(item.destination_server.as_deref()),
            Selection::AppSet(item) => item
                .destination_namespace
                .as_deref()
                .or(item.destination_server.as_deref()),
        };
        value.unwrap_or("-")
    }

    /// `gitOpsCardFacts` after `compactFacts` drops empty values, as `(label, value)`.
    pub fn facts(self) -> Vec<(&'static str, String)> {
        let ns = || Some(self.namespace().unwrap_or("-").to_owned());
        let age = Some(self.age().to_owned());
        let facts: [(&'static str, Option<String>); 4] = match self {
            Selection::Flux(item) => [
                ("Revision", self.revision_label()),
                ("Namespace", ns()),
                (
                    "Inventory",
                    Some(format!(
                        "{}{}",
                        item.inventory.len(),
                        if item.inventory_partial { "+" } else { "" }
                    )),
                ),
                ("Age", age),
            ],
            Selection::Project(item) => [
                ("Namespace", ns()),
                (
                    "Description",
                    Some(item.description.clone().unwrap_or_else(|| "-".into())),
                ),
                ("Age", age),
                ("", None),
            ],
            _ => [
                ("Revision", self.revision_label()),
                ("Destination", Some(self.destination().to_owned())),
                ("Namespace", ns()),
                ("Age", age),
            ],
        };
        facts
            .into_iter()
            .filter_map(|(label, value)| {
                value
                    .filter(|value| !value.is_empty())
                    .map(|value| (label, value))
            })
            .collect()
    }

    /// The resource kind `gitOpsSelectionResource` inspects.
    pub fn kind(self) -> &'a str {
        match self {
            Selection::App(_) => "Application",
            Selection::AppSet(_) => "ApplicationSet",
            Selection::Project(_) => "AppProject",
            Selection::Flux(item) => &item.resource_kind.kind,
        }
    }

    /// `gitOpsDetailsActionLabel`.
    pub fn details_label(self) -> String {
        let scope = self
            .namespace()
            .map_or_else(|| "at cluster scope".to_owned(), |ns| format!("in {ns}"));
        format!("Open details for {} {} {scope}", self.kind(), self.name())
    }

    /// `gitOpsCardActionLabel`.
    pub fn action_label(self) -> String {
        let verb = if self.opens_resources() {
            "View resources for"
        } else {
            "Open details for"
        };
        format!("{verb} {}", self.name())
    }

    /// `gitOpsSelectionResource`: the dynamic resource the Details button inspects.
    pub fn resource(self) -> ResourceSummary {
        let (plural, status, health) = match self {
            Selection::Flux(item) => (
                item.resource_kind.plural.as_str(),
                item.ready_status.clone(),
                item.ready_status.clone(),
            ),
            Selection::App(item) => {
                let status = item.health_status.clone().or(item.sync_status.clone());
                ("applications", status.clone(), status)
            }
            Selection::AppSet(item) => (
                "applicationsets",
                item.status
                    .clone()
                    .or(item.health_status.clone())
                    .or(item.sync_status.clone()),
                item.health_status
                    .clone()
                    .or(item.sync_status.clone())
                    .or(item.status.clone()),
            ),
            Selection::Project(item) => ("appprojects", item.status.clone(), item.status.clone()),
        };
        let (api_version, group, version, namespaced) = match self {
            Selection::Flux(item) => {
                let kind = &item.resource_kind;
                (
                    kind.api_version.as_str(),
                    kind.group.as_str(),
                    kind.version.as_str(),
                    kind.namespaced,
                )
            }
            _ => ("argoproj.io/v1alpha1", "argoproj.io", "v1alpha1", true),
        };
        let provider = if matches!(self, Selection::Flux(_)) {
            "flux"
        } else {
            "argo"
        };
        ResourceSummary {
            kind: self.kind().to_owned(),
            cluster: self.cluster().to_owned(),
            name: self.name().to_owned(),
            namespace: self.namespace().map(str::to_owned),
            age: self.age().to_owned(),
            api_version: Some(api_version.to_owned()),
            group: Some(group.to_owned()),
            version: Some(version.to_owned()),
            plural: Some(plural.to_owned()),
            namespaced: Some(namespaced),
            dynamic: Some(true),
            health: gitops_health(health.as_deref()),
            health_assessment: self.health().clone(),
            created_at: self.created_at().cloned(),
            ready: status.clone(),
            status,
            restarts: None,
            owner_ref: None,
            uid: None,
            controller_owner: None,
            argo_app: None,
            helm_release: None,
            git_ops_ownership_partial: false,
            git_ops_owner: Some(GitOpsOwnerSummary {
                provider: provider.to_owned(),
                kind: self.kind().to_owned(),
                name: self.name().to_owned(),
                namespace: self.namespace().map(str::to_owned),
                confidence: "metadata".to_owned(),
                provenance: "metadata".to_owned(),
                partial: true,
            }),
        }
    }
}

/// `gitOpsHealth`.
fn gitops_health(status: Option<&str>) -> ResourceHealth {
    match status.unwrap_or_default() {
        "Synced" | "Healthy" | "True" | "Active" => ResourceHealth::Healthy,
        "Degraded" | "Missing" | "False" => ResourceHealth::Degraded,
        "OutOfSync" | "Progressing" | "Unknown" => ResourceHealth::Attention,
        _ => ResourceHealth::Unknown,
    }
}

/// `argoRevisionTooltipRows` as unique `(target, resolved)` pairs; a pair is
/// unique by its full resolved revision, as in the web.
fn app_revisions(app: &ArgoApplicationSummary) -> Vec<(Option<String>, Option<String>)> {
    if app.sources.is_empty() {
        return app
            .source_revision
            .iter()
            .filter(|revision| !revision.is_empty())
            .map(|revision| (Some(revision.clone()), None))
            .collect();
    }
    let mut rows: Vec<(Option<String>, Option<String>)> = Vec::new();
    for source in &app.sources {
        let target = present(source.target_revision.as_deref()).map(str::to_owned);
        let resolved = present(source.resolved_revision.as_deref()).map(str::to_owned);
        if target.is_none() && resolved.is_none() {
            continue;
        }
        if !rows.iter().any(|row| row.0 == target && row.1 == resolved) {
            rows.push((target, resolved));
        }
    }
    rows
}

/// `shortGitSha`: hex revisions of eight or more characters keep seven.
pub fn short_sha(revision: &str) -> &str {
    if revision.len() >= 8 && revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        &revision[..7]
    } else {
        revision
    }
}

/// `argoApplicationSourceLine`.
fn app_source_line(app: &ArgoApplicationSummary) -> Option<String> {
    match app.sources.as_slice() {
        [] => app.source_repo.clone(),
        [source] => source_line(source),
        sources => {
            let mut names: Vec<String> = Vec::new();
            for name in sources.iter().filter_map(compact_name) {
                if !names.contains(&name) {
                    names.push(name);
                }
            }
            if names.is_empty() {
                return Some(format!("{} sources", sources.len()));
            }
            let visible = names[..names.len().min(2)].join(" · ");
            let more = if names.len() > 2 { " · ..." } else { "" };
            Some(format!("{} sources · {visible}{more}", sources.len()))
        }
    }
}

/// `argoSourceLine`.
fn source_line(source: &ArgoApplicationSourceSummary) -> Option<String> {
    let (chart, repo, path) = (
        present(source.chart.as_deref()),
        present(source.repo_url.as_deref()),
        present(source.path.as_deref()),
    );
    match (chart, repo, path) {
        (Some(chart), Some(repo), _) => Some(format!("{chart} · {repo}")),
        (_, Some(repo), Some(path)) => Some(format!("{repo}/{path}")),
        _ => repo
            .or(chart)
            .or(path)
            .or(source.reference.as_deref())
            .map(str::to_owned),
    }
}

/// `argoSourceCompactName`.
fn compact_name(source: &ArgoApplicationSourceSummary) -> Option<String> {
    present(source.chart.as_deref())
        .or(present(source.path.as_deref()))
        .map(str::to_owned)
        .or_else(|| {
            present(source.repo_url.as_deref()).map(|repo| {
                let trimmed = repo.strip_suffix('/').unwrap_or(repo);
                let last = trimmed.rsplit('/').next().unwrap_or(trimmed);
                last.strip_suffix(".git").unwrap_or(last).to_owned()
            })
        })
        .or_else(|| source.reference.clone())
}
