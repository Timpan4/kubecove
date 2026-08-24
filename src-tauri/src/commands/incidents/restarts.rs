use crate::commands::helpers::{k8s_timestamp_to_datetime, list_params};
use crate::models::{AppError, IncidentSignalState, ResourceSummary};
use chrono::DateTime;
use futures_util::future::join_all;
use k8s_openapi::api::core::v1::{ContainerStatus, Pod};
use kube::{api::Api, Client};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub(super) struct RestartEvidence {
    pub(super) state: IncidentSignalState,
    pub(super) last_seen_at: Option<String>,
    pub(super) container: Option<String>,
    pub(super) reason: Option<String>,
    pub(super) exit_code: Option<i32>,
}

#[derive(Debug, Default)]
pub(super) struct RestartEvidenceList {
    pub(super) by_resource: BTreeMap<String, RestartEvidence>,
}

pub(super) fn resource_key(resource: &ResourceSummary) -> String {
    format!(
        "{}:{}:{}:{}",
        resource.kind,
        resource.api_version.as_deref().unwrap_or_default(),
        resource.namespace.as_deref().unwrap_or_default(),
        resource.name
    )
}

pub(super) async fn list_restart_evidence(
    client: Client,
    resources: &[ResourceSummary],
) -> Result<RestartEvidenceList, AppError> {
    let mut by_namespace: BTreeMap<&str, Vec<&ResourceSummary>> = BTreeMap::new();
    for resource in resources
        .iter()
        .filter(|resource| resource.kind == "Pod" && resource.restarts.unwrap_or_default() > 0)
    {
        if let Some(namespace) = resource.namespace.as_deref() {
            by_namespace.entry(namespace).or_default().push(resource);
        }
    }

    let mut result = RestartEvidenceList::default();
    let fetches = by_namespace.into_iter().map(|(namespace, restarted)| {
        let client = client.clone();
        async move {
            let pods = Api::<Pod>::namespaced(client, namespace)
                .list(&list_params())
                .await
                .map_err(AppError::from)?;
            Ok::<_, AppError>((restarted, pods))
        }
    });
    for fetch in join_all(fetches).await {
        let (restarted_resources, pods) = fetch?;
        let pods_by_name = pods
            .into_iter()
            .filter_map(|pod| pod.metadata.name.clone().map(|name| (name, pod)))
            .collect::<BTreeMap<_, _>>();
        for resource in restarted_resources {
            let Some(pod) = pods_by_name.get(&resource.name) else {
                continue;
            };
            if let Some(evidence) = restart_evidence_from_pod(pod) {
                result.by_resource.insert(resource_key(resource), evidence);
            }
        }
    }
    Ok(result)
}

struct RestartOccurrence {
    termination: Option<RestartTermination>,
    currently_failing: bool,
}

struct RestartTermination {
    last_seen_at: Option<String>,
    container: String,
    reason: Option<String>,
    exit_code: i32,
}

fn restart_evidence_from_pod(pod: &Pod) -> Option<RestartEvidence> {
    let occurrences = pod
        .status
        .as_ref()
        .and_then(|status| status.container_statuses.as_ref())?
        .iter()
        .filter(|status| status.restart_count > 0)
        .map(restart_occurrence)
        .collect::<Vec<_>>();
    if occurrences.is_empty() {
        return None;
    }
    let currently_failing = occurrences
        .iter()
        .any(|occurrence| occurrence.currently_failing);
    let latest = occurrences
        .iter()
        .filter_map(|occurrence| occurrence.termination.as_ref())
        .max_by_key(|termination| {
            termination
                .last_seen_at
                .as_deref()
                .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
                .map(|time| time.timestamp_millis())
                .unwrap_or_default()
        });
    let state = if currently_failing {
        IncidentSignalState::Active
    } else if latest.is_some_and(|termination| {
        termination_failed(termination.reason.as_deref(), termination.exit_code)
    }) {
        IncidentSignalState::Resolved
    } else {
        IncidentSignalState::Historical
    };

    Some(RestartEvidence {
        state,
        last_seen_at: latest.and_then(|termination| termination.last_seen_at.clone()),
        container: latest.map(|termination| termination.container.clone()),
        reason: latest.and_then(|termination| termination.reason.clone()),
        exit_code: latest.map(|termination| termination.exit_code),
    })
}

