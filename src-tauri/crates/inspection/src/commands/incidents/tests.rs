use super::*;
use crate::commands::helpers::{assess_kubernetes_health, kubernetes_health, Readiness};
use crate::models::{HealthAssessmentState, ResourceHealth};

#[test]
fn permission_classification_uses_kind_not_resource_name() {
    let error = AppError::new("pod forbidden-403 not found", AppErrorKind::NotFound);
    assert!(!is_forbidden_app_error(&error));
}

fn assess(resource: &mut ResourceSummary, readiness: Readiness<'_>) {
    let health = kubernetes_health(resource.status.as_deref(), readiness);
    assess_kubernetes_health(resource, health);
}

fn resource(name: &str) -> ResourceSummary {
    let mut resource = ResourceSummary {
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
        health: ResourceHealth::Healthy,
        health_assessment: crate::models::HealthAssessment::default(),
        created_at: Some("2026-06-04T10:00:00Z".to_string()),
        status: Some("Running".to_string()),
        ready: Some("true".to_string()),
        restarts: None,
        owner_ref: None,
        argo_app: None,
        helm_release: None,
        git_ops_owner: None,
        git_ops_ownership_partial: false,
    };
    assess(&mut resource, Readiness::Condition("true"));
    resource
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

fn build_items(
    resources: Vec<ResourceSummary>,
    warning_events: Vec<ResourceEventMatch>,
) -> Vec<IncidentCockpitItem> {
    build_incident_items(resources, warning_events, BTreeMap::new())
}

fn build_items_with_restarts(
    resources: Vec<ResourceSummary>,
    restart_evidence: BTreeMap<String, RestartEvidence>,
) -> Vec<IncidentCockpitItem> {
    build_incident_items(resources, Vec::new(), restart_evidence)
}

#[test]
fn degraded_status_wins_over_warning_event() {
    let mut pod = resource("api-0");
    pod.status = Some("CrashLoopBackOff".to_string());
    pod.ready = Some("false".to_string());
    assess(&mut pod, Readiness::Condition("false"));

    let items = build_items(
        vec![pod],
        vec![warning("api-0", "BackOff", "2026-06-04T10:01:00Z")],
    );

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].severity, IncidentSeverity::Degraded);
    assert_eq!(items[0].warning_event_count, 1);
    assert_eq!(
        items[0].latest_signal_at.as_deref(),
        Some("2026-06-04T10:01:00Z")
    );
    assert!(items[0].signals.iter().any(|signal| signal.kind == "event"));
}

#[test]
fn uncorroborated_warning_stays_healthy_and_historical() {
    let items = build_items(
        vec![resource("api-0")],
        vec![warning("api-0", "FailedMount", "2026-06-04T10:01:00Z")],
    );

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].severity, IncidentSeverity::Warning);
    assert_eq!(items[0].state, IncidentSignalState::Historical);
    assert_eq!(
        items[0].resource.health_assessment.state,
        HealthAssessmentState::Healthy
    );
    assert_eq!(items[0].resource.health, ResourceHealth::Healthy);
    assert_eq!(items[0].warning_event_count, 1);
    assert_eq!(
        items[0].latest_signal_at.as_deref(),
        Some("2026-06-04T10:01:00Z")
    );
    assert_eq!(
        items[0].latest_warning_event.as_ref().unwrap().reason,
        "FailedMount"
    );
}

#[test]
fn active_restart_worsens_healthy_resource_to_needs_attention() {
    let resource = resource("api-0");
    let mut restarts = BTreeMap::new();
    restarts.insert(
        resource_key(&resource),
        RestartEvidence {
            state: IncidentSignalState::Active,
            last_seen_at: Some("2026-06-04T10:01:00Z".to_string()),
            container: Some("api".to_string()),
            reason: Some("CrashLoopBackOff".to_string()),
            exit_code: Some(1),
        },
    );

    let items = build_items_with_restarts(vec![resource], restarts);

    assert_eq!(items[0].severity, IncidentSeverity::Attention);
    assert_eq!(
        items[0].resource.health_assessment.state,
        HealthAssessmentState::NeedsAttention
    );
    assert_eq!(items[0].resource.health, ResourceHealth::Attention);
}

