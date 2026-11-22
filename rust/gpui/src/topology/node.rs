//! Text and tones of one ownership-map card, ported from
//! `OwnershipResourceNode.svelte`, `topology-status.ts`,
//! `resource-lifecycle.ts` and `ownership-node-name.ts`.

use kubecove_lib::models::{ResourceHealth, TopologyNode};

/// Stoplight tones, ordered by rank so the worst wins.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tone {
    Neutral,
    Success,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleTone {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lifecycle {
    pub label: String,
    pub progress: bool,
    pub tone: LifecycleTone,
}

/// The left rail: a stoplight tone, or sky for resources still progressing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rail {
    Info,
    Tone(Tone),
}

pub struct Card {
    pub name: String,
    pub kind_label: String,
    pub meta: String,
    pub status: String,
    pub ready: Option<String>,
    pub restarts: Option<String>,
    pub rail: Rail,
    pub dot: Tone,
    pub restart_tone: Tone,
    pub lifecycle: Option<Lifecycle>,
}

/// The serialized `ResourceHealth` value the web compares against.
fn health_label(health: &ResourceHealth) -> &'static str {
    match health {
        ResourceHealth::Healthy => "healthy",
        ResourceHealth::Attention => "attention",
        ResourceHealth::Degraded => "degraded",
        ResourceHealth::Restarted => "restarted",
        ResourceHealth::Unknown => "unknown",
    }
}

/// `smartKubernetesName`: shortens generated names past 28 characters.
pub fn smart_name(name: &str, kind: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    if chars.len() <= 28 {
        return name.to_owned();
    }
    let parts: Vec<&str> = name.split('-').collect();
    let suffix = parts.last().copied().unwrap_or("");
    let alnum = |text: &str, range: std::ops::RangeInclusive<usize>| {
        range.contains(&text.len()) && text.chars().all(|c| c.is_ascii_alphanumeric())
    };
    if kind == "Pod" && parts.len() >= 3 && alnum(suffix, 4..=6) {
        let prefix = parts[..parts.len() - 2].join("-");
        if prefix.len() >= 4 {
            return format!("{prefix}...{suffix}");
        }
    }
    if matches!(kind, "ReplicaSet" | "Job") && parts.len() >= 2 && alnum(suffix, 6..=12) {
        let prefix = parts[..parts.len() - 1].join("-");
        if prefix.len() >= 4 {
            return format!("{prefix}-{}...", &suffix[..3]);
        }
    }
    let head: String = chars[..15].iter().collect();
    let tail: String = chars[chars.len() - 5..].iter().collect();
    format!("{head}...{tail}")
}

/// `resourceLifecycle`.
pub fn lifecycle(kind: &str, status: Option<&str>) -> Option<Lifecycle> {
    let status = status.filter(|status| !status.is_empty())?;
    if !matches!(
        kind,
        "Pod" | "Deployment" | "ReplicaSet" | "StatefulSet" | "DaemonSet"
    ) {
        return None;
    }
    let make = |label: &str, progress, tone| {
        Some(Lifecycle {
            label: label.to_owned(),
            progress,
            tone,
        })
    };
    match status {
        "ImagePullBackOff"
        | "ErrImagePull"
        | "CrashLoopBackOff"
        | "CreateContainerConfigError"
        | "CreateContainerError"
        | "RunContainerError"
        | "InvalidImageName"
        | "Rollout failed" => make(status, false, LifecycleTone::Error),
        "Paused" => make(status, false, LifecycleTone::Warning),
        "Rolling out" | "Starting" | "Terminating" | "Scaling down" => {
            make(status, true, LifecycleTone::Info)
        }
        "Pending" | "ContainerCreating" | "PodInitializing" if kind == "Pod" => {
            make("Starting", true, LifecycleTone::Info)
        }
        _ => None,
    }
}

fn normalized(value: Option<&str>) -> String {
    value.unwrap_or("").trim().to_lowercase()
}

fn terminal_success(status: Option<&str>) -> bool {
    matches!(
        normalized(status).as_str(),
        "complete" | "completed" | "succeeded"
    )
}

