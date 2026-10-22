use crate::models::{
    evaluate_health, HealthAssessmentEvidence, HealthAssessmentInput, HealthAssessmentSource,
    HealthAssessmentState, ResourceHealth, ResourceSummary,
};
use serde_json::json;

pub fn pod_restarts(
    statuses: Option<&[k8s_openapi::api::core::v1::ContainerStatus]>,
) -> Option<i32> {
    let total = statuses?.iter().try_fold(0_i32, |total, status| {
        (status.restart_count >= 0).then_some(())?;
        total.checked_add(status.restart_count)
    })?;
    (total > 0).then_some(total)
}

/// Readiness a built-in kind reports, before it is formatted for display.
#[derive(Clone, Copy)]
pub enum Readiness<'a> {
    Absent,
    /// A Ready condition status as the API reports it, such as `True` or `False`.
    Condition(&'a str),
    /// Ready against desired replicas; no desired replicas classifies nothing.
    Counts {
        ready: i32,
        desired: i32,
    },
}

/// Classifies a phase the API reports, such as a Pod phase, together with readiness.
pub fn kubernetes_health(phase: Option<&str>, readiness: Readiness<'_>) -> ResourceHealth {
    // Compare trimmed values case-insensitively instead of allocating lowercase copies per row.
    let phase = phase.unwrap_or_default().trim();
    let successful_terminal = matches_any(phase, &["succeeded", "complete", "completed"]);
    let condition = match readiness {
        Readiness::Condition(status) => status.trim(),
        Readiness::Absent | Readiness::Counts { .. } => "",
    };
    let counts = match readiness {
        Readiness::Counts { ready, desired } if desired > 0 => Some(ready >= desired),
        _ => None,
    };

    if matches_any(phase, &["pending", "terminating"]) {
        return ResourceHealth::Attention;
    }
    if matches_any(
        phase,
        &["failed", "error", "crashloopbackoff", "imagepullbackoff"],
    ) || (!successful_terminal && condition.eq_ignore_ascii_case("false"))
    {
        return ResourceHealth::Degraded;
    }
    if !successful_terminal && counts == Some(false) {
        return ResourceHealth::Attention;
    }
    if successful_terminal
        || matches_any(phase, &["running", "ready", "true", "ready: true"])
        || condition.eq_ignore_ascii_case("true")
        || counts == Some(true)
    {
        return ResourceHealth::Healthy;
    }

    ResourceHealth::Unknown
}

/// Records the Kubernetes evidence for a summary whose status, readiness and restarts are set.
pub fn assess_kubernetes_health(summary: &mut ResourceSummary, health: ResourceHealth) {
    summary.health_assessment = evaluate_health(HealthAssessmentInput {
        recognized_semantics: summary.status.is_some() || summary.ready.is_some(),
        provider_available: true,
        evidence: vec![
            HealthAssessmentEvidence {
                source: HealthAssessmentSource::Kubernetes,
                raw: json!({ "status": summary.status, "ready": summary.ready }),
                state: resource_health_state(health),
                current: true,
                reason: "Kubernetes status and readiness".to_string(),
            },
            HealthAssessmentEvidence {
                source: HealthAssessmentSource::ContainerRestart,
                raw: json!(summary.restarts),
                state: None,
                current: false,
                reason: "Container restart count is evidence, not health state".to_string(),
            },
        ],
    });
    summary.health = legacy_resource_health(summary.health_assessment.state);
}

pub fn legacy_resource_health(state: HealthAssessmentState) -> ResourceHealth {
    match state {
        HealthAssessmentState::Healthy => ResourceHealth::Healthy,
        HealthAssessmentState::NeedsAttention => ResourceHealth::Attention,
        HealthAssessmentState::Degraded => ResourceHealth::Degraded,
        HealthAssessmentState::Unknown | HealthAssessmentState::NotEvaluated => {
            ResourceHealth::Unknown
        }
    }
}

fn resource_health_state(health: ResourceHealth) -> Option<HealthAssessmentState> {
    match health {
        ResourceHealth::Healthy => Some(HealthAssessmentState::Healthy),
        ResourceHealth::Attention => Some(HealthAssessmentState::NeedsAttention),
        ResourceHealth::Degraded => Some(HealthAssessmentState::Degraded),
        ResourceHealth::Restarted | ResourceHealth::Unknown => Some(HealthAssessmentState::Unknown),
    }
}

fn matches_any(value: &str, options: &[&str]) -> bool {
    options
        .iter()
        .any(|option| value.eq_ignore_ascii_case(option))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pod_restarts_preserves_counts_without_overflow_or_negative_values() {
        use k8s_openapi::api::core::v1::ContainerStatus;
        let statuses = |counts: &[i32]| {
            counts
                .iter()
                .map(|count| ContainerStatus {
                    restart_count: *count,
                    ..Default::default()
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(pod_restarts(Some(&statuses(&[2, 3]))), Some(5));
        assert_eq!(pod_restarts(Some(&statuses(&[0]))), None);
        assert_eq!(pod_restarts(None), None);
        assert_eq!(pod_restarts(Some(&statuses(&[i32::MAX, 1]))), None);
        assert_eq!(pod_restarts(Some(&statuses(&[-1, 2]))), None);
    }
}
