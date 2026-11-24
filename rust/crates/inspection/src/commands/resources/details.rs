use super::builtin_summary::{BuiltinSummary, builtin_summary};
use crate::commands::helpers::{
    fetch_cluster_resource, fetch_resource, normalize_k8s_yaml_value, redact_secret,
    resource_json_value, serialize_json_value_document,
};
use crate::commands::{BackendCancellationRegistry, ClusterAccess, FiniteRead, diagnostic_field};
use crate::models::AppErrorKind;
use crate::models::{AppError, ResourceDetailsFull, ResourceSummary, YamlEncoding, YamlViewMode};
use crate::runtime::State;
use chrono::{DateTime, Utc};
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet};
use k8s_openapi::api::batch::v1::{CronJob, Job};
use k8s_openapi::api::core::v1::{
    ConfigMap, Node, PersistentVolume, PersistentVolumeClaim, Pod, Secret, Service,
};
use k8s_openapi::api::networking::v1::Ingress;
use k8s_openapi::api::storage::v1::StorageClass;
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;

use serde_json::Value;

pub struct PreparedResourceDetails<D = Value> {
    pub summary: ResourceSummary,
    pub metadata: Value,
    pub status: Option<Value>,
    pub document: D,
}

impl<D> PreparedResourceDetails<D> {
    pub(super) fn map_document<T>(
        self,
        encode: impl FnOnce(D) -> Result<T, AppError>,
    ) -> Result<PreparedResourceDetails<T>, AppError> {
        Ok(PreparedResourceDetails {
            summary: self.summary,
            metadata: self.metadata,
            status: self.status,
            document: encode(self.document)?,
        })
    }
}

impl From<PreparedResourceDetails<String>> for ResourceDetailsFull {
    fn from(prepared: PreparedResourceDetails<String>) -> Self {
        Self {
            summary: prepared.summary,
            yaml: prepared.document,
            metadata: prepared.metadata,
            status: prepared.status,
        }
    }
}

enum BuiltinDocument {
    Normalized(Value),
    RedactedSecret(Box<Secret>),
}

impl BuiltinDocument {
    fn into_value(self) -> Result<Value, AppError> {
        match self {
            Self::Normalized(value) => Ok(value),
            Self::RedactedSecret(secret) => resource_json_value(&secret),
        }
    }

    fn into_yaml(self) -> Result<String, AppError> {
        match self {
            Self::Normalized(value) => serialize_json_value_document(&value, YamlEncoding::Yaml),
            Self::RedactedSecret(secret) => serde_yaml::to_string(&secret).map_err(|e| {
                AppError::new(e.to_string(), AppErrorKind::Serialization).with_source(e)
            }),
        }
    }
}

pub async fn resource_details_from(
    access: &ClusterAccess,
    cluster_context: String,
    kind: String,
    name: String,
    namespace: Option<String>,
) -> Result<ResourceDetailsFull, AppError> {
    read_builtin_details(access, &cluster_context, &kind, &name, namespace.as_deref())
        .await?
        .map_document(BuiltinDocument::into_yaml)
        .map(Into::into)
}

pub async fn resource_details_document_from(
    access: &ClusterAccess,
    cluster_context: &str,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
) -> Result<PreparedResourceDetails, AppError> {
    read_builtin_details(access, cluster_context, kind, name, namespace)
        .await?
        .map_document(BuiltinDocument::into_value)
}

