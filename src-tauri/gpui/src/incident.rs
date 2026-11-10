//! Incident signals and the Events tab model for the detail panel, ported from
//! `src/features/resource-detail/helpers.ts` (`buildIncidentSignals`),
//! `incident-events.ts` and `eventsTabModel.ts`. Container signals are not
//! ported: the native client does not parse container statuses yet.

use chrono::{DateTime, FixedOffset, Local};
use kubecove_lib::models::{ResourceEventSummary, ResourceSummary};
use serde_json::Value;
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignalTone {
    Neutral,
    Info,
    Success,
    Warning,
    Error,
}

/// `IncidentSignal`; `value` already has timestamps formatted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IncidentSignal {
    pub label: String,
    pub value: String,
    pub tone: SignalTone,
    pub source: &'static str,
}

/// `ConditionRow` from `getConditionRows`.
pub struct ConditionRow {
    pub kind: String,
    pub status: String,
    pub reason: Option<String>,
    pub message: Option<String>,
    pub last_transition: Option<String>,
}

pub fn condition_rows(status: Option<&Value>) -> Vec<ConditionRow> {
    let text = |value: &Value, key: &str| value[key].as_str().map(str::to_owned);
    let shown = |value: &Value, key: &str, fallback: &str| match &value[key] {
        Value::String(text) => text.clone(),
        Value::Null => fallback.to_owned(),
        other => other.to_string(),
    };
    status
        .and_then(|status| status["conditions"].as_array())
        .into_iter()
        .flatten()
        .filter(|condition| condition.is_object())
        .map(|condition| ConditionRow {
            kind: shown(condition, "type", "Condition"),
            status: shown(condition, "status", "Unknown"),
            reason: text(condition, "reason"),
            message: text(condition, "message"),
            last_transition: text(condition, "lastTransitionTime"),
        })
        .collect()
}

pub(crate) fn parse(value: Option<&str>) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(value?).ok()
}

pub(crate) fn millis(value: Option<&str>) -> Option<i64> {
    parse(value).map(|time| time.timestamp_millis())
}

/// `formatFullTimestamp`: local `YYYY-MM-DD HH:MM:SS.mmm`, else the raw value.
pub fn full_timestamp(value: &str) -> String {
    parse(Some(value)).map_or(value.to_owned(), |time| {
        time.with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S%.3f")
            .to_string()
    })
}

pub(crate) fn is_warning(event: &ResourceEventSummary) -> bool {
    event.event_type == "Warning"
}

/// `sortIncidentEvents`: warnings first, then newest, highest count, reason.
pub fn sort_events(events: &mut [ResourceEventSummary]) {
    let time = |event: &ResourceEventSummary| millis(event.last_seen_at.as_deref()).unwrap_or(0);
    events.sort_by(|a, b| {
        is_warning(b)
            .cmp(&is_warning(a))
            .then_with(|| time(b).cmp(&time(a)))
            .then_with(|| b.count.cmp(&a.count))
            .then_with(|| a.reason.cmp(&b.reason))
    });
}

fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// `eventWarningSummary`.
fn warning_summary(events: &[ResourceEventSummary]) -> Option<String> {
    let warnings: Vec<_> = events.iter().filter(|event| is_warning(event)).collect();
    if warnings.is_empty() {
        return None;
    }
    let mut reasons: Vec<&str> = warnings.iter().map(|event| event.reason.as_str()).collect();
    reasons.sort_unstable();
    reasons.dedup();
    let repeats: i64 = warnings.iter().map(|event| i64::from(event.count)).sum();
    let repeats = usize::try_from(repeats).unwrap_or(0);
    Some(format!(
        "{} · {}",
        plural(reasons.len(), "warning reason", "warning reasons"),
        plural(repeats, "repeat", "repeats")
    ))
}

fn lower(value: Option<&str>) -> String {
    value.unwrap_or_default().trim().to_lowercase()
}

pub(crate) fn succeeded_pod(resource: &ResourceSummary) -> bool {
    resource.kind == "Pod"
        && ["succeeded", "complete", "completed"]
            .contains(&lower(resource.status.as_deref()).as_str())
}

pub(crate) fn completed_readiness(condition: &ConditionRow) -> bool {
    (condition.reason.as_deref() == Some("PodCompleted")
        && ["Ready", "ContainersReady"].contains(&condition.kind.as_str()))
        || condition.kind == "PodReadyToStartContainers"
}

pub(crate) fn disruption_target(condition: &ConditionRow) -> bool {
    condition.kind == "DisruptionTarget" && condition.status == "True"
}

/// `resourceStatusTone`.
pub fn status_tone(status: Option<&str>) -> SignalTone {
    match lower(status).as_str() {
        "running" | "succeeded" | "complete" | "completed" | "ready" => SignalTone::Success,
        "failed" | "error" | "crashloopbackoff" | "imagepullbackoff" => SignalTone::Error,
        "pending" | "terminating" | "unknown" => SignalTone::Warning,
        _ => SignalTone::Neutral,
    }
}

