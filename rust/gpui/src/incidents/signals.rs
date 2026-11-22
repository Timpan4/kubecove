//! Live detail signals, ported from `getConditionRows`, `getContainerStatusRows`
//! and `buildIncidentSignals` in `src/features/resource-detail/helpers.ts`.
//! The incident guide passes no events, so event signals are left out.

use super::guidance::Tone;
use kubecove_lib::models::ResourceSummary;
use serde_json::Value;

/// A restart older than this is stale (`staleRestartMs` default: one hour).
const STALE_RESTART_MS: i64 = 60 * 60 * 1000;

pub struct Signal {
    pub id: String,
    pub label: String,
    pub value: String,
    /// The first timestamp part of the value.
    pub timestamp: Option<String>,
    pub tone: Tone,
}

struct Condition {
    kind: String,
    status: String,
    reason: Option<String>,
    last_transition: Option<String>,
}

#[derive(Default)]
struct State {
    state: Option<String>,
    reason: Option<String>,
    message: Option<String>,
    finished_at: Option<String>,
    exit_code: Option<i64>,
}

struct Container {
    name: String,
    init: bool,
    ready: Option<bool>,
    restarts: i64,
    current: State,
    last: State,
}

fn string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn conditions(status: &Value) -> Vec<Condition> {
    let Some(list) = status.get("conditions").and_then(Value::as_array) else {
        return vec![];
    };
    list.iter()
        .filter(|condition| condition.is_object())
        .map(|condition| Condition {
            kind: string(condition, "type").unwrap_or_else(|| "Condition".into()),
            status: string(condition, "status").unwrap_or_else(|| "Unknown".into()),
            reason: string(condition, "reason"),
            last_transition: string(condition, "lastTransitionTime"),
        })
        .collect()
}

/// `parseStateFields`: the first object-valued entry names the state.
fn parse_state(state: Option<&Value>) -> State {
    let Some((name, fields)) = state
        .and_then(Value::as_object)
        .and_then(|state| state.iter().find(|(_, value)| value.is_object()))
    else {
        return State::default();
    };
    State {
        state: Some(name.clone()),
        reason: string(fields, "reason"),
        message: string(fields, "message"),
        finished_at: string(fields, "finishedAt"),
        exit_code: fields.get("exitCode").and_then(Value::as_i64),
    }
}

fn containers(status: &Value) -> Vec<Container> {
    [
        "initContainerStatuses",
        "containerStatuses",
        "ephemeralContainerStatuses",
    ]
    .iter()
    .flat_map(|key| {
        let init = *key == "initContainerStatuses";
        status
            .get(*key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|container| container.is_object())
            .map(move |container| Container {
                name: string(container, "name").unwrap_or_else(|| "container".into()),
                init,
                ready: container.get("ready").and_then(Value::as_bool),
                restarts: container
                    .get("restartCount")
                    .and_then(Value::as_i64)
                    .unwrap_or(0),
                current: parse_state(container.get("state")),
                last: parse_state(container.get("lastState")),
            })
    })
    .collect()
}

fn recent(timestamp: Option<&str>, now_ms: i64) -> bool {
    timestamp
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .is_some_and(|time| now_ms - time.timestamp_millis() <= STALE_RESTART_MS)
}

fn clean_completed(container: &Container) -> bool {
    container.current.state.as_deref() == Some("terminated")
        && container.current.reason.as_deref() == Some("Completed")
        && container.current.exit_code.is_none_or(|code| code == 0)
}

fn clean_previous_restart(container: &Container) -> bool {
    let last = &container.last;
    last.state.as_deref() == Some("terminated")
        && last.exit_code.is_none_or(|code| code == 0)
        && (last.reason.as_deref() == Some("Completed") || last.exit_code == Some(0))
}