async fn read_builtin_details(
    access: &ClusterAccess,
    cluster: &str,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
) -> Result<PreparedResourceDetails<BuiltinDocument>, AppError> {
    let client = access.read_client(cluster).await?;
    match kind {
        "Pod" => namespaced_details::<Pod>(client, cluster, name, namespace).await,
        "Service" => namespaced_details::<Service>(client, cluster, name, namespace).await,
        "ConfigMap" => namespaced_details::<ConfigMap>(client, cluster, name, namespace).await,
        "Secret" => {
            let secret = fetch_resource::<Secret>(client, namespace, name).await?;
            prepare_secret_details(cluster, secret, Utc::now())
        }
        "PersistentVolumeClaim" => {
            namespaced_details::<PersistentVolumeClaim>(client, cluster, name, namespace).await
        }
        "Deployment" => namespaced_details::<Deployment>(client, cluster, name, namespace).await,
        "ReplicaSet" => namespaced_details::<ReplicaSet>(client, cluster, name, namespace).await,
        "StatefulSet" => namespaced_details::<StatefulSet>(client, cluster, name, namespace).await,
        "DaemonSet" => namespaced_details::<DaemonSet>(client, cluster, name, namespace).await,
        "Ingress" => namespaced_details::<Ingress>(client, cluster, name, namespace).await,
        "Job" => namespaced_details::<Job>(client, cluster, name, namespace).await,
        "CronJob" => namespaced_details::<CronJob>(client, cluster, name, namespace).await,
        "Node" => cluster_details::<Node>(client, cluster, name).await,
        "StorageClass" => cluster_details::<StorageClass>(client, cluster, name).await,
        "PersistentVolume" => cluster_details::<PersistentVolume>(client, cluster, name).await,
        "CustomResourceDefinition" => {
            cluster_details::<CustomResourceDefinition>(client, cluster, name).await
        }
        _ => Err(AppError::new(
            format!("unsupported resource kind: {kind}"),
            AppErrorKind::Cluster,
        )),
    }
}

async fn namespaced_details<K>(
    client: kube::Client,
    cluster_context: &str,
    name: &str,
    namespace: Option<&str>,
) -> Result<PreparedResourceDetails<BuiltinDocument>, AppError>
where
    K: BuiltinSummary
        + kube::Resource<DynamicType = (), Scope = k8s_openapi::NamespaceResourceScope>
        + serde::Serialize
        + serde::de::DeserializeOwned
        + Clone
        + std::fmt::Debug
        + Send
        + Sync,
{
    let object = fetch_resource::<K>(client, namespace, name).await?;
    prepare_builtin_details(cluster_context, &object)
}

async fn cluster_details<K>(
    client: kube::Client,
    cluster_context: &str,
    name: &str,
) -> Result<PreparedResourceDetails<BuiltinDocument>, AppError>
where
    K: BuiltinSummary
        + kube::Resource<DynamicType = (), Scope = k8s_openapi::ClusterResourceScope>
        + serde::Serialize
        + serde::de::DeserializeOwned
        + Clone
        + std::fmt::Debug
        + Send
        + Sync,
{
    let object = fetch_cluster_resource::<K>(client, name).await?;
    prepare_builtin_details(cluster_context, &object)
}

fn prepare_builtin_details<K: BuiltinSummary + serde::Serialize>(
    cluster_context: &str,
    object: &K,
) -> Result<PreparedResourceDetails<BuiltinDocument>, AppError> {
    let mut document = resource_json_value(object)?;
    normalize_k8s_yaml_value(&mut document, YamlViewMode::Kubectl);
    details_from_fetched_with_document(
        cluster_context,
        object,
        BuiltinDocument::Normalized(document),
        Utc::now(),
    )
}

fn details_from_fetched_with_document<K: BuiltinSummary, D>(
    cluster_context: &str,
    object: &K,
    document: D,
    now: DateTime<Utc>,
) -> Result<PreparedResourceDetails<D>, AppError> {
    let serialization = |e: serde_json::Error| {
        AppError::new(e.to_string(), AppErrorKind::Serialization).with_source(e)
    };
    Ok(PreparedResourceDetails {
        summary: builtin_summary(cluster_context, object, now),
        document,
        metadata: serde_json::to_value(object.metadata()).map_err(serialization)?,
        status: object.raw_status().transpose().map_err(serialization)?,
    })
}

#[cfg(feature = "bench-support")]
pub fn details_from_fetched<K: BuiltinSummary>(
    cluster_context: &str,
    object: &K,
    yaml: String,
    now: DateTime<Utc>,
) -> Result<ResourceDetailsFull, AppError> {
    details_from_fetched_with_document(cluster_context, object, yaml, now).map(Into::into)
}

fn prepare_secret_details(
    cluster_context: &str,
    mut secret: Secret,
    now: DateTime<Utc>,
) -> Result<PreparedResourceDetails<BuiltinDocument>, AppError> {
    redact_secret(&mut secret);
    details_from_fetched_with_document(cluster_context, &secret, (), now)?
        .map_document(|()| Ok(BuiltinDocument::RedactedSecret(Box::new(secret))))
}

