use super::{fixed_now, summaries};
use crate::{
    commands::{events, incidents, resources},
    models::{IncidentCockpitSummary, ResourceEventSummary, ResourceListRequest, ResourceSummary},
};
use k8s_openapi::api::core::v1::{Event, Pod};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct IncidentFixture {
    pub resources: Vec<ResourceSummary>,
    pub events: Vec<Event>,
    pub pods: Vec<(Vec<ResourceSummary>, Vec<Pod>)>,
    pub requests: Vec<ResourceListRequest>,
    pub warnings: Vec<String>,
}

#[derive(Clone)]
pub struct ResourceEventFixture {
    pub events: Vec<Event>,
    pub kind: String,
    pub name: String,
    pub namespace: String,
    pub uid: String,
}

fn parse<T: DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).expect("valid event benchmark object")
}

fn event(index: usize, kind: &str, name: &str, namespace: &str, uid: Option<&str>) -> Event {
    parse(json!({
        "apiVersion":"v1","kind":"Event",
        "metadata":{"name":format!("warning-{index}"),"namespace":namespace,"creationTimestamp":"2026-09-30T11:59:00Z"},
        "involvedObject":{"apiVersion":"v1","kind":kind,"name":name,"namespace":namespace,"uid":uid},
        "reason":if index.is_multiple_of(2) {"BackOff"} else {"FailedScheduling"},
        "message":format!("deterministic warning event {index} with a retained diagnostic message"),
        "source":{"component":if index.is_multiple_of(3) {"legacy-kubelet"} else {"kubelet"}},
        "reportingComponent":if index.is_multiple_of(3) {Value::Null} else {Value::String("kubelet".to_string())},
        "type":"Warning","count":(index % 5 + 1),
        "eventTime":if index.is_multiple_of(4) {Value::Null} else {Value::String(format!("2026-09-30T11:59:{:02}Z", index % 10))},
        "lastTimestamp":if index.is_multiple_of(4) {Value::String(format!("2026-09-30T11:59:{:02}Z", index % 10))} else {Value::Null}
    }))
}

pub fn incident_fixture() -> IncidentFixture {
    let source = summaries::resource_summary_fixture(500);
    let mut resources = source
        .pods
        .iter()
        .map(|pod| resources::builtin_summary("incident-cluster", pod, fixed_now()))
        .collect::<Vec<_>>();
    let mut pods = source.pods;
    let mut events = Vec::with_capacity(500);
    for index in 0..500 {
        let resource_index = if index < 498 { index / 3 } else { 166 };
        let pod = &pods[resource_index];
        events.push(event(
            index,
            "Pod",
            pod.metadata.name.as_deref().expect("Pod fixture name"),
            pod.metadata
                .namespace
                .as_deref()
                .expect("Pod fixture namespace"),
            pod.metadata.uid.as_deref(),
        ));
    }
    // Same-name Pods must retain namespace-specific restart evidence.
    for (namespace, active) in [("collision-active", true), ("collision-historical", false)] {
        let mut pod = pods[3].clone();
        pod.metadata.name = Some("shared-name".to_string());
        pod.metadata.namespace = Some(namespace.to_string());
        let pod_status = pod.status.as_mut().expect("Pod status");
        pod_status.phase = Some("Running".to_string());
        pod_status.conditions = Some(
            serde_json::from_value(json!([{"type":"Ready","status":"True"}]))
                .expect("ready condition"),
        );
        let status = &mut pod_status
            .container_statuses
            .as_mut()
            .expect("container status")[0];
        status.restart_count = 3;
        status.ready = true;
        status.state = Some(
            serde_json::from_value(json!({"running":{"startedAt":"2026-09-30T12:00:00Z"}}))
                .expect("running state"),
        );
        if active {
            status.state = Some(
                serde_json::from_value(json!({"waiting":{"reason":"CrashLoopBackOff"}}))
                    .expect("waiting state"),
            );
        } else {
            let termination = status
                .last_state
                .as_mut()
                .expect("last state")
                .terminated
                .as_mut()
                .expect("termination");
            termination.reason = Some("Completed".to_string());
            termination.exit_code = 0;
        }
        resources.push(resources::builtin_summary(
            "incident-cluster",
            &pod,
            fixed_now(),
        ));
        pods.push(pod);
    }
    // Typed Events reject malformed timestamps during deserialization. Missing
    // timestamps exercise the representable post-fetch fallback instead.
    events[0].event_time = None;
    events[0].last_timestamp = None;
    events[0].first_timestamp = None;
    events[0].metadata.creation_timestamp = None;
    let mut pods_by_namespace = BTreeMap::<String, Vec<Pod>>::new();
    for pod in pods {
        pods_by_namespace
            .entry(pod.metadata.namespace.clone().expect("Pod namespace"))
            .or_default()
            .push(pod);
    }
    let mut restarted_by_namespace = BTreeMap::<String, Vec<ResourceSummary>>::new();
    for resource in &resources {
        if resource.restarts.unwrap_or_default() > 0 {
            restarted_by_namespace
                .entry(resource.namespace.clone().expect("resource namespace"))
                .or_default()
                .push(resource.clone());
        }
    }
    // Namespace grouping precedes API fetches in production, so prepare both
    // sides of each fetched batch outside the post-fetch measurement.
    let batches = restarted_by_namespace
        .into_iter()
        .map(|(namespace, resources)| {
            (
                resources,
                pods_by_namespace
                    .remove(&namespace)
                    .expect("fetched namespace batch"),
            )
        })
        .collect();
    IncidentFixture {
        resources,
        events,
        pods: batches,
        requests: vec![ResourceListRequest {
            kind: Some("Pod".to_string()),
            resource_kind: None,
            namespace: None,
        }],
        warnings: vec!["Warning events are sampled from the prepared fixture.".to_string()],
    }
}

pub fn prepare_incident_summary(fixture: IncidentFixture) -> IncidentCockpitSummary {
    incidents::incident_cockpit_post_fetch(
        "incident-cluster".to_string(),
        fixture.requests,
        fixture.resources,
        fixture.events,
        fixture.pods,
        fixture.warnings,
        fixed_now(),
    )
}

pub fn resource_event_fixture() -> ResourceEventFixture {
    let mut events: Vec<Event> = (0..500)
        .map(|order| {
            let index = (order * 137) % 500;
            let uid = match index % 4 {
                0 => Some("selected-pod-uid"),
                1 => Some("stale-pod-uid"),
                2 => None,
                _ => Some("selected-pod-uid"),
            };
            event(index, "Pod", "ledger-0", "tenant-ledger", uid)
        })
        .collect();
    let precedence = events
        .iter_mut()
        .find(|event| event.metadata.name.as_deref() == Some("warning-0"))
        .expect("precedence event");
    precedence.event_time =
        Some(serde_json::from_value(json!("2026-09-30T12:00:00Z")).expect("event timestamp"));
    precedence.reporting_component = Some("preferred-controller".to_string());
    ResourceEventFixture {
        events,
        kind: "Pod".to_string(),
        name: "ledger-0".to_string(),
        namespace: "tenant-ledger".to_string(),
        uid: "selected-pod-uid".to_string(),
    }
}

pub fn prepare_resource_events(fixture: ResourceEventFixture) -> Vec<ResourceEventSummary> {
    events::resource_events_after_fetch_at(
        fixture.events,
        &fixture.kind,
        &fixture.name,
        Some(&fixture.namespace),
        Some(&fixture.uid),
        Some(fixed_now()),
    )
}
