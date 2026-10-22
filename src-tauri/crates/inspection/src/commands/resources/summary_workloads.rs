use super::builtin_summary::{builtin_summary, BuiltinSummary, KindStatus};
use crate::commands::helpers::{kubernetes_health, list_params, Readiness};
use crate::models::{AppError, ResourceHealth, ResourceSummary};
use chrono::{DateTime, Utc};
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet};
use k8s_openapi::api::batch::v1::{CronJob, Job};
use k8s_openapi::api::networking::v1::Ingress;
use kube::{api::Api, Client};
use serde_json::Value;

fn replica_health(ready: i32, desired: i32) -> ResourceHealth {
    kubernetes_health(None, Readiness::Counts { ready, desired })
}

impl BuiltinSummary for Deployment {
    fn kind_status(&self) -> Option<KindStatus> {
        let Some(status) = &self.status else {
            return Some(KindStatus::default());
        };
        let available = status.available_replicas.unwrap_or(0);
        let ready = status.ready_replicas.unwrap_or(0);
        let desired = status.replicas.unwrap_or(0);
        Some(KindStatus {
            status: (available > 0 || ready > 0 || desired > 0)
                .then(|| format!("Available: {available}")),
            ready: Some(format!("{ready}/{desired}")),
            restarts: None,
            health: replica_health(ready, desired),
        })
    }

    fn raw_status(&self) -> Option<serde_json::Result<Value>> {
        self.status.as_ref().map(serde_json::to_value)
    }
}

impl BuiltinSummary for ReplicaSet {
    fn kind_status(&self) -> Option<KindStatus> {
        let Some(status) = &self.status else {
            return Some(KindStatus::default());
        };
        let ready = status.ready_replicas.unwrap_or(0);
        Some(KindStatus {
            status: Some(format!(
                "Available: {}",
                status.available_replicas.unwrap_or(0)
            )),
            ready: Some(format!("{ready}/{}", status.replicas)),
            restarts: None,
            health: replica_health(ready, status.replicas),
        })
    }

    fn raw_status(&self) -> Option<serde_json::Result<Value>> {
        self.status.as_ref().map(serde_json::to_value)
    }
}

impl BuiltinSummary for StatefulSet {
    fn kind_status(&self) -> Option<KindStatus> {
        let Some(status) = &self.status else {
            return Some(KindStatus::default());
        };
        let ready = status.ready_replicas.unwrap_or(0);
        Some(KindStatus {
            ready: Some(format!("{ready}/{}", status.replicas)),
            health: replica_health(ready, status.replicas),
            ..KindStatus::default()
        })
    }

    fn raw_status(&self) -> Option<serde_json::Result<Value>> {
        self.status.as_ref().map(serde_json::to_value)
    }
}

impl BuiltinSummary for DaemonSet {
    fn kind_status(&self) -> Option<KindStatus> {
        let Some(status) = &self.status else {
            return Some(KindStatus::default());
        };
        Some(KindStatus {
            ready: Some(format!(
                "{}/{}",
                status.number_ready, status.desired_number_scheduled
            )),
            health: replica_health(status.number_ready, status.desired_number_scheduled),
            ..KindStatus::default()
        })
    }

    fn raw_status(&self) -> Option<serde_json::Result<Value>> {
        self.status.as_ref().map(serde_json::to_value)
    }
}

impl BuiltinSummary for Job {
    fn kind_status(&self) -> Option<KindStatus> {
        let Some(status) = &self.status else {
            return Some(KindStatus::default());
        };
        let succeeded = status.succeeded.unwrap_or(0);
        let completions = self
            .spec
            .as_ref()
            .and_then(|spec| spec.completions)
            .unwrap_or(1);
        let (phase, health) = if status.failed.unwrap_or(0) > 0 {
            ("Failed", ResourceHealth::Degraded)
        } else if succeeded > 0 {
            ("Complete", ResourceHealth::Healthy)
        } else if status.active.unwrap_or(0) > 0 {
            ("Active", replica_health(succeeded, completions))
        } else {
            ("Pending", ResourceHealth::Attention)
        };
        Some(KindStatus {
            status: Some(phase.to_string()),
            ready: Some(format!("{succeeded}/{completions}")),
            restarts: None,
            health,
        })
    }

    fn raw_status(&self) -> Option<serde_json::Result<Value>> {
        self.status.as_ref().map(serde_json::to_value)
    }
}

