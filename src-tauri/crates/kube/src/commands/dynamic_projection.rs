use crate::commands::helpers::{
    extract_argo_app, extract_git_ops_owner, extract_helm_release, extract_owner_ref,
    k8s_creation_timestamp_to_rfc3339, legacy_resource_health,
};
use crate::models::{
    argo_health_assessment, condition_evidence, evaluate_health, AppError, AppErrorKind,
    DiscoveredResourceKind, HealthAssessment, HealthAssessmentEvidence, HealthAssessmentInput,
    HealthAssessmentSource, HealthAssessmentState, ResourceHealth, ResourceSummary,
};
use chrono::{DateTime, TimeZone, Utc};
use kube::api::{ApiResource, DynamicObject};
use serde_json::Value;

pub fn api_resource_from_discovered(
    resource_kind: &DiscoveredResourceKind,
) -> Result<ApiResource, AppError> {
    if resource_kind.version.trim().is_empty()
        || resource_kind.kind.trim().is_empty()
        || resource_kind.plural.trim().is_empty()
    {
        return Err(AppError::new(
            "invalid discovered resource kind",
            AppErrorKind::Cluster,
        ));
    }

    let resource = ApiResource {
        group: resource_kind.group.clone(),
        version: resource_kind.version.clone(),
        api_version: resource_kind.api_version.clone(),
        kind: resource_kind.kind.clone(),
        plural: resource_kind.plural.clone(),
    };
    crate::commands::helpers::validate_api_resource(&resource)?;
    Ok(resource)
}

pub fn dynamic_status_from_data(data: &Value) -> Option<String> {
    let status = data.get("status")?;
    if let Some(phase) = status.get("phase").and_then(Value::as_str) {
        return Some(phase.to_string());
    }

    let conditions = status.get("conditions").and_then(Value::as_array)?;
    if let Some(condition) = conditions.iter().find(|condition| {
        condition
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|condition_type| condition_type == "Ready")
    }) {
        let condition_status = condition.get("status").and_then(Value::as_str)?;
        return Some(format!("Ready: {condition_status}"));
    }

    conditions.first().and_then(|condition| {
        let condition_type = condition.get("type").and_then(Value::as_str)?;
        let condition_status = condition.get("status").and_then(Value::as_str)?;
        Some(format!("{condition_type}: {condition_status}"))
    })
}

pub fn dynamic_resource_summary(
    cluster_context: &str,
    resource_kind: &DiscoveredResourceKind,
    object: &DynamicObject,
) -> ResourceSummary {
    dynamic_resource_summary_at(cluster_context, resource_kind, object, Utc::now())
}

pub fn dynamic_resource_summary_at(
    cluster_context: &str,
    resource_kind: &DiscoveredResourceKind,
    object: &DynamicObject,
    now: DateTime<Utc>,
) -> ResourceSummary {
    let mut summary = ResourceSummary {
        kind: resource_kind.kind.clone(),
        cluster: cluster_context.to_string(),
        name: object.metadata.name.clone().unwrap_or_default(),
        namespace: object.metadata.namespace.clone(),
        age: object
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|time| {
                Utc.timestamp_opt(time.0.as_second(), 0)
                    .single()
                    .unwrap_or(now)
            })
            .map_or_else(
                || "unknown".to_string(),
                |time| crate::commands::helpers::resource_age_at(time, now),
            ),
        api_version: Some(resource_kind.api_version.clone()),
        group: Some(resource_kind.group.clone()),
        version: Some(resource_kind.version.clone()),
        plural: Some(resource_kind.plural.clone()),
        namespaced: Some(resource_kind.namespaced),
        dynamic: Some(true),
        health: ResourceHealth::default(),
        health_assessment: crate::models::HealthAssessment::default(),
        created_at: k8s_creation_timestamp_to_rfc3339(&object.metadata.creation_timestamp),
        status: dynamic_status_from_data(&object.data),
        ready: None,
        restarts: None,
        owner_ref: extract_owner_ref(&object.metadata),
        argo_app: extract_argo_app(&object.metadata),
        helm_release: extract_helm_release(&object.metadata),
        git_ops_owner: extract_git_ops_owner(&object.metadata),
        git_ops_ownership_partial: false,
    };
    if resource_kind.group == "argoproj.io" && resource_kind.kind == "Application" {
        summary.health_assessment = argo_health_assessment(
            object
                .data
                .pointer("/status/health/status")
                .and_then(Value::as_str),
            object
                .data
                .pointer("/status/sync/status")
                .and_then(Value::as_str),
        );
        summary.health = legacy_resource_health(summary.health_assessment.state);
    } else {
        summary.health_assessment = dynamic_health_assessment(&object.data);
        summary.health = legacy_resource_health(summary.health_assessment.state);
    }
    summary
}

pub fn dynamic_resource_summaries(
    cluster_context: &str,
    resource_kind: &DiscoveredResourceKind,
    objects: &[DynamicObject],
) -> Vec<ResourceSummary> {
    objects
        .iter()
        .map(|object| dynamic_resource_summary(cluster_context, resource_kind, object))
        .collect()
}

#[cfg(feature = "bench-support")]
pub fn dynamic_resource_summaries_at(
    cluster_context: &str,
    resource_kind: &DiscoveredResourceKind,
    objects: &[DynamicObject],
    now: DateTime<Utc>,
) -> Vec<ResourceSummary> {
    objects
        .iter()
        .map(|object| dynamic_resource_summary_at(cluster_context, resource_kind, object, now))
        .collect()
}

pub fn dynamic_health_assessment(data: &Value) -> HealthAssessment {
    let conditions = data.pointer("/status/conditions").and_then(Value::as_array);
    let evidence: Vec<HealthAssessmentEvidence> = conditions
        .into_iter()
        .flatten()
        .filter_map(|condition| {
            let kind = condition.get("type")?.as_str()?;
            let status = condition.get("status")?.as_str()?;
            let state = match (kind, status) {
                ("Stalled", "True") | ("Ready" | "Healthy", "False") => {
                    HealthAssessmentState::Degraded
                }
                ("Reconciling", "True") => HealthAssessmentState::NeedsAttention,
                ("Ready" | "Healthy", "True") | ("Reconciling", "False") => {
                    HealthAssessmentState::Healthy
                }
                _ => HealthAssessmentState::Unknown,
            };
            matches!(kind, "Ready" | "Healthy" | "Reconciling" | "Stalled").then(|| {
                HealthAssessmentEvidence {
                    source: HealthAssessmentSource::Kubernetes,
                    raw: condition_evidence(condition),
                    state: Some(state),
                    current: true,
                    reason: format!("Kubernetes {kind} condition is {status}"),
                }
            })
        })
        .collect();
    evaluate_health(HealthAssessmentInput {
        recognized_semantics: !evidence.is_empty(),
        provider_available: true,
        evidence,
    })
}
