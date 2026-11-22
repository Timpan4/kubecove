//! Guided investigation, ported from `src/features/incidents/guidance.ts` and
//! `incident-actions.ts`. Actions are handoffs only: the native client lists
//! them but runs nothing (ADR 0004).

use super::{case_summary, case_title, resource_label, signals};
use kubecove_lib::models::{
    IncidentCockpitItem as Item, IncidentSeverity as Severity, IncidentSignalState as SignalState,
    ResourceDetailsFull, ResourceSummary, ResourceTopology, TopologyRelation,
};
use std::collections::{HashMap, HashSet, VecDeque};

/// `IncidentEnrichmentState`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Enrichment {
    Idle,
    Loading,
    Ready,
    Error,
}

/// `IncidentEvidenceTone`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    Error,
    Warning,
    Info,
    Neutral,
}

pub struct Evidence {
    pub id: String,
    pub label: String,
    pub detail: String,
    pub timestamp: Option<String>,
    pub tone: Tone,
}

pub struct Fact {
    pub label: &'static str,
    pub value: String,
    pub detail: String,
}

pub struct Step {
    pub title: String,
    pub description: &'static str,
    pub target: ResourceSummary,
    /// The detail tab the web opens: `logs`, `events`, `details` or `yaml`.
    pub tab: &'static str,
}

pub struct Action {
    pub label: String,
    pub description: String,
    pub target: ResourceSummary,
    pub destructive: bool,
}

pub struct Guidance {
    pub title: String,
    pub summary: String,
    pub facts: Vec<Fact>,
    pub evidence: Vec<Evidence>,
    pub missing: Vec<&'static str>,
    pub steps: Vec<Step>,
    pub actions: Vec<Action>,
}

/// `IncidentOwnerResolution`.
#[derive(Default)]
pub struct Owners {
    pub direct: Option<ResourceSummary>,
    pub workload: Option<ResourceSummary>,
    pub subject_found: bool,
    pub complete: bool,
}

const WORKLOAD_KINDS: &[&str] = &["Deployment", "StatefulSet", "DaemonSet"];

/// `isBuiltinOperationTarget` in `resource-actions-model.ts`.
fn builtin_target(resource: &ResourceSummary) -> bool {
    let builtin = match resource.kind.as_str() {
        "Deployment" | "StatefulSet" | "DaemonSet" => Some("apps/v1"),
        "Pod" | "ConfigMap" => Some("v1"),
        _ => None,
    };
    resource.dynamic != Some(true)
        && resource
            .api_version
            .as_deref()
            .is_none_or(|version| Some(version) == builtin)
}

/// `looseResourceKey` in `src/lib/resource-identity.ts`.
fn loose_key(resource: &ResourceSummary) -> String {
    format!(
        "{}:{}:{}:{}",
        resource.cluster,
        resource.kind,
        resource.namespace.as_deref().unwrap_or(""),
        resource.name
    )
}

/// `resolveIncidentOwner`: breadth-first up `owns` edges to the first
/// built-in workload, remembering the direct owner.
pub fn resolve_owner(resource: &ResourceSummary, topology: Option<&ResourceTopology>) -> Owners {
    let Some(topology) = topology else {
        return Owners::default();
    };
    // `findResourceIndex`: an exact identity wins over a loose one.
    let exact = super::resource_key(resource);
    let loose = loose_key(resource);
    let Some(subject) = topology
        .nodes
        .iter()
        .find(|node| super::resource_key(&node.summary) == exact)
        .or_else(|| {
            topology.nodes.iter().find(|node| {
                (node.summary.api_version.is_none() || resource.api_version.is_none())
                    && loose_key(&node.summary) == loose
            })
        })
    else {
        return Owners::default();
    };
    let nodes: HashMap<&str, _> = topology
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let owns = |relation: &TopologyRelation| *relation == TopologyRelation::Owns;
    let complete = topology.warnings.is_empty()
        && topology.edges.iter().all(|edge| {
            !owns(&edge.relation)
                || (nodes.contains_key(edge.source.as_str())
                    && nodes.contains_key(edge.target.as_str()))
        });
    let mut incoming: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in topology.edges.iter().filter(|edge| owns(&edge.relation)) {
        incoming
            .entry(edge.target.as_str())
            .or_default()
            .push(edge.source.as_str());
    }
    let mut queue = VecDeque::from([subject.id.as_str()]);
    let mut visited = HashSet::from([subject.id.as_str()]);
    let mut direct: Option<ResourceSummary> = None;
    while let Some(current) = queue.pop_front() {
        let mut owners: Vec<_> = incoming
            .get(current)
            .into_iter()
            .flatten()
            .filter_map(|id| nodes.get(id))
            .collect();
        owners.sort_by_key(|node| loose_key(&node.summary));
        for owner in owners {
            if !visited.insert(owner.id.as_str()) {
                continue;
            }
            direct.get_or_insert_with(|| owner.summary.clone());
            if WORKLOAD_KINDS.contains(&owner.summary.kind.as_str())
                && builtin_target(&owner.summary)
            {
                return Owners {
                    direct,
                    workload: Some(owner.summary.clone()),
                    subject_found: true,
                    complete,
                };
            }
            queue.push_back(owner.id.as_str());
        }
    }
    Owners {
        direct,
        workload: None,
        subject_found: true,
        complete,
    }
}

