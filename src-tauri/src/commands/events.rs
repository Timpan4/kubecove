use crate::commands::helpers::{k8s_timestamp_to_datetime, list_params, resource_age};
use crate::models::{AppError, ResourceEventSummary};
use chrono::{DateTime, Utc};
use kube::{api::Api, config::KubeConfigOptions, Client};
use std::time::Instant;

fn event_timestamp(event: &k8s_openapi::api::core::v1::Event) -> Option<DateTime<Utc>> {
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

fn event_source(event: &k8s_openapi::api::core::v1::Event) -> String {
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

fn event_matches_resource(
    event: &k8s_openapi::api::core::v1::Event,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
) -> bool {
    event.involved_object.kind.as_deref() == Some(kind)
        && event.involved_object.name.as_deref() == Some(name)
        && match namespace {
            Some(ns) => event.involved_object.namespace.as_deref() == Some(ns),
            None => true,
        }
}

fn summarize_event(event: &k8s_openapi::api::core::v1::Event) -> ResourceEventSummary {
    ResourceEventSummary {
        event_type: event.type_.clone().unwrap_or_else(|| "Normal".to_string()),
        reason: event.reason.clone().unwrap_or_else(|| "Event".to_string()),
        message: event.message.clone().unwrap_or_default(),
        count: event.count.unwrap_or(1),
        last_seen: resource_age(event_timestamp(event)),
        last_seen_at: event_timestamp(event).map(|dt| dt.to_rfc3339()),
        source: event_source(event),
        namespace: event.metadata.namespace.clone(),
    }
}

pub async fn resource_events_from(
    cluster_context: String,
    kind: String,
    name: String,
    namespace: Option<String>,
) -> Result<Vec<ResourceEventSummary>, AppError> {
    let options = KubeConfigOptions {
        context: Some(cluster_context),
        ..Default::default()
    };

    let config = kube::Config::from_kubeconfig(&options)
        .await
        .map_err(|e| AppError::kube(e.to_string()))?;

    let client = Client::try_from(config).map_err(|e| AppError::kube(e.to_string()))?;
    let event_api: Api<k8s_openapi::api::core::v1::Event> = if let Some(ns) = &namespace {
        Api::namespaced(client, ns)
    } else {
        Api::all(client)
    };
    let field_selector = match namespace.as_deref() {
        Some(ns) => format!(
            "involvedObject.kind={},involvedObject.name={},involvedObject.namespace={}",
            kind, name, ns,
        ),
        None => format!("involvedObject.kind={},involvedObject.name={}", kind, name),
    };

    let mut events: Vec<_> = event_api
        .list(&list_params().fields(&field_selector))
        .await
        .map_err(|e| AppError::kube(e.to_string()))?
        .into_iter()
        .filter(|event| event_matches_resource(event, &kind, &name, namespace.as_deref()))
        .collect();

    events.sort_by_key(|event| event_timestamp(event));
    events.reverse();

    Ok(events.iter().map(summarize_event).collect())
}

#[tauri::command]
pub async fn list_resource_events(
    cluster_context: String,
    kind: String,
    name: String,
    namespace: Option<String>,
) -> Result<Vec<ResourceEventSummary>, AppError> {
    let started = Instant::now();
    let namespace_label = namespace.as_deref().unwrap_or("<cluster>");
    eprintln!(
        "[k8s-manager:backend] list_resource_events start context={} kind={} namespace={} name={}",
        cluster_context, kind, namespace_label, name
    );
    let result = resource_events_from(
        cluster_context.clone(),
        kind.clone(),
        name.clone(),
        namespace.clone(),
    )
    .await;
    match &result {
        Ok(events) => eprintln!(
            "[k8s-manager:backend] list_resource_events done context={} kind={} namespace={} name={} rows={} ms={}",
            cluster_context,
            kind,
            namespace_label,
            name,
            events.len(),
            started.elapsed().as_millis()
        ),
        Err(err) => eprintln!(
            "[k8s-manager:backend] list_resource_events error context={} kind={} namespace={} name={} error_kind={} message={} ms={}",
            cluster_context,
            kind,
            namespace_label,
            name,
            err.kind,
            err.message,
            started.elapsed().as_millis()
        ),
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{Event, EventSource, ObjectReference};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::{MicroTime, ObjectMeta, Time};

    #[test]
    fn event_helpers_match_source_and_timestamp_precedence() {
        let event_time = Time(k8s_openapi::jiff::Timestamp::from_second(1_700_000_100).unwrap());
        let last_timestamp = Time(k8s_openapi::jiff::Timestamp::from_second(1_700_000_000).unwrap());
        let event = Event {
            event_time: Some(MicroTime(event_time.0)),
            last_timestamp: Some(last_timestamp),
            reporting_component: Some("deployment-controller".to_string()),
            involved_object: ObjectReference {
                kind: Some("Pod".to_string()),
                name: Some("api-0".to_string()),
                namespace: Some("payments".to_string()),
                ..Default::default()
            },
            metadata: ObjectMeta {
                namespace: Some("payments".to_string()),
                ..Default::default()
            },
            type_: Some("Warning".to_string()),
            reason: Some("BackOff".to_string()),
            message: Some("retrying".to_string()),
            count: Some(3),
            ..Default::default()
        };

        assert!(event_matches_resource(&event, "Pod", "api-0", Some("payments")));
        assert!(!event_matches_resource(&event, "Pod", "api-1", Some("payments")));
        assert_eq!(event_source(&event), "deployment-controller");
        assert_eq!(event_timestamp(&event).unwrap().timestamp(), 1_700_000_100);

        let summary = summarize_event(&event);
        assert_eq!(summary.event_type, "Warning");
        assert_eq!(summary.reason, "BackOff");
        assert_eq!(summary.count, 3);
        assert_eq!(summary.namespace, Some("payments".to_string()));
    }

    #[test]
    fn event_source_falls_back_to_legacy_source() {
        let event = Event {
            source: Some(EventSource {
                component: Some("kubelet".to_string()),
                ..Default::default()
            }),
            involved_object: ObjectReference::default(),
            metadata: ObjectMeta::default(),
            ..Default::default()
        };

        assert_eq!(event_source(&event), "kubelet");
    }
}
