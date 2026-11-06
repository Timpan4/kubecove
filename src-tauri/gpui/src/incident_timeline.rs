//! Incident timeline entries for the Details tab, ported from
//! `src/features/resource-detail/incident-timeline.ts` and
//! `incident-timeline-labels.ts`. Container and log entries are not ported:
//! the native client does not parse container statuses or read logs yet.

use crate::incident::{
    ConditionRow, SignalTone, completed_readiness, disruption_target, is_warning, millis, parse,
    succeeded_pod, time_of_day,
};
use chrono::Local;
use kubecove_lib::models::{ResourceEventSummary, ResourceSummary};

/// `IncidentTimelineItem`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimelineItem {
    pub id: String,
    pub source: &'static str,
    pub tone: SignalTone,
    pub title: String,
    pub detail: String,
    pub timestamp: Option<String>,
}

impl TimelineItem {
    /// `entryKey` in `ResourceTimeline.svelte`.
    pub fn key(&self) -> String {
        format!(
            "{}:{}",
            self.id,
            self.timestamp.as_deref().unwrap_or_default()
        )
    }
}

/// `buildIncidentTimeline` for conditions and warning events, oldest first.
pub fn incident_timeline(
    resource: &ResourceSummary,
    conditions: &[ConditionRow],
    events: &[ResourceEventSummary],
) -> Vec<TimelineItem> {
    let mut items: Vec<TimelineItem> = Vec::new();
    let mut push = |item: TimelineItem| {
        if !items.iter().any(|existing| existing.id == item.id) {
            items.push(item);
        }
    };
    let succeeded = succeeded_pod(resource);
    for condition in conditions {
        let disruption = disruption_target(condition);
        if (succeeded && completed_readiness(condition))
            || (condition.status == "True" && !disruption)
        {
            continue;
        }
        push(TimelineItem {
            id: format!("condition:{}:{}", condition.kind, condition.status),
            source: "condition",
            tone: match (disruption, condition.status.as_str()) {
                (true, _) => SignalTone::Info,
                (false, "False") => SignalTone::Error,
                _ => SignalTone::Warning,
            },
            title: format!("{}={}", condition.kind, condition.status),
            detail: [condition.reason.as_deref(), condition.message.as_deref()]
                .into_iter()
                .flatten()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" · "),
            timestamp: condition.last_transition.clone(),
        });
    }
    for event in events.iter().filter(|event| is_warning(event)) {
        let repeats = if event.count > 1 {
            format!(" · {} repeats", event.count)
        } else {
            String::new()
        };
        push(TimelineItem {
            id: format!(
                "event:{}:{}:{}",
                event.reason,
                event.last_seen_at.as_deref().unwrap_or("unknown"),
                event.message
            ),
            source: "event",
            tone: SignalTone::Warning,
            title: format!("Warning {}", event.reason),
            detail: format!("{}{repeats}", event.message),
            timestamp: event.last_seen_at.clone(),
        });
    }
    let order = |item: &TimelineItem| millis(item.timestamp.as_deref()).unwrap_or(i64::MAX);
    items.sort_by(|a, b| order(a).cmp(&order(b)).then_with(|| a.id.cmp(&b.id)));
    items
}

/// `IncidentTimelineDayGroup`; `key` is `None` for entries without a time.
pub struct DayGroup<'a> {
    pub key: Option<String>,
    pub label: String,
    pub items: Vec<&'a TimelineItem>,
}

/// `groupIncidentTimelineByDay` in local time. Labels follow the en-GB
/// `toLocaleDateString` the web captures render (`29 Jun 2026`); undated entries go last.
pub fn group_by_day(items: &[TimelineItem]) -> Vec<DayGroup<'_>> {
    let mut groups: Vec<DayGroup> = Vec::new();
    let mut unknown: Vec<&TimelineItem> = Vec::new();
    for item in items {
        let Some(date) = parse(item.timestamp.as_deref()).map(|time| time.with_timezone(&Local))
        else {
            unknown.push(item);
            continue;
        };
        let key = date.format("%Y-%m-%d").to_string();
        match groups
            .iter_mut()
            .find(|group| group.key.as_deref() == Some(&key))
        {
            Some(group) => group.items.push(item),
            None => groups.push(DayGroup {
                key: Some(key),
                label: date.format("%-d %b %Y").to_string(),
                items: vec![item],
            }),
        }
    }
    if !unknown.is_empty() {
        groups.push(DayGroup {
            key: None,
            label: "Time not reported".into(),
            items: unknown,
        });
    }
    groups
}