fn restart_occurrence(status: &ContainerStatus) -> RestartOccurrence {
    let termination = status
        .last_state
        .as_ref()
        .and_then(|state| state.terminated.as_ref())
        .map(|terminated| RestartTermination {
            last_seen_at: terminated
                .finished_at
                .as_ref()
                .and_then(|time| k8s_timestamp_to_datetime(&time.0))
                .map(|time| time.to_rfc3339()),
            container: status.name.clone(),
            reason: terminated.reason.clone(),
            exit_code: terminated.exit_code,
        });
    RestartOccurrence {
        termination,
        currently_failing: current_container_failure(status),
    }
}

fn current_container_failure(status: &ContainerStatus) -> bool {
    let Some(state) = status.state.as_ref() else {
        return false;
    };
    state.waiting.as_ref().is_some_and(|waiting| {
        matches!(
            waiting.reason.as_deref(),
            Some(
                "CrashLoopBackOff"
                    | "ImagePullBackOff"
                    | "ErrImagePull"
                    | "CreateContainerConfigError"
                    | "CreateContainerError"
                    | "RunContainerError"
                    | "InvalidImageName"
            )
        )
    }) || state.terminated.as_ref().is_some_and(|terminated| {
        termination_failed(terminated.reason.as_deref(), terminated.exit_code)
    })
}

fn termination_failed(reason: Option<&str>, exit_code: i32) -> bool {
    exit_code != 0 || reason.is_some_and(|reason| reason != "Completed")
}

pub(super) fn restart_signal_label(state: IncidentSignalState) -> &'static str {
    match state {
        IncidentSignalState::Active => "Active restart",
        IncidentSignalState::Resolved => "Resolved restart",
        IncidentSignalState::Historical => "Historical restart",
    }
}