/// `topologyStatusTone`.
pub fn status_tone(status: Option<&str>) -> Tone {
    match normalized(status).as_str() {
        "available" | "complete" | "completed" | "ready" | "running" | "succeeded" => Tone::Success,
        "pending" | "restarting" | "terminating" | "unknown" | "waiting" => Tone::Warning,
        "crashloopbackoff" | "degraded" | "error" | "failed" | "imagepullbackoff" | "not ready"
        | "notready" => Tone::Error,
        _ => Tone::Neutral,
    }
}

fn health_tone(health: &str) -> Tone {
    match health {
        "healthy" => Tone::Success,
        "degraded" => Tone::Error,
        "attention" => Tone::Warning,
        _ => Tone::Neutral,
    }
}

/// `topologyRestartTone`.
pub fn restart_tone(restarts: i32) -> Tone {
    match restarts {
        5.. => Tone::Error,
        3..=4 => Tone::Warning,
        _ => Tone::Neutral,
    }
}

/// `topologyReadyTone`.
pub fn ready_tone(ready: Option<&str>, status: Option<&str>) -> Tone {
    let value = normalized(ready);
    if value.is_empty() {
        return Tone::Neutral;
    }
    if terminal_success(status) && matches!(value.as_str(), "false" | "not ready") {
        return Tone::Success;
    }
    match value.as_str() {
        "true" | "ready" | "completed" => return Tone::Success,
        "false" | "not ready" | "notready" => return Tone::Error,
        _ => {}
    }
    let Some((ready, desired)) = value.split_once('/') else {
        return Tone::Neutral;
    };
    let count = |text: &str| {
        let text = text.trim();
        (!text.is_empty() && text.chars().all(|c| c.is_ascii_digit()))
            .then(|| text.parse::<u64>().ok())
            .flatten()
    };
    let (Some(ready), Some(desired)) = (count(ready), count(desired)) else {
        return Tone::Neutral;
    };
    if desired > 0 && ready >= desired {
        Tone::Success
    } else if status_tone(status) == Tone::Warning {
        Tone::Warning
    } else {
        Tone::Error
    }
}

/// `topologyReadyText`.
pub fn ready_text(ready: Option<&str>, status: Option<&str>) -> Option<String> {
    let ready = ready.filter(|ready| !ready.is_empty())?;
    let value = normalized(Some(ready));
    Some(
        if terminal_success(status) && matches!(value.as_str(), "false" | "not ready") {
            "Completed".to_owned()
        } else if value == "true" {
            "Ready".to_owned()
        } else if value == "false" {
            "Not ready".to_owned()
        } else {
            ready.to_owned()
        },
    )
}

pub fn card(node: &TopologyNode) -> Card {
    let summary = &node.summary;
    let health = health_label(&node.health);
    let status = node
        .status
        .as_deref()
        .filter(|status| !status.is_empty())
        .unwrap_or(health);
    let lifecycle = lifecycle(&node.kind, node.status.as_deref());
    let restarts = summary.restarts.unwrap_or(0);
    let rail_tone = status_tone(Some(status))
        .max(ready_tone(summary.ready.as_deref(), Some(status)))
        .max(health_tone(health));
    let meta = [
        Some(node.namespace.as_deref().unwrap_or("Cluster scoped")),
        Some(summary.age.as_str()).filter(|age| !age.is_empty()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ");
    Card {
        name: smart_name(&node.name, &node.kind),
        kind_label: node.kind.to_uppercase(),
        meta,
        status: lifecycle
            .as_ref()
            .map_or_else(|| status.to_owned(), |lifecycle| lifecycle.label.clone()),
        ready: ready_text(summary.ready.as_deref(), Some(status)),
        restarts: (restarts > 0).then(|| {
            format!(
                "{restarts} {}",
                if restarts == 1 { "restart" } else { "restarts" }
            )
        }),
        rail: match &lifecycle {
            Some(lifecycle) if lifecycle.progress && health != "degraded" => Rail::Info,
            _ => Rail::Tone(rail_tone),
        },
        dot: rail_tone,
        restart_tone: restart_tone(restarts),
        lifecycle,
    }
}