/// The single dated group's heading is hidden when it is today.
pub fn hide_today_heading<'a>(groups: impl Iterator<Item = Option<&'a str>>) -> bool {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let mut dated = groups.flatten();
    dated.next().is_some_and(|key| key == today) && dated.next().is_none()
}

/// `incidentTimelineTime`.
pub fn timeline_time(item: &TimelineItem) -> String {
    time_of_day(item.timestamp.as_deref(), "No time")
}

fn after<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let lower = text.to_lowercase();
    let start = lower.find(&prefix.to_lowercase())? + prefix.len();
    text.get(start..)
}

fn contains(text: &str, needle: &str) -> bool {
    text.to_lowercase().contains(&needle.to_lowercase())
}

/// `incidentTimelineTitle`: presentation titles for known reasons.
pub fn timeline_title(item: &TimelineItem, resource_kind: &str) -> String {
    let detail = item.detail.as_str();
    let storage = contains(detail, "low on resource: ephemeral-storage");
    let probe = ["startup", "readiness", "liveness"]
        .into_iter()
        .find(|probe| contains(detail, &format!("{probe} probe")));
    if item.source == "event"
        && let Some(probe) = probe
    {
        let name = format!("{}{} probe", probe[..1].to_uppercase(), &probe[1..]);
        if contains(detail, "CONTAINER_EXITED") {
            return format!("{name} could not run: container exited");
        }
        if contains(detail, "timed out") {
            let duration = after(detail, "timed out after ")
                .map(|rest| {
                    rest.split_whitespace()
                        .next()
                        .unwrap_or_default()
                        .to_owned()
                })
                .filter(|duration| duration.chars().next().is_some_and(|c| c.is_ascii_digit()));
            return match duration {
                Some(duration) => format!("{name} timed out after {duration}"),
                None => format!("{name} timed out"),
            };
        }
        if contains(detail, "errored") {
            return format!("{name} could not run");
        }
        if contains(detail, "failed") {
            return format!("{name} failed");
        }
    }
    let or_detail = |fallback: &str| {
        if detail.is_empty() {
            fallback.to_owned()
        } else {
            detail.to_owned()
        }
    };
    match item.title.as_str() {
        "Warning FailedScheduling" => {
            if contains(detail, "untolerated taint") {
                return "Scheduling blocked: node taint not tolerated".into();
            }
            if let Some(resource) = after(detail, "Insufficient ")
                .and_then(|rest| {
                    rest.split(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_'))
                        .next()
                })
                .filter(|resource| !resource.is_empty())
            {
                return format!("Scheduling blocked: insufficient {resource}");
            }
            return or_detail("Pod could not be scheduled");
        }
        "Warning Evicted" if storage => return "Pod evicted: node low on ephemeral storage".into(),
        "Warning Evicted" => return or_detail("Pod evicted"),
        "Warning BackOff" => {
            return match after(detail, "restarting failed container ")
                .and_then(|rest| rest.split_whitespace().next())
            {
                Some(container) => format!("{container} restart delayed after failure"),
                None => or_detail("Container restart delayed"),
            };
        }
        _ => {}
    }
    if item.source == "condition" {
        let failed = if detail.contains("PodFailed") {
            ": pod failed"
        } else {
            ""
        };
        match item.title.as_str() {
            "ContainersReady=False" => return format!("Containers not ready{failed}"),
            "Ready=False" => return format!("{resource_kind} not ready{failed}"),
            "PodReadyToStartContainers=False" => return "Pod not ready to start containers".into(),
            "DisruptionTarget=True" if storage => {
                return "Pod marked for termination: node low on ephemeral storage".into();
            }
            "DisruptionTarget=True" if detail.contains("TerminationByKubelet") => {
                return "Pod marked for termination by kubelet".into();
            }
            "DisruptionTarget=True" => return "Pod marked for disruption".into(),
            _ => return item.title.clone(),
        }
    }
    if detail.is_empty() {
        item.title
            .strip_prefix("Warning ")
            .unwrap_or(&item.title)
            .to_owned()
    } else {
        detail.to_owned()
    }
}
