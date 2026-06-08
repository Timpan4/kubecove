use super::{resource_scope_from, ClusterLiveStore};
use crate::commands::{
    helpers::{k8s_timestamp_to_datetime, list_params, resource_age},
    kubeconfig::KubeconfigSource,
};
use crate::models::{
    AppError, IncidentCockpitItem, IncidentCockpitSummary, IncidentSeverity, IncidentSignalSummary,
    ResourceEventSummary, ResourceListRequest, ResourceSummary,
};
use chrono::{DateTime, Utc};
use k8s_openapi::api::core::v1::Event;
use kube::{api::Api, Client};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;
use tauri::State;

const MAX_WARNING_EVENTS_PER_RESOURCE: usize = 3;
const MAX_WARNING_EVENTS_TOTAL: usize = 500;

#[tauri::command]
pub async fn list_incident_cockpit(
    cluster_context: String,
    requests: Vec<ResourceListRequest>,
    kubeconfig_env_var: Option<String>,
    live_store: State<'_, ClusterLiveStore>,
) -> Result<IncidentCockpitSummary, AppError> {
    let started = Instant::now();
    eprintln!(
        "[kubecove:backend] list_incident_cockpit start context={} requests={}",
        cluster_context,
        requests.len()
    );
    let result = incident_cockpit_from(
        cluster_context.clone(),
        requests,
        kubeconfig_env_var,
        live_store.inner().clone(),
    )
    .await;
    match &result {
        Ok(summary) => eprintln!(
            "[kubecove:backend] list_incident_cockpit done context={} items={} warnings={} ms={}",
            cluster_context,
            summary.items.len(),
            summary.warnings.len(),
            started.elapsed().as_millis()
        ),
        Err(err) => eprintln!(
            "[kubecove:backend] list_incident_cockpit error context={} error_kind={} message={} ms={}",
            cluster_context,
            err.kind,
            err.message,
            started.elapsed().as_millis()
        ),
    }
    result
}

pub async fn incident_cockpit_from(
    cluster_context: String,
    requests: Vec<ResourceListRequest>,
    kubeconfig_env_var: Option<String>,
    live_store: ClusterLiveStore,
) -> Result<IncidentCockpitSummary, AppError> {
    let resources = resource_scope_from(
        cluster_context.clone(),
        requests.clone(),
        live_store,
        kubeconfig_env_var.clone(),
    )
    .await?;
    let event_scope = event_namespace_scope(&requests, &resources);
    let mut warnings = Vec::new();
    let event_result =
        match list_warning_events(&cluster_context, event_scope, kubeconfig_env_var).await {
            Ok(result) => result,
            Err(err) if is_forbidden_app_error(&err) => {
                warnings.push("Warning events unavailable: forbidden by RBAC.".to_string());
                WarningEventList::default()
            }
            Err(err) => return Err(err),
        };
    if !event_result.denied_namespaces.is_empty() {
        warnings.push(format!(
            "Warning events unavailable in namespaces: {}.",
            event_result.denied_namespaces.join(", ")
        ));
    }
    let events = event_result.events;
    if events.len() >= MAX_WARNING_EVENTS_TOTAL {
        warnings.push(format!(
            "Warning events capped at {} most recent matches.",
            MAX_WARNING_EVENTS_TOTAL
        ));
    }

    Ok(IncidentCockpitSummary {
        cluster: cluster_context,
        generated_at: Utc::now().to_rfc3339(),
        requested_scope: requests,
        items: build_incident_items(resources, events),
        warnings,
    })
}

fn is_forbidden_app_error(error: &AppError) -> bool {
    let message = error.message.to_ascii_lowercase();
    message.contains("forbidden") || message.contains("403")
}

#[derive(Debug, Clone)]
enum EventNamespaceScope {
    All,
    Namespaces(Vec<String>),
    None,
}