/// `buildIncidentSignals` without container rows, which the web passes only for Pods.
pub fn incident_signals(
    resource: &ResourceSummary,
    conditions: &[ConditionRow],
    events: &[ResourceEventSummary],
) -> Vec<IncidentSignal> {
    let mut signals = Vec::new();
    if let Some(value) = warning_summary(events) {
        signals.push(IncidentSignal {
            label: "Warning events".into(),
            value,
            tone: SignalTone::Warning,
            source: "event",
        });
    }
    let restarts = resource.restarts.unwrap_or(0);
    if restarts > 0 {
        signals.push(IncidentSignal {
            label: "Restarts".into(),
            value: restarts.to_string(),
            tone: if restarts > 5 {
                SignalTone::Error
            } else {
                SignalTone::Warning
            },
            source: "status",
        });
    }
    let succeeded = succeeded_pod(resource);
    for condition in conditions {
        let disruption = disruption_target(condition);
        if !disruption
            && (condition.status == "True" || (succeeded && completed_readiness(condition)))
        {
            continue;
        }
        let mut value = format!("{}={}", condition.kind, condition.status);
        if let Some(reason) = &condition.reason {
            value.push_str(&format!(" · {reason}"));
        }
        if let Some(time) = &condition.last_transition {
            value.push_str(&format!(" · since {}", full_timestamp(time)));
        }
        signals.push(IncidentSignal {
            label: "Condition".into(),
            value,
            tone: match (disruption, condition.status.as_str()) {
                (true, _) => SignalTone::Info,
                (false, "False") => SignalTone::Error,
                _ => SignalTone::Warning,
            },
            source: "condition",
        });
    }
    let status = lower(resource.status.as_deref());
    if let Some(value) = &resource.status
        && [
            "failed",
            "error",
            "crashloopbackoff",
            "imagepullbackoff",
            "pending",
            "terminating",
            "unknown",
        ]
        .contains(&status.as_str())
    {
        signals.push(IncidentSignal {
            label: "Status".into(),
            value: value.clone(),
            tone: status_tone(Some(value)),
            source: "status",
        });
    }
    if let Some(ready) = &resource.ready
        && ready.to_lowercase() == "false"
        && !succeeded
    {
        signals.push(IncidentSignal {
            label: "Ready".into(),
            value: ready.clone(),
            tone: SignalTone::Error,
            source: "status",
        });
    }
    signals
}

/// `EventSummary` from `eventsTabModel.ts`.
pub struct EventSummary<'a> {
    pub total: usize,
    pub warnings: usize,
    pub sources: usize,
    pub namespace: String,
    pub latest: Option<&'a ResourceEventSummary>,
}

fn normalized(value: &str) -> String {
    value.trim().to_lowercase()
}

/// `buildEventSummary`: the first newest event, unique sources and namespaces.
pub fn event_summary(events: &[ResourceEventSummary]) -> EventSummary<'_> {
    let time = |event: &ResourceEventSummary| millis(event.last_seen_at.as_deref());
    let mut latest: Option<&ResourceEventSummary> = None;
    for event in events {
        if latest.is_none_or(|current| time(event).cmp(&time(current)) == Ordering::Greater) {
            latest = Some(event);
        }
    }
    fn unique(values: Vec<&str>) -> Vec<&str> {
        let mut values: Vec<&str> = values
            .into_iter()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .collect();
        values.sort_unstable();
        values.dedup();
        values
    }
    let sources = unique(events.iter().map(|event| event.source.as_str()).collect()).len();
    let namespaces = unique(
        events
            .iter()
            .filter_map(|event| event.namespace.as_deref())
            .collect(),
    );
    EventSummary {
        total: events.len(),
        warnings: events
            .iter()
            .filter(|event| normalized(&event.event_type) == "warning")
            .count(),
        sources,
        namespace: match namespaces.as_slice() {
            [] => "cluster".into(),
            [one] => (*one).to_owned(),
            many => format!("{} namespaces", many.len()),
        },
        latest,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventFilter {
    All,
    Warning,
    Normal,
}

/// `filterResourceEvents`.
pub fn filter_events<'a>(
    events: &'a [ResourceEventSummary],
    filter: EventFilter,
    query: &str,
) -> Vec<&'a ResourceEventSummary> {
    let query = normalized(query);
    events
        .iter()
        .filter(|event| match filter {
            EventFilter::All => true,
            EventFilter::Warning => normalized(&event.event_type) == "warning",
            EventFilter::Normal => normalized(&event.event_type) == "normal",
        })
        .filter(|event| {
            query.is_empty()
                || [
                    event.message.as_str(),
                    event.reason.as_str(),
                    event.source.as_str(),
                    event.event_type.as_str(),
                    event.namespace.as_deref().unwrap_or_default(),
                ]
                .into_iter()
                .any(|value| normalized(value).contains(&query))
        })
        .collect()
}
