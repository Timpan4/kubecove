use super::builtin_summary::{builtin_summary, metadata_summary, BuiltinSummary, KindStatus};
use crate::commands::helpers::{kubernetes_health, list_params, pod_restarts, Readiness};
use crate::models::{AppError, ResourceSummary};
use chrono::Utc;
use k8s_openapi::api::core::v1::{ConfigMap, PersistentVolumeClaim, Pod, Secret, Service};
use k8s_openapi::api::discovery::v1::EndpointSlice;
use kube::{api::Api, Client};
use serde_json::Value;

impl BuiltinSummary for Pod {
    fn kind_status(&self) -> Option<KindStatus> {
        let Some(status) = &self.status else {
            return Some(KindStatus::default());
        };
        let phase = status.phase.clone().filter(|phase| !phase.is_empty());
        let ready = status
            .conditions
            .as_ref()
            .and_then(|conditions| {
                conditions
                    .iter()
                    .find(|condition| condition.type_ == "Ready")
            })
            .map(|condition| condition.status.clone());
        let readiness = ready
            .as_deref()
            .map_or(Readiness::Absent, Readiness::Condition);
        Some(KindStatus {
            health: kubernetes_health(phase.as_deref(), readiness),
            status: phase,
            ready,
            restarts: pod_restarts(status.container_statuses.as_deref()),
        })
    }

    fn raw_status(&self) -> Option<serde_json::Result<Value>> {
        self.status.as_ref().map(serde_json::to_value)
    }
}

impl BuiltinSummary for Service {
    fn raw_status(&self) -> Option<serde_json::Result<Value>> {
        self.status.as_ref().map(serde_json::to_value)
    }
}

impl BuiltinSummary for ConfigMap {}

impl BuiltinSummary for Secret {}

impl BuiltinSummary for PersistentVolumeClaim {
    fn raw_status(&self) -> Option<serde_json::Result<Value>> {
        self.status.as_ref().map(serde_json::to_value)
    }
}

impl BuiltinSummary for EndpointSlice {}

pub(super) async fn core_resource_summaries(
    client: Client,
    cluster_context: &str,
    kind: &str,
    namespace: Option<&str>,
) -> Result<Option<Vec<ResourceSummary>>, AppError> {
    let now = Utc::now();
    let summaries = match kind {
        "Pod" => {
            let api: Api<Pod> = if let Some(ns) = namespace {
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
            .map(|pod| builtin_summary(cluster_context, &pod.into_resource(), now))
            .collect()
        }
        "Service" => {
            let api: Api<Service> = if let Some(ns) = namespace {
                Api::namespaced(client, ns)
            } else {
                Api::all(client)
            };
            api.list(&list_params())
                .await
                .map_err(AppError::from)?
                .iter()
                .map(|svc| builtin_summary(cluster_context, svc, now))
                .collect()
        }
        "ConfigMap" => {
            let api: Api<ConfigMap> = if let Some(ns) = namespace {
                Api::namespaced(client, ns)
            } else {
                Api::all(client)
            };
            api.list_metadata(&list_params())
                .await
                .map_err(AppError::from)?
                .iter()
                .map(|cm| metadata_summary::<ConfigMap>(cluster_context, &cm.metadata, now))
                .collect()
        }
        "Secret" => {
            let api: Api<Secret> = if let Some(ns) = namespace {
                Api::namespaced(client, ns)
            } else {
                Api::all(client)
            };
            api.list_metadata(&list_params())
                .await
                .map_err(AppError::from)?
                .iter()
                .map(|sec| metadata_summary::<Secret>(cluster_context, &sec.metadata, now))
                .collect()
        }
        "PersistentVolumeClaim" => {
            let api: Api<PersistentVolumeClaim> = if let Some(ns) = namespace {
                Api::namespaced(client, ns)
            } else {
                Api::all(client)
            };
            api.list(&list_params())
                .await
                .map_err(AppError::from)?
                .iter()
                .map(|pvc| builtin_summary(cluster_context, pvc, now))
                .collect()
        }
        _ => return Ok(None),
    };

    Ok(Some(summaries))
}
