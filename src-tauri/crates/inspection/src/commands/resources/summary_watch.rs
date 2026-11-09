//! The row a typed list holds for one watched object, so a watch event can
//! update a cached list without listing the kind again.
use super::builtin_summary::{BuiltinSummary, builtin_summary, metadata_summary};
use super::summary_projection::{
    DeploymentProjection, JobProjection, PodProjection, ReplicaSetProjection,
};
use crate::models::ResourceSummary;
use chrono::Utc;
use k8s_openapi::api::{
    apps::v1::{DaemonSet, StatefulSet},
    batch::v1::CronJob,
    core::v1::{ConfigMap, Node, PersistentVolume, PersistentVolumeClaim, Secret, Service},
    networking::v1::Ingress,
    storage::v1::StorageClass,
};
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::api::DynamicObject;
use serde::de::DeserializeOwned;

/// The row `resources_summary_from` lists for `object` of the typed `kind`.
/// Each kind goes through the projection its list decodes, so the row matches
/// a fresh list's row. `None` for a kind without a typed list or an object
/// that does not decode.
pub fn watched_resource_summary(
    kind: &str,
    cluster_context: &str,
    object: &DynamicObject,
) -> Option<ResourceSummary> {
    let now = Utc::now();
    fn decoded<T: DeserializeOwned>(object: &DynamicObject) -> Option<T> {
        serde_json::to_value(object)
            .and_then(serde_json::from_value)
            .ok()
    }
    fn typed<K: BuiltinSummary + DeserializeOwned>(
        cluster_context: &str,
        object: &DynamicObject,
        now: chrono::DateTime<Utc>,
    ) -> Option<ResourceSummary> {
        Some(builtin_summary(
            cluster_context,
            &decoded::<K>(object)?,
            now,
        ))
    }
    match kind {
        "Pod" => Some(builtin_summary(
            cluster_context,
            &decoded::<PodProjection>(object)?.into_resource(),
            now,
        )),
        "Deployment" => Some(builtin_summary(
            cluster_context,
            &decoded::<DeploymentProjection>(object)?.into_resource(),
            now,
        )),
        "ReplicaSet" => Some(builtin_summary(
            cluster_context,
            &decoded::<ReplicaSetProjection>(object)?.into_resource(),
            now,
        )),
        "Job" => Some(builtin_summary(
            cluster_context,
            &decoded::<JobProjection>(object)?.into_resource(),
            now,
        )),
        // Their lists read metadata only.
        "ConfigMap" => Some(metadata_summary::<ConfigMap>(
            cluster_context,
            &object.metadata,
            now,
        )),
        "Secret" => Some(metadata_summary::<Secret>(
            cluster_context,
            &object.metadata,
            now,
        )),
        "Service" => typed::<Service>(cluster_context, object, now),
        "PersistentVolumeClaim" => typed::<PersistentVolumeClaim>(cluster_context, object, now),
        "StatefulSet" => typed::<StatefulSet>(cluster_context, object, now),
        "DaemonSet" => typed::<DaemonSet>(cluster_context, object, now),
        "Ingress" => typed::<Ingress>(cluster_context, object, now),
        "CronJob" => typed::<CronJob>(cluster_context, object, now),
        "Node" => typed::<Node>(cluster_context, object, now),
        "StorageClass" => typed::<StorageClass>(cluster_context, object, now),
        "PersistentVolume" => typed::<PersistentVolume>(cluster_context, object, now),
        "CustomResourceDefinition" => {
            typed::<CustomResourceDefinition>(cluster_context, object, now)
        }
        _ => None,
    }
}