pub(super) fn restart_signal_message(
    resource: &ResourceSummary,
    evidence: &RestartEvidence,
) -> String {
    let count = resource.restarts.unwrap_or_default();
    let status = resource.status.as_deref().unwrap_or("unknown");
    let ready = resource.ready.as_deref().unwrap_or("unknown");
    let occurrence = match (
        evidence.container.as_deref(),
        evidence.reason.as_deref(),
        evidence.exit_code,
    ) {
        (Some(container), Some(reason), Some(exit_code)) => {
            format!(" Last termination: {container} {reason} (exit {exit_code}).")
        }
        (Some(container), None, Some(exit_code)) => {
            format!(" Last termination: {container} exit {exit_code}.")
        }
        _ => " Restart time unavailable.".to_string(),
    };
    let explanation = match evidence.state {
        IncidentSignalState::Active => {
            format!("Current status {status}, Ready {ready}, keeps this signal active.")
        }
        IncidentSignalState::Resolved => {
            format!("Current status {status}, Ready {ready}, shows previous failure resolved.")
        }
        IncidentSignalState::Historical => {
            format!("Current status {status}, Ready {ready}, makes restart history only.")
        }
    };
    format!(
        "{count} {} observed. {explanation}{occurrence}",
        if count == 1 { "restart" } else { "restarts" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pod(name: &str, reason: &str, exit_code: i32, state: serde_json::Value) -> Pod {
        serde_json::from_value(serde_json::json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": { "name": name, "namespace": "default" },
            "status": { "containerStatuses": [{
                "name": "app", "ready": true, "restartCount": 1,
                "image": "example.test/app:latest", "imageID": "example.test/app@sha256:deadbeef",
                "containerID": "containerd://test", "state": state,
                "lastState": { "terminated": {
                    "containerID": "containerd://old", "exitCode": exit_code,
                    "finishedAt": "2026-06-04T10:00:00Z", "reason": reason,
                    "startedAt": "2026-06-04T09:59:00Z"
                }}
            }] }
        }))
        .expect("pod fixture should deserialize")
    }

    #[test]
    fn completed_restart_stays_historical_despite_unrelated_pod_health() {
        let mut pod = pod(
            "completed",
            "Completed",
            0,
            serde_json::json!({ "running": { "startedAt": "2026-06-04T10:01:00Z" } }),
        );
        pod.status
            .as_mut()
            .expect("pod fixture has status")
            .container_statuses
            .as_mut()
            .expect("pod fixture has container statuses")
            .push(serde_json::from_value(serde_json::json!({
                "name": "sidecar", "ready": false, "restartCount": 0,
                "image": "example.test/sidecar:latest", "imageID": "example.test/sidecar@sha256:deadbeef",
                "containerID": "containerd://sidecar",
                "state": { "waiting": { "reason": "Pending" } }
            })).expect("sidecar fixture should deserialize"));
        let evidence =
            restart_evidence_from_pod(&pod).expect("restart termination should provide evidence");

        assert_eq!(evidence.state, IncidentSignalState::Historical);
        assert_eq!(evidence.reason.as_deref(), Some("Completed"));
        assert_eq!(
            evidence.last_seen_at.as_deref(),
            Some("2026-06-04T10:00:00+00:00")
        );
    }

    #[test]
    fn recovered_failure_is_resolved_and_current_failure_is_active() {
        let resolved = restart_evidence_from_pod(&pod(
            "resolved",
            "Error",
            1,
            serde_json::json!({ "running": { "startedAt": "2026-06-04T10:01:00Z" } }),
        ))
        .expect("restart termination should provide evidence");
        let active = restart_evidence_from_pod(&pod(
            "active",
            "Error",
            1,
            serde_json::json!({ "waiting": { "reason": "CrashLoopBackOff" } }),
        ))
        .expect("restart termination should provide evidence");

        assert_eq!(resolved.state, IncidentSignalState::Resolved);
        assert_eq!(active.state, IncidentSignalState::Active);
    }

    #[test]
    fn restarted_container_without_termination_stays_historical_with_no_snapshot_details() {
        let mut pod = pod(
            "missing-last-state",
            "Completed",
            0,
            serde_json::json!({ "running": { "startedAt": "2026-06-04T10:01:00Z" } }),
        );
        pod.status
            .as_mut()
            .expect("pod fixture has status")
            .container_statuses
            .as_mut()
            .expect("pod fixture has container statuses")[0]
            .last_state = None;

        let evidence = restart_evidence_from_pod(&pod)
            .expect("restart count should retain historical evidence");

        assert_eq!(evidence.state, IncidentSignalState::Historical);
        assert!(evidence.last_seen_at.is_none());
        assert!(evidence.container.is_none());
        assert!(evidence.reason.is_none());
        assert!(evidence.exit_code.is_none());
        let resource: ResourceSummary = serde_json::from_value(serde_json::json!({
            "kind": "Pod", "cluster": "kind-admin", "name": "missing-last-state",
            "namespace": "default", "age": "1m", "restarts": 1
        }))
        .expect("resource fixture should deserialize");
        assert!(restart_signal_message(&resource, &evidence).contains("Restart time unavailable."));
    }

    #[test]
    fn older_currently_failing_restart_keeps_signal_active_over_newer_resolved_restart() {
        let mut pod = pod(
            "multiple-restarts",
            "Error",
            1,
            serde_json::json!({ "waiting": { "reason": "CrashLoopBackOff" } }),
        );
        pod.status
            .as_mut()
            .expect("pod fixture has status")
            .container_statuses
            .as_mut()
            .expect("pod fixture has container statuses")
            .push(serde_json::from_value(serde_json::json!({
                "name": "newer", "ready": true, "restartCount": 1,
                "image": "example.test/newer:latest", "imageID": "example.test/newer@sha256:deadbeef",
                "containerID": "containerd://newer",
                "state": { "running": { "startedAt": "2026-06-04T10:11:00Z" } },
                "lastState": { "terminated": {
                    "containerID": "containerd://newer-old", "exitCode": 1,
                    "finishedAt": "2026-06-04T10:10:00Z", "reason": "Error",
                    "startedAt": "2026-06-04T10:09:00Z"
                }}
            })).expect("newer container fixture should deserialize"));

        let evidence =
            restart_evidence_from_pod(&pod).expect("restarted containers should provide evidence");

        assert_eq!(evidence.state, IncidentSignalState::Active);
        assert_eq!(evidence.container.as_deref(), Some("newer"));
        assert_eq!(
            evidence.last_seen_at.as_deref(),
            Some("2026-06-04T10:10:00+00:00")
        );
    }
}