impl BuiltinSummary for CronJob {
    fn kind_status(&self) -> Option<KindStatus> {
        let active = self
            .status
            .as_ref()
            .and_then(|status| status.active.as_ref())
            .map_or(0, Vec::len);
        // Active runs are not a health signal, so a CronJob is never classified.
        Some(KindStatus {
            status: (active > 0).then(|| format!("{active} active")),
            ..KindStatus::default()
        })
    }

    fn raw_status(&self) -> Option<serde_json::Result<Value>> {
        self.status.as_ref().map(serde_json::to_value)
    }
}

pub(super) async fn workload_resource_summaries(
    client: Client,
    cluster_context: &str,
    kind: &str,
    namespace: Option<&str>,
) -> Result<Option<Vec<ResourceSummary>>, AppError> {
    let now = Utc::now();
    let summaries = match kind {
        "Deployment" => deployment_summaries(client, cluster_context, namespace, now).await?,
        "ReplicaSet" => replicaset_summaries(client, cluster_context, namespace, now).await?,
        "StatefulSet" => statefulset_summaries(client, cluster_context, namespace, now).await?,
        "DaemonSet" => daemonset_summaries(client, cluster_context, namespace, now).await?,
        "Ingress" => ingress_summaries(client, cluster_context, namespace, now).await?,
        "Job" => job_summaries(client, cluster_context, namespace, now).await?,
        "CronJob" => cronjob_summaries(client, cluster_context, namespace, now).await?,
        _ => return Ok(None),
    };

    Ok(Some(summaries))
}

async fn deployment_summaries(
    client: Client,
    cluster_context: &str,
    namespace: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Vec<ResourceSummary>, AppError> {
    let api: Api<Deployment> = if let Some(ns) = namespace {
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
    .map(|deploy| builtin_summary(cluster_context, &deploy.into_resource(), now))
    .collect())
}

async fn replicaset_summaries(
    client: Client,
    cluster_context: &str,
    namespace: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Vec<ResourceSummary>, AppError> {
    let api: Api<ReplicaSet> = if let Some(ns) = namespace {
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
    .map(|rs| builtin_summary(cluster_context, &rs.into_resource(), now))
    .collect())
}

async fn statefulset_summaries(
    client: Client,
    cluster_context: &str,
    namespace: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Vec<ResourceSummary>, AppError> {
    let api: Api<StatefulSet> = if let Some(ns) = namespace {
        Api::namespaced(client, ns)
    } else {
        Api::all(client)
    };
    Ok(api
        .list(&list_params())
        .await
        .map_err(AppError::from)?
        .iter()
        .map(|ss| builtin_summary(cluster_context, ss, now))
        .collect())
}

async fn daemonset_summaries(
    client: Client,
    cluster_context: &str,
    namespace: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Vec<ResourceSummary>, AppError> {
    let api: Api<DaemonSet> = if let Some(ns) = namespace {
        Api::namespaced(client, ns)
    } else {
        Api::all(client)
    };
    Ok(api
        .list(&list_params())
        .await
        .map_err(AppError::from)?
        .iter()
        .map(|ds| builtin_summary(cluster_context, ds, now))
        .collect())
}

async fn ingress_summaries(
    client: Client,
    cluster_context: &str,
    namespace: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Vec<ResourceSummary>, AppError> {
    let api: Api<Ingress> = if let Some(ns) = namespace {
        Api::namespaced(client, ns)
    } else {
        Api::all(client)
    };
    Ok(api
        .list(&list_params())
        .await
        .map_err(AppError::from)?
        .iter()
        .map(|ing| builtin_summary(cluster_context, ing, now))
        .collect())
}

async fn job_summaries(
    client: Client,
    cluster_context: &str,
    namespace: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Vec<ResourceSummary>, AppError> {
    let api: Api<Job> = if let Some(ns) = namespace {
        Api::namespaced(client, ns)
    } else {
        Api::all(client)
    };
    Ok(kubecove_kube::commands::list_projection::list::<_, super::summary_projection::JobProjection>(api, &list_params())
        .await
        .map_err(AppError::from)?
        .into_iter()
        .map(|job| builtin_summary(cluster_context, &job.into_resource(), now))
        .collect())
}

async fn cronjob_summaries(
    client: Client,
    cluster_context: &str,
    namespace: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Vec<ResourceSummary>, AppError> {
    let api: Api<CronJob> = if let Some(ns) = namespace {
        Api::namespaced(client, ns)
    } else {
        Api::all(client)
    };
    Ok(api
        .list(&list_params())
        .await
        .map_err(AppError::from)?
        .iter()
        .map(|cj| builtin_summary(cluster_context, cj, now))
        .collect())
}
