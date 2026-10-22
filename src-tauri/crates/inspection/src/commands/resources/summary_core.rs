use crate::commands::helpers::{
    base_resource_summary, base_resource_summary_with_health, list_params, update_resource_health,
};
use crate::models::{
    AppError, HealthAssessment, HealthAssessmentCompleteness, HealthAssessmentState,
    ResourceSummary,
};
use chrono::{DateTime, TimeZone, Utc};
use kube::{api::Api, Client};

pub(crate) fn age_from_metadata_at(
    metadata: &k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta,
    now: DateTime<Utc>,
) -> String {
    metadata
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
        )
}

pub(crate) fn pod_resource_summary(
    cluster_context: &str,
    pod: &k8s_openapi::api::core::v1::Pod,
) -> ResourceSummary {
    pod_resource_summary_at(cluster_context, pod, Utc::now())
}

pub fn pod_resource_summary_at(
    cluster_context: &str,
    pod: &k8s_openapi::api::core::v1::Pod,
    now: DateTime<Utc>,
) -> ResourceSummary {
    // Health is always re-evaluated below, so start from an empty placeholder instead of the default.
    let placeholder = HealthAssessment {
        state: HealthAssessmentState::NotEvaluated,
        completeness: HealthAssessmentCompleteness::Complete,
        winning_sources: Vec::new(),
        reasons: Vec::new(),
        evidence: Vec::new(),
    };
    let mut summary = base_resource_summary_with_health(
        "Pod",
        cluster_context,
        &pod.metadata,
        age_from_metadata_at(&pod.metadata, now),
        placeholder,
    );
    if let Some(ref status) = pod.status {
        summary.status = status.phase.clone().filter(|phase| !phase.is_empty());
        summary.ready = status
            .conditions
            .as_ref()
            .and_then(|conds| conds.iter().find(|c| c.type_ == "Ready"))
            .map(|c| c.status.clone());
        summary.restarts =
            crate::commands::helpers::pod_restarts(status.container_statuses.as_deref());
    }
    update_resource_health(&mut summary);
    summary
}

pub(crate) fn service_resource_summary(
    cluster_context: &str,
    service: &k8s_openapi::api::core::v1::Service,
) -> ResourceSummary {
    service_resource_summary_at(cluster_context, service, Utc::now())
}

pub fn service_resource_summary_at(
    cluster_context: &str,
    service: &k8s_openapi::api::core::v1::Service,
    now: DateTime<Utc>,
) -> ResourceSummary {
    base_resource_summary(
        "Service",
        cluster_context,
        &service.metadata,
        age_from_metadata_at(&service.metadata, now),
    )
}

#[cfg(any(test, feature = "bench-support"))]
pub fn config_map_resource_summary_at(
    cluster_context: &str,
    config_map: &k8s_openapi::api::core::v1::ConfigMap,
    now: DateTime<Utc>,
) -> ResourceSummary {
    base_resource_summary(
        "ConfigMap",
        cluster_context,
        &config_map.metadata,
        age_from_metadata_at(&config_map.metadata, now),
    )
}

#[cfg(any(test, feature = "bench-support"))]
pub fn secret_resource_summary_at(
    cluster_context: &str,
    secret: &k8s_openapi::api::core::v1::Secret,
    now: DateTime<Utc>,
) -> ResourceSummary {
    base_resource_summary(
        "Secret",
        cluster_context,
        &secret.metadata,
        age_from_metadata_at(&secret.metadata, now),
    )
}

pub(crate) fn persistent_volume_claim_resource_summary(
    cluster_context: &str,
    claim: &k8s_openapi::api::core::v1::PersistentVolumeClaim,
) -> ResourceSummary {
    persistent_volume_claim_resource_summary_at(cluster_context, claim, Utc::now())
}

pub fn persistent_volume_claim_resource_summary_at(
    cluster_context: &str,
    claim: &k8s_openapi::api::core::v1::PersistentVolumeClaim,
    now: DateTime<Utc>,
) -> ResourceSummary {
    base_resource_summary(
        "PersistentVolumeClaim",
        cluster_context,
        &claim.metadata,
        age_from_metadata_at(&claim.metadata, now),
    )
}

pub(super) async fn core_resource_summaries(
    client: Client,
    cluster_context: &str,
    kind: &str,
    namespace: Option<&str>,
) -> Result<Option<Vec<ResourceSummary>>, AppError> {
    let summaries = match kind {
        "Pod" => {
            let api: Api<k8s_openapi::api::core::v1::Pod> = if let Some(ns) = namespace {
                Api::namespaced(client, ns)
            } else {
                Api::all(client)
            };
            kubecove_kube::commands::list_projection::list::<
                _,
                super::summary_projection::PodProjection,
            >(api, &list_params())
            .await
            .map_err(AppError::from)?
            .into_iter()
            .map(|pod| pod_resource_summary(cluster_context, &pod.into_resource()))
            .collect()
        }
        "Service" => {
            let api: Api<k8s_openapi::api::core::v1::Service> = if let Some(ns) = namespace {
                Api::namespaced(client, ns)
            } else {
                Api::all(client)
            };
            api.list(&list_params())
                .await
                .map_err(AppError::from)?
                .iter()
                .map(|svc| service_resource_summary(cluster_context, svc))
                .collect()
        }
        "ConfigMap" => {
            let api: Api<k8s_openapi::api::core::v1::ConfigMap> = if let Some(ns) = namespace {
                Api::namespaced(client, ns)
            } else {
                Api::all(client)
            };
            api.list_metadata(&list_params())
                .await
                .map_err(AppError::from)?
                .iter()
                .map(|cm| {
                    base_resource_summary(
                        "ConfigMap",
                        cluster_context,
                        &cm.metadata,
                        age_from_metadata_at(&cm.metadata, Utc::now()),
                    )
                })
                .collect()
        }
        "Secret" => {
            let api: Api<k8s_openapi::api::core::v1::Secret> = if let Some(ns) = namespace {
                Api::namespaced(client, ns)
            } else {
                Api::all(client)
            };
            api.list_metadata(&list_params())
                .await
                .map_err(AppError::from)?
                .iter()
                .map(|sec| {
                    base_resource_summary(
                        "Secret",
                        cluster_context,
                        &sec.metadata,
                        age_from_metadata_at(&sec.metadata, Utc::now()),
                    )
                })
                .collect()
        }
        "PersistentVolumeClaim" => {
            let api: Api<k8s_openapi::api::core::v1::PersistentVolumeClaim> =
                if let Some(ns) = namespace {
                    Api::namespaced(client, ns)
                } else {
                    Api::all(client)
                };
            api.list(&list_params())
                .await
                .map_err(AppError::from)?
                .iter()
                .map(|pvc| persistent_volume_claim_resource_summary(cluster_context, pvc))
                .collect()
        }
        _ => return Ok(None),
    };

    Ok(Some(summaries))
}
