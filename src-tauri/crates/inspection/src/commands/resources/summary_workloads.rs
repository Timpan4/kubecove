use super::builtin_summary::{
    builtin_summary, list_namespaced_summaries, namespace_or_all, BuiltinSummary, KindStatus,
};
use crate::commands::helpers::{kubernetes_health, list_params, Readiness};
use crate::models::{AppError, ResourceHealth, ResourceSummary};
use chrono::{DateTime, Utc};
use k8s_openapi::api::apps::v1::{
    DaemonSet, Deployment, DeploymentStatus, ReplicaSet, StatefulSet,
};
use k8s_openapi::api::batch::v1::{CronJob, Job};
use k8s_openapi::api::networking::v1::Ingress;
use kube::Client;
use serde_json::Value;

fn replica_health(ready: i32, desired: i32) -> ResourceHealth {
    kubernetes_health(None, Readiness::Counts { ready, desired })
}

impl BuiltinSummary for Deployment {
    fn kind_status(&self) -> Option<KindStatus> {
        if self.status.is_none() && self.spec.as_ref().and_then(|spec| spec.replicas).is_none() {
            return Some(KindStatus::default());
        }
        let pending_status = DeploymentStatus::default();
        let status = self.status.as_ref().unwrap_or(&pending_status);
        let available = status.available_replicas.unwrap_or(0);
        let ready = status.ready_replicas.unwrap_or(0);
        let desired = self
            .spec
            .as_ref()
            .and_then(|spec| spec.replicas)
            .unwrap_or(status.replicas.unwrap_or(0));
        let paused = self
            .spec
            .as_ref()
            .is_some_and(|spec| spec.paused == Some(true));
        let failed = status.conditions.iter().flatten().any(|condition| {
            condition.type_ == "Progressing"
                && condition.status == "False"
                && condition.reason.as_deref() == Some("ProgressDeadlineExceeded")
        });
        let updated = status.updated_replicas.unwrap_or(0);
        let rolling = self
            .metadata
            .generation
            .is_some_and(|generation| generation > status.observed_generation.unwrap_or(0))
            || updated < desired
            || status.replicas.unwrap_or(0) > updated
            || status.available_replicas.unwrap_or(0) < desired;
        let lifecycle = if paused {
            Some("Paused".to_string())
        } else if failed {
            Some("Rollout failed".to_string())
        } else if rolling {
            Some("Rolling out".to_string())
        } else {
            (available > 0 || ready > 0 || desired > 0).then(|| format!("Available: {available}"))
        };
        Some(KindStatus {
            status: lifecycle,
            ready: Some(format!("{ready}/{desired}")),
            restarts: None,
            health: if failed && !paused {
                ResourceHealth::Degraded
            } else {
                replica_health(ready, desired)
            },
        })
    }

    fn raw_status(&self) -> Option<serde_json::Result<Value>> {
        self.status.as_ref().map(serde_json::to_value)
    }
}

impl BuiltinSummary for ReplicaSet {
    fn kind_status(&self) -> Option<KindStatus> {
        let desired = self
            .spec
            .as_ref()
            .and_then(|spec| spec.replicas)
            .or_else(|| self.status.as_ref().map(|status| status.replicas));
        let Some(desired) = desired else {
            return Some(KindStatus::default());
        };
        let ready = self
            .status
            .as_ref()
            .and_then(|status| status.ready_replicas)
            .unwrap_or(0);
        let available = self
            .status
            .as_ref()
            .and_then(|status| status.available_replicas)
            .unwrap_or(0);
        let scaling_down = self
            .status
            .as_ref()
            .is_some_and(|status| status.replicas > desired);
        Some(KindStatus {
            status: Some(if scaling_down {
                "Scaling down".to_string()
            } else if ready < desired || available < desired {
                "Starting".to_string()
            } else {
                format!("Available: {available}")
            }),
            ready: Some(format!("{ready}/{desired}")),
            restarts: None,
            health: replica_health(ready, desired),
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
        "StatefulSet" => {
            list_namespaced_summaries::<StatefulSet>(client, cluster_context, namespace, now)
                .await?
        }
        "DaemonSet" => {
            list_namespaced_summaries::<DaemonSet>(client, cluster_context, namespace, now).await?
        }
        "Ingress" => {
            list_namespaced_summaries::<Ingress>(client, cluster_context, namespace, now).await?
        }
        "Job" => job_summaries(client, cluster_context, namespace, now).await?,
        "CronJob" => {
            list_namespaced_summaries::<CronJob>(client, cluster_context, namespace, now).await?
        }
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
    Ok(kubecove_kube::commands::list_projection::list::<
        _,
        super::summary_projection::DeploymentProjection,
    >(
        namespace_or_all::<Deployment>(client, namespace),
        &list_params(),
    )
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
    Ok(kubecove_kube::commands::list_projection::list::<
        _,
        super::summary_projection::ReplicaSetProjection,
    >(
        namespace_or_all::<ReplicaSet>(client, namespace),
        &list_params(),
    )
    .await
    .map_err(AppError::from)?
    .into_iter()
    .map(|rs| builtin_summary(cluster_context, &rs.into_resource(), now))
    .collect())
}

async fn job_summaries(
    client: Client,
    cluster_context: &str,
    namespace: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Vec<ResourceSummary>, AppError> {
    Ok(kubecove_kube::commands::list_projection::list::<
        _,
        super::summary_projection::JobProjection,
    >(namespace_or_all::<Job>(client, namespace), &list_params())
    .await
    .map_err(AppError::from)?
    .into_iter()
    .map(|job| builtin_summary(cluster_context, &job.into_resource(), now))
    .collect())
}
