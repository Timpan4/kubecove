use crate::commands::helpers::{
    base_resource_summary, fmt_ready, list_params, update_resource_health,
};
use crate::models::{AppError, ResourceSummary};
use chrono::{DateTime, Utc};
use kube::{api::Api, Client};

use super::ingress_status::apply_ingress_status;

pub(crate) fn deployment_resource_summary(
    cluster_context: &str,
    deploy: &k8s_openapi::api::apps::v1::Deployment,
) -> ResourceSummary {
    deployment_resource_summary_at(cluster_context, deploy, Utc::now())
}

pub fn deployment_resource_summary_at(
    cluster_context: &str,
    deploy: &k8s_openapi::api::apps::v1::Deployment,
    now: DateTime<Utc>,
) -> ResourceSummary {
    let mut summary = base_resource_summary(
        "Deployment",
        cluster_context,
        &deploy.metadata,
        super::summary_core::age_from_metadata_at(&deploy.metadata, now),
    );
    if let Some(ref status) = deploy.status {
        let available = status.available_replicas.unwrap_or(0);
        let ready = status.ready_replicas.unwrap_or(0);
        let desired = status.replicas.unwrap_or(0);
        summary.ready = Some(format!("{ready}/{desired}"));
        if available > 0 || ready > 0 || desired > 0 {
            summary.status = Some(format!("Available: {available}"));
        }
    }
    update_resource_health(&mut summary);
    summary
}

pub(crate) fn replicaset_resource_summary(
    cluster_context: &str,
    replica_set: &k8s_openapi::api::apps::v1::ReplicaSet,
) -> ResourceSummary {
    replicaset_resource_summary_at(cluster_context, replica_set, Utc::now())
}

pub fn replicaset_resource_summary_at(
    cluster_context: &str,
    replica_set: &k8s_openapi::api::apps::v1::ReplicaSet,
    now: DateTime<Utc>,
) -> ResourceSummary {
    let mut summary = base_resource_summary(
        "ReplicaSet",
        cluster_context,
        &replica_set.metadata,
        super::summary_core::age_from_metadata_at(&replica_set.metadata, now),
    );
    if let Some(ref status) = replica_set.status {
        summary.ready = Some(fmt_ready(status.ready_replicas, status.replicas));
        summary.status = Some(format!(
            "Available: {}",
            status.available_replicas.unwrap_or(0)
        ));
    }
    update_resource_health(&mut summary);
    summary
}

pub(crate) fn statefulset_resource_summary(
    cluster_context: &str,
    stateful_set: &k8s_openapi::api::apps::v1::StatefulSet,
) -> ResourceSummary {
    statefulset_resource_summary_at(cluster_context, stateful_set, Utc::now())
}

pub fn statefulset_resource_summary_at(
    cluster_context: &str,
    stateful_set: &k8s_openapi::api::apps::v1::StatefulSet,
    now: DateTime<Utc>,
) -> ResourceSummary {
    let mut summary = base_resource_summary(
        "StatefulSet",
        cluster_context,
        &stateful_set.metadata,
        super::summary_core::age_from_metadata_at(&stateful_set.metadata, now),
    );
    if let Some(ref status) = stateful_set.status {
        summary.ready = Some(fmt_ready(status.ready_replicas, status.replicas));
    }
    update_resource_health(&mut summary);
    summary
}

pub(crate) fn daemonset_resource_summary(
    cluster_context: &str,
    daemon_set: &k8s_openapi::api::apps::v1::DaemonSet,
) -> ResourceSummary {
    daemonset_resource_summary_at(cluster_context, daemon_set, Utc::now())
}

pub fn daemonset_resource_summary_at(
    cluster_context: &str,
    daemon_set: &k8s_openapi::api::apps::v1::DaemonSet,
    now: DateTime<Utc>,
) -> ResourceSummary {
    let mut summary = base_resource_summary(
        "DaemonSet",
        cluster_context,
        &daemon_set.metadata,
        super::summary_core::age_from_metadata_at(&daemon_set.metadata, now),
    );
    if let Some(ref status) = daemon_set.status {
        summary.ready = Some(format!(
            "{}/{}",
            status.number_ready, status.desired_number_scheduled
        ));
    }
    update_resource_health(&mut summary);
    summary
}

