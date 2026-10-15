use super::{
    event_match_at, incident_cockpit_summary_from_parts, sort_warning_event_matches,
    ResourceEventMatch,
};
use crate::commands::incidents::restarts::restart_evidence_from_pods;
use crate::models::{IncidentCockpitSummary, ResourceListRequest, ResourceSummary};
use chrono::{DateTime, Utc};
use k8s_openapi::api::core::v1::{Event, Pod};
use std::collections::BTreeMap;

pub fn incident_cockpit_post_fetch(
    cluster_context: String,
    requests: Vec<ResourceListRequest>,
    resources: Vec<ResourceSummary>,
    warning_events: Vec<Event>,
    restarted_pods: Vec<(Vec<ResourceSummary>, Vec<Pod>)>,
    warnings: Vec<String>,
    generated_at: DateTime<Utc>,
) -> IncidentCockpitSummary {
    let mut restart_evidence = BTreeMap::new();
    for (resources, pods) in restarted_pods {
        restart_evidence_from_pods(&resources, pods, &mut restart_evidence);
    }
    let warning_events = warning_events_after_fetch(warning_events, generated_at);
    incident_cockpit_summary_from_parts(
        cluster_context,
        requests,
        resources,
        warning_events,
        restart_evidence,
        warnings,
        generated_at,
    )
}

fn warning_events_after_fetch(events: Vec<Event>, now: DateTime<Utc>) -> Vec<ResourceEventMatch> {
    let mut matches = events
        .into_iter()
        .filter_map(|event| event_match_at(event, now))
        .collect();
    sort_warning_event_matches(&mut matches);
    matches
}