fn event_namespace_scope(
    requests: &[ResourceListRequest],
    resources: &[ResourceSummary],
) -> EventNamespaceScope {
    if requests.iter().any(|request| request.namespace.is_none()) {
        return EventNamespaceScope::All;
    }
    let namespaces = resources
        .iter()
        .filter_map(|resource| resource.namespace.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if namespaces.is_empty() {
        EventNamespaceScope::None
    } else {
        EventNamespaceScope::Namespaces(namespaces)
    }
}

async fn client_for_context(
    cluster_context: &str,
    kubeconfig_env_var: Option<String>,
) -> Result<Client, AppError> {
    let source = KubeconfigSource::new(kubeconfig_env_var)?;
    source.client_for_context(cluster_context).await
}

async fn list_warning_events(
    cluster_context: &str,
    scope: EventNamespaceScope,
    kubeconfig_env_var: Option<String>,
) -> Result<WarningEventList, AppError> {
    let client = client_for_context(cluster_context, kubeconfig_env_var).await?;
    let namespace_scopes = match scope {
        EventNamespaceScope::All => vec![None],
        EventNamespaceScope::Namespaces(namespaces) => namespaces.into_iter().map(Some).collect(),
        EventNamespaceScope::None => return Ok(WarningEventList::default()),
    };
    let mut events = Vec::new();
    let mut denied_namespaces = Vec::new();
    for namespace in namespace_scopes {
        let api: Api<Event> = if let Some(namespace) = &namespace {
            Api::namespaced(client.clone(), namespace)
        } else {
            Api::all(client.clone())
        };
        let list = match api.list(&list_params()).await {
            Ok(list) => list,
            Err(err) => {
                let app_error = AppError::from(err);
                if let Some(namespace) = namespace.as_ref() {
                    if is_forbidden_app_error(&app_error) {
                        denied_namespaces.push(namespace.clone());
                        continue;
                    }
                }
                return Err(app_error);
            }
        };
        let mut namespace_events = list.into_iter().filter_map(event_match).collect::<Vec<_>>();
        events.append(&mut namespace_events);
    }
    events.sort_by(event_match_sort);
    events.truncate(MAX_WARNING_EVENTS_TOTAL);
    Ok(WarningEventList {
        events,
        denied_namespaces,
    })
}

#[derive(Debug, Clone, Default)]
struct WarningEventList {
    events: Vec<ResourceEventMatch>,
    denied_namespaces: Vec<String>,
}

#[derive(Debug, Clone)]
struct ResourceEventMatch {
    key: String,
    summary: ResourceEventSummary,
}

fn event_timestamp(event: &Event) -> Option<DateTime<Utc>> {
    event
        .event_time
        .as_ref()
        .and_then(|t| k8s_timestamp_to_datetime(&t.0))
        .or_else(|| {
            event
                .last_timestamp
                .as_ref()
                .and_then(|t| k8s_timestamp_to_datetime(&t.0))
        })
        .or_else(|| {
            event
                .first_timestamp
                .as_ref()
                .and_then(|t| k8s_timestamp_to_datetime(&t.0))
        })
        .or_else(|| {
            event
                .metadata
                .creation_timestamp
                .as_ref()
                .and_then(|t| k8s_timestamp_to_datetime(&t.0))
        })
}

fn event_source(event: &Event) -> String {
    if let Some(component) = &event.reporting_component {
        if !component.is_empty() {
            return component.clone();
        }
    }
    event
        .source
        .as_ref()
        .and_then(|source| source.component.clone())
        .unwrap_or_else(|| "unknown".to_string())
}

fn event_match(event: Event) -> Option<ResourceEventMatch> {
    if event.type_.as_deref() != Some("Warning") {
        return None;
    }
    let kind = event.involved_object.kind.clone()?;
    let name = event.involved_object.name.clone()?;
    let api_version = event.involved_object.api_version.clone();
    let namespace = event.involved_object.namespace.clone();
    let timestamp = event_timestamp(&event);
    let last_seen_at = timestamp.as_ref().map(DateTime::to_rfc3339);
    let last_seen = resource_age(timestamp);
    let source = event_source(&event);
    let summary = ResourceEventSummary {
        event_type: event.type_.unwrap_or_else(|| "Warning".to_string()),
        reason: event.reason.unwrap_or_else(|| "Event".to_string()),
        message: event.message.unwrap_or_default(),
        count: event.count.unwrap_or(1),
        last_seen,
        last_seen_at,
        source,
        namespace: event.metadata.namespace.clone().or(namespace.clone()),
    };
    Some(ResourceEventMatch {
        key: resource_match_key(&kind, api_version.as_deref(), namespace.as_deref(), &name),
        summary,
    })
}

fn event_match_sort(a: &ResourceEventMatch, b: &ResourceEventMatch) -> std::cmp::Ordering {
    event_time_ms(&b.summary)
        .cmp(&event_time_ms(&a.summary))
        .then_with(|| b.summary.count.cmp(&a.summary.count))
        .then_with(|| a.summary.reason.cmp(&b.summary.reason))
}

fn event_time_ms(event: &ResourceEventSummary) -> i64 {
    event
        .last_seen_at
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|dt| dt.timestamp_millis())
        .unwrap_or_default()
}