#[test]
fn resolved_restart_does_not_worsen_healthy_resource() {
    let resource = resource("api-0");
    let mut restarts = BTreeMap::new();
    restarts.insert(
        resource_key(&resource),
        RestartEvidence {
            state: IncidentSignalState::Resolved,
            last_seen_at: Some("2026-06-04T10:01:00Z".to_string()),
            container: Some("api".to_string()),
            reason: Some("Error".to_string()),
            exit_code: Some(1),
        },
    );

    let items = build_items_with_restarts(vec![resource], restarts);

    assert_eq!(items[0].severity, IncidentSeverity::Restarted);
    assert_eq!(items[0].state, IncidentSignalState::Resolved);
    assert_eq!(
        items[0].resource.health_assessment.state,
        HealthAssessmentState::Healthy
    );
    assert_eq!(items[0].resource.health, ResourceHealth::Healthy);
}

#[test]
fn healthy_resource_without_warning_is_omitted() {
    let items = build_items(vec![resource("api-0")], Vec::new());

    assert!(items.is_empty());
}

#[test]
fn succeeded_pod_with_false_ready_is_omitted_without_warning() {
    let mut pod = resource("job-pod");
    pod.status = Some("Succeeded".to_string());
    pod.ready = Some("False".to_string());
    assess(&mut pod, Readiness::Condition("False"));

    let items = build_items(vec![pod], Vec::new());

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
    assess(
        &mut deployment,
        Readiness::Counts {
            ready: 0,
            desired: 3,
        },
    );

    let items = build_items(vec![deployment], Vec::new());

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].severity, IncidentSeverity::Attention);
    assert_eq!(items[0].warning_event_count, 0);
    assert_eq!(
        items[0].latest_signal_at.as_deref(),
        Some("2026-06-04T10:00:00Z")
    );
    assert_eq!(items[0].signals[0].label, "Needs attention");
    assert!(items[0].signals[0].message.contains("Ready 0/3"));
}

#[test]
fn items_sort_by_severity_then_recent_signal() {
    let mut restarted = resource("api-1");
    restarted.restarts = Some(2);
    assess(&mut restarted, Readiness::Condition("true"));
    let mut failed = resource("api-0");
    failed.status = Some("Failed".to_string());
    assess(&mut failed, Readiness::Condition("true"));

    let items = build_items(
        vec![restarted, failed, resource("api-2"), resource("api-3")],
        vec![
            warning("api-2", "BackOff", "2026-06-04T10:00:00Z"),
            warning("api-3", "FailedMount", "2026-06-04T10:02:00Z"),
        ],
    );

    assert_eq!(items[0].resource.name, "api-0");
    assert_eq!(items[1].resource.name, "api-3");
    assert_eq!(items[2].resource.name, "api-2");
    assert_eq!(items.len(), 3);
}

#[test]
fn warning_event_count_preserves_all_matched_events() {
    let items = build_items(
        vec![resource("api-0")],
        vec![
            warning("api-0", "BackOff", "2026-06-04T10:03:00Z"),
            warning("api-0", "FailedMount", "2026-06-04T10:02:00Z"),
            warning("api-0", "FailedPull", "2026-06-04T10:01:00Z"),
            warning("api-0", "Unhealthy", "2026-06-04T10:00:00Z"),
        ],
    );

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].warning_event_count, 4);
    assert_eq!(
        items[0].latest_signal_at.as_deref(),
        Some("2026-06-04T10:03:00Z")
    );
    assert_eq!(
        items[0]
            .signals
            .iter()
            .filter(|signal| signal.kind == "event")
            .count(),
        MAX_WARNING_EVENTS_PER_RESOURCE
    );
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

    let items = build_items(vec![apps, batch], events);

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].resource.api_version.as_deref(), Some("batch/v1"));
    assert_eq!(
        items[0].latest_warning_event.as_ref().unwrap().reason,
        "FailedCreate"
    );
}
