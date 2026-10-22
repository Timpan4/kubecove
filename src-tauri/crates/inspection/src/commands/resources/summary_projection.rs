#[cfg(feature = "bench-support")]
use chrono::{DateTime, Utc};
use k8s_openapi::api::{
    apps::v1::{Deployment, DeploymentStatus, ReplicaSet, ReplicaSetStatus},
    batch::v1::{Job, JobSpec, JobStatus},
    core::v1::{ContainerStatus, Pod, PodCondition, PodStatus},
};
use kubecove_kube::commands::list_projection::Typed;
use serde::{Deserialize, Deserializer};

#[derive(Clone, Deserialize)]
#[serde(transparent)]
pub(super) struct PodProjection(Typed<Pod, PodSummaryStatus>);
#[derive(Clone, Deserialize)]
#[serde(transparent)]
pub(super) struct DeploymentProjection(Typed<Deployment, ReplicaStatus>);
#[derive(Clone, Deserialize)]
#[serde(transparent)]
pub(super) struct ReplicaSetProjection(Typed<ReplicaSet, ReplicaStatus>);
#[derive(Clone, Deserialize)]
#[serde(transparent)]
pub(super) struct JobProjection(Typed<Job, JobSummaryStatus, JobSummarySpec>);

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PodSummaryStatus {
    phase: Option<String>,
    conditions: Option<Vec<ReadyCondition>>,
    container_statuses: Option<Vec<Restarts>>,
}

#[derive(Clone, Deserialize)]
struct ReadyCondition {
    #[serde(default, rename = "type", deserialize_with = "null_as_default")]
    type_: String,
    #[serde(default, deserialize_with = "null_as_default")]
    status: String,
}

#[derive(Clone, Deserialize)]
struct Restarts {
    #[serde(default, rename = "restartCount", deserialize_with = "null_as_default")]
    restart_count: i32,
}

fn null_as_default<'de, D: Deserializer<'de>, T: Deserialize<'de> + Default>(
    deserializer: D,
) -> Result<T, D::Error> {
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

impl PodProjection {
    pub(super) fn into_resource(self) -> Pod {
        let value = self.0;
        Pod {
            metadata: value.metadata,
            status: value.status.map(|status| PodStatus {
                phase: status.phase,
                conditions: status.conditions.map(|conditions| {
                    conditions
                        .into_iter()
                        .map(|c| PodCondition {
                            type_: c.type_,
                            status: c.status,
                            ..PodCondition::default()
                        })
                        .collect()
                }),
                container_statuses: status.container_statuses.map(|statuses| {
                    statuses
                        .into_iter()
                        .map(|s| ContainerStatus {
                            restart_count: s.restart_count,
                            ..ContainerStatus::default()
                        })
                        .collect()
                }),
                ..PodStatus::default()
            }),
            ..Pod::default()
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ReplicaStatus {
    available_replicas: Option<i32>,
    ready_replicas: Option<i32>,
    replicas: Option<i32>,
}

impl DeploymentProjection {
    pub(super) fn into_resource(self) -> Deployment {
        let value = self.0;
        Deployment {
            metadata: value.metadata,
            status: value.status.map(|status| DeploymentStatus {
                available_replicas: status.available_replicas,
                ready_replicas: status.ready_replicas,
                replicas: status.replicas,
                ..DeploymentStatus::default()
            }),
            ..Deployment::default()
        }
    }
}

impl ReplicaSetProjection {
    pub(super) fn into_resource(self) -> ReplicaSet {
        let value = self.0;
        ReplicaSet {
            metadata: value.metadata,
            status: value.status.map(|status| ReplicaSetStatus {
                available_replicas: status.available_replicas,
                ready_replicas: status.ready_replicas,
                replicas: status.replicas.unwrap_or_default(),
                ..ReplicaSetStatus::default()
            }),
            ..ReplicaSet::default()
        }
    }
}

#[derive(Clone, Deserialize)]
pub(super) struct JobSummaryStatus {
    active: Option<i32>,
    failed: Option<i32>,
    succeeded: Option<i32>,
}

#[derive(Clone, Deserialize)]
pub(super) struct JobSummarySpec {
    completions: Option<i32>,
}

impl JobProjection {
    pub(super) fn into_resource(self) -> Job {
        let value = self.0;
        Job {
            metadata: value.metadata,
            spec: value.spec.map(|spec| JobSpec {
                completions: spec.completions,
                ..JobSpec::default()
            }),
            status: value.status.map(|status| JobStatus {
                active: status.active,
                failed: status.failed,
                succeeded: status.succeeded,
                ..JobStatus::default()
            }),
        }
    }
}

#[cfg(feature = "bench-support")]
pub fn decode_pod_list_summaries(
    text: &str,
    now: DateTime<Utc>,
) -> Result<Vec<crate::models::ResourceSummary>, serde_json::Error> {
    let list: kube::api::ObjectList<PodProjection> = serde_json::from_str(text)?;
    Ok(list
        .items
        .into_iter()
        .map(|pod| {
            super::summary_core::pod_resource_summary_at("benchmark", &pod.into_resource(), now)
        })
        .collect())
}

#[cfg(feature = "bench-support")]
pub fn decode_summary_projection(kind: &str, text: &str) -> Result<usize, serde_json::Error> {
    macro_rules! decode {
        ($projection:ty) => {{
            let list: kube::api::ObjectList<$projection> = serde_json::from_str(text)?;
            std::hint::black_box(list).items.len()
        }};
    }
    Ok(match kind {
        "Deployment" => decode!(DeploymentProjection),
        "ReplicaSet" => decode!(ReplicaSetProjection),
        "Job" => decode!(JobProjection),
        "Event" => decode!(crate::commands::event_projection::EventProjection),
        "Widget" => decode!(kubecove_kube::commands::list_projection::Dynamic),
        _ => unreachable!("benchmark kind"),
    })
}

#[cfg(test)]
#[path = "summary_projection_tests.rs"]
mod tests;