fn resource_match_key(
    kind: &str,
    api_version: Option<&str>,
    namespace: Option<&str>,
    name: &str,
) -> String {
    format!(
        "{}:{}:{}:{}",
        kind,
        api_version.unwrap_or_default(),
        namespace.unwrap_or_default(),
        name
    )
}

fn resource_key(resource: &ResourceSummary) -> String {
    resource_match_key(
        &resource.kind,
        resource.api_version.as_deref(),
        resource.namespace.as_deref(),
        &resource.name,
    )
}

fn build_incident_items(
    resources: Vec<ResourceSummary>,
    warning_events: Vec<ResourceEventMatch>,
) -> Vec<IncidentCockpitItem> {
    let mut events_by_resource: BTreeMap<String, Vec<ResourceEventSummary>> = BTreeMap::new();
    for event in warning_events {
        events_by_resource
            .entry(event.key)
            .or_default()
            .push(event.summary);
    }

    let mut items = resources
        .into_iter()
        .filter_map(|resource| {
            let key = resource_key(&resource);
            let events = events_by_resource.remove(&key).unwrap_or_default();
            incident_item(resource, events)
        })
        .collect::<Vec<_>>();
    items.sort_by(incident_item_sort);
    items
}

fn incident_item(
    resource: ResourceSummary,
    warning_events: Vec<ResourceEventSummary>,
) -> Option<IncidentCockpitItem> {
    let mut signals = Vec::new();
    if is_degraded(&resource) {
        signals.push(IncidentSignalSummary {
            kind: "status".to_string(),
            label: "Degraded".to_string(),
            message: status_message(&resource),
            source: "status".to_string(),
            last_seen_at: resource.created_at.clone(),
        });
    } else if is_attention(&resource) {
        signals.push(IncidentSignalSummary {
            kind: "status".to_string(),
            label: "Needs attention".to_string(),
            message: status_message(&resource),
            source: "status".to_string(),
            last_seen_at: resource.created_at.clone(),
        });
    }
    if resource.restarts.unwrap_or_default() > 0 {
        signals.push(IncidentSignalSummary {
            kind: "restart".to_string(),
            label: "Restarted".to_string(),
            message: format!(
                "{} restarts observed in summary data.",
                resource.restarts.unwrap_or_default()
            ),
            source: "status".to_string(),
            last_seen_at: resource.created_at.clone(),
        });
    }
    for event in warning_events.iter().take(MAX_WARNING_EVENTS_PER_RESOURCE) {
        signals.push(IncidentSignalSummary {
            kind: "event".to_string(),
            label: event.reason.clone(),
            message: event.message.clone(),
            source: event.source.clone(),
            last_seen_at: event.last_seen_at.clone(),
        });
    }

    let latest_warning_event = warning_events.first().cloned();
    let severity = severity_for(&resource, latest_warning_event.is_some())?;

    Some(IncidentCockpitItem {
        resource,
        severity,
        signals,
        latest_warning_event,
    })
}