pub(crate) fn ingress_resource_summary(
    cluster_context: &str,
    ingress: &k8s_openapi::api::networking::v1::Ingress,
) -> ResourceSummary {
    ingress_resource_summary_at(cluster_context, ingress, Utc::now())
}

pub fn ingress_resource_summary_at(
    cluster_context: &str,
    ingress: &k8s_openapi::api::networking::v1::Ingress,
    now: DateTime<Utc>,
) -> ResourceSummary {
    let mut summary = base_resource_summary(
        "Ingress",
        cluster_context,
        &ingress.metadata,
        super::summary_core::age_from_metadata_at(&ingress.metadata, now),
    );
    apply_ingress_status(&mut summary, ingress.status.as_ref());
    update_resource_health(&mut summary);
    summary
}

pub(crate) fn job_resource_summary(
    cluster_context: &str,
    job: &k8s_openapi::api::batch::v1::Job,
) -> ResourceSummary {
    job_resource_summary_at(cluster_context, job, Utc::now())
}

pub fn job_resource_summary_at(
    cluster_context: &str,
    job: &k8s_openapi::api::batch::v1::Job,
    now: DateTime<Utc>,
) -> ResourceSummary {
    let mut summary = base_resource_summary(
        "Job",
        cluster_context,
        &job.metadata,
        super::summary_core::age_from_metadata_at(&job.metadata, now),
    );
    if let Some(ref status) = job.status {
        let active = status.active.unwrap_or(0);
        let failed = status.failed.unwrap_or(0);
        let succeeded = status.succeeded.unwrap_or(0);
        summary.status = if failed > 0 {
            Some("Failed".to_string())
        } else if succeeded > 0 {
            Some("Complete".to_string())
        } else if active > 0 {
            Some("Active".to_string())
        } else {
            Some("Pending".to_string())
        };
        summary.ready = Some(format!(
            "{}/{}",
            succeeded,
            job.spec.as_ref().and_then(|s| s.completions).unwrap_or(1)
        ));
    }
    update_resource_health(&mut summary);
    summary
}

pub(crate) fn cronjob_resource_summary(
    cluster_context: &str,
    cron_job: &k8s_openapi::api::batch::v1::CronJob,
) -> ResourceSummary {
    cronjob_resource_summary_at(cluster_context, cron_job, Utc::now())
}

pub fn cronjob_resource_summary_at(
    cluster_context: &str,
    cron_job: &k8s_openapi::api::batch::v1::CronJob,
    now: DateTime<Utc>,
) -> ResourceSummary {
    let mut summary = base_resource_summary(
        "CronJob",
        cluster_context,
        &cron_job.metadata,
        super::summary_core::age_from_metadata_at(&cron_job.metadata, now),
    );
    if let Some(ref status) = cron_job.status {
        let active = status.active.as_ref().map_or(0, std::vec::Vec::len);
        if active > 0 {
            summary.status = Some(format!("{active} active"));
        }
    }
    update_resource_health(&mut summary);
    summary
}

pub(super) async fn workload_resource_summaries(
    client: Client,
    cluster_context: &str,
    kind: &str,
    namespace: Option<&str>,
) -> Result<Option<Vec<ResourceSummary>>, AppError> {
    let summaries = match kind {
        "Deployment" => deployment_summaries(client, cluster_context, namespace).await?,
        "ReplicaSet" => replicaset_summaries(client, cluster_context, namespace).await?,
        "StatefulSet" => statefulset_summaries(client, cluster_context, namespace).await?,
        "DaemonSet" => daemonset_summaries(client, cluster_context, namespace).await?,
        "Ingress" => ingress_summaries(client, cluster_context, namespace).await?,
        "Job" => job_summaries(client, cluster_context, namespace).await?,
        "CronJob" => cronjob_summaries(client, cluster_context, namespace).await?,
        _ => return Ok(None),
    };

    Ok(Some(summaries))
}

