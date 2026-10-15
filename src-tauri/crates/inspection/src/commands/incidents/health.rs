use crate::commands::helpers::legacy_resource_health;
use crate::models::{
    evaluate_health, HealthAssessmentCompleteness, HealthAssessmentEvidence, HealthAssessmentInput,
    HealthAssessmentSource, HealthAssessmentState, IncidentSeverity, ResourceEventSummary,
    ResourceSummary,
};
use serde_json::json;

use super::restarts::{restart_signal_message, RestartEvidence};
use crate::models::IncidentSignalState;

pub(super) fn apply_incident_health_evidence(
    resource: &mut ResourceSummary,
    warning_events: &[ResourceEventSummary],
    restart_evidence: Option<&RestartEvidence>,
) {
    let base_assessment = std::mem::take(&mut resource.health_assessment);
    let kubernetes_concern = base_assessment.evidence.iter().any(|evidence| {
        evidence.source == HealthAssessmentSource::Kubernetes
            && evidence.current
            && matches!(
                evidence.state,
                Some(HealthAssessmentState::NeedsAttention | HealthAssessmentState::Degraded)
            )
    });
    let recognized_semantics = base_assessment.state != HealthAssessmentState::NotEvaluated;
    let provider_available = base_assessment.completeness == HealthAssessmentCompleteness::Complete;
    let mut evidence = base_assessment.evidence;

    if let Some(restart) = restart_evidence {
        let current = restart.state == IncidentSignalState::Active;
        evidence.push(HealthAssessmentEvidence {
            source: HealthAssessmentSource::ContainerRestart,
            raw: json!({
                "container": restart.container,
                "reason": restart.reason,
                "exitCode": restart.exit_code,
            }),
            state: current.then_some(HealthAssessmentState::NeedsAttention),
            current,
            reason: restart_signal_message(resource, restart),
        });
    }
    for event in warning_events {
        evidence.push(HealthAssessmentEvidence {
            source: HealthAssessmentSource::WarningEvent,
            raw: json!({
                "reason": event.reason,
                "message": event.message,
                "count": event.count,
                "lastSeenAt": event.last_seen_at,
            }),
            state: kubernetes_concern.then_some(HealthAssessmentState::NeedsAttention),
            current: kubernetes_concern,
            reason: format!("Kubernetes warning event: {}", event.reason),
        });
    }

    resource.health_assessment = evaluate_health(HealthAssessmentInput {
        recognized_semantics,
        provider_available,
        evidence,
    });
    resource.health = legacy_resource_health(resource.health_assessment.state);
}

pub(super) fn has_current_kubernetes_concern(resource: &ResourceSummary) -> bool {
    resource.health_assessment.evidence.iter().any(|evidence| {
        evidence.source == HealthAssessmentSource::Kubernetes
            && evidence.current
            && matches!(
                evidence.state,
                Some(HealthAssessmentState::NeedsAttention | HealthAssessmentState::Degraded)
            )
    })
}

pub(super) fn current_status_severity(resource: &ResourceSummary) -> Option<IncidentSeverity> {
    match resource.health_assessment.state {
        HealthAssessmentState::Degraded => Some(IncidentSeverity::Degraded),
        HealthAssessmentState::NeedsAttention => Some(IncidentSeverity::Attention),
        HealthAssessmentState::Healthy
        | HealthAssessmentState::Unknown
        | HealthAssessmentState::NotEvaluated => None,
    }
}

pub(super) fn severity_for(
    status_severity: Option<IncidentSeverity>,
    has_warning: bool,
    has_restart: bool,
) -> Option<IncidentSeverity> {
    if status_severity.is_some() {
        return status_severity;
    }
    if has_warning {
        return Some(IncidentSeverity::Warning);
    }
    if has_restart {
        return Some(IncidentSeverity::Restarted);
    }
    None
}