/// `isActionableContainerRestart`.
fn actionable_restart(container: &Container, now_ms: i64, succeeded: bool) -> bool {
    let (current, last) = (&container.current, &container.last);
    if container.restarts <= 0 || (succeeded && clean_completed(container)) {
        return false;
    }
    if container.init
        && clean_completed(container)
        && clean_previous_restart(container)
        && !recent(last.finished_at.as_deref(), now_ms)
    {
        return false;
    }
    let has_previous = last.state.is_some()
        || last.reason.is_some()
        || last.message.is_some()
        || last.finished_at.is_some()
        || last.exit_code.is_some();
    current.state.as_deref() == Some("waiting")
        || container.ready == Some(false)
        || (current.state.as_deref() == Some("terminated") && current.exit_code != Some(0))
        || last.exit_code.is_some_and(|code| code != 0)
        || last
            .reason
            .as_deref()
            .is_some_and(|reason| reason != "Completed")
        || recent(last.finished_at.as_deref(), now_ms)
        || (!clean_previous_restart(container) && !has_previous)
}

fn container_incident(container: &Container) -> bool {
    let current = &container.current;
    if clean_completed(container) {
        return false;
    }
    (current.state.as_deref() == Some("terminated") && current.exit_code != Some(0))
        || current.state.as_deref() == Some("waiting")
        || (container.ready == Some(false)
            && (current.state.is_some() || current.reason.is_some() || current.message.is_some()))
}

fn container_tone(container: &Container) -> Tone {
    let current = &container.current;
    let reason = current
        .reason
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_lowercase();
    let not_ready = container.ready == Some(false);
    if (current.state.as_deref() == Some("terminated") && current.exit_code != Some(0))
        || (not_ready && (reason.contains("error") || reason.contains("crash")))
        || (not_ready && current.state.as_deref() != Some("waiting"))
    {
        Tone::Error
    } else {
        Tone::Warning
    }
}

/// Value parts joined; the first timestamp part is returned separately.
#[derive(Default)]
struct Parts {
    text: String,
    timestamp: Option<String>,
}

impl Parts {
    fn text(&mut self, text: &str) {
        self.text.push_str(text);
    }
    fn timestamp(&mut self, value: &str) {
        self.text.push_str(value);
        self.timestamp.get_or_insert_with(|| value.to_owned());
    }
}

fn container_value(container: &Container) -> Parts {
    let current = &container.current;
    let mut parts = Parts::default();
    let separate = |parts: &mut Parts| {
        if !parts.text.is_empty() {
            parts.text(" · ");
        }
    };
    if let Some(reason) = &current.reason {
        parts.text(reason);
    }
    if let Some(message) = &current.message {
        separate(&mut parts);
        parts.text(message);
    }
    if let Some(code) = current.exit_code {
        separate(&mut parts);
        parts.text(&format!("exit {code}"));
    }
    if let Some(finished) = &current.finished_at {
        separate(&mut parts);
        parts.text("finished ");
        parts.timestamp(finished);
    }
    if parts.text.is_empty() {
        parts.text(if container.ready == Some(false) {
            "not ready"
        } else {
            "state needs attention"
        });
    }
    parts
}

fn restart_value(containers: &[&Container]) -> Parts {
    let mut parts = Parts::default();
    for (index, container) in containers.iter().enumerate() {
        if index > 0 {
            parts.text("; ");
        }
        let times = if container.restarts == 1 {
            "time"
        } else {
            "times"
        };
        parts.text(&format!(
            "{} restarted {} {times}",
            container.name, container.restarts
        ));
        if let Some(reason) = &container.last.reason {
            parts.text(&format!(" · {reason}"));
        }
        if let Some(code) = container.last.exit_code {
            parts.text(&format!(" · exit {code}"));
        }
        if let Some(finished) = &container.last.finished_at {
            parts.text(" · finished ");
            parts.timestamp(finished);
        }
    }
    parts
}

fn condition_value(condition: &Condition) -> Parts {
    let mut parts = Parts::default();
    parts.text(&format!("{}={}", condition.kind, condition.status));
    if let Some(reason) = &condition.reason {
        parts.text(&format!(" · {reason}"));
    }
    if let Some(since) = &condition.last_transition {
        parts.text(" · since ");
        parts.timestamp(since);
    }
    parts
}