async fn deployment_summaries(
    client: Client,
    cluster_context: &str,
    namespace: Option<&str>,
) -> Result<Vec<ResourceSummary>, AppError> {
    let api: Api<k8s_openapi::api::apps::v1::Deployment> = if let Some(ns) = namespace {
        Api::namespaced(client, ns)
    } else {
        Api::all(client)
    };
    Ok(kubecove_kube::commands::list_projection::list::<
        _,
        super::summary_projection::DeploymentProjection,
    >(api, &list_params())
    .await
    .map_err(AppError::from)?
    .into_iter()
    .map(|deploy| deployment_resource_summary(cluster_context, &deploy.into_resource()))
    .collect())
}

async fn replicaset_summaries(
    client: Client,
    cluster_context: &str,
    namespace: Option<&str>,
) -> Result<Vec<ResourceSummary>, AppError> {
    let api: Api<k8s_openapi::api::apps::v1::ReplicaSet> = if let Some(ns) = namespace {
        Api::namespaced(client, ns)
    } else {
        Api::all(client)
    };
    Ok(kubecove_kube::commands::list_projection::list::<
        _,
        super::summary_projection::ReplicaSetProjection,
    >(api, &list_params())
    .await
    .map_err(AppError::from)?
    .into_iter()
    .map(|rs| replicaset_resource_summary(cluster_context, &rs.into_resource()))
    .collect())
}

async fn statefulset_summaries(
    client: Client,
    cluster_context: &str,
    namespace: Option<&str>,
) -> Result<Vec<ResourceSummary>, AppError> {
    let api: Api<k8s_openapi::api::apps::v1::StatefulSet> = if let Some(ns) = namespace {
        Api::namespaced(client, ns)
    } else {
        Api::all(client)
    };
    Ok(api
        .list(&list_params())
        .await
        .map_err(AppError::from)?
        .iter()
        .map(|ss| statefulset_resource_summary(cluster_context, ss))
        .collect())
}

async fn daemonset_summaries(
    client: Client,
    cluster_context: &str,
    namespace: Option<&str>,
) -> Result<Vec<ResourceSummary>, AppError> {
    let api: Api<k8s_openapi::api::apps::v1::DaemonSet> = if let Some(ns) = namespace {
        Api::namespaced(client, ns)
    } else {
        Api::all(client)
    };
    Ok(api
        .list(&list_params())
        .await
        .map_err(AppError::from)?
        .iter()
        .map(|ds| daemonset_resource_summary(cluster_context, ds))
        .collect())
}

async fn ingress_summaries(
    client: Client,
    cluster_context: &str,
    namespace: Option<&str>,
) -> Result<Vec<ResourceSummary>, AppError> {
    let api: Api<k8s_openapi::api::networking::v1::Ingress> = if let Some(ns) = namespace {
        Api::namespaced(client, ns)
    } else {
        Api::all(client)
    };
    Ok(api
        .list(&list_params())
        .await
        .map_err(AppError::from)?
        .iter()
        .map(|ing| ingress_resource_summary(cluster_context, ing))
        .collect())
}

async fn job_summaries(
    client: Client,
    cluster_context: &str,
    namespace: Option<&str>,
) -> Result<Vec<ResourceSummary>, AppError> {
    let api: Api<k8s_openapi::api::batch::v1::Job> = if let Some(ns) = namespace {
        Api::namespaced(client, ns)
    } else {
        Api::all(client)
    };
    Ok(kubecove_kube::commands::list_projection::list::<_, super::summary_projection::JobProjection>(api, &list_params())
        .await
        .map_err(AppError::from)?
        .into_iter()
        .map(|job| job_resource_summary(cluster_context, &job.into_resource()))
        .collect())
}

async fn cronjob_summaries(
    client: Client,
    cluster_context: &str,
    namespace: Option<&str>,
) -> Result<Vec<ResourceSummary>, AppError> {
    let api: Api<k8s_openapi::api::batch::v1::CronJob> = if let Some(ns) = namespace {
        Api::namespaced(client, ns)
    } else {
        Api::all(client)
    };
    Ok(api
        .list(&list_params())
        .await
        .map_err(AppError::from)?
        .iter()
        .map(|cj| cronjob_resource_summary(cluster_context, cj))
        .collect())
}
