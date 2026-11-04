//! Incident queue model, ported from `src/features/incidents/model.ts`.

pub mod guidance;
mod signals;

use crate::collate::collate;
use crate::model::{WORKSPACE_KINDS, ownership_group};
use kubecove_lib::models::{
    HealthAssessmentSource, IncidentCockpitItem as Item, IncidentSeverity as Severity,
    IncidentSignalState as SignalState, ResourceListRequest, ResourceSummary,
};
use std::cmp::Ordering;

/// `buildWorkspaceFetchPlans` for the default workspace: every kind in every namespace.
pub fn workspace_requests() -> Vec<ResourceListRequest> {
    WORKSPACE_KINDS
        .iter()
        .map(|kind| ResourceListRequest {
            kind: Some((*kind).to_owned()),
            resource_kind: None,
            namespace: None,
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Filter {
    All,
    Degraded,
    Attention,
    Restarted,
    Warning,
}

#[derive(Default, Clone, Copy)]
pub struct Counts {
    pub total: usize,
    pub degraded: usize,
    pub attention: usize,
    pub restarted: usize,
    pub warning: usize,
}

fn matches(item: &Item, filter: Filter) -> bool {
    let evidence = |source: HealthAssessmentSource| {
        item.resource
            .health_assessment
            .evidence
            .iter()
            .any(|evidence| evidence.source == source)
    };
    let signal = |kind: &str| item.signals.iter().any(|signal| signal.kind == kind);
    match filter {
        Filter::All => true,
        Filter::Degraded => item.severity == Severity::Degraded,
        Filter::Attention => item.severity == Severity::Attention,
        Filter::Restarted => {
            item.severity == Severity::Restarted
                || signal("restart")
                || evidence(HealthAssessmentSource::ContainerRestart)
        }
        Filter::Warning => {
            item.severity == Severity::Warning
                || item.warning_event_count > 0
                || signal("event")
                || evidence(HealthAssessmentSource::WarningEvent)
        }
    }
}

/// `countIncidentItems`.
pub fn counts(items: &[Item]) -> Counts {
    let mut counts = Counts::default();
    for item in items {
        counts.total += 1;
        counts.degraded += usize::from(item.severity == Severity::Degraded);
        counts.attention += usize::from(item.severity == Severity::Attention);
        counts.restarted += usize::from(matches(item, Filter::Restarted));
        counts.warning += usize::from(matches(item, Filter::Warning));
    }
    counts
}

/// `buildIncidentFilterOptions`.
pub fn filter_options(counts: Counts) -> [(Filter, &'static str, usize); 5] {
    [
        (Filter::All, "All", counts.total),
        (Filter::Degraded, "Degraded", counts.degraded),
        (Filter::Attention, "Needs attention", counts.attention),
        (Filter::Restarted, "Restart evidence", counts.restarted),
        (Filter::Warning, "Warnings", counts.warning),
    ]
}

fn state_weight(state: SignalState) -> u8 {
    match state {
        SignalState::Active => 3,
        SignalState::Resolved => 2,
        SignalState::Historical => 1,
    }
}

fn severity_weight(severity: Severity) -> u8 {
    match severity {
        Severity::Degraded => 4,
        Severity::Attention => 3,
        Severity::Restarted => 2,
        Severity::Warning => 1,
    }
}

/// `latestSignalTime`: milliseconds, or 0 when absent or unparsable.
fn latest_signal_time(item: &Item) -> i64 {
    item.latest_signal_at
        .as_deref()
        .or_else(|| {
            item.latest_warning_event
                .as_ref()
                .and_then(|event| event.last_seen_at.as_deref())
        })
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map_or(0, |time| time.timestamp_millis())
}

/// `sortIncidentItems`: state, severity, recency, then namespace, kind and name.
fn compare(a: &Item, b: &Item) -> Ordering {
    state_weight(b.state)
        .cmp(&state_weight(a.state))
        .then(severity_weight(b.severity).cmp(&severity_weight(a.severity)))
        .then(latest_signal_time(b).cmp(&latest_signal_time(a)))
        .then_with(|| {
            collate(
                a.resource.namespace.as_deref().unwrap_or(""),
                b.resource.namespace.as_deref().unwrap_or(""),
                false,
            )
        })
        .then_with(|| collate(&a.resource.kind, &b.resource.kind, false))
        .then_with(|| collate(&a.resource.name, &b.resource.name, false))
}

pub struct Group<'a> {
    pub label: String,
    pub items: Vec<&'a Item>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Empty {
    Clean,
    Filtered,
    Ready,
}

pub struct SurfaceState<'a> {
    pub groups: Vec<Group<'a>>,
    pub visible: usize,
    pub selected: Option<&'a Item>,
    pub empty: Empty,
}

/// `buildIncidentSurfaceState`: the selection falls back to the first visible item.
pub fn surface_state<'a>(
    items: &'a [Item],
    filter: Filter,
    selected_key: Option<&str>,
) -> SurfaceState<'a> {
    let counts = counts(items);
    let mut visible: Vec<&Item> = items.iter().filter(|item| matches(item, filter)).collect();
    visible.sort_by(|a, b| compare(a, b));
    let selected = selected_key
        .and_then(|key| visible.iter().copied().find(|item| item_key(item) == key))
        .or_else(|| visible.first().copied());
    let mut groups: Vec<Group> = Vec::new();
    for item in &visible {
        let label = ownership_group(&item.resource).0;
        match groups.iter_mut().find(|group| group.label == label) {
            Some(group) => group.items.push(item),
            None => groups.push(Group {
                label,
                items: vec![item],
            }),
        }
    }
    let empty = if counts.total == 0 {
        Empty::Clean
    } else if groups.is_empty() {
        Empty::Filtered
    } else {
        Empty::Ready
    };
    SurfaceState {
        visible: visible.len(),
        groups,
        selected,
        empty,
    }
}