fn severity_for(resource: &ResourceSummary, has_warning: bool) -> Option<IncidentSeverity> {
    if is_degraded(resource) {
        return Some(IncidentSeverity::Degraded);
    }
    if is_attention(resource) {
        return Some(IncidentSeverity::Attention);
    }
    if resource.restarts.unwrap_or_default() > 0 {
        return Some(IncidentSeverity::Restarted);
    }
    if has_warning {
        return Some(IncidentSeverity::Warning);
    }
    None
}

fn is_degraded(resource: &ResourceSummary) -> bool {
    let status = resource
        .status
        .as_deref()
        .unwrap_or_default()
        .to_lowercase();
    let ready = resource.ready.as_deref().unwrap_or_default().to_lowercase();
    matches!(
        status.as_str(),
        "failed" | "error" | "crashloopbackoff" | "imagepullbackoff"
    ) || ready == "false"
}

fn is_attention(resource: &ResourceSummary) -> bool {
    if is_degraded(resource) {
        return false;
    }
    if let Some((ready_count, desired_count)) =
        ready_ratio(resource.ready.as_deref().unwrap_or_default())
    {
        if desired_count > 0 && ready_count < desired_count {
            return true;
        }
    }
    let status = resource
        .status
        .as_deref()
        .unwrap_or_default()
        .to_lowercase();
    matches!(status.as_str(), "pending" | "terminating" | "unknown")
}

fn ready_ratio(ready: &str) -> Option<(i32, i32)> {
    let (ready_count, desired_count) = ready.split_once('/')?;
    Some((
        ready_count.trim().parse().ok()?,
        desired_count.trim().parse().ok()?,
    ))
}