/// `resourceStatusTone` for the statuses that produce a status signal.
fn status_tone(status: &str) -> Tone {
    match status.trim().to_lowercase().as_str() {
        "failed" | "error" | "crashloopbackoff" | "imagepullbackoff" => Tone::Error,
        "pending" | "terminating" | "unknown" => Tone::Warning,
        _ => Tone::Neutral,
    }
}

/// `isSuccessfulTerminalResource` in `resource-status.ts`.
fn succeeded(resource: &ResourceSummary) -> bool {
    let status = resource
        .status
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_lowercase();
    resource.kind == "Pod" && matches!(status.as_str(), "succeeded" | "completed" | "complete")
}

fn signal(id: String, label: String, parts: Parts, tone: Tone) -> Signal {
    Signal {
        id,
        label,
        value: parts.text,
        timestamp: parts.timestamp,
        tone,
    }
}

/// `buildIncidentSignals(summary, conditions, [], containers)`.
pub fn detail_signals(resource: &ResourceSummary, status: Option<&Value>) -> Vec<Signal> {
    let empty = Value::Null;
    let status = status.unwrap_or(&empty);
    let conditions = conditions(status);
    let containers = if resource.kind == "Pod" {
        containers(status)
    } else {
        vec![]
    };
    let now_ms = chrono::Utc::now().timestamp_millis();
    let succeeded = succeeded(resource);
    let mut signals = Vec::new();
    for container in containers.iter().filter(|c| container_incident(c)) {
        let state = container.current.state.as_deref();
        signals.push(signal(
            format!(
                "container:{}:{}",
                container.name,
                state.unwrap_or("not-ready")
            ),
            format!("{} {}", container.name, state.unwrap_or("not ready")),
            container_value(container),
            container_tone(container),
        ));
    }
    let actionable: Vec<&Container> = containers
        .iter()
        .filter(|container| actionable_restart(container, now_ms, succeeded))
        .collect();
    // The guide always passes container rows (empty for non-Pods), so only
    // actionable container restarts produce the signal.
    if !actionable.is_empty() {
        let total: i64 = actionable.iter().map(|container| container.restarts).sum();
        let tone = if total > 5 {
            Tone::Error
        } else {
            Tone::Warning
        };
        signals.push(signal(
            "restarts".into(),
            "Restarts".into(),
            restart_value(&actionable),
            tone,
        ));
    }
    for condition in &conditions {
        let id = format!("condition:{}", condition.kind);
        if condition.kind == "DisruptionTarget" && condition.status == "True" {
            signals.push(signal(
                id,
                "Condition".into(),
                condition_value(condition),
                Tone::Info,
            ));
            continue;
        }
        let completed_readiness = (condition.reason.as_deref() == Some("PodCompleted")
            && matches!(condition.kind.as_str(), "Ready" | "ContainersReady"))
            || condition.kind == "PodReadyToStartContainers";
        if condition.status == "True" || (succeeded && completed_readiness) {
            continue;
        }
        let tone = if condition.status == "False" {
            Tone::Error
        } else {
            Tone::Warning
        };
        signals.push(signal(
            id,
            "Condition".into(),
            condition_value(condition),
            tone,
        ));
    }
    if let Some(status) = &resource.status
        && matches!(
            status.to_lowercase().as_str(),
            "failed"
                | "error"
                | "crashloopbackoff"
                | "imagepullbackoff"
                | "pending"
                | "terminating"
                | "unknown"
        )
    {
        signals.push(Signal {
            id: "status".into(),
            label: "Status".into(),
            value: status.clone(),
            timestamp: None,
            tone: status_tone(status),
        });
    }
    if let Some(ready) = &resource.ready
        && ready.to_lowercase() == "false"
        && !succeeded
    {
        signals.push(Signal {
            id: "ready".into(),
            label: "Ready".into(),
            value: ready.clone(),
            timestamp: None,
            tone: Tone::Error,
        });
    }
    signals
}