/// `resourceKey` in `src/lib/resource-identity.ts`.
pub fn resource_key(resource: &ResourceSummary) -> String {
    format!(
        "{}:{}:{}:{}:{}",
        resource.cluster,
        resource.api_version.as_deref().unwrap_or(""),
        resource.kind,
        resource.namespace.as_deref().unwrap_or(""),
        resource.name
    )
}

pub fn item_key(item: &Item) -> String {
    resource_key(&item.resource)
}

pub fn resource_label(resource: &ResourceSummary) -> String {
    format!("{}/{}", resource.kind, resource.name)
}

/// `incidentSeverityLabel`.
pub fn severity_label(item: &Item) -> &'static str {
    match (item.severity, item.state) {
        (Severity::Degraded, _) => "Degraded",
        (Severity::Attention, _) => "Needs attention",
        (Severity::Restarted, SignalState::Historical) => "Historical restart",
        (Severity::Restarted, SignalState::Resolved) => "Resolved restart",
        (Severity::Restarted, SignalState::Active) => "Active restart",
        (Severity::Warning, _) => "Warning",
    }
}

/// `incidentScopeLabel`.
pub fn scope_label(item: &Item) -> String {
    format!(
        "{} / {}",
        item.resource.namespace.as_deref().unwrap_or("cluster"),
        item.resource.cluster
    )
}

/// `incidentSignalSummary`: the first three signals, then a hidden count.
pub fn signal_summary(item: &Item) -> String {
    if item.signals.is_empty() {
        return item
            .latest_warning_event
            .as_ref()
            .map_or_else(|| "-".to_owned(), |event| event.message.clone());
    }
    let summary = item
        .signals
        .iter()
        .take(3)
        .map(|signal| {
            if signal.message.is_empty() {
                signal.label.clone()
            } else {
                format!("{}: {}", signal.label, signal.message)
            }
        })
        .collect::<Vec<_>>()
        .join(" | ");
    match item.signals.len().saturating_sub(3) {
        0 => summary,
        hidden => format!("{summary} | +{hidden} more"),
    }
}

/// `incidentCaseTitle`.
fn case_title(item: &Item) -> String {
    let signal = item
        .signals
        .first()
        .map(|signal| signal.label.clone())
        .or_else(|| {
            item.latest_warning_event
                .as_ref()
                .map(|event| event.reason.clone())
        })
        .unwrap_or_else(|| severity_label(item).to_owned());
    format!("{}: {signal}", resource_label(&item.resource))
}

/// `incidentCaseSummary`.
fn case_summary(item: &Item) -> String {
    if let Some(signal) = item.signals.first().filter(|s| !s.message.is_empty()) {
        return signal.message.clone();
    }
    match item.latest_warning_event.as_ref() {
        Some(event) if !event.message.is_empty() => event.message.clone(),
        _ => format!("{} signal in {}.", severity_label(item), scope_label(item)),
    }
}