fn workload_actions(resource: &ResourceSummary, owner: bool) -> Vec<Action> {
    if !builtin_target(resource) {
        return vec![];
    }
    let subject = if owner {
        format!("owning {}", resource.kind)
    } else {
        format!("this {}", resource.kind)
    };
    let label = resource_label(resource);
    let action = |verb: &str, what: &str| Action {
        label: format!("{verb} {subject}"),
        description: format!("Open guarded {what} for {label}."),
        target: resource.clone(),
        destructive: false,
    };
    match resource.kind.as_str() {
        "Deployment" | "StatefulSet" => vec![
            action("Restart", "rollout restart"),
            action("Scale", "replica controls"),
        ],
        "DaemonSet" => vec![action("Restart", "rollout restart")],
        _ => vec![],
    }
}

/// `buildIncidentAvailableActions`.
fn actions(item: &Item, owners: &Owners, topology: Enrichment) -> Vec<Action> {
    if item.state != SignalState::Active {
        return vec![];
    }
    if item.resource.kind != "Pod" {
        return workload_actions(&item.resource, false);
    }
    let confirmed = topology == Enrichment::Ready && owners.complete;
    let workload = owners.workload.as_ref().filter(|_| confirmed);
    let direct = owners.direct.as_ref().filter(|_| confirmed);
    let note = match (workload, direct) {
        (Some(workload), _) => format!(
            "{} should replace the deleted Pod.",
            resource_label(workload)
        ),
        (None, None) if confirmed && owners.subject_found => {
            "No controller owner was resolved; it will not be recreated automatically.".into()
        }
        (None, Some(direct)) => format!(
            "{} is not a supported restart or scale target; automatic replacement is not confirmed.",
            resource_label(direct)
        ),
        (None, None) => {
            "Controller ownership is unavailable; automatic replacement is not confirmed.".into()
        }
    };
    let mut actions = vec![Action {
        label: if workload.is_some() {
            "Recreate this Pod"
        } else {
            "Delete this Pod"
        }
        .into(),
        description: format!(
            "Open guarded delete preview for {}. {note}",
            resource_label(&item.resource)
        ),
        target: item.resource.clone(),
        destructive: true,
    }];
    if let Some(workload) = workload {
        actions.extend(workload_actions(workload, true));
    }
    actions
}

fn snapshot_evidence(item: &Item) -> Vec<Evidence> {
    let mut evidence: Vec<Evidence> = item
        .signals
        .iter()
        .enumerate()
        .map(|(index, signal)| Evidence {
            id: format!("snapshot:{index}:{}", signal.kind),
            label: signal.label.clone(),
            detail: if signal.message.is_empty() {
                signal.source.clone()
            } else {
                signal.message.clone()
            },
            timestamp: signal.last_seen_at.clone(),
            tone: match signal.state {
                SignalState::Historical => Tone::Neutral,
                SignalState::Resolved => Tone::Info,
                SignalState::Active if item.severity == Severity::Degraded => Tone::Error,
                SignalState::Active => Tone::Warning,
            },
        })
        .collect();
    if let Some(event) = &item.latest_warning_event {
        let same = |a: &str, b: &str| a.trim().to_lowercase() == b.trim().to_lowercase();
        if !evidence
            .iter()
            .any(|entry| same(&entry.label, &event.reason) && same(&entry.detail, &event.message))
        {
            evidence.push(Evidence {
                id: "snapshot:warning-event".into(),
                label: event.reason.clone(),
                detail: event.message.clone(),
                timestamp: event.last_seen_at.clone(),
                tone: Tone::Warning,
            });
        }
    }
    evidence
}

fn detailed_evidence(details: Option<&ResourceDetailsFull>) -> Vec<Evidence> {
    let Some(details) = details else {
        return vec![];
    };
    signals::detail_signals(&details.summary, details.status.as_ref())
        .into_iter()
        .map(|signal| Evidence {
            id: format!("detail:{}", signal.id),
            label: signal.label,
            detail: signal.value,
            timestamp: signal.timestamp,
            tone: signal.tone,
        })
        .collect()
}

