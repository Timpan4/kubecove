use crate::commands::{
    diagnostic_field,
    helpers::{k8s_timestamp_to_datetime, list_params, resource_age},
    BackendCancellationRegistry, ClusterAccess, FiniteRead,
};
use crate::models::{AppError, ResourceEventSummary};
use crate::runtime::State;
use chrono::{DateTime, Utc};
use kube::api::Api;

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
    uid: Option<&str>,
) -> bool {
    event.involved_object.kind.as_deref() == Some(kind)
        && event.involved_object.name.as_deref() == Some(name)
        && match namespace {
            Some(ns) => event.involved_object.namespace.as_deref() == Some(ns),
            None => true,
        }
        && match (uid, event.involved_object.uid.as_deref()) {
            (Some(expected), Some(actual)) => expected == actual,
            _ => true,
        }
}

fn summarize_event_at(
    event: &k8s_openapi::api::core::v1::Event,
    now: Option<DateTime<Utc>>,
) -> ResourceEventSummary {
    ResourceEventSummary {
        event_type: event.type_.clone().unwrap_or_else(|| "Normal".to_string()),
        reason: event.reason.clone().unwrap_or_else(|| "Event".to_string()),
        message: event.message.clone().unwrap_or_default(),
        count: event.count.unwrap_or(1),
        last_seen: match (event_timestamp(event), now) {
            (Some(timestamp), Some(now)) => {
                crate::commands::helpers::resource_age_at(timestamp, now)
            }
            (timestamp, _) => resource_age(timestamp),
        },
        last_seen_at: event_timestamp(event).map(|dt| dt.to_rfc3339()),
        source: event_source(event),
        namespace: event.metadata.namespace.clone(),
    }
}

pub(crate) fn resource_events_after_fetch(
    events: Vec<k8s_openapi::api::core::v1::Event>,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
    uid: Option<&str>,
) -> Vec<ResourceEventSummary> {
    resource_events_after_fetch_at(events, kind, name, namespace, uid, None)
}

pub fn resource_events_after_fetch_at(
    mut events: Vec<k8s_openapi::api::core::v1::Event>,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
    uid: Option<&str>,
    now: Option<DateTime<Utc>>,
) -> Vec<ResourceEventSummary> {
    events.retain(|event| event_matches_resource(event, kind, name, namespace, uid));
    events.sort_by_key(event_timestamp);
    events.reverse();
    events
        .iter()
        .map(|event| summarize_event_at(event, now))
        .collect()
}

pub async fn resource_events_from(
    access: &ClusterAccess,
    cluster_context: String,
    kind: String,
    name: String,
    namespace: Option<String>,
    uid: Option<String>,
) -> Result<Vec<ResourceEventSummary>, AppError> {
    let client = access.read_client(&cluster_context).await?;
    let event_api: Api<k8s_openapi::api::core::v1::Event> = if let Some(ns) = &namespace {
        Api::namespaced(client, ns)
    } else {
        Api::all(client)
    };
    let field_selector = match namespace.as_deref() {
        Some(ns) => format!(
            "involvedObject.kind={kind},involvedObject.name={name},involvedObject.namespace={ns}",
        ),
        None => format!("involvedObject.kind={kind},involvedObject.name={name}"),
    };

    let events: Vec<_> = kubecove_kube::commands::list_projection::list::<
        _,
        super::event_projection::EventProjection,
    >(event_api, &list_params().fields(&field_selector))
    .await
    .map_err(AppError::from)?
    .into_iter()
    .map(|event| event.0)
    .collect();

    Ok(resource_events_after_fetch(
        events,
        &kind,
        &name,
        namespace.as_deref(),
        uid.as_deref(),
    ))
}

pub async fn list_resource_events(
    cluster_context: String,
    kind: String,
    name: String,
    namespace: Option<String>,
    uid: Option<String>,
    kubeconfig_env_var: Option<String>,
    request_id: Option<String>,
    cancel_scope: Option<String>,
    cancellations: State<'_, BackendCancellationRegistry>,
) -> Result<Vec<ResourceEventSummary>, AppError> {
    FiniteRead::new("list_resource_events")
        .label("context", &cluster_context)
        .label("kind", &kind)
        .label("namespace", namespace.as_deref().unwrap_or("<cluster>"))
        .label("name", &name)
        .cancellable(&cancellations, cancel_scope, request_id)
        .run(
            {
                let kind = kind.clone();
                async move {
                    let access = ClusterAccess::resolve(kubeconfig_env_var.as_deref())?;
                    resource_events_from(&access, cluster_context, kind, name, namespace, uid).await
                }
            },
            |events| {
                vec![
                    diagnostic_field("kind", &kind),
                    diagnostic_field("rows", events.len()),
                ]
            },
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{Event, EventSource, ObjectReference};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::{MicroTime, ObjectMeta, Time};

    #[test]
    fn event_helpers_match_source_and_timestamp_precedence() {
        let event_time = Time(k8s_openapi::jiff::Timestamp::from_second(1_700_000_100).unwrap());
        let last_timestamp =
            Time(k8s_openapi::jiff::Timestamp::from_second(1_700_000_000).unwrap());
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

        assert!(event_matches_resource(
            &event,
            "Pod",
            "api-0",
            Some("payments"),
            None
        ));
        assert!(!event_matches_resource(
            &event,
            "Pod",
            "api-1",
            Some("payments"),
            None
        ));
        assert_eq!(event_source(&event), "deployment-controller");
        assert_eq!(event_timestamp(&event).unwrap().timestamp(), 1_700_000_100);

        let summary = summarize_event_at(&event, None);
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

    #[test]
    fn event_matching_allows_cluster_scope_only_when_namespace_is_absent() {
        let event = Event {
            involved_object: ObjectReference {
                kind: Some("Node".to_string()),
                name: Some("kind-worker".to_string()),
                namespace: None,
                ..Default::default()
            },
            metadata: ObjectMeta::default(),
            ..Default::default()
        };

        assert!(event_matches_resource(
            &event,
            "Node",
            "kind-worker",
            None,
            None
        ));
        assert!(!event_matches_resource(
            &event,
            "Node",
            "kind-worker",
            Some("default"),
            None,
        ));
    }

    #[test]
    fn event_matching_requires_matching_uid_when_both_present() {
        let event = Event {
            involved_object: ObjectReference {
                kind: Some("Pod".to_string()),
                name: Some("api-0".to_string()),
                uid: Some("real".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        assert!(event_matches_resource(
            &event,
            "Pod",
            "api-0",
            None,
            Some("real")
        ));
        assert!(!event_matches_resource(
            &event,
            "Pod",
            "api-0",
            None,
            Some("forged")
        ));
        assert!(event_matches_resource(&event, "Pod", "api-0", None, None));
    }

    #[test]
    fn event_timestamp_falls_back_to_creation_timestamp() {
        let created = Time(k8s_openapi::jiff::Timestamp::from_second(1_700_000_050).unwrap());
        let event = Event {
            involved_object: ObjectReference::default(),
            metadata: ObjectMeta {
                creation_timestamp: Some(created),
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(event_timestamp(&event).unwrap().timestamp(), 1_700_000_050);
    }
}
