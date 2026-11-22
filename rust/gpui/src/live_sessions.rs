//! Live-session read model, ported from
//! `src/features/live-sessions/liveSessionReadModel.ts` and `helpers.ts`.

use kubecove_lib::models::{
    PodExecSessionStatus, PodExecSessionSummary, PortForwardSessionStatus,
    PortForwardSessionSummary,
};

/// The host's sessions and the kubeconfig source they must come from to be
/// shown, like the web's `settings.kubeconfigSourceKey`.
#[derive(PartialEq)]
pub struct LiveSessions {
    pub port_forwards: Vec<PortForwardSessionSummary>,
    pub exec_sessions: Vec<PodExecSessionSummary>,
    pub source_key: Option<String>,
    pub show_source_labels: bool,
}

/// `normalizeKubeconfigEnvVar` in `src/lib/settings.ts`.
fn env_var_name(env_var: Option<&str>) -> &str {
    env_var
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("KUBECONFIG")
}

/// `normalizeKubeconfigSource` in `helpers.ts`.
fn active_source(source: Option<&str>) -> String {
    match source {
        Some(key) if key.starts_with("kubeconfigSource=") || key.starts_with("kubeconfigEnv=") => {
            key.to_owned()
        }
        other => format!("kubeconfigEnv={}", env_var_name(other)),
    }
}

impl LiveSessions {
    /// `buildLiveSessionReadModel`: sessions in any of `contexts`, the
    /// workspace's contexts, from the active source, oldest first.
    pub fn for_contexts(
        &self,
        contexts: &[String],
    ) -> (Vec<&PortForwardSessionSummary>, Vec<&PodExecSessionSummary>) {
        let source = active_source(self.source_key.as_deref());
        // ADR 0003's 2026-10-22 amendment excludes sessions without a source key.
        let visible = |cluster: &str, key: Option<&String>| {
            contexts.iter().any(|context| context == cluster)
                && key.is_some_and(|key| *key == source)
        };
        let mut forwards: Vec<_> = self
            .port_forwards
            .iter()
            .filter(|s| visible(&s.cluster_context, s.kubeconfig_source_key.as_ref()))
            .collect();
        let mut exec: Vec<_> = self
            .exec_sessions
            .iter()
            .filter(|s| visible(&s.cluster_context, s.kubeconfig_source_key.as_ref()))
            .collect();
        forwards.sort_by(|a, b| a.started_at.cmp(&b.started_at));
        exec.sort_by(|a, b| a.started_at.cmp(&b.started_at));
        (forwards, exec)
    }
}

/// `portForwardSessionTitle` in `LiveSessionsSurface.svelte`.
pub fn forward_title(session: &PortForwardSessionSummary) -> String {
    format!(
        "{}/{}:{}",
        session.target_kind, session.target_name, session.remote_port
    )
}

/// `portForwardSessionResolution` in `LiveSessionsSurface.svelte`.
pub fn forward_resolution(session: &PortForwardSessionSummary) -> String {
    let pod = format!(
        "{}:{}",
        session.resolved_pod_name, session.resolved_pod_port
    );
    if session.target_kind == "Service" {
        format!("Latest Pod used: {pod} (each connection may use a different ready Pod)")
    } else {
        format!("Pod: {pod}")
    }
}

/// `portForwardLocalUrl` in `helpers.ts`.
pub fn local_url(session: &PortForwardSessionSummary) -> String {
    if session.local_url.is_empty() {
        format!("http://{}:{}", session.local_address, session.local_port)
    } else {
        session.local_url.clone()
    }
}

/// The serialized status the web prints, and whether `liveSessionBadgeVariant`
/// marks it destructive.
pub fn forward_status(status: PortForwardSessionStatus) -> (&'static str, bool) {
    match status {
        PortForwardSessionStatus::Listening => ("listening", false),
        PortForwardSessionStatus::Reconnecting => ("reconnecting", false),
        PortForwardSessionStatus::Connected => ("connected", false),
        PortForwardSessionStatus::Error => ("error", true),
        PortForwardSessionStatus::Stopped => ("stopped", true),
    }
}

pub fn exec_status(status: PodExecSessionStatus) -> &'static str {
    match status {
        PodExecSessionStatus::Starting => "starting",
        PodExecSessionStatus::Connecting => "connecting",
        PodExecSessionStatus::Running => "running",
        PodExecSessionStatus::Exited => "exited",
        PodExecSessionStatus::Error => "error",
    }
}

/// `podExecCommandText`: the argv as JSON.
pub fn exec_command(command: &[String]) -> String {
    serde_json::to_string(command).unwrap_or_default()
}