/// `mergeEvidence`: snapshot restarts first, then detail, then snapshot; at most six.
fn merge(detail: Vec<Evidence>, snapshot: Vec<Evidence>) -> Vec<Evidence> {
    let (restarts, rest): (Vec<_>, Vec<_>) = snapshot
        .into_iter()
        .partition(|entry| entry.id.ends_with(":restart"));
    let mut seen = HashSet::new();
    restarts
        .into_iter()
        .chain(detail)
        .chain(rest)
        .filter(|entry| {
            seen.insert(format!(
                "{}\0{}\0{}",
                entry.label.trim().to_lowercase(),
                entry.detail.trim().to_lowercase(),
                entry.timestamp.as_deref().unwrap_or("")
            ))
        })
        .take(6)
        .collect()
}

fn steps(item: &Item, owner: Option<&ResourceSummary>) -> Vec<Step> {
    let step = |title: &str, description, target: &ResourceSummary, tab| Step {
        title: title.to_owned(),
        description,
        target: target.clone(),
        tab,
    };
    let resource = &item.resource;
    let mut steps = Vec::new();
    if resource.kind == "Pod" && resource.namespace.is_some() {
        steps.push(step(
            "Read container logs",
            "Look for the application error before changing the Pod or its owner.",
            resource,
            "logs",
        ));
    }
    steps.push(step(
        "Review events",
        "Check scheduling, kubelet, admission, and repeated warning evidence.",
        resource,
        "events",
    ));
    steps.push(step(
        "Inspect resource details",
        "Review conditions, container state, metadata, and lifecycle fields.",
        resource,
        "details",
    ));
    if let Some(owner) = owner {
        steps.push(step(
            &format!("Inspect owning {}", owner.kind),
            "Compare owner health and sibling replicas before changing workload state.",
            owner,
            "details",
        ));
    }
    steps.push(step(
        "Inspect YAML",
        "Confirm desired configuration and controller references.",
        resource,
        "yaml",
    ));
    steps
}

/// `buildIncidentGuidance`.
pub fn build(
    item: &Item,
    details: Option<&ResourceDetailsFull>,
    owners: &Owners,
    details_state: Enrichment,
    topology_state: Enrichment,
) -> Guidance {
    let resource = &item.resource;
    let pod = resource.kind == "Pod";
    let confirmed = topology_state == Enrichment::Ready && owners.complete;
    let owner = confirmed
        .then(|| owners.workload.as_ref().or(owners.direct.as_ref()))
        .flatten();
    let evidence = merge(detailed_evidence(details), snapshot_evidence(item));
    let title = evidence.first().map_or_else(
        || case_title(item),
        |first| format!("{}: {}", first.label, first.detail),
    );
    let owner_summary = match owner {
        _ if !pod => String::new(),
        Some(owner) => format!(" Controller owner resolves to {}.", resource_label(owner)),
        None => " Controller ownership is not confirmed.".into(),
    };
    let mut missing = vec!["Root cause is not confirmed by current evidence."];
    match details_state {
        Enrichment::Loading => {
            missing.push("Live conditions and container state are still loading.")
        }
        Enrichment::Error => {
            missing.push("Live conditions and container state could not be loaded.")
        }
        _ => {}
    }
    match topology_state {
        Enrichment::Loading if pod => missing.push("Controller ownership is still loading."),
        Enrichment::Error if pod => {
            missing.push("Controller ownership could not be loaded; owner actions are unavailable.")
        }
        _ => {}
    }
    if item.latest_warning_event.is_none() {
        missing.push("No Warning event is present in the incident snapshot.");
    }
    let active = item.state == SignalState::Active;
    let facts = vec![
        Fact {
            label: "Impact",
            value: if active {
                resource_label(resource)
            } else {
                "No active impact".into()
            },
            detail: match item.state {
                SignalState::Active => "Current signal is scoped to this exact resource.".into(),
                SignalState::Resolved => {
                    "Resolved evidence is retained for this exact resource.".into()
                }
                SignalState::Historical => {
                    "Historical evidence is retained for this exact resource.".into()
                }
            },
        },
        Fact {
            label: "Confidence",
            value: if details.is_some() {
                "Live detail evidence"
            } else {
                "Incident snapshot"
            }
            .into(),
            detail: if details.is_some() {
                "Conditions and container state were loaded."
            } else {
                "No live detail claim is inferred."
            }
            .into(),
        },
        Fact {
            label: "Scope",
            value: resource
                .namespace
                .clone()
                .unwrap_or_else(|| "Cluster scoped".into()),
            detail: match owner {
                Some(owner) => format!("{} · owner {}", resource.cluster, resource_label(owner)),
                None => resource.cluster.clone(),
            },
        },
    ];
    Guidance {
        title,
        summary: format!("{}{owner_summary}", case_summary(item)),
        facts,
        evidence,
        missing,
        steps: steps(item, owner),
        actions: actions(item, owners, topology_state),
    }
}