#[cfg(feature = "bench-support")]
pub fn secret_details_from_fetched(
    cluster_context: &str,
    secret: Secret,
    now: DateTime<Utc>,
) -> Result<ResourceDetailsFull, AppError> {
    prepare_secret_details(cluster_context, secret, now)?
        .map_document(BuiltinDocument::into_yaml)
        .map(Into::into)
}

pub async fn get_resource_details(
    cluster_context: String,
    kind: String,
    name: String,
    namespace: Option<String>,
    kubeconfig_env_var: Option<String>,
    request_id: Option<String>,
    cancel_scope: Option<String>,
    cancellations: State<'_, BackendCancellationRegistry>,
) -> Result<ResourceDetailsFull, AppError> {
    FiniteRead::new("get_resource_details")
        .label("context", &cluster_context)
        .label("kind", &kind)
        .label("namespace", namespace.as_deref().unwrap_or("<cluster>"))
        .label("name", &name)
        .cancellable(&cancellations, cancel_scope, request_id)
        .run(
            {
                let kind = kind.clone();
                async move {
                    let access = ClusterAccess::resolve(kubeconfig_env_var.as_deref())?;
                    resource_details_from(&access, cluster_context, kind, name, namespace).await
                }
            },
            |details| {
                vec![
                    diagnostic_field("kind", &kind),
                    diagnostic_field("yamlBytes", details.yaml.len()),
                    diagnostic_field("hasStatus", details.status.is_some()),
                ]
            },
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::FixedClusterAccess;
    use http::{Request, Response};
    use kube::{Client, client::Body};
    use serde_json::json;

    #[tokio::test]
    async fn replicaset_details_load_from_namespaced_api() {
        for status in [
            Some(json!({"replicas": 3, "readyReplicas": 2, "availableReplicas": 1})),
            None,
        ] {
            let object = json!({
                "apiVersion": "apps/v1",
                "kind": "ReplicaSet",
                "metadata": {
                    "name": "calagopus-5f554d575f",
                    "namespace": "calagopus",
                    "ownerReferences": [{
                        "apiVersion": "apps/v1", "kind": "Deployment",
                        "name": "calagopus", "uid": "deployment-uid", "controller": true
                    }]
                },
                "status": status
            });
            let service = tower::service_fn(move |request: Request<Body>| {
                assert_eq!(request.method(), http::Method::GET);
                assert_eq!(
                    request.uri().path(),
                    "/apis/apps/v1/namespaces/calagopus/replicasets/calagopus-5f554d575f"
                );
                let body = object.to_string().into_bytes();
                async move { Ok::<_, std::convert::Infallible>(Response::new(Body::from(body))) }
            });
            let access = FixedClusterAccess::new(Client::new(service, "default")).access();
            let details = resource_details_from(
                &access,
                "test-cluster".into(),
                "ReplicaSet".into(),
                "calagopus-5f554d575f".into(),
                Some("calagopus".into()),
            )
            .await
            .expect("ReplicaSet details should load");
            assert_eq!(details.summary.kind, "ReplicaSet");
            assert_eq!(details.summary.cluster, "test-cluster");
            assert_eq!(details.summary.name, "calagopus-5f554d575f");
            assert_eq!(details.summary.namespace.as_deref(), Some("calagopus"));
            assert_eq!(details.summary.owner_ref.as_deref(), Some("calagopus"));
            assert_eq!(
                details.summary.ready.as_deref(),
                status.as_ref().map(|_| "2/3")
            );
            assert_eq!(
                details.summary.status.as_deref(),
                status.as_ref().map(|_| "Starting")
            );
            assert_eq!(details.status, status);
            assert_eq!(details.metadata["name"], "calagopus-5f554d575f");
            let yaml: serde_json::Value = serde_yaml::from_str(&details.yaml).unwrap();
            assert_eq!(yaml["kind"], "ReplicaSet");
            let document = super::super::yaml::resource_yaml_from(
                &access,
                "test-cluster".into(),
                "ReplicaSet".into(),
                "calagopus-5f554d575f".into(),
                Some("calagopus".into()),
                None,
                None,
            )
            .await
            .expect("ReplicaSet YAML should load");
            let document: serde_json::Value = serde_yaml::from_str(&document).unwrap();
            assert_eq!(document, yaml);
        }
    }
}
