use super::topology::{input_from_metadata, TopologyInputResource};
use crate::commands::helpers::{fmt_ready, list_params};
use crate::models::AppError;
use k8s_openapi::api::{
    apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet},
    batch::v1::{CronJob, Job},
    core::v1::{ConfigMap, PersistentVolumeClaim, Pod, Secret, Service},
    networking::v1::Ingress,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use k8s_openapi::NamespaceResourceScope;
use kube::{api::Api, Client};

fn inputs_from_metadata<T>(cluster_context: &str, items: Vec<T>) -> Vec<TopologyInputResource>
where
    T: k8s_openapi::Metadata<Ty = ObjectMeta>,
{
    items
        .iter()
        .map(|item| {
            input_from_metadata(
                cluster_context,
                <T as k8s_openapi::Resource>::KIND,
                <T as k8s_openapi::Resource>::API_VERSION,
                item.metadata(),
            )
        })
        .collect()
}

async fn list_namespaced<T>(client: Client, namespaces: &[String]) -> Result<Vec<T>, AppError>
where
    T: Clone
        + std::fmt::Debug
        + serde::de::DeserializeOwned
        + k8s_openapi::Metadata<Ty = ObjectMeta>
        + kube::Resource<DynamicType = (), Scope = NamespaceResourceScope>
        + Send
        + Sync
        + 'static,
{
    let mut out = Vec::new();
    if namespaces.is_empty() {
        let api: Api<T> = Api::all(client);
        out.extend(
            api.list(&list_params())
                .await
                .map_err(|e| AppError::kube(e.to_string()))?
                .items,
        );
        return Ok(out);
    }
    for namespace in namespaces {
        let api: Api<T> = Api::namespaced(client.clone(), namespace);
        out.extend(
            api.list(&list_params())
                .await
                .map_err(|e| AppError::kube(e.to_string()))?
                .items,
        );
    }
    Ok(out)
}

pub(super) async fn collect_topology_inputs(
    client: Client,
    cluster_context: &str,
    namespaces: &[String],
) -> Result<Vec<TopologyInputResource>, AppError> {
    let mut inputs = Vec::new();

    for deploy in list_namespaced::<Deployment>(client.clone(), namespaces).await? {
        let mut input =
            input_from_metadata(cluster_context, "Deployment", "apps/v1", &deploy.metadata);
        if let Some(status) = deploy.status {
            let ready = status.ready_replicas.unwrap_or(0);
            let desired = status.replicas.unwrap_or(0);
            input.summary.ready = Some(format!("{}/{}", ready, desired));
            input.summary.status = Some(format!(
                "Available: {}",
                status.available_replicas.unwrap_or(0)
            ));
        }
        inputs.push(input);
    }

    for ds in list_namespaced::<DaemonSet>(client.clone(), namespaces).await? {
        let mut input = input_from_metadata(cluster_context, "DaemonSet", "apps/v1", &ds.metadata);
        if let Some(status) = ds.status {
            input.summary.ready = Some(format!(
                "{}/{}",
                status.number_ready, status.desired_number_scheduled
            ));
            input.summary.status = Some(format!(
                "Available: {}",
                status.number_available.unwrap_or(0)
            ));
        }
        inputs.push(input);
    }

    for rs in list_namespaced::<ReplicaSet>(client.clone(), namespaces).await? {
        let mut input = input_from_metadata(cluster_context, "ReplicaSet", "apps/v1", &rs.metadata);
        if let Some(status) = rs.status {
            input.summary.ready = Some(fmt_ready(status.ready_replicas, status.replicas));
            input.summary.status = Some(format!(
                "Available: {}",
                status.available_replicas.unwrap_or(0)
            ));
        }
        inputs.push(input);
    }

    for ss in list_namespaced::<StatefulSet>(client.clone(), namespaces).await? {
        let mut input =
            input_from_metadata(cluster_context, "StatefulSet", "apps/v1", &ss.metadata);
        if let Some(status) = ss.status {
            input.summary.ready = Some(fmt_ready(status.ready_replicas, status.replicas));
        }
        inputs.push(input);
    }

    for cj in list_namespaced::<CronJob>(client.clone(), namespaces).await? {
        let mut input = input_from_metadata(cluster_context, "CronJob", "batch/v1", &cj.metadata);
        if let Some(status) = cj.status {
            let active = status
                .active
                .as_ref()
                .map(|active| active.len())
                .unwrap_or(0);
            if active > 0 {
                input.summary.status = Some(format!("{} active", active));
            }
        }
        inputs.push(input);
    }

    for job in list_namespaced::<Job>(client.clone(), namespaces).await? {
        let mut input = input_from_metadata(cluster_context, "Job", "batch/v1", &job.metadata);
        if let Some(status) = job.status {
            let active = status.active.unwrap_or(0);
            let failed = status.failed.unwrap_or(0);
            let succeeded = status.succeeded.unwrap_or(0);
            input.summary.status = if failed > 0 {
                Some("Failed".to_string())
            } else if succeeded > 0 {
                Some("Complete".to_string())
            } else if active > 0 {
                Some("Active".to_string())
            } else {
                Some("Pending".to_string())
            };
            input.summary.ready = Some(format!(
                "{}/{}",
                succeeded,
                job.spec
                    .as_ref()
                    .and_then(|spec| spec.completions)
                    .unwrap_or(1)
            ));
        }
        inputs.push(input);
    }

    for pod in list_namespaced::<Pod>(client.clone(), namespaces).await? {
        let mut input = input_from_metadata(cluster_context, "Pod", "v1", &pod.metadata);
        if let Some(status) = pod.status {
            input.summary.status = status.phase.filter(|phase| !phase.is_empty());
            input.summary.ready = status
                .conditions
                .as_ref()
                .and_then(|conds| conds.iter().find(|condition| condition.type_ == "Ready"))
                .map(|condition| condition.status.clone());
            let restarts: i32 = status
                .container_statuses
                .as_ref()
                .map(|statuses| {
                    statuses
                        .iter()
                        .map(|container| container.restart_count)
                        .sum()
                })
                .unwrap_or(0);
            if restarts > 0 {
                input.summary.restarts = Some(restarts);
            }
        }
        inputs.push(input);
    }

    inputs.extend(inputs_from_metadata(
        cluster_context,
        list_namespaced::<PersistentVolumeClaim>(client.clone(), namespaces).await?,
    ));
    inputs.extend(inputs_from_metadata(
        cluster_context,
        list_namespaced::<Service>(client.clone(), namespaces).await?,
    ));
    inputs.extend(inputs_from_metadata(
        cluster_context,
        list_namespaced::<Ingress>(client.clone(), namespaces).await?,
    ));
    inputs.extend(inputs_from_metadata(
        cluster_context,
        list_namespaced::<ConfigMap>(client.clone(), namespaces).await?,
    ));
    inputs.extend(inputs_from_metadata(
        cluster_context,
        list_namespaced::<Secret>(client, namespaces).await?,
    ));

    Ok(inputs)
}
