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
use std::collections::HashMap;

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

/// `sortIncidentItems`: state, severity, recency, then namespace, kind and
/// name. `a_time` and `b_time` are the items' `latest_signal_time`.
fn compare(a: &Item, a_time: i64, b: &Item, b_time: i64) -> Ordering {
    state_weight(b.state)
        .cmp(&state_weight(a.state))
        .then(severity_weight(b.severity).cmp(&severity_weight(a.severity)))
        .then(b_time.cmp(&a_time))
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Empty {
    Clean,
    Filtered,
    Ready,
}

/// A line of the signal queue: a group heading or one of its items.
#[derive(Clone, Copy)]
pub enum Row {
    /// An index into `Queue::labels`.
    Heading(usize),
    /// An index into the cockpit items.
    Item(usize),
}

/// `buildIncidentSurfaceState` without the selection: the items are sorted
/// and grouped once per load, and `rows` lists every visible group's heading
/// followed by its items for the current filter.
pub struct Queue {
    pub counts: Counts,
    /// Ownership group labels.
    pub labels: Vec<String>,
    /// Item indices in `sortIncidentItems` order.
    order: Vec<usize>,
    /// Each item's index into `labels`.
    group: Vec<usize>,
    pub rows: Vec<Row>,
    /// The number of items that pass the filter.
    pub visible: usize,
    pub empty: Empty,
}

impl Queue {
    /// Sorts and groups `items` once; `filter` then only selects from them.
    pub fn new(items: &[Item], filter: Filter) -> Self {
        let times: Vec<i64> = items.iter().map(latest_signal_time).collect();
        let mut order: Vec<usize> = (0..items.len()).collect();
        order.sort_by(|&a, &b| compare(&items[a], times[a], &items[b], times[b]));
        let mut labels: Vec<String> = Vec::new();
        let mut label_of: HashMap<String, usize> = HashMap::new();
        let group = items
            .iter()
            .map(|item| {
                let label = ownership_group(&item.resource, false).0;
                *label_of.entry(label).or_insert_with_key(|label| {
                    labels.push(label.clone());
                    labels.len() - 1
                })
            })
            .collect();
        let mut queue = Self {
            counts: counts(items),
            labels,
            order,
            group,
            rows: Vec::new(),
            visible: 0,
            empty: Empty::Clean,
        };
        queue.filter(items, filter);
        queue
    }

    /// Rebuilds the rows for `filter` from the sorted items. A filter keeps
    /// the sort order, so groups still follow their first visible item.
    pub fn filter(&mut self, items: &[Item], filter: Filter) {
        let mut members: Vec<Vec<usize>> = vec![Vec::new(); self.labels.len()];
        let mut groups: Vec<usize> = Vec::new();
        let mut visible = 0;
        for &item in &self.order {
            if !matches(&items[item], filter) {
                continue;
            }
            let group = self.group[item];
            if members[group].is_empty() {
                groups.push(group);
            }
            members[group].push(item);
            visible += 1;
        }
        self.rows.clear();
        self.rows.reserve(groups.len() + visible);
        for group in groups {
            self.rows.push(Row::Heading(group));
            self.rows
                .extend(members[group].iter().map(|&item| Row::Item(item)));
        }
        self.visible = visible;
        self.empty = if items.is_empty() {
            Empty::Clean
        } else if visible == 0 {
            Empty::Filtered
        } else {
            Empty::Ready
        };
    }

    /// The item for `selected_key`, falling back to the first visible item.
    pub fn selected<'a>(&self, items: &'a [Item], selected_key: Option<&str>) -> Option<&'a Item> {
        let mut visible = self.rows.iter().filter_map(|row| match row {
            Row::Item(item) => Some(&items[*item]),
            Row::Heading(_) => None,
        });
        let first = visible.clone().next();
        selected_key
            .and_then(|key| visible.find(|item| is_key(&item.resource, key)))
            .or(first)
    }
}

/// `resource_key(resource) == key` without building the key.
pub fn is_key(resource: &ResourceSummary, key: &str) -> bool {
    let parts = [
        resource.cluster.as_str(),
        resource.api_version.as_deref().unwrap_or(""),
        resource.kind.as_str(),
        resource.namespace.as_deref().unwrap_or(""),
    ];
    let mut rest = key;
    for part in parts {
        match rest
            .strip_prefix(part)
            .and_then(|rest| rest.strip_prefix(':'))
        {
            Some(next) => rest = next,
            None => return false,
        }
    }
    rest == resource.name
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