fn status_message(resource: &ResourceSummary) -> String {
    [
        resource
            .status
            .as_ref()
            .map(|status| format!("Status {}", status)),
        resource
            .ready
            .as_ref()
            .map(|ready| format!("Ready {}", ready)),
        resource
            .restarts
            .filter(|restarts| *restarts > 0)
            .map(|restarts| format!("{} restarts", restarts)),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ")
}

fn severity_weight(severity: &IncidentSeverity) -> u8 {
    match severity {
        IncidentSeverity::Degraded => 4,
        IncidentSeverity::Attention => 3,
        IncidentSeverity::Restarted => 2,
        IncidentSeverity::Warning => 1,
    }
}

fn incident_item_sort(a: &IncidentCockpitItem, b: &IncidentCockpitItem) -> std::cmp::Ordering {
    severity_weight(&b.severity)
        .cmp(&severity_weight(&a.severity))
        .then_with(|| {
            event_time_ms_opt(&b.latest_warning_event)
                .cmp(&event_time_ms_opt(&a.latest_warning_event))
        })
        .then_with(|| a.resource.namespace.cmp(&b.resource.namespace))
        .then_with(|| a.resource.kind.cmp(&b.resource.kind))
        .then_with(|| a.resource.name.cmp(&b.resource.name))
}

fn event_time_ms_opt(event: &Option<ResourceEventSummary>) -> i64 {
    event.as_ref().map(event_time_ms).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource(name: &str) -> ResourceSummary {
        ResourceSummary {
            kind: "Pod".to_string(),
            cluster: "kind-admin".to_string(),
            name: name.to_string(),
            namespace: Some("default".to_string()),
            age: "1m".to_string(),
            api_version: Some("v1".to_string()),
            group: None,
            version: Some("v1".to_string()),
            plural: Some("pods".to_string()),
            namespaced: Some(true),
            dynamic: None,
            created_at: Some("2026-06-04T10:00:00Z".to_string()),
            status: Some("Running".to_string()),
            ready: Some("true".to_string()),
            restarts: None,
            owner_ref: None,
            argo_app: None,
            helm_release: None,
        }
    }

    fn warning(key_name: &str, reason: &str, at: &str) -> ResourceEventMatch {
        ResourceEventMatch {
            key: resource_match_key("Pod", Some("v1"), Some("default"), key_name),
            summary: ResourceEventSummary {
                event_type: "Warning".to_string(),
                reason: reason.to_string(),
                message: "Back-off restarting failed container".to_string(),
                count: 3,
                last_seen: "1m".to_string(),
                last_seen_at: Some(at.to_string()),
                source: "kubelet".to_string(),
                namespace: Some("default".to_string()),
            },
        }
    }

    #[test]
    fn degraded_status_wins_over_warning_event() {
        let mut pod = resource("api-0");
        pod.status = Some("CrashLoopBackOff".to_string());
        pod.ready = Some("false".to_string());

        let items = build_incident_items(
            vec![pod],
            vec![warning("api-0", "BackOff", "2026-06-04T10:01:00Z")],
        );

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].severity, IncidentSeverity::Degraded);
        assert!(items[0].signals.iter().any(|signal| signal.kind == "event"));
    }

    #[test]
    fn warning_only_resource_becomes_warning_item() {
        let items = build_incident_items(
            vec![resource("api-0")],
            vec![warning("api-0", "FailedMount", "2026-06-04T10:01:00Z")],
        );

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].severity, IncidentSeverity::Warning);
        assert_eq!(
            items[0].latest_warning_event.as_ref().unwrap().reason,
            "FailedMount"
        );
    }

    #[test]
    fn healthy_resource_without_warning_is_omitted() {
        let items = build_incident_items(vec![resource("api-0")], Vec::new());

        assert!(items.is_empty());
    }

    #[test]
    fn workload_ready_ratio_below_desired_becomes_attention_item() {
        let mut deployment = resource("api");
        deployment.kind = "Deployment".to_string();
        deployment.api_version = Some("apps/v1".to_string());
        deployment.group = Some("apps".to_string());
        deployment.plural = Some("deployments".to_string());
        deployment.ready = Some("0/3".to_string());

        let items = build_incident_items(vec![deployment], Vec::new());

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].severity, IncidentSeverity::Attention);
        assert_eq!(items[0].signals[0].label, "Needs attention");
        assert!(items[0].signals[0].message.contains("Ready 0/3"));
    }

    #[test]
    fn items_sort_by_severity_then_recent_warning() {
        let mut restarted = resource("api-1");
        restarted.restarts = Some(2);
        let mut failed = resource("api-0");
        failed.status = Some("Failed".to_string());

        let items = build_incident_items(
            vec![restarted, failed, resource("api-2"), resource("api-3")],
            vec![
                warning("api-2", "BackOff", "2026-06-04T10:00:00Z"),
                warning("api-3", "FailedMount", "2026-06-04T10:02:00Z"),
            ],
        );

        assert_eq!(items[0].resource.name, "api-0");
        assert_eq!(items[1].resource.name, "api-1");
        assert_eq!(items[2].resource.name, "api-3");
        assert_eq!(items[3].resource.name, "api-2");
    }

    #[test]
    fn namespace_scoped_empty_resources_do_not_query_all_events() {
        let scope = event_namespace_scope(
            &[ResourceListRequest {
                kind: Some("Pod".to_string()),
                namespace: Some("default".to_string()),
                resource_kind: None,
            }],
            &[],
        );

        assert!(matches!(scope, EventNamespaceScope::None));
    }

    #[test]
    fn warning_events_match_resources_by_api_version() {
        let mut apps = resource("api");
        apps.api_version = Some("apps/v1".to_string());
        let mut batch = resource("api");
        batch.api_version = Some("batch/v1".to_string());
        let events = vec![ResourceEventMatch {
            key: resource_match_key("Pod", Some("batch/v1"), Some("default"), "api"),
            summary: ResourceEventSummary {
                event_type: "Warning".to_string(),
                reason: "FailedCreate".to_string(),
                message: "batch resource warning".to_string(),
                count: 1,
                last_seen: "1m".to_string(),
                last_seen_at: Some("2026-06-04T10:03:00Z".to_string()),
                source: "controller".to_string(),
                namespace: Some("default".to_string()),
            },
        }];

        let items = build_incident_items(vec![apps, batch], events);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].resource.api_version.as_deref(), Some("batch/v1"));
        assert_eq!(
            items[0].latest_warning_event.as_ref().unwrap().reason,
            "FailedCreate"
        );
    }
}
