use super::builtin_summary::{builtin_summary, BuiltinSummary, KindStatus};
use crate::commands::helpers::{kubernetes_health, list_params, Readiness};
use crate::models::{AppError, ResourceHealth, ResourceSummary};
use chrono::{DateTime, Utc};
use k8s_openapi::api::core::v1::{Node, PersistentVolume};
use k8s_openapi::api::storage::v1::StorageClass;
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use k8s_openapi::Resource;
use kube::{api::Api, Client};
use serde_json::Value;

impl BuiltinSummary for Node {
    fn kind_status(&self) -> Option<KindStatus> {
        let Some(conditions) = self
            .status
            .as_ref()
            .and_then(|status| status.conditions.as_ref())
        else {
            return Some(KindStatus::default());
        };
        let ready_count = conditions
            .iter()
            .filter(|condition| condition.type_ == "Ready" && condition.status == "True")
            .count();
        let status = conditions
            .iter()
            .find(|condition| condition.type_ == "Ready")
            .map(|condition| condition.status.clone());
        let readiness = i32::try_from(ready_count).map_or(Readiness::Absent, |ready| {
            Readiness::Counts { ready, desired: 1 }
        });
        Some(KindStatus {
            health: kubernetes_health(status.as_deref(), readiness),
            status,
            ready: Some(format!("{ready_count}/1")),
            restarts: None,
        })
    }

    fn raw_status(&self) -> Option<serde_json::Result<Value>> {
        self.status.as_ref().map(serde_json::to_value)
    }
}

impl BuiltinSummary for StorageClass {}

impl BuiltinSummary for PersistentVolume {
    fn kind_status(&self) -> Option<KindStatus> {
        let phase = self.status.as_ref().and_then(|status| status.phase.clone());
        Some(KindStatus {
            health: kubernetes_health(phase.as_deref(), Readiness::Absent),
            status: phase,
            ..KindStatus::default()
        })
    }

    fn raw_status(&self) -> Option<serde_json::Result<Value>> {
        self.status.as_ref().map(serde_json::to_value)
    }
}

impl BuiltinSummary for CustomResourceDefinition {
    fn kind_status(&self) -> Option<KindStatus> {
        let conditions = self
            .status
            .as_ref()
            .and_then(|status| status.conditions.as_ref());
        let condition = conditions.and_then(|conditions| {
            conditions
                .iter()
                .find(|condition| condition.type_ == "Established")
                .or_else(|| conditions.first())
        });
        // CRDs report Established rather than Ready, so no condition classifies their health.
        Some(KindStatus {
            status: condition.map(|condition| format!("{}: {}", condition.type_, condition.status)),
            health: ResourceHealth::Unknown,
            ..KindStatus::default()
        })
    }

    fn raw_status(&self) -> Option<serde_json::Result<Value>> {
        self.status.as_ref().map(serde_json::to_value)
    }

    fn add_identity(summary: &mut ResourceSummary) {
        summary.group = Some(Self::GROUP.to_string());
        summary.version = Some(Self::VERSION.to_string());
        summary.plural = Some(Self::URL_PATH_SEGMENT.to_string());
        summary.namespaced = Some(false);
    }
}

pub(super) async fn cluster_resource_summaries(
    client: Client,
    cluster_context: &str,
    kind: &str,
) -> Result<Option<Vec<ResourceSummary>>, AppError> {
    let now = Utc::now();
    let summaries = match kind {
        "Node" => list_summaries::<Node>(client, cluster_context, now).await?,
        "StorageClass" => list_summaries::<StorageClass>(client, cluster_context, now).await?,
        "PersistentVolume" => {
            list_summaries::<PersistentVolume>(client, cluster_context, now).await?
        }
        "CustomResourceDefinition" => {
            list_summaries::<CustomResourceDefinition>(client, cluster_context, now).await?
        }
        _ => return Ok(None),
    };

    Ok(Some(summaries))
}

async fn list_summaries<K>(
    client: Client,
    cluster_context: &str,
    now: DateTime<Utc>,
) -> Result<Vec<ResourceSummary>, AppError>
where
    K: BuiltinSummary
        + kube::Resource<DynamicType = (), Scope = k8s_openapi::ClusterResourceScope>
        + Clone
        + std::fmt::Debug
        + serde::de::DeserializeOwned,
{
    let api: Api<K> = Api::all(client);
    Ok(api
        .list(&list_params())
        .await
        .map_err(AppError::from)?
        .iter()
        .map(|object| builtin_summary(cluster_context, object, now))
        .collect())
}
